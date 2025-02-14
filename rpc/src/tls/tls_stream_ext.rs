/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use rustls::pki_types::CertificateDer;
use tokio_rustls::{client, server, TlsStream};
use x509_parser::parse_x509_certificate;

pub trait TlsStreamExt {
    fn peer_certificate(&self) -> Option<&CertificateDer>;

    fn peer_common_name(&self) -> Option<String> {
        let cert = parse_x509_certificate(self.peer_certificate()?).ok()?.1;
        let cn = cert.subject().iter_common_name().next()?;
        Some(cn.attr_value().as_str().ok()?.to_string())
    }

    fn peer_organization(&self) -> Option<String> {
        let cert = parse_x509_certificate(self.peer_certificate()?).ok()?.1;
        let org = cert.subject().iter_organization().next()?;
        Some(org.attr_value().as_str().ok()?.to_string())
    }

    fn peer_org_and_cn(&self) -> Option<(String, String)> {
        let cert = parse_x509_certificate(self.peer_certificate()?).ok()?.1;
        let org = cert.subject().iter_organization().next()?;
        let cn = cert.subject().iter_common_name().next()?;
        Some((
            org.attr_value().as_str().ok()?.to_string(),
            cn.attr_value().as_str().ok()?.to_string(),
        ))
    }
}

impl<S> TlsStreamExt for TlsStream<S> {
    fn peer_certificate(&self) -> Option<&CertificateDer> {
        match self {
            tokio_rustls::TlsStream::Client(stream) => {
                stream.peer_certificate()
            }
            tokio_rustls::TlsStream::Server(stream) => {
                stream.peer_certificate()
            }
        }
    }
}

impl<S> TlsStreamExt for server::TlsStream<S> {
    fn peer_certificate(&self) -> Option<&CertificateDer> {
        self.get_ref().1.peer_certificates()?.iter().next()
    }
}

impl<S> TlsStreamExt for client::TlsStream<S> {
    fn peer_certificate(&self) -> Option<&CertificateDer> {
        self.get_ref().1.peer_certificates()?.iter().next()
    }
}
