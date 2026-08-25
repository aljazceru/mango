//! Redpill TEE-attested aggregator: response-shape dispatch + four REPORTDATA decoders +
//! debug-mode gate + three-way AND composition for Orchestrated responses.
//!
//! Substrate: `api.redpill.ai` routes across Phala-pure (Shape A), Phala/NearAI-orchestrated
//! (Shape B), Chutes (Shape C), and Tinfoil (Shape D — currently broken at the relay).
//! Spike: `.planning/spikes/002-redpill-tee-verification-research/`.
//!
//! The model-ecdsa REPORTDATA decoder is byte-identical to Venice (Phase 33) and is
//! reused VERBATIM via `pub use crate::attestation::venice::verify_venice_report_data
//! as verify_redpill_model_reportdata` — single source of truth, no copy-paste.
//!
//! Task 1 surface: shape dispatcher, `quote_bytes` helper, debug-mode gate, RedpillError,
//! and the model-decoder re-export. Decoders + orchestrator land in Tasks 2 + 3.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::error::AttestationError;
use crate::llm::BackendConfig;
use crate::llm::LlmError;

const HTTP_TIMEOUT_SECS: u64 = 30;

// REPORTDATA at TDX v4 quote bytes [568..632] (header 48B + body offset 520..584)
pub(crate) const REPORTDATA_OFFSET: usize = 568;
pub(crate) const REPORTDATA_LEN: usize = 64;
// td_attributes is at body offset 120..128 (quote offset 48 + 120..48 + 128)
pub(crate) const TD_ATTRIBUTES_OFFSET: usize = 48 + 120;
pub(crate) const ATTESTATION_PATH: &str = "/v1/attestation/report";
pub(crate) const ATTESTATION_TTL_SECS: u64 = 5 * 60; // 5 min per CONTEXT D-20

/// Re-export the Venice model-ecdsa REPORTDATA decoder. Single source of truth —
/// the model layout in Shape A and the model component of Shape B are byte-identical
/// to the Venice REPORTDATA, so we share the implementation.
pub use crate::attestation::venice::verify_venice_report_data as verify_redpill_model_reportdata;

// ── Errors ───────────────────────────────────────────────────────────────────

/// Redpill-specific error taxonomy. NOT UniFFI-exported — callers that cross the FFI
/// boundary map this to `LlmError::NetworkError`.
#[derive(Debug, thiserror::Error)]
pub enum RedpillError {
    #[error("Redpill response shape unknown / unsupported")]
    UnknownShape,

    #[error("Redpill quote bytes failed to decode: {0}")]
    QuoteDecode(String),

    #[error("Redpill TDX quote in debug mode (td_attributes[0] & 0x01 != 0)")]
    DebugMode,

    #[error("Redpill REPORTDATA binding failed: {component}: {detail}")]
    ReportDataMismatch {
        component: &'static str,
        detail: String,
    },

    #[error("Redpill compose-manager actions_hash mismatch: expected={expected} actual={actual}")]
    ComposeManagerMismatch { expected: String, actual: String },

