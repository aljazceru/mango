use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::Deserialize;
use sev::certs::snp::{builtin, ca::Chain as CaChain, Certificate, Chain, Verifiable};
use sev::firmware::guest::AttestationReport;
use sev::parser::ByteParser;
use sev::Generation;

use super::policy::{SnpPolicy, TdxPolicy, TeePolicy};
use super::{AttestationError, AttestationEvent};
use crate::llm::{BackendConfig, ProviderKind};
use crate::net::tls::live_tls_public_key_fp;

pub type CertificateCache = Arc<RwLock<HashMap<String, Vec<u8>>>>;

const ATTESTATION_PATH: &str = "/.well-known/tinfoil-attestation";
const TDX_EXPECTED_TD_ATTRIBUTES: [u8; 8] = [0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00];
const TDX_EXPECTED_XFAM: [u8; 8] = [0xe7, 0x02, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00];
const SNP_REPORT_SIZE: usize = 1184;

#[derive(Debug, Deserialize)]
struct AttestationDoc {
    format: String,
    body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttestationFormat {
    TdxGuestV2,
    SevSnpGuestV2,
}

impl TryFrom<&str> for AttestationFormat {
    type Error = AttestationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "https://tinfoil.sh/predicate/tdx-guest/v2" => Ok(Self::TdxGuestV2),
            "https://tinfoil.sh/predicate/sev-snp-guest/v2" => Ok(Self::SevSnpGuestV2),
            other => Err(AttestationError::QuoteVerification {
                reason: format!("Unsupported attestation format: {other}"),
            }),
        }
    }
}

pub async fn verify_attestation_endpoint(
    backend: &BackendConfig,
    certificate_cache: CertificateCache,
    policy: &TeePolicy,
) -> Result<AttestationEvent, AttestationError> {
    match backend.provider_kind() {
        ProviderKind::Tinfoil
            if backend.transport_kind() == crate::llm::ProviderTransportKind::TinfoilSecure =>
        {
            Err(AttestationError::Unsupported)
        }
        ProviderKind::Ppq
            if backend.transport_kind() == crate::llm::ProviderTransportKind::PpqPrivateE2ee =>
        {
            Err(AttestationError::Unsupported)
        }
        _ => verify_quote_endpoint(backend, certificate_cache, policy).await,
    }
}

pub fn extract_tls_public_key_fp_from_report(
    tee_type: &str,
    report_blob: &[u8],
) -> Result<String, AttestationError> {
    match tee_type {
        "AmdSevSnp" => {
            let report = AttestationReport::from_bytes(report_blob).map_err(|e| {
                AttestationError::QuoteVerification {
                    reason: format!("Invalid SNP report: {e}"),
                }
            })?;
            Ok(hex::encode(&report.report_data[..32]))
        }
        "IntelTdx" | "NvidiaH100Cc" => {
            let quote = dcap_qvl::quote::Quote::parse(report_blob).map_err(|e| {
                AttestationError::QuoteVerification {
                    reason: format!("Invalid TDX quote: {e}"),
                }
            })?;
            let report_data = match &quote.report {
                dcap_qvl::quote::Report::TD10(report) => &report.report_data,
                dcap_qvl::quote::Report::TD15(report) => &report.base.report_data,
                dcap_qvl::quote::Report::SgxEnclave(report) => &report.report_data,
            };
            Ok(hex::encode(&report_data[..32]))
        }
        other => Err(AttestationError::QuoteVerification {
            reason: format!("Unsupported tee type for TLS fingerprint extraction: {other}"),
        }),
    }
}

async fn verify_quote_endpoint(
    backend: &BackendConfig,
    certificate_cache: CertificateCache,
    policy: &TeePolicy,
) -> Result<AttestationEvent, AttestationError> {
    let url = attestation_url(&backend.base_url);
    let client = Client::builder()
        .hickory_dns(false)
        .timeout(Duration::from_secs(30))
        .tls_info(true)
        .build()
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(AttestationError::NetworkError {
            reason: format!("{url} returned HTTP {}", status.as_u16()),
        });
    }

    let live_tls_fp =
        live_tls_public_key_fp(&response).map_err(|e| AttestationError::QuoteVerification {
            reason: e.to_string(),
        })?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?;
    let doc: AttestationDoc =
        serde_json::from_slice(&bytes).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Invalid attestation JSON: {e}"),
        })?;
    let report_blob = decode_attestation_body(&doc.body)?;
    let format = AttestationFormat::try_from(doc.format.as_str())?;

    match format {
        AttestationFormat::TdxGuestV2 => {
            verify_tdx_attestation(&backend.id, &report_blob, &live_tls_fp, &policy.tdx).await
        }
        AttestationFormat::SevSnpGuestV2 => {
            verify_snp_attestation(
                &backend.id,
                &report_blob,
                &live_tls_fp,
                certificate_cache,
                &policy.snp,
            )
            .await
        }
    }
}

