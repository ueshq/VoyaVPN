use std::{sync::Arc, time::Duration};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
    ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use thiserror::Error;
use tokio::{net::TcpStream, time::timeout};
use tokio_rustls::TlsConnector;

const CERT_FETCH_TIMEOUT: Duration = Duration::from_secs(8);
const BEGIN_CERT: &str = "-----BEGIN CERTIFICATE-----";
const END_CERT: &str = "-----END CERTIFICATE-----";

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CertificateFetchRequest {
    pub address: String,
    pub port: u16,
    pub server_name: Option<String>,
    pub allow_insecure: bool,
    pub include_chain: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CertificateFetchResult {
    pub pem: String,
    pub sha256: Vec<String>,
    pub chain_count: u32,
    pub warning: Option<String>,
}

#[derive(Debug, Error)]
pub enum CertificateError {
    #[error("certificate address is empty")]
    EmptyAddress,
    #[error("certificate server name is invalid")]
    InvalidServerName,
    #[error("TLS connection timed out")]
    Timeout,
    #[error("TCP connection failed: {0}")]
    Tcp(#[from] std::io::Error),
    #[error("TLS handshake failed: {0}")]
    Tls(#[from] rustls::Error),
    #[error("server did not return a certificate")]
    MissingPeerCertificate,
    #[error("certificate PEM is invalid")]
    InvalidPem,
}

pub type Result<T> = std::result::Result<T, CertificateError>;

pub async fn fetch_certificate(request: CertificateFetchRequest) -> Result<CertificateFetchResult> {
    let address = request.address.trim();
    if address.is_empty() {
        return Err(CertificateError::EmptyAddress);
    }

    let server_name = request
        .server_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(address)
        .to_string();
    let server_name =
        ServerName::try_from(server_name).map_err(|_| CertificateError::InvalidServerName)?;
    let stream = timeout(
        CERT_FETCH_TIMEOUT,
        TcpStream::connect((address, request.port)),
    )
    .await
    .map_err(|_| CertificateError::Timeout)??;

    let connector = TlsConnector::from(Arc::new(client_config(request.allow_insecure)));
    let tls = timeout(CERT_FETCH_TIMEOUT, connector.connect(server_name, stream))
        .await
        .map_err(|_| CertificateError::Timeout)??;
    let (_, session) = tls.get_ref();
    let peer = session
        .peer_certificates()
        .ok_or(CertificateError::MissingPeerCertificate)?;
    if peer.is_empty() {
        return Err(CertificateError::MissingPeerCertificate);
    }

    let selected = if request.include_chain {
        peer.to_vec()
    } else {
        vec![peer[0].clone()]
    };

    let pem = concatenate_pem(&selected);
    let sha256 = selected
        .iter()
        .map(|cert| certificate_sha256(cert.as_ref()))
        .collect();

    Ok(CertificateFetchResult {
        pem,
        sha256,
        chain_count: u32::try_from(peer.len()).unwrap_or(u32::MAX),
        warning: request
            .allow_insecure
            .then(|| "Certificate validation was bypassed for this fetch only.".to_string()),
    })
}

pub fn calculate_certificate_sha256(pem: &str) -> Result<Vec<String>> {
    let certificates = parse_pem_chain(pem)?;
    if certificates.is_empty() {
        return Err(CertificateError::InvalidPem);
    }

    Ok(certificates
        .iter()
        .map(|cert| certificate_sha256(cert))
        .collect())
}

fn client_config(allow_insecure: bool) -> ClientConfig {
    let mut root_store = RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for cert in native.certs {
        let _ = root_store.add(cert);
    }
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let mut config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    if allow_insecure {
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(NoCertificateVerification));
    }

    config
}

fn concatenate_pem(certs: &[CertificateDer<'_>]) -> String {
    certs
        .iter()
        .map(|cert| cert_to_pem(cert.as_ref()))
        .collect::<Vec<_>>()
        .join("")
}

fn cert_to_pem(der: &[u8]) -> String {
    let encoded = BASE64_STANDARD.encode(der);
    let mut pem = String::new();
    pem.push_str(BEGIN_CERT);
    pem.push('\n');
    for chunk in encoded.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).expect("base64 is valid UTF-8"));
        pem.push('\n');
    }
    pem.push_str(END_CERT);
    pem.push('\n');
    pem
}

fn parse_pem_chain(pem: &str) -> Result<Vec<Vec<u8>>> {
    let mut certs = Vec::new();
    let mut rest = pem;

    while let Some(begin) = rest.find(BEGIN_CERT) {
        let after_begin = &rest[begin + BEGIN_CERT.len()..];
        let Some(end) = after_begin.find(END_CERT) else {
            return Err(CertificateError::InvalidPem);
        };
        let body = after_begin[..end]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        let der = BASE64_STANDARD
            .decode(body)
            .map_err(|_| CertificateError::InvalidPem)?;
        certs.push(der);
        rest = &after_begin[end + END_CERT.len()..];
    }

    Ok(certs)
}

fn certificate_sha256(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    digest.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[derive(Debug)]
struct NoCertificateVerification;

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----\nAQIDBAU=\n-----END CERTIFICATE-----\n";

    #[test]
    fn certificate_sha256_parses_pem_chain() {
        let hashes = calculate_certificate_sha256(TEST_CERT).expect("valid test PEM");

        assert_eq!(
            hashes,
            vec!["74F81FE167D99B4CB41D6D0CCDA82278CAEE9F3E2F25D5E5A3936FF3DCEC60D0"]
        );
    }

    #[test]
    fn certificate_sha256_rejects_missing_pem() {
        assert!(matches!(
            calculate_certificate_sha256("not pem"),
            Err(CertificateError::InvalidPem)
        ));
    }
}
