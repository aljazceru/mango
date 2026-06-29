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
#[ignore = "live — Shape B (Orchestrated, three-way AND) against phala/gpt-oss-120b"]
async fn live_shape_b_orchestrated_three_way_and() {
    let backend = redpill_backend("phala/gpt-oss-120b");
    let policy = TdxPolicy::default();
    let verified = ensure_verified_redpill_attestation(&backend, "phala/gpt-oss-120b", &policy)
        .await
        .expect("Shape B attestation must succeed (three-way AND)");
    assert!(
        matches!(verified.shape, RedpillShape::Orchestrated { .. }),
        "Shape B must dispatch to Orchestrated, got {:?}",
        verified.shape
    );
    let comps = verified
        .orchestrated_components
        .as_ref()
        .expect("Orchestrated must populate components");
    assert!(
        !comps.gateway_signing_address_hex.is_empty(),
        "gateway address must be populated"
    );
    assert!(
        !comps.model_signing_address_hex.is_empty(),
        "model address must be populated"
    );
    assert!(
        !comps.compose_manager_actions_hash_hex.is_empty(),
        "compose-manager actions hash must be populated"
    );
    eprintln!(
        "[live] shape=Orchestrated gateway={} model={} compose={}",
        comps.gateway_signing_address_hex,
        comps.model_signing_address_hex,
        comps.compose_manager_actions_hash_hex
    );
}

#[tokio::test]
#[ignore = "live — Shape C (Chutes, per-enclave freshness, RED-09) against deepseek/deepseek-v3.2"]
async fn live_shape_c_chutes_per_enclave_freshness() {
    let backend = redpill_backend("deepseek/deepseek-v3.2");
    let policy = TdxPolicy::default();
    let verified = ensure_verified_redpill_attestation(&backend, "deepseek/deepseek-v3.2", &policy)
        .await
        .expect("Shape C attestation must succeed");
    assert!(
        matches!(verified.shape, RedpillShape::Chutes),
        "Shape C must dispatch to Chutes, got {:?}",
        verified.shape
    );
    assert!(
        matches!(verified.freshness, Freshness::PerEnclave),
        "Shape C freshness must be PerEnclave (RED-09), got {:?}",
        verified.freshness
    );
    eprintln!(
        "[live] shape=Chutes freshness=PerEnclave model={}",
        verified.model
    );
}

#[tokio::test]
#[ignore = "live — Tinfoil-routed model fails closed (RED-10)"]
async fn live_tinfoil_route_refused() {
    let backend = redpill_backend("meta-llama/llama-3.3-70b-instruct");
    let policy = TdxPolicy::default();
    let result =
        ensure_verified_redpill_attestation(&backend, "meta-llama/llama-3.3-70b-instruct", &policy)
            .await;
    // Either the /v1/models providers check fires upstream, or the orchestrator
    // detects HTTP 502 'Unsupported Tinfoil' inside fetch_and_verify. Both surface
    // as an error — Tinfoil-via-Redpill must NEVER succeed via the aggregator.
    assert!(
        result.is_err(),
        "Tinfoil-routed model must fail closed via the aggregator path"
    );
    eprintln!("[live] tinfoil-route refused: {:?}", result.err());
}
