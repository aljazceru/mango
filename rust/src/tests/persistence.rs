use crate::persistence::queries::{
    self, get_active_backend_id, insert_agent_session, insert_agent_step, insert_conversation,
    insert_message, list_agent_sessions, list_agent_steps, list_backends, list_conversations,
    list_messages, AgentSessionRow, AgentStepRow, ConversationRow, MessageRow,
};
use crate::persistence::Database;
use crate::KeychainProvider;
use crate::{EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};

fn wait_until<F>(timeout: std::time::Duration, mut predicate: F) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if predicate() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    predicate()
}

// ── Migration tests ───────────────────────────────────────────────────────────

/// Verify that opening a v1-state database applies MIGRATION_V2 and data survives.
///
/// This test manually creates a v1 database (bypassing Database::open to avoid
/// auto-running v2), pre-populates with test data, then re-opens via Database::open
/// which applies MIGRATION_V2. Verifies user_version==2, all data survives, and
/// the new idx_agent_steps_session_order index exists.
#[test]
fn test_migration_v1_to_v2() {
    use crate::persistence::schema::MIGRATION_V1;
    let tmp = std::env::temp_dir().join(format!("test_v1v2_{}.db", uuid::Uuid::new_v4()));
    let path = tmp.to_str().unwrap();

    // Step 1: Manually create a v1 database (only MIGRATION_V1)
    {
        let mut conn = rusqlite::Connection::open(path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let tx = conn.transaction().unwrap();
        tx.execute_batch(MIGRATION_V1).unwrap();
        tx.pragma_update(None, "user_version", 1i32).unwrap();
        tx.commit().unwrap();

        // Populate with test data at v1
        conn.execute(
            "INSERT INTO conversations (id, title, model_id, backend_id, created_at, updated_at)
             VALUES ('c1', 'Test Conv', 'model1', 'tinfoil', 100, 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES ('m1', 'c1', 'user', 'hello v1', 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_sessions (id, title, backend_id, created_at, updated_at)
             VALUES ('as1', 'Agent Test', 'tinfoil', 100, 100)",
            [],
        )
        .unwrap();
    }

    // Step 2: Reopen via Database::open -- should apply MIGRATION_V2
    {
        let db = Database::open(path).unwrap();
        let version: i32 = db
            .conn()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(
            version, 24,
            "user_version should be 24 after all migrations"
        );

        // Verify pre-existing data survived
        let conv_title: String = db
            .conn()
            .query_row(
                "SELECT title FROM conversations WHERE id = 'c1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(conv_title, "Test Conv");

        let msg_content: String = db
            .conn()
            .query_row("SELECT content FROM messages WHERE id = 'm1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(msg_content, "hello v1");

        let sess_title: String = db
            .conn()
            .query_row(
                "SELECT title FROM agent_sessions WHERE id = 'as1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sess_title, "Agent Test");

        // Verify new index exists
        let idx_exists: bool = db
            .conn()
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_agent_steps_session_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            idx_exists,
            "idx_agent_steps_session_order index should exist after v2 migration"
        );
    }

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_migration_v1_tables() {
    let db = Database::open(":memory:").unwrap();
    let tables: Vec<String> = db
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        tables.contains(&"conversations".to_string()),
        "conversations table missing"
    );
    assert!(
        tables.contains(&"messages".to_string()),
        "messages table missing"
    );
    assert!(
        tables.contains(&"backends".to_string()),
        "backends table missing"
    );
    assert!(
        tables.contains(&"agent_sessions".to_string()),
        "agent_sessions table missing"
    );
    assert!(
        tables.contains(&"agent_steps".to_string()),
        "agent_steps table missing"
    );
    assert!(
        tables.contains(&"attestation_cache".to_string()),
        "attestation_cache table missing"
    );
}

#[test]
fn test_migration_version_increments() {
    let db = Database::open(":memory:").unwrap();
    let version: i32 = db
        .conn()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        version, 24,
        "user_version should be 24 after all migrations"
    );
}

#[test]
fn test_migration_idempotent() {
    let tmp = std::env::temp_dir().join(format!("test_idem_{}.db", uuid::Uuid::new_v4()));
    let path = tmp.to_str().unwrap();
    {
        let _db = Database::open(path).unwrap();
        // First open: migrates to v1
    }
    {
        let db = Database::open(path).unwrap();
        let version: i32 = db
            .conn()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        // Second open should not re-run migrations, so version stays put.
        assert_eq!(
            version, 24,
            "user_version must still be 24 on second open (idempotent)"
        );
    }
    let _ = std::fs::remove_file(&tmp);
}

// ── Backend seeding tests ─────────────────────────────────────────────────────

#[test]
fn test_backends_seeded() {
    let db = Database::open(":memory:").unwrap();
    let backends = list_backends(db.conn()).unwrap();
    // v1 seeds tinfoil, v10 seeds ppq-ai (INSERT OR IGNORE, so 2 total)
    assert_eq!(
        backends.len(),
        2,
        "should have 2 seeded backends (tinfoil + ppq-ai)"
    );
    let tinfoil = backends
        .iter()
        .find(|b| b.id == "tinfoil")
        .expect("tinfoil backend missing");
    assert_eq!(
        tinfoil.base_url, "https://inference.tinfoil.sh/v1/",
        "tinfoil base_url mismatch"
    );
}

#[test]
fn test_load_backends_active_flag() {
    let db = Database::open(":memory:").unwrap();
    let active_id = get_active_backend_id(db.conn()).unwrap();
    assert_eq!(
        active_id,
        Some("tinfoil".to_string()),
        "tinfoil should be the active backend"
    );
}

// ── Conversation / message persistence tests ──────────────────────────────────

#[test]
fn test_conversation_survives_reopen() {
    let tmp = std::env::temp_dir().join(format!("test_reopen_{}.db", uuid::Uuid::new_v4()));
    let path = tmp.to_str().unwrap();
    {
        let db = Database::open(path).unwrap();
        insert_conversation(
            db.conn(),
            &ConversationRow {
                id: "conv1".into(),
                title: "Test".into(),
                model_id: "model".into(),
                backend_id: "tinfoil".into(),
                system_prompt: None,
                created_at: 100,
                updated_at: 100,
                tools_enabled: false,
            },
        )
        .unwrap();
        insert_message(
            db.conn(),
            &MessageRow {
                id: "msg1".into(),
                conversation_id: "conv1".into(),
                role: "user".into(),
                content: "hello".into(),
                created_at: 100,
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
    } // db dropped, connection closed
    {
        let db = Database::open(path).unwrap();
        let convs = list_conversations(db.conn()).unwrap();
        assert_eq!(convs.len(), 1, "should have 1 conversation after reopen");
        assert_eq!(convs[0].id, "conv1");
        let msgs = list_messages(db.conn(), "conv1").unwrap();
        assert_eq!(msgs.len(), 1, "should have 1 message after reopen");
        assert_eq!(msgs[0].content, "hello");
    }
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_list_conversations_ordered() {
    let db = Database::open(":memory:").unwrap();
    // Insert two conversations; updated_at=200 should come first
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: "conv_old".into(),
            title: "Old".into(),
            model_id: "model".into(),
            backend_id: "tinfoil".into(),
            system_prompt: None,
            created_at: 100,
            updated_at: 100,
            tools_enabled: false,
        },
    )
    .unwrap();
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: "conv_new".into(),
            title: "New".into(),
            model_id: "model".into(),
            backend_id: "tinfoil".into(),
            system_prompt: None,
            created_at: 200,
            updated_at: 200,
            tools_enabled: false,
        },
    )
    .unwrap();
    let convs = list_conversations(db.conn()).unwrap();
    assert_eq!(convs.len(), 2);
    assert_eq!(
        convs[0].id, "conv_new",
        "newest conversation should be first"
    );
    assert_eq!(convs[1].id, "conv_old");
}

#[test]
fn test_messages_ordered_by_created_at() {
    let db = Database::open(":memory:").unwrap();
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: "conv1".into(),
            title: "Chat".into(),
            model_id: "model".into(),
            backend_id: "tinfoil".into(),
            system_prompt: None,
            created_at: 1,
            updated_at: 1,
            tools_enabled: false,
        },
    )
    .unwrap();
    // Insert messages out of chronological order
    for (id, ts) in [("msg3", 300i64), ("msg1", 100i64), ("msg2", 200i64)] {
        insert_message(
            db.conn(),
            &MessageRow {
                id: id.into(),
                conversation_id: "conv1".into(),
                role: "user".into(),
                content: format!("msg at {}", ts),
                created_at: ts,
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
    }
    let msgs = list_messages(db.conn(), "conv1").unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].id, "msg1", "oldest message should be first");
    assert_eq!(msgs[1].id, "msg2");
    assert_eq!(msgs[2].id, "msg3");
}

#[test]
fn test_message_route_metadata_round_trips() {
    let db = Database::open(":memory:").unwrap();
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: "conv-route".into(),
            title: "Route".into(),
            model_id: "qwen3-vl-30b".into(),
            backend_id: "tinfoil".into(),
            system_prompt: None,
            created_at: 1,
            updated_at: 1,
            tools_enabled: false,
        },
    )
    .unwrap();

    insert_message(
        db.conn(),
        &MessageRow {
            id: "msg-route".into(),
            conversation_id: "conv-route".into(),
            role: "assistant".into(),
            content: "answer".into(),
            created_at: 2,
            token_count: None,
            image_path: None,
            route_backend_id: Some("tinfoil".into()),
            route_model_id: Some("qwen3-vl-30b".into()),
            route_decision: Some("remote".into()),
            route_reason: Some("attachment present".into()),
            route_provider_name: Some("Tinfoil".into()),
            route_tee_label: Some("Intel TDX".into()),
            route_tee_verified: Some(true),
        },
    )
    .unwrap();

    let msg = list_messages(db.conn(), "conv-route")
        .unwrap()
        .into_iter()
        .next()
        .expect("message");
    assert_eq!(msg.route_backend_id.as_deref(), Some("tinfoil"));
    assert_eq!(msg.route_model_id.as_deref(), Some("qwen3-vl-30b"));
    assert_eq!(msg.route_decision.as_deref(), Some("remote"));
    assert_eq!(msg.route_reason.as_deref(), Some("attachment present"));
    assert_eq!(msg.route_provider_name.as_deref(), Some("Tinfoil"));
    assert_eq!(msg.route_tee_label.as_deref(), Some("Intel TDX"));
    assert_eq!(msg.route_tee_verified, Some(true));
}

