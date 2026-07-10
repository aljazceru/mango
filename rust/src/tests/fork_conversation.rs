//! Tests for `persistence::queries::fork_conversation`.
//!
//! Covers invariants from quick/260423-93w-PLAN.md:
//!   1. Duplicates all messages in order with distinct ids.
//!   2. Inherits metadata (model_id, backend_id, system_prompt, tools_enabled) + title " (fork)" suffix.
//!   3. Source and fork diverge independently after a post-fork insert.
//!   4. `image_path` is referenced (string-equal), not copied.
//!   5. Forking a non-existent source returns Err.
//!   6. Forking an empty conversation is OK (0 messages).

use crate::persistence::queries::{
    fork_conversation, insert_conversation, insert_message, list_conversations, list_messages,
    ConversationRow, MessageRow,
};
use crate::persistence::Database;

fn mem_db() -> Database {
    Database::open(":memory:").expect("open in-memory database")
}

fn seed_source(db: &mut Database, id: &str, title: &str, tools_enabled: bool) -> ConversationRow {
    let row = ConversationRow {
        id: id.to_string(),
        title: title.to_string(),
        model_id: "gpt-test".to_string(),
        backend_id: "tinfoil".to_string(),
        system_prompt: Some("be nice".to_string()),
        created_at: 1000,
        updated_at: 1000,
        tools_enabled,
    };
    insert_conversation(db.conn(), &row).expect("insert source conversation");
    row
}

fn seed_message(
    db: &mut Database,
    id: &str,
    conv_id: &str,
    role: &str,
    content: &str,
    created_at: i64,
    image_path: Option<&str>,
) -> MessageRow {
    let row = MessageRow {
        id: id.to_string(),
        conversation_id: conv_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        created_at,
        token_count: None,
        image_path: image_path.map(|s| s.to_string()),
        route_backend_id: None,
        route_model_id: None,
        route_decision: None,
        route_reason: None,
        route_provider_name: None,
        route_tee_label: None,
        route_tee_verified: None,
    };
    insert_message(db.conn(), &row).expect("insert message");
    row
}

#[test]
fn test_fork_duplicates_messages_in_order() {
    let mut db = mem_db();
    seed_source(&mut db, "c-src", "Hello World", false);
    seed_message(&mut db, "m1", "c-src", "user", "hi", 1001, None);
    seed_message(&mut db, "m2", "c-src", "assistant", "hello!", 1002, None);
    seed_message(&mut db, "m3", "c-src", "user", "how are you?", 1003, None);

    fork_conversation(db.conn_mut(), "c-src", "c-fork", 2000).expect("fork");

    let src_msgs = list_messages(db.conn(), "c-src").expect("list src");
    let fork_msgs = list_messages(db.conn(), "c-fork").expect("list fork");
    assert_eq!(src_msgs.len(), 3);
    assert_eq!(fork_msgs.len(), 3);

    for (i, (s, f)) in src_msgs.iter().zip(fork_msgs.iter()).enumerate() {
        assert_eq!(s.role, f.role, "role mismatch at {i}");
        assert_eq!(s.content, f.content, "content mismatch at {i}");
        assert_ne!(s.id, f.id, "message ids must differ at {i}");
        assert_eq!(f.conversation_id, "c-fork");
    }
}

#[test]
fn test_fork_inherits_metadata() {
    let mut db = mem_db();
    seed_source(&mut db, "c-src", "My Chat", true);

    fork_conversation(db.conn_mut(), "c-src", "c-fork", 2000).expect("fork");

    let convs = list_conversations(db.conn()).expect("list convs");
    let src = convs.iter().find(|c| c.id == "c-src").expect("src present");
    let fork = convs
        .iter()
        .find(|c| c.id == "c-fork")
        .expect("fork present");

    assert_eq!(fork.title, "My Chat (fork)");
    assert_eq!(fork.model_id, src.model_id);
    assert_eq!(fork.backend_id, src.backend_id);
    assert_eq!(fork.system_prompt, src.system_prompt);
    assert_eq!(fork.tools_enabled, src.tools_enabled);
    assert_eq!(fork.created_at, 2000);
    assert_eq!(fork.updated_at, 2000);
}

#[test]
fn test_fork_source_and_fork_diverge_independently() {
    let mut db = mem_db();
    seed_source(&mut db, "c-src", "Divergence", false);
    seed_message(&mut db, "m1", "c-src", "user", "a", 1001, None);
    seed_message(&mut db, "m2", "c-src", "assistant", "b", 1002, None);
    seed_message(&mut db, "m3", "c-src", "user", "c", 1003, None);

    fork_conversation(db.conn_mut(), "c-src", "c-fork", 2000).expect("fork");

    // Insert a new message into the fork.
    seed_message(&mut db, "m-new", "c-fork", "assistant", "new!", 3000, None);

    let src_msgs = list_messages(db.conn(), "c-src").expect("list src");
    let fork_msgs = list_messages(db.conn(), "c-fork").expect("list fork");
    assert_eq!(src_msgs.len(), 3, "source unchanged");
    assert_eq!(fork_msgs.len(), 4, "fork grew by one");
}

#[test]
fn test_fork_preserves_image_path_reference() {
    let mut db = mem_db();
    seed_source(&mut db, "c-src", "With Image", false);
    seed_message(
        &mut db,
        "m1",
        "c-src",
        "user",
        "see pic",
        1001,
        Some("/tmp/img.mgo1"),
    );

    fork_conversation(db.conn_mut(), "c-src", "c-fork", 2000).expect("fork");

    let fork_msgs = list_messages(db.conn(), "c-fork").expect("list fork");
    assert_eq!(fork_msgs.len(), 1);
    assert_eq!(
        fork_msgs[0].image_path.as_deref(),
        Some("/tmp/img.mgo1"),
        "image_path must be byte-equal (reference, not copy)"
    );
}

#[test]
fn test_fork_missing_source_errors() {
    let mut db = mem_db();
    let result = fork_conversation(db.conn_mut(), "does-not-exist", "c-fork", 2000);
    assert!(result.is_err(), "forking missing source must error");

    // And no fork row should have been inserted.
    let convs = list_conversations(db.conn()).expect("list convs");
    assert!(convs.iter().all(|c| c.id != "c-fork"));
}

#[test]
fn test_fork_empty_conversation_ok() {
    let mut db = mem_db();
    seed_source(&mut db, "c-src", "Empty", false);

    fork_conversation(db.conn_mut(), "c-src", "c-fork", 2000).expect("fork");

    let fork_msgs = list_messages(db.conn(), "c-fork").expect("list fork");
    assert_eq!(fork_msgs.len(), 0);

    let convs = list_conversations(db.conn()).expect("list convs");
    assert!(convs.iter().any(|c| c.id == "c-fork"));
}
