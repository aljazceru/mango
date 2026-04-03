use crate::persistence::{
    queries::{delete_memory, insert_memory, list_memories, MemoryRow},
    Database,
};
use crate::memory::extract::should_extract;

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