fn attestation_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let root = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
    format!("{root}{ATTESTATION_PATH}")
}

fn decode_attestation_body(body: &str) -> Result<Vec<u8>, AttestationError> {
    let encoded = base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Invalid attestation body encoding: {e}"),
        })?;
    let mut decoder = GzDecoder::new(encoded.as_slice());
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Invalid attestation body compression: {e}"),
        })?;
    Ok(out)
}

async fn verify_tdx_attestation(
    backend_id: &str,
    quote_bytes: &[u8],
    expected_tls_public_key_fp: &str,
    tdx_policy: &TdxPolicy,
) -> Result<AttestationEvent, AttestationError> {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let collateral =
        dcap_qvl::collateral::get_collateral(dcap_qvl::collateral::PHALA_PCCS_URL, quote_bytes)
            .await
            .map_err(|e| AttestationError::CollateralFetch {
                reason: e.to_string(),
            })?;
    let verified = dcap_qvl::verify::verify(quote_bytes, &collateral, now_secs).map_err(|e| {
        AttestationError::QuoteVerification {
            reason: e.to_string(),
        }
    })?;
    let quote = dcap_qvl::quote::Quote::parse(quote_bytes).map_err(|e| {
        AttestationError::QuoteVerification {
            reason: format!("Invalid TDX quote: {e}"),
        }
    })?;

    match &verified.report {
        dcap_qvl::quote::Report::TD10(report) => verify_tdx_policy(
            &report.tee_tcb_svn,
            &report.mr_seam,
            &report.td_attributes,
            &report.xfam,
            tdx_policy,
        )?,
        dcap_qvl::quote::Report::TD15(report) => verify_tdx_policy(
            &report.base.tee_tcb_svn,
            &report.base.mr_seam,
            &report.base.td_attributes,
            &report.base.xfam,
            tdx_policy,
        )?,
        dcap_qvl::quote::Report::SgxEnclave(_) => {
            return Err(AttestationError::QuoteVerification {
                reason: "SGX enclave reports are not supported".to_string(),
            })
        }
    }

    let attested_fp = extract_tls_public_key_fp_from_report("IntelTdx", quote_bytes)?;
    if attested_fp != expected_tls_public_key_fp {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "Live TLS certificate fingerprint {} did not match attested fingerprint {}",
                expected_tls_public_key_fp, attested_fp
            ),
        });
    }

    let tee_type = match quote.report {
        dcap_qvl::quote::Report::TD10(_) | dcap_qvl::quote::Report::TD15(_) => "IntelTdx",
        dcap_qvl::quote::Report::SgxEnclave(_) => {
            return Err(AttestationError::UnsupportedTeeType {
                tee_type: "SGX".to_string(),
            })
        }
    };

    Ok(AttestationEvent::Verified {
        backend_id: backend_id.to_string(),
        tee_type: tee_type.to_string(),
        report_blob: quote_bytes.to_vec(),
        expires_at: now_secs + 4 * 3600,
        tls_public_key_fp: Some(attested_fp),
        vcek_url: None,
        vcek_der: None,
    })
}

