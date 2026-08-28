//! Venice.ai TEE attestation: REPORTDATA layout decoder + secp256k1 binding check.
//!
//! Per spike 001 (.claude/skills/spike-findings-confidential-app/sources/001-venice-tee-protocol-research/):
//! Venice runs on Phala dstack with Intel TDX + NVIDIA H100 CC.
//! REPORTDATA layout: `[20B keccak-address][12B zero-pad][32B raw nonce]`.
//! The 20B address binds the per-session secp256k1 signing key into the TDX quote.
//!
//! Public surface (consumed by Plan 03 `llm/venice.rs`):
//! - [`VerifiedVeniceAttestation`] — handle for a successfully attested session
//! - [`VeniceAttestationResponse`] — wire-format struct for `/api/v1/tee/attestation`
//! - [`verify_venice_report_data`] — pure decoder (golden-capture testable)
//! - [`fetch_and_verify_venice_attestation`] — full network + crypto orchestrator
//! - [`ensure_verified_venice_attestation`] — cached wrapper (in-memory, 4h TTL)
//! - [`invalidate_cached_venice_attestation`] — manual eviction hook

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use rand::Rng;
use serde::Deserialize;
use sha3::{Digest, Keccak256};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::error::AttestationError;
use super::tdx::ReportDataLayout;
use crate::llm::BackendConfig;
use crate::llm::LlmError;

const ATTESTATION_PATH: &str = "/api/v1/tee/attestation";
const ATTESTATION_TTL_SECS: u64 = 4 * 3600;
const HTTP_TIMEOUT_SECS: u64 = 30;

static VERIFIED_ATTESTATIONS: Lazy<Mutex<HashMap<String, VerifiedVeniceAttestation>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Verified Venice session attestation.
///
/// Holds the per-session secp256k1 signing pubkey (cryptographically bound into
/// the TDX REPORTDATA), the nonce that was bound into the quote, and the raw
/// report blob (logged-only; never persisted to SQLite per D3 / Pitfall 5).
#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
pub struct VerifiedVeniceAttestation {
    #[zeroize(skip)]
    pub request_base_url: String,
    #[zeroize(skip)]
    pub model: String,
    pub signing_pubkey_uncompressed: [u8; 65],
    pub submitted_nonce: [u8; 32],
    #[zeroize(skip)]
    pub report_blob: Vec<u8>,
    #[zeroize(skip)]
    pub expires_at: u64,
}

/// Wire format for `GET /api/v1/tee/attestation?model=…&nonce=…`.
///
/// `nvidia_payload` is typed as `Option<String>` (NOT `Option<Value>`) because
/// the server returns a JSON-encoded string containing JSON — see Pitfall 2.
/// The double-parse happens in [`fetch_and_verify_venice_attestation`].
#[derive(Debug, Deserialize)]
pub struct VeniceAttestationResponse {
    pub intel_quote: String,
    /// Spike capture observed both `signing_key` and `signing_public_key` spellings — Pitfall 3.
    #[serde(alias = "signing_public_key")]
    pub signing_key: String,
    /// JSON-as-string. Forwarded literally to NRAS after a `serde_json::from_str`.
    pub nvidia_payload: Option<String>,
    pub signing_address: String,
    pub nonce: String,
    pub model: String,
    /// Logged for debugging only — NEVER trusted as a verification signal.
    #[serde(default)]
    pub server_verification: Option<serde_json::Value>,
}

/// Decode and verify the Venice TDX REPORTDATA against the advertised signing key.
///
/// Layout: `[0..20]` = keccak256(pubkey[1..])[12..32] (Ethereum-style address);
/// `[20..32]` = zero pad; `[32..64]` = client-submitted nonce.
///
/// All four checks must pass:
/// 1. Pubkey hex decodes to 65 bytes starting with `0x04` (uncompressed secp256k1)
/// 2. Address bytes match keccak digest tail
/// 3. Padding bytes are all zero
/// 4. Nonce echo matches client-submitted nonce
pub fn verify_venice_report_data(
    report_data: &[u8; 64],
    signing_pubkey_hex: &str,
    submitted_nonce: &[u8; 32],
) -> Result<(), AttestationError> {
    let pubkey =
        hex::decode(signing_pubkey_hex).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Venice signing pubkey hex decode failed: {e}"),
        })?;
    if pubkey.len() != 65 || pubkey[0] != 0x04 {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not in uncompressed secp256k1 form".into(),
        });
    }
    let mut h = Keccak256::new();
    h.update(&pubkey[1..]);
    let digest = h.finalize();
    let addr20 = &digest[12..32];
    if addr20 != &report_data[0..20] {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not bound to TDX REPORTDATA[0..20]".into(),
        });
    }
    if report_data[20..32].iter().any(|&b| b != 0) {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice REPORTDATA[20..32] padding non-zero".into(),
        });
    }
    if report_data[32..64] != submitted_nonce[..] {
        return Err(AttestationError::NonceMismatch {
            expected: hex::encode(submitted_nonce),
            actual: hex::encode(&report_data[32..64]),
        });
    }
    Ok(())
}