#[test]
fn test_migration_v24_adds_nullable_message_route_columns() {
    let tmp = std::env::temp_dir().join(format!("test_v23v24_{}.db", uuid::Uuid::new_v4()));
    let path = tmp.to_str().unwrap();

    {
        let mut conn = rusqlite::Connection::open(path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let tx = conn.transaction().unwrap();
        for sql in crate::persistence::schema::MIGRATIONS.iter().take(23) {
            tx.execute_batch(sql).unwrap();
        }
        tx.execute(
            "INSERT INTO conversations (id, title, model_id, backend_id, created_at, updated_at)
             VALUES ('conv-old', 'Old', 'model', 'tinfoil', 1, 1)",
            [],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES ('msg-old', 'conv-old', 'assistant', 'old answer', 2)",
            [],
        )
        .unwrap();
        tx.pragma_update(None, "user_version", 23i32).unwrap();
        tx.commit().unwrap();
    }

    {
        let db = Database::open(path).unwrap();
        let conn = db.conn();
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 24);

        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(messages)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        for col in [
            "route_backend_id",
            "route_model_id",
            "route_decision",
            "route_reason",
            "route_provider_name",
            "route_tee_label",
            "route_tee_verified",
        ] {
            assert!(cols.iter().any(|existing| existing == col), "{col} missing");
        }

        let msg = list_messages(conn, "conv-old")
            .unwrap()
            .into_iter()
            .next()
            .expect("old message");
        assert_eq!(msg.content, "old answer");
        assert!(msg.route_backend_id.is_none());
        assert!(msg.route_model_id.is_none());
        assert!(msg.route_tee_verified.is_none());
    }

    let _ = std::fs::remove_file(&tmp);
}

