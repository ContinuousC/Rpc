/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{hash::Hash, marker::PhantomData};

use futures::{Stream, StreamExt, TryStreamExt};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

use crate::{
    AsyncClientConnection, AsyncRequest, AsyncResponse, Error, GenericValue,
    MsgReadStream, MsgStream, MsgWriteStream, Protocol, RequestHandler, Result,
    TraceCtx,
};

use super::AsyncClientBuilder;
use super::{client_builder::Init, client_connection::RequestMap};

pub struct AsyncClient<P: Protocol, V: GenericValue, A> {
    connector: JoinHandle<()>,
    req_sender: mpsc::Sender<(A, V, TraceCtx, oneshot::Sender<V>)>,
    term_sender: watch::Sender<bool>,
    connected: watch::Receiver<bool>,
    _marker: PhantomData<P>,
}

impl<P, V> AsyncClient<P, V, ()>
where
    V: GenericValue,
    P: Protocol,
{
    pub fn new<C, S>(connector: C) -> Self
    where
        C: Stream<Item = Result<S>> + Send + Unpin + 'static,
        S: MsgStream<AsyncResponse<V>, AsyncRequest<V>>,
    {
        Self::new_broker(connector.map_ok(|stream| {
            let (read_stream, write_stream) = stream.split();
            (
                |requests: RequestMap<(), V>| {
                    read_stream
                        .with_context_extractor(move |res| {
                            requests
                                .lock()
                                .get(&((), res.req_id))
                                .map(|(ctx, _)| ctx.clone())
                        })
                        .map(|res| ((), res))
                },
                write_stream.map(|((), req)| req),
            )
        }))
    }
}

impl<P, V, A> AsyncClient<P, V, A>
where
    V: GenericValue,
    A: Hash + Eq + Clone + Send + Sync + 'static,
    P: Protocol,
{
    pub fn builder() -> AsyncClientBuilder<P, Init> {
        AsyncClientBuilder::new()
    }

    pub fn new_broker<C, F, R, W>(connector: C) -> Self
    where
        C: Stream<Item = Result<(F, W)>> + Send + Unpin + 'static,
        R: MsgReadStream<(A, AsyncResponse<V>)>,
        W: MsgWriteStream<(A, AsyncRequest<V>)>,
        F: FnOnce(RequestMap<A, V>) -> R + Send + 'static,
    {
        Self::new_broker_unconnected(
            connector,
            V::serialize_from(serde_json::json!({"Err": "not connected!"}))
                .ok(),
        )
    }

    pub fn new_broker_unconnected<C, F, R, W>(
        connector: C,
        unconnected: Option<V>,
    ) -> Self
    where
        C: Stream<Item = Result<(F, W)>> + Send + Unpin + 'static,
        R: MsgReadStream<(A, AsyncResponse<V>)>,
        W: MsgWriteStream<(A, AsyncRequest<V>)>,
        F: FnOnce(RequestMap<A, V>) -> R + Send + 'static,
    {
        let (connected_sender, connected) = watch::channel(false);
        let (req_sender, req_receiver) = mpsc::channel(1000);
        let (term_sender, term_receiver) = watch::channel(false);

        Self {
            connector: tokio::spawn(client_connector::<P, _, _, _, _, V, A>(
                connector,
                connected_sender,
                req_receiver,
                unconnected,
                term_receiver,
            )),
            req_sender,
            term_sender,
            connected,
            _marker: PhantomData,
        }
    }

    pub async fn request(&self, extra_args: A, request: V) -> Result<V> {
        self.request_timeout(
            extra_args,
            request,
            std::time::Duration::from_secs(60),
        )
        .await
    }

    pub async fn request_timeout(
        &self,
        extra_args: A,
        request: V,
        timeout: std::time::Duration,
    ) -> Result<V> {
        let (res_sender, res_receiver) = oneshot::channel();
        self.req_sender
            .send((extra_args, request, TraceCtx::current(), res_sender))
            .await
            .map_err(|_| Error::SendRequest)?;
        tokio::select! {
            res = res_receiver => res.map_err(|_| Error::ReceiveResult),
            _ = tokio::time::sleep(timeout) => Err(Error::Timeout)
        }
    }

    pub async fn connected(&self, timeout: std::time::Duration) -> Result<()> {
        let mut connected = self.connected.clone();
        let timeout = tokio::time::sleep(timeout);
        tokio::pin!(timeout);
        loop {
            let conn = *connected.borrow();
            match conn {
                true => return Ok(()),
                false => tokio::select! {
                    _ = &mut timeout => return Err(Error::Timeout),
                    _ = connected.changed() => continue
                },
            }
        }
    }

    pub async fn shutdown(mut self) -> Result<()> {
        self.term_sender.send(true).map_err(|_| Error::SendTerm)?;
        (&mut self.connector).await.map_err(Error::ClientReader)
    }
}

