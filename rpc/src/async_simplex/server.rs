/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{
    future::Future,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use futures::{
    future::{AbortHandle, Abortable},
    Stream, StreamExt,
};

use tokio::{sync::watch, task::JoinHandle};

use crate::{
    handler::IntoSessionHandler, AsyncRequest, AsyncResponse,
    AsyncServerConnection, Error, GenericValue, MsgReadStream, MsgStream,
    MsgWriteStream, Protocol, Result, ServerRequestHandler, SessionHandler,
    Shutdown,
};

use super::AsyncServerBuilder;

pub struct AsyncServer<P> {
    listener: JoinHandle<()>,
    term_sender: watch::Sender<bool>,
    _marker: PhantomData<P>,
}

impl<P> AsyncServer<P>
where
    P: Protocol,
{
    pub fn builder() -> AsyncServerBuilder<P, super::server_builder::Init> {
        AsyncServerBuilder::new()
    }

    pub fn new<L, Fut, S, H, V>(listener: L, handler: H) -> Self
    where
        L: Stream<Item = Fut> + Send + Unpin + 'static,
        Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
        S: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
        H: IntoSessionHandler<P, V, S> + 'static,
        H::Handler: SessionHandler<P, V, S>
            + ServerRequestHandler<
                P,
                V,
                ExtraArgs = (),
                Session = <H::Handler as SessionHandler<P, V, S>>::Session,
                // Error = <H::Handler as SessionHandler<P, V, S>>::Error,
            > + Shutdown,
        V: GenericValue,
    {
        Self::new_auth(listener, |_| Some(()), handler)
    }

    pub fn new_auth<L, Fut, S, F, V, H, I>(
        listener: L,
        authenticate: F,
        handler: H,
    ) -> Self
    where
        L: Stream<Item = Fut> + Send + Unpin + 'static,
        Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
        S: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
        F: Fn(&S) -> Option<I::ExtraArgs> + Send + Sync + 'static,
        H: IntoSessionHandler<P, V, S, Handler = I> + 'static,
        I: SessionHandler<P, V, S>
            + ServerRequestHandler<
                P,
                V,
                Session = <H::Handler as SessionHandler<P, V, S>>::Session,
                // Error = <H::Handler as SessionHandler<P, V, S>>::Error,
            > + Shutdown
            + 'static,
        I::ExtraArgs: Clone,
        V: GenericValue,
    {
        let (term_sender, term_receiver) = watch::channel(false);
        Self {
            listener: tokio::spawn(server_listener::<P, _, _, _, _, _, _>(
                listener,
                authenticate,
                handler.into_session_handler(),
                term_receiver,
            )),
            term_sender,
            _marker: PhantomData,
        }
    }

    pub fn new_broker<C, R, W, V, H>(connector: C, handler: H) -> Self
    where
        C: Stream<Item = Result<(R, W)>> + Send + Unpin + 'static,
        R: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
        W: MsgWriteStream<(H::ExtraArgs, AsyncResponse<V>)>,
        H: ServerRequestHandler<P, V, Session = ()> + Shutdown + 'static,
        H::ExtraArgs: Clone,
        V: GenericValue,
    {
        let (term_sender, term_receiver) = watch::channel(false);
        Self {
            listener: tokio::spawn(server_listener_broker::<P, _, _, _, _, _>(
                connector,
                handler,
                term_receiver,
            )),
            term_sender,
            _marker: PhantomData,
        }
    }

    pub async fn shutdown(self) -> Result<()> {
        self.term_sender.send(true).map_err(|_| Error::SendTerm)?;
        self.await_shutdown().await
    }

    pub async fn await_shutdown(mut self) -> Result<()> {
        (&mut self.listener).await.map_err(Error::ServerListener)
    }
}

impl<P> Drop for AsyncServer<P> {
    fn drop(&mut self) {
        let _ = self.term_sender.send(true);
    }
}

async fn server_listener<P, L, Fut, S, F, V, H>(
    listener: L,
    authenticate: F,
    handler: H,
    mut term_receiver: watch::Receiver<bool>,
) where
    P: Protocol,
    L: Stream<Item = Fut> + Unpin,
    Fut: Future<Output = Result<S>> + Unpin,
    S: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
    F: Fn(&S) -> Option<H::ExtraArgs> + Send + Sync + 'static,
    V: GenericValue,
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
    let authenticate = Arc::new(authenticate);
    let connections = Arc::new(Mutex::new(Vec::new()));
    let handler = Arc::new(handler);

    let (abort_handle, abort_reg) = AbortHandle::new_pair();
    let mut server =
        Abortable::new(listener, abort_reg).for_each_concurrent(None, |r| {
            let connections = connections.clone();
            let authenticate = authenticate.clone();
            let handler = handler.clone();
            async move {
                log::debug!("incoming connection");
                match r.await {
                    Ok(stream) => {
                        match handler
                            .session(&stream)
                            .await
                            .map(|s| (s, authenticate(&stream)))
                        {
                            Ok((session, Some(extra_args))) => {
                                log::debug!(
                                    "incoming connection authenticated"
                                );
                                let conn =
                                    AsyncServerConnection::<P>::new_session(
                                        stream,
                                        handler.clone(),
                                        session,
                                        extra_args,
                                    );
                                let mut conns = connections.lock().unwrap();
                                conns.push(conn);

                                // Change to drain_filter() when stabilized.
                                let mut i = 0;
                                while i < conns.len() {
                                    if !conns[i].is_active() {
                                        log::debug!(
                                            "cleaning up server connection #{}",
                                            i
                                        );
                                        let _ = conns.swap_remove(i);
                                        // tokio::spawn(async move {
                                        //     conn.await_shutdown().await
                                        // });
                                    } else {
                                        i += 1;
                                    }
                                }
                                log::debug!(
                                    "{} server connections left after cleanup",
                                    conns.len()
                                );
                            }
                            Ok((_, None)) => {
                                log::warn!("authentication failed");
                            }
                            Err(e) => {
                                log::warn!(
                                    "failed to intialize session: {}",
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("failed to accept: {}", e);
                    }
                }
            }
        });

    abortable!(term_receiver, &mut server);
    abort_handle.abort();
    server.await;

    if let Some(connections) = Arc::try_unwrap(connections)
        .ok()
        .and_then(|cs| cs.into_inner().ok())
    {
        for connection in connections {
            let _ = connection.await_shutdown().await;
        }
    }

    match Arc::try_unwrap(handler) {
        Ok(handler) => {
            if let Err(e) = handler.shutdown().await {
                log::warn!("connection handler shutdown failed: {}", e)
            }
        }
        Err(_) => {
            log::warn!("cannot shutdown handler because it is still in use")
        }
    }
}

async fn server_listener_broker<P, C, R, W, V, H>(
    mut connector: C,
    handler: H,
    mut term_receiver: watch::Receiver<bool>,
) where
    P: Protocol,
    C: Stream<Item = Result<(R, W)>> + Send + Unpin + 'static,
    R: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
    W: MsgWriteStream<(H::ExtraArgs, AsyncResponse<V>)>,
    V: GenericValue,
    H: ServerRequestHandler<P, V, Session = ()> + Shutdown + 'static,
    H::ExtraArgs: Clone,
{
    let handler = Arc::new(handler);

    while let Some(Some(conn)) = abortable!(term_receiver, connector.next()) {
        log::debug!("incoming connection");
        let (read_stream, write_stream) = match conn {
            Ok(streams) => streams,
            Err(e) => {
                log::warn!("failed to connect: {}", e);
                continue;
            }
        };

        if let Some(Err(e)) = pin_abortable!(
            term_receiver,
            AsyncServerConnection::new_split_session(
                read_stream,
                write_stream,
                handler.clone(),
                ()
            )
            .await_shutdown()
        ) {
            log::warn!("connection failed: {}", e)
        }
    }

    match Arc::try_unwrap(handler) {
        Ok(handler) => {
            if let Err(e) = handler.shutdown().await {
                log::warn!("connection handler shutdown failed: {}", e);
            }
        }
        Err(_) => {
            log::warn!("cannot shutdown handler because it is still in use")
        }
    }
}
