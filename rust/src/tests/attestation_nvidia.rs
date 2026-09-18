//! Unit tests for NVIDIA CC attestation JWT verification.
//! Covers ATST-03.

use crate::attestation::nvidia::{NvidiaAttestationClaims, OverallResult};

#[test]
fn test_nvidia_claims_deserialize() {
    // Sample NRAS JWT claims payload
    let json = r#"{
        "iss": "https://nras.attestation.nvidia.com",
        "eat_nonce": "abc123",
        "x-nvidia-overall-att-result": "true"
    }"#;

    let claims: NvidiaAttestationClaims =
        serde_json::from_str(json).expect("should deserialize claims");

    assert_eq!(claims.eat_nonce, "abc123");
    assert!(
        matches!(&claims.nvidia_overall_att_result, OverallResult::Str(s) if s == "true"),
        "expected Str(\"true\"), got {:?}",
        claims.nvidia_overall_att_result
    );
}

#[test]
fn test_nvidia_claims_accepts_bool_overall_result() {
    let json = r#"{
        "iss": "https://nras.attestation.nvidia.com",
        "eat_nonce": "abc123",
        "x-nvidia-overall-att-result": true
    }"#;
    let claims: NvidiaAttestationClaims = serde_json::from_str(json).unwrap();
    assert!(matches!(
        claims.nvidia_overall_att_result,
        OverallResult::Bool(true)
    ));
}
