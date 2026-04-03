use crate::llm::backend::{
    known_provider_presets, tinfoil_backend, BackendConfig, HealthStatus, TeeType,
};

#[test]
fn test_tinfoil_config() {
    let b = tinfoil_backend();
    assert_eq!(b.id, "tinfoil");
    assert_eq!(b.name, "Tinfoil");
    assert_eq!(b.base_url, "https://inference.tinfoil.sh/v1/");
    assert_eq!(b.tee_type, TeeType::IntelTdx);
    assert!(!b.models.is_empty());
}

#[test]
fn test_backend_summary_hides_api_key() {
    let b = tinfoil_backend();
    let summary = b.to_summary(true, HealthStatus::Unknown);
    assert_eq!(summary.id, "tinfoil");
    assert_eq!(summary.name, "Tinfoil");
    assert!(summary.is_active);
    // BackendSummary has no api_key field -- verified by the fact that this compiles
    // and we only access id, name, models, tee_type, is_active, health_status
    assert!(!summary.models.is_empty());
}

#[test]
fn test_backend_summary_inactive() {
    let b = tinfoil_backend();
    let summary = b.to_summary(false, HealthStatus::Unknown);
    assert!(!summary.is_active);
}

#[test]
fn test_parse_tee_type_amd_sev_snp() {
    assert_eq!(crate::parse_tee_type("AmdSevSnp"), TeeType::AmdSevSnp);
}

#[test]
fn test_known_provider_presets_includes_ppq_ai() {
    let presets = known_provider_presets();
    let ppq = presets
        .iter()
        .find(|p| p.id == "ppq-ai")
        .expect("ppq-ai preset not found in known_provider_presets()");
    assert_eq!(ppq.base_url, "https://api.ppq.ai/private/v1/");
    assert_eq!(ppq.tee_type, TeeType::AmdSevSnp);
    assert!(
        ppq.description.contains("AMD SEV-SNP"),
        "description should contain 'AMD SEV-SNP', got: {}",
        ppq.description
    );
}

#[test]
fn test_ppq_ai_supports_tool_use() {
    let b = BackendConfig {
        id: "ppq-ai".into(),
        name: "PPQ.AI".into(),
        base_url: "https://api.ppq.ai/private/v1/".into(),
        api_key: "sk-test".into(),
        models: vec![],
        tee_type: TeeType::AmdSevSnp,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };
    let summary = b.to_summary(true, HealthStatus::Unknown);
    assert!(
        summary.supports_tool_use,
        "ppq-ai backend should support tool use"
    );
}

#[test]
fn test_supports_tool_use_false_propagates() {
    let b = BackendConfig {
        id: "tinfoil".into(),
        name: "Tinfoil".into(),
        base_url: "https://inference.tinfoil.sh/v1/".into(),
        api_key: "sk-test".into(),
        models: vec![],
        tee_type: TeeType::IntelTdx,
        max_concurrent_requests: 5,
        supports_tool_use: false,
    };
    let summary = b.to_summary(true, HealthStatus::Unknown);
    assert!(
        !summary.supports_tool_use,
        "supports_tool_use=false on BackendConfig must propagate to BackendSummary (no hardcoded override)"
    );
}
