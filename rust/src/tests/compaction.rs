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
    app.test_verify_backend_attestation("tinfoil", "AmdSevSnp");
    app
}

fn cancel_stream(app: &FfiApp) {
    app.test_send_internal(InternalEvent::StreamCancelled);
    app.sync();
}

// ── Context overflow → compaction offer ───────────────────────────────────────

#[test]
fn overflow_stream_error_offers_compaction() {
    let app = make_app();
    app.dispatch(AppAction::NewConversation);
    app.sync();
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
    app.dispatch(AppAction::NewConversation);
    app.sync();
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
        cancel_stream(&app);
    }
    let conv_id = app
        .state()
        .current_conversation_id
        .clone()
        .expect("conversation exists");
    app.dispatch(AppAction::LoadConversation {
        conversation_id: conv_id.clone(),
    });
    app.sync();
    let before = app.state().messages.len();
    assert!(before >= 4, "expected persisted messages, got {before}");

    app.test_send_internal(InternalEvent::ConversationCompactionComplete {
        conversation_id: conv_id.clone(),
        covered_count: 2,
        expected_covered_ids: app
            .state()
            .messages
            .iter()
            .take(2)
            .map(|m| m.id.clone())
            .collect(),
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

    assert!(queries::get_model_context_limit(conn, "b", "m")
        .unwrap()
        .is_none());
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

#[test]
fn deleting_covered_rows_invalidates_summary_but_tail_deletion_keeps_it() {
    for truncate in [false, true] {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO conversations (id, title, model_id, backend_id, created_at, updated_at)
             VALUES ('c1', 'T', 'm', 'b', 1, 1)",
            [],
        )
        .unwrap();
        for (id, at) in [("a", 1), ("b", 1), ("c", 2), ("d", 3)] {
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at)
                 VALUES (?1, 'c1', 'user', 'text', ?2)",
                rusqlite::params![id, at],
            )
            .unwrap();
        }
        queries::update_conversation_compaction(conn, "c1", Some("summary"), 3).unwrap();
        if truncate {
            queries::delete_messages_after(conn, "c1", 2).unwrap();
        } else {
            queries::delete_message(conn, "d").unwrap();
        }
        assert!(queries::get_conversation_compaction(conn, "c1")
            .unwrap()
            .is_some());
        if truncate {
            queries::delete_messages_after(conn, "c1", 1).unwrap();
        } else {
            queries::delete_message(conn, "b").unwrap();
        }
        assert!(queries::get_conversation_compaction(conn, "c1")
            .unwrap()
            .is_none());
        let covered: i64 = conn
            .query_row(
                "SELECT compaction_covered_count FROM conversations WHERE id = 'c1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(covered, 0);
    }
}

#[test]
fn editing_a_covered_prefix_clears_summary_and_rejects_inflight_completion() {
    let app = make_app();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    for i in 0..4 {
        app.dispatch(AppAction::SendMessage {
            text: format!("turn {i}"),
            force_role: None,
        });
        app.sync();
        cancel_stream(&app);
    }
    let conversation_id = app.state().current_conversation_id.clone().unwrap();
    app.dispatch(AppAction::LoadConversation {
        conversation_id: conversation_id.clone(),
    });
    app.sync();
    let before = app.state();
    let expected_covered_ids: Vec<String> = before
        .messages
        .iter()
        .take(2)
        .map(|m| m.id.clone())
        .collect();
    app.test_send_internal(InternalEvent::ConversationCompactionComplete {
        conversation_id: conversation_id.clone(),
        covered_count: 2,
        expected_covered_ids: expected_covered_ids.clone(),
        retry_after: false,
        result: Ok("original summary".into()),
    });
    app.sync();
    assert!(app.state().compaction.is_some());
    app.dispatch(AppAction::EditMessage {
        message_id: expected_covered_ids[0].clone(),
        new_text: "replacement".into(),
    });
    app.sync();
    assert!(app.state().compaction.is_none());
    app.test_send_internal(InternalEvent::ConversationCompactionComplete {
        conversation_id: conversation_id.clone(),
        covered_count: 2,
        expected_covered_ids,
        retry_after: true,
        result: Ok("stale summary".into()),
    });
    app.sync();
    assert!(app.state().compaction.is_none());
    cancel_stream(&app);
    app.dispatch(AppAction::LoadConversation { conversation_id });
    app.sync();
    assert!(
        app.state().compaction.is_none(),
        "invalidated state must also be cleared in SQLite"
    );
}

