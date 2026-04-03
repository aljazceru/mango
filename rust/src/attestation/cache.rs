//! SQLite attestation result cache with TTL-based expiry.
//!
//! Per D-07: minimal attestation cache table for Phase 3.
//! Per D-08: TTL is 4 hours for TDX/DCAP, 1 hour for NVIDIA CC JWT.
//! Per D-09: cache key is (backend_id, tee_type); re-verification replaces entry.
//!
//! IMPORTANT: `rusqlite::Connection` is NOT Send+Sync. Per Pitfall 6 in RESEARCH.md,
//! this struct must only be used from the actor thread (never moved into Tokio tasks).
//!
//! Phase 4 change: AttestationCache now borrows a &Connection from the shared
//! Database instead of owning its own Connection. The attestation_cache table is
//! created by migration v1 in Database::open -- no DDL here.

use rusqlite::Connection;

use super::error::AttestationError;
use super::{AttestationRecord, AttestationStatus};

/// SQLite-backed attestation result cache.
///
/// Borrows a `&Connection` from the shared `persistence::Database`.
/// All reads and writes happen synchronously on the actor thread.
/// Never move this into an async task -- see Pitfall 6 in RESEARCH.md.
///
/// The `attestation_cache` table is guaranteed to exist because
/// `Database::open` runs migration v1 before returning.
pub struct AttestationCache<'a> {
    conn: &'a Connection,
}

#[allow(dead_code)]
impl<'a> AttestationCache<'a> {
    /// Create an AttestationCache that borrows a Connection from Database.
    ///
    /// The `attestation_cache` table must already exist (created by migration v1).
    /// This constructor is infallible -- no I/O occurs at creation time.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Read a cached attestation record for `(backend_id, tee_type)`.
    ///
    /// Returns `None` if:
    /// - No entry exists for this key, or
    /// - The cached entry has expired (`expires_at <= now`)
    ///
    /// Per D-09: only the non-expired entry is returned.
    pub fn get(
        &self,
        backend_id: &str,
        tee_type: &str,
    ) -> Result<Option<AttestationRecord>, AttestationError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT status, report_blob, verified_at, expires_at
                 FROM attestation_cache
                 WHERE backend_id = ?1 AND tee_type = ?2 AND expires_at > ?3",
            )
            .map_err(|e| AttestationError::CacheFailed {
                reason: e.to_string(),
            })?;

        let result = stmt.query_row(rusqlite::params![backend_id, tee_type, now as i64], |row| {
            let status_str: String = row.get(0)?;
            let report_blob: Vec<u8> = row.get(1)?;
            let verified_at: i64 = row.get(2)?;
            let expires_at: i64 = row.get(3)?;
            Ok(AttestationRecord {
                backend_id: backend_id.to_string(),
                tee_type: tee_type.to_string(),
                status: deserialize_status(&status_str),
                report_blob,
                verified_at: verified_at as u64,
                expires_at: expires_at as u64,
            })
        });

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AttestationError::CacheFailed {
                reason: e.to_string(),
            }),
        }
    }

    /// Read the most recent non-expired cached record for a backend, regardless of tee_type.
    ///
    /// Used at startup to pre-populate AppState.attestation_statuses from the cache.
    /// The tee_type stored in the cache may differ from the BackendConfig.tee_type
    /// (e.g., IntelTdx backend that returns AmdSevSnp from the attestation endpoint),
    /// so we query by backend_id only and take the entry with the latest verified_at.
    pub fn get_latest_for_backend(
        &self,
        backend_id: &str,
    ) -> Result<Option<AttestationRecord>, AttestationError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT tee_type, status, report_blob, verified_at, expires_at
                 FROM attestation_cache
                 WHERE backend_id = ?1 AND expires_at > ?2
                 ORDER BY verified_at DESC
                 LIMIT 1",
            )
            .map_err(|e| AttestationError::CacheFailed {
                reason: e.to_string(),
            })?;

        let result = stmt.query_row(rusqlite::params![backend_id, now as i64], |row| {
            let tee_type: String = row.get(0)?;
            let status_str: String = row.get(1)?;
            let report_blob: Vec<u8> = row.get(2)?;
            let verified_at: i64 = row.get(3)?;
            let expires_at: i64 = row.get(4)?;
            Ok(AttestationRecord {
                backend_id: backend_id.to_string(),
                tee_type,
                status: deserialize_status(&status_str),
                report_blob,
                verified_at: verified_at as u64,
                expires_at: expires_at as u64,
            })
        });

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AttestationError::CacheFailed {
                reason: e.to_string(),
            }),
        }
    }

    /// Write (upsert) an attestation record.
    ///
    /// Per D-09: `INSERT OR REPLACE` replaces any existing entry for the same
    /// `(backend_id, tee_type)` key.
    pub fn put(&self, record: &AttestationRecord) -> Result<(), AttestationError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO attestation_cache
                 (backend_id, tee_type, status, report_blob, verified_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    record.backend_id,
                    record.tee_type,
                    serialize_status(&record.status),
                    record.report_blob,
                    record.verified_at as i64,
                    record.expires_at as i64,
                ],
            )
            .map_err(|e| AttestationError::CacheFailed {
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// Retrieve the raw attestation report blob for a given backend and TEE type.
    ///
    /// Per D-12: raw report is stored in SQLite and exposed via a dedicated FFI
    /// method -- not in AppState. Returns `None` if no cached entry exists
    /// (regardless of TTL -- the raw report may be useful even after expiry).
    pub fn get_raw_report(
        &self,
        backend_id: &str,
        tee_type: &str,
    ) -> Result<Option<Vec<u8>>, AttestationError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT report_blob FROM attestation_cache
                 WHERE backend_id = ?1 AND tee_type = ?2",
            )
            .map_err(|e| AttestationError::CacheFailed {
                reason: e.to_string(),
            })?;

        let result = stmt.query_row(rusqlite::params![backend_id, tee_type], |row| row.get(0));

        match result {
            Ok(blob) => Ok(Some(blob)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AttestationError::CacheFailed {
                reason: e.to_string(),
            }),
        }
    }
}

// ── Status serialization helpers ─────────────────────────────────────────────

/// Serialize AttestationStatus to a TEXT value for SQLite storage.
fn serialize_status(s: &AttestationStatus) -> String {
    match s {
        AttestationStatus::Verified => "verified".to_string(),
        AttestationStatus::Unverified => "unverified".to_string(),
        AttestationStatus::Failed { reason } => format!("failed:{}", reason),
        AttestationStatus::Expired => "expired".to_string(),
    }
}

/// Deserialize AttestationStatus from a SQLite TEXT value.
///
/// Backward compat: "provider_verified" rows from existing SQLite databases
/// are mapped to `Verified` (the distinction no longer exists).
#[allow(dead_code)]
fn deserialize_status(s: &str) -> AttestationStatus {
    match s {
        "verified" | "provider_verified" => AttestationStatus::Verified,
        "unverified" => AttestationStatus::Unverified,
        "expired" => AttestationStatus::Expired,
        other if other.starts_with("failed:") => AttestationStatus::Failed {
            reason: other["failed:".len()..].to_string(),
        },
        other => AttestationStatus::Failed {
            reason: format!("unknown status: {}", other),
        },
    }
}
