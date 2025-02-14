/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{future::Future, marker::PhantomData, sync::Arc};

use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsAcceptor;

#[cfg(feature = "serde_cbor")]
use crate::CborStream;
#[cfg(feature = "serde_json")]
use crate::JsonStream;
use crate::TraceCtx;

use super::Split;

pub trait StreamWrapper<S>: Send + Sync + 'static {
    type Wrapped: Send + Sync + Unpin + 'static;
    fn wrap(&self, stream: S) -> impl Future<Output = Self::Wrapped> + Send;
}

pub struct PlainStreamWrapper;

impl<S> StreamWrapper<S> for PlainStreamWrapper
where
    S: Send + Sync + Unpin + 'static,
{
    type Wrapped = S;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        stream
    }
}

#[cfg(feature = "serde_json")]
pub struct JsonStreamWrapper<T, R, W> {
    inner: T,
    _marker: PhantomData<(R, W)>,
}

#[cfg(feature = "serde_json")]
impl<T, R, W, F> JsonStreamWrapper<T, R, W> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "serde_json")]
impl<S, T, R, W> StreamWrapper<S> for JsonStreamWrapper<T, R, W>
where
    S: Send + 'static,
    T: StreamWrapper<S> + Unpin,
    T::Wrapped: Split + 'static,
    R: DeserializeOwned + Send + Sync + Unpin + 'static,
    W: Serialize + Send + Sync + Unpin + 'static,
    // F: for<'a> Fn(&'a R) -> Option<TraceCtx> + Send + Sync + Unpin + 'static,
{
    type Wrapped = JsonStream<T::Wrapped, R, W>;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        JsonStream::new(self.inner.wrap(stream).await)
    }
}

#[cfg(feature = "serde_cbor")]
pub struct CborStreamWrapper<T, R, W> {
    inner: T,
    _marker: PhantomData<(R, W)>,
}

#[cfg(feature = "serde_cbor")]
impl<T, R, W> CborStreamWrapper<T, R, W> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "serde_cbor")]
impl<S, T, R, W> StreamWrapper<S> for CborStreamWrapper<T, R, W>
where
    S: Send + 'static,
    T: StreamWrapper<S>,
    T::Wrapped: AsyncRead + AsyncWrite + Send + 'static,
    R: DeserializeOwned + Send + Sync + Unpin + 'static,
    W: Serialize + Send + Sync + Unpin + 'static,
{
    type Wrapped = CborStream<T::Wrapped, R, W>;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        CborStream::new(self.inner.wrap(stream).await)
    }
}

#[cfg(feature = "bincode")]
pub struct BincodeStreamWrapper<T, R, W, F> {
    inner: T,
    trace_ctx: F,
    _marker: PhantomData<(R, W)>,
}

#[cfg(feature = "bincode")]
impl<T, R, W, F> BincodeStreamWrapper<T, R, W, F> {
    pub fn new(inner: T, trace_ctx: F) -> Self {
        Self {
            inner,
            trace_ctx,
            _marker: PhantomData,
        }
    }
}

#[cfg(feature = "bincode")]
impl<S, T, R, W, F> StreamWrapper<S> for BincodeStreamWrapper<T, R, W, F>
where
    S: Send + 'static,
    T: StreamWrapper<S>,
    T::Wrapped: AsyncRead + AsyncWrite + Send + 'static,
    R: DeserializeOwned + Send + Sync + Unpin + 'static,
    W: Serialize + Send + Sync + Unpin + 'static,
    F: for<'a> Fn(&'a R) -> Option<TraceCtx>
        + Send
        + Sync
        + Unpin
        + Clone
        + 'static,
{
    type Wrapped = BincodeStream<T::Wrapped, R, W, F>;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        BincodeStream::new(
            self.inner.wrap(stream).await,
            self.trace_ctx.clone(),
        )
    }
}

impl<S> StreamWrapper<S> for TlsAcceptor
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Wrapped = tokio_rustls::server::TlsStream<S>;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        self.accept(stream)
            .await
            .expect("incoming connection failed")
    }
}

pub struct TlsConnector {
    connector: tokio_rustls::TlsConnector,
    server_name: rustls::pki_types::ServerName<'static>,
}

impl TlsConnector {
    pub fn new(
        connector: tokio_rustls::TlsConnector,
        server_name: rustls::pki_types::ServerName<'static>,
    ) -> Self {
        Self {
            connector,
            server_name,
        }
    }
}

impl<S> StreamWrapper<S> for TlsConnector
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Wrapped = tokio_rustls::client::TlsStream<S>;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        self.connector
            .connect(self.server_name.clone(), stream)
            .await
            .expect("connection failed")
    }
}

impl<S, T> StreamWrapper<S> for Arc<T>
where
    S: Send + 'static,
    T: StreamWrapper<S>,
{
    type Wrapped = T::Wrapped;
    async fn wrap(&self, stream: S) -> Self::Wrapped {
        T::wrap(self, stream).await
    }
}
