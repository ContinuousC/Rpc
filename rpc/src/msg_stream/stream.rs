/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{future::Future, marker::PhantomData, pin::Pin};

use crate::{
    error::{Error, Result},
    TraceCtx,
};
use tokio::sync::mpsc;

pub trait MsgStream<R: 'static, W: 'static>:
    MsgReadStream<R> + MsgWriteStream<W>
{
    type Stream;
    type ReadStream: MsgReadStream<R>;
    type WriteStream: MsgWriteStream<W>;

    fn split(self) -> (Self::ReadStream, Self::WriteStream);
    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self;
}

pub trait MsgReadStream<T: 'static>: Send + Sync + Sized + 'static {
    fn recv(&mut self)
        -> impl Future<Output = Result<Option<T>>> + Send + Sync;

    fn with_context_extractor<F>(self, _: F) -> impl MsgReadStream<T>
    where
        F: for<'a> Fn(&'a T) -> Option<TraceCtx> + Send + Sync + 'static,
    {
        self
    }

    fn map<F>(self, f: F) -> MapReadStream<Self, T, F>
    where
        Self: Sized,
    {
        MapReadStream(self, f, PhantomData)
    }

    fn boxed(self) -> Box<dyn BoxMsgReadStream<T>>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

pub trait BoxMsgReadStream<T>: Send + Sync + 'static {
    fn boxed_recv<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<T>>> + Send + Sync + 'a>>;
}

impl<T: 'static, S: MsgReadStream<T>> BoxMsgReadStream<T> for S {
    fn boxed_recv<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<T>>> + Send + Sync + 'a>>
    {
        Box::pin(MsgReadStream::recv(self))
    }
}

pub trait MsgWriteStream<T: 'static>: Send + Sync + 'static {
    fn send(
        &mut self,
        msg: T,
    ) -> impl Future<Output = Result<()>> + Send + Sync;
    fn shutdown(&mut self) -> impl Future<Output = Result<()>> + Send + Sync;

    fn map<F>(self, f: F) -> MapWriteStream<Self, T, F>
    where
        Self: Sized,
    {
        MapWriteStream(self, f, PhantomData)
    }

    fn boxed(self) -> Box<dyn BoxMsgWriteStream<T>>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

pub trait BoxMsgWriteStream<T>: Send + Sync + 'static {
    fn boxed_send<'a>(
        &'a mut self,
        msg: T,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + Sync + 'a>>;
    fn boxed_shutdown<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + Sync + 'a>>;
}

impl<T: 'static, S: MsgWriteStream<T>> BoxMsgWriteStream<T> for S {
    fn boxed_send<'a>(
        &'a mut self,
        msg: T,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + Sync + 'a>> {
        Box::pin(MsgWriteStream::send(self, msg))
    }

    fn boxed_shutdown<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + Sync + 'a>> {
        Box::pin(MsgWriteStream::shutdown(self))
    }
}

pub struct MapReadStream<T, U, F>(T, F, PhantomData<U>);

impl<T, U, V, F> MsgReadStream<V> for MapReadStream<T, U, F>
where
    T: MsgReadStream<U>,
    U: Send + Sync + 'static,
    F: FnMut(U) -> V + Send + Sync + 'static,
    V: 'static,
{
    async fn recv(&mut self) -> Result<Option<V>> {
        match self.0.recv().await {
            Ok(Some(v)) => Ok(Some(self.1(v))),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

pub struct MapWriteStream<T, U, F>(T, F, PhantomData<U>);

impl<T, U, V, F> MsgWriteStream<V> for MapWriteStream<T, U, F>
where
    T: MsgWriteStream<U>,
    V: Send + Sync + 'static,
    U: Send + Sync + 'static,
    F: FnMut(V) -> U + Send + Sync + 'static,
{
    async fn send(&mut self, msg: V) -> Result<()> {
        self.0.send(self.1(msg)).await
    }

    async fn shutdown(&mut self) -> Result<()> {
        self.0.shutdown().await
    }
}

impl<T: Send + Sync + 'static> MsgReadStream<T>
    for Box<dyn BoxMsgReadStream<T> + Send + Sync>
{
    async fn recv(&mut self) -> Result<Option<T>> {
        self.as_mut().boxed_recv().await
    }
}

impl<T: Send + Sync + 'static> MsgWriteStream<T>
    for Box<dyn BoxMsgWriteStream<T>>
{
    async fn send(&mut self, msg: T) -> Result<()> {
        self.as_mut().boxed_send(msg).await
    }
    async fn shutdown(&mut self) -> Result<()> {
        self.as_mut().boxed_shutdown().await
    }
}

impl<T: Send + 'static> MsgReadStream<T> for mpsc::Receiver<T> {
    async fn recv(&mut self) -> Result<Option<T>> {
        Ok(mpsc::Receiver::recv(self).await)
    }
}

impl<T: Send + Sync + 'static> MsgWriteStream<T> for mpsc::Sender<T> {
    async fn send(&mut self, msg: T) -> Result<()> {
        mpsc::Sender::send(self, msg)
            .await
            .map_err(|_| Error::SendMpsc)
    }
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
