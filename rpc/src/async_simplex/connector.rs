/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{convert::TryInto, path::Path, sync::Arc};

use futures::{Stream, StreamExt, TryStreamExt};
use rustls::{pki_types::ServerName, ClientConfig};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, ToSocketAddrs, UnixStream},
};
use tokio_rustls::{client::TlsStream, TlsConnector};
use tokio_stream::wrappers::TcpListenerStream;

use crate::{
    msg_stream::Split, CborStream, Error, JsonStream, MsgStream, Result,
};

pub struct Connector<T>(pub(crate) T);

impl Connector<()> {
    pub fn unix<A>(addr: A) -> Connector<impl Stream<Item = Result<UnixStream>>>
    where
        A: AsRef<Path>,
    {
        Connector(
            futures::stream::repeat(())
                .then({
                    let sock_path = addr.as_ref().to_path_buf();
                    move |()| UnixStream::connect(sock_path.clone())
                })
                .map_err(Error::Connect),
        )
    }

    pub async fn tcp<A>(
        addr: A,
    ) -> Connector<impl Stream<Item = Result<TcpStream>>>
    where
        A: ToSocketAddrs + Clone + 'static,
    {
        Connector(
            futures::stream::repeat(())
                .then(move |()| TcpStream::connect(addr.clone()))
                .map_err(Error::Connect),
        )
    }

    pub async fn tcp_listener<A>(
        addr: A,
    ) -> Result<Connector<impl Stream<Item = Result<TcpStream>>>>
    where
        A: ToSocketAddrs + Clone + 'static,
    {
        Ok(Connector(
            TcpListenerStream::new(
                TcpListener::bind(addr).await.map_err(Error::Bind)?,
            )
            .map_err(Error::Accept),
        ))
    }
}

impl<T, S> Connector<T>
where
    T: Stream<Item = Result<S>> + Send + 'static,
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub fn tls(
        self,
        tls_config: Arc<ClientConfig>,
        server_name: &str,
    ) -> Result<Connector<impl Stream<Item = Result<TlsStream<S>>>>> {
        let connector = TlsConnector::from(tls_config);
        let server_name: ServerName<'static> = server_name
            .to_string()
            .try_into()
            .map_err(|e: rustls::pki_types::InvalidDnsNameError| {
                Error::ServerName(e.to_string())
            })?;
        Ok(Connector(
            self.0
                .and_then(move |s| {
                    let connect = connector.connect(server_name.clone(), s);
                    async move { connect.await.map_err(Error::Connect) }
                })
                .boxed(),
        ))
    }
}

impl<T, S> Connector<T>
where
    T: Stream<Item = Result<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Split + Send + Sync + Unpin + 'static,
{
    pub fn cbor<R, W>(
        self,
    ) -> Connector<impl Stream<Item = Result<impl MsgStream<R, W>>> + Unpin>
    where
        R: DeserializeOwned + Send + Sync + 'static,
        W: Serialize + Send + Sync + 'static,
    {
        Connector(self.0.map_ok(CborStream::new))
    }

    /// For backward compatibility.
    pub fn cbor_compat<R, W>(
        self,
        version: u64,
    ) -> Connector<impl Stream<Item = Result<impl MsgStream<R, W>>> + Unpin>
    where
        R: DeserializeOwned + Send + Sync + 'static,
        W: Serialize + Send + Sync + 'static,
    {
        Connector(
            self.0
                .and_then(move |s| async move {
                    let mut stream = CborStream::new_varint(s, false);
                    crate::handshake_client(&mut stream, version).await?;
                    Ok(stream.change_proto())
                })
                .boxed(),
        )
    }

    pub fn json<R, W>(
        self,
    ) -> Connector<impl Stream<Item = Result<impl MsgStream<R, W>>>>
    where
        R: DeserializeOwned + Send + Sync + 'static,
        W: Serialize + Send + Sync + 'static,
    {
        Connector(self.0.map_ok(|stream| JsonStream::new(stream, |_msg| None)))
    }
}