async fn verify_snp_attestation(
    backend_id: &str,
    report_bytes: &[u8],
    expected_tls_public_key_fp: &str,
    certificate_cache: CertificateCache,
    snp_policy: &SnpPolicy,
) -> Result<AttestationEvent, AttestationError> {
    if report_bytes.len() != SNP_REPORT_SIZE {
        return Err(AttestationError::QuoteVerification {
            reason: format!("Unexpected SNP report size: {}", report_bytes.len()),
        });
    }

    let report = AttestationReport::from_bytes(report_bytes).map_err(|e| {
        AttestationError::QuoteVerification {
            reason: format!("Invalid SNP report: {e}"),
        }
    })?;
    let generation = match (report.cpuid_fam_id, report.cpuid_mod_id) {
        (Some(family), Some(model)) => Generation::identify_cpu(family, model).map_err(|e| {
            AttestationError::QuoteVerification {
                reason: format!("Unknown AMD generation family={family:#x} model={model:#x}: {e}"),
            }
        })?,
        _ => Generation::Genoa,
    };

    let product_name = match generation {
        Generation::Milan => "Milan",
        Generation::Genoa => "Genoa",
        Generation::Turin => "Turin",
    };

    let (ark_pem, ask_pem): (&[u8], &[u8]) = match generation {
        Generation::Milan => (builtin::milan::ARK, builtin::milan::ASK),
        Generation::Genoa => (builtin::genoa::ARK, builtin::genoa::ASK),
        Generation::Turin => (builtin::turin::ARK, builtin::turin::ASK),
    };
    let ca_chain =
        CaChain::from_pem(ark_pem, ask_pem).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Invalid AMD CA chain: {e}"),
        })?;

    let vcek_url = build_vcek_url(product_name, &report);
    let cached_vcek_der = certificate_cache
        .read()
        .ok()
        .and_then(|cache| cache.get(&vcek_url).cloned());
    let (_vcek_der, fresh_vcek_der) =
        verify_or_fetch_vcek(&ca_chain, &report, &vcek_url, cached_vcek_der).await?;

    verify_snp_policy(&report, snp_policy)?;

    let attested_fp = extract_tls_public_key_fp_from_report("AmdSevSnp", report_bytes)?;
    if attested_fp != expected_tls_public_key_fp {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "Live TLS certificate fingerprint {} did not match attested fingerprint {}",
                expected_tls_public_key_fp, attested_fp
            ),
        });
    }

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(AttestationEvent::Verified {
        backend_id: backend_id.to_string(),
        tee_type: "AmdSevSnp".to_string(),
        report_blob: report_bytes.to_vec(),
        expires_at: now_secs + 4 * 3600,
        tls_public_key_fp: Some(attested_fp),
        vcek_url: Some(vcek_url),
        vcek_der: fresh_vcek_der,
    })
}

fn verify_tdx_policy(
    tee_tcb_svn: &[u8; 16],
    mr_seam: &[u8; 48],
    td_attributes: &[u8; 8],
    xfam: &[u8; 8],
    policy: &TdxPolicy,
) -> Result<(), AttestationError> {
    let minimum_svn = policy.minimum_tee_tcb_svn_bytes().map_err(|e| {
        AttestationError::QuoteVerification {
            reason: format!("Invalid TDX TCB SVN hex in policy: {e}"),
        }
    })?;
    if !tee_tcb_svn
        .iter()
        .zip(minimum_svn.iter())
        .all(|(actual, minimum)| actual >= minimum)
    {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "TDX TEE TCB SVN below minimum: {}",
                hex::encode(tee_tcb_svn)
            ),
        });
    }

    let mr_seam_hex = hex::encode(mr_seam);
    if !policy.accepted_mr_seams.contains(&mr_seam_hex) {
        return Err(AttestationError::QuoteVerification {
            reason: format!("TDX MRSEAM not in accepted allowlist: {mr_seam_hex}"),
        });
    }

    if td_attributes != &TDX_EXPECTED_TD_ATTRIBUTES {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "TDX TD attributes mismatch: expected {}, got {}",
                hex::encode(TDX_EXPECTED_TD_ATTRIBUTES),
                hex::encode(td_attributes)
            ),
        });
    }

    if xfam != &TDX_EXPECTED_XFAM {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "TDX XFAM mismatch: expected {}, got {}",
                hex::encode(TDX_EXPECTED_XFAM),
                hex::encode(xfam)
            ),
        });
    }

    Ok(())
}

