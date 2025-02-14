/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{marker::PhantomData, sync::Arc};

use futures::{FutureExt, Stream, StreamExt};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    Connector, Error, MsgReadStream, MsgStream, MsgWriteStream, Result,
};

pub struct AsyncBrokerClient<R, W> {
    connector: JoinHandle<Result<()>>,
    connected: watch::Receiver<bool>,
    broker_sender: mpsc::Sender<W>,
    term_sender: watch::Sender<bool>,
    _marker: PhantomData<R>,
}

impl<R, W> AsyncBrokerClient<R, W>
where
    R: Send + 'static,
    W: Send + 'static,
{
    pub fn new<C, S, H>(connector: Connector<C>, handler: H) -> Result<Self>
    where
        C: Stream<Item = Result<S>> + Send + Unpin + 'static,
        S: MsgStream<R, W>,
        H: Fn(R) -> std::result::Result<(), W> + Send + Sync + 'static,
    {
        Self::new_unconnected(connector, handler, |_| {})
    }

    pub fn new_unconnected<C, S, H, U>(
        connector: Connector<C>,
        handler: H,
        unconnected: U,
    ) -> Result<Self>
    where
        C: Stream<Item = Result<S>> + Send + Unpin + 'static,
        S: MsgStream<R, W>,
        H: Fn(R) -> std::result::Result<(), W> + Send + Sync + 'static,
        U: Fn(W) + Send + Sync + 'static,
    {
        let (connected_sender, connected) = watch::channel(false);
        let (term_sender, term_receiver) = watch::channel(false);
        let (broker_sender, broker_receiver) = mpsc::channel(1000);

        Ok(Self {
            connector: tokio::spawn(broker_connector(
                connector,
                connected_sender,
                handler,
                unconnected,
                broker_sender.clone(),
                broker_receiver,
                term_receiver,
            )),
            connected,
            broker_sender,
            term_sender,
            _marker: PhantomData,
        })
    }

    pub fn sender(&self) -> mpsc::Sender<W> {
        self.broker_sender.clone()
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

    pub async fn shutdown(self) -> Result<()> {
        self.term_sender.send(true).map_err(|_| Error::SendTerm)?;
        std::mem::drop(self.broker_sender);
        self.connector.await.unwrap_or(Err(Error::BrokerHandler))
    }
}

async fn broker_connector<C, S, R, W, H, U>(
    mut connector: Connector<C>,
    connected_sender: watch::Sender<bool>,
    handler: H,
    unconnected: U,
    broker_sender: mpsc::Sender<W>,
    mut broker_receiver: mpsc::Receiver<W>,
    mut term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    C: Stream<Item = Result<S>> + Unpin,
    S: MsgStream<R, W>,
    H: Fn(R) -> std::result::Result<(), W> + Send + Sync + 'static,
    U: Fn(W) + Send + Sync + 'static,
    R: Send + 'static,
    W: Send + 'static,
{
    let handler = Arc::new(handler);
    while !*term_receiver.borrow() {
        let conn = tokio::select! {
            maybe_conn = connector.0.next() => match maybe_conn {
                Some(conn) => conn,
                None => break,
            },
            maybe_req = broker_receiver.recv() => match maybe_req {
                Some(msg) => {
                    unconnected(msg);
                    continue;
                },
                None => break
            },
            _ = term_receiver.changed() => continue
        };

        let stream = match conn {
            Ok(stream) => stream,
            Err(e) => {
                log::warn!("failed to connect to broker: {}", e);
                abortable_sleep!(
                    term_receiver,
                    std::time::Duration::from_secs(10)
                );
                continue;
            }
        };
        let (read_stream, write_stream) = stream.split();
        let (stream_term_sender, stream_term_receiver) = watch::channel(false);
        let mut reader = tokio::spawn(broker_reader(
            read_stream,
            broker_sender.clone(),
            handler.clone(),
            stream_term_receiver.clone(),
        ));
        let mut writer = broker_writer(
            write_stream,
            &mut broker_receiver,
            stream_term_receiver.clone(),
        )
        .boxed();

        let _ = connected_sender.send(true);
        let (r, w) = loop {
            tokio::select! {
                r = &mut reader => {
                    stream_term_sender.send(true).map_err(|_| Error::SendTerm)?;
                    break (r, writer.await);
                },
                w = &mut writer => {
                    stream_term_sender.send(true).map_err(|_| Error::SendTerm)?;
                    break (reader.await, w);
                }
                _ = term_receiver.changed() => {
                    if !*term_receiver.borrow() {
                        continue;
                    }
                    stream_term_sender.send(true).map_err(|_| Error::SendTerm)?;
                    break (reader.await, writer.await);
                }
            }
        };
        let _ = connected_sender.send(false);

        if let Err(e) = &r {
            log::debug!("Failed to join broker client reader: {}", e);
        }

        if let Ok(Err(e)) = &r {
            log::debug!("Broker client reader failed: {}", e);
        }

        if let Err(e) = &w {
            log::debug!("Broker client writer failed: {}", e);
        }

        abortable_sleep!(term_receiver, std::time::Duration::from_secs(10));
    }
    Ok(())
}

async fn broker_reader<S, R, W, H>(
    mut read_stream: S,
    sender: mpsc::Sender<W>,
    handler: Arc<H>,
    mut term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    S: MsgReadStream<R>,
    H: Fn(R) -> std::result::Result<(), W>,
    R: 'static,
{
    while let Some(Ok(Some(msg))) =
        abortable!(term_receiver, read_stream.recv())
    {
        if let Err(msg) = handler(msg) {
            sender.send(msg).await.map_err(|_| Error::BrokerHandler)?;
        }
    }
    Ok(())
}

async fn broker_writer<S, W>(
    mut write_stream: S,
    broker_receiver: &mut mpsc::Receiver<W>,
    mut term_receiver: watch::Receiver<bool>,
) -> Result<()>
where
    S: MsgWriteStream<W>,
    W: 'static,
{
    while let Some(Some(message)) =
        abortable!(term_receiver, broker_receiver.recv())
    {
        if let Err(e) = write_stream.send(message).await {
            log::warn!("failed to send message to broker: {}", e);
            let _ = write_stream.shutdown().await;
            return Err(Error::BrokerHandler);
        }
    }
    write_stream.shutdown().await
}
