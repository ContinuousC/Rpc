/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub fn impl_rpc_proto(
    _msg_type_name: &Ident,
    proto_type_name: &Ident,
    _err_type_name: &Ident,
) -> TokenStream {
    quote! {

        #[derive(Clone, Copy)]
        pub struct #proto_type_name;

        impl rpc::Protocol for #proto_type_name {
            // type Req = Value;
            // type Res = std::result::Result<Value,#err_type_name>;
        }
    }
}
