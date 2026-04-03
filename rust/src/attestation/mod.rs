//! Attestation verification module.
//!
//! Provides endpoint-based attestation verification, SQLite-backed attestation
//! result caching with TTL, and task dispatch.
//!
//! Per D-10: AttestationStatus crosses the UniFFI boundary.
//! Per D-12: Raw attestation blobs stay in ActorState/SQLite -- never in AppState.

pub mod cache;
pub mod endpoint;
pub mod error;
pub mod policy;
pub mod task;

pub use error::AttestationError;
pub use policy::{SnpPolicy, TdxPolicy, TeePolicy};
pub use task::spawn_attestation_task;

// ── Public types ─────────────────────────────────────────────────────────────

/// Attestation verification status for a backend.
///
/// Crosses the UniFFI boundary per D-10. Carried in AppState as
/// Vec<(backend_id, AttestationStatus)> per D-11.
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum AttestationStatus {
    /// Cryptographic verification passed (TDX quote, SNP report, or NRAS JWT verified).
    Verified,
    /// Not yet checked or no attestation endpoint available for this backend.
    Unverified,
    /// Verification was attempted and failed.
    Failed { reason: String },
    /// Was verified but TTL has elapsed; re-verification pending.
    Expired,
}

/// Internal attestation record stored in SQLite per D-07.
///
/// Not UniFFI-exported -- stays in ActorState/SQLite per D-12.
#[derive(Clone, Debug)]
pub struct AttestationRecord {
    /// Backend identifier (e.g. "tinfoil").
    pub backend_id: String,
    /// Serialized TEE type string (e.g. "IntelTdx", "NvidiaH100Cc").
    pub tee_type: String,
    /// Verification status at cache time.
    pub status: AttestationStatus,
    /// Raw attestation report blob (TDX quote bytes or NRAS JWT bytes).
    pub report_blob: Vec<u8>,
    /// Unix timestamp when verification completed.
    pub verified_at: u64,
    /// Unix timestamp when this cached result expires (per D-08).
    pub expires_at: u64,
}

/// Internal event sent from attestation tasks back to the actor loop.
///
/// Mirrors the InternalEvent pattern from llm::streaming. Not UniFFI-exported.
#[derive(Debug)]
pub enum AttestationEvent {
    /// Verification succeeded (TDX quote, SNP report, or NRAS JWT verified).
    Verified {
        backend_id: String,
        tee_type: String,
        report_blob: Vec<u8>,
        expires_at: u64,
        /// SHA-256 of the attested TLS leaf public key (SPKI DER).
        /// Used to opportunistically pin request transport to the attested endpoint.
        tls_public_key_fp: Option<String>,
        /// For AMD SEV-SNP backends: the VCEK URL used to fetch the certificate and
        /// the raw DER bytes of the newly-fetched VCEK certificate. `None` if the
        /// VCEK was served from the in-memory cache (no new bytes to persist).
        /// The actor thread writes these to the vcek_cert_cache SQLite table.
        vcek_url: Option<String>,
        vcek_der: Option<Vec<u8>>,
    },
    /// Verification attempted and failed.
    Failed {
        backend_id: String,
        reason: String,
        /// `true` when the failure is transient (network error, rate-limit, DNS failure,
        /// collateral fetch error) — i.e. verification was never attempted against the
        /// actual TEE report.  A transient failure should NOT downgrade a `Verified`
        /// status; the backend may be reachable on the next retry.
        ///
        /// `false` for genuine cryptographic failures (`QuoteVerification`,
        /// `NonceMismatch`, `JwtVerification`) where the TEE report was parsed and
        /// found to be invalid.  Those must downgrade status regardless of prior value.
        is_transient: bool,
    },
}
