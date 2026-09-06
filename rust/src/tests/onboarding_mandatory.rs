use crate::crypto::bootstrap_db::{AuthParams, BootstrapDb};
use crate::crypto::key_derivation::{
    derive_kek, generate_dek, generate_salt, wrap_dek, DEFAULT_ITERATIONS, DEFAULT_MEMORY_KIB,
    DEFAULT_PARALLELISM,
};
use crate::persistence::queries::{
    insert_backend, insert_message, set_setting, BackendRow, ConversationRow, MessageRow,
};
use crate::persistence::Database;
/// Mandatory PIN enrollment after onboarding (Finding 2).
///
/// These tests use real file-backed databases and verify that:
/// - CompleteOnboarding and SkipOnboarding route through PinSetup when no auth is set.
/// - The first conversation is created exactly once after PIN enrollment.
/// - Restart during pending enrollment resumes at PinSetup without duplicates.
/// - Navigation/send actions are blocked while enrollment is incomplete.
use crate::{
    AppAction, EmbeddingStatus, FfiApp, NullBiometricProvider, NullEmbeddingProvider,
    NullKeychainProvider, NullLocalLlmProvider, Screen,
};

fn temp_dir(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "mango_onboard_test_{}_{}",
        tag,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir.to_str().unwrap().to_string()
}

fn test_pin() -> String {
    "1234".into()
}

fn make_file_app(dir: &str) -> std::sync::Arc<FfiApp> {
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

// ── CompleteOnboarding routes through PinSetup ───────────────────────────────

#[test]
fn test_complete_onboarding_routes_to_pin_setup_file_backed() {
    let dir = temp_dir("complete_gate");
    let app = make_file_app(&dir);

    // Advance to the end of the onboarding wizard.
    for _ in 0..4 {
        app.dispatch(AppAction::NextOnboardingStep);
        app.sync();
    }

    app.dispatch(AppAction::CompleteOnboarding);
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "CompleteOnboarding with no auth must route to PinSetup, got {:?}",
        state.router.current_screen
    );
    assert!(
        state.conversations.is_empty(),
        "first conversation should not be created before PIN enrollment"
    );

    // Finish enrollment.
    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::Chat { .. }),
        "after enrollment should land on first Chat, got {:?}",
        state.router.current_screen
    );
    assert_eq!(
        state.conversations.len(),
        1,
        "exactly one first conversation"
    );
    assert!(
        state.show_first_chat_placeholder,
        "welcome placeholder should be set"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── SkipOnboarding routes through PinSetup ───────────────────────────────────

#[test]
fn test_skip_onboarding_routes_to_pin_setup_file_backed() {
    let dir = temp_dir("skip_gate");
    let app = make_file_app(&dir);

    app.dispatch(AppAction::SkipOnboarding);
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "SkipOnboarding with no auth must route to PinSetup, got {:?}",
        state.router.current_screen
    );

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::Home),
        "after enrollment should land on Home for skip, got {:?}",
        state.router.current_screen
    );
    assert!(state.auth_initialized, "auth should be initialized");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Restart mid-enrollment resumes PinSetup with no duplicate conversation ─────