// ── Keychain tests ────────────────────────────────────────────────────────────

#[test]
fn test_null_keychain() {
    let kc = NullKeychainProvider;
    // store should be a no-op (no panic)
    kc.store("svc".into(), "key".into(), "secret".into());
    // load should always return None
    let result = kc.load("svc".into(), "key".into());
    assert!(
        result.is_none(),
        "NullKeychainProvider::load must return None"
    );
    // delete should be a no-op (no panic)
    kc.delete("svc".into(), "key".into());
}

// ── Schema safety tests ───────────────────────────────────────────────────────

#[test]
fn test_api_key_not_in_sqlite() {
    let db = Database::open(":memory:").unwrap();
    let cols: Vec<String> = db
        .conn()
        .prepare("PRAGMA table_info(backends)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        !cols.contains(&"api_key".to_string()),
        "api_key column must not exist in backends table; found columns: {:?}",
        cols
    );
}

// ── Agent schema tests ────────────────────────────────────────────────────────

#[test]
fn test_agent_session_insert() {
    let db = Database::open(":memory:").unwrap();
    db.conn()
        .execute(
            "INSERT INTO agent_sessions (id, title, backend_id, created_at, updated_at)
             VALUES ('sess1', 'Test Session', 'tinfoil', 1000, 1000)",
            [],
        )
        .unwrap();
    let title: String = db
        .conn()
        .query_row(
            "SELECT title FROM agent_sessions WHERE id = 'sess1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(title, "Test Session");
}

#[test]
fn test_agent_steps_fk() {
    let db = Database::open(":memory:").unwrap();
    // Attempt to insert a step referencing a non-existent session -- must fail
    let result = db.conn().execute(
        "INSERT INTO agent_steps (id, session_id, step_number, action_type, created_at)
         VALUES ('step1', 'nonexistent_session', 1, 'test_action', 0)",
        [],
    );
    assert!(
        result.is_err(),
        "FK constraint should reject insert with invalid session_id"
    );
}

// ── Attestation cache compatibility test ──────────────────────────────────────

#[test]
fn test_attestation_cache_compat() {
    let db = Database::open(":memory:").unwrap();
    // Insert a row into the attestation_cache table using the same schema
    // as the Phase 3 AttestationCache::put method
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO attestation_cache
             (backend_id, tee_type, status, report_blob, verified_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "tinfoil",
                "IntelTdx",
                "verified",
                vec![0u8, 1, 2, 3],
                1000i64,
                5000i64
            ],
        )
        .unwrap();
    // Query it back
    let status: String = db
        .conn()
        .query_row(
            "SELECT status FROM attestation_cache WHERE backend_id = 'tinfoil' AND tee_type = 'IntelTdx'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "verified");
    // Verify INSERT OR REPLACE works (upsert)
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO attestation_cache
             (backend_id, tee_type, status, report_blob, verified_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "tinfoil",
                "IntelTdx",
                "provider_verified",
                vec![9u8],
                2000i64,
                6000i64
            ],
        )
        .unwrap();
    let updated_status: String = db
        .conn()
        .query_row(
            "SELECT status FROM attestation_cache WHERE backend_id = 'tinfoil' AND tee_type = 'IntelTdx'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        updated_status, "provider_verified",
        "upsert should replace the row"
    );
}