#[test]
fn background_compaction_does_not_retry_or_change_visible_conversation_state() {
    let app = make_app();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    for i in 0..4 {
        app.dispatch(AppAction::SendMessage {
            text: format!("turn {i}"),
            force_role: None,
        });
        app.sync();
        cancel_stream(&app);
    }
    let conversation_id = app.state().current_conversation_id.clone().unwrap();
    app.dispatch(AppAction::LoadConversation {
        conversation_id: conversation_id.clone(),
    });
    app.sync();
    let source = app.state();
    let expected_covered_ids: Vec<String> = source
        .messages
        .iter()
        .take(2)
        .map(|m| m.id.clone())
        .collect();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    app.dispatch(AppAction::SendMessage {
        text: "other conversation".into(),
        force_role: None,
    });
    app.sync();
    cancel_stream(&app);
    let visible = app.state();
    for result in [
        Ok("background summary".into()),
        Err("background failure".into()),
    ] {
        app.test_send_internal(InternalEvent::ConversationCompactionComplete {
            conversation_id: conversation_id.clone(),
            covered_count: 2,
            expected_covered_ids: expected_covered_ids.clone(),
            retry_after: true,
            result,
        });
        app.sync();
        let after = app.state();
        assert_eq!(
            after.current_conversation_id,
            visible.current_conversation_id
        );
        assert_eq!(
            after
                .messages
                .iter()
                .map(|m| (&m.id, &m.content))
                .collect::<Vec<_>>(),
            visible
                .messages
                .iter()
                .map(|m| (&m.id, &m.content))
                .collect::<Vec<_>>(),
        );
        assert_eq!(after.busy_state, visible.busy_state);
        assert_eq!(after.last_error, visible.last_error);
        assert_eq!(after.compaction, visible.compaction);
        assert_eq!(after.compaction_offered, visible.compaction_offered);
        assert_eq!(after.toast, visible.toast);
    }
    app.dispatch(AppAction::LoadConversation { conversation_id });
    app.sync();
    assert!(
        app.state().compaction.is_some(),
        "background summary is still persisted for its owner"
    );
}

#[test]
fn queued_compaction_retry_is_dropped_after_navigation() {
    let app = make_app();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    let conversation_id = app.state().current_conversation_id.clone().unwrap();
    app.dispatch(AppAction::NewConversation);
    app.sync();
    let before = app.state();
    app.core_tx
        .send(crate::CoreMsg::ConversationAction {
            conversation_id,
            action: AppAction::RetryLastMessage,
        })
        .unwrap();
    app.sync();
    let after = app.state();
    assert_eq!(
        after.current_conversation_id,
        before.current_conversation_id
    );
    assert_eq!(
        after
            .messages
            .iter()
            .map(|m| (&m.id, &m.content))
            .collect::<Vec<_>>(),
        before
            .messages
            .iter()
            .map(|m| (&m.id, &m.content))
            .collect::<Vec<_>>(),
    );
    assert_eq!(after.last_error, before.last_error);
    assert_eq!(after.busy_state, before.busy_state);
}

#[test]
fn rag_prompt_excludes_unattached_documents_and_keeps_rowid_scores() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    for id in ["attached", "unattached"] {
        conn.execute(
            "INSERT INTO documents (id, name, format, size_bytes, ingestion_date, chunk_count)
             VALUES (?1, ?1, 'txt', 1, 1, 1)",
            [id],
        )
        .unwrap();
    }
    let far = queries::insert_chunk(conn, "attached", 0, "attached far", 0).unwrap();
    let near = queries::insert_chunk(conn, "attached", 1, "attached near", 0).unwrap();
    let secret = queries::insert_chunk(conn, "unattached", 0, "UNATTACHED SECRET", 0).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let index = crate::rag::VectorIndex::new(temp.path().to_str().unwrap(), None).unwrap();
    let mut query = vec![0.0; crate::embedding::EMBEDDING_DIM];
    query[0] = 1.0;
    let mut orthogonal = vec![0.0; crate::embedding::EMBEDDING_DIM];
    orthogonal[1] = 1.0;
    index.add(far as u64, &orthogonal).unwrap();
    index.add(near as u64, &query).unwrap();
    index.add(secret as u64, &query).unwrap();
    let chunks =
        crate::retrieve_attached_chunks(conn, &index, &query, &["attached".into()]).unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].text, "attached near");
    assert!(chunks[0].score < 0.01);
    assert_eq!(chunks[1].text, "attached far");
    assert!(chunks[1].score > 0.9);
    let prompt = crate::rag::build_system_with_context("base prompt", &chunks);
    assert!(prompt.contains("attached near"));
    assert!(prompt.contains("attached far"));
    assert!(!prompt.contains("UNATTACHED SECRET"));
    assert!(crate::retrieve_attached_chunks(conn, &index, &query, &[])
        .unwrap()
        .is_empty());
}

