/// Integration tests for backend CRUD and settings persistence (Phase 6, Plan 02).
///
/// These tests exercise the persistence layer directly using an in-memory SQLite
/// database to verify ROUT-03, SETT-02, and SETT-03 behaviors.
use std::time::Duration;

use crate::persistence::{self, queries, Database};
use crate::{AppAction, EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};

fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    std::thread::sleep(Duration::from_millis(150));
    app
}

fn wait() {
    std::thread::sleep(Duration::from_millis(200));
}

#[test]
fn test_add_backend_persists() {
    let db = Database::open(":memory:").unwrap();
    let row = queries::BackendRow {
        id: "new-backend".into(),
        name: "New Backend".into(),
        base_url: "https://new.test/v1".into(),
        model_list: "[\"model-a\"]".into(),
        tee_type: "IntelTdx".into(),
        display_order: 10,
        is_active: 0,
        created_at: 1000,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };
    queries::insert_backend(db.conn(), &row).unwrap();
    let all = queries::list_backends(db.conn()).unwrap();
    assert!(
        all.iter().any(|b| b.id == "new-backend"),
        "new backend should be in list"
    );
    // v1 seeds tinfoil, v10 seeds ppq-ai, plus 1 new = 3 total
    assert_eq!(all.len(), 3, "2 seeded + 1 new = 3 total");
}

#[test]
fn test_remove_backend_persists() {
    let db = Database::open(":memory:").unwrap();
    let row = queries::BackendRow {
        id: "temp".into(),
        name: "Temp".into(),
        base_url: "https://temp.test".into(),
        model_list: "[]".into(),
        tee_type: "Unknown".into(),
        display_order: 5,
        is_active: 0,
        created_at: 1000,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };
    queries::insert_backend(db.conn(), &row).unwrap();
    // v1 seeds tinfoil, v10 seeds ppq-ai, plus temp = 3 total
    assert_eq!(
        queries::list_backends(db.conn()).unwrap().len(),
        3,
        "3 backends after insert"
    );
    queries::delete_backend(db.conn(), "temp").unwrap();
    // After deleting temp, back to 2 seeded backends
    assert_eq!(
        queries::list_backends(db.conn()).unwrap().len(),
        2,
        "2 backends after delete"
    );
}

#[test]
fn test_reorder_backend_updates_order() {
    let db = Database::open(":memory:").unwrap();
    // Add a second backend so we can test reordering
    let row = queries::BackendRow {
        id: "custom".into(),
        name: "Custom".into(),
        base_url: "https://custom.test/v1".into(),
        model_list: "[]".into(),
        tee_type: "IntelTdx".into(),
        display_order: 1,
        is_active: 0,
        created_at: 1000,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };
    queries::insert_backend(db.conn(), &row).unwrap();
    // Swap display orders: set custom to 0 and tinfoil to 1
    queries::update_backend_display_order(db.conn(), "custom", 0).unwrap();
    queries::update_backend_display_order(db.conn(), "tinfoil", 1).unwrap();
    let all = queries::list_backends(db.conn()).unwrap();
    assert_eq!(all[0].id, "custom", "custom should now be first");
    assert_eq!(all[1].id, "tinfoil", "tinfoil should now be second");
}

#[test]
fn test_set_default_backend_writes_setting() {
    let db = Database::open(":memory:").unwrap();
    queries::set_setting(db.conn(), "default_backend_id", "tinfoil").unwrap();
    let val = queries::get_setting(db.conn(), "default_backend_id").unwrap();
    assert_eq!(val, Some("tinfoil".to_string()));
}

#[test]
fn test_set_default_model_writes_setting() {
    let db = Database::open(":memory:").unwrap();
    queries::set_setting(db.conn(), "default_model_id", "llama-70b").unwrap();
    let val = queries::get_setting(db.conn(), "default_model_id").unwrap();
    assert_eq!(val, Some("llama-70b".to_string()));
}

#[test]
fn test_override_conversation_backend_persists() {
    let db = Database::open(":memory:").unwrap();
    // Insert a conversation on tinfoil backend
    let conv = persistence::ConversationRow {
        id: "conv-1".into(),
        title: "Test".into(),
        model_id: "m".into(),
        backend_id: "tinfoil".into(),
        system_prompt: None,
        created_at: 1000,
        updated_at: 1000,
    };
    queries::insert_conversation(db.conn(), &conv).unwrap();
    // Override the backend to a custom value (backend_id is a plain text field, no FK constraint)
    queries::update_conversation_backend(db.conn(), "conv-1", "custom-backend", 2000).unwrap();
    let convs = queries::list_conversations(db.conn()).unwrap();
    let updated = convs.iter().find(|c| c.id == "conv-1").unwrap();
    assert_eq!(
        updated.backend_id, "custom-backend",
        "backend_id should be updated to custom-backend"
    );
}

/// Verify SetBraveApiKey action persists the key and updates brave_api_key_set (Phase 24, D-18).
#[test]
fn test_brave_api_key_persists() {
    let app = make_app();

    // Initially brave_api_key_set should be false (no key set)
    let initial_state = app.state();
    assert!(!initial_state.brave_api_key_set, "brave_api_key_set should start false");

    // Set a Brave API key
    app.dispatch(AppAction::SetBraveApiKey { api_key: "test-brave-key-abc123".to_string() });
    wait();

    let state = app.state();
    assert!(state.brave_api_key_set, "brave_api_key_set should be true after setting key");

    // Clear the key with empty string
    app.dispatch(AppAction::SetBraveApiKey { api_key: "".to_string() });
    wait();

    let state = app.state();
    assert!(!state.brave_api_key_set, "brave_api_key_set should be false after clearing key");
}

/// Verify memory_count field in AppState tracks memory count correctly (Phase 24, D-03/D-04).
#[test]
fn test_memory_count() {

    let app = make_app();

    // Initially memory_count should be 0
    let initial_state = app.state();
    assert_eq!(initial_state.memory_count, 0, "memory_count should start at 0");

    // Delete a nonexistent memory (should update count, still 0)
    app.dispatch(AppAction::DeleteMemory { memory_id: "nonexistent".to_string() });
    wait();

    let state = app.state();
    assert_eq!(state.memory_count, 0, "memory_count should remain 0 after deleting nonexistent");
}