/// Full Venice attestation orchestrator.
///
/// Sequence:
/// 1. Generate fresh 32-byte nonce
/// 2. GET `{base_url}/api/v1/tee/attestation?model=…&nonce=…` (no Authorization, per D14)
/// 3. Parse `VeniceAttestationResponse`
/// 4. Decode `intel_quote` from hex
/// 5. `verify_tdx_quote(..., ReportDataLayout::VeniceAddrPadNonce)` — sig + collateral + nonce-at-32
/// 6. Re-parse quote to read `td_attributes`; reject if debug bit set (Pitfall 6 / VEN-06)
/// 7. Extract REPORTDATA and call `verify_venice_report_data` (address binding + zero pad)
/// 8. Sanity gate: server-echoed model must equal client-requested model
/// 9. Double-parse `nvidia_payload` (Pitfall 2) and forward to `fetch_and_verify_nvidia`
/// 10. Build `VerifiedVeniceAttestation` (TTL = now + 4h)
pub async fn fetch_and_verify_venice_attestation(
    backend: &BackendConfig,
    requested_model: &str,
    _tdx_policy: &super::policy::TdxPolicy,
) -> Result<VerifiedVeniceAttestation, AttestationError> {
    // 1. Fresh nonce
    let mut nonce_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce_hex = hex::encode(nonce_bytes);

    // 2. Build URL + GET
    // Strip a trailing `/api/v1` so backends configured with the canonical
    // OpenAI-compatible root (e.g. `https://api.venice.ai/api/v1/`) don't
    // double the segment — ATTESTATION_PATH already carries `/api/v1`.
    // Mirrors the `/v1` normalization in attestation/redpill.rs.
    let base = backend
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/api/v1");
    let url = format!(
        "{}{}?model={}&nonce={}",
        base,
        ATTESTATION_PATH,
        urlencoding::encode(requested_model),
        nonce_hex
    );

    log::debug!(
        target: "attestation",
        "[venice] fetching attestation backend={} url={}",
        backend.id,
        url
    );

    crate::net::tls::ensure_default_crypto_provider();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("HTTP client build failed: {e}"),
        })?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("Venice attestation GET failed: {e}"),
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(AttestationError::NetworkError {
            reason: format!("Venice attestation returned HTTP {}", status.as_u16()),
        });
    }

    let body = response
        .text()
        .await
        .map_err(|e| AttestationError::NetworkError {
            reason: format!("Venice attestation body read failed: {e}"),
        })?;

    // 3. Parse wire format
    let resp: VeniceAttestationResponse =
        serde_json::from_str(&body).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Venice attestation JSON parse failed: {e}"),
        })?;

    log::debug!(
        target: "attestation",
        "[venice] server_verification={:?}",
        resp.server_verification
    );

    // 4. Hex-decode the intel quote
    let quote_bytes =
        hex::decode(&resp.intel_quote).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Venice intel_quote hex decode failed: {e}"),
        })?;
    if quote_bytes.len() < 48 {
        return Err(AttestationError::QuoteVerification {
            reason: format!("Venice intel_quote too short: {} bytes", quote_bytes.len()),
        });
    }

    // 5. Cryptographic TDX verify (sig, collateral, TCB, CRL) + nonce at [32..64]
    let _tdx_event = super::tdx::verify_tdx_quote(
        &quote_bytes,
        &nonce_bytes,
        &backend.id,
        ReportDataLayout::VeniceAddrPadNonce,
    )
    .await?;

    // 6. Debug-bit gate (Pitfall 6 / VEN-06): re-parse to inspect td_attributes + read REPORTDATA
    let parsed = dcap_qvl::quote::Quote::parse(&quote_bytes).map_err(|e| {
        AttestationError::QuoteVerification {
            reason: format!("Venice quote re-parse failed: {e}"),
        }
    })?;

    let (td_attributes, report_data): ([u8; 8], [u8; 64]) = match &parsed.report {
        dcap_qvl::quote::Report::TD10(td) => (td.td_attributes, td.report_data),
        dcap_qvl::quote::Report::TD15(td) => (td.base.td_attributes, td.base.report_data),
        dcap_qvl::quote::Report::SgxEnclave(_) => {
            return Err(AttestationError::QuoteVerification {
                reason: "Venice quote is not a TDX TD10/TD15 report".into(),
            })
        }
    };
    if td_attributes[0] & 0x01 != 0 {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice TDX is in debug mode (td_attributes[0] & 0x01 != 0)".into(),
        });
    }

    // 7. REPORTDATA decoder — address binding + zero pad + nonce echo
    verify_venice_report_data(&report_data, &resp.signing_key, &nonce_bytes)?;

    // 8. Model echo gate (RESEARCH §6 — server must echo the model the client asked for)
    if resp.model != requested_model {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "Venice model echo mismatch: requested {requested_model:?}, server returned {:?}",
                resp.model
            ),
        });
    }

    // 9. NRAS double-parse (Pitfall 2): nvidia_payload is JSON inside a JSON string.
    //    Forward the inner string to fetch_and_verify_nvidia (which already POSTs it
    //    as `evidence` to nras.attestation.nvidia.com).
    let nvidia_payload_str =
        resp.nvidia_payload
            .ok_or_else(|| AttestationError::QuoteVerification {
                reason: "Venice response missing nvidia_payload".into(),
            })?;
    // Sanity-check that the inner content actually parses as JSON before forwarding.
    let _: serde_json::Value = serde_json::from_str(&nvidia_payload_str).map_err(|e| {
        AttestationError::QuoteVerification {
            reason: format!("Venice nvidia_payload inner JSON parse failed: {e}"),
        }
    })?;
    let _nvidia_evt =
        super::nvidia::fetch_and_verify_nvidia(&nvidia_payload_str, &nonce_hex, &backend.id)
            .await?;

    // 10. Build verified handle
    let pk_bytes =
        hex::decode(&resp.signing_key).map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Venice signing pubkey hex decode failed: {e}"),
        })?;
    if pk_bytes.len() != 65 || pk_bytes[0] != 0x04 {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not in uncompressed secp256k1 form".into(),
        });
    }
    let mut pk65 = [0u8; 65];
    pk65.copy_from_slice(&pk_bytes);

    let now_secs = now_secs();
    // Normalise base url to "{root}/api/v1" for Plan 03 transport reuse.
    let root = backend
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/api/v1");
    Ok(VerifiedVeniceAttestation {
        request_base_url: format!("{root}/api/v1"),
        model: requested_model.to_string(),
        signing_pubkey_uncompressed: pk65,
        submitted_nonce: nonce_bytes,
        report_blob: quote_bytes,
        expires_at: now_secs + ATTESTATION_TTL_SECS,
    })
}