fn compaction_actor() -> crate::ActorState {
    let mut actor = crate::image_red_tests::default_actor_state_for_image_tests();
    let backend = |id: &str, url: &str, model: &str, tee_type| crate::llm::BackendConfig {
        id: id.into(),
        name: id.into(),
        base_url: url.into(),
        api_key: String::new(),
        models: vec![model.into()],
        tee_type,
        max_concurrent_requests: 1,
        supports_tool_use: false,
    };
    actor.backends = vec![
        backend(
            "local",
            "local://model",
            "local-model",
            crate::llm::TeeType::Unknown,
        ),
        // An unsupported URL scheme keeps failed-preflight re-attestation off the network.
        backend(
            "remote",
            "invalid://compaction",
            "remote-model",
            crate::llm::TeeType::IntelTdx,
        ),
    ];
    actor.app_state.hybrid_profiles.push(crate::HybridProfile {
        id: "profile".into(),
        name: "Hybrid".into(),
        local_backend_id: "local".into(),
        local_model_id: "local-model".into(),
        remote_backend_id: "remote".into(),
        remote_model_id: "remote-model".into(),
        policy: crate::RoutingPolicy::default(),
        preprocessing: crate::LocalPreprocessing::default(),
    });
    actor
        .app_state
        .attestation_statuses
        .push(crate::AttestationStatusEntry {
            backend_id: "remote".into(),
            status: crate::AttestationStatus::Verified {
                shape: None,
                freshness: None,
                orchestrated_components: None,
            },
        });
    actor
        .attestation_expires_at
        .insert("remote".into(), crate::unix_time_secs() + 60);
    actor
        .attested_tls_public_keys
        .insert("remote".into(), "pin".into());
    actor
}

fn resolved_compaction(actor: &crate::ActorState, backend: &str, model: &str) -> (String, String) {
    let (tx, _rx) = flume::unbounded();
    let (backend, model) =
        crate::compaction_backend_and_model(actor, Some(backend), Some(model), tx)
            .expect("compaction target");
    (backend.id, model)
}

#[test]
fn hybrid_compaction_uses_the_attested_remote_profiles_model() {
    let actor = compaction_actor();
    assert_eq!(
        resolved_compaction(&actor, "hybrid:profile", "local-model"),
        ("remote".into(), "remote-model".into()),
    );
}

#[test]
fn hybrid_compaction_falls_back_locally_when_remote_preflight_fails() {
    let mut actor = compaction_actor();
    actor.attested_tls_public_keys.clear();
    assert_eq!(
        resolved_compaction(&actor, "hybrid:profile", "remote-model"),
        ("local".into(), "local-model".into()),
        "a verified status without a TLS pin is insufficient",
    );

    actor
        .attested_tls_public_keys
        .insert("remote".into(), "pin".into());
    actor
        .attestation_expires_at
        .insert("remote".into(), crate::unix_time_secs().saturating_sub(1));
    assert_eq!(
        resolved_compaction(&actor, "hybrid:profile", "remote-model"),
        ("local".into(), "local-model".into()),
        "expired attestation must not authorize remote history disclosure",
    );

    actor
        .attestation_expires_at
        .insert("remote".into(), crate::unix_time_secs() + 60);
    actor.app_state.attestation_statuses.clear();
    assert_eq!(
        resolved_compaction(&actor, "hybrid:profile", "remote-model"),
        ("local".into(), "local-model".into()),
        "a fresh pin alone must not bypass attestation status",
    );
}

#[test]
fn hybrid_compaction_never_uses_an_unattested_remote_leg() {
    let mut actor = compaction_actor();
    actor.backends[1].tee_type = crate::llm::TeeType::Unknown;
    assert_eq!(
        resolved_compaction(&actor, "hybrid:profile", "remote-model"),
        ("local".into(), "local-model".into()),
    );

    actor.backends.pop();
    assert_eq!(
        resolved_compaction(&actor, "hybrid:profile", "remote-model"),
        ("local".into(), "local-model".into()),
        "a missing remote leg must not prevent local compaction",
    );
}

#[test]
fn compaction_preserves_single_backend_model_selection_without_cross_provider_fallback() {
    let actor = compaction_actor();
    assert_eq!(
        resolved_compaction(&actor, "local", "conversation-model"),
        ("local".into(), "conversation-model".into()),
    );
    assert_eq!(
        resolved_compaction(&actor, "local", ""),
        ("local".into(), "local-model".into()),
    );
    for backend in ["missing", "hybrid:missing"] {
        let (tx, _rx) = flume::unbounded();
        assert!(crate::compaction_backend_and_model(&actor, Some(backend), None, tx).is_none());
    }
}
