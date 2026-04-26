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

use super::error::AttestationError;

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

    #[error("Redpill Tinfoil-routed model not supported via aggregator; use direct-Tinfoil integration")]
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
    quote_bytes.len() > TD_ATTRIBUTES_OFFSET
        && (quote_bytes[TD_ATTRIBUTES_OFFSET] & 0x01) == 0
}
