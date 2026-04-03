//! Error types for attestation verification.
//!
//! Follows the exact pattern from llm::error (thiserror + uniffi::Error).
//! Per D-10: AttestationError crosses the UniFFI boundary as an exception type.

/// Error taxonomy for attestation operations -- crosses UniFFI boundary.
/// Mirrors LlmError pattern: thiserror for Display, uniffi::Error for FFI.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AttestationError {
    #[error("Failed to fetch collateral: {reason}")]
    CollateralFetch { reason: String },

    #[error("Quote verification failed: {reason}")]
    QuoteVerification { reason: String },

    #[error("Nonce mismatch: expected {expected}, got {actual}")]
    NonceMismatch { expected: String, actual: String },

    #[error("JWT verification failed: {reason}")]
    JwtVerification { reason: String },

    #[error("Cache operation failed: {reason}")]
    CacheFailed { reason: String },

    #[error("Unsupported TEE type for attestation")]
    Unsupported,

    #[error("Network error: {reason}")]
    NetworkError { reason: String },

    #[error("Attestation cache lock poisoned")]
    CacheLockPoisoned,

    #[error("Unsupported TEE type: {tee_type}")]
    UnsupportedTeeType { tee_type: String },
}

impl AttestationError {
    /// Returns `true` when this error represents a transient process failure —
    /// i.e. we were unable to even attempt verification due to a network or
    /// collateral-fetch problem (HTTP 429, timeout, DNS failure, connection
    /// refused).  A transient error does NOT mean the TEE report is invalid;
    /// the existing `Verified` status should be preserved in that case.
    ///
    /// Returns `false` for genuine verification failures (`QuoteVerification`,
    /// `NonceMismatch`, `JwtVerification`) where the TEE report was reachable
    /// but cryptographically invalid.  Those must still downgrade status.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            AttestationError::NetworkError { .. } | AttestationError::CollateralFetch { .. }
        )
    }

    /// Human-readable display string (mirrors LlmError::display_message pattern).
    pub fn display_message(&self) -> String {
        match self {
            AttestationError::CollateralFetch { reason } => {
                format!("Attestation collateral fetch failed: {}", reason)
            }
            AttestationError::QuoteVerification { reason } => {
                format!("TDX quote verification failed: {}", reason)
            }
            AttestationError::NonceMismatch { expected, actual } => {
                format!(
                    "Attestation nonce mismatch -- possible replay attack (expected {}, got {})",
                    expected, actual
                )
            }
            AttestationError::JwtVerification { reason } => {
                format!("NVIDIA attestation JWT verification failed: {}", reason)
            }
            AttestationError::CacheFailed { reason } => {
                format!("Attestation cache error: {}", reason)
            }
            AttestationError::Unsupported => {
                "This backend does not support attestation verification".to_string()
            }
            AttestationError::NetworkError { reason } => {
                format!("Attestation network error: {}", reason)
            }
            AttestationError::CacheLockPoisoned => {
                "Attestation cache unavailable (internal lock error)".to_string()
            }
            AttestationError::UnsupportedTeeType { tee_type } => {
                format!("Unsupported TEE type for attestation: {tee_type}")
            }
        }
    }
}