// ── FfiApp integration tests (Plan 04-02) ─────────────────────────────────────

/// Verify that FfiApp loads backends from SQLite on startup.
///
/// Uses in-memory DB (empty data_dir), which is seeded with tinfoil
/// during migration v1. Confirms that:
/// - Tinfoil backend appears in AppState.backends after startup
/// - active_backend_id matches the is_active=1 backend (tinfoil) from the DB
#[test]
fn test_ffiapp_loads_backends_from_db() {
    let app = FfiApp::new(
        "".to_string(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    assert!(
        wait_until(std::time::Duration::from_secs(2), || app
            .state()
            .backends
            .len()
            >= 2),
        "timed out waiting for seeded backends to load into AppState"
    );
    let state = app.state();
    // v1 seeds tinfoil (active), v10 seeds ppq-ai (inactive) = 2 total backends
    assert_eq!(
        state.backends.len(),
        2,
        "Expected 2 backends seeded from SQLite (tinfoil + ppq-ai)"
    );
    let tinfoil_summary = state
        .backends
        .iter()
        .find(|b| b.id == "tinfoil")
        .expect("tinfoil backend should be present");
    assert_eq!(
        tinfoil_summary.id, "tinfoil",
        "First active backend should be tinfoil"
    );
    assert_eq!(
        state.active_backend_id,
        Some("tinfoil".to_string()),
        "Active backend should be tinfoil (is_active=1 in migration seed)"
    );
}

/// Verify that FfiApp loads conversations from SQLite into AppState on startup.
///
/// Pre-populates a DB file with a conversation, then starts FfiApp pointing at
/// that directory. Confirms that the conversation appears in AppState.conversations.
#[test]
fn test_ffiapp_loads_conversations_from_db() {
    // Create a unique temp directory for the test
    let dir = std::env::temp_dir().join(format!("test_ffiapp_conv_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let db_file = dir.join("mango.db");
    let db_path = db_file.to_str().unwrap().to_string();

    // Pre-populate the DB with a conversation before starting FfiApp
    {
        let db = Database::open(&db_path).unwrap();
        insert_conversation(
            db.conn(),
            &ConversationRow {
                id: "conv-startup-1".into(),
                title: "Hello persistence".into(),
                model_id: "meta-llama/Llama-3.3-70B-Instruct".into(),
                backend_id: "tinfoil".into(),
                system_prompt: None,
                created_at: 1000,
                updated_at: 2000,
                tools_enabled: false,
            },
        )
        .unwrap();
    } // DB connection closed

    // Start FfiApp pointing at the directory that contains the pre-populated DB
    let data_dir = dir.to_str().unwrap().to_string();
    let app = FfiApp::new(
        data_dir,
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    app.sync();

    let state = app.state();
    assert_eq!(
        state.conversations.len(),
        1,
        "Expected 1 conversation loaded from SQLite on startup"
    );
    assert_eq!(state.conversations[0].id, "conv-startup-1");
    assert_eq!(state.conversations[0].title, "Hello persistence");
    assert_eq!(state.conversations[0].backend_id, "tinfoil");
    assert_eq!(state.conversations[0].updated_at, 2000);

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Agent session CRUD tests ──────────────────────────────────────────────────

/// Verify that agent session and step rows survive a DB close/reopen cycle.
#[test]
fn test_agent_session_survives_reopen() {
    let tmp = std::env::temp_dir().join(format!("test_agent_reopen_{}.db", uuid::Uuid::new_v4()));
    let path = tmp.to_str().unwrap();
    {
        let db = Database::open(path).unwrap();
        insert_agent_session(
            db.conn(),
            &AgentSessionRow {
                id: "sess-reopen-1".into(),
                title: "Reopen Test Session".into(),
                status: "running".into(),
                backend_id: "tinfoil".into(),
                created_at: 1000,
                updated_at: 2000,
            },
        )
        .unwrap();
        insert_agent_step(
            db.conn(),
            &AgentStepRow {
                id: "step-reopen-1".into(),
                session_id: "sess-reopen-1".into(),
                step_number: 1,
                action_type: "tool_call".into(),
                action_payload: r#"{"tool":"search"}"#.into(),
                result: Some("found results".into()),
                status: "completed".into(),
                created_at: 1500,
                tool_origin: None,
            },
        )
        .unwrap();
    } // db dropped, connection closed
    {
        let db = Database::open(path).unwrap();
        let sessions = list_agent_sessions(db.conn()).unwrap();
        assert_eq!(
            sessions.len(),
            1,
            "should have 1 agent session after reopen"
        );
        assert_eq!(sessions[0].id, "sess-reopen-1");
        assert_eq!(sessions[0].title, "Reopen Test Session");
        assert_eq!(sessions[0].status, "running");

        let steps = list_agent_steps(db.conn(), "sess-reopen-1").unwrap();
        assert_eq!(steps.len(), 1, "should have 1 agent step after reopen");
        assert_eq!(steps[0].id, "step-reopen-1");
        assert_eq!(steps[0].action_type, "tool_call");
        assert_eq!(steps[0].result, Some("found results".into()));
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Verify that FfiApp loads agent sessions from SQLite into AppState on startup.
///
/// Startup only hydrates agent sessions when the `agents` cargo feature is
/// enabled (releases ship without the agent surface), so this test must run
/// under the same gate or it fails deterministically in default builds.
#[cfg(feature = "agents")]
#[test]
fn test_ffiapp_loads_agent_sessions_from_db() {
    let dir = std::env::temp_dir().join(format!("test_ffiapp_agent_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let db_file = dir.join("mango.db");
    let db_path = db_file.to_str().unwrap().to_string();

    // Pre-populate the DB with an agent session before starting FfiApp
    {
        let db = Database::open(&db_path).unwrap();
        insert_agent_session(
            db.conn(),
            &AgentSessionRow {
                id: "agent-startup-1".into(),
                title: "Background Research".into(),
                status: "completed".into(),
                backend_id: "tinfoil".into(),
                created_at: 5000,
                updated_at: 9000,
            },
        )
        .unwrap();
    } // DB connection closed

    // Start FfiApp pointing at the directory that contains the pre-populated DB
    let data_dir = dir.to_str().unwrap().to_string();
    let app = FfiApp::new(
        data_dir,
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    app.sync();

    let state = app.state();
    assert_eq!(
        state.agent_sessions.len(),
        1,
        "Expected 1 agent session loaded from SQLite on startup"
    );
    assert_eq!(state.agent_sessions[0].id, "agent-startup-1");
    assert_eq!(state.agent_sessions[0].title, "Background Research");
    assert_eq!(state.agent_sessions[0].status, "completed");
    assert_eq!(state.agent_sessions[0].backend_id, "tinfoil");
    assert_eq!(state.agent_sessions[0].updated_at, 9000);

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

/// Verify that list_agent_sessions returns sessions ordered by updated_at DESC (newest first).
#[test]
fn test_list_agent_sessions_ordered() {
    let db = Database::open(":memory:").unwrap();
    insert_agent_session(
        db.conn(),
        &AgentSessionRow {
            id: "sess-old".into(),
            title: "Old Session".into(),
            status: "completed".into(),
            backend_id: "tinfoil".into(),
            created_at: 100,
            updated_at: 100,
        },
    )
    .unwrap();
    insert_agent_session(
        db.conn(),
        &AgentSessionRow {
            id: "sess-new".into(),
            title: "New Session".into(),
            status: "running".into(),
            backend_id: "tinfoil".into(),
            created_at: 200,
            updated_at: 500,
        },
    )
    .unwrap();
    let sessions = list_agent_sessions(db.conn()).unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, "sess-new", "newest session should be first");
    assert_eq!(sessions[1].id, "sess-old");
}

#[test]
fn test_migration_v11_seeds_ppq_ai_private_transport() {
    let db = Database::open(":memory:").unwrap();

    // Verify user_version is 14 after all migrations (v14 seeds TEE policy)
    let version: i32 = db
        .conn()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        version, 24,
        "user_version should be 24 after all migrations including v24"
    );

    // Query the ppq-ai row directly
    let row: (String, String, String, i32, i32) = db
        .conn()
        .query_row(
            "SELECT tee_type, base_url, model_list, display_order, is_active FROM backends WHERE id = 'ppq-ai'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .expect("ppq-ai backend row should exist after migration v11");

    let (tee_type, base_url, model_list, display_order, is_active) = row;

    assert_eq!(tee_type, "AmdSevSnp", "tee_type should be AmdSevSnp");
    assert_eq!(base_url, "https://api.ppq.ai/private/v1/");
    assert_eq!(display_order, 1, "display_order should be 1");
    assert_eq!(is_active, 0, "is_active should be 0 (inactive by default)");

    // Verify all 5 private/ model IDs are in the model_list JSON
    let expected_models = [
        "private/kimi-k2-5",
        "private/deepseek-r1-0528",
        "private/gpt-oss-120b",
        "private/llama3-3-70b",
        "private/qwen3-vl-30b",
    ];
    for model in &expected_models {
        assert!(
            model_list.contains(model),
            "model_list should contain '{}', got: {}",
            model,
            model_list
        );
    }
}

// ── Phase 35 — MIGRATION_V20 (contextvm tools + agent_steps.tool_origin) ──────

#[test]
fn test_migration_v20_creates_contextvm_tools_table() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='contextvm_tools'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "contextvm_tools table missing");
    let idx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='index' AND \
                   name IN ('idx_contextvm_tools_name', \
                            'idx_contextvm_tools_enabled')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(idx_count, 2, "contextvm_tools indices missing");
}

#[test]
fn test_migration_v21_backfills_contextvm_provider_profile_columns() {
    let tmp = std::env::temp_dir().join(format!("test_v20v21_{}.db", uuid::Uuid::new_v4()));
    let path = tmp.to_str().unwrap();

    {
        let mut conn = rusqlite::Connection::open(path).unwrap();
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let tx = conn.transaction().unwrap();
        for sql in crate::persistence::schema::MIGRATIONS.iter().take(20) {
            tx.execute_batch(sql).unwrap();
        }
        tx.pragma_update(None, "user_version", 20i32).unwrap();
        tx.commit().unwrap();

        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(contextvm_tools)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(
            !cols.iter().any(|c| c == "provider_name"),
            "fixture should model the old v20 schema before profile columns"
        );
    }

    {
        let db = Database::open(path).unwrap();
        let conn = db.conn();
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(contextvm_tools)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        for col in [
            "provider_name",
            "provider_about",
            "provider_picture",
            "provider_nip05",
        ] {
            assert!(
                cols.iter().any(|c| c == col),
                "contextvm_tools.{} missing after v21 migration; cols: {:?}",
                col,
                cols
            );
        }

        let row = queries::ContextvmToolRow {
            provider_name: Some("Echo Provider".into()),
            provider_about: Some("Smoke-test provider".into()),
            provider_picture: Some("https://example.invalid/pic.png".into()),
            provider_nip05: Some("echo@example.invalid".into()),
            ..fixture_contextvm_row("pkA:echo", "echo", true, 1_700_000_010)
        };
        queries::upsert_contextvm_tool(conn, &row).unwrap();
        let fetched = queries::get_contextvm_tool_by_name(conn, "echo")
            .unwrap()
            .unwrap();
        assert_eq!(fetched.provider_name.as_deref(), Some("Echo Provider"));
        assert_eq!(
            fetched.provider_about.as_deref(),
            Some("Smoke-test provider")
        );
        assert_eq!(
            fetched.provider_picture.as_deref(),
            Some("https://example.invalid/pic.png")
        );
        assert_eq!(
            fetched.provider_nip05.as_deref(),
            Some("echo@example.invalid")
        );
    }

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_migration_v20_adds_tool_origin_to_agent_steps() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    let mut stmt = conn.prepare("PRAGMA table_info(agent_steps)").unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert!(
        cols.iter().any(|c| c == "tool_origin"),
        "agent_steps.tool_origin column missing; cols: {:?}",
        cols
    );
}

#[test]
fn test_unique_index_on_tool_name() {
    let db = Database::open(":memory:").unwrap();
    let now = 1_700_000_000_i64;
    let conn = db.conn();
    conn.execute(
        "INSERT INTO contextvm_tools \
         (id, tool_name, description, provider_pubkey, schema_json, enabled, last_seen_at) \
         VALUES ('p1:foo', 'foo', '', 'pk1', '{}', 0, ?1)",
        [now],
    )
    .unwrap();
    let res = conn.execute(
        "INSERT INTO contextvm_tools \
         (id, tool_name, description, provider_pubkey, schema_json, enabled, last_seen_at) \
         VALUES ('p2:foo', 'foo', '', 'pk2', '{}', 0, ?1)",
        [now],
    );
    assert!(
        res.is_err(),
        "duplicate tool_name should violate unique index"
    );
}

#[test]
fn test_pre_v20_database_upgrades_cleanly() {
    // Migration runner is idempotent and handles V20 on top of any earlier
    // schema. Smoke test: open in-memory DB twice in a row, no error.
    let db1 = Database::open(":memory:").unwrap();
    drop(db1);
    let db2 = Database::open(":memory:").unwrap();
    let mut stmt = db2
        .conn()
        .prepare("PRAGMA table_info(agent_steps)")
        .unwrap();
    let has_origin = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .filter_map(Result::ok)
        .any(|c| c == "tool_origin");
    assert!(has_origin);
}

fn fixture_contextvm_row(
    id: &str,
    name: &str,
    enabled: bool,
    ts: i64,
) -> queries::ContextvmToolRow {
    queries::ContextvmToolRow {
        id: id.into(),
        tool_name: name.into(),
        display_name: None,
        description: format!("Description of {}", name),
        provider_pubkey: id.split(':').next().unwrap().into(),
        provider_display_name: None,
        provider_name: None,
        provider_about: None,
        provider_picture: None,
        provider_nip05: None,
        schema_json: "{\"type\":\"object\"}".into(),
        enabled,
        last_seen_at: ts,
    }
}

#[test]
fn test_round_trip_enabled_contextvm_tool() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    let row = fixture_contextvm_row("pkA:get_weather", "get_weather", true, 1_700_000_010);
    queries::upsert_contextvm_tool(conn, &row).unwrap();

    let fetched = queries::get_contextvm_tool_by_name(conn, "get_weather")
        .unwrap()
        .unwrap();
    assert_eq!(fetched, row);

    let enabled = queries::list_enabled_contextvm_tools(conn).unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].tool_name, "get_weather");
}

#[test]
fn test_list_enabled_skips_disabled_rows_and_orders_by_last_seen_desc() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();
    queries::upsert_contextvm_tool(
        conn,
        &fixture_contextvm_row("pkA:tool_old", "tool_old", true, 1_700_000_001),
    )
    .unwrap();
    queries::upsert_contextvm_tool(
        conn,
        &fixture_contextvm_row("pkA:tool_new", "tool_new", true, 1_700_000_999),
    )
    .unwrap();
    queries::upsert_contextvm_tool(
        conn,
        &fixture_contextvm_row("pkA:tool_off", "tool_off", false, 1_700_000_500),
    )
    .unwrap();
    let names: Vec<String> = queries::list_enabled_contextvm_tools(conn)
        .unwrap()
        .into_iter()
        .map(|r| r.tool_name)
        .collect();
    assert_eq!(names, vec!["tool_new".to_string(), "tool_old".to_string()]);
}

#[test]
fn test_update_contextvm_tool_enabled_persists_after_reopen() {
    let path = std::env::temp_dir().join(format!("test_ctx_{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let db = Database::open(path.to_str().unwrap()).unwrap();
        queries::upsert_contextvm_tool(
            db.conn(),
            &fixture_contextvm_row("pkA:foo", "foo", false, 1),
        )
        .unwrap();
        queries::update_contextvm_tool_enabled(db.conn(), "pkA:foo", true).unwrap();
    }
    // Reopen and verify enabled persists across handles.
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let row = queries::get_contextvm_tool_by_name(db.conn(), "foo")
        .unwrap()
        .unwrap();
    assert!(row.enabled, "enabled flag must persist across DB reopens");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_auto_discover_tools_setting_round_trips() {
    let db = Database::open(":memory:").unwrap();
    // Default: missing key → None.
    assert!(queries::get_setting(db.conn(), "auto_discover_tools")
        .unwrap()
        .is_none());
    queries::set_setting(db.conn(), "auto_discover_tools", "1").unwrap();
    assert_eq!(
        queries::get_setting(db.conn(), "auto_discover_tools")
            .unwrap()
            .as_deref(),
        Some("1")
    );
    queries::set_setting(db.conn(), "auto_discover_tools", "0").unwrap();
    assert_eq!(
        queries::get_setting(db.conn(), "auto_discover_tools")
            .unwrap()
            .as_deref(),
        Some("0")
    );
}
