//! Unit tests for SQLite attestation cache with TTL.
//! Covers ATST-04.
//!
//! Phase 4 update: AttestationCache<'a> now borrows a &Connection instead of
//! owning one. Tests open a Database (which runs migration v1 and creates the
//! attestation_cache table) and borrow its connection.

use crate::attestation::cache::AttestationCache;
use crate::attestation::{AttestationRecord, AttestationStatus};
use crate::persistence::Database;

fn make_record(backend_id: &str, tee_type: &str, expires_at: u64) -> AttestationRecord {
    AttestationRecord {
        backend_id: backend_id.to_string(),
        tee_type: tee_type.to_string(),
        status: AttestationStatus::Verified,
        report_blob: vec![0xCA, 0xFE, 0xBA, 0xBE],
        verified_at: 1_700_000_000,
        expires_at,
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn test_cache_create() {
    // Creating an in-memory cache (via Database) should succeed and not panic
    let db = Database::open(":memory:").expect("in-memory database should open");
    let _cache = AttestationCache::new(db.conn());
}

#[test]
fn test_cache_write_and_read() {
    let db = Database::open(":memory:").unwrap();
    let cache = AttestationCache::new(db.conn());
    let now = now_secs();
    let expires_at = now + 3600; // 1 hour in the future

    let record = make_record("tinfoil", "IntelTdx", expires_at);
    cache.put(&record).expect("put should succeed");

    let retrieved = cache
        .get("tinfoil", "IntelTdx")
        .expect("get should not error")
        .expect("record should be present before TTL");

    assert_eq!(retrieved.backend_id, "tinfoil");
    assert_eq!(retrieved.tee_type, "IntelTdx");
    assert_eq!(retrieved.status, AttestationStatus::Verified);
    assert_eq!(retrieved.report_blob, vec![0xCA, 0xFE, 0xBA, 0xBE]);
    assert_eq!(retrieved.verified_at, 1_700_000_000);
    assert_eq!(retrieved.expires_at, expires_at);
}

#[test]
fn test_cache_expiry() {
    let db = Database::open(":memory:").unwrap();
    let cache = AttestationCache::new(db.conn());
    let now = now_secs();
    // expires_at is 1 second in the past -- should be treated as expired
    let expires_at = now - 1;

    let record = make_record("test-backend", "IntelTdx", expires_at);
    cache.put(&record).expect("put should succeed");

    let result = cache
        .get("test-backend", "IntelTdx")
        .expect("get should not error");

    assert!(
        result.is_none(),
        "expired record should return None, got: {:?}",
        result
    );
}

#[test]
fn test_cache_upsert() {
    let db = Database::open(":memory:").unwrap();
    let cache = AttestationCache::new(db.conn());
    let now = now_secs();
    let expires_at = now + 3600;

    // Write initial record with Verified status
    let initial = make_record("tinfoil", "IntelTdx", expires_at);
    cache.put(&initial).expect("initial put should succeed");

    // Overwrite with Failed status
    let updated = AttestationRecord {
        backend_id: "tinfoil".to_string(),
        tee_type: "IntelTdx".to_string(),
        status: AttestationStatus::Failed {
            reason: "cert revoked".to_string(),
        },
        report_blob: vec![0xFF],
        verified_at: 1_700_001_000,
        expires_at,
    };
    cache.put(&updated).expect("upsert should succeed");

    let retrieved = cache
        .get("tinfoil", "IntelTdx")
        .expect("get should not error")
        .expect("upserted record should be present");

    assert_eq!(
        retrieved.status,
        AttestationStatus::Failed {
            reason: "cert revoked".to_string()
        }
    );
    assert_eq!(retrieved.report_blob, vec![0xFF]);
    assert_eq!(retrieved.verified_at, 1_700_001_000);
}

#[test]
fn test_get_raw_report() {
    let db = Database::open(":memory:").unwrap();
    let cache = AttestationCache::new(db.conn());
    let now = now_secs();
    let report_blob = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];

    let record = AttestationRecord {
        backend_id: "tinfoil".to_string(),
        tee_type: "IntelTdx".to_string(),
        status: AttestationStatus::Verified,
        report_blob: report_blob.clone(),
        verified_at: 1_700_000_000,
        expires_at: now + 3600,
    };
    cache.put(&record).expect("put should succeed");

    let raw = cache
        .get_raw_report("tinfoil", "IntelTdx")
        .expect("get_raw_report should not error")
        .expect("raw report should be present");

    assert_eq!(
        raw, report_blob,
        "raw report bytes should match what was stored"
    );
}

// --- TTL expiry and bypass tests (TEST-03) ---
// These verify get_latest_for_backend rejects expired entries while
// get_raw_report bypasses TTL by design (per D-06, D-07).

#[test]
fn test_get_latest_for_backend_expiry() {
    // get_latest_for_backend uses expires_at > now filter -- must return None for expired entry.
    let db = Database::open(":memory:").unwrap();
    let cache = AttestationCache::new(db.conn());
    let expired_at = now_secs() - 1; // 1 second in the past
    cache
        .put(&make_record("tinfoil", "AmdSevSnp", expired_at))
        .expect("put should succeed");
    let result = cache
        .get_latest_for_backend("tinfoil")
        .expect("get_latest_for_backend should not error");
    assert!(
        result.is_none(),
        "get_latest_for_backend must return None for expired entry"
    );
}

#[test]
fn test_get_raw_report_bypasses_ttl() {
    // get_raw_report omits the TTL filter -- returns blob even when expired (by design).
    let db = Database::open(":memory:").unwrap();
    let cache = AttestationCache::new(db.conn());
    let expired_at = now_secs() - 1; // 1 second in the past
    cache
        .put(&make_record("tinfoil", "AmdSevSnp", expired_at))
        .expect("put should succeed");
    let raw = cache
        .get_raw_report("tinfoil", "AmdSevSnp")
        .expect("get_raw_report should not error");
    assert!(
        raw.is_some(),
        "get_raw_report must return blob even when TTL is expired (by design)"
    );
    assert_eq!(
        raw.unwrap(),
        vec![0xCA, 0xFE, 0xBA, 0xBE],
        "raw report bytes must match stored value"
    );
}
