/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use futures::{Stream, StreamExt, TryStreamExt};
use rustls::{pki_types::ServerName, ClientConfig};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpStream, ToSocketAddrs, UnixStream};
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use crate::msg_stream::Split;
use crate::{AsyncClient, CborStream, JsonStream, Protocol};
use crate::{Error, Result};

pub struct AsyncClientBuilder<P: Protocol, S>(PhantomData<P>, S);

pub struct Init(());
pub struct WithConnector<C> {
    connector: C,
}
pub struct WithTls<C> {
    connector: C,
}

impl<P> AsyncClientBuilder<P, Init>
where
    P: Protocol,
{
    pub fn new() -> Self {
        Self(PhantomData, Init(()))
    }

    pub fn unix<A>(
        self,
        addr: A,
    ) -> AsyncClientBuilder<
        P,
        WithConnector<impl Stream<Item = Result<UnixStream>>>,
    >
    where
        A: AsRef<Path> + 'static,
    {
        AsyncClientBuilder(
            PhantomData,
            WithConnector {
                connector: futures::stream::repeat(())
                    .then({
                        let sock_path = addr.as_ref().to_path_buf();
                        move |()| UnixStream::connect(sock_path.clone())
                    })
                    .map_err(Error::Connect),
            },
        )
    }

    pub async fn tcp<A>(
        self,
        addr: A,
    ) -> AsyncClientBuilder<
        P,
        WithConnector<impl Stream<Item = Result<TcpStream>>>,
    >
    where
        A: ToSocketAddrs + Clone + 'static,
    {
        AsyncClientBuilder(
            PhantomData,
            WithConnector {
                connector: futures::stream::repeat(())
                    .then(move |()| TcpStream::connect(addr.clone()))
                    .map_err(Error::Connect),
            },
        )
    }
}

impl<P> Default for AsyncClientBuilder<P, Init>
where
    P: Protocol,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<P, C, S> AsyncClientBuilder<P, WithConnector<C>>
where
    P: Protocol,
    C: Stream<Item = Result<S>>,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn notls(
        self,
    ) -> AsyncClientBuilder<P, WithTls<impl Stream<Item = Result<S>> + Unpin>>
    {
        AsyncClientBuilder(
            PhantomData,
            WithTls {
                connector: Box::pin(self.1.connector),
            },
        )
    }

    pub fn tls(
        self,
        tls_config: Arc<ClientConfig>,
        server_name: ServerName<'static>,
    ) -> AsyncClientBuilder<P, WithTls<impl Stream<Item = Result<TlsStream<S>>>>>
    {
        let connector = TlsConnector::from(tls_config);
        AsyncClientBuilder(
            PhantomData,
            WithTls {
                connector: {
                    Box::pin(self.1.connector.and_then(move |s| {
                        let connect = connector.connect(server_name.clone(), s);
                        async move { connect.await.map_err(Error::Connect) }
                    }))
                },
            },
        )
    }
}

impl<P, C, S> AsyncClientBuilder<P, WithTls<C>>
where
    P: Protocol,
    C: Stream<Item = Result<S>> + Send + Unpin + 'static,
    S: Split + 'static,
{
    pub fn cbor(self) -> AsyncClient<P, serde_cbor::Value, ()> {
        AsyncClient::new(self.1.connector.map_ok(CborStream::new))
    }

    pub fn json(self) -> AsyncClient<P, serde_json::Value, ()> {
        AsyncClient::new(
            self.1
                .connector
                .map_ok(|stream| JsonStream::new(stream, |_msg| None)),
        )
    }
}
