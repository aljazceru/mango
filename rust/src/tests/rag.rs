use crate::persistence::queries::{
    delete_document, get_chunk_text_by_rowids, get_conversation_attached_docs, insert_chunk,
    insert_conversation, insert_document, list_chunks_for_document, list_documents,
    update_conversation_attached_docs, ConversationRow, DocumentRow,
};
use crate::persistence::Database;
use crate::{AppAction, EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};
/// RAG tests for Phase 8.
///
/// Covers two categories:
/// 1. Persistence-level: MIGRATION_V6, document/chunk CRUD, conversation attached_document_ids.
/// 2. Actor integration: IngestDocument, DeleteDocument, Attach/Detach pipeline via FfiApp.
use std::time::Duration;

// ── MIGRATION_V6 tests ────────────────────────────────────────────────────────

/// Verify that opening a fresh database creates the documents and chunks tables.
#[test]
fn test_migration_v6_creates_tables() {
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
        tables.contains(&"documents".to_string()),
        "documents table should exist after MIGRATION_V6"
    );
    assert!(
        tables.contains(&"chunks".to_string()),
        "chunks table should exist after MIGRATION_V6"
    );
}

/// Verify migration version is 6 after opening a fresh database.
#[test]
fn test_migration_v6_version() {
    let db = Database::open(":memory:").unwrap();
    let version: i32 = db
        .conn()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(
        version, 15,
        "user_version should be 15 after all migrations including MIGRATION_V15"
    );
}

/// Verify that conversations table has the new attached_document_ids column.
#[test]
fn test_migration_v6_attached_docs_column() {
    let db = Database::open(":memory:").unwrap();
    let cols: Vec<String> = db
        .conn()
        .prepare("PRAGMA table_info(conversations)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        cols.contains(&"attached_document_ids".to_string()),
        "conversations.attached_document_ids column should exist after MIGRATION_V6; found: {:?}",
        cols
    );
}

// ── Document CRUD tests ───────────────────────────────────────────────────────

/// Helper to create a test DocumentRow.
fn make_doc(id: &str, name: &str) -> DocumentRow {
    DocumentRow {
        id: id.into(),
        name: name.into(),
        format: "txt".into(),
        size_bytes: 1024,
        ingestion_date: 1000,
        chunk_count: 0,
    }
}

#[test]
fn test_document_crud() {
    let db = Database::open(":memory:").unwrap();

    // Insert a document
    let doc = make_doc("doc-1", "test.txt");
    insert_document(db.conn(), &doc).unwrap();

    // List documents
    let docs = list_documents(db.conn()).unwrap();
    assert_eq!(docs.len(), 1, "Should have 1 document after insert");
    assert_eq!(docs[0].id, "doc-1");
    assert_eq!(docs[0].name, "test.txt");
    assert_eq!(docs[0].format, "txt");
    assert_eq!(docs[0].size_bytes, 1024);
    assert_eq!(docs[0].ingestion_date, 1000);
    assert_eq!(docs[0].chunk_count, 0);

    // Delete the document
    delete_document(db.conn(), "doc-1").unwrap();
    let docs_after = list_documents(db.conn()).unwrap();
    assert_eq!(docs_after.len(), 0, "Should have 0 documents after delete");
}

// ── Chunk CRUD tests ──────────────────────────────────────────────────────────

