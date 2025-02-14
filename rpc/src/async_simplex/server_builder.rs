/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{future::Future, marker::PhantomData, path::Path, sync::Arc};

use futures::{future::ready, Stream, StreamExt, TryFutureExt, TryStreamExt};
use rustls::{pki_types::ServerName, ClientConfig, ServerConfig};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, ToSocketAddrs, UnixListener, UnixStream},
};
use tokio_rustls::{client, server::TlsStream, TlsAcceptor, TlsConnector};
use tokio_stream::wrappers::{TcpListenerStream, UnixListenerStream};

use crate::{
    msg_stream::Split, AsyncRequest, AsyncResponse, AsyncServer, CborStream,
    Error, GenericValue, JsonStream, MsgReadStream, MsgStream, MsgWriteStream,
    Protocol, RequestHandler, Result, ServerRequestHandler, SessionHandler,
    SessionLess, Shutdown, TraceCtx,
};

pub struct AsyncServerBuilder<P: Protocol, S>(PhantomData<P>, S);

pub struct Init(());
pub struct WithListener<L> {
    listener: L,
}

pub struct WithTls<L> {
    listener: L,
}

pub struct WithEncoding<L, V> {
    listener: L,
    value: PhantomData<V>,
}

pub struct WithBrokerConnector<C> {
    connector: C,
}

pub struct WithBrokerTls<C> {
    connector: C,
}

pub struct WithBrokerEncoding<C, V> {
    connector: C,
    value: PhantomData<V>,
}

impl<P> AsyncServerBuilder<P, Init>
where
    P: Protocol,
{
    pub fn new() -> Self {
        Self(PhantomData, Init(()))
    }

    pub fn unix<A>(
        self,
        addr: A,
    ) -> Result<
        AsyncServerBuilder<
            P,
            WithListener<impl Stream<Item = Result<UnixStream>>>,
        >,
    >
    where
        A: AsRef<Path> + 'static,
    {
        Ok(AsyncServerBuilder(
            PhantomData,
            WithListener {
                listener: UnixListenerStream::new(
                    UnixListener::bind(addr).map_err(Error::Bind)?,
                )
                .map_err(Error::Accept),
            },
        ))
    }

    pub fn unix_broker<A>(
        self,
        addr: A,
    ) -> AsyncServerBuilder<
        P,
        WithBrokerConnector<impl Stream<Item = Result<UnixStream>>>,
    >
    where
        A: AsRef<Path> + Clone + 'static,
    {
        AsyncServerBuilder(
            PhantomData,
            WithBrokerConnector {
                connector: futures::stream::repeat(())
                    .then(move |()| {
                        log::info!(
                            "Connecting to {}...",
                            addr.as_ref().display()
                        );
                        UnixStream::connect(addr.clone())
                    })
                    .map_err(Error::Connect),
            },
        )
    }

    pub async fn tcp<A>(
        self,
        addr: A,
    ) -> Result<
        AsyncServerBuilder<
            P,
            WithListener<impl Stream<Item = Result<TcpStream>>>,
        >,
    >
    where
        A: ToSocketAddrs,
    {
        Ok(AsyncServerBuilder(
            PhantomData,
            WithListener {
                listener: TcpListenerStream::new(
                    TcpListener::bind(addr).await.map_err(Error::Bind)?,
                )
                .map_err(Error::Accept),
            },
        ))
    }

    pub fn tcp_broker<A>(
        self,
        addr: A,
    ) -> AsyncServerBuilder<
        P,
        WithBrokerConnector<impl Stream<Item = Result<TcpStream>>>,
    >
    where
        A: ToSocketAddrs + Clone + 'static,
    {
        AsyncServerBuilder(
            PhantomData,
            WithBrokerConnector {
                connector: futures::stream::repeat(())
                    .then(move |()| TcpStream::connect(addr.clone()))
                    .map_err(Error::Connect),
            },
        )
    }
}

impl<P> Default for AsyncServerBuilder<P, Init>
where
    P: Protocol,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<P, L, S> AsyncServerBuilder<P, WithListener<L>>
