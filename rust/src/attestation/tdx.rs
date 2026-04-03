//! Intel TDX/DCAP quote verification via dcap-qvl.
//!
//! Wraps `dcap_qvl::collateral::get_collateral` + `dcap_qvl::verify::verify`
//! for TDX DCAP quote cryptographic verification.
//!
//! Per D-04: self-verification is the primary path.
//! Per D-13: nonce in TDX report_data field must match the challenge nonce.

use std::time::SystemTime;

use super::error::AttestationError;
use super::AttestationEvent;

/// TDX quote minimum size check (DCAP quote header is 48 bytes minimum).
const MIN_QUOTE_LEN: usize = 48;

/// Decode a TDX DCAP quote from either hex or base64 encoding.
///
/// Some providers send the intel_quote as a hex string; others use base64.
/// Per Pitfall 4 from RESEARCH.md: try hex first, then base64.
///
/// Returns `AttestationError::QuoteVerification` if neither encoding is valid
/// or if the decoded bytes are shorter than the minimum TDX quote header size.
pub fn decode_quote(s: &str) -> Result<Vec<u8>, AttestationError> {
    // Try hex first, then fall back to base64
    let decoded = if let Ok(bytes) = hex::decode(s) {
        bytes
    } else {
        // Fall back to base64
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        STANDARD.decode(s).map_err(|_| AttestationError::QuoteVerification {
            reason: "Failed to decode quote: neither valid hex nor base64".to_string(),
        })?
    };

    if decoded.len() < MIN_QUOTE_LEN {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "Quote too short: {} bytes (minimum {} required for TDX quote header)",
                decoded.len(),
                MIN_QUOTE_LEN
            ),
        });
    }

    Ok(decoded)
}

/// Verify an Intel TDX DCAP quote against Intel PCS collateral.
///
/// Fetches collateral from Phala's hosted PCCS (primary) or Intel PCS (fallback),
/// then runs cryptographic verification via `dcap_qvl::verify::verify`.
///
/// Per D-13: validates the nonce embedded in the TDX report's `report_data` field.
/// The first 32 bytes of `report_data` must match `expected_nonce`.
///
/// Per D-08: returns `AttestationEvent::Verified` with `expires_at = now + 4 hours`.
pub async fn verify_tdx_quote(
    quote_bytes: &[u8],
    expected_nonce: &[u8; 32],
    backend_id: &str,
) -> Result<AttestationEvent, AttestationError> {
    log::debug!(target: "attestation", "[attestation] verify_tdx_quote backend={} quote_bytes={}", backend_id, quote_bytes.len());
    if quote_bytes.len() < MIN_QUOTE_LEN {
        return Err(AttestationError::QuoteVerification {
            reason: format!(
                "Quote too short: {} bytes (minimum {} required)",
                quote_bytes.len(),
                MIN_QUOTE_LEN
            ),
        });
    }

    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Fetch collateral from Phala's PCCS (no Intel API key required)
    // Per Pitfall 2 from RESEARCH.md: use Phala PCCS as primary to avoid Intel PCS rate limits.
    log::debug!(target: "attestation", "[attestation] fetching collateral from Phala PCCS backend={}", backend_id);
    let collateral =
        dcap_qvl::collateral::get_collateral(dcap_qvl::collateral::PHALA_PCCS_URL, quote_bytes)
            .await
            .map_err(|e| {
                log::warn!(target: "attestation", "[attestation] collateral fetch failed backend={} error={}", backend_id, e);
                AttestationError::CollateralFetch {
                    reason: e.to_string(),
                }
            })?;

    // Verify the quote against the fetched collateral
    let report = dcap_qvl::verify::verify(quote_bytes, &collateral, now_secs).map_err(|e| {
        log::warn!(target: "attestation", "[attestation] quote verification failed backend={} error={}", backend_id, e);
        AttestationError::QuoteVerification {
            reason: e.to_string(),
        }
    })?;

    // Validate nonce: first 32 bytes of TDX report_data must match expected_nonce.
    // Per D-13: nonce validation is mandatory -- prevents replay attacks.
    let report_data = match &report.report {
        dcap_qvl::quote::Report::TD10(td) => &td.report_data[..],
        dcap_qvl::quote::Report::TD15(td) => &td.base.report_data[..],
        dcap_qvl::quote::Report::SgxEnclave(enclave) => &enclave.report_data[..],
    };

    let nonce_in_report = &report_data[..32.min(report_data.len())];
    if nonce_in_report != expected_nonce.as_slice() {
        log::warn!(target: "attestation", "[attestation] nonce mismatch backend={} expected={} actual={}", backend_id, hex::encode(expected_nonce), hex::encode(nonce_in_report));
        return Err(AttestationError::NonceMismatch {
            expected: hex::encode(expected_nonce),
            actual: hex::encode(nonce_in_report),
        });
    }

    // Per D-08: TDX/DCAP TTL is 4 hours.
    let expires_at = now_secs + 4 * 3600;

    Ok(AttestationEvent::Verified {
        backend_id: backend_id.to_string(),
        tee_type: "IntelTdx".to_string(),
        report_blob: quote_bytes.to_vec(),
        expires_at,
        tls_public_key_fp: None,
        // TDX verification does not use AMD KDS — no VCEK cert involved.
        vcek_url: None,
        vcek_der: None,
    })
}
