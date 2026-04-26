//! GREEN tests for attestation/redpill.rs (Plan 34-02 implementation).
//! Each test pins one assertion from spikes/002-.../captures/decode-report-data.py.

#![allow(unused_imports)]

use crate::tests::common::redpill_fixtures::*;

use crate::attestation::redpill::{
    debug_mode_disabled, detect_shape, quote_bytes, RedpillError, RedpillShape,
    TD_ATTRIBUTES_OFFSET,
};

// ----- Shape dispatcher (RED-03) -----

#[test]
fn dispatch_shape_a_flat() {
    let v = shape_a_value();
    assert_eq!(detect_shape(&v).unwrap(), RedpillShape::Flat);
}

#[test]
fn dispatch_shape_b_orchestrated() {
    let v = shape_b_value();
    let shape = detect_shape(&v).unwrap();
    assert!(matches!(shape, RedpillShape::Orchestrated { .. }));
}

#[test]
fn dispatch_shape_c_chutes() {
    let v = shape_c_value();
    assert_eq!(detect_shape(&v).unwrap(), RedpillShape::Chutes);
}

#[test]
fn dispatch_unknown_shape_fails_closed() {
    let v = serde_json::json!({"unrelated": true});
    assert!(matches!(detect_shape(&v), Err(RedpillError::UnknownShape)));
}

// ----- quote_bytes() helper (auto-detect base64 vs hex) — RED-04 -----

#[test]
fn quote_bytes_hex_round_trip() {
    let hex_quote = shape_a_value()["intel_quote"]
        .as_str()
        .unwrap()
        .to_string();
    let bytes = quote_bytes(&hex_quote).unwrap();
    assert!(bytes.len() >= 1000, "TDX v4 quote ≈ 5006 bytes");
    assert_eq!(
        &bytes[..8],
        &[0x04, 0x00, 0x02, 0x00, 0x81, 0x00, 0x00, 0x00],
        "TDX v4 header"
    );
}

#[test]
fn quote_bytes_base64_round_trip() {
    let b64_quote = shape_c_value()["all_attestations"][0]["intel_quote"]
        .as_str()
        .unwrap()
        .to_string();
    let bytes = quote_bytes(&b64_quote).unwrap();
    assert!(bytes.len() >= 1000);
    assert_eq!(
        &bytes[..8],
        &[0x04, 0x00, 0x02, 0x00, 0x81, 0x00, 0x00, 0x00]
    );
}

#[test]
fn quote_bytes_strips_0x_prefix() {
    let prefixed = format!("0x{}", shape_a_value()["intel_quote"].as_str().unwrap());
    let bytes = quote_bytes(&prefixed).unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(
        &bytes[..8],
        &[0x04, 0x00, 0x02, 0x00, 0x81, 0x00, 0x00, 0x00]
    );
}

// ----- Debug-mode gate — RED-08 -----

#[test]
fn debug_bit_clear_in_all_captures() {
    // Shape A
    let a = quote_bytes(shape_a_value()["intel_quote"].as_str().unwrap()).unwrap();
    assert!(
        debug_mode_disabled(&a),
        "Shape A capture must have debug bit clear"
    );
    // Shape B model
    let b_model = quote_bytes(
        shape_b_value()["model_attestations"][0]["intel_quote"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(
        debug_mode_disabled(&b_model),
        "Shape B model capture debug bit clear"
    );
    // Shape C all attestations
    for entry in shape_c_value()["all_attestations"].as_array().unwrap() {
        let q = quote_bytes(entry["intel_quote"].as_str().unwrap()).unwrap();
        assert!(debug_mode_disabled(&q), "Shape C entry debug bit clear");
    }
}

#[test]
fn debug_bit_set_rejected() {
    let mut q = quote_bytes(shape_a_value()["intel_quote"].as_str().unwrap()).unwrap();
    q[TD_ATTRIBUTES_OFFSET] |= 0x01;
    assert!(
        !debug_mode_disabled(&q),
        "synthetic debug-bit-set quote must be rejected"
    );
}