where
    P: Protocol,
    L: Stream<Item = Result<S>>,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn notls(
        self,
    ) -> AsyncServerBuilder<
        P,
        WithTls<impl Stream<Item = impl Future<Output = Result<S>>>>,
    > {
        AsyncServerBuilder(
            PhantomData,
            WithTls {
                listener: self.1.listener.map(ready),
            },
        )
    }

    pub fn tls(
        self,
        tls_config: Arc<ServerConfig>,
    ) -> AsyncServerBuilder<
        P,
        WithTls<impl Stream<Item = impl Future<Output = Result<TlsStream<S>>>>>,
    > {
        let acceptor = TlsAcceptor::from(tls_config);
        AsyncServerBuilder(
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

impl<P, L, Fut, S> AsyncServerBuilder<P, WithTls<L>>
where
    P: Protocol,
    L: Stream<Item = Fut> + Send + Unpin + 'static,
    Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Split + Send + Sync + Unpin + 'static,
{
    pub fn cbor(
        self,
    ) -> AsyncServerBuilder<
        P,
        WithEncoding<
            impl Stream<
                Item = impl Future<
                    Output = Result<
                        CborStream<
                            S,
                            AsyncRequest<serde_cbor::Value>,
                            AsyncResponse<serde_cbor::Value>,
                        >,
                    >,
                >,
            >,
            serde_cbor::Value,
        >,
    > {
        AsyncServerBuilder(
            PhantomData,
            WithEncoding {
                listener: self
                    .1
                    .listener
                    .map(|fut| fut.map_ok(CborStream::new)),
                value: PhantomData,
            },
        )
    }

    pub fn json(
        self,
    ) -> AsyncServerBuilder<
        P,
        WithEncoding<
            impl Stream<
                Item = impl Future<
                    Output = Result<
                        JsonStream<
                            S,
                            AsyncRequest<serde_json::Value>,
                            AsyncResponse<serde_json::Value>,
                            impl for<'a> Fn(
                                    &'a AsyncRequest<serde_json::Value>,
                                )
                                    -> Option<TraceCtx>
                                + Send
                                + Sync
                                + 'static,
                        >,
                    >,
                >,
            >,
            serde_json::Value,
        >,
    > {
        AsyncServerBuilder(
            PhantomData,
            WithEncoding {
                listener: self.1.listener.map(|fut| {
                    fut.map_ok(|stream| {
                        JsonStream::new(
                            stream,
                            |msg: &AsyncRequest<serde_json::Value>| {
                                Some(msg.trace_ctx.clone())
                            },
                        )
                    })
                }),
                value: PhantomData,
            },
        )
    }
}

impl<P, L, Fut, S, V> AsyncServerBuilder<P, WithEncoding<L, V>>
where
    P: Protocol,
    L: Stream<Item = Fut> + Send + Unpin + 'static,
    Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
    S: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
    V: GenericValue,
{
    pub fn handler_session<H>(self, handler: H) -> AsyncServer<P>
    where
        H: SessionHandler<P, V, S>
            + ServerRequestHandler<
                P,
                V,
                ExtraArgs = (),
                Session = <H as SessionHandler<P, V, S>>::Session,
                // Error = <H as SessionHandler<P, V, S>>::Error,
            > + Shutdown
            + 'static,
    {
        AsyncServer::new(self.1.listener, handler)
    }

    pub fn handler<H>(self, handler: H) -> AsyncServer<P>
    where
        H: RequestHandler<P, V, ExtraArgs = ()> + Shutdown + 'static,
    {
        AsyncServer::new(self.1.listener, SessionLess::new(handler))
    }

    pub fn handler_auth<F, H>(
        self,
        authenticate: F,
        handler: H,
    ) -> AsyncServer<P>
    where
        F: Fn(&S) -> Option<H::ExtraArgs> + Send + Sync + 'static,
        H: SessionHandler<P, V, S>
            + ServerRequestHandler<
                P,
                V,
                Session = <H as SessionHandler<P, V, S>>::Session,
                // Error = <H as SessionHandler<P, V, S>>::Error,
            > + Shutdown
            + 'static,
        H::ExtraArgs: Clone,
    {
        AsyncServer::new_auth(self.1.listener, authenticate, handler)
    }
}

impl<P, C, S> AsyncServerBuilder<P, WithBrokerConnector<C>>
where
    P: Protocol,
    C: Stream<Item = Result<S>>,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn tls(
        self,
        tls_config: Arc<ClientConfig>,
        server_name: ServerName<'static>,
    ) -> AsyncServerBuilder<
        P,
        WithBrokerTls<impl Stream<Item = Result<client::TlsStream<S>>>>,
    > {
        let connector = TlsConnector::from(tls_config);
        AsyncServerBuilder(
            PhantomData,
            WithBrokerTls {
                connector: Box::pin(self.1.connector.and_then(move |s| {
                    let connect = connector.connect(server_name.clone(), s);
                    async move { connect.await.map_err(Error::Connect) }
                })),
            },
        )
    }
}

impl<P, C, S> AsyncServerBuilder<P, WithBrokerTls<C>>
where
    P: Protocol,
    C: Stream<Item = Result<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn cbor(
        self,
    ) -> AsyncServerBuilder<P, WithBrokerEncoding<C, serde_cbor::Value>> {
        AsyncServerBuilder(
            PhantomData,
            WithBrokerEncoding {
                connector: self.1.connector,
                value: PhantomData,
            },
        )
    }

    pub fn json(
        self,
    ) -> AsyncServerBuilder<P, WithBrokerEncoding<C, serde_json::Value>> {
        AsyncServerBuilder(
            PhantomData,
            WithBrokerEncoding {
                connector: self.1.connector,
                value: PhantomData,
            },
        )
    }

    pub fn bincode(
        self,
    ) -> AsyncServerBuilder<P, WithBrokerEncoding<C, serde_json::Value>> {
        AsyncServerBuilder(
            PhantomData,
            WithBrokerEncoding {
                connector: self.1.connector,
                value: PhantomData,
            },
        )
    }
}

impl<P, C, V, S> AsyncServerBuilder<P, WithBrokerEncoding<C, V>>
where
    P: Protocol,
    C: Stream<Item = Result<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    V: GenericValue,
{
    pub fn handler<F, Fut, R, W, H>(
        self,
        broker_handler: F,
        handler: H,
    ) -> AsyncServer<P>
    where
        F: Fn(S) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(R, W)>> + Send + Unpin + 'static,
        R: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
        W: MsgWriteStream<(H::ExtraArgs, AsyncResponse<V>)>,
        H: RequestHandler<P, V> + Shutdown + 'static,
        H::ExtraArgs: Clone,
    {
        self.handler_session(broker_handler, SessionLess::new(handler))
    }

    pub fn handler_session<F, Fut, R, W, H>(
        self,
        broker_handler: F,
        handler: H,
    ) -> AsyncServer<P>
    where
        F: Fn(S) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(R, W)>> + Send + Unpin + 'static,
        R: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
        W: MsgWriteStream<(H::ExtraArgs, AsyncResponse<V>)>,
        H: ServerRequestHandler<P, V, Session = ()> + Shutdown + 'static,
        H::ExtraArgs: Clone,
    {
        AsyncServer::new_broker(
            self.1.connector.and_then(broker_handler),
            handler,
        )
    }
}
