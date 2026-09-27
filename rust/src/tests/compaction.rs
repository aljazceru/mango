//! Chat compaction, learned model context limits, and local-models error
//! scoping (regression: the Local Models picker must never show chat errors).

use crate::llm::streaming::InternalEvent;
use crate::llm::LlmError;
use crate::persistence::{queries, Database};
use crate::{
    AppAction, EmbeddingStatus, FfiApp, NullBiometricProvider, NullEmbeddingProvider,
    NullKeychainProvider,
};

/// Helper: create FfiApp with in-memory DB and let the actor initialize.
fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    app
}

// ── Context overflow → compaction offer ───────────────────────────────────────

#[test]
fn overflow_stream_error_offers_compaction() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamError {
        error: LlmError::ApiError {
            status_code: 400,
            reason: "Input length (139080) exceeds model's maximum context length (131072)."
                .to_string(),
        },
    });
    app.sync();

    let state = app.state();
    assert!(
        state.compaction_offered,
        "a context overflow must offer compaction"
    );
    let error = state.last_error.expect("error must surface");
    assert!(
        error.contains("Compact the chat"),
        "error must steer to compaction, got: {error}"
    );
    assert!(
        error.contains("131072"),
        "error must state the model's real limit, got: {error}"
    );
    assert!(
        !error.contains("Server error"),
        "overflow must not surface the raw provider body, got: {error}"
    );
}

#[test]
fn non_overflow_stream_error_does_not_offer_compaction() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamError {
        error: LlmError::ApiError {
            status_code: 400,
            reason: "invalid request".to_string(),
        },
    });
    app.sync();

    let state = app.state();
    assert!(!state.compaction_offered);
    let error = state.last_error.expect("error must surface");
    assert!(error.contains("Server error (400)"), "got: {error}");
}

// ── Compaction keeps device history ───────────────────────────────────────────

#[test]
fn compaction_complete_stores_summary_and_keeps_history() {
    let app = make_app();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    for i in 0..4 {
        app.dispatch(AppAction::SendMessage {
            text: format!("message {i}"),
            force_role: None,
        });
        app.sync();
    }
    let conv_id = app
        .state()
        .current_conversation_id
        .clone()
        .expect("conversation exists");
    let before = app.state().messages.len();
    assert!(before >= 4, "expected persisted messages, got {before}");

    app.test_send_internal(InternalEvent::ConversationCompactionComplete {
        conversation_id: conv_id.clone(),
        covered_count: 2,
        retry_after: false,
        result: Ok("summary of earlier turns".to_string()),
    });
    app.sync();

    let state = app.state();
    assert_eq!(
        state.messages.len(),
        before,
        "compaction must never modify device history"
    );
    let compaction = state.compaction.expect("compaction state must be exposed");
    assert_eq!(compaction.conversation_id, conv_id);
    assert_eq!(compaction.covered_message_count, 2);
    assert_eq!(compaction.total_message_count, before as u64);
    assert!(!state.compaction_offered);
}

#[test]
fn compact_action_with_short_chat_is_a_noop_toast() {
    let app = make_app();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    let conv_id = app
        .state()
        .current_conversation_id
        .clone()
        .expect("conversation exists");

    app.dispatch(AppAction::CompactConversation {
        conversation_id: conv_id,
        retry_after: false,
    });
    app.sync();

    let state = app.state();
    let toast = state.toast.expect("a toast explains the no-op");
    assert!(toast.contains("Nothing to compact"), "got: {toast}");
}

// ── Local-models errors stay off the chat error channel ──────────────────────

#[test]
fn local_model_actions_scope_errors_away_from_chat() {
    let app = make_app();

    app.dispatch(AppAction::DownloadLocalModel {
        model_id: "bogus-model".to_string(),
    });
    app.sync();
    let state = app.state();
    let scoped = state.local_models_error.expect("picker error must be set");
    assert!(scoped.contains("Unknown local model"), "got: {scoped}");
    assert_eq!(
        state.last_error, None,
        "local-models errors must not leak into the chat-scoped error channel"
    );

    // Capability rejection on an unsupported device is a picker error too.
    app.dispatch(AppAction::SetLocalInferenceEnabled { enabled: true });
    app.sync();
    let state = app.state();
    assert!(
        state.local_models_error.is_some(),
        "capability rejection must surface on the picker"
    );
    assert_eq!(state.last_error, None);
}

// ── Persistence round-trips ───────────────────────────────────────────────────

#[test]
fn compaction_and_model_limit_round_trip() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    conn.execute(
        "INSERT INTO conversations (id, title, model_id, backend_id, created_at, updated_at)
         VALUES ('c1', 'T', 'm', 'b', 1, 1)",
        [],
    )
    .unwrap();

    assert!(queries::get_conversation_compaction(conn, "c1")
        .unwrap()
        .is_none());
    queries::update_conversation_compaction(conn, "c1", Some("summary"), 3).unwrap();
    let row = queries::get_conversation_compaction(conn, "c1")
        .unwrap()
        .expect("stored");
    assert_eq!(row.summary, "summary");
    assert_eq!(row.covered_count, 3);
    // Clearing works.
    queries::update_conversation_compaction(conn, "c1", None, 0).unwrap();
    assert!(queries::get_conversation_compaction(conn, "c1")
        .unwrap()
        .is_none());

    assert!(queries::get_model_context_limit(conn, "b", "m").unwrap().is_none());
    queries::upsert_model_context_limit(conn, "b", "m", 131072).unwrap();
    assert_eq!(
        queries::get_model_context_limit(conn, "b", "m").unwrap(),
        Some(131072)
    );
    // Provider re-reports a different limit: latest wins.
    queries::upsert_model_context_limit(conn, "b", "m", 262144).unwrap();
    assert_eq!(
        queries::get_model_context_limit(conn, "b", "m").unwrap(),
        Some(262144)
    );
}

#[test]
fn message_order_is_deterministic_for_compaction_ordinals() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    conn.execute(
        "INSERT INTO conversations (id, title, model_id, backend_id, created_at, updated_at)
         VALUES ('c1', 'T', 'm', 'b', 1, 1)",
        [],
    )
    .unwrap();
    // Same created_at, ids inserted in reverse: `id` must break the tie.
    for id in ["b", "a"] {
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES (?1, 'c1', 'user', 'x', 5)",
            [id],
        )
        .unwrap();
    }
    let rows = queries::list_messages(conn, "c1").unwrap();
    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["a", "b"],
        "covered_count ordinals depend on deterministic message order"
    );
}
