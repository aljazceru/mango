//! Unit tests for TEE attestation policy structs, MIGRATION_V14 seeding,
//! and runtime policy loading via get_tee_policy.

use crate::attestation::{SnpPolicy, TdxPolicy};
use crate::persistence::{queries, Database};

/// MIGRATION_V14 should seed `tee_policy_tdx` with the correct minimum TCB SVN.
#[test]
fn test_migration_v14_seeds_tdx_minimum_tee_tcb_svn() {
    let db = Database::open(":memory:").unwrap();
    let json = queries::get_setting(db.conn(), "tee_policy_tdx")
        .unwrap()
        .expect("tee_policy_tdx should be seeded by MIGRATION_V14");
    let policy: TdxPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(policy.minimum_tee_tcb_svn, "03010200000000000000000000000000");
}

/// MIGRATION_V14 should seed `tee_policy_tdx` with 4 accepted MR_SEAM entries.
#[test]
fn test_migration_v14_seeds_tdx_accepted_mr_seams() {
    let db = Database::open(":memory:").unwrap();
    let json = queries::get_setting(db.conn(), "tee_policy_tdx")
        .unwrap()
        .expect("tee_policy_tdx should be seeded by MIGRATION_V14");
    let policy: TdxPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(policy.accepted_mr_seams.len(), 4);
    assert!(
        policy
            .accepted_mr_seams
            .contains(&"476a2997c62bccc78370913d0a80b956e3721b24272bc66c4d6307ced4be2865c40e26afac75f12df3425b03eb59ea7c".to_string())
    );
}

/// MIGRATION_V14 should seed `tee_policy_snp` with correct minimum values.
#[test]
fn test_migration_v14_seeds_snp_policy() {
    let db = Database::open(":memory:").unwrap();
    let json = queries::get_setting(db.conn(), "tee_policy_snp")
        .unwrap()
        .expect("tee_policy_snp should be seeded by MIGRATION_V14");
    let policy: SnpPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(policy.minimum_bootloader, 7);
    assert_eq!(policy.minimum_tee, 0);
    assert_eq!(policy.minimum_snp, 14);
    assert_eq!(policy.minimum_microcode, 72);
}

/// get_tee_policy returns TdxPolicy defaults matching MIGRATION_V14 seeded values.
#[test]
fn test_get_tee_policy_returns_correct_tdx_defaults() {
    let db = Database::open(":memory:").unwrap();
    let policy = queries::get_tee_policy(db.conn()).unwrap();
    assert_eq!(
        policy.tdx.minimum_tee_tcb_svn,
        "03010200000000000000000000000000"
    );
    assert_eq!(policy.tdx.accepted_mr_seams.len(), 4);
}

/// get_tee_policy returns SnpPolicy defaults matching MIGRATION_V14 seeded values.
#[test]
fn test_get_tee_policy_returns_correct_snp_defaults() {
    let db = Database::open(":memory:").unwrap();
    let policy = queries::get_tee_policy(db.conn()).unwrap();
    assert_eq!(policy.snp.minimum_bootloader, 7);
    assert_eq!(policy.snp.minimum_tee, 0);
    assert_eq!(policy.snp.minimum_snp, 14);
    assert_eq!(policy.snp.minimum_microcode, 72);
}

/// Modifying tee_policy_snp via set_setting and calling get_tee_policy returns updated values.
#[test]
fn test_get_tee_policy_reflects_runtime_change() {
    let db = Database::open(":memory:").unwrap();
    let updated_json = r#"{"minimum_bootloader":9,"minimum_tee":1,"minimum_snp":20,"minimum_microcode":100}"#;
    queries::set_setting(db.conn(), "tee_policy_snp", updated_json).unwrap();

    let policy = queries::get_tee_policy(db.conn()).unwrap();
    assert_eq!(policy.snp.minimum_bootloader, 9);
    assert_eq!(policy.snp.minimum_tee, 1);
    assert_eq!(policy.snp.minimum_snp, 20);
    assert_eq!(policy.snp.minimum_microcode, 100);
}

/// TdxPolicy::default() returns values matching the prior hardcoded constants.
#[test]
fn test_tdx_policy_default_matches_hardcoded_constants() {
    let policy = TdxPolicy::default();
    // The prior compile-time constant TDX_MINIMUM_TEE_TCB_SVN was
    // [0x03, 0x01, 0x02, 0x00, 0x00, ...] which hex-encodes to this string.
    assert_eq!(policy.minimum_tee_tcb_svn, "03010200000000000000000000000000");

    // Prior TDX_ACCEPTED_MR_SEAMS had exactly 4 entries.
    assert_eq!(policy.accepted_mr_seams.len(), 4);

    // Verify the bytes decode correctly.
    let bytes = policy.minimum_tee_tcb_svn_bytes().unwrap();
    assert_eq!(bytes[0], 0x03);
    assert_eq!(bytes[1], 0x01);
    assert_eq!(bytes[2], 0x02);
    assert_eq!(bytes[3], 0x00);
    // All remaining bytes should be 0.
    for &b in &bytes[4..] {
        assert_eq!(b, 0x00);
    }
}

/// SnpPolicy::default() returns values matching the prior hardcoded constants.
#[test]
fn test_snp_policy_default_matches_hardcoded_constants() {
    let policy = SnpPolicy::default();
    // Prior SNP_MINIMUM_TCB was { bootloader: 0x07, tee: 0x00, snp: 0x0e, microcode: 0x48 }
    // 0x07 = 7, 0x00 = 0, 0x0e = 14, 0x48 = 72
    assert_eq!(policy.minimum_bootloader, 7);
    assert_eq!(policy.minimum_tee, 0);
    assert_eq!(policy.minimum_snp, 14);
    assert_eq!(policy.minimum_microcode, 72);
}
