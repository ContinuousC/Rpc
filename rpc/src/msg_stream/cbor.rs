/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{convert::TryInto, marker::PhantomData};

use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{
    split, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf,
    WriteHalf,
};

use super::varint::{varint_len, VarInt};
use crate::{
    Error, MsgReadStream, MsgStream, MsgWriteStream, Result, TlsStreamExt,
};

pub struct CborStream<S, R, W> {
    stream: S,
    data: Vec<u8>,
    varint: bool,
    _msg: PhantomData<(R, W)>,
}

pub struct CborReadStream<S, T> {
    stream: ReadHalf<S>,
    data: Vec<u8>,
    varint: bool,
    _msg: PhantomData<T>,
}

pub struct CborWriteStream<S, T> {
    stream: WriteHalf<S>,
    varint: bool,
    _msg: PhantomData<T>,
}

impl<S, R, W> CborStream<S, R, W>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
    W: Serialize + Send + Sync + 'static,
{
    pub fn new(stream: S) -> Self {
        Self::new_varint(stream, true)
    }

    /// Constructor allowing to select non-varint length encoding for
    /// backward compatibility.
    pub fn new_varint(stream: S, varint: bool) -> Self {
        Self {
            stream,
            data: Vec::with_capacity(1024),
            varint,
            _msg: PhantomData,
        }
    }

    pub fn change_proto<U, V>(self) -> CborStream<S, U, V>
    where
        U: DeserializeOwned + Send + Sync + 'static,
        V: Serialize + Send + Sync + 'static,
    {
        CborStream {
            stream: self.stream,
            data: self.data,
            varint: self.varint,
            _msg: PhantomData,
        }
    }
}

impl<S, R, W> MsgStream<R, W> for CborStream<S, R, W>
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    W: Serialize + Send + Sync + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
{
    type Stream = S;
    type ReadStream = CborReadStream<S, R>;
    type WriteStream = CborWriteStream<S, W>;

    fn split(self) -> (CborReadStream<S, R>, CborWriteStream<S, W>) {
        let (read_half, write_half) = split(self.stream);
        (
            CborReadStream {
                stream: read_half,
                data: self.data,
                varint: self.varint,
                _msg: PhantomData,
            },
            CborWriteStream {
                stream: write_half,
                varint: self.varint,
                _msg: PhantomData,
            },
        )
    }

    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self {
        Self {
            stream: read.stream.unsplit(write.stream),
            data: read.data,
            varint: read.varint,
            _msg: PhantomData,
        }
    }
}

impl<S, R, W> MsgReadStream<R> for CborStream<S, R, W>
where
    S: AsyncRead + Send + Sync + Unpin + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
    W: Send + Sync + 'static,
{
    async fn recv(&mut self) -> Result<Option<R>> {
        cbor_stream_recv(self.varint, &mut self.data, &mut self.stream).await
    }
}

impl<S, T> MsgReadStream<T> for CborReadStream<S, T>
where
    S: AsyncRead + Send + Sync + Unpin + 'static,
    T: DeserializeOwned + Send + Sync + 'static,
{
    async fn recv(&mut self) -> Result<Option<T>> {
        cbor_stream_recv(self.varint, &mut self.data, &mut self.stream).await
    }
}

impl<S, R, W> MsgWriteStream<W> for CborStream<S, R, W>
where
    S: AsyncWrite + Send + Sync + Unpin + 'static,
    W: Serialize + Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    async fn send(&mut self, msg: W) -> Result<()> {
        cbor_stream_send(self.varint, &mut self.stream, msg).await
    }
    async fn shutdown(&mut self) -> Result<()> {
        Ok(self.stream.shutdown().await?)
    }
}

impl<S, T> MsgWriteStream<T> for CborWriteStream<S, T>
where
    S: AsyncWrite + Send + Sync + Unpin + 'static,
    T: Serialize + Send + Sync + 'static,
{
    async fn send(&mut self, msg: T) -> Result<()> {
        cbor_stream_send(self.varint, &mut self.stream, msg).await
    }
    async fn shutdown(&mut self) -> Result<()> {
        Ok(self.stream.shutdown().await?)
    }
}

async fn cbor_stream_recv<S, T>(
    varint: bool,
    data: &mut Vec<u8>,
    stream: &mut S,
) -> Result<Option<T>>
where
    S: AsyncRead + Send + Sync + Unpin + 'static,
    T: DeserializeOwned + Send + Sync + 'static,
{
    loop {
        if !data.is_empty() {
            let llen = match varint {
                true => varint_len(data[0]),
                false => 4,
            };
            if data.len() >= llen {
                let len = match varint {
                    true => u64::from_varint(data) as usize,
                    false => u32::from_be_bytes(data[0..4].try_into().unwrap())
                        as usize,
                };
                if data.len() >= llen + len {
                    let buf = &data[llen..llen + len];
                    let res = serde_cbor::from_slice(buf).map_err(|_| {
                        match serde_cbor::from_slice::<serde_cbor::Value>(buf) {
                            Ok(v) => Error::InvalidCborMessage(v),
                            Err(e) => Error::Cbor(e),
                        }
                    });
                    data.drain(0..llen + len);
                    //log::debug!("Received message {}", serde_json::to_string(&msg).unwrap());
                    return res.map(Some);
                }
            }
        }

        if stream.read_buf(data).await? == 0 {
            return match data.is_empty() {
                true => Ok(None),
                false => Err(Error::UnexpectedEof),
            };
        }
    }
}

async fn cbor_stream_send<S, T>(
    varint: bool,
    stream: &mut S,
    msg: T,
) -> Result<()>
where
    S: AsyncWrite + Send + Sync + Unpin + 'static,
    T: Serialize + Send + Sync + 'static,
{
    //log::debug!("Sending message {}", serde_json::to_string(&msg).unwrap());
    // TODO: make cancellation-safe?
    let buf = serde_cbor::to_vec(&msg)?;
    match varint {
        true => stream.write_all(&(buf.len() as u64).to_varint()).await?,
        false => {
            stream
                .write_all((buf.len() as u32).to_be_bytes().as_slice())
                .await?
        }
    };
    //stream.flush().await?;
    stream.write_all(&buf).await?;
    stream.flush().await?;
    Ok(())
}

impl<S, R, W> TlsStreamExt for CborStream<S, R, W>
where
    S: TlsStreamExt,
{
    fn peer_certificate(&self) -> Option<&rustls::pki_types::CertificateDer> {
        self.stream.peer_certificate()
    }
}