/// Cached wrapper. In-memory only — never persisted (D3, Pitfall 5).
///
/// Cache key: `"{base_url}|{model}"`. Expired entries are evicted on access.
pub async fn ensure_verified_venice_attestation(
    backend: &BackendConfig,
    requested_model: &str,
    tdx_policy: &super::policy::TdxPolicy,
) -> Result<VerifiedVeniceAttestation, LlmError> {
    let cache_key = format!(
        "{}|{}",
        backend.base_url.trim_end_matches('/'),
        requested_model
    );
    let now = now_secs();

    {
        let mut cache = VERIFIED_ATTESTATIONS
            .lock()
            .map_err(|_| LlmError::NetworkError {
                reason: "Attestation cache lock poisoned".into(),
            })?;
        if let Some(cached) = cache.get(&cache_key) {
            if cached.expires_at > now {
                return Ok(cached.clone());
            }
            // Drop expired entry — ZeroizeOnDrop wipes the pubkey + nonce.
            cache.remove(&cache_key);
        }
    }

    let verified = fetch_and_verify_venice_attestation(backend, requested_model, tdx_policy)
        .await
        .map_err(|e| LlmError::NetworkError {
            reason: format!("Venice attestation: {e:?}"),
        })?;

    VERIFIED_ATTESTATIONS
        .lock()
        .map_err(|_| LlmError::NetworkError {
            reason: "Attestation cache lock poisoned".into(),
        })?
        .insert(cache_key, verified.clone());

    Ok(verified)
}

/// Manually evict a cached Venice attestation (e.g. after a 401 or signing failure
/// in the transport layer prompts a re-attest).
pub fn invalidate_cached_venice_attestation(backend: &BackendConfig, model: &str) {
    if let Ok(mut cache) = VERIFIED_ATTESTATIONS.lock() {
        let key = format!("{}|{}", backend.base_url.trim_end_matches('/'), model);
        cache.remove(&key);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
