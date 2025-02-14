/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

#[macro_use]
mod macros;

mod async_duplex;
mod async_simplex;
mod error;
mod from_serialize;
mod generic_value;
mod handler;
mod handshake;
mod msg_stream;
mod otel;
mod template;
mod tls;

pub use otel::TraceCtx;

pub use async_duplex::AsyncDuplex;
pub use async_simplex::{
    handle_async_broker_stream,
    AsyncBroker,
    AsyncBrokerBuilder,
    AsyncBrokerClient,
    AsyncBrokerHandlerBuilder,
    // async_tls_cbor_tcp_client, async_tls_cbor_tcp_server,
    // async_tls_cbor_unix_client, async_tls_cbor_unix_server,
    // async_tls_json_tcp_client, async_tls_json_tcp_server,
    // async_tls_json_unix_client, async_tls_json_unix_server,
    AsyncClient,
    AsyncClientConnection,
    AsyncProto,
    AsyncRequest,
    AsyncResponse,
    AsyncServer,
    AsyncServerConnection,
    BrokerHandler,
    BrokerNode,
    Connector,
    NodeMap,
};
pub use error::{Error, Result};
pub use from_serialize::FromSerialize;
pub use generic_value::GenericValue;
pub use handler::{
    RequestHandler, ServerRequestHandler, SessionHandler, SessionLess, Shutdown,
};
pub use handshake::{handshake_client, handshake_server};
pub use handshake::{HandshakeClient, HandshakeServer};
#[cfg(feature = "serde_cbor")]
pub use msg_stream::{CborReadStream, CborStream, CborWriteStream};
#[cfg(feature = "serde_json")]
pub use msg_stream::{JsonReadStream, JsonStream, JsonWriteStream};
pub use msg_stream::{MsgReadStream, MsgStream, MsgWriteStream, Protocol};
pub use rpc_derive::rpc;
pub use template::Template;
pub use tls::{tls_client_config, tls_server_config, TlsStreamExt};
#[cfg(feature = "tracing")]
pub use tracing;
