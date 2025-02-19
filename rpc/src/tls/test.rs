/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::convert::TryFrom;
use std::path::Path;

use rustls::pki_types::ServerName;
use tokio::test;

use crate::{self as rpc};
use crate::{AsyncClient, AsyncServer};

#[rpc::rpc(service, stub)]
trait PingPongService {
    async fn ping(&self, i: u64) -> u64;
}

struct PingPongImpl;

impl PingPongService for PingPongImpl {
    type Error = Error;
    async fn ping(&self, i: u64) -> Result<u64, Self::Error> {
        Ok(i)
    }
}

#[derive(thiserror::Error, Debug)]
enum Error {}

#[test]
async fn tls_client_server() {
    let r = tls_client_server_test(
        Path::new("tests/tls_client_server.sock"),
        Path::new("tests/certs/ca.crt"),
        Path::new("tests/certs/server.crt"),
        Path::new("tests/certs/server.key"),
        Path::new("tests/certs/client.crt"),
        Path::new("tests/certs/client.key"),
    )
    .await
    .expect("failed to run client/server test with correct certs");
    assert_eq!(r, 123);
}

#[test]
async fn tls_evil_client() {
    tls_client_server_test(
        Path::new("tests/tls_evil_client.sock"),
        Path::new("tests/certs/ca.crt"),
        Path::new("tests/certs/server.crt"),
        Path::new("tests/certs/server.key"),
        Path::new("tests/certs/evil-client.crt"),
        Path::new("tests/certs/evil-client.key"),
    )
    .await
    .expect_err("succeeded to run client/server test with incorrect certs");
}

#[test]
async fn tls_evil_server() {
    tls_client_server_test(
        Path::new("tests/tls_evil_server.sock"),
        Path::new("tests/certs/ca.crt"),
        Path::new("tests/certs/evil-server.crt"),
        Path::new("tests/certs/evil-server.key"),
        Path::new("tests/certs/client.crt"),
        Path::new("tests/certs/client.key"),
    )
    .await
    .expect_err("succeeded to run client/server test with incorrect certs");
}

async fn tls_client_server_test(
    sock_path: &Path,
    ca_crt: &Path,
    server_crt: &Path,
    server_key: &Path,
    client_crt: &Path,
    client_key: &Path,
) -> Result<u64, String> {
    let _ = env_logger::builder().is_test(true).try_init();

    let cert_paths = [ca_crt, server_crt, server_key, client_crt, client_key];
    let generate_certs = async {
        for path in &cert_paths {
            if tokio::fs::metadata(path).await.is_err() {
                return true;
            }
        }
        false
    }
    .await;

    if generate_certs {
        assert!(
            tokio::process::Command::new("/bin/bash")
                .current_dir("tests/certs/")
                .arg("create-test-certificates.sh")
                .status()
                .await
                .is_ok_and(|s| s.success()),
            "failed to generate test certificates"
        )
    }

    if tokio::fs::metadata(sock_path).await.is_ok() {
        tokio::fs::remove_file(sock_path).await.unwrap_or_else(|e| {
            panic!("failed to remove existing '{}': {}", sock_path.display(), e)
        });
    }

    let server_config = crate::tls_server_config(
        Path::new(ca_crt),
        Path::new(server_crt),
        Path::new(server_key),
    )
    .await
    .expect("failed to create server tls config");

    let client_config = crate::tls_client_config(
        Path::new(ca_crt),
        Path::new(client_crt),
        Path::new(client_key),
    )
    .await
    .expect("failed to create client tls config");

    let server = AsyncServer::<PingPongProto>::builder()
        .unix(sock_path.to_path_buf())
        .expect("failed to initialize listener")
        .tls(server_config)
        .json()
        .handler(PingPongHandler::new(PingPongImpl));

    let client = AsyncClient::<PingPongProto, serde_json::Value, ()>::builder()
        .unix(sock_path.to_path_buf())
        .tls(client_config, ServerName::try_from("localhost").unwrap())
        .json();

    client
        .connected(std::time::Duration::from_secs(1))
        .await
        .map_err(|e| format!("client connection failed: {e}"))?;

    let timeout = tokio::time::sleep(std::time::Duration::from_secs(1));

    let res: Result<u64, String> = tokio::select! {
        res = client
            .request((),serde_json::to_value(PingPongRequest::Ping { i:123 }).unwrap())
            => res
                .map_err(|e| format!("request failed: {e}"))
                .and_then(|r| serde_json::from_value::<Result<u64,String>>(r).unwrap()),
        _ = timeout => panic!("timeout")
    };
    //assert_eq!(res, Ok(123));

    client.shutdown().await.expect("client shutdown failed");
    server.shutdown().await.expect("server shutdown failed");

    tokio::fs::remove_file(sock_path).await.unwrap_or_else(|e| {
        panic!("failed to remove '{}': {}", sock_path.display(), e)
    });

    res
}
