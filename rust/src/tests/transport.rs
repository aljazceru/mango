use crate::llm::transport::ProviderTransportKind;
use crate::llm::{BackendConfig, TeeType};

fn backend(id: &str, base_url: &str) -> BackendConfig {
    BackendConfig {
        id: id.to_string(),
        name: id.to_string(),
        base_url: base_url.to_string(),
        api_key: "sk-test".to_string(),
        models: vec!["dummy".to_string()],
        tee_type: TeeType::Unknown,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    }
}

#[test]
fn test_standard_backends_use_openai_transport() {
    let ppq = backend("ppq-ai", "https://api.ppq.ai/v1/");
    let custom = backend("custom", "https://example.com/v1/");

    assert_eq!(
        ppq.transport_kind(),
        ProviderTransportKind::OpenAiCompatible
    );
    assert_eq!(
        custom.transport_kind(),
        ProviderTransportKind::OpenAiCompatible
    );
}

#[test]
fn test_tinfoil_base_url_selects_secure_transport() {
    let tinfoil = backend("tinfoil", "https://inference.tinfoil.sh/v1/");

    assert_eq!(
        tinfoil.transport_kind(),
        ProviderTransportKind::TinfoilSecure
    );
}

#[test]
fn test_ppq_private_base_url_selects_private_transport() {
    let ppq_private = backend("ppq-ai", "https://api.ppq.ai/private/v1/");

    assert_eq!(
        ppq_private.transport_kind(),
        ProviderTransportKind::PpqPrivateE2ee
    );
}

#[test]
fn test_openai_transport_builds_model_endpoint() {
    let tinfoil = backend("tinfoil", "https://inference.tinfoil.sh/v1/");
    let url = tinfoil.transport_kind().model_list_url(&tinfoil).unwrap();

    assert_eq!(url, "https://inference.tinfoil.sh/v1/models");
}

#[test]
fn test_secure_transports_return_explicit_error() {
    let tinfoil = backend("tinfoil", "https://inference.tinfoil.sh/v1/");
    let ppq_private = backend("ppq-ai", "https://api.ppq.ai/private/v1/");

    let tinfoil_error = tinfoil
        .transport_kind()
        .openai_api_base(&tinfoil)
        .expect_err("secure transport should not pretend to be plain OpenAI transport");
    assert!(
        tinfoil_error
            .to_string()
            .contains("Tinfoil secure transport"),
        "unexpected error: {}",
        tinfoil_error
    );

    let error = ppq_private
        .transport_kind()
        .openai_api_base(&ppq_private)
        .expect_err("private transport should not pretend to be plain OpenAI transport");

    assert!(
        error.to_string().contains("PPQ private E2EE transport"),
        "unexpected error: {}",
        error
    );
}

#[test]
fn test_private_transport_probes_public_model_endpoint() {
    let ppq_private = backend("ppq-ai", "https://api.ppq.ai/private/v1/");
    let url = ppq_private
        .transport_kind()
        .model_list_url(&ppq_private)
        .expect("private transport should still support model probing");

    assert_eq!(url, "https://api.ppq.ai/v1/models");
}

#[test]
fn test_openai_transport_without_pin_builds_standard_client() {
    let custom = backend("custom", "https://example.com/v1/");
    let (_client, used_pin) = custom
        .transport_kind()
        .build_reqwest_client(&custom, None, std::time::Duration::from_secs(1))
        .expect("standard client should build successfully");

    assert!(
        !used_pin,
        "transport should report pinning disabled when no pin is requested"
    );
}

#[test]
fn venice_routes_to_venice_e2ee() {
    let venice = backend("venice-ai", "https://api.venice.ai/api/v1/");
    assert_eq!(
        venice.transport_kind(),
        ProviderTransportKind::VeniceE2ee
    );

    // Existing routes still work
    let tinfoil = backend("tinfoil", "https://inference.tinfoil.sh/v1/");
    assert_eq!(
        tinfoil.transport_kind(),
        ProviderTransportKind::TinfoilSecure
    );

    // Venice transport must reject the OpenAI api_base path (forces use of venice::*).
    let err = venice
        .transport_kind()
        .openai_api_base(&venice)
        .expect_err("Venice E2EE transport must not pretend to be plain OpenAI");
    assert!(
        err.to_string().contains("Venice E2EE transport"),
        "unexpected error: {err}"
    );

    // model_list_url should succeed via super::venice::model_list_url
    let url = venice
        .transport_kind()
        .model_list_url(&venice)
        .expect("model list URL should build for Venice");
    assert!(url.ends_with("/api/v1/models"), "unexpected url: {url}");
}

#[test]
fn redpill_routes_to_redpill_transport() {
    let redpill = backend("redpill", "https://api.redpill.ai/v1/");
    assert_eq!(
        redpill.transport_kind(),
        ProviderTransportKind::Redpill,
        "Redpill backend must route to the Redpill transport variant"
    );

    // model_list_url goes through llm::redpill::model_list_url
    let url = redpill
        .transport_kind()
        .model_list_url(&redpill)
        .expect("Redpill model_list_url must succeed");
    assert_eq!(url, "https://api.redpill.ai/v1/models");

    // openai_api_base IS supported for Redpill (no E2EE wrapper).
    let api_base = redpill
        .transport_kind()
        .openai_api_base(&redpill)
        .expect("Redpill openai_api_base must succeed (vanilla OpenAI-compatible)");
    assert_eq!(api_base, "https://api.redpill.ai/v1");

    // Existing routes must still resolve correctly (no regression).
    let venice = backend("venice-ai", "https://api.venice.ai/api/v1/");
    assert_eq!(
        venice.transport_kind(),
        ProviderTransportKind::VeniceE2ee
    );
    let tinfoil = backend("tinfoil", "https://inference.tinfoil.sh/v1/");
    assert_eq!(
        tinfoil.transport_kind(),
        ProviderTransportKind::TinfoilSecure
    );
}

#[test]
fn test_openai_transport_fails_closed_when_pinned_client_cannot_be_built() {
    let custom = backend("custom", "https://example.com/v1/");
    let error = custom
        .transport_kind()
        .build_reqwest_client(
            &custom,
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            std::time::Duration::from_secs(1),
        )
        .expect_err("pinning errors must fail closed");

    assert!(
        error.to_string().contains("requires pinned TLS"),
        "transport must surface a pinning error instead of falling back: {}",
        error
    );
}
