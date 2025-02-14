/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, FnArg, Ident, Signature, TraitItemFn};

use crate::attributes::ServiceAttrs;

use super::attributes::Attributes;
use super::input::{Input, Method};

pub fn impl_rpc_service(
    Attributes {
        err_type,
        log_errors,
        ..
    }: &Attributes,
    ServiceAttrs {
        session,
        shutdown,
        extra_args,
        lock_debug_task_names,
        ..
    }: &ServiceAttrs,
    Input {
        service_name,
        methods,
        ..
    }: &Input,
    msg_type_name: &Ident,
    proto_type_name: &Ident,
    handler_type_name: &Ident,
    err_type_name: &Ident,
) -> TokenStream {
    let extra_args_names = extra_args
        .as_ref()
        .map_or_else(Vec::new, |s| s.names.clone());
    let extra_args_types = extra_args
        .as_ref()
        .map_or_else(Vec::new, |s| s.types.clone());
    let extra_args_signature = extra_args
        .as_ref()
        .map_or_else(Vec::new, |s| s.signature.clone());

    let from_err = match err_type.is_some() {
        true => {
            quote! {Self::Error: std::convert::Into<#err_type_name>}
        }
        false => quote! {},
    };

    let to_err = match err_type.is_some() {
        true => quote! {.map_err(|e| e.to_string())},
        false => quote! {.map_err(|e| e.to_string())},
    };

    let from_serialize_err = match err_type.is_some() {
        true => quote! {String: std::convert::Into<#err_type_name>},
        false => quote! {},
    };

    let req_matches: Vec<_> = methods
        .iter()
        .map(
            |Method {
                 var_fields,
                 var_name,
                 fn_name,
                 fields,
                 ..
            }| {
                let req = match session {
					false => quote!(self.0.#fn_name(#(#extra_args_names,)* #(#fields),*)),
					true => quote!(self.0.#fn_name(session, #(#extra_args_names,)* #(#fields),*)),
				};
				let task_name = fn_name.to_string();
				let req = match lock_debug_task_names {
                    true => {
						quote!(lock_debug::tokio::debug::run_task(#task_name, #req))
					},
					false => req,
                };
				#[cfg(feature = "tracing")]
				let req = quote!(#req.instrument(rpc::tracing::span!(rpc::tracing::Level::INFO, #task_name, otel.kind="server")));
				let req = quote!(#req.await #to_err);
				let req = match log_errors {
					true => {
						let service_name = msg_type_name.to_string();
						let reqfmt = format!("{}({})", fn_name, fields.iter()
											 .map(|_| "{:?}")
											 .collect::<Vec<_>>().join(", "));
						quote!({
							let reqstr = format!(#reqfmt, #(#fields),*);
							match #req {
								Ok(v) => Ok(v),
								Err(e) => {
									log::warn!("{}: failed rpc handler: {}{} -> Err({:?})",
											   #service_name, &reqstr[..reqstr.len().min(100)],
											   if reqstr.len() > 100 {"..."} else {""},
											   e);
									Err(e)
								},
							}
						})
					},
					false => req
				};
				let req = quote!(V::serialize_from(#req #to_err));
				match var_fields.is_empty() {
                    true => quote!(#msg_type_name::#var_name => #req),
                    false => quote!(#msg_type_name::#var_name { #(#fields),* } => #req)
                }
            },
        )
        .collect();

    let trait_methods: Vec<_> = methods
        .iter()
        .map(|Method { method, .. }| TraitItemFn {
            sig: Signature {
				asyncness: None,
                inputs: {
                    let mut iter = method.sig.inputs.clone().into_iter();
                    iter.next()
                        .into_iter()
                        .chain(
                            session
                                .then(|| parse_quote!(session: &Self::Session)),
                        )
                        .chain(extra_args_signature.clone())
                        .chain(iter)
                        .collect()
                },
                output: match &method.sig.output {
                    syn::ReturnType::Default => {
                        parse_quote!(-> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send)
                    }
                    syn::ReturnType::Type(_, t) => {
                        parse_quote!(-> impl std::future::Future<Output = std::result::Result<#t, Self::Error>> + Send)
                    }
                },
                ..method.sig.clone()
            },
            default: None,
            ..method.clone()
        })
        .collect();

    let trait_methods_proxy: Vec<_> = trait_methods
        .iter()
        .map(
            |TraitItemFn {
                 sig: sig @ Signature { ident, inputs, .. },
                 ..
             }| {
                let args = inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        FnArg::Receiver(_) => None,
                        FnArg::Typed(arg) => Some(&arg.pat),
                    })
                    .collect::<Vec<_>>();
                quote! {
                    #sig {
                        self.as_ref().#ident(#(#args),*)
                    }
                }
            },
        )
        .collect();

    let shutdown_impl = match shutdown {
        true => quote! {
            impl<T> rpc::Shutdown for #handler_type_name<T>
            where T: rpc::Shutdown {
                type Error = T::Error;
                async fn shutdown(self) -> std::result::Result<(), Self::Error> {
                    T::shutdown(self.0).await
                }
            }
        },
        false => quote! {
            impl<T> rpc::Shutdown for #handler_type_name<T>
            where T: Send + Sync {
                type Error = std::convert::Infallible;
                async fn shutdown(self) -> std::result::Result<(), Self::Error> {
                    Ok(())
                }
            }
        },
    };

    #[cfg(not(feature = "tracing"))]
    let imports = quote!();
    #[cfg(feature = "tracing")]
    let imports = quote!(
        use rpc::tracing::instrument::Instrument;
    );

    match session {
        false => quote! {

            pub trait #service_name : Send + Sync where #from_err {
                type Error : std::error::Error + Send + Sync + 'static;
                #(#trait_methods)*
            }

            pub struct #handler_type_name<T>(T);

            impl<T> #handler_type_name<T>
            where T: #service_name + Sync {
                pub fn new(service: T) -> Self {
                    Self(service)
                }

                pub fn inner(&self) -> &T {
                    &self.0
                }

                pub fn into_inner(self) -> T {
                    self.0
                }
            }

            impl<T,V> rpc::RequestHandler<#proto_type_name,V> for #handler_type_name<T>
            where T: #service_name, V: rpc::GenericValue, #from_serialize_err {
                type ExtraArgs = ( #(#extra_args_types, )* );
                type Error = V::Error;
                async fn handle(&self, req: V, (#(#extra_args_names,)*): Self::ExtraArgs) -> std::result::Result<V,V::Error> {
                    #imports
                    match req.deserialize_into() {
                        Ok(req) => match req { #(#req_matches),* },
                        Err(e) => V::serialize_from::<std::result::Result<(),#err_type_name>>(
                            Err(e.to_string().into()))
                    }
                }
            }

            #shutdown_impl

            impl<T> #service_name for std::sync::Arc<T>
            where T: #service_name {
                type Error = T::Error;
                #(#trait_methods_proxy)*
            }

        },
        true => quote! {

            pub trait #service_name : Send + Sync where #from_err {
                type Session : Send + Sync;
                type Error : std::error::Error + Send + Sync + 'static;
                #(#trait_methods)*
            }

            /* A handler type is defined to work around rust's orphan
             * rules. These rules disallow implementing implementing a
             * foreign trait for a type not covered by a local type,
             * making it impossible to implement
             * rpc::ServerRequestHandler for any T where T:
             * #service_name (E0210). Therefore, we define a local
             * type here to wrap the T parameter, and implement the
             * trait for the wrapped type.

             * Another possible workaround would be to provide a
             * separate proc-macro to implement the trait for the
             * specific (locally-defined) type.
             */

            pub struct #handler_type_name<T>(T);

            impl<T> #handler_type_name<T>
            where T: #service_name {
                pub fn new(service: T) -> Self {
                    Self(service)
                }

                pub fn inner(&self) -> &T {
                    &self.0
                }

                pub fn into_inner(self) -> T {
                    self.0
                }
            }

            impl<T,V,S> rpc::SessionHandler<#proto_type_name,V,S> for #handler_type_name<T>
            where #handler_type_name<T>: rpc::ServerRequestHandler<#proto_type_name,V, Session = T::Session> + 'static,
                  T: rpc::SessionHandler<#proto_type_name,V,S>,
                  V: rpc::GenericValue, S: Send + Sync
            {
                type Session = T::Session;
                type Error = T::Error;
                async fn session(&self, info: &S) -> std::result::Result<Self::Session,Self::Error> {
                    self.0.session(info).await
                }
            }

            impl<T,V> rpc::ServerRequestHandler<#proto_type_name,V> for #handler_type_name<T>
            where T: #service_name + Send + Sync, V: rpc::GenericValue, #from_serialize_err {
                type Session = T::Session;
                type ExtraArgs = ( #(#extra_args_types, )* );
                type Error = V::Error;
                async fn handle_with_session(&self, session: &Self::Session, req: V, (#(#extra_args_names,)*): Self::ExtraArgs) -> std::result::Result<V,V::Error> {
                    #imports
                    match req.deserialize_into() {
                        Ok(req) => match req { #(#req_matches),* },
                        Err(e) => V::serialize_from::<std::result::Result<(),#err_type_name>>(
                            Err(e.to_string().into()))
                    }
                }
            }

            #shutdown_impl

            impl<T> #service_name for std::sync::Arc<T>
            where T: #service_name {
                type Session = T::Session;
                type Error = T::Error;
                #(#trait_methods_proxy)*
            }

        },
    }
}
