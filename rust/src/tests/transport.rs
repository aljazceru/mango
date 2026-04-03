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

    assert_eq!(tinfoil.transport_kind(), ProviderTransportKind::TinfoilSecure);
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
        tinfoil_error.to_string().contains("Tinfoil secure transport"),
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
