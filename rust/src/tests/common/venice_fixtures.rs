//! Shared fixtures for Venice provider tests. Loaded from rust/tests/fixtures/venice/.

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
