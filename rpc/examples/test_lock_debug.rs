/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{convert::TryFrom, path::Path};

use rustls::pki_types::ServerName;
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};
use thiserror::Error;

use lock_debug::{location, tokio::debug::RwLock};
use rpc::{rpc, tls_client_config, tls_server_config};
use tokio::join;

#[rpc(service(lock_debug_task_names), stub, log_errors)]
trait TestService {
    async fn test_method(&self, arg: bool);
}

struct TestImpl {
    a: RwLock<&'static str>,
    b: RwLock<&'static str>,
}

impl TestService for TestImpl {
    type Error = Error;
    async fn test_method(&self, arg: bool) -> Result<(), Error> {
        if arg {
            let a = self.a.write(location!()).await;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let b = self.b.write(location!()).await;
            eprintln!("a = {}, b = {}", *a, *b);
        } else {
            let b = self.b.write(location!()).await;
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let a = self.a.write(location!()).await;
            eprintln!("a = {}, b = {}", *a, *b);
        }
        Ok(())
    }
}

#[derive(Error, Debug)]
enum Error {}

#[tokio::main]
async fn main() {
    TermLogger::init(
        LevelFilter::Warn,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )
    .unwrap();

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
        .handler(TestHandler::new(TestImpl {
            a: RwLock::new("lock a", "value a"),
            b: RwLock::new("lock b", "value b"),
        }));

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

    let (a, b) = join!(stub.test_method(true), stub.test_method(false));
    a.unwrap();
    b.unwrap();

    //stub.0.shutdown().await.expect("client shutdown failed");
    //server.shutdown().await.expect("server shutdown failed");
}
