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
pub mod nonce;
pub mod nvidia;
pub mod policy;
pub mod redpill;
pub mod task;
pub mod tdx;
pub mod venice;

pub use error::AttestationError;
pub use policy::{SnpPolicy, TdxPolicy, TeePolicy};
pub use task::spawn_attestation_task;

// ── Public types ─────────────────────────────────────────────────────────────

/// One verified component within an Orchestrated (3-quote) Redpill response.
/// Label is one of "gateway" | "model" | "compose_manager".
/// Value is the hex address or actions hash from the verified component.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct OrchestratedComponent {
    pub label: String,
    pub value: String,
}

/// Attestation verification status for a backend.
///
/// Crosses the UniFFI boundary per D-10. Carried in AppState as
/// Vec<(backend_id, AttestationStatus)> per D-11.
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum AttestationStatus {
    /// Cryptographic verification passed (TDX quote, SNP report, or NRAS JWT verified).
    ///
    /// For Redpill aggregator backends, carries the per-shape diagnostics:
    /// - `shape`: "Flat" | "Orchestrated" | "Chutes" (Phase 34 RED-11).
    /// - `freshness`: "PerRequest" | "PerEnclave" (RED-09).
    /// - `orchestrated_components`: per-component hex/actions hashes for Shape B.
    /// All three are `None` for non-aggregator providers.
    Verified {
        shape: Option<String>,
        freshness: Option<String>,
        orchestrated_components: Option<Vec<OrchestratedComponent>>,
    },
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
    /// "Flat" | "Orchestrated" | "Chutes" — Redpill aggregator shape; None for other backends.
    pub shape: Option<String>,
    /// "PerRequest" | "PerEnclave" — freshness semantic; None for backends that don't carry one.
    pub freshness: Option<String>,
    /// Per-component breakdown for Orchestrated shapes: Vec<(label, value)>; None otherwise.
    pub orchestrated_components: Option<Vec<(String, String)>>,
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
        /// Attestation response shape for aggregator-style providers (Phase 34
        /// Redpill: "Flat" | "Orchestrated" | "Chutes"). `None` for single-shape
        /// providers (Tinfoil, PPQ, Venice). RED-11 — surfaced in the badge sub-line.
        shape: Option<String>,
        /// Freshness semantics for the verified attestation. `"PerRequest"` for
        /// shapes that bind a per-request client nonce (Tinfoil/PPQ/Venice/Redpill A+B);
        /// `"PerEnclave"` for enclave-baked nonce (Redpill Shape C / Chutes). RED-09 —
        /// drives the trust-UI sub-line (e.g. "Verified for this enclave instance").
        freshness: Option<String>,
        /// Per-component breakdown for Orchestrated shapes (Redpill Shape B):
        /// `Vec<(label, value)>` where label is one of "gateway" | "model" | "compose_manager"
        /// and value is the verified hex address / actions hash. `None` for non-Orchestrated.
        /// RED-11 — drives the three-way breakdown ("gateway ✓ • model ✓ • compose ✓").
        orchestrated_components: Option<Vec<(String, String)>>,
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

/// Map an [`AttestationEvent`] into the actor-loop carrier tuple.
///
/// Threads `shape`, `freshness`, and `orchestrated_components` from
/// [`AttestationEvent::Verified`] through to the persisted [`AttestationRecord`].
/// (Closes RED-09 / RED-11 actor-loop drop — Phase 34.1.)
///
/// Return shape mirrors the inline destructure that previously lived at
/// `lib.rs:7411-7464`:
/// - `String` — backend_id
/// - [`AttestationStatus`] — status (still unit `Verified` for now; struct-variant promotion is 34.1-02)
/// - `Option<(AttestationRecord, report_blob, tls_public_key_fp, vcek_url, vcek_der)>` — `Some` on Verified, `None` on Failed
/// - `bool` — `failed_is_transient`
pub fn map_event_to_record_and_status(
    event: AttestationEvent,
    now_secs: u64,
) -> (
    String,
    AttestationStatus,
    Option<(
        AttestationRecord,
        Vec<u8>,
        Option<String>,
        Option<String>,
        Option<Vec<u8>>,
    )>,
    bool,
) {
    match event {
        AttestationEvent::Verified {
            backend_id,
            tee_type,
            report_blob,
            expires_at,
            tls_public_key_fp,
            vcek_url,
            vcek_der,
            shape,
            freshness,
            orchestrated_components,
        } => {
            let status_uniffi = AttestationStatus::Verified {
                shape: shape.clone(),
                freshness: freshness.clone(),
                orchestrated_components: orchestrated_components.as_ref().map(|v| {
                    v.iter()
                        .map(|(label, value)| OrchestratedComponent {
                            label: label.clone(),
                            value: value.clone(),
                        })
                        .collect()
                }),
            };
            let record = AttestationRecord {
                backend_id: backend_id.clone(),
                tee_type,
                status: status_uniffi.clone(),
                report_blob: report_blob.clone(),
                verified_at: now_secs,
                expires_at,
                shape: shape.clone(),
                freshness: freshness.clone(),
                orchestrated_components: orchestrated_components.clone(),
            };
            (
                backend_id,
                status_uniffi,
                Some((record, report_blob, tls_public_key_fp, vcek_url, vcek_der)),
                false,
            )
        }
        AttestationEvent::Failed {
            backend_id,
            reason,
            is_transient,
        } => (
            backend_id,
            AttestationStatus::Failed { reason },
            None,
            is_transient,
        ),
    }
}
