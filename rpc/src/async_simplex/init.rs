/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{convert::TryInto, path::Path, sync::Arc};

use futures::{Stream, StreamExt, TryStreamExt};
use rustls::{ClientConfig, ServerConfig, ServerName};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, ToSocketAddrs, UnixListener, UnixStream},
};
use tokio_rustls::TlsAcceptor;
use tokio_stream::wrappers::{TcpListenerStream, UnixListenerStream};

use crate::{
    AsyncClient, AsyncRequest, AsyncResponse, AsyncServer, CborStream, Error,
    JsonStream, Protocol, RequestHandler, Result,
};

pub async fn async_tls_json_unix_server<P, A, H>(
    addr: A,
    handler: H,
    tls_config: Arc<ServerConfig>,
) -> Result<AsyncServer<P>>
where
    P: Protocol,
    A: AsRef<Path> + 'static,
    H: RequestHandler<P, serde_json::Value, ExtraArgs = ()>
        + Send
        + Sync
        + 'static,
{
    Ok(AsyncServer::new(
        tls_listener(tls_config, unix_listener(addr)?).map_ok(JsonStream::new),
        handler,
    ))
}

pub async fn async_tls_json_unix_client<P, A, S>(
    addr: A,
    tls_config: Arc<ClientConfig>,
    server_name: S,
) -> Result<AsyncClient<P, serde_json::Value, ()>>
where
    P: Protocol,
    A: AsRef<Path> + 'static,
    S: TryInto<ServerName>,
{
    Ok(AsyncClient::new(Box::pin(
        tls_connector(
            tls_config,
            server_name.try_into().ok().unwrap(),
            unix_connector(addr)?,
        )
        .map_ok(JsonStream::new),
    )))
}

pub async fn async_tls_json_tcp_server<P, A, H>(
    addr: A,
    handler: H,
    tls_config: Arc<ServerConfig>,
) -> Result<AsyncServer<P>>
where
    P: Protocol,
    A: ToSocketAddrs + 'static,
    H: RequestHandler<P, serde_json::Value, ExtraArgs = ()>
        + Send
        + Sync
        + 'static,
{
    Ok(AsyncServer::new(
        tls_listener(tls_config, tcp_listener(addr).await?)
            .map_ok(JsonStream::new),
        handler,
    ))
}

pub async fn async_tls_json_tcp_client<P, A, S>(
    addr: A,
    tls_config: Arc<ClientConfig>,
    server_name: S,
) -> Result<AsyncClient<P, serde_json::Value, ()>>
where
    P: Protocol,
    A: ToSocketAddrs + Send + Clone + 'static,
    S: TryInto<ServerName>,
{
    Ok(AsyncClient::new(Box::pin(
        tls_connector(
            tls_config,
            server_name.try_into().ok().unwrap(),
            tcp_connector(addr)?,
        )
        .map_ok(JsonStream::new),
    )))
}

pub async fn async_tls_cbor_unix_server<P, A, H>(
    addr: A,
    handler: H,
    tls_config: Arc<ServerConfig>,
) -> Result<AsyncServer<P>>
where
    P: Protocol,
    A: AsRef<Path> + 'static,
    H: RequestHandler<P, serde_cbor::Value, ExtraArgs = ()>
        + Send
        + Sync
        + 'static,
{
    Ok(AsyncServer::new(
        tls_listener(tls_config, unix_listener(addr)?).map_ok(CborStream::new),
        handler,
    ))
}

pub async fn async_tls_cbor_unix_client<P, A, S>(
    addr: A,
    tls_config: Arc<ClientConfig>,
    server_name: S,
) -> Result<AsyncClient<P, serde_cbor::Value, ()>>
where
    P: Protocol,
    A: AsRef<Path> + 'static,
    S: TryInto<ServerName>,
{
    Ok(AsyncClient::new(Box::pin(
        tls_connector(
            tls_config,
            server_name.try_into().ok().unwrap(),
            unix_connector(addr)?,
        )
        .map_ok(CborStream::new),
    )))
}

