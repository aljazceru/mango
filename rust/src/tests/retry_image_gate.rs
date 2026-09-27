/// Retry must not destroy history when the retried image turn is rejected.
///
/// Regression (codex review finding 4): RetryLastMessage used to delete the
/// last assistant + user messages BEFORE the send-time vision gate could
/// reject the turn; after switching the conversation to a text-only model, a
/// rejected retry left the exchange permanently deleted. The handler now
/// checks the conversation model before touching any rows.
use crate::persistence::queries::{insert_conversation, insert_message, ConversationRow, MessageRow};
use crate::persistence::Database;
use crate::{
    AppAction, EmbeddingStatus, FfiApp, NullBiometricProvider, NullEmbeddingProvider,
    NullKeychainProvider, NullLocalLlmProvider,
};

fn temp_dir(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "mango_retry_gate_{}_{}",
        tag,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir.to_str().unwrap().to_string()
}

fn make_app(dir: &str) -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        dir.into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    app
}

#[test]
fn retry_image_turn_on_text_only_model_preserves_history() {
    let dir = temp_dir("retry_gate");

    // A conversation whose model is text-only.
    let db = Database::open(&format!("{}/mango.db", dir)).unwrap();
    let conv_id = "conv-retry-gate".to_string();
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: conv_id.clone(),
            title: "Retry gate".into(),
            model_id: "gpt-oss-120b".into(),
            backend_id: "ppq-ai".into(),
            system_prompt: None,
            created_at: 1000,
            updated_at: 1000,
            tools_enabled: false,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-user".into(),
            conversation_id: conv_id.clone(),
            role: "user".into(),
            content: "describe\n\n[Image: photo.jpg]".into(),
            created_at: 1001,
            token_count: None,
            image_path: Some("/tmp/does-not-need-to-exist.jpg".into()),
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-assistant".into(),
            conversation_id: conv_id.clone(),
            role: "assistant".into(),
            content: "old answer".into(),
            created_at: 1002,
            token_count: None,
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    drop(db);

    // Startup after seeding so the actor loads the conversation list.
    let app = make_app(&dir);
    app.dispatch(AppAction::LoadConversation { conversation_id: conv_id.clone() });
    app.sync();

    app.dispatch(AppAction::RetryLastMessage);
    app.sync();

    let state = app.state();
    assert!(
        state
            .last_error
            .as_deref()
            .is_some_and(|e| e.contains("does not support image")),
        "retry must surface the vision gate error, got {:?}",
        state.last_error
    );
    assert_eq!(
        state.messages.len(),
        2,
        "the retried exchange must NOT be deleted on rejection"
    );

    let db = Database::open(&format!("{}/mango.db", dir)).unwrap();
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [&conv_id],
            |r| r.get(0),
        )
        .unwrap();
    drop(db);
    assert_eq!(count, 2, "rows must survive the rejected retry");
}

#[test]
fn retry_image_turn_with_empty_model_rejected_without_deletion() {
    let dir = temp_dir("retry_gate_empty_model");

    // Conversation with NO model set — the send path would fall back to the
    // backend default (text-only when none is known), so the retry must be
    // rejected before any history is deleted.
    let db = Database::open(&format!("{}/mango.db", dir)).unwrap();
    let conv_id = "conv-retry-empty".to_string();
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: conv_id.clone(),
            title: "Empty model".into(),
            model_id: String::new(),
            backend_id: String::new(),
            system_prompt: None,
            created_at: 1000,
            updated_at: 1000,
            tools_enabled: false,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-user-e".into(),
            conversation_id: conv_id.clone(),
            role: "user".into(),
            content: "[Image: photo.jpg]".into(),
            created_at: 1001,
            token_count: None,
            image_path: Some("/tmp/does-not-need-to-exist.jpg".into()),
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    drop(db);

    let app = make_app(&dir);
    app.dispatch(AppAction::LoadConversation { conversation_id: conv_id.clone() });
    app.sync();
    app.dispatch(AppAction::RetryLastMessage);
    app.sync();

    let state = app.state();
    assert!(
        state.last_error.is_some(),
        "undetermined model must reject the image retry, got {:?}",
        state.last_error
    );
    assert_eq!(
        state.messages.len(),
        1,
        "the image message must survive the rejected retry"
    );
}

