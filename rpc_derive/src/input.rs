/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, Ident, ItemTrait, Pat, TraitItem, TraitItemFn};

use super::case_conversion::pascal_case;

pub struct Input {
    pub service_name: Ident,
    pub methods: Vec<Method>,
}

pub struct Method {
    pub fn_name: Ident,
    pub var_name: Ident,
    pub var_fields: Vec<TokenStream>,
    pub fields: Vec<TokenStream>,
    pub arg_list: Vec<String>,
    pub method: TraitItemFn,
}

impl Input {
    pub fn parse(input: ItemTrait) -> Self {
        Self {
            service_name: input.ident.clone(),
            methods: input
                .items
                .iter()
                .map(|item| match item {
                    TraitItem::Fn(method) => {
                        let fn_name = method.sig.ident.clone();
                        let var_name = Ident::new(
                            &pascal_case(fn_name.to_string().as_str()),
                            proc_macro2::Span::call_site(),
                        );

                        let mut var_fields = Vec::new();
                        let mut fields = Vec::new();
                        let mut arg_list = Vec::new();

                        for arg in method.sig.inputs.iter() {
                            if let FnArg::Typed(typed_arg) = arg {
                                if let Pat::Ident(pat) = typed_arg.pat.as_ref()
                                {
                                    let field_name = &pat.ident;
                                    let field_type = typed_arg.ty.as_ref();

                                    let (msg, _non_msg): (Vec<_>, Vec<_>) =
                                        typed_arg.attrs.iter().partition(|a| {
                                            a.path().is_ident("msg")
                                        });

                                    let field_attrs: Vec<_> = msg
					.iter()
					.map(|a| {
					    let tokens: proc_macro2::TokenStream =
						a.parse_args().unwrap();
					    quote! {#[#tokens]}
					})
					.collect();

                                    var_fields.push(
					quote! {#(#field_attrs)* #field_name: #field_type},
				    );
                                    fields.push(quote! {#field_name});
                                    arg_list.push(field_name.to_string());
                                }
                            }
                        }

                        Method {
                            var_name,
                            fn_name,
                            var_fields,
                            fields,
                            arg_list,
                            method: method.clone(),
                        }
                    }
                    _ => panic!("Unexpected item in rpc trait"),
                })
                .collect(),
        }
    }
}
