use crate::persistence::queries::{
    get_active_backend_id, insert_agent_session, insert_agent_step, insert_conversation,
    insert_message, list_agent_sessions, list_agent_steps, list_backends, list_conversations,
    list_messages, AgentSessionRow, AgentStepRow, ConversationRow, MessageRow,
};
use crate::persistence::Database;
use crate::KeychainProvider;
use crate::{EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};

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
            version, 15,
            "user_version should be 15 after all migrations (v15 adds memories table)"
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
        version, 15,
        "user_version should be 15 after all migrations (v1+v2+...+v15)"
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
        // Second open should not re-run migrations, so version stays at 13
        assert_eq!(
            version, 15,
            "user_version must still be 15 on second open (idempotent)"
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
    );
    std::thread::sleep(std::time::Duration::from_millis(100));
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
    );
    std::thread::sleep(std::time::Duration::from_millis(100));

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
    );
    std::thread::sleep(std::time::Duration::from_millis(100));

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
        version, 15,
        "user_version should be 15 after all migrations including v15"
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