#[test]
fn test_chunk_crud() {
    let db = Database::open(":memory:").unwrap();

    // Insert document first (FK constraint)
    insert_document(db.conn(), &make_doc("doc-chunks", "sample.txt")).unwrap();

    // Insert chunks and verify returned rowids are positive and unique
    let rowid1 = insert_chunk(db.conn(), "doc-chunks", 0, "first chunk text", 0).unwrap();
    let rowid2 = insert_chunk(db.conn(), "doc-chunks", 1, "second chunk text", 100).unwrap();
    let rowid3 = insert_chunk(db.conn(), "doc-chunks", 2, "third chunk text", 200).unwrap();

    assert!(rowid1 > 0, "Rowid should be positive");
    assert_ne!(rowid1, rowid2, "Rowids should be unique");
    assert_ne!(rowid2, rowid3, "Rowids should be unique");

    // List chunks ordered by chunk_index
    let chunks = list_chunks_for_document(db.conn(), "doc-chunks").unwrap();
    assert_eq!(chunks.len(), 3, "Should have 3 chunks");
    assert_eq!(chunks[0].chunk_index, 0);
    assert_eq!(chunks[0].text, "first chunk text");
    assert_eq!(chunks[0].char_offset, 0);
    assert_eq!(chunks[1].chunk_index, 1);
    assert_eq!(chunks[1].text, "second chunk text");
    assert_eq!(chunks[2].chunk_index, 2);

    // Verify rowid matches id field
    assert_eq!(
        chunks[0].id, rowid1,
        "Chunk rowid should match insert_chunk return value"
    );
}

// ── Cascade delete tests ──────────────────────────────────────────────────────

#[test]
fn test_chunk_cascade_delete() {
    let db = Database::open(":memory:").unwrap();

    insert_document(db.conn(), &make_doc("doc-cascade", "cascade.txt")).unwrap();
    insert_chunk(db.conn(), "doc-cascade", 0, "chunk a", 0).unwrap();
    insert_chunk(db.conn(), "doc-cascade", 1, "chunk b", 50).unwrap();

    // Verify chunks exist before delete
    let before = list_chunks_for_document(db.conn(), "doc-cascade").unwrap();
    assert_eq!(
        before.len(),
        2,
        "Should have 2 chunks before document delete"
    );

    // Delete document -- chunks should cascade
    delete_document(db.conn(), "doc-cascade").unwrap();

    // Verify chunks are gone
    let after = list_chunks_for_document(db.conn(), "doc-cascade").unwrap();
    assert_eq!(
        after.len(),
        0,
        "Chunks should be deleted via ON DELETE CASCADE"
    );
}

// ── get_chunk_text_by_rowids tests ────────────────────────────────────────────

#[test]
fn test_get_chunk_text_by_rowids() {
    let db = Database::open(":memory:").unwrap();

    insert_document(db.conn(), &make_doc("doc-rowids", "rowid_test.txt")).unwrap();

    let rid1 = insert_chunk(db.conn(), "doc-rowids", 0, "alpha text", 0).unwrap();
    let rid2 = insert_chunk(db.conn(), "doc-rowids", 1, "beta text", 100).unwrap();
    let rid3 = insert_chunk(db.conn(), "doc-rowids", 2, "gamma text", 200).unwrap();

    // Retrieve text for all 3 rowids
    let results = get_chunk_text_by_rowids(db.conn(), &[rid1, rid2, rid3]).unwrap();
    assert_eq!(results.len(), 3, "Should return 3 (rowid, text) pairs");

    // Build a map for order-independent assertion
    let map: std::collections::HashMap<i64, String> = results.into_iter().collect();
    assert_eq!(map[&rid1], "alpha text");
    assert_eq!(map[&rid2], "beta text");
    assert_eq!(map[&rid3], "gamma text");
}

#[test]
fn test_get_chunk_text_by_rowids_empty() {
    let db = Database::open(":memory:").unwrap();
    let results = get_chunk_text_by_rowids(db.conn(), &[]).unwrap();
    assert_eq!(results.len(), 0, "Empty rowids should return empty vec");
}

// ── Conversation attached_document_ids tests ──────────────────────────────────

/// Helper to insert a conversation for testing attached docs.
fn insert_test_conversation(db: &Database, id: &str) {
    insert_conversation(
        db.conn(),
        &ConversationRow {
            id: id.into(),
            title: "Test Chat".into(),
            model_id: "model-x".into(),
            backend_id: "tinfoil".into(),
            system_prompt: None,
            created_at: 1000,
            updated_at: 1000,
        },
    )
    .unwrap();
}

