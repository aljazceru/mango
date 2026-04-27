//! Unit tests for NVIDIA CC attestation JWT verification.
//! Covers ATST-03.

use crate::attestation::error::AttestationError;
use crate::attestation::nvidia::{verify_nvidia_jwt, NvidiaAttestationClaims, OverallResult};

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

    assert_eq!(claims.iss, "https://nras.attestation.nvidia.com");
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

#[test]
fn test_verify_nvidia_jwt_invalid_token() {
    // An obviously malformed JWT should fail with JwtVerification error
    // before any key material is touched.
    let dummy_jwk = serde_json::json!({"kty":"EC","crv":"P-384","x":"AA","y":"AA"});
    let result = verify_nvidia_jwt("not.a.jwt", "abc123", &dummy_jwk);
    assert!(
        matches!(result, Err(AttestationError::JwtVerification { .. })),
        "invalid JWT should return JwtVerification error, got: {:?}",
        result
    );
}

#[test]
fn test_verify_nvidia_jwt_invalid_jwk() {
    // A malformed JWK should fail with JwtVerification.
    let bad_jwk = serde_json::json!({"kty":"???","totally":"invalid"});
    let result = verify_nvidia_jwt("header.payload.signature", "abc123", &bad_jwk);
    assert!(
        matches!(result, Err(AttestationError::JwtVerification { .. })),
        "bad JWK should return JwtVerification error"
    );
}
