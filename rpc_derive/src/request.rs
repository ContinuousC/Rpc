/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::attributes::Attributes;
use super::input::{Input, Method};

pub fn def_request_type(
    Attributes { .. }: &Attributes,
    Input { methods, .. }: &Input,
    msg_type_name: &Ident,
    derive_schemars: &TokenStream,
) -> TokenStream {
    let msg_vars: Vec<_> = methods
        .iter()
        .map(
            |Method {
                 var_name,
                 var_fields,
                 ..
             }| match var_fields.is_empty() {
                true => quote! {
                    #var_name
                },
                false => quote! {
                    #var_name {
                    #(#var_fields),*
                    }
                },
            },
        )
        .collect();

    quote! {
    #[derive(serde::Serialize,serde::Deserialize,Debug)]
        #derive_schemars
        #[serde(rename_all = "snake_case")]
        pub enum #msg_type_name {
            #(#msg_vars),*
        }
    }
}
