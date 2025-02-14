/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use serde::{Deserialize, Serialize};

// use crate::Protocol;

// use super::wrapper::StreamWrapper;

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Request {
    Ping(u64),
}

#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Response {
    Pong(u64),
}

// #[derive(Clone, Copy)]
// struct PingPongProto;

// impl Protocol for PingPongProto {
//     // type Req = Request;
//     // type Res = Response;
// }

// #[test]
// async fn json_stream() {
//     synchronous_communication(
//         JsonStreamWrapper::new(PlainStreamWrapper),
//         JsonStreamWrapper::new(PlainStreamWrapper),
//     )
//     .await;
// }

// #[test]
// async fn cbor_stream() {
//     synchronous_communication(
//         CborStreamWrapper::new(PlainStreamWrapper),
//         CborStreamWrapper::new(PlainStreamWrapper),
//     )
//     .await;
// }

// async fn synchronous_communication<S, C>(server_wrapper: S, client_wrapper: C)
// where
//     S: StreamWrapper<UnixStream>,
//     C: StreamWrapper<UnixStream>,
//     S::Wrapped: MsgStream<Request, Response>,
//     C::Wrapped: MsgStream<Response, Request>,
// {
//     let (server_stream, client_stream) =
//         UnixStream::pair().expect("failed to create socket pair");

//     let mut server_stream = server_wrapper.wrap(server_stream).await;
//     let mut client_stream = client_wrapper.wrap(client_stream).await;

//     let server = tokio::spawn(async move {
//         while let Some(Request::Ping(i)) =
//             server_stream.recv().await.expect("server recv failed")
//         {
//             server_stream
//                 .send(Response::Pong(i))
//                 .await
//                 .expect("server send failed");
//         }
//     });

//     let client = tokio::spawn(async move {
//         for i in 0..10000 {
//             client_stream
//                 .send(Request::Ping(i))
//                 .await
//                 .expect("client send failed");
//             assert!(
//                 client_stream
//                     .recv()
//                     .await
//                     .expect("client recv failed")
//                     .expect("unexpected EOF")
//                     == Response::Pong(i)
//             );
//         }
//     });

//     let (s, c) = tokio::join!(server, client);
//     s.expect("server failed");
//     c.expect("client failed");
// }
