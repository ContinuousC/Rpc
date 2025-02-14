/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

// use std::time::Duration;

// use rand::Rng;

use crate::{self as rpc};

#[rpc::rpc(service, stub)]
trait PingPongService {
    async fn ping(&self, i: u64) -> u64;
}

//struct PingPongImpl;

// impl PingPongService for PingPongImpl {
//     type Error = Error;
//     async fn ping(&self, i: u64) -> Result<u64, Self::Error> {
//         let duration =
//             Duration::from_millis(rand::thread_rng().gen_range(0..1000));
//         tokio::time::sleep(duration).await;
//         Ok(i)
//     }
// }

// #[derive(thiserror::Error, Debug)]
// enum Error {}

// #[test]
// async fn json_stream_async() {
//     asynchronous_communication::<_, _, serde_json::Value>(
//         JsonStreamWrapper::new(PlainStreamWrapper),
//         JsonStreamWrapper::new(PlainStreamWrapper),
//     )
//     .await;
// }

// #[test]
// async fn cbor_stream_async() {
//     asynchronous_communication::<_, _, serde_cbor::Value>(
//         CborStreamWrapper::new(PlainStreamWrapper),
//         CborStreamWrapper::new(PlainStreamWrapper),
//     )
//     .await;
// }

// async fn asynchronous_communication<S, C, V>(
//     server_wrapper: S,
//     client_wrapper: C,
// ) where
//     V: GenericValue,
//     S: StreamWrapper<UnixStream>,
//     C: StreamWrapper<UnixStream>,
//     S::Wrapped: MsgStream<AsyncRequest<V>, AsyncResponse<V>>,
//     C::Wrapped: MsgStream<AsyncResponse<V>, AsyncRequest<V>>,
// {
//     let (server_stream, client_stream) =
//         UnixStream::pair().expect("failed to create socket pair");

//     let server = tokio::spawn(
//         AsyncServerConnection::<PingPongProto>::new(
//             server_wrapper.wrap(server_stream).await,
//             Arc::new(PingPongHandler::new(PingPongImpl)),
//             (),
//         )
//         .await_shutdown(),
//     );

//     let client = Arc::new(AsyncClientConnection::<PingPongProto, V, ()>::new(
//         client_wrapper.wrap(client_stream).await,
//     ));

//     let run = tokio::spawn(async move {
//         let responses = (0..10000)
//             .into_iter()
//             .map(|i| {
//                 let client = client.clone();
//                 async move {
//                     let res: Result<u64, String> = client
//                         .request(
//                             (),
//                             V::serialize_from(PingPongRequest::Ping { i })
//                                 .expect("request serialization failed"),
//                         )
//                         .await
//                         .expect("failed to recv response from channel")
//                         .deserialize_into()
//                         .expect("response deserialization failed");
//                     assert!(res == Ok(i));
//                     res
//                 }
//             })
//             .collect::<FuturesUnordered<_>>()
//             .collect::<Vec<_>>()
//             .await;
//         assert!(responses.len() == 10000);
//         Arc::try_unwrap(client)
//             .ok()
//             .expect("client still in use")
//             .shutdown()
//             .await
//             .expect("client shutdown failed");
//     });

//     let run = async { tokio::join!(server, run) };
//     let timeout = tokio::time::sleep(Duration::from_secs(5));

//     tokio::select! {
//         _ = timeout => {
//             panic!("timeout expired! Requests are probably \
//                     not being processed concurrently.");
//         }
//         (s,r) = run => {
//             s.expect("failed to join server").expect("server failed");
//             r.expect("failed to join test run");
//         }
//     }
// }
