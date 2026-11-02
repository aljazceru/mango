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
use x509_parser::prelude::{FromDer, X509Certificate};

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
    let (_, cert) = X509Certificate::from_der(der)
        .map_err(|e| TlsPinError::InvalidCertificate(e.to_string()))?;
    // `public_key().raw` is the borrowed DER of the SubjectPublicKeyInfo —
    // identical bytes to re-encoding it, without the extra allocation.
    Ok(hex::encode(Sha256::digest(cert.public_key().raw)))
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

/// Standard WebPKI-verified HTTPS client (no pinning, no attestation coupling).
/// Used by account-management transports (e.g. the PPQ account client) that talk
/// to plain public APIs over rustls with system/webpki roots.
pub fn plain_https_client(timeout: Duration) -> Result<reqwest::Client, TlsPinError> {
    ensure_default_crypto_provider();

    let provider: Arc<rustls::crypto::CryptoProvider> = aws_lc_rs::default_provider().into();

    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| TlsPinError::InvalidConfig(e.to_string()))?
        .with_root_certificates(Arc::new(root_cert_store()))
        .with_no_client_auth();

    reqwest::Client::builder()
        .no_hickory_dns()
        .no_proxy()
        // Account-management calls carry root credentials in custom headers;
        // a 307/308 to another origin would forward them (audit finding).
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .use_preconfigured_tls(config)
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
    log::debug!(
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

#[cfg(test)]
mod tests {
    use super::*;

    // Self-signed RSA fixture; the pinned fingerprint below is the SHA-256
    // of the DER-encoded SubjectPublicKeyInfo, matching the output of the
    // previous x509-cert-based implementation on this same cert.
    const FIXTURE_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIDHTCCAgWgAwIBAgIUX+LhtkIezZMdAHxWyyVT9XwE//owDQYJKoZIhvcNAQEL
BQAwHjEcMBoGA1UEAwwTZmluZ2VycHJpbnQtZml4dHVyZTAeFw0yNjExMDIwNjAy
NDhaFw0yNzExMDIwNjAyNDhaMB4xHDAaBgNVBAMME2ZpbmdlcnByaW50LWZpeHR1
cmUwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDZCCoHPOH53HFglj6D
Rv9kiYv+0B0QCc3QUm/Lo+Taps+ivsASDkIohBlN3XYJJf/PU/C2GU/trzQJPdae
n0ez02qP+B/91bnGGfd6xIzM83tB2F+nr7v55psnf1RhEUQWo/kYgia/9qlVPYZ1
6Idzd2iyKwfDkUFNTvwEhcyp8G0PxJDHCafPgbjBSbM3crzwuz74DxCplhILmuDN
YNS+nHVE0YeZK49T71YfaHvx4vC/tb52PW65B/GnnYrk/mPJC/93ox4TxbH3rphK
q1tBnBKQdOnJGz/Hs2QCNTMWkd2BXangLk1TOm2QMLEfvJRYo8ky/3YUlqPYuURr
x1SLAgMBAAGjUzBRMB0GA1UdDgQWBBRsHoTKKyQKXOYC9eB4QNMletiyqzAfBgNV
HSMEGDAWgBRsHoTKKyQKXOYC9eB4QNMletiyqzAPBgNVHRMBAf8EBTADAQH/MA0G
CSqGSIb3DQEBCwUAA4IBAQBCD6Ax3cTTPlrAXKM5xztc8Dcw9eY8XeY9uBf1B5Ky
WUBaySNxyUIQzvYgau3UicAZLfcXRN6BicL8X4xRkOC03mbXaVIhrNrj+ky1ZOpf
f+SE5zccNXuxDBKAJLcG+6anbNvgmY00Bhfxy9aVA2icqI/A5VLDwRBF8jLU52bY
ERqyIb1m33SQ+2JHNzeEc9zTsBThKRKXlQtdm+2f+dLIp82UVvSLdSmReFNV6rUb
ryInjBPO6+r2/hYCUEcyzzD4vyfZMTufdWVQRyU6+d9lwIQg6/RYcBpKGtccmXV+
z7asJfk1oLWjA7YsBiZv0KJq3GJ/2cQlV21zL7GKiRQl
-----END CERTIFICATE-----";

    #[test]
    fn fingerprint_matches_spki_sha256() {
        let (_, pem) = x509_parser::pem::parse_x509_pem(FIXTURE_CERT_PEM.as_bytes()).unwrap();
        assert_eq!(
            certificate_public_key_fp_from_der(&pem.contents).unwrap(),
            "e8d83a4ba14bfadb58a900b765773894952104738c8dbe2ea4bfeddbbd8654b5"
        );
    }

    #[test]
    fn fingerprint_rejects_garbage_der() {
        assert!(matches!(
            certificate_public_key_fp_from_der(b"not a certificate"),
            Err(TlsPinError::InvalidCertificate(_))
        ));
    }
}