#[test]
fn test_conversation_attached_docs_null_by_default() {
    let db = Database::open(":memory:").unwrap();
    insert_test_conversation(&db, "conv-null");

    let docs = get_conversation_attached_docs(db.conn(), "conv-null").unwrap();
    assert_eq!(
        docs.len(),
        0,
        "attached_document_ids should be empty vec when NULL"
    );
}

#[test]
fn test_conversation_attached_docs_round_trip() {
    let db = Database::open(":memory:").unwrap();
    insert_test_conversation(&db, "conv-attach");

    // Update with doc IDs
    let doc_ids = vec!["doc-a".to_string(), "doc-b".to_string()];
    update_conversation_attached_docs(db.conn(), "conv-attach", &doc_ids).unwrap();

    // Read back
    let result = get_conversation_attached_docs(db.conn(), "conv-attach").unwrap();
    assert_eq!(
        result, doc_ids,
        "Attached doc IDs should round-trip through JSON serialisation"
    );
}

#[test]
fn test_conversation_attached_docs_clear_with_empty() {
    let db = Database::open(":memory:").unwrap();
    insert_test_conversation(&db, "conv-clear");

    // First, set some docs
    update_conversation_attached_docs(db.conn(), "conv-clear", &["doc-x".to_string()]).unwrap();

    // Then clear by passing empty slice (should set NULL)
    update_conversation_attached_docs(db.conn(), "conv-clear", &[]).unwrap();

    let result = get_conversation_attached_docs(db.conn(), "conv-clear").unwrap();
    assert_eq!(
        result.len(),
        0,
        "Clearing with empty slice should result in empty vec"
    );
}

#[test]
fn test_conversation_attached_docs_nonexistent_conversation() {
    let db = Database::open(":memory:").unwrap();
    let result = get_conversation_attached_docs(db.conn(), "nonexistent").unwrap();
    assert_eq!(
        result.len(),
        0,
        "Nonexistent conversation should return empty vec"
    );
}

// ── Actor integration tests (Phase 8 Plan 02) ─────────────────────────────────

/// Helper: create FfiApp with in-memory DB and NullEmbeddingProvider.
fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    // 100ms is stable under parallel test load; actor init includes VectorIndex + document list load.
    std::thread::sleep(Duration::from_millis(100));
    app
}

/// Helper: sleep to let the actor process a dispatched action.
fn wait() {
    std::thread::sleep(Duration::from_millis(200));
}

/// Helper: ingest a document and poll for completion with a generous timeout.
///
/// IngestDocument dispatches spawn_blocking for embedding, then delivers EmbeddingComplete.
/// Under parallel test load the Tokio pool may be contended; poll with retries.
fn ingest_doc(app: &FfiApp, filename: &str) {
    let content =
        b"Hello world this is a test document with enough text to produce at least one chunk"
            .to_vec();
    app.dispatch(AppAction::IngestDocument {
        filename: filename.into(),
        content,
    });
    // Poll up to 2 seconds (20 x 100ms) for ingestion to complete (ingestion_progress = None).
    // NullEmbeddingProvider completes synchronously inside spawn_blocking,
    // but under heavy parallel test load the Tokio thread pool may be busy.
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        let state = app.state();
        // EmbeddingComplete sets ingestion_progress = None and adds the document
        if state.ingestion_progress.is_none() && state.documents.iter().any(|d| d.name == filename)
        {
            return;
        }
    }
    // Fallback: no match found after 2s -- test will fail with a clear assertion
}

