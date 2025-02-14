/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use serde::{Deserialize, Serialize};

use super::MsgStream;
use crate::error::{Error, Result};

/// Handshake messages sent by the server.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandshakeServer {
    Accepted,
    Unsupported { error: String },
}

/// Handshake messages sent by the client.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandshakeClient {
    Handshake {
        version: u64,
        encoding: Encoding,
        compression: Option<Compression>,
    },
}

/// Available message encodings.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Cbor,
    /// Messages are serialized as CBOR and prefixed with a two-byte length
    #[serde(other)]
    Unsupported,
}

/// Available compression algorithms.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Compression {
    #[serde(other)]
    Unsupported,
}

pub async fn handshake_server<S>(
    stream: &mut S,
    min: u64,
    max: u64,
) -> Result<u64>
where
    S: MsgStream<HandshakeClient, HandshakeServer>,
{
    let HandshakeClient::Handshake {
        version,
        encoding,
        compression,
    } = stream.recv().await?.ok_or(Error::UnexpectedEof)?;

    if version < min || version > max {
        stream
            .send(HandshakeServer::Unsupported {
                error: "version not supported".to_string(),
            })
            .await?;
        Err(Error::VersionMismatch)?;
    }

    if encoding != Encoding::Cbor {
        stream
            .send(HandshakeServer::Unsupported {
                error: "encoding not supported".to_string(),
            })
            .await?;
        Err(Error::VersionMismatch)?;
    }

    if compression.is_some() {
        stream
            .send(HandshakeServer::Unsupported {
                error: "compression not supported".to_string(),
            })
            .await?;
        Err(Error::VersionMismatch)?;
    }

    stream.send(HandshakeServer::Accepted).await?;
    Ok(version)
}

pub async fn handshake_client<S>(stream: &mut S, version: u64) -> Result<()>
where
    S: MsgStream<HandshakeServer, HandshakeClient>,
{
    stream
        .send(HandshakeClient::Handshake {
            version,
            encoding: Encoding::Cbor,
            compression: None,
        })
        .await?;

    match stream.recv().await?.ok_or(Error::UnexpectedEof)? {
        HandshakeServer::Accepted => Ok(()),
        HandshakeServer::Unsupported { error: _ } => {
            Err(Error::VersionMismatch)?
        }
    }
}
