use std::time::Duration;
use crate::{AppAction, EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};

#[test]
fn test_degraded_status_propagates_to_app_state() {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Degraded,
    );
    std::thread::sleep(Duration::from_millis(100));
    let state = app.state();
    assert_eq!(state.embedding_status, EmbeddingStatus::Degraded);
}

#[test]
fn test_actor_handles_send_message_when_degraded() {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Degraded,
    );
    std::thread::sleep(Duration::from_millis(100));
    app.dispatch(AppAction::NewConversation);
    std::thread::sleep(Duration::from_millis(100));
    let state = app.state();
    assert_eq!(state.embedding_status, EmbeddingStatus::Degraded);
    assert_eq!(state.conversations.len(), 1);
}

#[test]
fn test_active_status_on_normal_init() {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    std::thread::sleep(Duration::from_millis(100));
    let state = app.state();
    assert_eq!(state.embedding_status, EmbeddingStatus::Active);
}
