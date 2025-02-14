/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{future::Future, sync::Arc};

use futures::future::{ready, Ready};
use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    AsyncRequest, AsyncResponse, CborStream, JsonStream, MsgStream, Protocol,
};

pub trait AsyncStreamWrapper<S, P: Protocol>: Send + Sync + 'static {
    type ServerStream: MsgStream<AsyncRequest<P::Req>, AsyncResponse<P::Res>>;
    type ClientStream: MsgStream<AsyncResponse<P::Res>, AsyncRequest<P::Req>>;

    type ServerFut: Future<Output = Self::ServerStream> + Send;
    type ClientFut: Future<Output = Self::ClientStream> + Send;

    fn wrap_server(&self, stream: S) -> Self::ServerFut;
    fn wrap_client(&self, stream: S) -> Self::ClientFut;
}

impl<S, P, T> AsyncStreamWrapper<S, P> for Arc<T>
where
    P: Protocol,
    T: AsyncStreamWrapper<S, P>,
{
    type ServerStream = T::ServerStream;
    type ClientStream = T::ClientStream;

    type ServerFut = T::ServerFut;
    type ClientFut = T::ClientFut;

    fn wrap_server(&self, stream: S) -> Self::ServerFut {
        T::wrap_server(self, stream)
    }

    fn wrap_client(&self, stream: S) -> Self::ClientFut {
        T::wrap_client(self, stream)
    }
}

pub struct AsyncJsonStreamWrapper;
pub struct AsyncCborStreamWrapper;

impl<S, P> AsyncStreamWrapper<S, P> for AsyncJsonStreamWrapper
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    P: Protocol,
{
    type ServerStream =
        JsonStream<S, AsyncRequest<P::Req>, AsyncResponse<P::Res>>;
    type ClientStream =
        JsonStream<S, AsyncResponse<P::Res>, AsyncRequest<P::Req>>;

    type ServerFut = Ready<Self::ServerStream>;
    type ClientFut = Ready<Self::ClientStream>;

    fn wrap_server(&self, stream: S) -> Self::ServerFut {
        ready(JsonStream::new(stream))
    }

    fn wrap_client(&self, stream: S) -> Self::ClientFut {
        ready(JsonStream::new(stream))
    }
}

impl<S, P> AsyncStreamWrapper<S, P> for AsyncCborStreamWrapper
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    P: Protocol,
    P::Req: Serialize + DeserializeOwned,
    P::Res: Serialize + DeserializeOwned,
{
    type ServerStream =
        CborStream<S, AsyncRequest<P::Req>, AsyncResponse<P::Res>>;
    type ClientStream =
        CborStream<S, AsyncResponse<P::Res>, AsyncRequest<P::Req>>;

    type ServerFut = Ready<Self::ServerStream>;
    type ClientFut = Ready<Self::ClientStream>;

    fn wrap_server(&self, stream: S) -> Self::ServerFut {
        ready(CborStream::new(stream))
    }

    fn wrap_client(&self, stream: S) -> Self::ClientFut {
        ready(CborStream::new(stream))
    }
}