pub async fn async_tls_cbor_tcp_server<P, A, H>(
    addr: A,
    handler: H,
    tls_config: Arc<ServerConfig>,
) -> Result<AsyncServer<P>>
where
    P: Protocol,
    A: ToSocketAddrs + 'static,
    H: RequestHandler<P, serde_cbor::Value, ExtraArgs = ()>
        + Send
        + Sync
        + 'static,
{
    async_tls_cbor_tcp_server_auth(addr, |_| Some(()), handler, tls_config)
        .await
}

pub async fn async_tls_cbor_tcp_server_auth<P, A, H, F>(
    addr: A,
    authenticate: F,
    handler: H,
    tls_config: Arc<ServerConfig>,
) -> Result<AsyncServer<P>>
where
    P: Protocol,
    A: ToSocketAddrs + 'static,
    F: Fn(
            &CborStream<
                tokio_rustls::server::TlsStream<TcpStream>,
                AsyncRequest<serde_cbor::Value>,
                AsyncResponse<serde_cbor::Value>,
            >,
        ) -> Option<H::ExtraArgs>
        + Send
        + 'static,
    H: RequestHandler<P, serde_cbor::Value> + Send + Sync + 'static,
    H::ExtraArgs: Clone + Send + Sync,
{
    Ok(AsyncServer::new_auth(
        tls_listener(tls_config, tcp_listener(addr).await?)
            .map_ok(CborStream::new),
        authenticate,
        handler,
    ))
}

pub async fn async_tls_cbor_tcp_client<P, A, S>(
    addr: A,
    tls_config: Arc<ClientConfig>,
    server_name: S,
) -> Result<AsyncClient<P, serde_cbor::Value, ()>>
where
    P: Protocol,
    A: ToSocketAddrs + Send + Clone + 'static,
    S: TryInto<ServerName>,
{
    Ok(AsyncClient::new(Box::pin(
        tls_connector(
            tls_config,
            server_name.try_into().ok().unwrap(),
            tcp_connector(addr)?,
        )
        .map_ok(CborStream::new),
    )))
}

fn unix_listener<A: AsRef<Path>>(
    addr: A,
) -> Result<impl Stream<Item = Result<UnixStream>>> {
    Ok(
        UnixListenerStream::new(UnixListener::bind(addr).map_err(Error::Bind)?)
            .map_err(Error::Accept),
    )
}

fn unix_connector<A: AsRef<Path>>(
    addr: A,
) -> Result<impl Stream<Item = Result<UnixStream>>> {
    Ok(futures::stream::repeat(())
        .then({
            let sock_path = addr.as_ref().to_path_buf();
            move |()| UnixStream::connect(sock_path.clone())
        })
        .map_err(Error::Connect))
}

async fn tcp_listener<A: ToSocketAddrs>(
    addr: A,
) -> Result<impl Stream<Item = Result<TcpStream>>> {
    Ok(TcpListenerStream::new(
        TcpListener::bind(addr).await.map_err(Error::Bind)?,
    )
    .map_err(Error::Accept))
}

fn tcp_connector<A: ToSocketAddrs + Clone + 'static>(
    addr: A,
) -> Result<impl Stream<Item = Result<TcpStream>>> {
    Ok(futures::stream::repeat(())
        .then(move |()| TcpStream::connect(addr.clone()))
        .map_err(Error::Accept))
}

fn tls_listener<L, S>(
    tls_config: Arc<ServerConfig>,
    listener: L,
) -> impl Stream<Item = Result<tokio_rustls::server::TlsStream<S>>> + Unpin
where
    L: Stream<Item = Result<S>> + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let acceptor = TlsAcceptor::from(tls_config);

    Box::pin(listener.and_then(move |s| {
        let accept = acceptor.accept(s);
        async move { accept.await.map_err(Error::Accept) }
    }))
}

fn tls_connector<L, S>(
    tls_config: Arc<ClientConfig>,
    server_name: ServerName,
    connector: L,
) -> impl Stream<Item = Result<impl AsyncRead + AsyncWrite>>
where
    L: Stream<Item = Result<S>>,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tlsconn = tokio_rustls::TlsConnector::from(tls_config);

    connector.and_then(move |s| {
        let connect = tlsconn.connect(server_name.clone(), s);
        async move { connect.await.map_err(Error::Connect) }
    })
}
