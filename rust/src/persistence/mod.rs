pub mod error;
pub mod queries;
pub mod schema;

pub use error::PersistenceError;
#[allow(unused_imports)]
pub use queries::{
    delete_backend, delete_backend_health, delete_chunks_for_document, delete_conversation,
    delete_document, delete_message, delete_messages_after, get_active_backend_id,
    get_chunk_text_by_rowids, get_conversation_attached_docs, get_setting, insert_agent_session,
    insert_agent_step, insert_backend, insert_chunk, insert_conversation, insert_document,
    insert_message, list_agent_sessions, list_agent_steps, list_backend_health, list_backends,
    list_chunks_for_document, list_conversations, list_documents, list_messages,
    rename_conversation, set_setting, update_backend_display_order, update_backend_models,
    update_conversation_attached_docs, update_conversation_backend, update_conversation_model,
    update_conversation_system_prompt, update_conversation_updated_at, update_document_chunk_count,
    upsert_backend_health, AgentSessionRow, AgentStepRow, BackendHealthRow, BackendRow, ChunkRow,
    ConversationRow, DocumentRow, MessageRow,
};

/// SQLite-backed application database.
///
/// Opens a connection with WAL journal mode and foreign key enforcement.
/// Runs all pending schema migrations on first open.
///
/// Per Pitfall 6 from Phase 3 RESEARCH.md: `rusqlite::Connection` is NOT Send+Sync.
/// This struct must only be used from the actor thread -- never move it into async tasks.
pub struct Database {
    conn: rusqlite::Connection,
}

impl Database {
    /// Open the database at `path` and run any pending migrations.
    ///
    /// Pass `":memory:"` for tests; pass an on-disk file path for production.
    pub fn open(path: &str) -> Result<Self, PersistenceError> {
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Run pending migrations in order, advancing `user_version` for each.
    fn run_migrations(&mut self) -> Result<(), PersistenceError> {
        let current: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap_or(0);
        for (idx, sql) in schema::MIGRATIONS.iter().enumerate() {
            let target = (idx + 1) as i32;
            if current < target {
                let tx = self.conn.transaction()?;
                tx.execute_batch(sql)
                    .map_err(|e| PersistenceError::MigrationFailed {
                        version: target,
                        message: e.to_string(),
                    })?;
                tx.pragma_update(None, "user_version", target)?;
                tx.commit()?;
            }
        }
        Ok(())
    }

    /// Return a reference to the underlying `rusqlite::Connection`.
    pub fn conn(&self) -> &rusqlite::Connection {
        &self.conn
    }
}
