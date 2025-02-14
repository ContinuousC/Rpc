/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{convert::TryFrom, path::Path};

use rustls::pki_types::ServerName;
use thiserror::Error;
use tokio::test;

use rpc::{rpc, tls_client_config, tls_server_config};

#[rpc(service, stub)]
trait TestService {
    async fn test_method(&self, arg1: String) -> String;
}

struct TestImpl;

impl TestService for TestImpl {
    type Error = Error;
    async fn test_method(&self, arg1: String) -> Result<String, Error> {
        Ok(arg1)
    }
}

#[derive(Error, Debug)]
enum Error {}

#[test]
async fn service_client_server() {
    let sock_path = Path::new("test_service.sock");

    let _ = tokio::fs::remove_file(sock_path).await;

    let _server = rpc::AsyncServer::<TestProto>::builder()
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
        .handler(TestHandler::new(TestImpl));

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

    assert_eq!(
        stub.test_method(String::from("12345")).await.unwrap(),
        "12345"
    );

    //stub.0.shutdown().await.expect("client shutdown failed");
    //server.shutdown().await.expect("server shutdown failed");
}
