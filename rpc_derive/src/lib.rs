/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

mod attributes;
mod input;

mod request;
mod rpc;
mod service;
mod stub;

mod javascript;
mod python;

mod case_conversion;
mod protocol;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn rpc(attr: TokenStream, input: TokenStream) -> TokenStream {
    rpc::impl_rpc(attr, input)
}
