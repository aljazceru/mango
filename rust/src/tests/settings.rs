/// Integration tests for backend CRUD and settings persistence (Phase 6, Plan 02).
///
/// These tests exercise the persistence layer directly using an in-memory SQLite
/// database to verify ROUT-03, SETT-02, and SETT-03 behaviors.
use crate::persistence::{self, queries, Database};

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

#[test]
fn test_brave_api_key_persists() {
    let db = Database::open(":memory:").unwrap();
    // Initially no key set
    let val = queries::get_setting(db.conn(), "brave_api_key").unwrap();
    assert_eq!(val, None, "brave_api_key should be None initially");

    // Set a key
    queries::set_setting(db.conn(), "brave_api_key", "test-key-123").unwrap();
    let val = queries::get_setting(db.conn(), "brave_api_key").unwrap();
    assert_eq!(val, Some("test-key-123".to_string()), "brave_api_key should persist");

    // Overwrite the key
    queries::set_setting(db.conn(), "brave_api_key", "updated-key-456").unwrap();
    let val = queries::get_setting(db.conn(), "brave_api_key").unwrap();
    assert_eq!(val, Some("updated-key-456".to_string()), "brave_api_key should be overwritten");
}

#[test]
fn test_memory_count() {
    let db = Database::open(":memory:").unwrap();

    // Initially zero memories
    let count: i64 = db.conn()
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "memory count should be 0 initially");

    // Insert a memory
    let row = queries::MemoryRow {
        id: "mem-1".into(),
        conversation_id: "conv-1".into(),
        content: "Test memory content".into(),
        usearch_key: 1,
        created_at: 1000,
    };
    queries::insert_memory(db.conn(), &row).unwrap();

    let count: i64 = db.conn()
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "memory count should be 1 after insert");

    // Delete the memory
    queries::delete_memory(db.conn(), "mem-1").unwrap();

    let count: i64 = db.conn()
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0, "memory count should be 0 after delete");
}
