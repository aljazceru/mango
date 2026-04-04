use std::time::Duration;

use crate::persistence::{
    queries::{delete_memory, insert_memory, list_memories, update_memory, MemoryRow},
    Database,
};
use crate::memory::extract::should_extract;
use crate::memory::retrieve::{build_system_with_memories, MemoryResult, DEFAULT_MEMORY_TOP_K};
use crate::persistence::queries::get_memory_content_by_usearch_keys;
use crate::{AppAction, EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider, Screen};

// ── Actor integration helpers (duplicated from agent.rs for independence) ─────

fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    // Allow actor thread to initialize
    std::thread::sleep(Duration::from_millis(150));
    app
}

fn wait() {
    std::thread::sleep(Duration::from_millis(200));
}

fn setup_db() -> Database {
    Database::open(":memory:").unwrap()
}

#[test]
fn test_migration_v15_creates_memories_table() {
    let db = setup_db();
    // Verify table exists by running a count query
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_insert_and_list_memories() {
    let db = setup_db();
    let row1 = MemoryRow {
        id: "mem-1".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "User likes Rust".to_string(),
        usearch_key: 1000,
        created_at: 100,
    };
    let row2 = MemoryRow {
        id: "mem-2".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "User works at Acme".to_string(),
        usearch_key: 2000,
        created_at: 200,
    };
    insert_memory(db.conn(), &row1).unwrap();
    insert_memory(db.conn(), &row2).unwrap();
    let memories = list_memories(db.conn()).unwrap();
    assert_eq!(memories.len(), 2);
    // DESC order: row2 (created_at=200) first
    assert_eq!(memories[0].content, "User works at Acme");
    assert_eq!(memories[1].content, "User likes Rust");
}

#[test]
fn test_delete_memory() {
    let db = setup_db();
    let row = MemoryRow {
        id: "mem-del".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Temporary fact".to_string(),
        usearch_key: 3000,
        created_at: 300,
    };
    insert_memory(db.conn(), &row).unwrap();
    delete_memory(db.conn(), "mem-del").unwrap();
    let memories = list_memories(db.conn()).unwrap();
    assert_eq!(memories.len(), 0);
}

#[test]
fn test_usearch_key_unique_constraint() {
    let db = setup_db();
    let row1 = MemoryRow {
        id: "mem-a".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Fact A".to_string(),
        usearch_key: 9999,
        created_at: 100,
    };
    let row2 = MemoryRow {
        id: "mem-b".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Fact B".to_string(),
        usearch_key: 9999, // duplicate key
        created_at: 200,
    };
    insert_memory(db.conn(), &row1).unwrap();
    assert!(insert_memory(db.conn(), &row2).is_err());
}

#[test]
fn test_should_extract_empty() {
    assert!(!should_extract(&[]));
}

#[test]
fn test_should_extract_single_message() {
    let msgs = vec![("user".to_string(), "hello world, this is a fairly long message that has enough characters to pass the threshold check".to_string())];
    assert!(!should_extract(&msgs));
}

#[test]
fn test_should_extract_short_messages() {
    let msgs = vec![
        ("user".to_string(), "hi".to_string()),
        ("assistant".to_string(), "hello".to_string()),
    ];
    assert!(!should_extract(&msgs));
}

#[test]
fn test_should_extract_sufficient_messages() {
    let msgs = vec![
        ("user".to_string(), "My name is Alex and I work at Acme Corporation as a senior engineer".to_string()),
        ("assistant".to_string(), "Nice to meet you Alex! That sounds like a great position at Acme.".to_string()),
    ];
    assert!(should_extract(&msgs));
}

#[test]
fn test_extraction_json_parsing_valid() {
    let json = r#"["User prefers dark mode", "User's name is Alex"]"#;
    let result: Vec<String> = serde_json::from_str(json).unwrap_or_default();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], "User prefers dark mode");
}

#[test]
fn test_extraction_json_parsing_invalid() {
    let prose = "I found some interesting facts about the user.";
    let result: Vec<String> = serde_json::from_str(prose).unwrap_or_default();
    assert!(result.is_empty());
}

#[test]
fn test_extraction_json_parsing_empty_array() {
    let json = "[]";
    let result: Vec<String> = serde_json::from_str(json).unwrap_or_default();
    assert!(result.is_empty());
}

// ── retrieve.rs tests (Phase 21) ──────────────────────────────────────────────

#[test]
fn test_build_system_with_memories_empty_returns_base() {
    let result = build_system_with_memories("base prompt", &[]);
    assert_eq!(result, "base prompt");
}

#[test]
fn test_build_system_with_memories_single() {
    let memories = vec![MemoryResult {
        content: "User likes Rust".to_string(),
        score: 0.1,
    }];
    let result = build_system_with_memories("base prompt", &memories);
    assert!(result.starts_with("<memories>"), "Should start with <memories>");
    assert!(result.contains("[1] "), "Should contain [1] numbering");
    assert!(result.ends_with("base prompt"), "Should end with base prompt");
}

