//! Pure-function tests for routing::route_mode (settings status card + route switch).
use crate::llm::local_models::LocalModelSummary;
use crate::llm::{BackendSummary, HealthStatus, TeeType};
use crate::routing::route_mode::*;
use crate::routing::{HybridProfile, LocalPreprocessing, RoutingPolicy};

pub(crate) fn backend(id: &str, name: &str, tee: TeeType, models: &[&str], key: bool) -> BackendSummary {
    BackendSummary {
        id: id.into(),
        name: name.into(),
        models: models.iter().map(|m| m.to_string()).collect(),
        tee_type: tee,
        is_active: false,
        health_status: HealthStatus::Healthy,
        supports_tool_use: true,
        has_api_key: key,
    }
}

pub(crate) fn local_model(id: &str, name: &str, verified: bool) -> LocalModelSummary {
    LocalModelSummary {
        id: id.into(),
        name: name.into(),
        description: String::new(),
        quantization: "Q4_K_M".into(),
        size_bytes: 1,
        min_ram_bytes: 1,
        downloaded: verified,
        verified,
        path: None,
        backend_id: if verified { Some(format!("local-{id}")) } else { None },
    }
}

pub(crate) fn profile(id: &str) -> HybridProfile {
    HybridProfile {
        id: id.into(),
        name: "Everyday".into(),
        local_backend_id: "local-qwen3".into(),
        local_model_id: "qwen3".into(),
        remote_backend_id: "tinfoil".into(),
        remote_model_id: "glm-5-3".into(),
        policy: RoutingPolicy::default(),
        preprocessing: LocalPreprocessing::default(),
    }
}

fn fixtures() -> (Vec<BackendSummary>, Vec<HybridProfile>, Vec<LocalModelSummary>) {
    (
        vec![
            backend("tinfoil", "Tinfoil", TeeType::IntelTdx, &["glm-5-3", "llama-4"], true),
            backend("ppq-ai", "PPQ.AI", TeeType::AmdSevSnp, &["gpt-oss-120b"], true),
            backend("local-qwen3", "Qwen3 1.7B", TeeType::Unknown, &["qwen3"], true),
        ],
        vec![profile("p1")],
        vec![local_model("qwen3", "Qwen3 1.7B", true), local_model("gemma3", "Gemma 3 4B", false)],
    )
}

#[test]
fn route_is_derived_from_backend_id_prefix() {
    assert_eq!(route_for_backend_id("hybrid:p1"), InferenceRoute::Hybrid);
    assert_eq!(route_for_backend_id("local-qwen3"), InferenceRoute::OnDevice);
    assert_eq!(route_for_backend_id("tinfoil"), InferenceRoute::Cloud);
    assert_eq!(route_for_backend_id(""), InferenceRoute::Cloud);
}

#[test]
fn cloud_status_names_provider_and_attests_it() {
    let (b, h, l) = fixtures();
    let s = inference_status(&b, &h, &l, true, "tinfoil", "glm-5-3");
    assert_eq!(s.route, InferenceRoute::Cloud);
    assert_eq!(s.backend_name, "Tinfoil");
    assert_eq!(s.model_id, "glm-5-3");
    assert_eq!(s.tee_type, Some(TeeType::IntelTdx));
    assert_eq!(s.attested_backend_id.as_deref(), Some("tinfoil"));
    assert!(s.hybrid_available);
    assert!(s.on_device_available);
}

#[test]
fn hybrid_status_uses_profile_remote_for_model_and_attestation() {
    let (b, h, l) = fixtures();
    let s = inference_status(&b, &h, &l, true, "hybrid:p1", "ignored");
    assert_eq!(s.route, InferenceRoute::Hybrid);
    assert_eq!(s.model_id, "glm-5-3");
    assert_eq!(s.attested_backend_id.as_deref(), Some("tinfoil"));
    assert_eq!(s.tee_type, Some(TeeType::IntelTdx));
    assert_eq!(s.local_model_name.as_deref(), Some("Qwen3 1.7B"));
}

#[test]
fn on_device_status_has_no_tee_and_no_attestation() {
    let (b, h, l) = fixtures();
    let s = inference_status(&b, &h, &l, true, "local-qwen3", "qwen3");
    assert_eq!(s.route, InferenceRoute::OnDevice);
    assert_eq!(s.tee_type, None);
    assert_eq!(s.attested_backend_id, None);
    assert_eq!(s.local_model_name.as_deref(), Some("Qwen3 1.7B"));
}

#[test]
fn availability_flags_follow_profiles_models_and_capability() {
    let (b, _, l) = fixtures();
    let s = inference_status(&b, &[], &l, false, "tinfoil", "glm-5-3");
    assert!(!s.hybrid_available, "no profiles");
    assert!(!s.on_device_available, "device unsupported");
    let unverified = vec![local_model("gemma3", "Gemma 3 4B", false)];
    let s = inference_status(&b, &[], &unverified, true, "tinfoil", "glm-5-3");
    assert!(!s.on_device_available, "no verified model");
}

#[test]
fn unknown_hybrid_profile_degrades_to_empty_cloud_status() {
    let (b, h, l) = fixtures();
    let s = inference_status(&b, &h, &l, true, "hybrid:gone", "");
    assert_eq!(s.route, InferenceRoute::Hybrid);
    assert_eq!(s.attested_backend_id, None);
    assert!(s.backend_name.is_empty());
}

