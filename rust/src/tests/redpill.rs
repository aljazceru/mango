//! RED→GREEN tests for llm/redpill.rs + backend.rs/transport.rs wiring.
//! Plans 03 and 04 implement.

#![allow(unused_imports)]

#[test]
fn redpill_preset_present() {
    use crate::llm::backend::known_provider_presets;
    let presets = known_provider_presets();
    let r = presets
        .iter()
        .find(|p| p.id == "redpill")
        .expect("redpill preset must be in known_provider_presets()");
    assert_eq!(r.name, "Redpill");
    assert!(
        r.base_url.starts_with("https://api.redpill.ai/v1"),
        "unexpected base_url: {}",
        r.base_url
    );
    assert!(matches!(r.tee_type, crate::llm::TeeType::IntelTdx));
    let desc = r.description.to_lowercase();
    assert!(
        desc.contains("aggregator") || desc.contains("phala") || desc.contains("intel tdx"),
        "description should mention aggregator/phala/Intel TDX: {}",
        r.description
    );
}

#[test]
fn attestation_url_format() {
    use crate::llm::redpill::format_redpill_attestation_url;
    let url = format_redpill_attestation_url(
        "openai/gpt-oss-20b",
        "abc123",
        "https://api.redpill.ai/v1",
    );
    assert!(
        url.ends_with("/v1/attestation/report?model=openai%2Fgpt-oss-20b&nonce=abc123"),
        "unexpected URL: {url}"
    );
    // Trailing slash variant
    let url2 = format_redpill_attestation_url(
        "openai/gpt-oss-20b",
        "abc123",
        "https://api.redpill.ai/v1/",
    );
    assert!(
        url2.ends_with("/v1/attestation/report?model=openai%2Fgpt-oss-20b&nonce=abc123"),
        "unexpected URL (trailing-slash): {url2}"
    );
}

#[test]
fn tinfoil_route_refused_with_typed_error() {
    use crate::attestation::redpill::RedpillError;
    use crate::tests::common::redpill_fixtures::SHAPE_D_TINFOIL_REFUSAL_JSON;
    // The 502 body contains "Unsupported Tinfoil attestation format". Plan 02's
    // fetch_and_verify path detects this string and returns TinfoilUnsupported.
    assert!(SHAPE_D_TINFOIL_REFUSAL_JSON.contains("Unsupported Tinfoil"));
    // Synthesize a Display string for the error — must mention the typed variant.
    let err = RedpillError::TinfoilUnsupported;
    let msg = format!("{err}");
    let lc = msg.to_lowercase();
    assert!(lc.contains("tinfoil"), "error must mention Tinfoil: {msg}");
    assert!(
        lc.contains("direct"),
        "error message must hint at direct-Tinfoil: {msg}"
    );
}

#[test]
fn backend_summary_after_add() {
    use crate::llm::backend::{BackendConfig, ProviderKind, TeeType};
    use crate::llm::transport::ProviderTransportKind;

    let cfg = BackendConfig {
        id: "redpill".into(),
        name: "Redpill".into(),
        base_url: "https://api.redpill.ai/v1/".into(),
        api_key: "test".into(),
        models: vec!["openai/gpt-oss-20b".into()],
        tee_type: TeeType::IntelTdx,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };

    // RED-11: provider_kind dispatches to Redpill.
    assert_eq!(cfg.provider_kind(), ProviderKind::Redpill);

    // Transport routing: BackendConfig::transport_kind() returns Redpill variant.
    assert_eq!(
        ProviderTransportKind::for_backend(&cfg),
        ProviderTransportKind::Redpill
    );
    assert_eq!(cfg.transport_kind(), ProviderTransportKind::Redpill);

    // Backend summary surfaces Redpill backend without leaking the api_key.
    let summary = cfg.to_summary(true, crate::llm::backend::HealthStatus::Healthy);
    assert_eq!(summary.id, "redpill");
    assert_eq!(summary.name, "Redpill");
    assert_eq!(summary.tee_type, TeeType::IntelTdx);
    assert!(summary.is_active);
    assert!(summary.has_api_key);
    assert!(summary.supports_tool_use);
}
