//! REPORTDATA decoder + Venice attestation unit tests against golden capture.
//! Plan 33-02 fills the deterministic decoder tests; the live signature test
//! and synthetic-quote debug-bit test remain `#[ignore]` and are covered by
//! Plan 33-04's live integration suite.

#![allow(unused_imports)]

use crate::attestation::error::AttestationError;
use crate::attestation::venice::verify_venice_report_data;
use crate::tests::common::venice_fixtures::*;

fn make_valid_report_data(pk_hex: &str, nonce: &[u8; 32]) -> [u8; 64] {
    use sha3::Digest;
    let pk = hex::decode(pk_hex).expect("pk hex");
    let mut h = sha3::Keccak256::new();
    h.update(&pk[1..]);
    let digest = h.finalize();
    let mut rd = [0u8; 64];
    rd[..20].copy_from_slice(&digest[12..32]);
    // [20..32] zeros (already)
    rd[32..64].copy_from_slice(nonce);
    rd
}

#[test]
fn reportdata_layout_ok() {
    let pk_hex = golden_signing_pubkey_hex();
    let nonce = golden_nonce_32();
    let rd = make_valid_report_data(&pk_hex, &nonce);
    assert!(verify_venice_report_data(&rd, &pk_hex, &nonce).is_ok());
}

#[test]
fn reportdata_address_mismatch() {
    let pk_hex = golden_signing_pubkey_hex();
    let nonce = golden_nonce_32();
    let mut rd = [0u8; 64];
    // Plant garbage in the address slot.
    rd[0] = 0xFF;
    rd[19] = 0xFF;
    // [20..32] zero (valid pad), but [32..64] still all-zero (mismatched nonce).
    // Since address is checked before nonce, we expect the QuoteVerification "not bound" error.
    let result = verify_venice_report_data(&rd, &pk_hex, &nonce);
    assert!(
        matches!(
            &result,
            Err(AttestationError::QuoteVerification { reason }) if reason.contains("not bound")
        ),
        "expected address-binding QuoteVerification error, got: {result:?}"
    );
}

#[test]
fn reportdata_nonce_mismatch() {
    let pk_hex = golden_signing_pubkey_hex();
    let nonce = golden_nonce_32();
    // Build a REPORTDATA with the correct address binding but a different nonce
    // bytes echoed in [32..64] vs the one we hand to the verifier.
    let other_nonce = [0xABu8; 32];
    let rd = make_valid_report_data(&pk_hex, &other_nonce);
    let result = verify_venice_report_data(&rd, &pk_hex, &nonce);
    assert!(
        matches!(&result, Err(AttestationError::NonceMismatch { .. })),
        "expected NonceMismatch, got: {result:?}"
    );
}

#[test]
fn reportdata_padding_nonzero() {
    let pk_hex = golden_signing_pubkey_hex();
    let nonce = golden_nonce_32();
    let mut rd = make_valid_report_data(&pk_hex, &nonce);
    // Tamper with one of the 12 padding bytes.
    rd[20] = 0xAA;
    let result = verify_venice_report_data(&rd, &pk_hex, &nonce);
    assert!(
        matches!(
            &result,
            Err(AttestationError::QuoteVerification { reason }) if reason.contains("padding non-zero")
        ),
        "expected padding QuoteVerification error, got: {result:?}"
    );
}

#[tokio::test]
#[ignore = "Synthetic TDX quote construction is heavy; debug-bit gate covered by live test in Plan 04 (VEN-06)"]
async fn tdx_debug_bit_rejected() {
    panic!(
        "deferred: see attestation/venice.rs::fetch_and_verify_venice_attestation \
        debug-bit branch — covered by Plan 04 live test (VEN-06)"
    );
}

#[tokio::test]
#[ignore = "Requires fresh Phala PCCS collateral against golden capture; covered by Plan 04 live test (VEN-03)"]
async fn tdx_verify_golden_capture_signature() {
    panic!(
        "deferred: signature/collateral verification against the golden capture requires \
        a live PCCS round-trip — covered by Plan 04 live test (VEN-03)"
    );
}
