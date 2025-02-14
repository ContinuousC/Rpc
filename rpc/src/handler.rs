/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{future::Future, marker::PhantomData, sync::Arc};

use crate::{GenericValue, Protocol};

pub trait RequestHandler<P, V>: Send + Sync
where
    P: Protocol,
    V: GenericValue,
{
    type ExtraArgs: Send + Sync;
    type Error: std::error::Error + Send + Sync; // = V::Error;
    fn handle(
        &self,
        request: V,
        extra_args: Self::ExtraArgs,
    ) -> impl Future<Output = Result<V, Self::Error>> + Send;
}

pub trait ServerRequestHandler<P, V>: Send + Sync
where
    P: Protocol,
    V: GenericValue,
{
    type Session: Send + Sync;
    type ExtraArgs: Send + Sync;
    type Error: std::error::Error + Send + Sync; // = V::Error;

    fn handle_with_session(
        &self,
        session: &Self::Session,
        request: V,
        extra_args: Self::ExtraArgs,
    ) -> impl Future<Output = Result<V, Self::Error>> + Send;
}

pub trait SessionHandler<P, V, S>: Send + Sync
where
    P: Protocol,
    V: GenericValue,
{
    type Session: Send + Sync + 'static;
    type Error: std::error::Error + Send + Sync; // = V::Error;
    fn session(
        &self,
        info: &S,
    ) -> impl Future<Output = Result<Self::Session, Self::Error>> + Send;
    // async fn close_session(
    //     &self,
    //     _session: Self::Session,
    // ) -> Result<(), Self::Error> {
    //     Ok(())
    // }
}

pub trait Shutdown: Send + Sync + Sized {
    type Error: std::error::Error + Send + Sync;
    fn shutdown(self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

pub trait IntoSessionHandler<P, V, S>
where
    P: Protocol,
    V: GenericValue,
    S: Send + Sync,
{
    type Handler: SessionHandler<P, V, S>;
    // type ExtraArgs: Send + Sync;
    // where
    //     Self::Handler::ExtraArgs = Self::ExtraArgs;
    fn into_session_handler(self) -> Self::Handler;
}

// This conflicts with the one below.
// impl<F, R, W> AsyncRequestHandler<R, W> for F
// where
//     F: Fn(R) -> W,
// {
//     type Output = Ready<W>;
//     fn handle(&self, request: R) -> Self::Output {
//         ready(self(request))
//     }
// }

// impl<F, Fut, P> RequestHandler for F
// where
//     P: Protocol,
//     F: Fn(P::Req) -> Fut + Send + Sync + 'static,
//     Fut: Future<Output = P::Res> + Send + 'static,
// {
//     async fn handle(&self, request: P::Req) -> P::Res {
//         self(request).await
//     }
// }

impl<T, P, V> RequestHandler<P, V> for Arc<T>
where
    P: Protocol,
    V: GenericValue + 'static,
    T: RequestHandler<P, V>,
{
    type ExtraArgs = T::ExtraArgs;
    type Error = T::Error;
    async fn handle(
        &self,
        request: V,
        extra_args: Self::ExtraArgs,
    ) -> Result<V, Self::Error> {
        T::handle(self, request, extra_args).await
    }
    // async fn shutdown(self) -> Result<(), Self::Error> {
    //     match Arc::try_unwrap(self) {
    //         Ok(last) => last.shutdown().await,
    //         Err(_) => Ok(()),
    //     }
    // }
}

impl<T, P, V> ServerRequestHandler<P, V> for Arc<T>
where
    P: Protocol,
    V: GenericValue + 'static,
    T: ServerRequestHandler<P, V>,
{
    type Session = T::Session;
    type ExtraArgs = T::ExtraArgs;
    type Error = T::Error;
    async fn handle_with_session(
        &self,
        session: &Self::Session,
        request: V,
        extra_args: Self::ExtraArgs,
    ) -> Result<V, Self::Error> {
        T::handle_with_session(self, session, request, extra_args).await
    }
}

#[derive(Clone)]
pub struct SessionLess<H, P, V>(H, PhantomData<(P, V)>);

impl<H, P, V> SessionLess<H, P, V> {
    pub fn new(handler: H) -> Self {
        Self(handler, PhantomData)
    }
}

impl<H, P, V> ServerRequestHandler<P, V> for SessionLess<H, P, V>
where
    P: Protocol,
    V: GenericValue,
    H: RequestHandler<P, V>,
{
    type Session = ();
    type ExtraArgs = H::ExtraArgs;
    type Error = H::Error;

    async fn handle_with_session(
        &self,
        _session: &Self::Session,
        request: V,
        extra_args: Self::ExtraArgs,
    ) -> Result<V, Self::Error> {
        self.0.handle(request, extra_args).await
    }
}

impl<H, P, V, S> SessionHandler<P, V, S> for SessionLess<H, P, V>
where
    P: Protocol,
    V: GenericValue,
    S: Send + Sync,
    H: RequestHandler<P, V>,
{
    type Session = ();
    type Error = H::Error;
    async fn session(&self, _info: &S) -> Result<Self::Session, Self::Error> {
        Ok(())
    }
    // async fn close_session(
    //     &self,
    //     _session: Self::Session,
    // ) -> Result<(), Self::Error> {
    //     Ok(())
    // }
}

impl<H, P, V> Shutdown for SessionLess<H, P, V>
where
    P: Protocol,
    V: GenericValue,
    H: Shutdown,
{
    type Error = H::Error;
    async fn shutdown(self) -> Result<(), Self::Error> {
        H::shutdown(self.0).await
    }
}

// impl<H, P, V> IntoServerRequestHandler<P, V> for H
// where
//     P: Protocol,
//     V: GenericValue,
//     H: RequestHandler<P, V>,
// {
//     type Handler = SessionLess<H, P, V>;
//     fn into_server_request_handler(self) -> Self::Handler {
//         SessionLess::new(self)
//     }
// }

impl<H, P, V, S> IntoSessionHandler<P, V, S> for H
where
    P: Protocol,
    V: GenericValue,
    S: Send + Sync,
    H: SessionHandler<P, V, S>,
{
    type Handler = Self;
    // type ExtraArgs = Self::ExtraArgs;
    fn into_session_handler(self) -> Self::Handler {
        self
    }
}
