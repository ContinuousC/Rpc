/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::fmt::Write;
use std::iter::once;

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::attributes::ServiceAttrs;

use super::case_conversion::snake_case;
use super::input::{Input, Method};

const RESERVED: &[&str] = &[
    "as", "except", "with", "assert", "finally", "yield", "break", "for",
    "class", "from",
];

fn unreserved(s: &str) -> String {
    match RESERVED.contains(&s) {
        true => format!("{}_", s),
        false => s.to_string(),
    }
}

pub fn python_service(
    ServiceAttrs {
        session,
        extra_args,
        ..
    }: &ServiceAttrs,
    Input {
        service_name,
        methods,
        ..
    }: &Input,
) -> TokenStream {
    let mut py_def = String::new();

    let py_extra_args = extra_args.as_ref().map_or_else(Vec::new, |s| {
        s.names.iter().map(|a| a.to_string()).collect()
    });
    let py_service_name = Ident::new(
        &format!("py_{}", snake_case(service_name.to_string().as_str())),
        proc_macro2::Span::call_site(),
    );

    for Method {
        method, arg_list, ..
    } in methods
    {
        write!(
            py_def,
            "if '{}' {} req:\n    \
                if not callable(getattr(self, '{}', None)):\n        \
                    raise Exception('Request not implemented: {}')\n    \
                return self.{}({})\n\n",
            method.sig.ident,
            if arg_list.is_empty() { "==" } else { "in" },
            method.sig.ident,
            method.sig.ident,
            unreserved(&method.sig.ident.to_string()),
            session
                .then(|| "session".to_string())
                .into_iter()
                .chain(py_extra_args.iter().map(|s| unreserved(s)))
                .chain(arg_list.iter().map(|arg| format!(
                    "req['{}']['{}']",
                    method.sig.ident, arg
                )))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }

    writeln!(py_def, "raise Exception('Request not implemented: ' + req)")
        .unwrap();

    quote! {
        pub fn #py_service_name() -> &'static str {
            #py_def
        }
    }
}

pub fn python_stub(
    ServiceAttrs { extra_args, .. }: &ServiceAttrs,
    Input {
        service_name,
        methods,
        ..
    }: &Input,
) -> TokenStream {
    let mut py_def = String::new();

    let py_extra_args = extra_args.as_ref().map_or_else(Vec::new, |s| {
        s.names.iter().map(|a| a.to_string()).collect()
    });
    let py_stub_name = Ident::new(
        &format!("py_{}_stub", snake_case(service_name.to_string().as_str())),
        proc_macro2::Span::call_site(),
    );

    for Method {
        method, arg_list, ..
    } in methods
    {
        write!(
            py_def,
            "def {}({}):\n    \
			 return self.__request({}\"{}\", {}, ctx)\n\n",
            unreserved(&method.sig.ident.to_string()),
            once("self".to_string())
                .chain(py_extra_args.iter().map(|s| unreserved(s)))
                .chain(arg_list.iter().map(|s| unreserved(s)))
                .chain(std::iter::once("ctx = {}".to_string()))
                .collect::<Vec<_>>()
                .join(", "),
            py_extra_args
                .iter()
                .map(|a| format!("{}, ", unreserved(a)))
                .collect::<Vec<_>>()
                .concat(),
            unreserved(&method.sig.ident.to_string()),
            match arg_list.len() {
                0 => "None".to_string(),
                _ => format!(
                    "{{{}}}",
                    arg_list
                        .iter()
                        .map(|a| format!("'{}': {}", a, unreserved(a)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        )
        .unwrap();
    }

    quote! {
        pub fn #py_stub_name() -> &'static str {
            #py_def
        }
    }
}
