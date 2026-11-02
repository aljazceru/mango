//! Conversation auto-retention (quick/261018-conv-retention).
//!
//! Query coverage includes archive/delete sweeps and shared image cleanup.
//! Actor regressions use realistic millisecond timestamps and archived chats.

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

#[test]
fn actor_retention_uses_millisecond_timestamps() {
    use crate::{apply_conversation_retention, ConversationRetentionMode};

    for mode in [
        ConversationRetentionMode::Archive,
        ConversationRetentionMode::Delete,
    ] {
        let mut actor = crate::image_red_tests::default_actor_state_for_image_tests();
        actor.app_state.conversation_retention_mode = mode;
        actor.app_state.conversation_retention_days = 30;
        let now = crate::now_secs();
        let day_ms = 86_400_000;
        let conn = actor.db.as_ref().unwrap().conn();
        insert_conversation(conn, &conv("old", now - 40 * day_ms)).unwrap();
        insert_conversation(conn, &conv("fresh", now - 10 * day_ms)).unwrap();

        apply_conversation_retention(&actor);

        let active = list_active_conversations(conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "fresh");
        match mode {
            ConversationRetentionMode::Archive => {
                assert_eq!(list_archived_conversations(conn).unwrap()[0].id, "old");
                let archived_at: i64 = conn
                    .query_row(
                        "SELECT archived_at FROM conversations WHERE id = 'old'",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(
                    archived_at >= now,
                    "archive timestamp must also be milliseconds"
                );
            }
            ConversationRetentionMode::Delete => {
                assert_eq!(list_conversations(conn).unwrap().len(), 1)
            }
            ConversationRetentionMode::Off => unreachable!(),
        }
    }
}

#[test]
fn delete_all_conversations_includes_archived() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("mango.db");
    {
        let db = Database::open(db_path.to_str().unwrap()).unwrap();
        insert_conversation(db.conn(), &conv("archived", crate::now_secs())).unwrap();
        set_conversation_archived(db.conn(), "archived", true, crate::now_secs()).unwrap();
    }
    let app = crate::FfiApp::new(
        dir.path().to_str().unwrap().into(),
        Box::new(crate::NullKeychainProvider),
        Box::new(crate::NullEmbeddingProvider),
        crate::EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    app.sync();
    app.dispatch(crate::AppAction::DeleteAllConversations);
    app.sync();
    let db = Database::open(db_path.to_str().unwrap()).unwrap();
    assert!(app.state().archived_conversations.is_empty());
    assert!(list_conversations(db.conn()).unwrap().is_empty());
}

#[test]
fn deleting_history_preserves_fork_images_until_last_reference() {
    use crate::persistence::queries::{
        delete_conversation, delete_message_with_image_cleanup as delete_message,
        delete_messages_after, fork_conversation,
    };

    for deletion in ["conversation", "retention", "after", "message"] {
        let dir = tempfile::tempdir().unwrap();
        let mut db = Database::open(dir.path().join("mango.db").to_str().unwrap()).unwrap();
        let images_dir = dir.path().join("images");
        std::fs::create_dir(&images_dir).unwrap();
        let image_path = images_dir.join("shared.jpg.mgo1");
        std::fs::write(&image_path, b"encrypted image").unwrap();
        insert_conversation(db.conn(), &conv("source", 100)).unwrap();
        let mut image_msg = msg("image", "source");
        image_msg.image_path = Some(image_path.to_str().unwrap().into());
        insert_message(db.conn(), &image_msg).unwrap();
        fork_conversation(db.conn_mut(), "source", "fork", 1000).unwrap();

        match deletion {
            "conversation" => delete_conversation(db.conn(), "source").unwrap(),
            "retention" => {
                delete_conversations_older_than(db.conn(), 500).unwrap();
            }
            "after" => delete_messages_after(db.conn(), "source", 0).unwrap(),
            "message" => delete_message(db.conn(), "image").unwrap(),
            _ => unreachable!(),
        }
        assert!(
            image_path.exists(),
            "{deletion} must retain the fork's image"
        );
        let fork_msg = list_messages(db.conn(), "fork").unwrap().remove(0);
        delete_message(db.conn(), &fork_msg.id).unwrap();
        assert!(!image_path.exists(), "last reference deleted by {deletion}");
    }
}

#[test]
fn image_cleanup_does_not_remove_files_outside_app_images() {
    use crate::persistence::queries::delete_message_with_image_cleanup as delete_message;

    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path().join("mango.db").to_str().unwrap()).unwrap();
    std::fs::create_dir(dir.path().join("images")).unwrap();
    insert_conversation(db.conn(), &conv("c", 100)).unwrap();
    for (id, path) in [
        ("external", dir.path().join("external.jpg.mgo1")),
        ("plaintext", dir.path().join("images/plain.jpg")),
    ] {
        std::fs::write(&path, b"keep").unwrap();
        let mut row = msg(id, "c");
        row.image_path = Some(path.to_str().unwrap().into());
        insert_message(db.conn(), &row).unwrap();
        delete_message(db.conn(), id).unwrap();
        assert!(path.exists());
    }
}