impl<P: Protocol, V: GenericValue, A> Drop for AsyncClient<P, V, A> {
    fn drop(&mut self) {
        let _ = self.term_sender.send(true);
    }
}

impl<P, V, A> RequestHandler<P, V> for AsyncClient<P, V, A>
where
    P: Protocol,
    V: GenericValue,
    A: Clone + Send + Sync + 'static,
{
    type ExtraArgs = A;
    type Error = Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "rpc_client_handler",
            skip(self, request, extra_args)
        )
    )]
    async fn handle(&self, request: V, extra_args: A) -> Result<V> {
        let (res_sender, res_receiver) = oneshot::channel();
        self.req_sender
            .send((extra_args, request, TraceCtx::current(), res_sender))
            .await
            .map_err(|_| Error::SendRequest)?;
        res_receiver.await.map_err(|_| Error::ReceiveResult)
    }
}

async fn client_connector<P, C, F, R, W, V, A>(
    mut connector: C,
    connected_sender: watch::Sender<bool>,
    mut req_receiver: mpsc::Receiver<(A, V, TraceCtx, oneshot::Sender<V>)>,
    unconnected: Option<V>,
    mut term_receiver: watch::Receiver<bool>,
) where
    P: Protocol,
    V: GenericValue,
    A: Hash + Eq + Clone + Send + Sync + 'static,
    C: Stream<Item = Result<(F, W)>> + Unpin,
    R: MsgReadStream<(A, AsyncResponse<V>)>,
    W: MsgWriteStream<(A, AsyncRequest<V>)>,
    F: FnOnce(RequestMap<A, V>) -> R,
{
    while !*term_receiver.borrow() {
        let conn = tokio::select! {
            maybe_conn = connector.next() => match maybe_conn {
                Some(conn) => conn,
                None => break,
            },
            maybe_req = req_receiver.recv() => match maybe_req {
                Some((_,_,_, res_sender)) => {
                    if let Some(msg) = unconnected.as_ref() {
                        if res_sender.send(msg.clone()).is_err() {
                            log::warn!("failed to send \"not connected\" response");
                        }
                    }
                    continue;
                },
                None => break
            },
            _ = term_receiver.changed() => continue
        };

        let (read_stream, write_stream) = match conn {
            Ok(s) => {
                log::debug!("connection succeeded");
                s
            }
            Err(e) => {
                log::warn!("failed connection attempt: {}", e);
                abortable_sleep!(
                    term_receiver,
                    std::time::Duration::from_secs(10)
                );
                continue;
            }
        };

        let conn = AsyncClientConnection::<P, V, A>::new_split(
            read_stream,
            write_stream,
        );

        let _ = connected_sender.send(true);

        while !*term_receiver.borrow() {
            let (extra_args, req, trace_ctx, res_sender) = tokio::select! {
                maybe_req = req_receiver.recv() => match maybe_req {
                    Some(req) => req,
                    None => break,
                },
                _ = conn.disconnected() => break,
                _ = term_receiver.changed() => continue,
            };

            if let Err(e) = conn
                .request_sender(extra_args, req, trace_ctx, res_sender)
                .await
            {
                log::warn!("failed to send request: {}", e);
            }
        }

        let _ = connected_sender.send(false);
        pin_abortable!(term_receiver, conn.shutdown());
        abortable_sleep!(term_receiver, std::time::Duration::from_secs(10));
    }
}
