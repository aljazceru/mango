//! Live integration tests against api.redpill.ai. No API key required (endpoint is public).
//! Run: cargo test -p mango_core --lib live_redpill -- --ignored --nocapture

#![allow(unused_imports)]

#[tokio::test]
#[ignore = "live — Shape A (Phala-pure) attestation against openai/gpt-oss-20b"]
async fn live_shape_a_phala_pure() {
    // Will: build BackendConfig for redpill, call ensure_verified_redpill_attestation,
    // assert AttestationStatus::Verified { freshness: PerRequest }.
    panic!("not yet implemented");
}

#[tokio::test]
#[ignore = "live — Shape B (Orchestrated) three-way AND against phala/gpt-oss-120b"]
async fn live_shape_b_orchestrated_three_way_and() {
    // Will: assert all three components verified (gateway + model + compose-manager).
    panic!("not yet implemented");
}

#[tokio::test]
#[ignore = "live — Shape C (Chutes) per-enclave freshness against deepseek/deepseek-v3.2 (RED-09)"]
async fn live_shape_c_chutes_per_enclave_freshness() {
    // Will: assert AttestationStatus::Verified { freshness: PerEnclave } and the
    // trust-UI string mentions enclave-lifetime freshness.
    panic!("not yet implemented");
}
