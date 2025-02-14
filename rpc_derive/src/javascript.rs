/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::fmt::Write;

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::attributes::ServiceAttrs;

use super::case_conversion::snake_case;
use super::input::{Input, Method};

const RESERVED: &[&str] = &[
    "abstract",
    "arguments",
    "await",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "function",
    "goto",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "int",
    "interface",
    "let",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "volatile",
    "while",
    "with",
    "yield",
];

fn unreserved(s: &str) -> String {
    match RESERVED.contains(&s) {
        true => format!("{}_", s),
        false => s.to_string(),
    }
}

pub fn javascript_service(
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
    let mut js_def = String::new();

    let js_extra_args = extra_args.as_ref().map_or_else(Vec::new, |s| {
        s.names.iter().map(|a| unreserved(&a.to_string())).collect()
    });
    let js_service_name = Ident::new(
        &format!("js_{}", snake_case(service_name.to_string().as_str())),
        proc_macro2::Span::call_site(),
    );

    for Method {
        method, arg_list, ..
    } in methods
    {
        write!(
            js_def,
            "if ('{}' {} req) {{\n    \
         if (!('{}' in this.service))\n        \
         throw 'Request not implemented: {}';\n    \
         return this.service.{}({});\n}}\n\n",
            method.sig.ident,
            if arg_list.is_empty() { "==" } else { "in" },
            method.sig.ident,
            method.sig.ident,
            unreserved(&method.sig.ident.to_string()),
            session
                .then(|| "session".to_string())
                .into_iter()
                .chain(js_extra_args.iter().map(|s| unreserved(s)))
                .chain(arg_list.iter().map(|arg| format!(
                    "req['{}']['{}']",
                    method.sig.ident, arg
                )))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }

    writeln!(js_def, "throw \"Unsupported request: \" + req;").unwrap();

    quote! {
        pub fn #js_service_name() -> &'static str {
            #js_def
        }
    }
}

pub fn javascript_stub(
    ServiceAttrs {
        extra_args,
        javascript,
        ..
    }: &ServiceAttrs,
    Input {
        service_name,
        methods,
        ..
    }: &Input,
) -> TokenStream {
    let mut js_def = String::new();

    let js_extra_args = extra_args.as_ref().map_or_else(Vec::new, |s| {
        s.names.iter().map(|a| a.to_string()).collect()
    });
    let js_req_method = javascript
        .as_ref()
        .and_then(|s| s.as_ref().explicit())
        .and_then(|s| s.req_method.as_deref())
        .unwrap_or("request");

    let js_stub_name = Ident::new(
        &format!("js_{}_stub", snake_case(service_name.to_string().as_str())),
        proc_macro2::Span::call_site(),
    );

    for Method {
        method, arg_list, ..
    } in methods
    {
        write!(
            js_def,
            "async {}({}) {{\n    \
	     return await this.{}({}\"{}\", {}, ctx);\n\
	     }}\n\n",
            unreserved(&method.sig.ident.to_string()),
            js_extra_args
                .iter()
                .chain(arg_list)
                .map(|s| unreserved(s))
                .chain(std::iter::once("ctx = {}".to_string()))
                .collect::<Vec<_>>()
                .join(", "),
            js_req_method,
            js_extra_args
                .iter()
                .map(|a| format!("{}, ", unreserved(a)))
                .collect::<Vec<_>>()
                .concat(),
            method.sig.ident,
            match arg_list.len() {
                0 => "null".to_string(),
                1 => format!(
                    "{{{}}}",
                    arg_list
                        .iter()
                        .map(|a| match RESERVED.contains(&a.as_str()) {
                            true => format!("'{}': {}", a, unreserved(a)),
                            false => a.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                _ => format!("{{\n        {}}}", arg_list.join(", ")),
            }
        )
        .unwrap();
    }

    quote! {
        pub fn #js_stub_name() -> &'static str {
            #js_def
        }
    }
}