#[test]
fn attach_image_replacement_cleans_old_plaintext_but_not_same_path() {
    let dir = tempfile::tempdir().unwrap();
    {
        let db = Database::open(dir.path().join("mango.db").to_str().unwrap()).unwrap();
        let mut row = conv("vision", crate::now_secs());
        row.model_id = "qwen3-vl-30b".into();
        insert_conversation(db.conn(), &row).unwrap();
    }
    let staging = dir.path().join("image-attachments");
    std::fs::create_dir(&staging).unwrap();
    let old = staging.join("old.jpg");
    let new = staging.join("new.jpg");
    std::fs::write(&old, b"old image").unwrap();
    std::fs::write(&new, b"new image").unwrap();
    let app = crate::FfiApp::new(
        dir.path().to_str().unwrap().into(),
        Box::new(crate::NullKeychainProvider),
        Box::new(crate::NullEmbeddingProvider),
        crate::EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    app.sync();
    app.dispatch(crate::AppAction::LoadConversation {
        conversation_id: "vision".into(),
    });
    app.sync();
    for path in [&old, &old, &new] {
        app.dispatch(crate::AppAction::AttachImage {
            filename: path.file_name().unwrap().to_str().unwrap().into(),
            file_path: path.to_str().unwrap().into(),
            mime_type: "image/jpeg".into(),
        });
        app.sync();
        assert!(
            path.exists(),
            "reattaching the same path must not delete it"
        );
    }
    assert!(
        !old.exists(),
        "replacement must remove previous plaintext image"
    );
    assert!(new.exists());
    assert_eq!(app.state().pending_attachment.unwrap().filename, "new.jpg");
}

#[test]
fn temporary_retry_deletion_keeps_image_for_history_restore() {
    use crate::persistence::queries::{delete_message, delete_message_with_image_cleanup};

    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path().join("mango.db").to_str().unwrap()).unwrap();
    let images = dir.path().join("images");
    std::fs::create_dir(&images).unwrap();
    let path = images.join("retry.jpg.mgo1");
    std::fs::write(&path, b"encrypted").unwrap();
    insert_conversation(db.conn(), &conv("c", 100)).unwrap();
    let mut row = msg("retry", "c");
    row.image_path = Some(path.to_str().unwrap().into());
    insert_message(db.conn(), &row).unwrap();
    delete_message(db.conn(), "retry").unwrap();
    assert!(
        path.exists(),
        "rejected retry must be able to restore its image"
    );
    insert_message(db.conn(), &row).unwrap();
    delete_message_with_image_cleanup(db.conn(), "retry").unwrap();
    assert!(!path.exists());
}

#[test]
fn backend_reassignment_includes_archived_conversations() {
    for old_backend in ["removed", "hybrid:removed"] {
        let mut actor = crate::image_red_tests::default_actor_state_for_image_tests();
        actor.backends = vec![crate::llm::BackendConfig {
            id: "replacement".into(),
            name: "Replacement".into(),
            base_url: "https://example.invalid/v1".into(),
            api_key: "key".into(),
            models: vec!["replacement-model".into()],
            tee_type: crate::llm::TeeType::Unknown,
            max_concurrent_requests: 1,
            supports_tool_use: false,
        }];
        {
            let conn = actor.db.as_ref().unwrap().conn();
            let mut row = conv("archived", 100);
            row.backend_id = old_backend.into();
            insert_conversation(conn, &row).unwrap();
            set_conversation_archived(conn, "archived", true, 200).unwrap();
        }
        crate::refresh_conversations(&mut actor);
        assert!(actor.app_state.conversations.is_empty());
        if old_backend.starts_with("hybrid:") {
            crate::reassign_virtual_backend_references_excluding(&mut actor, old_backend, None);
        } else {
            crate::remove_backend_row_and_reassign(&mut actor, old_backend);
        }
        let rows = list_conversations(actor.db.as_ref().unwrap().conn()).unwrap();
        assert_eq!(rows[0].backend_id, "replacement");
        assert_eq!(rows[0].model_id, "replacement-model");
        assert_eq!(actor.app_state.archived_conversations.len(), 1);
    }
}
