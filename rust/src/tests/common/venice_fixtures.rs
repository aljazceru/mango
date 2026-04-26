//! Shared fixtures for Venice provider tests. Loaded from rust/tests/fixtures/venice/.
//!
//! ## MRSEAM reconcile sentinel (Phase 33 Wave 0)
//!
//! Live MRSEAM extracted from this fixture's `intel_quote` (TD10, DCAP v4):
//!   `7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d`
//! Matches index 1 of `TdxPolicy::default().accepted_mr_seams` (rust/src/attestation/policy.rs).
//! See `.planning/phases/33-.../33-MRSEAM-RECONCILE.md` for the full reconcile.
//! If the golden capture is rotated and this MRSEAM no longer matches an entry in the
//! default policy, re-run the reconcile and either extend the policy or refuse the rotation.

#![allow(dead_code)]

pub const GOLDEN_CAPTURE_JSON: &str =
    include_str!("../../../tests/fixtures/venice/attestation-sample.json");

/// Parse the golden capture into a `serde_json::Value` for ad-hoc field access in tests.
pub fn golden_capture() -> serde_json::Value {
    serde_json::from_str(GOLDEN_CAPTURE_JSON).expect("golden capture must be valid JSON")
}

/// Hex-decode the `intel_quote` field from the golden capture.
pub fn golden_intel_quote_bytes() -> Vec<u8> {
    let v = golden_capture();
    let hex_str = v["intel_quote"].as_str().expect("intel_quote present and string");
    hex::decode(hex_str).expect("intel_quote must be valid hex")
}

/// Hex-decode the `nonce` echo field as `[u8; 32]`.
pub fn golden_nonce_32() -> [u8; 32] {
    let v = golden_capture();
    let hex_str = v["nonce"].as_str().expect("nonce present");
    let bytes = hex::decode(hex_str).expect("nonce hex");
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes[..32]);
    out
}

/// Get the signing pubkey hex (handles both `signing_key` and `signing_public_key`).
pub fn golden_signing_pubkey_hex() -> String {
    let v = golden_capture();
    v.get("signing_public_key")
        .or_else(|| v.get("signing_key"))
        .and_then(|x| x.as_str())
        .expect("signing_public_key or signing_key present")
        .to_string()
}
