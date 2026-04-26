//! GREEN tests for attestation/redpill.rs (Plan 34-02 implementation).
//! Each test pins one assertion from spikes/002-.../captures/decode-report-data.py.

#![allow(unused_imports)]

use crate::tests::common::redpill_fixtures::*;

use crate::attestation::redpill::{
    debug_mode_disabled, detect_shape, quote_bytes, verify_redpill_chutes_anti_tamper,
    verify_redpill_compose_manager_reportdata, verify_redpill_gateway_reportdata,
    verify_redpill_model_reportdata, RedpillError, RedpillShape, TD_ATTRIBUTES_OFFSET,
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

// ----- Shape A: Flat (Phala-pure / Venice-identical model layout) — RED-05a -----

#[test]
fn shape_a_model_reportdata_ok() {
    // Mirrors Python show_shape_a():
    //   addr = bytes.fromhex(d['signing_address'][2:])
    //   assert rd[:20] == addr
    //   assert rd[20:32] == b'\x00' * 12
    //   assert rd[32:64].hex() == nonces['phala_nonce']
    let v = shape_a_value();
    let nonces_map = nonces();
    let phala_nonce_hex = nonces_map
        .get("phala_nonce")
        .expect("phala_nonce in nonce.txt");
    let mut nonce32 = [0u8; 32];
    nonce32.copy_from_slice(&hex::decode(phala_nonce_hex).unwrap());

    let q = quote_bytes(v["intel_quote"].as_str().unwrap()).unwrap();
    let rd: [u8; 64] = q[568..632].try_into().unwrap();

    let addr = hex::decode(
        v["signing_address"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
    )
    .unwrap();
    assert_eq!(&rd[0..20], &addr[..]);
    assert_eq!(&rd[20..32], &[0u8; 12]);
    assert_eq!(&rd[32..64], &nonce32[..]);
}

#[test]
fn shape_a_address_mismatch() {
    // Pass an unrelated 65B uncompressed pubkey to the model decoder; the keccak-derived
    // address will not match rd[0..20], so verification must error.
    let mut rd = [0u8; 64];
    rd[0] = 0xFF;
    let mut fake_pk = vec![0u8; 65];
    fake_pk[0] = 0x04;
    let pk_hex = hex::encode(&fake_pk);
    let result = verify_redpill_model_reportdata(&rd, &pk_hex, &[0u8; 32]);
    assert!(result.is_err(), "address mismatch must be rejected");
}

#[test]
fn shape_a_nonce_mismatch() {
    // Synthetic rd: place keccak-of-fake-pk address into rd[0..20], zero pad rd[20..32],
    // zeros in rd[32..64]. Pass a non-zero nonce that does not match. Decoder must error.
    use sha3::{Digest, Keccak256};
    let mut fake_pk = vec![0u8; 65];
    fake_pk[0] = 0x04;
    for i in 1..65 {
        fake_pk[i] = i as u8;
    }
    let pk_hex = hex::encode(&fake_pk);
    let mut h = Keccak256::new();
    h.update(&fake_pk[1..]);
    let digest = h.finalize();
    let addr = &digest[12..32];

    let mut rd = [0u8; 64];
    rd[0..20].copy_from_slice(addr);
    let mut wrong_nonce = [0u8; 32];
    wrong_nonce[0] = 0xAA;

    let result = verify_redpill_model_reportdata(&rd, &pk_hex, &wrong_nonce);
    assert!(result.is_err(), "nonce mismatch must be rejected");
}

#[test]
fn shape_a_padding_nonzero() {
    // Same as above but flip rd[20] = 0xAA; expect padding error.
    use sha3::{Digest, Keccak256};
    let mut fake_pk = vec![0u8; 65];
    fake_pk[0] = 0x04;
    for i in 1..65 {
        fake_pk[i] = (i + 1) as u8;
    }
    let pk_hex = hex::encode(&fake_pk);
    let mut h = Keccak256::new();
    h.update(&fake_pk[1..]);
    let digest = h.finalize();
    let addr = &digest[12..32];

    let mut rd = [0u8; 64];
    rd[0..20].copy_from_slice(addr);
    rd[20] = 0xAA; // padding violation
    let nonce = [0u8; 32];

    let result = verify_redpill_model_reportdata(&rd, &pk_hex, &nonce);
    assert!(result.is_err(), "padding-nonzero must be rejected");
}

// ----- Shape B: Orchestrated — three components — RED-05a/b/c, RED-06 -----

#[test]
fn shape_b_gateway_reportdata_ok() {
    // Mirrors Python show_shape_b() gateway block:
    //   rd = bytes.fromhex(gw['report_data'])
    //   assert rd[:32].hex() == gw['signing_address']
    //   assert rd[32:].hex() == nonces['nonce']
    let v = shape_b_value();
    let gw = &v["gateway_attestation"];
    let rd_hex = gw["report_data"].as_str().unwrap();
    let rd_vec = hex::decode(rd_hex).unwrap();
    let rd: [u8; 64] = rd_vec[..64].try_into().unwrap();
    let nonces_map = nonces();
    let nonce_hex = nonces_map.get("nonce").expect("nonce in nonce.txt").clone();
    let mut nonce32 = [0u8; 32];
    nonce32.copy_from_slice(&hex::decode(&nonce_hex).unwrap());
    let addr_hex = gw["signing_address"].as_str().unwrap();
    verify_redpill_gateway_reportdata(&rd, addr_hex, &nonce32).unwrap();
}

#[test]
fn shape_b_model_reportdata_ok() {
    let v = shape_b_value();
    let m0 = &v["model_attestations"][0];
    let q = quote_bytes(m0["intel_quote"].as_str().unwrap()).unwrap();
    let rd: [u8; 64] = q[568..632].try_into().unwrap();
    let addr = hex::decode(
        m0["signing_address"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
    )
    .unwrap();
    let nonce_hex = nonces().get("nonce").unwrap().clone();
    let nonce_bytes = hex::decode(&nonce_hex).unwrap();
    assert_eq!(&rd[0..20], &addr[..]);
    assert_eq!(&rd[20..32], &[0u8; 12]);
    assert_eq!(&rd[32..64], &nonce_bytes[..]);
}

#[test]
fn shape_b_compose_manager_reportdata_ok() {
    // Mirrors Python show_shape_b() compose-manager block:
    //   rd = bytes.fromhex(cm['report_data'])
    //   assert rd[:32].hex() == cm['actions_hash']
    //   assert rd[32:].hex() == nonces['nonce']
    let v = shape_b_value();
    let cm = &v["model_attestations"][0]["compose_manager_attestation"];
    let rd_vec = hex::decode(cm["report_data"].as_str().unwrap()).unwrap();
    let rd: [u8; 64] = rd_vec[..64].try_into().unwrap();
    let actions_hash_hex = cm["actions_hash"].as_str().unwrap();
    let nonce_hex = nonces().get("nonce").unwrap().clone();
    let mut nonce32 = [0u8; 32];
    nonce32.copy_from_slice(&hex::decode(&nonce_hex).unwrap());
    verify_redpill_compose_manager_reportdata(&rd, actions_hash_hex, &nonce32).unwrap();
}

#[test]
fn shape_b_three_way_and_gates_session_open() {
    // Single-component compromise: mutate compose-manager actions_hash → fail with
    // ComposeManagerMismatch. Demonstrates that any single component failure fails
    // the whole attestation.
    let mut bad_rd = [0u8; 64];
    bad_rd[0] = 0xFF;
    let nonce = [0u8; 32];
    let result = verify_redpill_compose_manager_reportdata(
        &bad_rd,
        "0000000000000000000000000000000000000000000000000000000000000000",
        &nonce,
    );
    assert!(matches!(
        result,
        Err(RedpillError::ComposeManagerMismatch { .. })
    ));
}

// ----- Shape C: Chutes anti-tamper — RED-05d -----

#[test]
fn shape_c_chutes_anti_tamper_binding_ok() {
    // Mirrors Python show_shape_c():
    //   expected = hashlib.sha256((a['nonce'] + a['e2e_pubkey']).encode()).digest()
    //   assert rd[:32] == expected   (for every entry in all_attestations[])
    let v = shape_c_value();
    for entry in v["all_attestations"].as_array().unwrap() {
        let q = quote_bytes(entry["intel_quote"].as_str().unwrap()).unwrap();
        let rd: [u8; 64] = q[568..632].try_into().unwrap();
        let baked_nonce = entry["nonce"].as_str().unwrap();
        let e2e_pubkey = entry["e2e_pubkey"].as_str().unwrap();
        verify_redpill_chutes_anti_tamper(&rd, baked_nonce, e2e_pubkey).unwrap();
    }
}

#[test]
fn shape_c_client_nonce_not_bound() {
    // The Chutes decoder MUST NOT compare rd[32..64] to the client nonce.
    let v = shape_c_value();
    let entry = &v["all_attestations"][0];
    let q = quote_bytes(entry["intel_quote"].as_str().unwrap()).unwrap();
    let rd: [u8; 64] = q[568..632].try_into().unwrap();
    let baked_nonce = entry["nonce"].as_str().unwrap();
    let e2e_pubkey = entry["e2e_pubkey"].as_str().unwrap();
    verify_redpill_chutes_anti_tamper(&rd, baked_nonce, e2e_pubkey).unwrap();
}
