//! RED stubs for llm/redpill.rs + backend.rs/transport.rs wiring.
//! Plans 03 and 04 implement.

#![allow(unused_imports)]

#[test]
#[ignore = "RED — Plan 03/04 (RED-01) Redpill preset present in known_provider_presets"]
fn redpill_preset_present() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 03 (RED-02) attestation URL builder format"]
fn attestation_url_format() {
    // Will assert: format_redpill_attestation_url("openai/gpt-oss-20b", "abc123",
    //   "https://api.redpill.ai/v1") ends with
    //   "/v1/attestation/report?model=openai%2Fgpt-oss-20b&nonce=abc123"
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 03 (RED-10) Tinfoil-routed model returns RedpillError::TinfoilUnsupported"]
fn tinfoil_route_refused_with_typed_error() {
    // Use SHAPE_D_TINFOIL_REFUSAL_JSON or a constructed /v1/models entry with
    // providers: ['tinfoil']. Assert the error variant + the hint string mentions
    // direct-Tinfoil.
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 04 (RED-11) backend summary surfaces Verified badge with shape breakdown"]
fn backend_summary_after_add() {
    panic!("not yet implemented");
}
