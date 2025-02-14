/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::{path::Path, sync::Arc};

use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
    ClientConfig, RootCertStore, ServerConfig,
};
use tokio::fs;

use crate::error::{Error, Result};

pub async fn tls_server_config(
    ca_path: &Path,
    cert_path: &Path,
    key_path: &Path,
) -> Result<Arc<ServerConfig>> {
    let ca = read_cert(ca_path).await?;
    let key = read_key(key_path).await?;
    let certs = read_certs(cert_path).await?;

    let mut root_certs = RootCertStore::empty();
    root_certs.add(ca.clone()).map_err(Error::RootCertStore)?;

    let chain = certs.into_iter().chain([ca]).collect();
    let verifier = WebPkiClientVerifier::builder(Arc::new(root_certs))
        .build()
        .map_err(Error::VerifierBuild)?;

    Ok(Arc::new(
        ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(chain, key)
            .map_err(Error::ServerConfig)?,
    ))
}

pub async fn tls_client_config(
    ca_path: &Path,
    cert_path: &Path,
    key_path: &Path,
) -> Result<Arc<ClientConfig>> {
    let ca = read_cert(ca_path).await?;
    let key = read_key(key_path).await?;
    let certs = read_certs(cert_path).await?;

    let mut root_certs = RootCertStore::empty();
    root_certs.add(ca.clone()).map_err(Error::RootCertStore)?;

    let chain = certs.into_iter().chain([ca]).collect();
    //let verifier = AllowAnyAuthenticatedClient::new(root_certs);

    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_certs)
            .with_client_auth_cert(chain, key)
            .map_err(Error::ClientConfig)?,
    ))
}

// pub fn get_certificate_ids<S>(stream: &TlsStream<S>) -> Result<(String, String)>
// where
//     S: AsyncRead + AsyncWrite + Unpin,
// {
//     let peer_cert_data = stream
//         .get_ref()
//         .1
//         .get_peer_certificates()
//         .ok_or(api::Error::AuthenticationFailed)?
//         .into_iter()
//         .next()
//         .ok_or(api::Error::AuthenticationFailed)?;
//     let peer_cert = parse_x509_certificate(&peer_cert_data.0)
//         .map_err(|_| api::Error::AuthenticationFailed)?
//         .1;

//     let mut orgs = peer_cert.subject().iter_organization();
//     let org = orgs
//         .next()
//         .ok_or(api::Error::AuthenticationFailed)?
//         .attr_value
//         .as_str()
//         .map_err(|_| api::Error::AuthenticationFailed)?
//         .to_string();

//     let mut names = peer_cert.subject().iter_common_name();
//     let name = names
//         .next()
//         .ok_or(api::Error::AuthenticationFailed)?
//         .attr_value
//         .as_str()
//         .map_err(|_| api::Error::AuthenticationFailed)?
//         .to_string();

//     Ok((org, name))
// }

async fn read_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let data = fs::read(path)
        .await
        .map_err(|e| Error::ReadCert(path.to_path_buf(), e))?;
    rustls_pemfile::certs(&mut data.as_slice())
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::DecodeCert(path.to_path_buf(), e))
}

async fn read_cert(path: &Path) -> Result<CertificateDer<'static>> {
    read_certs(path)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| Error::NoCert(path.to_path_buf()))
}

async fn read_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let data = fs::read(path)
        .await
        .map_err(|e| Error::ReadKey(path.to_path_buf(), e))?;
    rustls_pemfile::private_key(&mut data.as_slice())
        .map_err(|e| Error::DecodeKey(path.to_path_buf(), e))?
        .ok_or_else(|| Error::NoKey(path.to_path_buf()))
}
