use std::sync::Arc;
use std::time::Duration;

use reqwest::tls::TlsInfo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme,
};
use sha2::{Digest, Sha256};
use x509_cert::der::{Decode, Encode};

#[derive(Debug)]
pub enum TlsPinError {
    InvalidCertificate(String),
    InvalidConfig(String),
    Network(String),
}

impl std::fmt::Display for TlsPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCertificate(message) => write!(f, "invalid certificate: {message}"),
            Self::InvalidConfig(message) => write!(f, "invalid TLS config: {message}"),
            Self::Network(message) => write!(f, "TLS network error: {message}"),
        }
    }
}

impl std::error::Error for TlsPinError {}

pub fn certificate_public_key_fp_from_der(der: &[u8]) -> Result<String, TlsPinError> {
    let cert = x509_cert::Certificate::from_der(der)
        .map_err(|e| TlsPinError::InvalidCertificate(e.to_string()))?;
    let spki = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|e| TlsPinError::InvalidCertificate(e.to_string()))?;
    Ok(hex::encode(Sha256::digest(spki)))
}

pub fn live_tls_public_key_fp(response: &reqwest::Response) -> Result<String, TlsPinError> {
    response
        .extensions()
        .get::<TlsInfo>()
        .and_then(|info| info.peer_certificate())
        .ok_or_else(|| TlsPinError::InvalidCertificate("missing tls peer certificate".to_string()))
        .and_then(|der| certificate_public_key_fp_from_der(der.as_ref()))
}

pub fn pinned_reqwest_client(
    expected_public_key_fp: &str,
    timeout: Duration,
) -> Result<reqwest::Client, TlsPinError> {
    let tls = pinned_rustls_client_config(expected_public_key_fp)?;
    reqwest::Client::builder()
        .hickory_dns(false)
        .timeout(timeout)
        .tls_info(true)
        .use_preconfigured_tls(tls)
        .build()
        .map_err(|e| TlsPinError::Network(e.to_string()))
}

fn pinned_rustls_client_config(
    expected_public_key_fp: &str,
) -> Result<Arc<ClientConfig>, TlsPinError> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let webpki = WebPkiServerVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| TlsPinError::InvalidConfig(e.to_string()))?;

    let verifier = PinnedServerCertVerifier {
        expected_public_key_fp: expected_public_key_fp.to_string(),
        inner: webpki,
    };

    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();

    Ok(Arc::new(config))
}

#[derive(Debug)]
struct PinnedServerCertVerifier {
    expected_public_key_fp: String,
    inner: Arc<WebPkiServerVerifier>,
}

impl ServerCertVerifier for PinnedServerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        let actual = certificate_public_key_fp_from_der(end_entity.as_ref())
            .map_err(|e| TlsError::General(e.to_string()))?;
        if actual != self.expected_public_key_fp {
            return Err(TlsError::General(format!(
                "tls public key fingerprint mismatch: expected {}, got {}",
                self.expected_public_key_fp, actual
            )));
        }

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}
