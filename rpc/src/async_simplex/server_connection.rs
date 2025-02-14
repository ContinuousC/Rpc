/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{marker::PhantomData, sync::Arc};

use futures::TryFutureExt;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    AsyncRequest, AsyncResponse, Error, GenericValue, MsgReadStream, MsgStream,
    MsgWriteStream, Protocol, RequestHandler, Result, ServerRequestHandler,
    SessionLess, TraceCtx,
};

pub struct AsyncServerConnection<P: Protocol> {
    reader: JoinHandle<()>,
    writer: JoinHandle<()>,
    term_sender: watch::Sender<bool>,
    _marker: PhantomData<P>,
}

impl<P: Protocol> AsyncServerConnection<P> {
    pub fn new<V, S, H>(stream: S, handler: H, extra_args: H::ExtraArgs) -> Self
    where
        V: GenericValue,
        S: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
        H: RequestHandler<P, V> + Clone + 'static,
        H::ExtraArgs: Clone,
    {
        Self::new_session(stream, SessionLess::new(handler), (), extra_args)
    }

    pub fn new_session<V, S, H>(
        stream: S,
        handler: H,
        session: H::Session,
        extra_args: H::ExtraArgs,
    ) -> Self
    where
        V: GenericValue,
        S: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
        H: ServerRequestHandler<P, V> + Clone + 'static,
        H::ExtraArgs: Clone,
    {
        let (read_stream, write_stream) = stream.split();
        Self::new_split_session(
            read_stream.map(move |req| (extra_args.clone(), req)),
            write_stream.map(move |(_, res)| res),
            handler,
            session,
        )
    }

    pub fn new_split<V, R, W, H>(
        read_stream: R,
        write_stream: W,
        handler: H,
    ) -> Self
    where
        V: GenericValue,
        R: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
        W: MsgWriteStream<(H::ExtraArgs, AsyncResponse<V>)>,
        H: RequestHandler<P, V> + Clone + 'static,
        H::ExtraArgs: Clone,
    {
        Self::new_split_session(
            read_stream,
            write_stream,
            SessionLess::new(handler),
            (),
        )
    }

    pub fn new_split_session<V, R, W, H>(
        read_stream: R,
        write_stream: W,
        handler: H,
        session: H::Session,
    ) -> Self
    where
        V: GenericValue,
        R: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
        W: MsgWriteStream<(H::ExtraArgs, AsyncResponse<V>)>,
        H: ServerRequestHandler<P, V> + Clone + 'static,
        H::ExtraArgs: Clone,
    {
        let (term_sender, term_receiver) = watch::channel(false);
        let (res_sender, res_receiver) = mpsc::channel(1000);

        Self {
            reader: tokio::spawn(async_connection_reader(
                read_stream,
                handler,
                Arc::new(session),
                res_sender,
                term_receiver,
            )),
            writer: tokio::spawn(async_connection_writer(
                write_stream,
                res_receiver,
            )),
            term_sender,
            //shutdown_warning: ShutdownWarning(true),
            _marker: PhantomData,
        }
    }

    pub fn is_active(&self) -> bool {
        //!self.term_sender.is_closed()
        !self.reader.is_finished() && !self.writer.is_finished()
    }

    pub async fn shutdown(self) -> Result<()> {
        self.term_sender.send(true).map_err(|_| Error::SendTerm)?;
        self.await_shutdown().await
    }

    pub async fn await_shutdown(mut self) -> Result<()> {
        let (r, w) = tokio::join!(&mut self.reader, &mut self.writer);
        r.map_err(Error::ServerReader)
            .and(w.map_err(Error::ServerWriter))
    }
}

impl<P: Protocol> Drop for AsyncServerConnection<P> {
    fn drop(&mut self) {
        log::debug!("AsyncServerConnection::drop");
        let _ = self.term_sender.send(true);
    }
}

async fn async_connection_reader<P, S, V, H>(
    mut read_stream: S,
    handler: H,
    session: Arc<H::Session>,
    res_sender: mpsc::Sender<(H::ExtraArgs, TraceCtx, AsyncResponse<V>)>,
    mut term_receiver: watch::Receiver<bool>,
) where
    P: Protocol,
    V: GenericValue,
    S: MsgReadStream<(H::ExtraArgs, AsyncRequest<V>)>,
    H: ServerRequestHandler<P, V> + Clone + 'static,
    H::ExtraArgs: Clone,
{
    log::debug!("waiting for requests");
    while let Some(Ok(Some((extra_args, req)))) = abortable!(term_receiver, {
        read_stream.recv().map_err(|e| {
            log::error!("server recv failed: {}", e);
            e
        })
    }) {
        log::debug!("received request {}", req.req_id);
        let res_sender = res_sender.clone();
        let handler = handler.clone();

        tokio::spawn({
            let session = session.clone();
            let handle = async move {
                req.trace_ctx.set_parent();
                match handler
                    .handle_with_session(
                        &session,
                        req.request,
                        extra_args.clone(),
                    )
                    .await
                {
                    Ok(response) => {
                        if let Err(e) = res_sender
                            .send((
                                extra_args,
                                req.trace_ctx,
                                AsyncResponse {
                                    req_id: req.req_id,
                                    response,
                                },
                            ))
                            .await
                        {
                            log::warn!("server request send failed: {}", e);
                        }
                    }
                    Err(e) => {
                        log::warn!("request handler failed: {}", e);
                    }
                }
            };
            #[cfg(feature = "tracing")]
            let handle = {
                use tracing::Instrument;
                handle.instrument(tracing::span!(
                    tracing::Level::INFO,
                    "rpc_server_handler"
                ))
            };
            handle
        });
    }
    log::debug!("async_connection_reader finished");
}

async fn async_connection_writer<S, T, V>(
    mut write_stream: S,
    mut res_receiver: mpsc::Receiver<(T, TraceCtx, AsyncResponse<V>)>,
) where
    V: GenericValue,
    S: MsgWriteStream<(T, AsyncResponse<V>)>,
    T: 'static,
{
    log::debug!("starting async_connection_writer");
    while let Some((extra_args, trace_ctx, res)) = res_receiver.recv().await {
        log::debug!("sending response {}", res.req_id);
        let send = async {
            trace_ctx.set_parent();
            write_stream.send((extra_args, res)).await
        };
        #[cfg(feature = "tracing")]
        let send = {
            use tracing::Instrument;
            send.instrument(tracing::span!(
                tracing::Level::INFO,
                "write_stream_send"
            ))
        };
        if let Err(e) = send.await {
            log::error!("server send failed: {}", e);
            break;
        }
    }
    if let Err(e) = write_stream.shutdown().await {
        log::warn!("server write stream shutdown failed: {}", e);
    }
    log::debug!("async_connection_writer finished");
}
