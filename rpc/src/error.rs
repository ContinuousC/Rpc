/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Peer requested unsupported protocol version")]
    VersionMismatch,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Received error from peer: {0}")]
    Peer(String),
    #[error("Unsupported API function")]
    Unsupported,
    #[error("The message is too long")]
    MessageTooLong,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "serde_json")]
    #[error("JSON en-/decoding error: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "serde_cbor")]
    #[error("CBOR en-/decoding error: {0}")]
    Cbor(#[from] serde_cbor::Error),
    #[cfg(feature = "serde_cbor")]
    #[error("Invalid message in CBOR stream: {0:?}")]
    InvalidCborMessage(serde_cbor::Value),
    #[error("Unexpected EOF")]
    UnexpectedEof,
    #[error("Failed to send on MPSC channel")]
    SendMpsc,
    #[error("Failed to receive from MPSC channel")]
    RecvMpsc,

    /* TLS errors. */
    #[error("Failed to read certificate '{}': {1}", .0.display())]
    ReadCert(PathBuf, tokio::io::Error),
    #[error("Failed to read key '{}': {1}", .0.display())]
    ReadKey(PathBuf, tokio::io::Error),
    #[error("Failed to decode certificate '{}': {1}", .0.display())]
    DecodeCert(PathBuf, std::io::Error),
    #[error("Failed to decode key '{}': {1}", .0.display())]
    DecodeKey(PathBuf, std::io::Error),
    #[error("Failed to add certificate to root certificate store: {0}")]
    RootCertStore(rustls::Error),
    #[error("No certificate found in '{}'", .0.display())]
    NoCert(PathBuf),
    #[error("No key found in '{}'", .0.display())]
    NoKey(PathBuf),
    #[error("Failed to initialize server TLS config: {0}")]
    ServerConfig(rustls::Error),
    #[error("Failed to initialize server TLS config: {0}")]
    VerifierBuild(rustls::server::VerifierBuilderError),
    #[error("Failed to initialize client TLS config: {0}")]
    ClientConfig(rustls::Error),

    #[error("server reader error: {0}")]
    ServerReader(tokio::task::JoinError),
    #[error("server writer error: {0}")]
    ServerWriter(tokio::task::JoinError),
    #[error("server listener error: {0}")]
    ServerListener(tokio::task::JoinError),
    #[error("client reader error: {0}")]
    ClientReader(tokio::task::JoinError),
    #[error("client writer error: {0}")]
    ClientWriter(tokio::task::JoinError),
    #[error("failed to send request")]
    SendRequest,
    #[error("failed to retrieve result")]
    ReceiveResult,
    #[error("failed to send termination signal")]
    SendTerm,

    #[error("failed to bind: {0}")]
    Bind(std::io::Error),
    #[error("failed to accept: {0}")]
    Accept(std::io::Error),
    #[error("failed to connect: {0}")]
    Connect(std::io::Error),
    #[error("authentication failed")]
    Authentication,
    #[error("broker handler failed")]
    BrokerHandler,
    #[error("got message for missing node!")]
    MissingNode,
    #[error("failed to build tls server name")]
    ServerName(String),

    #[error("timeout")]
    Timeout,
}
