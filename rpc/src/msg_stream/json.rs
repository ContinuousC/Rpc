/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{
    Error, MsgReadStream, MsgStream, MsgWriteStream, Result, TlsStreamExt,
    TraceCtx,
};

use super::Split;

pub struct JsonStream<S, R, W, F> {
    stream: S,
    data: Vec<u8>,
    seek_pos: usize,
    ctx: F,
    _msg: PhantomData<(R, W)>,
}

pub struct JsonReadStream<S, T, F> {
    stream: S,
    data: Vec<u8>,
    seek_pos: usize,
    ctx: F,
    _msg: PhantomData<T>,
}

pub struct JsonWriteStream<S, T> {
    stream: S,
    _msg: PhantomData<T>,
}

impl<S, R, W, F> JsonStream<S, R, W, F>
where
    S: Split + Send + Sync + Unpin + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
    W: Serialize + Send + Sync + 'static,
    F: for<'a> Fn(&'a R) -> Option<TraceCtx> + Send + Sync + 'static,
{
    pub fn new(stream: S, ctx: F) -> Self {
        Self {
            stream,
            data: Vec::new(),
            seek_pos: 0,
            ctx,
            _msg: PhantomData,
        }
    }
}

impl<S, R, W, F> MsgStream<R, W> for JsonStream<S, R, W, F>
where
    S: Split + Send + Sync + Unpin + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
    W: Serialize + Send + Sync + 'static,
    F: for<'a> Fn(&'a R) -> Option<TraceCtx> + Send + Sync + 'static,
{
    type Stream = S;
    type ReadStream = JsonReadStream<S::ReadStream, R, F>;
    type WriteStream = JsonWriteStream<S::WriteStream, W>;

    fn split(self) -> (Self::ReadStream, Self::WriteStream) {
        let (read, write) = self.stream.split();
        (
            JsonReadStream {
                stream: read,
                data: self.data,
                seek_pos: self.seek_pos,
                ctx: self.ctx,
                _msg: PhantomData,
            },
            JsonWriteStream {
                stream: write,
                _msg: PhantomData,
            },
        )
    }

    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self {
        let stream = S::unsplit(read.stream, write.stream);
        Self {
            stream,
            data: read.data,
            seek_pos: read.seek_pos,
            ctx: read.ctx,
            _msg: PhantomData,
        }
    }
}

impl<S, R, W, F> MsgReadStream<R> for JsonStream<S, R, W, F>
where
    S: AsyncRead + Send + Sync + Unpin + 'static,
    R: DeserializeOwned + Send + Sync + 'static,
    W: Send + Sync + 'static,
    F: for<'a> Fn(&'a R) -> Option<TraceCtx> + Send + Sync + 'static,
{
    async fn recv(&mut self) -> Result<Option<R>> {
        loop {
            if let Some(i) = self
                .data
                .iter()
                .skip(self.seek_pos)
                .position(|c| *c == b'\n')
            {
                let i = i + self.seek_pos;
                let msg = serde_json::from_slice(&self.data[..i]);
                self.seek_pos = 0;
                let _ = self.data.drain(..=i);
                return Ok(Some(msg?));
            }
            self.seek_pos = self.data.len();
            if self.stream.read_buf(&mut self.data).await? == 0 {
                return match self.data.is_empty() {
                    true => Ok(None),
                    false => Err(Error::UnexpectedEof),
                };
            }
        }
    }

    fn with_context_extractor<G>(self, ctx: G) -> impl MsgReadStream<R>
    where
        G: for<'a> Fn(&'a R) -> Option<TraceCtx> + Send + Sync + 'static,
    {
        JsonStream::<S, R, W, G> {
            stream: self.stream,
            data: self.data,
            seek_pos: self.seek_pos,
            ctx,
            _msg: PhantomData,
        }
    }
}

impl<S, T, F> MsgReadStream<T> for JsonReadStream<S, T, F>
where
    S: AsyncRead + Send + Sync + Unpin + 'static,
    T: DeserializeOwned + Send + Sync + 'static,
    F: for<'a> Fn(&'a T) -> Option<TraceCtx> + Send + Sync + 'static,
{
    async fn recv(&mut self) -> Result<Option<T>> {
        loop {
            if let Some(i) = self
                .data
                .iter()
                .skip(self.seek_pos)
                .position(|c| *c == b'\n')
            {
                let i = i + self.seek_pos;
                let msg = serde_json::from_slice(&self.data[..i]);
                self.seek_pos = 0;
                let _ = self.data.drain(..=i);
                return Ok(Some(msg?));
            }
            self.seek_pos = self.data.len();
            log::trace!("reading from JsonReadStream");
            let try_len = self.stream.read_buf(&mut self.data).await;
            log::trace!("read result: {:?}", try_len);
            if try_len? == 0 {
                return match self.data.is_empty() {
                    true => Ok(None),
                    false => Err(Error::UnexpectedEof),
                };
            }
        }
    }

    fn with_context_extractor<G>(self, ctx: G) -> impl MsgReadStream<T>
    where
        G: for<'a> Fn(&'a T) -> Option<TraceCtx> + Send + Sync + 'static,
    {
        JsonReadStream::<S, T, G> {
            stream: self.stream,
            data: self.data,
            seek_pos: self.seek_pos,
            ctx,
            _msg: PhantomData,
        }
    }
}

impl<S, R, W, F> MsgWriteStream<W> for JsonStream<S, R, W, F>
where
    S: AsyncWrite + Send + Sync + Unpin + 'static,
    W: Serialize + Send + Sync + 'static,
    R: Send + Sync + 'static,
    F: for<'a> Fn(&'a R) -> Option<TraceCtx> + Send + Sync + 'static,
{
    async fn send(&mut self, msg: W) -> Result<()> {
        let mut buf = serde_json::to_vec(&msg)?;
        buf.push(b'\n');
        self.stream.write_all(&buf).await?;
        self.stream.flush().await?;
        Ok(())
    }
    async fn shutdown(&mut self) -> Result<()> {
        Ok(self.stream.shutdown().await?)
    }
}

impl<S, T> MsgWriteStream<T> for JsonWriteStream<S, T>
where
    S: AsyncWrite + Send + Sync + Unpin + 'static,
    T: Serialize + Send + Sync + 'static,
{
    async fn send(&mut self, msg: T) -> Result<()> {
        let mut buf = serde_json::to_vec(&msg)?;
        buf.push(b'\n');
        self.stream.write_all(&buf).await?;
        self.stream.flush().await?;
        Ok(())
    }
    async fn shutdown(&mut self) -> Result<()> {
        Ok(self.stream.shutdown().await?)
    }
}

impl<S, R, W, F> TlsStreamExt for JsonStream<S, R, W, F>
where
    S: TlsStreamExt,
{
    fn peer_certificate(&self) -> Option<&rustls::pki_types::CertificateDer> {
        self.stream.peer_certificate()
    }
}