#[test]
fn test_restart_resumes_pending_enrollment_no_duplicate_conversation() {
    let dir = temp_dir("restart_resume");
    {
        let app = make_file_app(&dir);
        for _ in 0..4 {
            app.dispatch(AppAction::NextOnboardingStep);
            app.sync();
        }
        app.dispatch(AppAction::CompleteOnboarding);
        app.sync();
        assert!(matches!(
            app.state().router.current_screen,
            Screen::PinSetup
        ));
        // Simulate process death by dropping the app without completing enrollment.
    }

    // New process over the same data directory.
    let app = make_file_app(&dir);
    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "restart should resume at PinSetup, got {:?}",
        state.router.current_screen
    );

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert_eq!(
        state.conversations.len(),
        1,
        "must create exactly one first conversation"
    );
    assert!(matches!(state.router.current_screen, Screen::Chat { .. }));

    // Drop and restart again; the continuation should already be cleared.
    let conv_id = state
        .conversations
        .first()
        .map(|c| c.id.clone())
        .expect("first conversation id");
    {
        let _ = app;
    }

    let app2 = make_file_app(&dir);
    app2.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app2.sync();
    let state = app2.state();
    assert_eq!(state.conversations.len(), 1, "no duplicate after unlock");
    assert_eq!(
        state.conversations.first().map(|c| c.id.as_str()),
        Some(conv_id.as_str())
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Navigation and chat actions blocked while enrollment pending ───────────────

#[test]
fn test_enrollment_gate_blocks_navigation_and_chat() {
    let dir = temp_dir("enrollment_gate");
    let app = make_file_app(&dir);

    for _ in 0..4 {
        app.dispatch(AppAction::NextOnboardingStep);
        app.sync();
    }
    app.dispatch(AppAction::CompleteOnboarding);
    app.sync();
    assert!(matches!(
        app.state().router.current_screen,
        Screen::PinSetup
    ));

    // Attempts to bypass via navigation/send should be redirected to PinSetup.
    app.dispatch(AppAction::PushScreen {
        screen: Screen::Home,
    });
    app.sync();
    assert!(matches!(
        app.state().router.current_screen,
        Screen::PinSetup
    ));

    app.dispatch(AppAction::NewConversation);
    app.sync();
    assert!(matches!(
        app.state().router.current_screen,
        Screen::PinSetup
    ));
    assert!(app.state().conversations.is_empty());

    app.dispatch(AppAction::SendMessage {
        text: "hello".into(),
        force_role: None,
    });
    app.sync();
    assert!(matches!(
        app.state().router.current_screen,
        Screen::PinSetup
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

// ── In-memory mode still allows pre-enrollment Chat for test compatibility ─────

#[test]
fn test_in_memory_complete_onboarding_goes_directly_to_chat() {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    for _ in 0..4 {
        app.dispatch(AppAction::NextOnboardingStep);
        app.sync();
    }

    app.dispatch(AppAction::CompleteOnboarding);
    app.sync();

    // In-memory test mode bypasses the mandatory enrollment gate.
    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::Chat { .. }),
        "in-memory CompleteOnboarding should still reach Chat, got {:?}",
        state.router.current_screen
    );
    assert_eq!(state.conversations.len(), 1);
}

// ── R3: transactionally persist first-run continuation before clearing marker ──

fn db_path(dir: &str) -> String {
    format!("{}/mango.db", dir)
}

fn bootstrap_path(dir: &str) -> String {
    format!("{}/mango_auth.db", dir)
}

fn make_active_auth(pin: &str) -> (AuthParams, String, [u8; 32]) {
    let dek = generate_dek();
    let salt = generate_salt();
    let kek = derive_kek(
        pin.as_bytes(),
        &salt,
        DEFAULT_MEMORY_KIB,
        DEFAULT_ITERATIONS,
        DEFAULT_PARALLELISM,
    )
    .expect("derive kek");
    let wrapped = wrap_dek(&kek, &dek);
    let dek_hex: String = dek.iter().map(|b| format!("{:02x}", b)).collect();
    let params = AuthParams {
        salt: salt.to_vec(),
        wrapped_dek: wrapped,
        duress_hash: None,
        kdf_memory_kib: DEFAULT_MEMORY_KIB,
        kdf_iterations: DEFAULT_ITERATIONS,
        kdf_parallelism: DEFAULT_PARALLELISM,
    };
    (params, dek_hex, dek)
}

fn seed_main_db_for_continuation(dir: &str, pin: &str) -> (String, String, AuthParams, [u8; 32]) {
    let path = db_path(dir);
    let db = Database::open(&path).expect("open plaintext");

    // Seed a backend so the first-run continuation has a default model.
    let backend = BackendRow {
        id: "test-backend".into(),
        name: "Test".into(),
        base_url: "https://test".into(),
        model_list: "[\"test-model\"]".into(),
        tee_type: "none".into(),
        display_order: 0,
        is_active: 1,
        created_at: crate::now_secs(),
        max_concurrent_requests: 1,
        supports_tool_use: false,
    };
    insert_backend(db.conn(), &backend).expect("insert backend");
    set_setting(db.conn(), "default_backend_id", "test-backend").expect("set default backend");
    set_setting(db.conn(), "default_model_id", "test-model").expect("set default model");
    set_setting(db.conn(), "has_completed_onboarding", "false").expect("set incomplete");
    drop(db);

    let (params, dek_hex, dek) = make_active_auth(pin);
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    (path, dek_hex, params, dek)
}

#[test]
fn test_first_run_continuation_is_idempotent() {
    let dir = temp_dir("first_run_idempotent");
    let (path, dek_hex, params, _dek) = seed_main_db_for_continuation(&dir, &test_pin());
    let conv_id = uuid::Uuid::new_v4().to_string();

    // Pre-insert the conversation that the continuation would create.
    {
        let db = Database::open_encrypted(&path, &dek_hex).expect("open encrypted");
        let now = crate::now_secs();
        let row = ConversationRow {
            id: conv_id.clone(),
            title: "New Conversation".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: now,
            updated_at: now,
            tools_enabled: false,
        };
        crate::persistence::queries::insert_conversation(db.conn(), &row).expect("insert");
    }

    // Bootstrap with active auth and a continuation pointing at the existing conversation.
    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_auth_params(&params).expect("write active auth");
    bs.set_pending_first_run("complete", Some(&conv_id))
        .expect("write continuation");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    app.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::Chat { .. }),
        "continuation should reach Chat, got {:?}",
        state.router.current_screen
    );
    assert_eq!(
        state.conversations.len(),
        1,
        "must not create duplicate conversation"
    );
    assert_eq!(
        state.conversations.first().map(|c| c.id.as_str()),
        Some(conv_id.as_str())
    );

    // The continuation marker must be cleared.
    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    assert!(bs.read_pending_first_run().unwrap_or_default().is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_first_run_insert_failure_retains_continuation() {
    let dir = temp_dir("first_run_insert_fail");
    let (path, dek_hex, params, _dek) = seed_main_db_for_continuation(&dir, &test_pin());
    let conv_id = uuid::Uuid::new_v4().to_string();

    // Install a trigger that aborts any insert into conversations.
    {
        let db = Database::open_encrypted(&path, &dek_hex).expect("open encrypted");
        db.conn()
            .execute_batch(
                "CREATE TRIGGER block_conversation_insert
                 BEFORE INSERT ON conversations
                 BEGIN
                     SELECT RAISE(ABORT, 'injected');
                 END;",
            )
            .expect("create trigger");
    }

    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_auth_params(&params).expect("write active auth");
    bs.set_pending_first_run("complete", Some(&conv_id))
        .expect("write continuation");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    app.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app.sync();

    let state = app.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Chat { .. }),
        "insert failure must not route to Chat"
    );
    assert!(state.conversations.is_empty());
    assert!(
        state.last_error.is_some() || state.toast.is_some(),
        "failure must be observable"
    );

    // Bootstrap must still have the continuation.
    {
        let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
        assert!(
            bs.read_pending_first_run().unwrap_or_default().is_some(),
            "continuation marker must survive insert failure"
        );
    }

    // Drop the app, remove the trigger, and retry.
    {
        let _ = app;
    }
    {
        let db = Database::open_encrypted(&path, &dek_hex).expect("open encrypted");
        db.conn()
            .execute_batch("DROP TRIGGER block_conversation_insert")
            .expect("drop trigger");
    }

    let app2 = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app2.sync();

    app2.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app2.sync();

    let state = app2.state();
    assert!(
        matches!(state.router.current_screen, Screen::Chat { .. }),
        "retry after removing trigger should reach Chat, got {:?}",
        state.router.current_screen
    );
    assert_eq!(state.conversations.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_first_run_set_setting_failure_retains_continuation() {
    let dir = temp_dir("first_run_setting_fail");
    let (path, dek_hex, params, _dek) = seed_main_db_for_continuation(&dir, &test_pin());
    let conv_id = uuid::Uuid::new_v4().to_string();

    // Install a trigger that aborts any insert or update to settings.
    {
        let db = Database::open_encrypted(&path, &dek_hex).expect("open encrypted");
        db.conn()
            .execute_batch(
                "CREATE TRIGGER block_settings_write
                 BEFORE INSERT ON settings
                 BEGIN
                     SELECT RAISE(ABORT, 'injected');
                 END;
                 CREATE TRIGGER block_settings_update
                 BEFORE UPDATE ON settings
                 BEGIN
                     SELECT RAISE(ABORT, 'injected');
                 END;",
            )
            .expect("create trigger");
    }

    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_auth_params(&params).expect("write active auth");
    bs.set_pending_first_run("complete", Some(&conv_id))
        .expect("write continuation");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    app.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app.sync();

    let state = app.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Chat { .. }),
        "settings write failure must not route to Chat"
    );
    assert!(state.conversations.is_empty());

    // Continuation must still be pending.
    {
        let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
        assert!(
            bs.read_pending_first_run().unwrap_or_default().is_some(),
            "continuation marker must survive settings write failure"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ── R3 follow-up: a committed-but-uncleared continuation must still hydrate ───
//    full chat state (conversation id + messages) before routing to Chat.

#[test]
fn test_first_run_restart_hydrates_existing_conversation_state() {
    let dir = temp_dir("first_run_hydrate");
    let (path, dek_hex, params, _dek) = seed_main_db_for_continuation(&dir, &test_pin());
    let conv_id = uuid::Uuid::new_v4().to_string();

    // Simulate a crash *after* the continuation transaction committed but
    // *before* pending_first_run was cleared: the conversation row and a
    // message already exist in the encrypted DB.
    {
        let db = Database::open_encrypted(&path, &dek_hex).expect("open encrypted");
        let now = crate::now_secs();
        let row = ConversationRow {
            id: conv_id.clone(),
            title: "Restarted Chat".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: now,
            updated_at: now,
            tools_enabled: false,
        };
        crate::persistence::queries::insert_conversation(db.conn(), &row).expect("insert");
        insert_message(
            db.conn(),
            &MessageRow {
                id: uuid::Uuid::new_v4().to_string(),
                conversation_id: conv_id.clone(),
                role: "user".into(),
                content: "pre-restart message".into(),
                created_at: now,
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
        .expect("insert message");
    }

    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_auth_params(&params).expect("write active auth");
    bs.set_pending_first_run("complete", Some(&conv_id))
        .expect("write continuation");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    assert!(matches!(app.state().router.current_screen, Screen::Locked));

    app.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app.sync();

    let state = app.state();
    // Router must point at the persisted conversation AND the coherent chat
    // state must be hydrated from the DB — not left stale because the row
    // already existed.
    assert_eq!(
        state.router.current_screen,
        Screen::Chat {
            conversation_id: conv_id.clone()
        },
        "router must point at the persisted conversation"
    );
    assert_eq!(
        state.current_conversation_id.as_deref(),
        Some(conv_id.as_str()),
        "current_conversation_id must be established from the persisted row"
    );
    assert_eq!(
        state.messages.len(),
        1,
        "messages must be hydrated from the persisted conversation"
    );
    assert_eq!(state.messages[0].content, "pre-restart message");
    assert!(
        !state.show_first_chat_placeholder,
        "welcome placeholder must not show over a hydrated conversation"
    );

    // The marker is cleared after successful hydration.
    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("reopen bootstrap");
    assert!(bs.read_pending_first_run().unwrap_or_default().is_none());
    drop(bs);

    // A save action on the hydrated conversation must be reachable and persist.
    app.dispatch(AppAction::SetSystemPrompt {
        prompt: Some("be terse".into()),
    });
    app.sync();
    let state = app.state();
    assert_eq!(
        state
            .conversations
            .iter()
            .find(|c| c.id == conv_id)
            .and_then(|c| c.system_prompt.as_deref()),
        Some("be terse"),
        "SetSystemPrompt must reach the hydrated conversation"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
