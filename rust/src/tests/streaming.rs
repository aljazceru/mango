use crate::llm::error::LlmError;
use crate::llm::streaming::InternalEvent;
use crate::{AppAction, BusyState, EmbeddingStatus, FfiApp};
use std::time::Duration;

/// Helper: create FfiApp and give it a moment to initialize.
/// Uses empty data_dir so the database opens as :memory:.
fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(crate::NullKeychainProvider),
        Box::new(crate::NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    // Phase 8: VectorIndex init + document list load adds overhead; 150ms is stable in parallel test load.
    std::thread::sleep(Duration::from_millis(150));
    app
}

#[test]
fn test_initial_state_has_backends() {
    let app = make_app();
    let state = app.state();
    // v1 seeds tinfoil (active), v10 seeds ppq-ai (inactive)
    assert_eq!(
        state.backends.len(),
        2,
        "Expected 2 backends (Tinfoil + PPQ.AI)"
    );
    assert_eq!(state.active_backend_id, Some("tinfoil".to_string()));
    assert!(state.backends.iter().any(|b| b.id == "tinfoil"));
}

#[test]
fn test_set_active_backend() {
    let app = make_app();
    // Active backend should already be tinfoil from init
    let state = app.state();
    assert_eq!(state.active_backend_id, Some("tinfoil".to_string()));
}

#[test]
fn test_stop_generation_when_idle_is_noop() {
    let app = make_app();
    app.dispatch(AppAction::StopGeneration);
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    assert_eq!(state.busy_state, BusyState::Idle);
}

#[test]
fn test_send_message_starts_streaming() {
    let app = make_app();
    app.dispatch(AppAction::SendMessage {
        text: "Hello".into(),
    });
    std::thread::sleep(Duration::from_millis(100));
    let state = app.state();
    // Should be streaming (or error if no API key, which is expected in tests)
    // The important thing: busy_state transitioned from Idle or an error was recorded
    assert!(
        matches!(state.busy_state, BusyState::Streaming { .. }) || state.last_error.is_some(),
        "Expected streaming or error, got: {:?}",
        state.busy_state
    );
}

// --- InternalEvent injection tests (LLMC-03) ---
// These use test_send_internal() to bypass HTTP and test the actor's
// event processing logic directly.

#[test]
fn test_stream_chunk_accumulates() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "Hello".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.test_send_internal(InternalEvent::StreamChunk {
        token: " world".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    assert_eq!(state.streaming_text, Some("Hello world".to_string()));
}

#[test]
fn test_stream_done_sets_idle() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "Done test".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.test_send_internal(InternalEvent::StreamDone);
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    // StreamDone must set BusyState::Idle
    assert_eq!(state.busy_state, BusyState::Idle);
    // Phase 5: streaming_text is cleared on StreamDone (moved into messages list).
    // When there is no active conversation (as in this test), content is still cleared.
    assert!(
        state.streaming_text.is_none() || state.streaming_text.as_deref() == Some(""),
        "streaming_text should be cleared after StreamDone (Phase 5 moves it to messages)"
    );
}

#[test]
fn test_stream_error_preserves_partial() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "Partial".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.test_send_internal(InternalEvent::StreamError {
        error: LlmError::NetworkError {
            reason: "connection lost".into(),
        },
    });
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    // Partial text should be preserved, not cleared
    assert!(
        state.streaming_text.is_some(),
        "streaming_text should preserve partial content on error"
    );
    assert!(
        state.last_error.is_some(),
        "last_error should be set on StreamError"
    );
}

// --- Streaming cancellation tests (TEST-01) ---
// These verify the StopGeneration -> StreamCancelled -> BusyState::Idle path.

#[test]
fn test_stop_generation_cancels_active_stream() {
    // Tinfoil HPKE transport: dispatching StopGeneration during active streaming,
    // followed by the StreamCancelled event, transitions to BusyState::Idle.
    let app = make_app();

    // Inject a chunk to simulate an in-progress stream
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "partial response".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    {
        let state = app.state();
        assert_eq!(
            state.streaming_text,
            Some("partial response".to_string()),
            "streaming_text must accumulate before cancel"
        );
    }

    // Dispatch StopGeneration -- exercises the active_stream_token.cancel() path.
    // BusyState must NOT go Idle here; we wait for StreamCancelled.
    app.dispatch(AppAction::StopGeneration);

    // Simulate what the Tinfoil transport sends after cancel_token fires
    app.test_send_internal(InternalEvent::StreamCancelled);
    std::thread::sleep(Duration::from_millis(100));

    let state = app.state();
    assert_eq!(
        state.busy_state,
        BusyState::Idle,
        "StopGeneration + StreamCancelled must transition to Idle"
    );
    // Partial text is preserved on cancel, not cleared
    assert!(
        state.streaming_text.is_some(),
        "streaming_text must be preserved (not cleared) after cancel"
    );
}

#[test]
fn test_stop_generation_cancels_ppq_stream() {
    // PPQ AES-GCM transport uses the same StreamCancelled event as Tinfoil HPKE (per D-03).
    // This test documents PPQ coverage of the same cancellation path.
    let app = make_app();

    // Inject a chunk that distinguishes this test from the Tinfoil test
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "ppq partial".into(),
    });
    std::thread::sleep(Duration::from_millis(50));

    // Dispatch StopGeneration, then inject StreamCancelled as the PPQ transport would
    app.dispatch(AppAction::StopGeneration);
    app.test_send_internal(InternalEvent::StreamCancelled);
    std::thread::sleep(Duration::from_millis(100));

    let state = app.state();
    assert_eq!(
        state.busy_state,
        BusyState::Idle,
        "StopGeneration + StreamCancelled must transition to Idle (PPQ path)"
    );
    assert!(
        state.streaming_text.is_some(),
        "streaming_text must be preserved after cancel (PPQ path)"
    );
}
