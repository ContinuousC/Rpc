/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use darling::{util::Override, FromMeta};
use syn::{
    parse::Parser, punctuated::Punctuated, token::Comma, FnArg, Ident, Pat,
    PatType, Type,
};

#[derive(FromMeta, Clone, Default)]
#[darling(default)]
pub struct Attributes {
    pub msg_type: Option<String>,
    pub err_type: Option<String>,
    pub proto_type: Option<String>,
    pub service: Option<Override<ServiceAttrs>>,
    pub stub: Option<Override<ServiceAttrs>>,
    pub log_errors: bool,
}

#[derive(FromMeta, Clone, Default)]
#[darling(default)]
pub struct ServiceAttrs {
    pub session: bool,
    pub shutdown: bool,
    pub extra_args: Option<ExtraArgs>,
    pub javascript: Option<Override<JsAttrs>>,
    pub python: Option<Override<PyAttrs>>,
    pub lock_debug_task_names: bool,
}

#[derive(Clone)]
pub struct ExtraArgs {
    pub names: Vec<Ident>,
    pub types: Vec<Type>,
    pub signature: Vec<FnArg>,
}

impl FromMeta for ExtraArgs {
    fn from_string(input: &str) -> Result<Self, darling::Error> {
        let parser = <Punctuated<FnArg, Comma>>::parse_separated_nonempty;
        let signature: Vec<FnArg> =
            parser.parse_str(input)?.into_iter().collect();
        let arguments: Vec<PatType> = signature
            .iter()
            .map(|arg| match arg {
                FnArg::Receiver(_) => panic!("extra_args cannot contain self"),
                FnArg::Typed(arg) => arg.clone(),
            })
            .collect();
        let names = arguments
            .iter()
            .map(|arg| match arg.pat.as_ref() {
                Pat::Ident(id) => id.ident.clone(),
                _ => panic!(
                    "extra_args arguments must have identifiers, \
					 not patterns"
                ),
            })
            .collect();
        let types = arguments
            .iter()
            .map(|arg| arg.ty.as_ref().clone())
            .collect();
        Ok(Self {
            names,
            types,
            signature,
        })
    }
}

#[derive(FromMeta, Clone, Default, Debug)]
#[darling(default)]
pub struct JsAttrs {
    pub req_method: Option<String>,
}

#[derive(FromMeta, Clone, Default, Debug)]
#[darling(default)]
pub struct PyAttrs {
    // ...
}
