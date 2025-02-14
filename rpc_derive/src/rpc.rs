/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use darling::export::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
use quote::quote;
use syn::{self, parse_macro_input, Ident, ItemTrait};

use crate::protocol::impl_rpc_proto;

use super::attributes::Attributes;
use super::input::Input;
use super::javascript::{javascript_service, javascript_stub};
use super::python::{python_service, python_stub};
use super::request::def_request_type;
use super::service::impl_rpc_service;
use super::stub::impl_rpc_stub;

pub fn impl_rpc(attrs: TokenStream, input: TokenStream) -> TokenStream {
    let attrs = Attributes::from_list(
        &NestedMeta::parse_meta_list(attrs.into()).unwrap(),
    )
    .expect("failed to parse attributes");
    let input = Input::parse(parse_macro_input!(input as ItemTrait));

    #[cfg(feature = "schemars")]
    let derive_schemars = quote! {#[derive(schemars::JsonSchema)]};
    #[cfg(not(feature = "schemars"))]
    let derive_schemars = quote! {};

    let msg_type_name = Ident::new(
        &match &attrs.msg_type {
            Some(name) => name.to_string(),
            None => {
                let service_name = input.service_name.to_string();
                format!(
                    "{}Request",
                    service_name
                        .strip_suffix("Service")
                        .unwrap_or(&service_name)
                )
            }
        },
        proc_macro2::Span::call_site(),
    );

    let err_type_name = Ident::new(
        &match &attrs.err_type {
            Some(name) => name.to_string(),
            None => "String".to_string(),
        },
        proc_macro2::Span::call_site(),
    );

    let proto_type_name = Ident::new(
        &match &attrs.proto_type {
            Some(name) => name.to_string(),
            None => {
                let service_name = input.service_name.to_string();
                format!(
                    "{}Proto",
                    service_name
                        .strip_suffix("Service")
                        .unwrap_or(&service_name)
                )
            }
        },
        proc_macro2::Span::call_site(),
    );

    let handler_type_name = Ident::new(
        &match &attrs.proto_type {
            Some(name) => name.to_string(),
            None => {
                let service_name = input.service_name.to_string();
                format!(
                    "{}Handler",
                    service_name
                        .strip_suffix("Service")
                        .unwrap_or(&service_name)
                )
            }
        },
        proc_macro2::Span::call_site(),
    );

    let mut output = proc_macro2::TokenStream::new();

    output.extend(impl_rpc_proto(
        &msg_type_name,
        &proto_type_name,
        &err_type_name,
    ));

    if let Some(service_attrs) = &attrs.service {
        output.extend(impl_rpc_service(
            &attrs,
            &service_attrs.clone().unwrap_or_default(),
            &input,
            &msg_type_name,
            &proto_type_name,
            &handler_type_name,
            &err_type_name,
        ))
    }

    if let Some(stub_attrs) = &attrs.stub {
        output.extend(impl_rpc_stub(
            &attrs,
            &stub_attrs.clone().unwrap_or_default(),
            &input,
            &msg_type_name,
            &proto_type_name,
        ))
    }

    output.extend(def_request_type(
        &attrs,
        &input,
        &msg_type_name,
        &derive_schemars,
    ));

    if let Some(service) = &attrs.service {
        let service_attrs = service.clone().unwrap_or_default();
        if let Some(_js) = &service_attrs.javascript {
            output.extend(javascript_service(&service_attrs, &input));
        }
        if let Some(_py) = &service_attrs.python {
            output.extend(python_service(&service_attrs, &input));
        }
    }

    if let Some(stub) = &attrs.stub {
        let stub_attrs = stub.clone().unwrap_or_default();
        if let Some(_js) = &stub_attrs.javascript {
            output.extend(javascript_stub(&stub_attrs, &input));
        }
        if let Some(_py) = &stub_attrs.python {
            output.extend(python_stub(&stub_attrs, &input));
        }
    }

    // println!("Generated interface:");
    // let mut formatter = std::process::Command::new("rustfmt")
    //     .arg("--edition=2018")
    //     .stdin(std::process::Stdio::piped())
    //     .stdout(std::process::Stdio::inherit())
    //     .spawn()
    //     .unwrap();
    // write!(
    //     &mut formatter.stdin.take().unwrap() as &mut dyn std::io::Write,
    //     "{}",
    //     output
    // )
    // .unwrap();
    // formatter.wait().unwrap();

    output.into()
}
