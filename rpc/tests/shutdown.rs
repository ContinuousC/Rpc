/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{
    convert::TryFrom,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use rustls::pki_types::ServerName;
use thiserror::Error;
use tokio::test;

use rpc::{
    rpc, tls_client_config, tls_server_config, GenericValue, SessionHandler,
    Shutdown,
};

#[rpc(service(session, shutdown), stub)]
trait TestService {
    async fn test_method(&self, arg1: String) -> String;
}

struct TestImpl(Arc<AtomicBool>);
struct TestSession;

impl TestService for TestImpl {
    type Error = Error;
    type Session = TestSession;
    async fn test_method(
        &self,
        _sess: &TestSession,
        arg1: String,
    ) -> Result<String, Error> {
        Ok(arg1)
    }
}

impl<V, S> SessionHandler<TestProto, V, S> for TestImpl
where
    V: GenericValue,
    S: Sync,
{
    type Session = TestSession;
    type Error = Error;
    async fn session(&self, _info: &S) -> Result<TestSession, Error> {
        Ok(TestSession)
    }
}

impl Shutdown for TestImpl {
    type Error = Error;
    async fn shutdown(self) -> Result<(), Error> {
        eprintln!("Running shutdown!");
        self.0.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Error, Debug)]
enum Error {}

#[test]
async fn service_shutdown() {
    let sock_path = Path::new("test_service.sock");

    let _ = tokio::fs::remove_file(sock_path).await;

    let stopped = Arc::new(AtomicBool::new(false));

    let server = rpc::AsyncServer::<TestProto>::builder()
        .unix(sock_path)
        .expect("failied to initialize listener")
        .tls(
            tls_server_config(
                Path::new("tests/certs/ca.crt"),
                Path::new("tests/certs/server.crt"),
                Path::new("tests/certs/server.key"),
            )
            .await
            .expect("failed to create tls server config"),
        )
        .json()
        .handler_session(TestHandler::new(TestImpl(stopped.clone())));

    let client =
        rpc::AsyncClient::<TestProto, serde_json::Value, ()>::builder()
            .unix(sock_path)
            .tls(
                tls_client_config(
                    Path::new("tests/certs/ca.crt"),
                    Path::new("tests/certs/client.crt"),
                    Path::new("tests/certs/client.key"),
                )
                .await
                .expect("failed to create tls client config"),
                ServerName::try_from("mndev02").unwrap(),
            )
            .json();

    client
        .connected(std::time::Duration::from_secs(1))
        .await
        .unwrap();

    let stub = TestServiceStub::<_, serde_json::Value>::new(client);
    std::mem::drop(stub);

    assert!(server.shutdown().await.is_ok());
    assert!(stopped.load(Ordering::SeqCst));

    //stub.0.shutdown().await.expect("client shutdown failed");
    //server.shutdown().await.expect("server shutdown failed");
}
