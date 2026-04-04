use rusqlite::Connection;

use super::error::PersistenceError;

// ── Row types ─────────────────────────────────────────────────────────────────

/// A row from the `backends` table.
#[derive(Debug, Clone)]
pub struct BackendRow {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model_list: String,
    pub tee_type: String,
    pub display_order: i64,
    pub is_active: i64,
    pub created_at: i64,
    /// Per D-05 (Phase 16): maximum concurrent requests enforced via Semaphore.
    pub max_concurrent_requests: i64,
    /// Per D-02 (Phase 17): capability flag loaded from MIGRATION_V13 column.
    pub supports_tool_use: bool,
}

/// A row from the `conversations` table.
#[derive(Debug, Clone)]
pub struct ConversationRow {
    pub id: String,
    pub title: String,
    pub model_id: String,
    pub backend_id: String,
    pub system_prompt: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A row from the `messages` table.
#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    pub token_count: Option<i64>,
}

// ── Backend queries ───────────────────────────────────────────────────────────

/// Return all backends ordered by `display_order`.
pub fn list_backends(conn: &Connection) -> Result<Vec<BackendRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, base_url, model_list, tee_type, display_order, is_active, created_at, max_concurrent_requests, supports_tool_use
         FROM backends ORDER BY display_order",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BackendRow {
                id: row.get(0)?,
                name: row.get(1)?,
                base_url: row.get(2)?,
                model_list: row.get(3)?,
                tee_type: row.get(4)?,
                display_order: row.get(5)?,
                is_active: row.get(6)?,
                created_at: row.get(7)?,
                max_concurrent_requests: row.get(8)?,
                supports_tool_use: row.get::<_, i64>(9)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Return the ID of the first active backend ordered by `display_order`, if any.
pub fn get_active_backend_id(conn: &Connection) -> Result<Option<String>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id FROM backends WHERE is_active = 1 ORDER BY display_order LIMIT 1",
    )?;
    let result = stmt.query_row([], |row| row.get(0));
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(PersistenceError::from(e)),
    }
}

// ── Conversation queries ──────────────────────────────────────────────────────

/// Insert a new conversation row.
pub fn insert_conversation(
    conn: &Connection,
    row: &ConversationRow,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO conversations (id, title, model_id, backend_id, system_prompt, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.title,
        row.model_id,
        row.backend_id,
        row.system_prompt,
        row.created_at,
        row.updated_at,
    ])?;
    Ok(())
}

/// Return all conversations ordered by `updated_at` descending (newest first).
pub fn list_conversations(conn: &Connection) -> Result<Vec<ConversationRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, model_id, backend_id, system_prompt, created_at, updated_at
         FROM conversations ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConversationRow {
                id: row.get(0)?,
                title: row.get(1)?,
                model_id: row.get(2)?,
                backend_id: row.get(3)?,
                system_prompt: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// A row from the `agent_sessions` table.
#[derive(Debug, Clone)]
pub struct AgentSessionRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub backend_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A row from the `agent_steps` table.
#[derive(Debug, Clone)]
pub struct AgentStepRow {
    pub id: String,
    pub session_id: String,
    pub step_number: i64,
    pub action_type: String,
    pub action_payload: String,
    pub result: Option<String>,
    pub status: String,
    pub created_at: i64,
}

// ── Agent session queries ─────────────────────────────────────────────────────

/// Insert a new agent session row.
pub fn insert_agent_session(
    conn: &Connection,
    row: &AgentSessionRow,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO agent_sessions (id, title, status, backend_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.title,
        row.status,
        row.backend_id,
        row.created_at,
        row.updated_at,
    ])?;
    Ok(())
}

/// Return all agent sessions ordered by `updated_at` descending (newest first).
pub fn list_agent_sessions(conn: &Connection) -> Result<Vec<AgentSessionRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, status, backend_id, created_at, updated_at
         FROM agent_sessions ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(AgentSessionRow {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                backend_id: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Insert a new agent step row.
pub fn insert_agent_step(conn: &Connection, row: &AgentStepRow) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO agent_steps (id, session_id, step_number, action_type, action_payload, result, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.session_id,
        row.step_number,
        row.action_type,
        row.action_payload,
        row.result,
        row.status,
        row.created_at,
    ])?;
    Ok(())
}

