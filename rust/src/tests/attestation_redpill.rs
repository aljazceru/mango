//! RED stubs for attestation/redpill.rs (Plan 02 will implement).
//! Each test pins one assertion from spikes/002-.../captures/decode-report-data.py.

#![allow(unused_imports)]

use crate::tests::common::redpill_fixtures::*;

// ----- Shape dispatcher (RED-03 — Plan 02) -----

#[test]
#[ignore = "RED — Plan 02 (RED-03) shape dispatcher returns Flat for Shape A fixture"]
fn dispatch_shape_a_flat() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-03) shape dispatcher returns Orchestrated for Shape B fixture"]
fn dispatch_shape_b_orchestrated() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-03) shape dispatcher returns Chutes for Shape C fixture"]
fn dispatch_shape_c_chutes() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-03) shape dispatcher returns UnknownShape on garbage"]
fn dispatch_unknown_shape_fails_closed() {
    panic!("not yet implemented");
}

// ----- Shape A: Flat (Phala-pure / Venice-identical model layout) — RED-05a -----

#[test]
#[ignore = "RED — Plan 02 (RED-05a) Shape A model REPORTDATA decoder accepts golden fixture"]
fn shape_a_model_reportdata_ok() {
    // Mirrors Python show_shape_a():
    //   addr = bytes.fromhex(d['signing_address'][2:])
    //   assert rd[:20] == addr
    //   assert rd[20:32] == b'\x00' * 12
    //   assert rd[32:64].hex() == nonces['phala_nonce']
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-05a) Shape A: address mismatch rejected"]
fn shape_a_address_mismatch() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-05a) Shape A: nonce mismatch rejected"]
fn shape_a_nonce_mismatch() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-05a) Shape A: zero-pad violation rejected"]
fn shape_a_padding_nonzero() {
    panic!("not yet implemented");
}

// ----- Shape B: Orchestrated — three components — RED-05a/b/c, RED-06 -----

#[test]
#[ignore = "RED — Plan 02 (RED-05b) Shape B gateway ed25519 REPORTDATA: pubkey [0..32] + nonce [32..64]"]
fn shape_b_gateway_reportdata_ok() {
    // Mirrors Python show_shape_b() gateway block:
    //   rd = bytes.fromhex(gw['report_data'])
    //   assert rd[:32].hex() == gw['signing_address']
    //   assert rd[32:].hex() == nonces['nonce']
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-05a) Shape B model component REPORTDATA: addr+pad+nonce (Venice-identical)"]
fn shape_b_model_reportdata_ok() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-05c) Shape B compose-manager REPORTDATA: actions_hash + nonce"]
fn shape_b_compose_manager_reportdata_ok() {
    // Mirrors Python show_shape_b() compose-manager block:
    //   rd = bytes.fromhex(cm['report_data'])
    //   assert rd[:32].hex() == cm['actions_hash']
    //   assert rd[32:].hex() == nonces['nonce']
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-06) Shape B three-way AND: failure of any single component fails the whole"]
fn shape_b_three_way_and_gates_session_open() {
    // Construct a Shape B response where the model attestation is correct but
    // the compose-manager actions_hash mismatches; assert verify returns Err.
    panic!("not yet implemented");
}

// ----- Shape C: Chutes anti-tamper — RED-05d -----

#[test]
#[ignore = "RED — Plan 02 (RED-05d) Shape C: SHA256(nonce_str ++ e2e_pubkey_str) == reportData[0..32]"]
fn shape_c_chutes_anti_tamper_binding_ok() {
    // Mirrors Python show_shape_c():
    //   expected = hashlib.sha256((a['nonce'] + a['e2e_pubkey']).encode()).digest()
    //   assert rd[:32] == expected   (for every entry in all_attestations[])
    // STRING concat of as-emitted ASCII bytes.
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-05d) Shape C: client ?nonce= is NOT bound (rd[32..64] unconstrained)"]
fn shape_c_client_nonce_not_bound() {
    // Document the freshness model: rd[32..64] does not equal client_nonce on Chutes.
    panic!("not yet implemented");
}

// ----- quote_bytes() helper (auto-detect base64 vs hex) — RED-04 -----

#[test]
#[ignore = "RED — Plan 02 (RED-04) quote_bytes() round-trips hex input from Shape A/B"]
fn quote_bytes_hex_round_trip() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-04) quote_bytes() round-trips base64 input from Shape C"]
fn quote_bytes_base64_round_trip() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-04) quote_bytes() decodes 0x-prefixed hex (strip prefix)"]
fn quote_bytes_strips_0x_prefix() {
    panic!("not yet implemented");
}

// ----- Debug-mode gate — RED-08 -----

#[test]
#[ignore = "RED — Plan 02 (RED-08) all captured quotes have td_attributes[0] & 0x01 == 0 (debug bit clear)"]
fn debug_bit_clear_in_all_captures() {
    // Mirrors Python show_shape_c():
    //   td_attr = raw[48 + 120 : 48 + 128]
    //   debug = bool(td_attr[0] & 1)
    //   assert not debug   (across all_attestations[i])
    // Apply to Shape A intel_quote, Shape B model_attestations[0].intel_quote,
    // Shape B gateway (per its own quote layout if applicable), and every Shape C entry.
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (RED-08) synthetic quote with debug bit set is rejected"]
fn debug_bit_set_rejected() {
    // Take a Shape A quote, flip the debug bit at byte [48 + 120], assert verify returns
    // RedpillError::DebugMode (or AttestationError::QuoteVerification with debug-mode reason).
    panic!("not yet implemented");
}