/// Dispatch IngestDocument with a small text file.
/// After async EmbeddingComplete arrives, AppState.documents should have 1 entry.
#[test]
fn test_ingest_document() {
    let app = make_app();
    ingest_doc(&app, "test.txt");

    let state = app.state();
    assert_eq!(
        state.documents.len(),
        1,
        "Expected 1 document after IngestDocument"
    );
    assert_eq!(
        state.documents[0].name, "test.txt",
        "Document name should match filename"
    );
    assert_eq!(
        state.documents[0].format, "txt",
        "Format should be derived from extension"
    );
    assert!(
        state.documents[0].chunk_count >= 1,
        "Should have at least 1 chunk"
    );
    assert!(
        state.ingestion_progress.is_none(),
        "ingestion_progress should be None after EmbeddingComplete"
    );
}

/// After ingest, dispatch DeleteDocument. AppState.documents should be empty.
#[test]
fn test_delete_document() {
    let app = make_app();
    ingest_doc(&app, "to_delete.txt");

    let state = app.state();
    assert_eq!(
        state.documents.len(),
        1,
        "Precondition: 1 document after ingest"
    );
    let doc_id = state.documents[0].id.clone();

    app.dispatch(AppAction::DeleteDocument {
        document_id: doc_id,
    });
    wait();

    let state = app.state();
    assert!(
        state.documents.is_empty(),
        "documents should be empty after DeleteDocument"
    );
}

/// After ingest, dispatch AttachDocumentToConversation.
/// AppState.current_conversation_attached_docs should contain the document_id.
#[test]
fn test_attach_document_to_conversation() {
    let app = make_app();

    // Create a conversation first (NewConversation sets current_conversation_id)
    app.dispatch(AppAction::NewConversation);
    wait();

    // Ingest a document
    ingest_doc(&app, "attach_test.txt");

    let state = app.state();
    assert_eq!(
        state.documents.len(),
        1,
        "Precondition: 1 document after ingest"
    );
    let doc_id = state.documents[0].id.clone();

    // Attach
    app.dispatch(AppAction::AttachDocumentToConversation {
        document_id: doc_id.clone(),
    });
    wait();

    let state = app.state();
    assert!(
        state.current_conversation_attached_docs.contains(&doc_id),
        "current_conversation_attached_docs should contain the attached document_id"
    );
}

/// After attaching a document, dispatch DetachDocumentFromConversation.
/// AppState.current_conversation_attached_docs should be empty.
#[test]
fn test_detach_document() {
    let app = make_app();

    // Setup: conversation + document + attach
    app.dispatch(AppAction::NewConversation);
    wait();
    ingest_doc(&app, "detach_test.txt");
    let state = app.state();
    let doc_id = state.documents[0].id.clone();
    app.dispatch(AppAction::AttachDocumentToConversation {
        document_id: doc_id.clone(),
    });
    wait();

    // Precondition: document is attached
    let state = app.state();
    assert!(
        state.current_conversation_attached_docs.contains(&doc_id),
        "Precondition: document should be attached"
    );

    // Detach
    app.dispatch(AppAction::DetachDocumentFromConversation {
        document_id: doc_id.clone(),
    });
    wait();

    let state = app.state();
    assert!(
        !state.current_conversation_attached_docs.contains(&doc_id),
        "current_conversation_attached_docs should not contain doc_id after detach"
    );
    assert!(
        state.current_conversation_attached_docs.is_empty(),
        "current_conversation_attached_docs should be empty after detaching the only document"
    );
}

/// NullEmbeddingProvider completes ingestion without crashing.
/// State should be consistent: documents populated, no ingestion_progress.
#[test]
fn test_null_embedding_provider_no_crash() {
    let app = make_app();
    ingest_doc(&app, "null_emb.txt");

    let state = app.state();
    // Document was added (extraction and chunking succeeded)
    assert_eq!(
        state.documents.len(),
        1,
        "Document should appear even with NullEmbeddingProvider"
    );
    // Ingestion completed cleanly
    assert!(
        state.ingestion_progress.is_none(),
        "ingestion_progress should be None after completion"
    );
    // No panic occurred (test reached here)
}