#[test]
fn retry_image_turn_routed_to_text_only_hybrid_local_restores_history() {
    use crate::crypto::file_crypto::encrypt_file;
    use crate::routing::{HybridProfile, LocalPreprocessing, RoutingPolicy};

    // No-lock harness gives a live DEK so image rehydration succeeds and the
    // turn reaches routing; the local leg then rejects the image AFTER the
    // retry handler deleted the exchange — exactly the window the restore
    // covers. (Local inference stays off: the local route errors before any
    // generation attempt.)
    let dir = temp_dir("retry_gate_hybrid");
    let keychain_state =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
            (String, String),
            String,
        >::new()));
    struct MapKeychain(
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(String, String), String>>>,
    );
    impl crate::KeychainProvider for MapKeychain {
        fn store(&self, s: String, k: String, v: String) -> bool {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).insert((s, k), v);
            true
        }
        fn load(&self, s: String, k: String) -> Option<String> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&(s, k))
                .cloned()
        }
        fn delete(&self, s: String, k: String) -> bool {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&(s, k));
            true
        }
    }
    let mk_app = || {
        let app = FfiApp::new(
            dir.clone().into(),
            Box::new(MapKeychain(keychain_state.clone())),
            Box::new(NullEmbeddingProvider),
            crate::EmbeddingStatus::Active,
            Box::new(NullLocalLlmProvider),
            Box::new(NullBiometricProvider),
        );
        app.sync();
        app
    };

    // Onboard straight into no-lock mode (live DEK, encrypted main DB).
    let app = mk_app();
    for _ in 0..4 {
        app.dispatch(AppAction::NextOnboardingStep);
        app.sync();
    }
    app.dispatch(AppAction::CompleteOnboarding);
    app.sync();
    app.dispatch(AppAction::SetupNoLock);
    app.sync();
    let dek_hex = keychain_state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&("mango".to_string(), "dek".to_string()))
        .cloned()
        .expect("no-lock enrollment caches the DEK");
    drop(app);

    // Hybrid profile: vision remote, attachment escalation OFF so the image
    // retry routes to the local leg.
    let app = mk_app();
    app.dispatch(AppAction::SaveHybridProfile {
        profile: HybridProfile {
            id: "hyb1".into(),
            name: "Test hybrid".into(),
            local_backend_id: "local-qwen2_5-0_5b-instruct-q4_0".into(),
            local_model_id: "qwen2_5-0_5b-instruct-q4_0".into(),
            remote_backend_id: "ppq-ai".into(),
            remote_model_id: "private/kimi-k3".into(),
            policy: RoutingPolicy {
                escalate_if_attachment: false,
                ..RoutingPolicy::default()
            },
            preprocessing: LocalPreprocessing::default(),
        },
    });
    app.sync();

    // Real encrypted image so rehydrate_retry_image_attachment succeeds.
    let dek_arr: [u8; 32] = hex::decode(&dek_hex)
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
        .expect("valid dek hex");
    let image_plain = b"jpeg-bytes";
    let encrypted = encrypt_file(&dek_arr, image_plain);
    let image_path = format!("{dir}/images/msg-user-h.jpg.mgo1");
    std::fs::create_dir_all(format!("{dir}/images")).unwrap();
    std::fs::write(&image_path, &encrypted).unwrap();

    let conv_id = "conv-retry-hybrid".to_string();
    let db = crate::persistence::Database::open_encrypted(
        &format!("{dir}/mango.db"),
        &dek_hex,
    )
    .unwrap();
    crate::persistence::queries::insert_conversation(
        db.conn(),
        &ConversationRow {
            id: conv_id.clone(),
            title: "Hybrid retry".into(),
            model_id: "qwen2_5-0_5b-instruct-q4_0".into(),
            backend_id: "hybrid:hyb1".into(),
            system_prompt: None,
            created_at: 1000,
            updated_at: 1000,
            tools_enabled: false,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-earlier".into(),
            conversation_id: conv_id.clone(),
            role: "user".into(),
            content: "earlier turn".into(),
            created_at: 1000,
            token_count: Some(2),
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-assistant-early".into(),
            conversation_id: conv_id.clone(),
            role: "assistant".into(),
            content: "earlier answer".into(),
            created_at: 1000,
            token_count: Some(2),
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-user-h".into(),
            conversation_id: conv_id.clone(),
            role: "user".into(),
            content: "describe\n\n[Image: photo.jpg]".into(),
            created_at: 1001,
            token_count: Some(7),
            image_path: Some(image_path),
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-assistant-h".into(),
            conversation_id: conv_id.clone(),
            role: "assistant".into(),
            content: "old answer".into(),
            created_at: 1002,
            token_count: Some(2),
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    drop(db);

    let app2 = mk_app();
    app2.dispatch(AppAction::LoadConversation { conversation_id: conv_id.clone() });
    app2.sync();
    app2.dispatch(AppAction::RetryLastMessage);
    app2.sync();

    let state = app2.state();
    assert!(
        state.last_error.is_some(),
        "hybrid local-route retry must be rejected, got {:?}",
        state.last_error
    );
    assert_eq!(
        state.messages.len(),
        4,
        "the deleted exchange (incl. earlier turns and answers) must be restored"
    );
    let db = crate::persistence::Database::open_encrypted(
        &format!("{dir}/mango.db"),
        &dek_hex,
    )
    .unwrap();
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [&conv_id],
            |r| r.get(0),
        )
        .unwrap();
    drop(db);
    assert_eq!(count, 4, "rows must survive the rejected retry");
    assert!(
        state.messages.iter().any(|m| m.id == "msg-assistant-early"),
        "earlier assistant answers must never be collateral damage"
    );
}

