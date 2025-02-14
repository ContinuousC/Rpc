/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{collections::HashMap, hash::Hash, marker::PhantomData, sync::Arc};

use parking_lot::Mutex;
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};

use crate::{
    Error, GenericValue, MsgReadStream, MsgStream, MsgWriteStream, Protocol,
    RequestHandler, Result, TraceCtx,
};

use super::messages::{AsyncRequest, AsyncResponse};

pub struct AsyncClientConnection<P: Protocol, V, A> {
    reader: JoinHandle<()>,
    writer: JoinHandle<()>,
    term_sender: watch::Sender<bool>,
    req_sender: mpsc::Sender<(A, V, TraceCtx, oneshot::Sender<V>)>,
    _marker: PhantomData<P>,
}

impl<P, V> AsyncClientConnection<P, V, ()>
where
    P: Protocol,
    V: GenericValue,
{
    pub fn new<S>(stream: S) -> Self
    where
        S: MsgStream<AsyncResponse<V>, AsyncRequest<V>>,
    {
        let (read_stream, write_stream) = stream.split();
        Self::new_split(
            |requests| {
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
    }
}

pub(crate) type RequestMap<A, V> =
    Arc<Mutex<HashMap<(A, u64), (TraceCtx, oneshot::Sender<V>)>>>;

impl<P, V, A> AsyncClientConnection<P, V, A>
where
    P: Protocol,
    V: GenericValue,
    A: Hash + Eq + Clone + Send + Sync + 'static,
{
    pub fn new_split<R, W, F>(read_stream: F, write_stream: W) -> Self
    where
        R: MsgReadStream<(A, AsyncResponse<V>)>,
        W: MsgWriteStream<(A, AsyncRequest<V>)>,
        F: FnOnce(RequestMap<A, V>) -> R,
    {
        let (req_sender, req_receiver) = mpsc::channel(1000);

        let requests: RequestMap<A, V> = Arc::new(Mutex::new(HashMap::new()));
        let (term_sender, term_receiver) = watch::channel(false);

        let read_stream = read_stream(requests.clone());
        Self {
            reader: tokio::spawn(async_client_reader(
                read_stream,
                requests.clone(),
                term_sender.clone(),
                term_receiver.clone(),
            )),
            writer: tokio::spawn(async_client_writer(
                write_stream,
                req_receiver,
                requests,
                term_receiver,
            )),
            req_sender,
            term_sender,
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
        self.request_sender(
            extra_args,
            request,
            TraceCtx::current(),
            res_sender,
        )
        .await?;
        tokio::select! {
            res = res_receiver => res.map_err(|_| Error::ReceiveResult),
            _ = tokio::time::sleep(timeout) => Err(Error::Timeout)
        }
    }

    pub(crate) async fn request_sender(
        &self,
        extra_args: A,
        request: V,
        trace_ctx: TraceCtx,
        res_sender: oneshot::Sender<V>,
    ) -> Result<()> {
        self.req_sender
            .send((extra_args, request, trace_ctx, res_sender))
            .await
            .map_err(|_| Error::SendRequest)
    }

    pub async fn disconnected(&self) {
        let mut term = self.term_sender.subscribe();
        while !*term.borrow() {
            if let Err(e) = term.changed().await {
                log::warn!("Failed to receive term signal: {e}");
                break;
            }
        }
    }

    /**
    Shutdown sequence:
    - client       : req_sender is dropped
    - client writer: req_receiver returns None, task exits and
                     write_stream is shut down (this is not done
                     automatically on drop!)
    - server reader: read_stream returns EOF, task exits and
                     last res_sender is dropped
    - server writer: res_receiver returns None, task exits and
                     write_stream is shut down (would be done
                     automatically since both halves were dropped)
    - client reader: read_stream returns EOF, causing the task to
                     exit and stream to shut down
     */
    pub async fn shutdown(mut self) -> Result<()> {
        self.term_sender.send(true).map_err(|_| Error::SendTerm)?;
        let (r, w) = tokio::join!(&mut self.reader, &mut self.writer);
        r.map_err(Error::ClientReader)
            .and(w.map_err(Error::ServerReader))
    }
}

impl<P: Protocol, V, A> Drop for AsyncClientConnection<P, V, A> {
    fn drop(&mut self) {
        let _ = self.term_sender.send(true);
    }
}

impl<P, V, A> RequestHandler<P, V> for AsyncClientConnection<P, V, A>
where
    P: Protocol,
    V: GenericValue,
    A: Send + Sync + 'static,
{
    type ExtraArgs = A;
    type Error = Error;

    async fn handle(&self, request: V, extra_args: A) -> Result<V> {
        let (res_sender, res_receiver) = oneshot::channel();
        self.req_sender
            .send((extra_args, request, TraceCtx::current(), res_sender))
            .await
            .map_err(|_| Error::SendRequest)?;
        res_receiver.await.map_err(|_| Error::ReceiveResult)
    }
}

pub async fn async_client_writer<S, V, A>(
    mut write_stream: S,
    mut req_receiver: mpsc::Receiver<(A, V, TraceCtx, oneshot::Sender<V>)>,
    requests: RequestMap<A, V>,
    mut term_receiver: watch::Receiver<bool>,
) where
    V: GenericValue,
    S: MsgWriteStream<(A, AsyncRequest<V>)>,
    A: Hash + Eq + Clone + Send + Sync + 'static,
{
    let mut req_ids: HashMap<A, u64> = HashMap::new();
    while let Some(Some((extra_args, request, trace_ctx, res_sender))) =
        abortable!(term_receiver, req_receiver.recv())
    {
        let req_id = req_ids.get(&extra_args).copied().unwrap_or(0);
        let req = AsyncRequest {
            req_id,
            request,
            trace_ctx: trace_ctx.clone(),
        };

        log::debug!("sending request {}", req_id);
        let send = async {
            req.trace_ctx.set_parent();
            write_stream.send((extra_args.clone(), req)).await
        };
        #[cfg(feature = "tracing")]
        let send = {
            use tracing::Instrument;
            send.instrument(tracing::span!(
                tracing::Level::INFO,
                "write_stream_send"
            ))
        };
        match send.await {
            Ok(()) => {
                requests.lock().insert(
                    (extra_args.clone(), req_id),
                    (trace_ctx, res_sender),
                );
                req_ids.insert(extra_args, req_id + 1);
            }
            Err(e) => {
                log::warn!("client send failed: {}", e);
                break;
            }
        }
    }
    if let Err(e) = write_stream.shutdown().await {
        log::warn!("shutdown failed: {}", e);
    }
    log::debug!("async_client_writer exited");
}

pub async fn async_client_reader<S, V, A>(
    mut read_stream: S,
    requests: RequestMap<A, V>,
    term_sender: watch::Sender<bool>,
    mut term_receiver: watch::Receiver<bool>,
) where
    V: GenericValue,
    S: MsgReadStream<(A, AsyncResponse<V>)>,
    A: Hash + Eq + Send + Sync + 'static,
{
    loop {
        match abortable!(term_receiver, read_stream.recv()) {
            Some(Ok(Some((
                extra_args,
                AsyncResponse { req_id, response },
            )))) => {
                log::debug!("received response {}", req_id);
                match requests.lock().remove(&(extra_args, req_id)) {
                    Some((_, sender)) => {
                        if sender.send(response).is_err() {
                            log::warn!(
                                "failed to send response received from server"
                            );
                        }
                    }
                    None => {
                        log::warn!("got unsollicited response!")
                    }
                }
            }
            Some(Ok(None)) | None => break,
            Some(Err(e)) => {
                log::error!("client recv failed: {}", e);
                break;
            }
        }
    }
    if let Err(e) = term_sender.send(true) {
        log::warn!("Failed to send connection termination signal: {e}");
    }
    log::debug!("async_client_reader exited");
}
