//! Venice provider transport unit tests.
//! All tests `#[ignore]`-gated until Plan 03/04 implement `crate::llm::venice::*`.

#![allow(unused_imports)]

use crate::tests::common::venice_fixtures::*;

#[test]
#[ignore = "RED — Plan 04 (VEN-01) backend preset"]
fn venice_preset_present() {
    // Will assert: crate::llm::backend::known_provider_presets() contains id == "venice-ai"
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 03 (VEN-02) attestation URL builder"]
fn attestation_url_format() {
    // Will assert: crate::llm::venice::format_attestation_url("e2ee-...", "abc...")
    //   produces "...?model=e2ee-...&nonce=abc..."
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (VEN-05) NRAS payload double-parse"]
fn nvidia_payload_double_parse() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 03 (VEN-07a) ECDH/AES round-trip"]
fn ecdh_aes_round_trip() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 03 (VEN-07b) envelope round-trip"]
fn envelope_round_trip() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 03 (VEN-08) request body shape"]
fn request_body_shape() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 04 (VEN-09) backend summary"]
fn backend_summary_after_add() {
    panic!("not yet implemented");
}