/// Update the status of an agent session.
pub fn update_agent_session_status(
    conn: &Connection,
    session_id: &str,
    status: &str,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE agent_sessions SET status = ?1, updated_at = ?2 WHERE id = ?3")?
        .execute(rusqlite::params![status, updated_at, session_id])?;
    Ok(())
}

/// Update the status and result of an agent step.
#[allow(dead_code)]
pub fn update_agent_step_status(
    conn: &Connection,
    step_id: &str,
    status: &str,
    result: Option<&str>,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE agent_steps SET status = ?1, result = ?2 WHERE id = ?3")?
        .execute(rusqlite::params![status, result, step_id])?;
    Ok(())
}

/// Count the number of steps for an agent session.
pub fn count_agent_steps(conn: &Connection, session_id: &str) -> Result<i64, PersistenceError> {
    let count: i64 = conn
        .prepare_cached("SELECT COUNT(*) FROM agent_steps WHERE session_id = ?1")?
        .query_row(rusqlite::params![session_id], |row| row.get(0))?;
    Ok(count)
}

/// Return all steps for an agent session ordered by `step_number` ascending.
pub fn list_agent_steps(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<AgentStepRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, session_id, step_number, action_type, action_payload, result, status, created_at
         FROM agent_steps WHERE session_id = ?1 ORDER BY step_number ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![session_id], |row| {
            Ok(AgentStepRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                step_number: row.get(2)?,
                action_type: row.get(3)?,
                action_payload: row.get(4)?,
                result: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── Message queries ───────────────────────────────────────────────────────────

/// Insert a new message row.
pub fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.conversation_id,
        row.role,
        row.content,
        row.created_at,
        row.token_count,
    ])?;
    Ok(())
}

/// Return all messages for a conversation ordered by `created_at` ascending.
pub fn list_messages(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<MessageRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, conversation_id, role, content, created_at, token_count
         FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![conversation_id], |row| {
            Ok(MessageRow {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                token_count: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── New Phase 5 conversation management queries ───────────────────────────────

/// Delete a conversation and all its messages.
///
/// Messages are deleted first (FK constraint: messages.conversation_id REFERENCES conversations.id).
/// The schema does not declare ON DELETE CASCADE so we must delete messages explicitly.
pub fn delete_conversation(
    conn: &Connection,
    conversation_id: &str,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM messages WHERE conversation_id = ?1")?
        .execute(rusqlite::params![conversation_id])?;
    conn.prepare_cached("DELETE FROM conversations WHERE id = ?1")?
        .execute(rusqlite::params![conversation_id])?;
    Ok(())
}

/// Rename a conversation, updating `updated_at`.
pub fn rename_conversation(
    conn: &Connection,
    conversation_id: &str,
    new_title: &str,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE conversations SET title = ?2, updated_at = ?3 WHERE id = ?1")?
        .execute(rusqlite::params![conversation_id, new_title, updated_at])?;
    Ok(())
}

/// Update the model_id for a conversation, refreshing `updated_at`.
pub fn update_conversation_model(
    conn: &Connection,
    conversation_id: &str,
    model_id: &str,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE conversations SET model_id = ?2, updated_at = ?3 WHERE id = ?1")?
        .execute(rusqlite::params![conversation_id, model_id, updated_at])?;
    Ok(())
}

/// Update the system_prompt for a conversation, refreshing `updated_at`.
///
/// Pass `None` to clear the per-conversation system prompt (falls back to global default).
pub fn update_conversation_system_prompt(
    conn: &Connection,
    conversation_id: &str,
    system_prompt: Option<&str>,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "UPDATE conversations SET system_prompt = ?2, updated_at = ?3 WHERE id = ?1",
    )?
    .execute(rusqlite::params![
        conversation_id,
        system_prompt,
        updated_at
    ])?;
    Ok(())
}

/// Touch `updated_at` for a conversation (called when a new message arrives).
pub fn update_conversation_updated_at(
    conn: &Connection,
    conversation_id: &str,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE conversations SET updated_at = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![conversation_id, updated_at])?;
    Ok(())
}

/// Delete all messages in a conversation created *after* `after_created_at`.
///
/// Used for EditMessage: truncate the message history after the edit point
/// so the conversation can be re-submitted from that point forward.
pub fn delete_messages_after(
    conn: &Connection,
    conversation_id: &str,
    after_created_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM messages WHERE conversation_id = ?1 AND created_at > ?2")?
        .execute(rusqlite::params![conversation_id, after_created_at])?;
    Ok(())
}

/// Delete a single message by ID.
///
/// Used for RetryLastMessage: remove the last assistant message so it can be regenerated.
pub fn delete_message(conn: &Connection, message_id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM messages WHERE id = ?1")?
        .execute(rusqlite::params![message_id])?;
    Ok(())
}

// ── Backend CRUD ──────────────────────────────────────────────────────────────

/// Insert a new backend row.
pub fn insert_backend(conn: &Connection, row: &BackendRow) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO backends (id, name, base_url, model_list, tee_type, display_order, is_active, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.name,
        row.base_url,
        row.model_list,
        row.tee_type,
        row.display_order,
        row.is_active,
        row.created_at,
    ])?;
    Ok(())
}

