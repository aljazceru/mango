//! Conversation auto-retention (quick/261018-conv-retention).
//!
//! Queries-level coverage: migration v27 column, archive sweep, delete sweep,
//! unarchive, and list filtering. Actor-level wiring (settings load + sweep on
//! startup) is exercised indirectly via load_post_unlock in other suites.

use crate::persistence::queries::{
    archive_conversations_older_than, delete_conversations_older_than, insert_conversation,
    insert_message, list_active_conversations, list_archived_conversations, list_conversations,
    list_messages, set_conversation_archived, ConversationRow, MessageRow,
};
use crate::persistence::Database;

fn conv(id: &str, updated_at: i64) -> ConversationRow {
    ConversationRow {
        id: id.into(),
        title: format!("conv {id}"),
        model_id: "m".into(),
        backend_id: "tinfoil".into(),
        system_prompt: None,
        created_at: updated_at,
        updated_at,
        tools_enabled: false,
    }
}

fn msg(id: &str, conversation_id: &str) -> MessageRow {
    MessageRow {
        id: id.into(),
        conversation_id: conversation_id.into(),
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
    }
}

#[test]
fn archive_sweep_archives_only_stale_active_conversations() {
    let db = Database::open(":memory:").unwrap();
    insert_conversation(db.conn(), &conv("old", 100)).unwrap();
    insert_conversation(db.conn(), &conv("fresh", 9_000)).unwrap();

    let n = archive_conversations_older_than(db.conn(), 500, 1_000).unwrap();
    assert_eq!(n, 1, "only the stale conversation is archived");

    // Idempotent: already-archived rows are skipped, archived_at preserved.
    let n2 = archive_conversations_older_than(db.conn(), 500, 2_000).unwrap();
    assert_eq!(n2, 0, "re-running the sweep is a no-op");

    let active = list_active_conversations(db.conn()).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "fresh");

    let archived = list_archived_conversations(db.conn()).unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, "old");

    // list_conversations (unfiltered) still sees both — internal lookups
    // (system-prompt fetch etc.) must keep working for archived rows.
    assert_eq!(list_conversations(db.conn()).unwrap().len(), 2);
}

#[test]
fn delete_sweep_removes_conversations_and_messages() {
    let db = Database::open(":memory:").unwrap();
    insert_conversation(db.conn(), &conv("old", 100)).unwrap();
    insert_message(db.conn(), &msg("m1", "old")).unwrap();
    insert_conversation(db.conn(), &conv("fresh", 9_000)).unwrap();
    insert_message(db.conn(), &msg("m2", "fresh")).unwrap();

    // Archived rows are deleted too — delete means delete.
    set_conversation_archived(db.conn(), "old", true, 500).unwrap();

    let n = delete_conversations_older_than(db.conn(), 500).unwrap();
    assert_eq!(n, 1);

    assert_eq!(list_conversations(db.conn()).unwrap().len(), 1);
    let remaining: Vec<String> = list_conversations(db.conn())
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(remaining, vec!["fresh".to_string()]);
    assert!(list_messages(db.conn(), "old").unwrap().is_empty());
}

#[test]
fn unarchive_restores_conversation_to_active_list() {
    let db = Database::open(":memory:").unwrap();
    insert_conversation(db.conn(), &conv("c1", 100)).unwrap();
    set_conversation_archived(db.conn(), "c1", true, 500).unwrap();
    assert!(list_active_conversations(db.conn()).unwrap().is_empty());

    set_conversation_archived(db.conn(), "c1", false, 900).unwrap();
    assert!(list_archived_conversations(db.conn()).unwrap().is_empty());
    let active = list_active_conversations(db.conn()).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "c1");
}
