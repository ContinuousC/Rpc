/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Ident, ImplItemFn, Signature};

use crate::attributes::ServiceAttrs;

use super::attributes::Attributes;
use super::input::{Input, Method};

pub fn impl_rpc_stub(
    Attributes { log_errors, .. }: &Attributes,
    ServiceAttrs { extra_args, .. }: &ServiceAttrs,
    Input {
        service_name,
        methods,
        ..
    }: &Input,
    msg_type_name: &Ident,
    proto_type_name: &Ident,
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

    let stub_name = Ident::new(
        &format!("{}Stub", service_name),
        proc_macro2::Span::call_site(),
    );

    let impl_methods: Vec<_> = methods
        .iter()
        .map(
            |Method {
                method,
                fields,
				fn_name,
                var_name,
                ..
            }| ImplItemFn {
				#[cfg(not(feature="tracing"))]
                attrs: Vec::new(),
				#[cfg(feature="tracing")]
                attrs: vec![parse_quote!(#[rpc::tracing::instrument(skip_all, fields(otel.kind="client"))])],
                vis: parse_quote!(pub),
                defaultness: None,
                sig: Signature {
                    inputs: {
                        let mut iter = method.sig.inputs.clone().into_iter();
                        iter.next()
                            .into_iter()
                            .chain(extra_args_signature.clone())
                            .chain(iter)
                            .collect()
                    },
                    output: match &method.sig.output {
                        syn::ReturnType::Default => {
                            parse_quote!(-> std::result::Result<(),String>)
                        }
                        syn::ReturnType::Type(_, t) => {
                            parse_quote!(-> std::result::Result<#t,String>)
                        }
                    },
                    ..method.sig.clone()
                },
                block: {
                    let req = match fields.is_empty() {
                        true => quote!(#msg_type_name::#var_name),
                        false => {
                            quote!(#msg_type_name::#var_name { #(#fields),* })
                        }
                    };
                    let req = quote!(self.0.handle(
                        <V as rpc::GenericValue>::serialize_from(#req)
                            .map_err(|e| e.to_string())?,
                        (#(#extra_args_names,)*)).await
                    );
					let req = match log_errors {
						true => {
							let service_name = msg_type_name.to_string();
							quote!({
								match #req {
									Ok(v) => Ok(v),
									Err(e) => {
										log::warn!("{}: failed rpc request (rpc error): \
													{}{} -> Err({})",
												   #service_name, &reqstr[..reqstr.len().min(100)],
												   if reqstr.len() > 100 { "..." } else { "" },
												   e);
										Err(e)
									}
								}
							})
						},
						false => req
					};
					let req = quote!(<V as rpc::GenericValue>::deserialize_into(
                        #req.map_err(|e| e.to_string())?
                    ).map_err(|e| e.to_string())?);
					match log_errors {
						true => {
							let service_name = msg_type_name.to_string();
							let reqfmt = format!("{}({})", fn_name, fields.iter()
												 .map(|_| "{:?}")
												 .collect::<Vec<_>>().join(", "));
							parse_quote!{{
								let reqstr = format!(#reqfmt, #(#fields),*);
								match #req {
									Ok(v) => Ok(v),
									Err(e) => {
										log::warn!("{}: failed rpc request (received error response): \
													{}{} -> Err({})",
												   #service_name, &reqstr[..reqstr.len().min(100)],
												   if reqstr.len() > 100 { "..." } else { "" },
												   e);
										Err(e)
									}
								}
							}}
						},
						false => parse_quote! {{#req}}
					}
                },
            },
        )
        .collect();

    quote! {

        pub struct #stub_name<T,V>(T, std::marker::PhantomData<V>);

        //  pub trait #stub_name {

        //      type Error : std::error::Error + Send + Sync + 'static;

        // //     async fn request<T>(&self, #(#extra_args_sig,)*
        // //         req: #msg_type_name) -> Result<T,Self::Error>
        // //     where T: serde::de::DeserializeOwned;

        //      #(#trait_methods)*

        //  }

        impl<T,V> #stub_name<T,V>
        where T: rpc::RequestHandler<#proto_type_name, V,
                                     ExtraArgs = (#(#extra_args_types, )*)>,
              V: rpc::GenericValue + Send,
              V::Error: Send + Sync,
              <T as rpc::RequestHandler<#proto_type_name, V>>::Error: std::fmt::Display + Send + Sync
        {
            //type Error = V::Error;

            pub fn new(handler: T) -> Self {
                Self(handler, std::marker::PhantomData)
            }

            pub fn inner(&self) -> &T {
                &self.0
            }

            pub fn into_inner(self) -> T {
                self.0
            }

            #(#impl_methods)*

        }

    }
}