/// Delete a backend row by ID.
pub fn delete_backend(conn: &Connection, backend_id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM backends WHERE id = ?1")?
        .execute(rusqlite::params![backend_id])?;
    Ok(())
}

/// Update the display_order for a backend (used for drag-to-reorder in the UI).
pub fn update_backend_display_order(
    conn: &Connection,
    backend_id: &str,
    display_order: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE backends SET display_order = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![backend_id, display_order])?;
    Ok(())
}

/// Update the model_list for a backend (refreshed from provider on model discovery).
pub fn update_backend_models(
    conn: &Connection,
    backend_id: &str,
    model_list: &str,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE backends SET model_list = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![backend_id, model_list])?;
    Ok(())
}

/// Update the backend_id for a conversation (used when user switches backend mid-conversation).
pub fn update_conversation_backend(
    conn: &Connection,
    conversation_id: &str,
    backend_id: &str,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE conversations SET backend_id = ?2, updated_at = ?3 WHERE id = ?1")?
        .execute(rusqlite::params![conversation_id, backend_id, updated_at])?;
    Ok(())
}

// ── Backend health persistence ────────────────────────────────────────────────

/// A row from the `backend_health` table.
#[derive(Debug, Clone)]
pub struct BackendHealthRow {
    pub backend_id: String,
    pub consecutive_failures: u32,
    pub last_failure_at: Option<i64>,
    pub state: String,
    pub backoff_until: Option<i64>,
}

/// Insert or replace a backend health row (upsert by primary key backend_id).
pub fn upsert_backend_health(
    conn: &Connection,
    row: &BackendHealthRow,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT OR REPLACE INTO backend_health
             (backend_id, consecutive_failures, last_failure_at, state, backoff_until)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(rusqlite::params![
        row.backend_id,
        row.consecutive_failures,
        row.last_failure_at,
        row.state,
        row.backoff_until,
    ])?;
    Ok(())
}

/// Return all backend health rows.
pub fn list_backend_health(conn: &Connection) -> Result<Vec<BackendHealthRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT backend_id, consecutive_failures, last_failure_at, state, backoff_until
         FROM backend_health",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BackendHealthRow {
                backend_id: row.get(0)?,
                consecutive_failures: row.get(1)?,
                last_failure_at: row.get(2)?,
                state: row.get(3)?,
                backoff_until: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Delete the health row for a specific backend (used when a backend is removed).
pub fn delete_backend_health(conn: &Connection, backend_id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM backend_health WHERE backend_id = ?1")?
        .execute(rusqlite::params![backend_id])?;
    Ok(())
}

// ── RAG document and chunk queries (Phase 8) ──────────────────────────────────

/// A row from the `documents` table.
#[derive(Debug, Clone)]
pub struct DocumentRow {
    pub id: String,
    pub name: String,
    /// One of: "pdf", "txt", "md"
    pub format: String,
    pub size_bytes: i64,
    pub ingestion_date: i64,
    pub chunk_count: i64,
}

/// A row from the `chunks` table.
///
/// `id` is an INTEGER PRIMARY KEY AUTOINCREMENT -- this rowid is used directly
/// as the usearch vector key so there is a 1:1 mapping between SQLite chunk rows
/// and HNSW index entries.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChunkRow {
    /// SQLite rowid -- also the usearch vector key.
    pub id: i64,
    pub document_id: String,
    pub chunk_index: i64,
    pub text: String,
    pub char_offset: i64,
}

/// Insert a new document row.
pub fn insert_document(conn: &Connection, row: &DocumentRow) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO documents (id, name, format, size_bytes, ingestion_date, chunk_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.name,
        row.format,
        row.size_bytes,
        row.ingestion_date,
        row.chunk_count,
    ])?;
    Ok(())
}

