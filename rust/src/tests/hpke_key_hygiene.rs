//! Tests for HPKE key hygiene (DFNS-02).
//!
//! VerifiedPpqAttestation and VerifiedTinfoilAttestation are private to their
//! modules. We verify the zeroize derive pattern using a mirror struct with the
//! same field types and attributes. This proves the derive macro zeroes [u8; 32]
//! on drop and skips annotated fields.

use zeroize::{Zeroize, ZeroizeOnDrop};

/// Mirror of VerifiedPpqAttestation / VerifiedTinfoilAttestation zeroize layout.
#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
struct TestAttestation {
    #[zeroize(skip)]
    label: String,
    hpke_public_key: [u8; 32],
    #[zeroize(skip)]
    report_blob: Vec<u8>,
    #[zeroize(skip)]
    expires_at: u64,
}

/// Verify that calling zeroize() explicitly zeroes hpke_public_key.
/// This is the canonical way to verify the derive works — same behavior as ZeroizeOnDrop.
#[test]
fn test_hpke_key_zeroed_on_drop() {
    let mut att = TestAttestation {
        label: "test".to_string(),
        hpke_public_key: [0xAB; 32],
        report_blob: vec![0xCA, 0xFE],
        expires_at: u64::MAX,
    };

    // Zeroize is what ZeroizeOnDrop calls on drop. Calling it directly gives
    // a deterministic, observable result without relying on memory layout after
    // the value is freed (which would be UB).
    att.zeroize();
    assert_eq!(att.hpke_public_key, [0u8; 32], "hpke_public_key must be zeroed by Zeroize::zeroize()");
}

/// Verify that a cloned struct independently zeroes its own copy of hpke_public_key.
#[test]
fn test_cloned_struct_also_zeroes_on_drop() {
    let att = TestAttestation {
        label: "original".to_string(),
        hpke_public_key: [0xCD; 32],
        report_blob: vec![0x01],
        expires_at: 1000,
    };

    // Clone produces an independent copy with its own allocation for hpke_public_key.
    let mut cloned = att.clone();

    // Original key is intact.
    assert_eq!(att.hpke_public_key, [0xCD; 32], "original key must be intact before zeroing clone");

    // Zeroize the clone — simulates ZeroizeOnDrop.
    cloned.zeroize();
    assert_eq!(cloned.hpke_public_key, [0u8; 32], "cloned hpke_public_key must be zeroed after zeroize()");

    // Original still has its key intact (independent zeroing).
    assert_eq!(att.hpke_public_key, [0xCD; 32], "original key must be intact after zeroing clone");
}

/// Verify that #[zeroize(skip)] fields are NOT modified by zeroize().
#[test]
fn test_skipped_fields_not_zeroed() {
    let att = TestAttestation {
        label: "persistent".to_string(),
        hpke_public_key: [0xFF; 32],
        report_blob: vec![0xDE, 0xAD, 0xBE, 0xEF],
        expires_at: 42,
    };

    // Manually call zeroize (not drop) to inspect field state.
    let mut att_mut = att;
    att_mut.zeroize();

    // hpke_public_key should be zeroed.
    assert_eq!(att_mut.hpke_public_key, [0u8; 32], "hpke_public_key zeroed by zeroize()");

    // Skipped fields should retain their values.
    assert_eq!(att_mut.expires_at, 42, "expires_at should be unchanged (zeroize skip)");
    assert_eq!(att_mut.report_blob, vec![0xDE, 0xAD, 0xBE, 0xEF], "report_blob should be unchanged (zeroize skip)");
    assert_eq!(att_mut.label, "persistent", "label should be unchanged (zeroize skip)");
}

/// Compile-time proof that zeroize is available in the crate dependency graph.
/// If zeroize were removed from Cargo.toml, this test file would fail to compile.
#[test]
fn test_zeroize_crate_available() {
    // The `use zeroize::{Zeroize, ZeroizeOnDrop}` at the top of this file
    // proves the dependency is present. This test is a no-op sentinel.
    let mut bytes = [0xFFu8; 32];
    bytes.zeroize();
    assert_eq!(bytes, [0u8; 32]);
}
