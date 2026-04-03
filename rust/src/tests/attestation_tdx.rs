//! Unit tests for Intel TDX/DCAP quote decoding and verification.
//! Covers ATST-02 and ATST-06.

use crate::attestation::error::AttestationError;
use crate::attestation::nonce::generate_nonce;
use crate::attestation::tdx::decode_quote;

#[test]
fn test_decode_quote_hex() {
    // 64 known bytes encoded as hex
    let known_bytes: Vec<u8> = (0u8..64).collect();
    let hex_str = hex::encode(&known_bytes);
    let decoded = decode_quote(&hex_str).expect("should decode hex successfully");
    assert_eq!(decoded.len(), 64);
    assert_eq!(decoded, known_bytes);
}

#[test]
fn test_decode_quote_base64() {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    // 64 known bytes encoded as base64
    let known_bytes: Vec<u8> = (0u8..64).collect();
    let b64_str = STANDARD.encode(&known_bytes);
    let decoded = decode_quote(&b64_str).expect("should decode base64 successfully");
    assert_eq!(decoded.len(), 64);
    assert_eq!(decoded, known_bytes);
}

#[test]
fn test_decode_quote_garbage() {
    let result = decode_quote("not_valid!!!");
    assert!(
        matches!(result, Err(AttestationError::QuoteVerification { .. })),
        "garbage input should return QuoteVerification error"
    );
}

#[test]
fn test_decode_quote_too_short() {
    // 10 bytes is below the 48-byte minimum TDX quote header size
    let short_bytes: Vec<u8> = (0u8..10).collect();
    let hex_str = hex::encode(&short_bytes);
    let result = decode_quote(&hex_str);
    assert!(
        matches!(result, Err(AttestationError::QuoteVerification { .. })),
        "too-short quote should return QuoteVerification error, got: {:?}",
        result
    );
}

#[tokio::test]
async fn test_verify_tdx_quote_short_input() {
    use crate::attestation::tdx::verify_tdx_quote;
    // 10-byte input is below minimum -- should return error, not panic
    let short_bytes = vec![0u8; 10];
    let nonce = [0u8; 32];
    let result = verify_tdx_quote(&short_bytes, &nonce, "test-backend").await;
    assert!(
        result.is_err(),
        "short input should return an error, not succeed"
    );
}

#[test]
fn test_nonce_uniqueness() {
    // ATST-06: nonces must be 32 bytes and unique across calls
    let n1 = generate_nonce();
    let n2 = generate_nonce();
    assert_eq!(n1.len(), 32);
    assert_eq!(n2.len(), 32);
    assert_ne!(n1, n2, "consecutive nonces must differ (ATST-06)");
}