/// Return all documents ordered by `ingestion_date` descending (newest first).
pub fn list_documents(conn: &Connection) -> Result<Vec<DocumentRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, format, size_bytes, ingestion_date, chunk_count
         FROM documents ORDER BY ingestion_date DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DocumentRow {
                id: row.get(0)?,
                name: row.get(1)?,
                format: row.get(2)?,
                size_bytes: row.get(3)?,
                ingestion_date: row.get(4)?,
                chunk_count: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Delete a document by ID. Chunks are deleted via ON DELETE CASCADE.
pub fn delete_document(conn: &Connection, document_id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM documents WHERE id = ?1")?
        .execute(rusqlite::params![document_id])?;
    Ok(())
}

/// Insert a new chunk row and return its rowid (used as the usearch vector key).
///
/// The returned `i64` is `conn.last_insert_rowid()` -- the SQLite AUTOINCREMENT rowid
/// that serves as the unique key in the HNSW index.
pub fn insert_chunk(
    conn: &Connection,
    document_id: &str,
    chunk_index: i64,
    text: &str,
    char_offset: i64,
) -> Result<i64, PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO chunks (document_id, chunk_index, text, char_offset)
         VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(rusqlite::params![
        document_id,
        chunk_index,
        text,
        char_offset
    ])?;
    Ok(conn.last_insert_rowid())
}

