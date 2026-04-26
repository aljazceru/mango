//! Live integration test against api.venice.ai.
//! Run with: VENICE_API_KEY=... cargo test -p mango_core live_venice -- --ignored

#![allow(unused_imports)]

#[tokio::test]
#[ignore = "live integration test against api.venice.ai; requires VENICE_API_KEY env (RED — Plan 04 VEN-LIVE)"]
async fn live_attestation_round_trip() {
    let _api_key = std::env::var("VENICE_API_KEY").expect("VENICE_API_KEY required");
    // Will: build BackendConfig for venice-ai, call ensure_verified_venice_attestation,
    //       assert AttestationStatus::Verified, then send a chat completion and decrypt SSE.
    panic!("not yet implemented");
}