    #[error(
        "Redpill Tinfoil-routed model not supported via aggregator; use direct-Tinfoil integration"
    )]
    TinfoilUnsupported,

    #[error("Redpill orchestrated three-way AND failed: {failed}")]
    OrchestratedComponentFailed { failed: &'static str },

    #[error("Redpill attestation network error: {0}")]
    Network(String),

    #[error("Redpill underlying attestation error: {0}")]
    Inner(#[from] AttestationError),
}

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedpillShape {
    Flat,
    Orchestrated { is_near_ai: bool },
    Chutes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Freshness {
    /// Shape A and B: client nonce echoed into REPORTDATA[32..64]
    PerRequest,
    /// Shape C: enclave-baked nonce; valid for enclave lifetime
    PerEnclave,
}

#[derive(Clone, Debug)]
pub struct OrchestratedComponents {
    pub gateway_signing_address_hex: String, // ed25519 pubkey
    pub model_signing_address_hex: String,   // ecdsa Eth-address
    pub compose_manager_actions_hash_hex: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedRedpillAttestation {
    pub backend_id: String,
    pub model: String,
    pub shape: RedpillShape,
    pub freshness: Freshness,
    /// For Orchestrated: which of the three components verified (always all three on success)
    pub orchestrated_components: Option<OrchestratedComponents>,
    pub expires_at: u64,
}

// ── Shape dispatcher (D-04 / RED-03) ─────────────────────────────────────────

/// Detect the response shape per `redpill-verifier::js/src/providers/detect.ts` priority.
/// Fails closed on unknown — no permissive fallback.
pub fn detect_shape(value: &serde_json::Value) -> Result<RedpillShape, RedpillError> {
    // 1. Highest priority: Chutes
    if value.get("attestation_type").and_then(|v| v.as_str()) == Some("chutes") {
        return Ok(RedpillShape::Chutes);
    }
    // 2. Orchestrated: gateway_attestation + model_attestations[]
    if value.get("gateway_attestation").is_some()
        && value
            .get("model_attestations")
            .and_then(|v| v.as_array())
            .is_some()
    {
        let is_near_ai = value
            .get("model_attestations")
            .and_then(|m| m.get(0))
            .and_then(|m0| m0.get("compose_manager_attestation"))
            .map(|cm| cm.is_object())
            .unwrap_or(false);
        return Ok(RedpillShape::Orchestrated { is_near_ai });
    }
    // 3. Flat: top-level signing_address + intel_quote
    if value.get("signing_address").is_some() && value.get("intel_quote").is_some() {
        return Ok(RedpillShape::Flat);
    }
    Err(RedpillError::UnknownShape)
}

// ── Quote bytes helper (D-06) ────────────────────────────────────────────────

/// Decode `s` as TDX quote bytes auto-detecting hex (Shape A/B) vs base64 (Shape C).
/// Strips a leading `0x` prefix. Returns `Err(RedpillError::QuoteDecode)` on garbage.
pub fn quote_bytes(s: &str) -> Result<Vec<u8>, RedpillError> {
    let s = s.trim().trim_start_matches("0x");
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex::decode(s).map_err(|e| RedpillError::QuoteDecode(format!("hex: {e}")))
    } else {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|e| RedpillError::QuoteDecode(format!("base64: {e}")))
    }
}

// ── Debug-mode gate (D-07 / RED-08) ──────────────────────────────────────────

/// Return true iff the TDX quote's `td_attributes` debug bit is CLEAR (production enclave).
/// Returns false (=> reject) on too-short quotes or debug-bit set.
/// Reference: `redpill-verifier::chutes.ts` and Python `decode-report-data.py`.
pub fn debug_mode_disabled(quote_bytes: &[u8]) -> bool {
    quote_bytes.len() > TD_ATTRIBUTES_OFFSET && (quote_bytes[TD_ATTRIBUTES_OFFSET] & 0x01) == 0
}

// ── REPORTDATA decoders (D-08 / D-09 / D-10 / D-11) ──────────────────────────
//
// The model decoder is `pub use`-d above as `verify_redpill_model_reportdata`.
// Below: gateway (ed25519), compose-manager, Chutes anti-tamper.

/// Gateway REPORTDATA layout (Shape B `gateway_attestation`):
/// `[0..32] = raw ed25519 pubkey` | `[32..64] = client nonce`.
pub fn verify_redpill_gateway_reportdata(
    report_data: &[u8; 64],
    gateway_signing_address_hex: &str,
    submitted_nonce: &[u8; 32],
) -> Result<(), RedpillError> {
    let expected_addr =
        hex::decode(gateway_signing_address_hex.trim_start_matches("0x")).map_err(|e| {
            RedpillError::ReportDataMismatch {
                component: "gateway",
                detail: format!("addr hex decode: {e}"),
            }
        })?;
    if expected_addr.len() != 32 {
        return Err(RedpillError::ReportDataMismatch {
            component: "gateway",
            detail: format!("ed25519 pubkey must be 32B, got {}", expected_addr.len()),
        });
    }
    if report_data[0..32] != expected_addr[..] {
        return Err(RedpillError::ReportDataMismatch {
            component: "gateway",
            detail: "ed25519 pubkey not bound to REPORTDATA[0..32]".into(),
        });
    }
    if report_data[32..64] != submitted_nonce[..] {
        return Err(RedpillError::ReportDataMismatch {
            component: "gateway",
            detail: format!(
                "nonce mismatch: expected={} actual={}",
                hex::encode(submitted_nonce),
                hex::encode(&report_data[32..64])
            ),
        });
    }
    Ok(())
}

/// Compose-manager REPORTDATA (Shape B `model_attestations[i].compose_manager_attestation`):
/// `[0..32] = actions_hash` | `[32..64] = client nonce`.
pub fn verify_redpill_compose_manager_reportdata(
    report_data: &[u8; 64],
    actions_hash_hex: &str,
    submitted_nonce: &[u8; 32],
) -> Result<(), RedpillError> {
    let expected = hex::decode(actions_hash_hex.trim_start_matches("0x")).map_err(|e| {
        RedpillError::ReportDataMismatch {
            component: "compose_manager",
            detail: format!("actions_hash hex decode: {e}"),
        }
    })?;
    if expected.len() != 32 {
        return Err(RedpillError::ReportDataMismatch {
            component: "compose_manager",
            detail: format!("actions_hash must be 32B, got {}", expected.len()),
        });
    }
    if report_data[0..32] != expected[..] {
        return Err(RedpillError::ComposeManagerMismatch {
            expected: hex::encode(&expected),
            actual: hex::encode(&report_data[0..32]),
        });
    }
    if report_data[32..64] != submitted_nonce[..] {
        return Err(RedpillError::ReportDataMismatch {
            component: "compose_manager",
            detail: "nonce mismatch in REPORTDATA[32..64]".into(),
        });
    }
    Ok(())
}

/// Chutes anti-tamper REPORTDATA (Shape C `all_attestations[i]`):
/// `[0..32] = SHA256(nonce_str ++ e2e_pubkey_str)` (STRING concat of as-emitted ASCII bytes).
/// `[32..64]` is intentionally NOT constrained — Chutes does not bind the client `?nonce=` here.
pub fn verify_redpill_chutes_anti_tamper(
    report_data: &[u8; 64],
    enclave_baked_nonce_str: &str,
    e2e_pubkey_str: &str,
) -> Result<(), RedpillError> {
    let mut h = Sha256::new();
    h.update(enclave_baked_nonce_str.as_bytes());
    h.update(e2e_pubkey_str.as_bytes());
    let digest = h.finalize();
    if report_data[0..32] != digest[..] {
        return Err(RedpillError::ReportDataMismatch {
            component: "chutes_anti_tamper",
            detail: format!(
                "SHA256(nonce_str ++ e2e_pubkey_str) mismatch: expected={} actual={}",
                hex::encode(digest),
                hex::encode(&report_data[0..32])
            ),
        });
    }
    // rd[32..64] is intentionally NOT constrained on Chutes (D-11). Freshness is bounded
    // by enclave lifetime, not by per-request client nonce.
    Ok(())
}

// ── Wire-format response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RedpillFlatResponse {
    pub intel_quote: String,
    #[serde(default)]
    pub signing_address: Option<String>,
    #[serde(default)]
    pub signing_algo: Option<String>,
    #[serde(default)]
    pub nvidia_payload: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedpillGatewayAttestation {
    pub signing_address: String,
    pub intel_quote: String,
    #[serde(default)]
    pub report_data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedpillComposeManagerAttestation {
    pub actions_hash: String,
    #[serde(default)]
    pub report_data: Option<String>,
    #[serde(default)]
    pub quote: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RedpillModelAttestation {
    #[serde(default)]
    pub model_name: Option<String>,
    pub signing_address: String,
    #[serde(default, alias = "signing_public_key")]
    pub signing_key: Option<String>,
    pub intel_quote: String,
    #[serde(default)]
    pub nvidia_payload: Option<String>,
    #[serde(default)]
    pub compose_manager_attestation: Option<RedpillComposeManagerAttestation>,
}

#[derive(Debug, Deserialize)]
pub struct RedpillOrchestratedResponse {
    pub gateway_attestation: RedpillGatewayAttestation,
    pub model_attestations: Vec<RedpillModelAttestation>,
}

#[derive(Debug, Deserialize)]
pub struct RedpillChutesAttestation {
    #[serde(default)]
    pub instance_id: Option<String>,
    /// Entries can be incomplete (e.g. `e2e_pubkey: null` on an instance that is
    /// mid-boot or being decommissioned). Incomplete entries are skipped by
    /// `verify_chutes`; at least one complete entry must remain.
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub e2e_pubkey: Option<String>,
    #[serde(default)]
    pub intel_quote: Option<String>,
    #[serde(default)]
    pub gpu_evidence: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RedpillChutesResponse {
    pub attestation_type: String,
    #[serde(default)]
    pub nonce: Option<String>,
    pub all_attestations: Vec<RedpillChutesAttestation>,
}

// ── Verify orchestrator + cache ──────────────────────────────────────────────

static VERIFIED_ATTESTATIONS: Lazy<Mutex<HashMap<String, VerifiedRedpillAttestation>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Full Redpill attestation orchestrator.
///
/// 1. Generate fresh 32B nonce; GET `{base}/v1/attestation/report?model=…&nonce=…` (no auth).
/// 2. Detect HTTP 502 + "Unsupported Tinfoil" body → `TinfoilUnsupported`.
/// 3. Parse JSON; `detect_shape`.
/// 4. Dispatch on shape (Flat / Orchestrated / Chutes) — verify TDX quote(s),
///    debug-mode gate, REPORTDATA decoder(s), NRAS, three-way AND on Orchestrated.
pub async fn fetch_and_verify_redpill_attestation(
    backend: &BackendConfig,
    requested_model: &str,
    tdx_policy: &super::policy::TdxPolicy,
) -> Result<VerifiedRedpillAttestation, RedpillError> {
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    let nonce_hex = hex::encode(nonce);

    // Strip both trailing slash and a `/v1` suffix so that backends configured
    // with base_url `https://api.redpill.ai/v1/` (the canonical OpenAI-compatible
    // root) don't double up the `/v1` segment when joined with ATTESTATION_PATH.
    let base = backend
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1");
    let url = format!(
        "{}{}?model={}&nonce={}",
        base,
        ATTESTATION_PATH,
        urlencoding::encode(requested_model),
        nonce_hex
    );

    log::debug!(
        target: "attestation",
        "[redpill] fetching attestation backend={} url={}",
        backend.id, url
    );

    crate::net::tls::ensure_default_crypto_provider();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|e| RedpillError::Network(e.to_string()))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| RedpillError::Network(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| RedpillError::Network(e.to_string()))?;

    // Detect Tinfoil-via-Redpill refusal (D-16)
    if status == reqwest::StatusCode::BAD_GATEWAY {
        if body.contains("Unsupported Tinfoil") {
            return Err(RedpillError::TinfoilUnsupported);
        }
        return Err(RedpillError::Network(format!("HTTP 502: {body}")));
    }
    if !status.is_success() {
        return Err(RedpillError::Network(format!(
            "HTTP {}: {}",
            status.as_u16(),
            body
        )));
    }

    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| RedpillError::Network(format!("parse: {e}")))?;
    let shape = detect_shape(&value)?;

    match shape {
        RedpillShape::Flat => {
            verify_flat(backend, requested_model, &value, &nonce, tdx_policy).await
        }
        RedpillShape::Orchestrated { is_near_ai } => {
            verify_orchestrated(
                backend,
                requested_model,
                &value,
                &nonce,
                is_near_ai,
                tdx_policy,
            )
            .await
        }
        RedpillShape::Chutes => {
            verify_chutes(backend, requested_model, &value, &nonce, tdx_policy).await
        }
    }
}

/// Shape A — Flat (Phala-pure). Single TDX quote + single NRAS.
async fn verify_flat(
    backend: &BackendConfig,
    requested_model: &str,
    value: &serde_json::Value,
    nonce: &[u8; 32],
    _policy: &super::policy::TdxPolicy,
) -> Result<VerifiedRedpillAttestation, RedpillError> {
    let intel_quote = value
        .get("intel_quote")
        .and_then(|v| v.as_str())
        .ok_or(RedpillError::UnknownShape)?;
    let q = quote_bytes(intel_quote)?;

    super::tdx::verify_tdx_quote(
        &q,
        nonce,
        &backend.id,
        super::tdx::ReportDataLayout::VeniceAddrPadNonce,
    )
    .await?;

    if !debug_mode_disabled(&q) {
        return Err(RedpillError::DebugMode);
    }

    if q.len() < REPORTDATA_OFFSET + REPORTDATA_LEN {
        return Err(RedpillError::QuoteDecode(
            "quote too short for REPORTDATA".into(),
        ));
    }
    let mut rd = [0u8; 64];
    rd.copy_from_slice(&q[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]);

    if let Some(pk_hex) = value
        .get("signing_key")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("signing_public_key").and_then(|v| v.as_str()))
    {
        verify_redpill_model_reportdata(&rd, pk_hex, nonce)?;
    } else {
        // No uncompressed pubkey — verify the address slice + zero pad + nonce manually.
        let addr_hex = value
            .get("signing_address")
            .and_then(|v| v.as_str())
            .ok_or(RedpillError::UnknownShape)?;
        let addr = hex::decode(addr_hex.trim_start_matches("0x")).map_err(|e| {
            RedpillError::ReportDataMismatch {
                component: "model",
                detail: format!("signing_address hex decode: {e}"),
            }
        })?;
        if addr.len() != 20 || rd[0..20] != addr[..] {
            return Err(RedpillError::ReportDataMismatch {
                component: "model",
                detail: "signing_address not bound to REPORTDATA[0..20]".into(),
            });
        }
        if rd[20..32].iter().any(|&b| b != 0) {
            return Err(RedpillError::ReportDataMismatch {
                component: "model",
                detail: "REPORTDATA[20..32] padding non-zero".into(),
            });
        }
        if rd[32..64] != nonce[..] {
            return Err(RedpillError::ReportDataMismatch {
                component: "model",
                detail: "nonce mismatch in REPORTDATA[32..64]".into(),
            });
        }
    }

    if let Some(payload_str) = value.get("nvidia_payload").and_then(|v| v.as_str()) {
        let _: serde_json::Value = serde_json::from_str(payload_str)
            .map_err(|e| RedpillError::Network(format!("nvidia_payload inner JSON parse: {e}")))?;
        let nonce_hex = hex::encode(nonce);
        super::nvidia::fetch_and_verify_nvidia(payload_str, &nonce_hex, &backend.id).await?;
    }

    Ok(VerifiedRedpillAttestation {
        backend_id: backend.id.clone(),
        model: requested_model.to_string(),
        shape: RedpillShape::Flat,
        freshness: Freshness::PerRequest,
        orchestrated_components: None,
        expires_at: now_secs() + ATTESTATION_TTL_SECS,
    })
}

/// Shape B — Orchestrated (gateway + model + compose-manager). Three-way AND.
async fn verify_orchestrated(
    backend: &BackendConfig,
    requested_model: &str,
    value: &serde_json::Value,
    nonce: &[u8; 32],
    is_near_ai: bool,
    _policy: &super::policy::TdxPolicy,
) -> Result<VerifiedRedpillAttestation, RedpillError> {
    let resp: RedpillOrchestratedResponse = serde_json::from_value(value.clone())
        .map_err(|e| RedpillError::Network(format!("orchestrated parse: {e}")))?;
    let m0 = resp
        .model_attestations
        .into_iter()
        .next()
        .ok_or(RedpillError::UnknownShape)?;

    // ── Gateway component ───────────────────────────────────────────────
    let gw_addr_hex = resp.gateway_attestation.signing_address.clone();
    {
        let rd: [u8; 64] = if let Some(ref rd_hex) = resp.gateway_attestation.report_data {
            let bytes = hex::decode(rd_hex).map_err(|e| RedpillError::ReportDataMismatch {
                component: "gateway",
                detail: format!("report_data hex decode: {e}"),
            })?;
            if bytes.len() < 64 {
                return Err(RedpillError::ReportDataMismatch {
                    component: "gateway",
                    detail: format!("report_data too short: {}", bytes.len()),
                });
            }
            let mut out = [0u8; 64];
            out.copy_from_slice(&bytes[..64]);
            out
        } else {
            let q = quote_bytes(&resp.gateway_attestation.intel_quote)?;
            super::tdx::verify_tdx_quote(
                &q,
                nonce,
                &backend.id,
                super::tdx::ReportDataLayout::VeniceAddrPadNonce,
            )
            .await
            .map_err(|_| RedpillError::OrchestratedComponentFailed { failed: "gateway" })?;
            if !debug_mode_disabled(&q) {
                return Err(RedpillError::DebugMode);
            }
            let mut out = [0u8; 64];
            out.copy_from_slice(&q[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]);
            out
        };
        verify_redpill_gateway_reportdata(&rd, &gw_addr_hex, nonce)
            .map_err(|_| RedpillError::OrchestratedComponentFailed { failed: "gateway" })?;
    }

    // ── Model component ─────────────────────────────────────────────────
    let model_addr_hex = m0.signing_address.clone();
    {
        let q = quote_bytes(&m0.intel_quote)?;
        super::tdx::verify_tdx_quote(
            &q,
            nonce,
            &backend.id,
            super::tdx::ReportDataLayout::VeniceAddrPadNonce,
        )
        .await
        .map_err(|_| RedpillError::OrchestratedComponentFailed { failed: "model" })?;
        if !debug_mode_disabled(&q) {
            return Err(RedpillError::DebugMode);
        }
        let mut rd = [0u8; 64];
        rd.copy_from_slice(&q[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]);

        if let Some(ref pk_hex) = m0.signing_key {
            verify_redpill_model_reportdata(&rd, pk_hex, nonce)
                .map_err(|_| RedpillError::OrchestratedComponentFailed { failed: "model" })?;
        } else {
            let addr = hex::decode(m0.signing_address.trim_start_matches("0x")).map_err(|e| {
                RedpillError::ReportDataMismatch {
                    component: "model",
                    detail: format!("addr hex decode: {e}"),
                }
            })?;
            if addr.len() != 20
                || rd[0..20] != addr[..]
                || rd[20..32].iter().any(|&b| b != 0)
                || rd[32..64] != nonce[..]
            {
                return Err(RedpillError::OrchestratedComponentFailed { failed: "model" });
            }
        }

        if let Some(ref payload_str) = m0.nvidia_payload {
            let _: serde_json::Value = serde_json::from_str(payload_str).map_err(|e| {
                RedpillError::Network(format!("nvidia_payload inner JSON parse: {e}"))
            })?;
            let nonce_hex = hex::encode(nonce);
            super::nvidia::fetch_and_verify_nvidia(payload_str, &nonce_hex, &backend.id)
                .await
                .map_err(|_| RedpillError::OrchestratedComponentFailed { failed: "model" })?;
        }
    }

    // ── Compose-manager component ───────────────────────────────────────
    let cm = m0
        .compose_manager_attestation
        .ok_or(RedpillError::OrchestratedComponentFailed {
            failed: "compose_manager",
        })?;
    let cm_actions_hash_hex = cm.actions_hash.clone();
    {
        let rd: [u8; 64] = if let Some(ref rd_hex) = cm.report_data {
            let bytes = hex::decode(rd_hex).map_err(|e| RedpillError::ReportDataMismatch {
                component: "compose_manager",
                detail: format!("report_data hex decode: {e}"),
            })?;
            if bytes.len() < 64 {
                return Err(RedpillError::OrchestratedComponentFailed {
                    failed: "compose_manager",
                });
            }
            let mut out = [0u8; 64];
            out.copy_from_slice(&bytes[..64]);
            out
        } else if let Some(ref qstr) = cm.quote {
            let q = quote_bytes(qstr)?;
            super::tdx::verify_tdx_quote(
                &q,
                nonce,
                &backend.id,
                super::tdx::ReportDataLayout::VeniceAddrPadNonce,
            )
            .await
            .map_err(|_| RedpillError::OrchestratedComponentFailed {
                failed: "compose_manager",
            })?;
            if !debug_mode_disabled(&q) {
                return Err(RedpillError::DebugMode);
            }
            let mut out = [0u8; 64];
            out.copy_from_slice(&q[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]);
            out
        } else {
            return Err(RedpillError::OrchestratedComponentFailed {
                failed: "compose_manager",
            });
        };
        verify_redpill_compose_manager_reportdata(&rd, &cm.actions_hash, nonce).map_err(|_| {
            RedpillError::OrchestratedComponentFailed {
                failed: "compose_manager",
            }
        })?;
    }

    Ok(VerifiedRedpillAttestation {
        backend_id: backend.id.clone(),
        model: requested_model.to_string(),
        shape: RedpillShape::Orchestrated { is_near_ai },
        freshness: Freshness::PerRequest,
        orchestrated_components: Some(OrchestratedComponents {
            gateway_signing_address_hex: gw_addr_hex,
            model_signing_address_hex: model_addr_hex,
            compose_manager_actions_hash_hex: cm_actions_hash_hex,
        }),
        expires_at: now_secs() + ATTESTATION_TTL_SECS,
    })
}

/// Shape C — Chutes anti-tamper. Per-entry TDX + per-GPU NRAS.
async fn verify_chutes(
    backend: &BackendConfig,
    requested_model: &str,
    value: &serde_json::Value,
    _nonce: &[u8; 32],
    _policy: &super::policy::TdxPolicy,
) -> Result<VerifiedRedpillAttestation, RedpillError> {
    let resp: RedpillChutesResponse = serde_json::from_value(value.clone())
        .map_err(|e| RedpillError::Network(format!("chutes parse: {e}")))?;

    // Skip incomplete enclave entries (missing nonce / e2e_pubkey / quote — e.g.
    // an instance mid-boot). Verification targets the first COMPLETE entry;
    // if none exists the response is unusable.
    let entry = resp
        .all_attestations
        .iter()
        .find(|a| a.nonce.is_some() && a.e2e_pubkey.is_some() && a.intel_quote.is_some())
        .ok_or(RedpillError::UnknownShape)?;
    let (entry_nonce, entry_e2e_pubkey, entry_intel_quote) = (
        entry.nonce.as_deref().unwrap(),
        entry.e2e_pubkey.as_deref().unwrap(),
        entry.intel_quote.as_deref().unwrap(),
    );

    let q = quote_bytes(entry_intel_quote)?;
    if q.len() < REPORTDATA_OFFSET + REPORTDATA_LEN {
        return Err(RedpillError::QuoteDecode("chutes quote too short".into()));
    }
    let mut rd = [0u8; 64];
    rd.copy_from_slice(&q[REPORTDATA_OFFSET..REPORTDATA_OFFSET + REPORTDATA_LEN]);

    // Chutes leaves rd[32..64] unconstrained (D-11), so we cannot expect the client
    // ?nonce= to appear there. We slice REPORTDATA ourselves and call the anti-tamper
    // decoder; for the dcap-qvl signature/collateral/TCB check we feed back rd[32..64]
    // as the "expected nonce" — the layout-nonce comparison degenerates to a tautology
    // and only the cryptographic gates remain effective. This is the documented
    // Plan-02 trade-off: see SUMMARY.md "ReportDataLayout discussion".
    let mut self_nonce = [0u8; 32];
    self_nonce.copy_from_slice(&rd[32..64]);

    super::tdx::verify_tdx_quote(
        &q,
        &self_nonce,
        &backend.id,
        super::tdx::ReportDataLayout::VeniceAddrPadNonce,
    )
    .await?;

    if !debug_mode_disabled(&q) {
        return Err(RedpillError::DebugMode);
    }

    verify_redpill_chutes_anti_tamper(&rd, entry_nonce, entry_e2e_pubkey)?;

    // Shape C: GPU evidence is bound to the per-enclave `entry.nonce`, NOT the
    // outer client nonce. NRAS will reject the JWT if we send the client nonce
    // here because the evidence's internal nonce won't match.
    for gpu in &entry.gpu_evidence {
        let payload_str = match gpu {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string(other)
                .map_err(|e| RedpillError::Network(format!("gpu_evidence serialize: {e}")))?,
        };
        let _ =
            super::nvidia::fetch_and_verify_nvidia(&payload_str, entry_nonce, &backend.id).await?;
    }

    Ok(VerifiedRedpillAttestation {
        backend_id: backend.id.clone(),
        model: requested_model.to_string(),
        shape: RedpillShape::Chutes,
        freshness: Freshness::PerEnclave,
        orchestrated_components: None,
        expires_at: now_secs() + ATTESTATION_TTL_SECS,
    })
}

/// Cached wrapper. In-memory only (D-20 — 5-min TTL).
pub async fn ensure_verified_redpill_attestation(
    backend: &BackendConfig,
    requested_model: &str,
    tdx_policy: &super::policy::TdxPolicy,
) -> Result<VerifiedRedpillAttestation, LlmError> {
    let cache_key = format!(
        "redpill|{}|{}",
        backend.base_url.trim_end_matches('/'),
        requested_model
    );
    let now = now_secs();

    {
        let mut cache = VERIFIED_ATTESTATIONS
            .lock()
            .map_err(|_| LlmError::NetworkError {
                reason: "Redpill attestation cache lock poisoned".into(),
            })?;
        if let Some(cached) = cache.get(&cache_key) {
            if cached.expires_at > now {
                return Ok(cached.clone());
            }
            cache.remove(&cache_key);
        }
    }

    let verified = fetch_and_verify_redpill_attestation(backend, requested_model, tdx_policy)
        .await
        .map_err(|e| LlmError::NetworkError {
            reason: format!("Redpill attestation: {e:?}"),
        })?;

    VERIFIED_ATTESTATIONS
        .lock()
        .map_err(|_| LlmError::NetworkError {
            reason: "Redpill attestation cache lock poisoned".into(),
        })?
        .insert(cache_key, verified.clone());

    Ok(verified)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