#[test]
fn test_build_system_with_memories_multiple_numbered() {
    let memories = vec![
        MemoryResult {
            content: "User likes Rust".to_string(),
            score: 0.1,
        },
        MemoryResult {
            content: "User works at Acme".to_string(),
            score: 0.2,
        },
    ];
    let result = build_system_with_memories("base", &memories);
    assert!(result.contains("[1]"), "Should contain [1]");
    assert!(result.contains("[2]"), "Should contain [2]");
}

#[test]
fn test_build_system_with_memories_no_empty_tag() {
    let result = build_system_with_memories("base prompt", &[]);
    assert!(!result.contains("<memories>"), "Empty memories should produce no <memories> tag");
}

#[test]
fn test_default_memory_top_k_is_5() {
    assert_eq!(DEFAULT_MEMORY_TOP_K, 5);
}

#[test]
fn test_get_memory_content_by_usearch_keys_empty() {
    let db = setup_db();
    let result = get_memory_content_by_usearch_keys(db.conn(), &[]).unwrap();
    assert_eq!(result, vec![]);
}

#[test]
fn test_get_memory_content_by_usearch_keys_found() {
    let db = setup_db();
    let row1 = MemoryRow {
        id: "mem-r1".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Fact about Rust".to_string(),
        usearch_key: 5000,
        created_at: 100,
    };
    let row2 = MemoryRow {
        id: "mem-r2".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Fact about Acme".to_string(),
        usearch_key: 6000,
        created_at: 200,
    };
    insert_memory(db.conn(), &row1).unwrap();
    insert_memory(db.conn(), &row2).unwrap();
    let mut result = get_memory_content_by_usearch_keys(db.conn(), &[5000, 6000]).unwrap();
    result.sort_by_key(|(k, _)| *k);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], (5000, "Fact about Rust".to_string()));
    assert_eq!(result[1], (6000, "Fact about Acme".to_string()));
}

#[test]
fn test_get_memory_content_by_usearch_keys_missing() {
    let db = setup_db();
    let result = get_memory_content_by_usearch_keys(db.conn(), &[99999]).unwrap();
    assert_eq!(result, vec![]);
}

#[test]
fn test_get_memory_content_by_usearch_keys_partial() {
    let db = setup_db();
    let row = MemoryRow {
        id: "mem-p1".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Partial fact".to_string(),
        usearch_key: 7000,
        created_at: 100,
    };
    insert_memory(db.conn(), &row).unwrap();
    let result = get_memory_content_by_usearch_keys(db.conn(), &[7000, 88888]).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 7000);
    assert_eq!(result[0].1, "Partial fact");
}

// ── Phase 23 tests: update_memory query and actor-level memory handlers ───────

/// Verify update_memory SQL query updates content and preserves the row ID (MEM-06).
#[test]
fn test_update_memory() {
    let db = setup_db();
    let row = MemoryRow {
        id: "mem-upd".to_string(),
        conversation_id: "conv-1".to_string(),
        content: "Original fact".to_string(),
        usearch_key: 10000,
        created_at: 500,
    };
    insert_memory(db.conn(), &row).unwrap();
    update_memory(db.conn(), "mem-upd", "Updated fact text").unwrap();
    let memories = list_memories(db.conn()).unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].content, "Updated fact text", "Content should be updated");
    assert_eq!(memories[0].id, "mem-upd", "ID should be unchanged after update");
}

/// Verify ListMemories actor handler populates AppState.memories without panic (MEM-04).
#[test]
fn test_list_memories_action() {
    let app = make_app();
    // Initial state: memories field is accessible and empty
    assert!(app.state().memories.is_empty(), "memories should start empty");
    app.dispatch(AppAction::ListMemories);
    wait();
    let state = app.state();
    // Handler ran without panic; memories field is accessible
    // (empty is fine -- no data inserted; SQL query correctness covered by test_insert_and_list_memories)
    let _ = state.memories.len(); // field is accessible
}

/// Verify DeleteMemory actor handler gracefully handles a nonexistent memory_id (MEM-05).
#[test]
fn test_delete_memory_action() {
    let app = make_app();
    app.dispatch(AppAction::DeleteMemory { memory_id: "nonexistent".to_string() });
    wait();
    let state = app.state();
    assert!(
        state.memories.is_empty(),
        "memories should remain empty after deleting nonexistent memory"
    );
}

/// Verify UpdateMemory actor handler gracefully handles a nonexistent memory_id (MEM-06).
#[test]
fn test_update_memory_action() {
    let app = make_app();
    app.dispatch(AppAction::UpdateMemory {
        memory_id: "nonexistent".to_string(),
        content: "Updated content".to_string(),
    });
    wait();
    // No panic = handler is wired correctly
    let state = app.state();
    assert!(
        state.memories.is_empty(),
        "memories should remain empty when updating nonexistent memory"
    );
}

/// Verify Screen::Memories navigation auto-loads memories without panic.
#[test]
fn test_memories_screen_navigation() {
    let app = make_app();
    app.dispatch(AppAction::PushScreen { screen: Screen::Memories });
    wait();
    // The screen navigation and auto-load of memories should complete without panic
    let state = app.state();
    assert_eq!(
        state.router.current_screen,
        Screen::Memories,
        "current_screen should be Memories after PushScreen"
    );
}
