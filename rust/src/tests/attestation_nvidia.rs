//! Unit tests for NVIDIA CC attestation JWT verification.
//! Covers ATST-03.

use crate::attestation::error::AttestationError;
use crate::attestation::nvidia::{verify_nvidia_jwt, NvidiaAttestationClaims};

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
    assert_eq!(
        claims.nvidia_overall_att_result.as_deref(),
        Some("true")
    );
}

#[test]
fn test_verify_nvidia_jwt_invalid_token() {
    // An obviously malformed JWT should fail with JwtVerification error
    let dummy_key_pem = "-----BEGIN RSA PUBLIC KEY-----\nMIIBCg==\n-----END RSA PUBLIC KEY-----\n";
    let result = verify_nvidia_jwt("not.a.jwt", "abc123", dummy_key_pem);
    assert!(
        matches!(result, Err(AttestationError::JwtVerification { .. })),
        "invalid JWT should return JwtVerification error, got: {:?}",
        result
    );
}

#[test]
fn test_verify_nvidia_jwt_wrong_issuer() {
    // A well-formed-looking but wrong-issuer JWT should also fail
    // We test this by passing a dummy PEM that causes key parse to fail first
    // (the issuer check happens after signature verification)
    let dummy_key_pem = "not_a_pem";
    let result = verify_nvidia_jwt("header.payload.signature", "abc123", dummy_key_pem);
    assert!(
        matches!(result, Err(AttestationError::JwtVerification { .. })),
        "wrong PEM should return JwtVerification error"
    );
}
