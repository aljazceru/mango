//! Shared fixtures for Redpill provider tests.
//! Loaded from rust/tests/fixtures/redpill/. Mirrors common/venice_fixtures.rs.

#![allow(dead_code)]

pub const SHAPE_A_FLAT_JSON: &str =
    include_str!("../../../tests/fixtures/redpill/attestation-phala-pure-raw.json");
pub const SHAPE_B_ORCHESTRATED_JSON: &str =
    include_str!("../../../tests/fixtures/redpill/attestation-phala-raw.json");
pub const SHAPE_C_CHUTES_JSON: &str =
    include_str!("../../../tests/fixtures/redpill/attestation-chutes-raw.json");
pub const SHAPE_D_TINFOIL_REFUSAL_JSON: &str =
    include_str!("../../../tests/fixtures/redpill/attestation-tinfoil-raw.json");
pub const NONCE_LOG: &str = include_str!("../../../tests/fixtures/redpill/nonce.txt");

/// Hex nonces submitted when each capture was taken (from nonce.txt).
/// Names mirror the Python decoder script's `nonces` dict.
/// Keys observed: "nonce" (Shape B), "phala_nonce" (Shape A),
/// "chutes_nonce" (Shape C), "tinfoil_nonce", "tinfoil_nonce2".
pub fn nonces() -> std::collections::HashMap<String, String> {
    NONCE_LOG
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

pub fn shape_a_value() -> serde_json::Value {
    serde_json::from_str(SHAPE_A_FLAT_JSON).expect("Shape A fixture is valid JSON")
}
pub fn shape_b_value() -> serde_json::Value {
    serde_json::from_str(SHAPE_B_ORCHESTRATED_JSON).expect("Shape B fixture is valid JSON")
}
pub fn shape_c_value() -> serde_json::Value {
    serde_json::from_str(SHAPE_C_CHUTES_JSON).expect("Shape C fixture is valid JSON")
}

/// Decode `quote_str` accepting either hex or base64 (matches Python `slice_quote_reportdata`).
/// Used by tests to slice REPORTDATA at TDX bytes [568..632].
pub fn quote_bytes_for_test(quote_str: &str) -> Vec<u8> {
    let s = quote_str.trim().trim_start_matches("0x");
    if s.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex::decode(s).expect("hex quote")
    } else {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .expect("b64 quote")
    }
}

/// REPORTDATA at TDX v4 quote bytes [568..632] (header 48B + body offset 520..584).
pub fn slice_reportdata(quote_bytes: &[u8]) -> [u8; 64] {
    let mut rd = [0u8; 64];
    rd.copy_from_slice(&quote_bytes[568..632]);
    rd
}
