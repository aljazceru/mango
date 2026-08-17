//! Live integration tests against api.redpill.ai. The attestation endpoint is public —
//! NO API key required for the four tests in this file. (Chat completions WOULD require
//! REDPILL_API_KEY, but these tests exercise only the attestation path.)
//!
//! Run:
//!   cargo test -p mango_core --lib live_redpill -- --ignored --nocapture
//!
//! All tests are gated by `#[ignore]` so they never run in default `cargo test`.

#![allow(unused_imports)]

use crate::attestation::policy::TdxPolicy;
use crate::attestation::redpill::{ensure_verified_redpill_attestation, Freshness, RedpillShape};
use crate::llm::backend::{BackendConfig, TeeType};

fn redpill_backend(model: &str) -> BackendConfig {
    BackendConfig {
        id: "redpill".into(),
        name: "Redpill".into(),
        base_url: "https://api.redpill.ai/v1/".into(),
        api_key: std::env::var("REDPILL_API_KEY").unwrap_or_default(),
        models: vec![model.into()],
        tee_type: TeeType::IntelTdx,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    }
}

#[tokio::test]
#[ignore = "live — Shape A (Phala-pure) attestation against openai/gpt-oss-20b"]
async fn live_shape_a_phala_pure() {
    let backend = redpill_backend("openai/gpt-oss-20b");
    let policy = TdxPolicy::default();
    let verified = ensure_verified_redpill_attestation(&backend, "openai/gpt-oss-20b", &policy)
        .await
        .expect("Shape A attestation must succeed");
    assert!(
        matches!(verified.shape, RedpillShape::Flat),
        "Shape A must dispatch to Flat, got {:?}",
        verified.shape
    );
    assert!(
        matches!(verified.freshness, Freshness::PerRequest),
        "Shape A freshness must be PerRequest, got {:?}",
        verified.freshness
    );
    assert!(verified.orchestrated_components.is_none());
    eprintln!(
        "[live] shape=Flat freshness=PerRequest model={}",
        verified.model
    );
}

#[tokio::test]
#[ignore = "live — Shape B (phala aggregator, aci/1 schema) against phala/gpt-oss-120b"]
async fn live_shape_b_orchestrated_three_way_and() {
    // 2026-08 upstream drift: Redpill migrated phala models to an `api_version:
    // aci/1` aggregator schema — no more gateway_attestation/model_attestations.
    // The response now dispatches as Flat (top-level signing_address + intel_quote)
    // and ships a top-level nvidia_payload whose evidence_list is EMPTY on these
    // TDX-only enclaves (GPU evidence would appear per-model when present).
    // The legacy three-way AND path is still covered offline by the orchestrated
    // fixtures in tests/attestation_redpill.rs.
    let backend = redpill_backend("phala/gpt-oss-120b");
    let policy = TdxPolicy::default();
    let verified = ensure_verified_redpill_attestation(&backend, "phala/gpt-oss-120b", &policy)
        .await
        .expect("Shape B attestation must succeed (aci/1 aggregator)");
    assert!(
        matches!(verified.shape, RedpillShape::Flat),
        "Shape B (aci/1 aggregator) must dispatch to Flat, got {:?}",
        verified.shape
    );
    assert!(
        matches!(verified.freshness, Freshness::PerRequest),
        "Shape B freshness must be PerRequest, got {:?}",
        verified.freshness
    );
    assert!(verified.orchestrated_components.is_none());
    eprintln!(
        "[live] shape=Flat (aci/1 aggregator) freshness=PerRequest model={}",
        verified.model
    );
}

#[tokio::test]
#[ignore = "live — Shape C (Chutes, per-enclave freshness, RED-09) against deepseek/deepseek-v3.2"]
async fn live_shape_c_chutes_per_enclave_freshness() {
    // 2026-08 upstream state: Chutes GPU evidence currently attests FALSE at
    // NRAS (x-nvidia-overall-att-result=false fleet-wide; JWT issuer and
    // eat_nonce still verify, so the crypto path is exercised end-to-end).
    // The client must FAIL CLOSED on that verdict — this test pins the full
    // machinery (TDX quote verify, anti-tamper decode, per-GPU NRAS JWT)
    // by asserting we reach and honor the NRAS policy verdict.
    // When Chutes/NVIDIA re-align their measurements, flip this back to a
    // success assertion on shape=Chutes / freshness=PerEnclave.
    let backend = redpill_backend("deepseek/deepseek-v3.2");
    let policy = TdxPolicy::default();
    let err = ensure_verified_redpill_attestation(&backend, "deepseek/deepseek-v3.2", &policy)
        .await
        .expect_err("NRAS-false GPU evidence must fail closed");
    let msg = format!("{err}");
    assert!(
        msg.contains("overall attestation result is not true"),
        "must fail on the NRAS verdict, got: {msg}"
    );
    eprintln!("[live] shape=Chutes machinery OK; NRAS verdict=false → failed closed");
}

// NOTE: `live_tinfoil_route_refused` (RED-10) was removed in 2026-08. Redpill's
// aci/1 aggregator no longer refuses Tinfoil-routed models with HTTP 502 — it
// now serves them from its own TDX enclave (downstream_tls_binding.domain =
// api.redpill.ai), so the attestation honestly proves the Redpill enclave and
// verification legitimately succeeds. The 502-detection path remains in
// fetch_and_verify_redpill_attestation for any future upstream change.
