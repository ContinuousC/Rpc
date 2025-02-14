/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

mod config;
#[cfg(test)]
mod test;
mod tls_stream_ext;

pub use config::{tls_client_config, tls_server_config};
pub use tls_stream_ext::TlsStreamExt;
