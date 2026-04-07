//! Phase 27: Chat tool use tests.
//! Wave 0 stubs -- these tests reference types/functions that Plan 01 creates.

use crate::persistence::{queries, Database};

#[test]
fn test_migration_v16() {
    // Verify migrations run successfully and conversations table has tools_enabled column
    let db = Database::open(":memory:").unwrap();
    // After migrations, tools_enabled column should exist
    let count: i64 = db.conn()
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'tools_enabled'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "conversations table should have tools_enabled column after migrations");
}

#[test]
fn test_tools_enabled_persistence() {
    let db = Database::open(":memory:").unwrap();
    let row = queries::ConversationRow {
        id: "conv-tools-1".into(),
        title: "Tools Test".into(),
        model_id: "gpt-4".into(),
        backend_id: "tinfoil".into(),
        system_prompt: None,
        created_at: 1000,
        updated_at: 1000,
        tools_enabled: true,
    };
    queries::insert_conversation(db.conn(), &row).unwrap();
    let convs = queries::list_conversations(db.conn()).unwrap();
    assert_eq!(convs.len(), 1);
    assert!(convs[0].tools_enabled, "tools_enabled should be true after insert with true");
}

#[test]
fn test_tools_enabled_default() {
    let db = Database::open(":memory:").unwrap();
    let row = queries::ConversationRow {
        id: "conv-default-1".into(),
        title: "Default Test".into(),
        model_id: "gpt-4".into(),
        backend_id: "tinfoil".into(),
        system_prompt: None,
        created_at: 1000,
        updated_at: 1000,
        tools_enabled: false,
    };
    queries::insert_conversation(db.conn(), &row).unwrap();
    let convs = queries::list_conversations(db.conn()).unwrap();
    assert!(!convs[0].tools_enabled, "tools_enabled should default to false");
}

#[test]
fn test_update_conversation_tools_enabled() {
    let db = Database::open(":memory:").unwrap();
    let row = queries::ConversationRow {
        id: "conv-update-1".into(),
        title: "Update Test".into(),
        model_id: "gpt-4".into(),
        backend_id: "tinfoil".into(),
        system_prompt: None,
        created_at: 1000,
        updated_at: 1000,
        tools_enabled: false,
    };
    queries::insert_conversation(db.conn(), &row).unwrap();
    queries::update_conversation_tools_enabled(db.conn(), "conv-update-1", true, 2000).unwrap();
    let convs = queries::list_conversations(db.conn()).unwrap();
    assert!(convs[0].tools_enabled, "tools_enabled should be true after update");
}

#[test]
fn test_build_chat_tools() {
    use crate::agent::tools::build_chat_tools;
    use async_openai::types::chat::ChatCompletionTools;

    let tools = build_chat_tools(false, true);
    let names: Vec<String> = tools.iter().filter_map(|t| match t {
        ChatCompletionTools::Function(f) => Some(f.function.name.clone()),
        _ => None,
    }).collect();

    assert!(!names.contains(&"finish".to_string()), "chat tools must not contain finish");
    assert!(!names.contains(&"search_documents".to_string()), "chat tools must not contain search_documents when include_doc_search=false");
    assert!(!names.contains(&"read_document".to_string()), "chat tools must not contain read_document when include_doc_search=false");
    assert!(names.contains(&"web_search".to_string()), "chat tools must contain web_search when brave_api_key_set=true");
}

#[test]
fn test_chat_tools_no_brave() {
    use crate::agent::tools::build_chat_tools;
    use async_openai::types::chat::ChatCompletionTools;

    let tools = build_chat_tools(false, false);
    let names: Vec<String> = tools.iter().filter_map(|t| match t {
        ChatCompletionTools::Function(f) => Some(f.function.name.clone()),
        _ => None,
    }).collect();

    assert!(!names.contains(&"web_search".to_string()), "chat tools must not contain web_search when brave_api_key_set=false");
    assert!(!names.contains(&"finish".to_string()), "chat tools must not contain finish");
}

#[test]
fn test_chat_tools_with_docs() {
    use crate::agent::tools::build_chat_tools;
    use async_openai::types::chat::ChatCompletionTools;

    let tools = build_chat_tools(true, true);
    let names: Vec<String> = tools.iter().filter_map(|t| match t {
        ChatCompletionTools::Function(f) => Some(f.function.name.clone()),
        _ => None,
    }).collect();

    assert!(names.contains(&"search_documents".to_string()), "chat tools must contain search_documents when include_doc_search=true");
    assert!(names.contains(&"read_document".to_string()), "chat tools must contain read_document when include_doc_search=true");
    assert!(!names.contains(&"finish".to_string()), "chat tools must not contain finish even with docs");
}
