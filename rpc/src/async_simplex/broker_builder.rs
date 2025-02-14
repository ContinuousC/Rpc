/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{future::Future, marker::PhantomData, path::Path, sync::Arc};

use futures::{future::ready, Stream, StreamExt, TryFutureExt, TryStreamExt};
use rustls::ServerConfig;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, ToSocketAddrs, UnixListener, UnixStream},
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};
use tokio_stream::wrappers::{TcpListenerStream, UnixListenerStream};

use crate::{Error, Result};

use super::broker::{AsyncBrokerHandler, BrokerHandler, BrokerNode};

pub struct AsyncBrokerHandlerBuilder<N, S>(PhantomData<N>, S);

pub struct Init(());

pub struct WithListener<L> {
    listener: L,
}

pub struct WithTls<L> {
    listener: L,
}

impl<N> AsyncBrokerHandlerBuilder<N, Init> {
    pub fn new() -> Self {
        Self(PhantomData, Init(()))
    }

    pub fn unix<A>(
        self,
        addr: A,
    ) -> Result<
        AsyncBrokerHandlerBuilder<
            N,
            WithListener<impl Stream<Item = Result<UnixStream>>>,
        >,
    >
    where
        A: AsRef<Path> + 'static,
    {
        Ok(AsyncBrokerHandlerBuilder(
            PhantomData,
            WithListener {
                listener: UnixListenerStream::new(
                    UnixListener::bind(addr).map_err(Error::Bind)?,
                )
                .map_err(Error::Accept),
            },
        ))
    }

    pub async fn tcp<A>(
        self,
        addr: A,
    ) -> Result<
        AsyncBrokerHandlerBuilder<
            N,
            WithListener<impl Stream<Item = Result<TcpStream>>>,
        >,
    >
    where
        A: ToSocketAddrs,
    {
        Ok(AsyncBrokerHandlerBuilder(
            PhantomData,
            WithListener {
                listener: TcpListenerStream::new(
                    TcpListener::bind(addr).await.map_err(Error::Bind)?,
                )
                .map_err(Error::Accept),
            },
        ))
    }
}

impl<N> Default for AsyncBrokerHandlerBuilder<N, Init> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N, L, S> AsyncBrokerHandlerBuilder<N, WithListener<L>>
where
    L: Stream<Item = Result<S>>,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn notls(
        self,
    ) -> AsyncBrokerHandlerBuilder<
        N,
        WithTls<impl Stream<Item = impl Future<Output = Result<S>>>>,
    > {
        AsyncBrokerHandlerBuilder(
            PhantomData,
            WithTls {
                listener: self.1.listener.map(ready),
            },
        )
    }

    pub fn tls(
        self,
        tls_config: Arc<ServerConfig>,
    ) -> AsyncBrokerHandlerBuilder<
        N,
        WithTls<impl Stream<Item = impl Future<Output = Result<TlsStream<S>>>>>,
    > {
        let acceptor = TlsAcceptor::from(tls_config);
        AsyncBrokerHandlerBuilder(
            PhantomData,
            WithTls {
                listener: self.1.listener.map(move |r| {
                    let acceptor = acceptor.clone();
                    Box::pin(ready(r).and_then(move |s| {
                        let accept = acceptor.accept(s);
                        async move { accept.await.map_err(Error::Accept) }
                    }))
                }),
            },
        )
    }
}

impl<N, L, Fut, S> AsyncBrokerHandlerBuilder<N, WithTls<L>>
where
    L: Stream<Item = Fut> + Send + Unpin + 'static,
    Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn handler<H>(self, handler: H) -> AsyncBrokerHandler<L, H>
    where
        H: BrokerHandler<S> + Send + Sync + 'static,
        H::ReadMsg: Send,
        H::WriteMsg: Send,
        H::Key: Clone + Send + Sync,
        H::Node: Send + Sync,
        <H::Node as BrokerNode>::Key: Send + Sync,
    {
        AsyncBrokerHandler::new(self.1.listener, handler)
    }
}
