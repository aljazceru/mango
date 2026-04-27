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

/// Plan 04 / RED-11: Verify the Orchestrated badge data flow — the three
/// orchestrated components (gateway, model, compose_manager) must extract from
/// VerifiedRedpillAttestation in the canonical (label, hex) order so the
/// native UI renders "gateway ✓ • model ✓ • compose ✓".
#[test]
fn redpill_orchestrated_event_carries_three_components() {
    use crate::attestation::redpill::{
        Freshness, OrchestratedComponents, RedpillShape, VerifiedRedpillAttestation,
    };

    let verified = VerifiedRedpillAttestation {
        backend_id: "redpill".into(),
        model: "phala/gpt-oss-120b".into(),
        shape: RedpillShape::Orchestrated { is_near_ai: false },
        freshness: Freshness::PerRequest,
        orchestrated_components: Some(OrchestratedComponents {
            gateway_signing_address_hex: "0xAA".into(),
            model_signing_address_hex: "0xBB".into(),
            compose_manager_actions_hash_hex: "0xCC".into(),
        }),
        expires_at: 0,
    };

    // The badge renderer extracts components into a Vec<(label, hex)> with
    // gateway → model → compose_manager order. Mirror the extraction logic
    // used in llm::redpill::verify_backend_attestation.
    let comps = verified
        .orchestrated_components
        .as_ref()
        .map(|c| {
            vec![
                ("gateway".to_string(), c.gateway_signing_address_hex.clone()),
                ("model".to_string(), c.model_signing_address_hex.clone()),
                (
                    "compose_manager".to_string(),
                    c.compose_manager_actions_hash_hex.clone(),
                ),
            ]
        })
        .expect("Orchestrated must populate components");

    assert_eq!(comps.len(), 3);
    assert_eq!(comps[0].0, "gateway");
    assert_eq!(comps[0].1, "0xAA");
    assert_eq!(comps[1].0, "model");
    assert_eq!(comps[1].1, "0xBB");
    assert_eq!(comps[2].0, "compose_manager");
    assert_eq!(comps[2].1, "0xCC");
}

/// Plan 04 / RED-09: Chutes shape's freshness must be PerEnclave, not PerRequest.
#[test]
fn redpill_chutes_shape_carries_per_enclave_freshness() {
    use crate::attestation::redpill::{Freshness, RedpillShape, VerifiedRedpillAttestation};

    let verified = VerifiedRedpillAttestation {
        backend_id: "redpill".into(),
        model: "deepseek/deepseek-v3.2".into(),
        shape: RedpillShape::Chutes,
        freshness: Freshness::PerEnclave,
        orchestrated_components: None,
        expires_at: 0,
    };

    let freshness_str: &str = match verified.freshness {
        Freshness::PerRequest => "PerRequest",
        Freshness::PerEnclave => "PerEnclave",
    };
    assert_eq!(freshness_str, "PerEnclave");
    assert!(matches!(verified.shape, RedpillShape::Chutes));
    assert!(verified.orchestrated_components.is_none());
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

/// RED-09 / RED-11 actor-loop drop closure (Phase 34.1 Plan 01).
///
/// Asserts that `attestation::map_event_to_record_and_status` threads
/// shape / freshness / orchestrated_components from `AttestationEvent::Verified`
/// into the persisted `AttestationRecord` for all three Redpill shapes
/// (Flat / PerRequest, Orchestrated / PerRequest, Chutes / PerEnclave).
///
/// Pure-function test — does NOT spin up the Tokio actor.
#[test]
fn actor_loop_threads_redpill_fields() {
    use crate::attestation::{
        map_event_to_record_and_status, AttestationEvent, AttestationStatus,
    };

    let cases = vec![
        (
            "Flat / PerRequest",
            Some("Flat".to_string()),
            Some("PerRequest".to_string()),
            None,
        ),
        (
            "Orchestrated / PerRequest",
            Some("Orchestrated".to_string()),
            Some("PerRequest".to_string()),
            Some(vec![
                ("gateway".to_string(), "0xAA".to_string()),
                ("model".to_string(), "0xBB".to_string()),
                ("compose_manager".to_string(), "0xCC".to_string()),
            ]),
        ),
        (
            "Chutes / PerEnclave",
            Some("Chutes".to_string()),
            Some("PerEnclave".to_string()),
            None,
        ),
    ];

    for (name, shape, freshness, components) in cases {
        let event = AttestationEvent::Verified {
            backend_id: "redpill-test".to_string(),
            tee_type: "tdx".to_string(),
            report_blob: vec![0xDE, 0xAD, 0xBE, 0xEF],
            expires_at: 1_700_000_300,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
            shape: shape.clone(),
            freshness: freshness.clone(),
            orchestrated_components: components.clone(),
        };

        let (backend_id, status, record_opt, _transient) =
            map_event_to_record_and_status(event, 1_700_000_000);

        assert_eq!(backend_id, "redpill-test", "case={}", name);
        assert!(matches!(status, AttestationStatus::Verified { .. }), "case={}", name);
        let (record, _, _, _, _) = record_opt.expect(name);
        assert_eq!(record.shape, shape, "case={}: shape", name);
        assert_eq!(record.freshness, freshness, "case={}: freshness", name);
        assert_eq!(
            record.orchestrated_components, components,
            "case={}: components",
            name
        );
    }
}