/// Return all chunks for a document ordered by `chunk_index` ascending.
pub fn list_chunks_for_document(
    conn: &Connection,
    document_id: &str,
) -> Result<Vec<ChunkRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, document_id, chunk_index, text, char_offset
         FROM chunks WHERE document_id = ?1 ORDER BY chunk_index ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![document_id], |row| {
            Ok(ChunkRow {
                id: row.get(0)?,
                document_id: row.get(1)?,
                chunk_index: row.get(2)?,
                text: row.get(3)?,
                char_offset: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Delete all chunks for a document and return their rowids (for usearch removal).
///
/// Collects the rowids before deleting so the caller can remove the corresponding
/// vectors from the HNSW index.
pub fn delete_chunks_for_document(
    conn: &Connection,
    document_id: &str,
) -> Result<Vec<i64>, PersistenceError> {
    // Collect rowids first
    let mut stmt = conn.prepare_cached("SELECT id FROM chunks WHERE document_id = ?1")?;
    let rowids: Vec<i64> = stmt
        .query_map(rusqlite::params![document_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    // Delete the rows
    conn.prepare_cached("DELETE FROM chunks WHERE document_id = ?1")?
        .execute(rusqlite::params![document_id])?;

    Ok(rowids)
}

/// Retrieve chunk text for a set of rowids returned by usearch search.
///
/// Used after an HNSW search: the search returns `(key, distance)` pairs where
/// `key` is the SQLite chunk rowid. This function fetches the text for display.
///
/// Returns `(rowid, text)` pairs. Rowids not found in the DB are silently omitted.
pub fn get_chunk_text_by_rowids(
    conn: &Connection,
    rowids: &[i64],
) -> Result<Vec<(i64, String)>, PersistenceError> {
    if rowids.is_empty() {
        return Ok(vec![]);
    }
    // Build IN clause with positional parameters
    let placeholders: String = rowids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT id, text FROM chunks WHERE id IN ({})", placeholders);
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = rowids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Update the attached_document_ids for a conversation.
///
/// Serialises `doc_ids` as a JSON array string. If `doc_ids` is empty, sets NULL.
/// Used by the actor when the user attaches or detaches documents from a conversation.
pub fn update_conversation_attached_docs(
    conn: &Connection,
    conversation_id: &str,
    doc_ids: &[String],
) -> Result<(), PersistenceError> {
    let json_value: Option<String> = if doc_ids.is_empty() {
        None
    } else {
        Some(serde_json::to_string(doc_ids).map_err(PersistenceError::from)?)
    };
    conn.prepare_cached("UPDATE conversations SET attached_document_ids = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![conversation_id, json_value])?;
    Ok(())
}

/// Read the attached_document_ids for a conversation.
///
/// Deserialises the JSON array. Returns an empty vec if the column is NULL
/// or the conversation does not exist.
pub fn get_conversation_attached_docs(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<String>, PersistenceError> {
    let mut stmt =
        conn.prepare_cached("SELECT attached_document_ids FROM conversations WHERE id = ?1")?;
    match stmt.query_row(rusqlite::params![conversation_id], |row| {
        row.get::<_, Option<String>>(0)
    }) {
        Ok(Some(json)) => {
            let ids: Vec<String> = serde_json::from_str(&json).map_err(PersistenceError::from)?;
            Ok(ids)
        }
        Ok(None) => Ok(vec![]),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(vec![]),
        Err(e) => Err(PersistenceError::from(e)),
    }
}

/// Update the chunk_count for a document (called after all chunks have been inserted).
pub fn update_document_chunk_count(
    conn: &Connection,
    document_id: &str,
    chunk_count: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE documents SET chunk_count = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![document_id, chunk_count])?;
    Ok(())
}

// ── Settings queries ──────────────────────────────────────────────────────────

/// Read a setting value by key. Returns `None` if the key does not exist.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, PersistenceError> {
    let mut stmt = conn.prepare_cached("SELECT value FROM settings WHERE key = ?1")?;
    match stmt.query_row(rusqlite::params![key], |row| row.get(0)) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(PersistenceError::from(e)),
    }
}

/// Insert or replace a setting value.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)")?
        .execute(rusqlite::params![key, value])?;
    Ok(())
}

/// Load the combined TEE attestation policy from the settings table.
///
/// Reads `tee_policy_tdx` and `tee_policy_snp` keys and deserializes them into
/// [`crate::attestation::TeePolicy`]. Falls back to compiled defaults for either
/// sub-policy if the key is absent (e.g. before MIGRATION_V14 has run).
pub fn get_tee_policy(
    conn: &Connection,
) -> Result<crate::attestation::TeePolicy, PersistenceError> {
    use crate::attestation::{SnpPolicy, TdxPolicy, TeePolicy};

    let tdx = match get_setting(conn, "tee_policy_tdx")? {
        Some(json) => serde_json::from_str::<TdxPolicy>(&json).map_err(PersistenceError::from)?,
        None => TdxPolicy::default(),
    };
    let snp = match get_setting(conn, "tee_policy_snp")? {
        Some(json) => serde_json::from_str::<SnpPolicy>(&json).map_err(PersistenceError::from)?,
        None => SnpPolicy::default(),
    };
    Ok(TeePolicy { tdx, snp })
}

// ── Memory queries (Phase 20) ─────────────────────────────────────────────────

/// A row from the `memories` table.
///
/// Each row stores one extracted memory fact, linked to the source conversation,
/// and keyed by `usearch_key` (the HNSW index entry) for semantic recall.
#[derive(Debug, Clone)]
pub struct MemoryRow {
    pub id: String,
    pub conversation_id: String,
    pub content: String,
    /// Integer key into the usearch HNSW index. UNIQUE per the schema constraint.
    pub usearch_key: i64,
    pub created_at: i64,
}

/// Insert a new memory row.
pub fn insert_memory(conn: &Connection, row: &MemoryRow) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO memories (id, conversation_id, content, usearch_key, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.conversation_id,
        row.content,
        row.usearch_key,
        row.created_at,
    ])?;
    Ok(())
}

/// Return all memory rows ordered by `created_at` descending (newest first).
pub fn list_memories(conn: &Connection) -> Result<Vec<MemoryRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, conversation_id, content, usearch_key, created_at
         FROM memories ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(MemoryRow {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                content: row.get(2)?,
                usearch_key: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Delete a single memory row by ID.
///
/// The caller is responsible for also removing the corresponding HNSW vector entry
/// using the `usearch_key` before calling this function.
pub fn delete_memory(conn: &Connection, memory_id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM memories WHERE id = ?1")?
        .execute(rusqlite::params![memory_id])?;
    Ok(())
}

/// Return the content of memories whose usearch_key is in `keys`.
///
/// Returns `Vec<(usearch_key, content)>` pairs. Missing keys are silently omitted.
/// If `keys` is empty, returns `Ok(vec![])` immediately without a DB query.
pub fn get_memory_content_by_usearch_keys(
    conn: &Connection,
    keys: &[i64],
) -> Result<Vec<(i64, String)>, PersistenceError> {
    if keys.is_empty() {
        return Ok(vec![]);
    }
    let placeholders: String = keys
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT usearch_key, content FROM memories WHERE usearch_key IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = keys
        .iter()
        .map(|k| k as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
