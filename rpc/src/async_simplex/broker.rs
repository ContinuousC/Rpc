/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{
    collections::HashMap,
    future::Future,
    hash::Hash,
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use futures::{
    future::{AbortHandle, Abortable},
    stream::FuturesUnordered,
    Stream, StreamExt,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{Error, MsgReadStream, MsgWriteStream, Result};

pub struct AsyncBroker<Node> {
    handlers: FuturesUnordered<JoinHandle<Result<()>>>,
    term_sender: watch::Sender<bool>,
    _marker: PhantomData<Node>,
}

pub struct AsyncBrokerHandler<L, H> {
    listener: L,
    handler: H,
}

pub struct AsyncBrokerBuilder<Node: BrokerNode> {
    listeners: FuturesUnordered<JoinHandle<Result<()>>>,
    term_sender: watch::Sender<bool>,
    term_receiver: watch::Receiver<bool>,
    nodes: Arc<RwLock<HashMap<Node::Key, Node>>>,
}

pub type NodeMap<N> = HashMap<<N as BrokerNode>::Key, N>;

pub trait BrokerNode {
    type Key: Hash + Eq;
}

pub trait BrokerHandler<S> {
    type Key;
    type Node: BrokerNode;
    type ReadStream: MsgReadStream<Self::ReadMsg>;
    type WriteStream: MsgWriteStream<Self::WriteMsg>;
    type ReadMsg: 'static;
    type WriteMsg: 'static;

    fn get_key(&self, stream: &S) -> Result<Self::Key>;

    fn add_node(
        &self,
        nodes: &mut HashMap<<Self::Node as BrokerNode>::Key, Self::Node>,
        key: &Self::Key,
        sender: mpsc::Sender<Self::WriteMsg>,
    ) -> Result<()>;

    fn remove_node(
        &self,
        nodes: &mut HashMap<<Self::Node as BrokerNode>::Key, Self::Node>,
        key: &Self::Key,
    );

    fn get_node<'a>(
        &self,
        nodes: &'a HashMap<<Self::Node as BrokerNode>::Key, Self::Node>,
        key: &Self::Key,
    ) -> Option<&'a Self::Node>;

    fn make_msg_stream(
        &self,
        stream: S,
    ) -> (Self::ReadStream, Self::WriteStream);

    fn handle_message(
        &self,
        node: &Self::Node,
        key: &Self::Key,
        msg: Self::ReadMsg,
    ) -> std::result::Result<(), Self::WriteMsg>;
}

impl<N> AsyncBroker<N>
where
    N: BrokerNode + Send + Sync,
    N::Key: Send + Sync,
{
    pub fn builder() -> AsyncBrokerBuilder<N> {
        AsyncBrokerBuilder::new()
    }

    pub fn builder_with_nodes(
        nodes: Arc<RwLock<NodeMap<N>>>,
    ) -> AsyncBrokerBuilder<N> {
        AsyncBrokerBuilder::with_nodes(nodes)
    }

    pub(crate) fn new(
        handlers: FuturesUnordered<JoinHandle<Result<()>>>,
        term_sender: watch::Sender<bool>,
    ) -> Self {
        Self {
            handlers,
            term_sender,
            _marker: PhantomData,
        }
    }

    pub async fn shutdown(self) -> Result<()> {
        let _ = self.term_sender.send(true);
        self.await_shutdown().await
    }

    pub async fn await_shutdown(self) -> Result<()> {
        self.handlers
            .fold(Ok(()), |res, r| async move {
                res.and(r.unwrap_or(Err(Error::BrokerHandler)))
            })
            .await
    }
}

impl<N> AsyncBrokerBuilder<N>
where
    N: BrokerNode + Send + Sync,
    N::Key: Send + Sync,
{
    pub fn new() -> Self {
        Self::with_nodes(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn with_nodes(nodes: Arc<RwLock<NodeMap<N>>>) -> Self {
        let (term_sender, term_receiver) = watch::channel(false);
        Self {
            term_sender,
            term_receiver,
            listeners: FuturesUnordered::new(),
            nodes,
        }
    }

    pub fn build(self) -> AsyncBroker<N> {
        AsyncBroker::new(self.listeners, self.term_sender)
    }

    pub fn handler<L, Fut, S, H>(
        self,
        handler: AsyncBrokerHandler<L, H>,
    ) -> Self
    where
        L: Stream<Item = Fut> + Send + Unpin + 'static,
        Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
        S: AsyncRead + AsyncWrite + Send + 'static,
        H: BrokerHandler<S, Node = N> + Send + Sync + 'static,
        H::ReadMsg: Send,
        H::WriteMsg: Send,
        H::Key: Clone + Send + Sync,
        H::Node: Send + Sync,
        <H::Node as BrokerNode>::Key: Send + Sync,
    {
        self.listeners.push(
            handler.spawn(self.nodes.clone(), self.term_receiver.clone()),
        );
        self
    }
}

impl<N> Default for AsyncBrokerBuilder<N>
where
    N: BrokerNode + Send + Sync,
    N::Key: Send + Sync,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L, Fut, S, H> AsyncBrokerHandler<L, H>
where
    L: Stream<Item = Fut> + Send + Unpin + 'static,
    Fut: Future<Output = Result<S>> + Send + Unpin + 'static,
    S: AsyncRead + AsyncWrite + Send + 'static,
    H: BrokerHandler<S> + Send + Sync + 'static,
    H::ReadMsg: Send,
    H::WriteMsg: Send,
    H::Key: Clone + Send + Sync,
    H::Node: Send + Sync,
    <H::Node as BrokerNode>::Key: Send + Sync,
{
    pub(crate) fn new(listener: L, handler: H) -> Self {
        Self { listener, handler }
    }

    fn spawn(
        self,
        nodes: Arc<RwLock<NodeMap<H::Node>>>,
        term_receiver: watch::Receiver<bool>,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn(broker_listener(
            self.listener,
            Arc::new(self.handler),
            nodes,
            term_receiver,
        ))
    }
}

async fn broker_listener<L, Fut, S, H>(
    listener: L,
    handler: Arc<H>,
    nodes: Arc<RwLock<NodeMap<H::Node>>>,
    mut term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    L: Stream<Item = Fut> + Unpin,
    Fut: Future<Output = Result<S>> + Unpin,
    S: AsyncRead + AsyncWrite + 'static,
    H: BrokerHandler<S> + Send + Sync + 'static,
    H::ReadMsg: Send + 'static,
    H::WriteMsg: Send + 'static,
    H::Node: Send + Sync,
    H::Key: Clone + Send + Sync,
    <H::Node as BrokerNode>::Key: Send + Sync,
{
    let (abort_handle, abort_reg) = AbortHandle::new_pair();
    let mut handle =
        Abortable::new(listener, abort_reg).for_each_concurrent(None, {
            let term_receiver = term_receiver.clone();
            move |mut stream| {
                let handler = handler.clone();
                let nodes = nodes.clone();
                let mut term_receiver = term_receiver.clone();
                async move {
                    let stream = match abortable!(term_receiver, &mut stream) {
                        Some(s) => s,
                        None => return,
                    };
                    match stream {
                        Ok(stream) => {
                            if let Err(e) = handle_async_broker_stream(
                                stream,
                                handler,
                                nodes,
                                term_receiver,
                            )
                            .await
                            {
                                log::warn!("stream handler failed: {}", e);
                            }
                        }
                        Err(e) => {
                            log::warn!("failure on incoming stream: {}", e);
                        }
                    }
                }
            }
        });

    if abortable!(term_receiver, &mut handle).is_none() {
        abort_handle.abort();
        handle.await;
    }
    Ok(())
}

pub async fn handle_async_broker_stream<S, H>(
    stream: S,
    handler: Arc<H>,
    nodes: Arc<RwLock<NodeMap<H::Node>>>,
    term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    S: 'static,
    H: BrokerHandler<S> + Send + Sync + 'static,
    H::ReadMsg: Send + 'static,
    H::WriteMsg: Send + 'static,
    H::Key: Clone + Send,
    H::Node: Send + Sync,
    <H::Node as BrokerNode>::Key: Send + Sync,
{
    let key = handler.get_key(&stream)?;
    log::debug!("incoming connection");

    let (read_stream, write_stream) = handler.make_msg_stream(stream);
    let (msg_sender, msg_receiver) = mpsc::channel(1000);

    handler.add_node(&mut nodes.write().unwrap(), &key, msg_sender.clone())?;

    let w =
        tokio::spawn(writer(write_stream, msg_receiver, term_receiver.clone()));
    let r = reader(
        read_stream,
        handler.clone(),
        msg_sender,
        key.clone(),
        nodes.clone(),
        term_receiver,
    )
    .await;

    handler.remove_node(&mut nodes.write().unwrap(), &key);
    w.abort();
    let w = w.await;
    log::debug!("connection closed");
    r.or_else(|_| w.unwrap_or(Err(Error::BrokerHandler)))
}

async fn reader<S, H>(
    mut stream: H::ReadStream,
    handler: Arc<H>,
    sender: mpsc::Sender<H::WriteMsg>,
    key: H::Key,
    nodes: Arc<RwLock<NodeMap<H::Node>>>,
    mut term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    H: BrokerHandler<S> + Send + Sync,
{
    loop {
        let msg = match abortable!(term_receiver, stream.recv()) {
            Some(Ok(Some(msg))) => msg,
            Some(Ok(None)) | None => return Ok(()),
            Some(Err(e)) => return Err(e),
        };
        let res = {
            let nodes_read = nodes.read().unwrap();
            let node = handler
                .get_node(&nodes_read, &key)
                .ok_or(Error::MissingNode)?;
            handler.handle_message(node, &key, msg)
        };
        if let Err(err) = res {
            let _ = sender.send(err).await;
        }
    }
}

async fn writer<S, T>(
    mut stream: S,
    mut receiver: mpsc::Receiver<T>,
    mut term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    S: MsgWriteStream<T>,
    T: 'static,
{
    while let Some(Some(msg)) = abortable!(term_receiver, receiver.recv()) {
        stream.send(msg).await?;
    }
    Ok(())
}
