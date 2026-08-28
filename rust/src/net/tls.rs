use std::sync::{Arc, Once};
use std::time::Duration;

use reqwest::tls::TlsInfo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::aws_lc_rs;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as TlsError, NamedGroup, RootCertStore,
    SignatureScheme,
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

pub fn ensure_default_crypto_provider() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let provider = aws_lc_rs::default_provider();
        let provider_summary = crypto_provider_summary(&provider);
        if let Err(existing) = provider.install_default() {
            if !crypto_provider_supports_post_quantum(&existing) {
                log::error!(
                    "[tls] rustls default CryptoProvider was already installed without X25519MLKEM768; \
                     Tinfoil attestation may fail against post-quantum-only endpoints"
                );
            }
            log::info!(
                "[tls] rustls default CryptoProvider already installed; aws-lc candidate: {provider_summary}; existing: {}",
                crypto_provider_summary(&existing)
            );
        } else {
            log::info!("[tls] installed rustls aws-lc CryptoProvider: {provider_summary}");
        }
    });
}

fn crypto_provider_supports_post_quantum(provider: &rustls::crypto::CryptoProvider) -> bool {
    provider
        .kx_groups
        .iter()
        .any(|group| group.name() == NamedGroup::X25519MLKEM768)
}

fn crypto_provider_summary(provider: &rustls::crypto::CryptoProvider) -> String {
    let groups = provider
        .kx_groups
        .iter()
        .map(|group| format!("{:?}", group.name()))
        .collect::<Vec<_>>()
        .join(",");
    let schemes = provider
        .signature_verification_algorithms
        .supported_schemes()
        .iter()
        .map(|scheme| format!("{scheme:?}"))
        .collect::<Vec<_>>()
        .join(",");
    let suites = provider
        .cipher_suites
        .iter()
        .map(|suite| format!("{:?}", suite.suite()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "target={}/{} groups=[{}] schemes=[{}] suites=[{}]",
        std::env::consts::OS,
        std::env::consts::ARCH,
        groups,
        schemes,
        suites
    )
}

#[cfg(test)]
pub(crate) fn default_crypto_provider_supports_post_quantum() -> bool {
    ensure_default_crypto_provider();
    rustls::crypto::CryptoProvider::get_default()
        .is_some_and(|provider| crypto_provider_supports_post_quantum(provider))
}

#[cfg(test)]
pub(crate) fn attested_transport_provider_supports_post_quantum() -> bool {
    crypto_provider_supports_post_quantum(&attested_transport_provider())
}

pub fn certificate_public_key_fp_from_der(der: &[u8]) -> Result<String, TlsPinError> {
    let cert = x509_cert::Certificate::from_der(der)
        .map_err(|e| TlsPinError::InvalidCertificate(e.to_string()))?;
    let spki = cert
        .tbs_certificate()
        .subject_public_key_info()
        .to_der()
        .map_err(|e: x509_cert::der::Error| TlsPinError::InvalidCertificate(e.to_string()))?;
    Ok(hex::encode(Sha256::digest(spki)))
}

pub fn live_tls_public_key_fp(response: &reqwest::Response) -> Result<String, TlsPinError> {
    response
        .extensions()
        .get::<TlsInfo>()
        .and_then(|info| info.peer_certificate())
        .ok_or_else(|| TlsPinError::InvalidCertificate("missing tls peer certificate".to_string()))
        .and_then(certificate_public_key_fp_from_der)
}

pub fn attested_reqwest_client(timeout: Duration) -> Result<reqwest::Client, TlsPinError> {
    let tls = attested_rustls_client_config()?;
    reqwest::Client::builder()
        .no_hickory_dns()
        .no_proxy()
        .timeout(timeout)
        .use_preconfigured_tls(tls)
        .build()
        .map_err(|e| TlsPinError::Network(e.to_string()))
}

pub fn pinned_reqwest_client(
    expected_public_key_fp: &str,
    timeout: Duration,
) -> Result<reqwest::Client, TlsPinError> {
    let tls = pinned_rustls_client_config(expected_public_key_fp)?;
    reqwest::Client::builder()
        .no_hickory_dns()
        .no_proxy()
        .timeout(timeout)
        .tls_info(true)
        .use_preconfigured_tls(tls)
        .build()
        .map_err(|e| TlsPinError::Network(e.to_string()))
}

fn root_cert_store() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    roots
}

fn attested_rustls_client_config() -> Result<ClientConfig, TlsPinError> {
    ensure_default_crypto_provider();
    let provider = attested_transport_provider();
    log::info!(
        "[tls] building attested reqwest client with {}",
        crypto_provider_summary(&provider)
    );

    ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsPinError::InvalidConfig(e.to_string()))
        .map(|builder| {
            builder
                .with_root_certificates(root_cert_store())
                .with_no_client_auth()
        })
}

fn attested_transport_provider() -> rustls::crypto::CryptoProvider {
    // Android aws-lc currently advertises X25519MLKEM768 but receives a
    // HandshakeFailure from Tinfoil endpoints when that hybrid key share is
    // offered first. Tinfoil accepts X25519, so keep Android remote routing
    // functional while retaining aws-lc and rustls everywhere.
    #[cfg(target_os = "android")]
    {
        let mut provider = aws_lc_rs::default_provider();
        provider
            .kx_groups
            .retain(|group| group.name() != NamedGroup::X25519MLKEM768);
        provider
    }
    #[cfg(not(target_os = "android"))]
    {
        aws_lc_rs::default_provider()
    }
}

fn pinned_rustls_client_config(expected_public_key_fp: &str) -> Result<ClientConfig, TlsPinError> {
    ensure_default_crypto_provider();

    let provider: Arc<rustls::crypto::CryptoProvider> = aws_lc_rs::default_provider().into();

    let webpki =
        WebPkiServerVerifier::builder_with_provider(Arc::new(root_cert_store()), provider.clone())
            .build()
            .map_err(|e| TlsPinError::InvalidConfig(e.to_string()))?;

    let verifier = PinnedServerCertVerifier {
        expected_public_key_fp: expected_public_key_fp.to_string(),
        inner: webpki,
    };

    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsPinError::InvalidConfig(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();

    Ok(config)
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