#[test]
fn deferred_image_retry_restores_history_while_attestation_pending() {
    use crate::routing::{HybridProfile, LocalPreprocessing, RoutingPolicy};

    // Same no-lock harness as the hybrid test, but attachment escalation is ON:
    // the image retry routes to the (unattested) PPQ remote leg and is DEFERRED
    // pending attestation. History must be fully restored while deferred.
    let dir = temp_dir("retry_gate_deferred");
    let keychain_state =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
            (String, String),
            String,
        >::new()));
    struct MapKeychain(
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(String, String), String>>>,
    );
    impl crate::KeychainProvider for MapKeychain {
        fn store(&self, s: String, k: String, v: String) -> bool {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).insert((s, k), v);
            true
        }
        fn load(&self, s: String, k: String) -> Option<String> {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&(s, k))
                .cloned()
        }
        fn delete(&self, s: String, k: String) -> bool {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&(s, k));
            true
        }
    }
    let mk_app = || {
        let app = FfiApp::new(
            dir.clone().into(),
            Box::new(MapKeychain(keychain_state.clone())),
            Box::new(NullEmbeddingProvider),
            crate::EmbeddingStatus::Active,
            Box::new(NullLocalLlmProvider),
            Box::new(NullBiometricProvider),
        );
        app.sync();
        app
    };

    let app = mk_app();
    for _ in 0..4 {
        app.dispatch(AppAction::NextOnboardingStep);
        app.sync();
    }
    app.dispatch(AppAction::CompleteOnboarding);
    app.sync();
    app.dispatch(AppAction::SetupNoLock);
    app.sync();
    let dek_hex = keychain_state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&("mango".to_string(), "dek".to_string()))
        .cloned()
        .expect("no-lock enrollment caches the DEK");
    drop(app);

    let app = mk_app();
    app.dispatch(AppAction::SaveHybridProfile {
        profile: HybridProfile {
            id: "hyb2".into(),
            name: "Escalate".into(),
            local_backend_id: "local-qwen2_5-0_5b-instruct-q4_0".into(),
            local_model_id: "qwen2_5-0_5b-instruct-q4_0".into(),
            remote_backend_id: "ppq-ai".into(),
            remote_model_id: "private/kimi-k3".into(),
            policy: RoutingPolicy::default(),
            preprocessing: LocalPreprocessing::default(),
        },
    });
    app.sync();

    let dek_arr: [u8; 32] = hex::decode(&dek_hex)
        .ok()
        .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
        .expect("valid dek hex");
    let encrypted = crate::crypto::file_crypto::encrypt_file(&dek_arr, b"jpeg-bytes");
    let image_path = format!("{dir}/images/msg-user-d.jpg.mgo1");
    std::fs::create_dir_all(format!("{dir}/images")).unwrap();
    std::fs::write(&image_path, &encrypted).unwrap();

    let conv_id = "conv-retry-deferred".to_string();
    let db = crate::persistence::Database::open_encrypted(
        &format!("{dir}/mango.db"),
        &dek_hex,
    )
    .unwrap();
    crate::persistence::queries::insert_conversation(
        db.conn(),
        &ConversationRow {
            id: conv_id.clone(),
            title: "Deferred retry".into(),
            model_id: "qwen2_5-0_5b-instruct-q4_0".into(),
            backend_id: "hybrid:hyb2".into(),
            system_prompt: None,
            created_at: 1000,
            updated_at: 1000,
            tools_enabled: false,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-earlier-d".into(),
            conversation_id: conv_id.clone(),
            role: "user".into(),
            content: "earlier turn".into(),
            created_at: 1000,
            token_count: Some(2),
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-assistant-early-d".into(),
            conversation_id: conv_id.clone(),
            role: "assistant".into(),
            content: "earlier answer".into(),
            created_at: 1000,
            token_count: Some(2),
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-user-d".into(),
            conversation_id: conv_id.clone(),
            role: "user".into(),
            content: "describe\n\n[Image: photo.jpg]".into(),
            created_at: 1001,
            token_count: Some(7),
            image_path: Some(image_path),
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-assistant-d".into(),
            conversation_id: conv_id.clone(),
            role: "assistant".into(),
            content: "old answer".into(),
            created_at: 1002,
            token_count: Some(2),
            image_path: None,
            route_backend_id: None,
            route_model_id: None,
            route_decision: None,
            route_reason: None,
            route_provider_name: None,
            route_tee_label: None,
            route_tee_verified: None,
        },
    )
    .unwrap();
    drop(db);

    let app2 = mk_app();
    app2.dispatch(AppAction::LoadConversation { conversation_id: conv_id.clone() });
    app2.sync();
    app2.dispatch(AppAction::RetryLastMessage);
    app2.sync();

    let state = app2.state();
    assert_eq!(
        state.messages.len(),
        4,
        "history must be fully restored while the retry waits for attestation"
    );
    assert!(
        matches!(
            state.busy_state,
            crate::BusyState::Loading { .. }
        ),
        "the retry should be deferred (attestation pending), got {:?}",
        state.busy_state
    );
    assert!(
        state.messages.iter().any(|m| m.id == "msg-assistant-early-d"),
        "earlier assistant answers must survive the deferred retry"
    );
}
