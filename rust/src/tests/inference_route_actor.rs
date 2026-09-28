//! FfiApp round-trip for the settings route switch. Uses Null providers, so the
//! device reports local inference as unsupported.
use crate::routing::route_mode::InferenceRoute;
use crate::{AppAction, EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};

fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    app.sync();
    app
}

#[test]
fn inference_status_is_populated_after_startup() {
    let app = make_app();
    let s = app.state().inference_status;
    assert_eq!(s.route, InferenceRoute::Cloud);
    assert!(!s.on_device_available, "Null provider cannot run local models");
}

#[test]
fn on_device_route_on_unsupported_device_toasts_and_stays() {
    let app = make_app();
    let before = app.state().inference_status.backend_id.clone();
    app.dispatch(AppAction::SetInferenceRoute { route: InferenceRoute::OnDevice });
    app.sync();
    let st = app.state();
    assert!(st.toast.is_some(), "blocked switch must explain itself");
    assert_eq!(st.inference_status.backend_id, before, "route must not change");
}

#[test]
fn hybrid_route_without_profile_opens_hybrid_setup() {
    let app = make_app();
    app.dispatch(AppAction::SetInferenceRoute { route: InferenceRoute::Hybrid });
    app.sync();
    let st = app.state();
    assert_eq!(st.router.current_screen, crate::Screen::SettingsHybridRouting);
    assert_eq!(st.toast.as_deref(), Some("Set up hybrid rules first."));
}