fn inputs<'a>(
    b: &'a [BackendSummary],
    h: &'a [HybridProfile],
    l: &'a [LocalModelSummary],
    current: &'a str,
    model: &'a str,
) -> RouteInputs<'a> {
    RouteInputs {
        backends: b,
        hybrid_profiles: h,
        local_models: l,
        local_supported: true,
        current_backend_id: current,
        current_model_id: model,
        remembered_cloud_backend_id: None,
        remembered_cloud_model_id: None,
        remembered_local_backend_id: None,
    }
}

#[test]
fn cloud_keeps_current_cloud_backend_and_model() {
    let (b, h, l) = fixtures();
    let t = resolve_route_target(InferenceRoute::Cloud, &inputs(&b, &h, &l, "ppq-ai", "gpt-oss-120b"));
    assert_eq!(t, Ok(RouteTarget::Backend { backend_id: "ppq-ai".into(), model_id: "gpt-oss-120b".into() }));
}

#[test]
fn cloud_from_device_restores_remembered_backend_and_model() {
    let (b, h, l) = fixtures();
    let mut i = inputs(&b, &h, &l, "local-qwen3", "qwen3");
    i.remembered_cloud_backend_id = Some("ppq-ai");
    i.remembered_cloud_model_id = Some("gpt-oss-120b");
    assert_eq!(
        resolve_route_target(InferenceRoute::Cloud, &i),
        Ok(RouteTarget::Backend { backend_id: "ppq-ai".into(), model_id: "gpt-oss-120b".into() })
    );
}

#[test]
fn cloud_without_memory_skips_failed_and_unkeyed_backends() {
    let (mut b, h, l) = fixtures();
    b[0].health_status = HealthStatus::Failed; // tinfoil failed
    b.insert(0, backend("redpill", "Redpill", TeeType::IntelTdx, &[], false)); // not configured
    let t = resolve_route_target(InferenceRoute::Cloud, &inputs(&b, &h, &l, "local-qwen3", "qwen3"));
    assert_eq!(t, Ok(RouteTarget::Backend { backend_id: "ppq-ai".into(), model_id: "gpt-oss-120b".into() }));
}

#[test]
fn cloud_remembered_model_not_served_falls_back_to_first_model() {
    let (b, h, l) = fixtures();
    let mut i = inputs(&b, &h, &l, "local-qwen3", "qwen3");
    i.remembered_cloud_backend_id = Some("tinfoil");
    i.remembered_cloud_model_id = Some("gpt-oss-120b");
    assert_eq!(
        resolve_route_target(InferenceRoute::Cloud, &i),
        Ok(RouteTarget::Backend { backend_id: "tinfoil".into(), model_id: "glm-5-3".into() })
    );
}

#[test]
fn cloud_blocked_without_configured_provider() {
    let b = vec![backend("qvac-local", "QVAC", TeeType::Unknown, &["m"], true)];
    let t = resolve_route_target(InferenceRoute::Cloud, &inputs(&b, &[], &[], "", ""));
    assert_eq!(t, Err(RouteBlocker::NoCloudProvider));
}

#[test]
fn hybrid_uses_current_or_first_profile() {
    let (b, _, l) = fixtures();
    let h = vec![profile("p1"), profile("p2")];
    assert_eq!(
        resolve_route_target(InferenceRoute::Hybrid, &inputs(&b, &h, &l, "hybrid:p2", "")),
        Ok(RouteTarget::HybridProfile { profile_id: "p2".into() })
    );
    assert_eq!(
        resolve_route_target(InferenceRoute::Hybrid, &inputs(&b, &h, &l, "tinfoil", "")),
        Ok(RouteTarget::HybridProfile { profile_id: "p1".into() })
    );
    assert_eq!(
        resolve_route_target(InferenceRoute::Hybrid, &inputs(&b, &[], &l, "tinfoil", "")),
        Err(RouteBlocker::NoHybridProfile)
    );
}

#[test]
fn on_device_picks_verified_model_or_explains_why_not() {
    let (b, h, l) = fixtures();
    assert_eq!(
        resolve_route_target(InferenceRoute::OnDevice, &inputs(&b, &h, &l, "tinfoil", "glm-5-3")),
        Ok(RouteTarget::Backend { backend_id: "local-qwen3".into(), model_id: "qwen3".into() })
    );
    let mut unsupported = inputs(&b, &h, &l, "tinfoil", "");
    unsupported.local_supported = false;
    assert_eq!(resolve_route_target(InferenceRoute::OnDevice, &unsupported), Err(RouteBlocker::LocalUnsupported));
    let none = vec![local_model("gemma3", "Gemma 3 4B", false)];
    assert_eq!(
        resolve_route_target(InferenceRoute::OnDevice, &inputs(&b, &h, &none, "tinfoil", "")),
        Err(RouteBlocker::NoLocalModel)
    );
}

#[test]
fn blockers_point_to_the_screen_that_fixes_them() {
    assert_eq!(blocker_setup_screen(&RouteBlocker::NoCloudProvider), Some(crate::Screen::SettingsProviders));
    assert_eq!(blocker_setup_screen(&RouteBlocker::NoHybridProfile), Some(crate::Screen::SettingsHybridRouting));
    assert_eq!(blocker_setup_screen(&RouteBlocker::NoLocalModel), Some(crate::Screen::SettingsLocalModels));
    assert_eq!(blocker_setup_screen(&RouteBlocker::LocalUnsupported), None);
    assert!(!blocker_message(&RouteBlocker::NoLocalModel).is_empty());
}