async fn verify_or_fetch_vcek(
    ca_chain: &CaChain,
    report: &AttestationReport,
    vcek_url: &str,
    cached_vcek_der: Option<Vec<u8>>,
) -> Result<(Vec<u8>, Option<Vec<u8>>), AttestationError> {
    if let Some(vcek_der) = cached_vcek_der {
        if verify_snp_signature_with_vcek(ca_chain, report, &vcek_der).is_ok() {
            return Ok((vcek_der, None));
        }
    }

    let client = Client::builder()
        .hickory_dns(false)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?;
    let vcek_der = client
        .get(vcek_url)
        .send()
        .await
        .and_then(|resp| resp.error_for_status())
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?
        .bytes()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: e.to_string(),
        })?
        .to_vec();

    verify_snp_signature_with_vcek(ca_chain, report, &vcek_der)?;
    Ok((vcek_der.clone(), Some(vcek_der)))
}

fn verify_snp_signature_with_vcek(
    ca_chain: &CaChain,
    report: &AttestationReport,
    vcek_der: &[u8],
) -> Result<(), AttestationError> {
    let vcek =
        Certificate::from_der(vcek_der).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Invalid VCEK certificate: {e}"),
        })?;
    let chain = Chain {
        ca: ca_chain.clone(),
        vek: vcek,
    };
    (&chain, report)
        .verify()
        .map_err(|e| AttestationError::QuoteVerification {
            reason: format!("SNP signature verification failed: {e}"),
        })?;
    Ok(())
}

// The default minimum_tee is 0x00 (no minimum enforced for the tee field).
// Clippy correctly flags `u8 < 0` as always-false; the check is retained
// for future minimum bumps without code changes.
#[allow(clippy::absurd_extreme_comparisons)]
fn verify_snp_policy(report: &AttestationReport, policy: &SnpPolicy) -> Result<(), AttestationError> {
    let guest_policy = report.policy;
    if !guest_policy.smt_allowed() {
        return Err(AttestationError::QuoteVerification {
            reason: "SNP guest policy disallows SMT".to_string(),
        });
    }
    if guest_policy.migrate_ma_allowed() {
        return Err(AttestationError::QuoteVerification {
            reason: "SNP guest policy allows migration agents".to_string(),
        });
    }
    if guest_policy.debug_allowed() {
        return Err(AttestationError::QuoteVerification {
            reason: "SNP guest policy allows debug mode".to_string(),
        });
    }
    if guest_policy.single_socket_required() {
        return Err(AttestationError::QuoteVerification {
            reason: "SNP guest policy unexpectedly requires single-socket mode".to_string(),
        });
    }

    if report.current_tcb.bootloader < policy.minimum_bootloader
        || report.current_tcb.tee < policy.minimum_tee
        || report.current_tcb.snp < policy.minimum_snp
        || report.current_tcb.microcode < policy.minimum_microcode
    {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "SNP current TCB below minimum: bootloader={} tee={} snp={} microcode={}",
                report.current_tcb.bootloader,
                report.current_tcb.tee,
                report.current_tcb.snp,
                report.current_tcb.microcode
            ),
        });
    }

    if report.reported_tcb.bootloader < policy.minimum_bootloader
        || report.reported_tcb.tee < policy.minimum_tee
        || report.reported_tcb.snp < policy.minimum_snp
        || report.reported_tcb.microcode < policy.minimum_microcode
    {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "SNP reported TCB below minimum: bootloader={} tee={} snp={} microcode={}",
                report.reported_tcb.bootloader,
                report.reported_tcb.tee,
                report.reported_tcb.snp,
                report.reported_tcb.microcode
            ),
        });
    }

    let info = report.plat_info;
    if !info.smt_enabled() {
        return Err(AttestationError::QuoteVerification {
            reason: "SNP platform info shows SMT disabled".to_string(),
        });
    }
    if !info.tsme_enabled() {
        return Err(AttestationError::QuoteVerification {
            reason: "SNP platform info shows TSME disabled".to_string(),
        });
    }

    Ok(())
}

fn build_vcek_url(product_name: &str, report: &AttestationReport) -> String {
    let chip_id = hex::encode(report.chip_id);
    format!(
        "https://kdsintf.amd.com/vcek/v1/{product_name}/{chip_id}?blSPL={}&teeSPL={}&snpSPL={}&ucodeSPL={}",
        report.reported_tcb.bootloader,
        report.reported_tcb.tee,
        report.reported_tcb.snp,
        report.reported_tcb.microcode
    )
}
