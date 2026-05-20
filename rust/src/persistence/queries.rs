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
    /// Phase 27 (CHAT-TOOL-01): whether tool use is enabled for this conversation.
    /// Persisted as INTEGER (0/1) in the conversations.tools_enabled column (MIGRATION_V16).
    pub tools_enabled: bool,
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
    /// Absolute path to the encrypted image file (`{data_dir}/images/{id}.jpg.mgo1`).
    /// None for text-only messages. File is AES-256-GCM encrypted (MGO1 format, T-ECE-02).
    pub image_path: Option<String>,
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
        "INSERT INTO conversations (id, title, model_id, backend_id, system_prompt, created_at, updated_at, tools_enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.title,
        row.model_id,
        row.backend_id,
        row.system_prompt,
        row.created_at,
        row.updated_at,
        row.tools_enabled as i64,
    ])?;
    Ok(())
}

/// Return all conversations ordered by `updated_at` descending (newest first).
pub fn list_conversations(conn: &Connection) -> Result<Vec<ConversationRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, model_id, backend_id, system_prompt, created_at, updated_at, tools_enabled
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
                tools_enabled: row.get::<_, i64>(7)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Enable or disable tool use for a specific conversation (Phase 27, CHAT-TOOL-02).
///
/// Updates the `tools_enabled` column and refreshes `updated_at`. Used by the
/// `SetConversationToolsEnabled` actor action handler.
pub fn update_conversation_tools_enabled(
    conn: &Connection,
    conversation_id: &str,
    enabled: bool,
    updated_at: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "UPDATE conversations SET tools_enabled = ?2, updated_at = ?3 WHERE id = ?1",
    )?
    .execute(rusqlite::params![
        conversation_id,
        enabled as i64,
        updated_at
    ])?;
    Ok(())
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
    /// Phase 35 (CTX-10) — tool provenance, persisted in
    /// `agent_steps.tool_origin` column added by MIGRATION_V20.
    /// `Some("local")` for built-in tools, `Some("contextvm")` for tools
    /// invoked via Nostr, `None` for non-tool_call rows.
    pub tool_origin: Option<String>,
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
        "INSERT INTO agent_steps (id, session_id, step_number, action_type, action_payload, result, status, created_at, tool_origin)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
        row.tool_origin,
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
        "SELECT id, session_id, step_number, action_type, action_payload, result, status, created_at, tool_origin
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
                tool_origin: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ── Message queries ───────────────────────────────────────────────────────────

/// Insert a new message row.
pub fn insert_message(conn: &Connection, row: &MessageRow) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count, image_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.conversation_id,
        row.role,
        row.content,
        row.created_at,
        row.token_count,
        row.image_path,
    ])?;
    Ok(())
}

/// Return all messages for a conversation ordered by `created_at` ascending.
pub fn list_messages(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<MessageRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, conversation_id, role, content, created_at, token_count, image_path
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
                image_path: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Return a single message row by ID.
///
/// Used by read_encrypted_image to look up the image_path for a given message_id
/// before decrypting and returning the JPEG bytes. Returns None if not found.
pub fn get_message_by_id(
    conn: &Connection,
    message_id: &str,
) -> Result<Option<MessageRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, conversation_id, role, content, created_at, token_count, image_path
         FROM messages WHERE id = ?1",
    )?;
    match stmt.query_row(rusqlite::params![message_id], |row| {
        Ok(MessageRow {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            token_count: row.get(5)?,
            image_path: row.get(6)?,
        })
    }) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(PersistenceError::from(e)),
    }
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

/// Fork a conversation: duplicate its row (new id, " (fork)" title suffix) and
/// every message row (new ids, preserved order + content + image_path) inside
/// a single SQLite transaction. Per quick/260423-93w.
///
/// - `created_at` / `updated_at` on the new conversation row are set to `now`
///   so the fork is marked as freshly created (sidebar sorting).
/// - Message `created_at` timestamps are preserved from the source so the
///   copied history renders with its original chronology.
/// - `image_path` values are copied verbatim as strings — both conversations
///   reference the same encrypted image file (MGO1 is immutable once written,
///   per quick/260419-ece).
///
/// Returns `PersistenceError` (from `rusqlite::Error::QueryReturnedNoRows`)
/// if `source_id` does not exist; transaction is rolled back on any error.
pub fn fork_conversation(
    conn: &mut rusqlite::Connection,
    source_id: &str,
    new_id: &str,
    now: i64,
) -> Result<(), PersistenceError> {
    let tx = conn.transaction()?;

    // 1. SELECT source conversation row.
    let source: ConversationRow = tx
        .prepare_cached(
            "SELECT id, title, model_id, backend_id, system_prompt, created_at, updated_at, tools_enabled
             FROM conversations WHERE id = ?1",
        )?
        .query_row(rusqlite::params![source_id], |row| {
            Ok(ConversationRow {
                id: row.get(0)?,
                title: row.get(1)?,
                model_id: row.get(2)?,
                backend_id: row.get(3)?,
                system_prompt: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                tools_enabled: row.get::<_, i64>(7)? != 0,
            })
        })?;

    // 2. INSERT new conversation row (copied metadata + " (fork)" title + fresh timestamps).
    tx.prepare_cached(
        "INSERT INTO conversations (id, title, model_id, backend_id, system_prompt, created_at, updated_at, tools_enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(rusqlite::params![
        new_id,
        format!("{} (fork)", source.title),
        source.model_id,
        source.backend_id,
        source.system_prompt,
        now,
        now,
        source.tools_enabled as i64,
    ])?;

    // 3. SELECT + copy all source messages in created_at ASC order (+ id ASC tiebreaker
    //    so repeated-timestamp messages preserve a deterministic order).
    let source_messages: Vec<MessageRow> = {
        let mut stmt = tx.prepare_cached(
            "SELECT id, conversation_id, role, content, created_at, token_count, image_path
             FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC, id ASC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![source_id], |row| {
                Ok(MessageRow {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                    token_count: row.get(5)?,
                    image_path: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    // 4. INSERT each copy with a fresh uuid and conversation_id = new_id.
    {
        let mut insert_stmt = tx.prepare_cached(
            "INSERT INTO messages (id, conversation_id, role, content, created_at, token_count, image_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for src_msg in &source_messages {
            let new_msg_id = uuid::Uuid::new_v4().to_string();
            insert_stmt.execute(rusqlite::params![
                new_msg_id,
                new_id,
                src_msg.role,
                src_msg.content,
                src_msg.created_at,
                src_msg.token_count,
                src_msg.image_path,
            ])?;
        }
    }

    tx.commit()?;
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

/// Update the content of a memory by ID.
///
/// Does NOT re-embed the vector -- the existing usearch entry becomes stale.
/// This is a deliberate v1 simplification; re-embedding is deferred.
pub fn update_memory(
    conn: &Connection,
    memory_id: &str,
    new_content: &str,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE memories SET content = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![memory_id, new_content])?;
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

// ── Directory source + directory file queries (Phase 32, MIGRATION_V18) ───────

/// A row from the `directory_sources` table.
///
/// Represents a user-chosen filesystem directory that should be kept in sync with the
/// local RAG index. Exactly one of `path` (Desktop), `bookmark_data` (iOS
/// security-scoped bookmark), or `tree_uri` (Android persistable tree URI) is expected
/// to be populated depending on the platform — the column names are nullable so the
/// same row shape works across all three.
#[derive(Debug, Clone)]
pub struct DirectorySourceRow {
    pub id: String,
    pub display_name: String,
    pub path: Option<String>,
    pub bookmark_data: Option<Vec<u8>>,
    pub tree_uri: Option<String>,
    /// JSON-encoded list of glob patterns to exclude from sync (e.g. `["*.tmp","/.git/**"]`).
    pub exclusion_globs_json: String,
    pub last_synced_at: Option<i64>,
    pub file_count: i64,
    pub created_at: i64,
}

/// A row from the `directory_files` table.
///
/// Stores a single file fingerprint inside a directory source. The fingerprint
/// (mtime_secs + size_bytes) is used by the sync pass to detect modified files
/// without rehashing contents.
#[derive(Debug, Clone)]
pub struct DirectoryFileRow {
    #[allow(dead_code)]
    pub id: i64,
    #[allow(dead_code)]
    pub source_id: String,
    pub file_path: String,
    pub mtime_secs: i64,
    pub size_bytes: i64,
    pub document_id: Option<String>,
}

/// Insert a new directory source row.
pub fn insert_directory_source(
    conn: &Connection,
    row: &DirectorySourceRow,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO directory_sources
             (id, display_name, path, bookmark_data, tree_uri, exclusion_globs,
              last_synced_at, file_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.display_name,
        row.path,
        row.bookmark_data,
        row.tree_uri,
        row.exclusion_globs_json,
        row.last_synced_at,
        row.file_count,
        row.created_at,
    ])?;
    Ok(())
}

/// Return all directory sources ordered by `created_at` ascending (insertion order).
pub fn list_directory_sources(
    conn: &Connection,
) -> Result<Vec<DirectorySourceRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, display_name, path, bookmark_data, tree_uri, exclusion_globs,
                last_synced_at, file_count, created_at
         FROM directory_sources ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(DirectorySourceRow {
                id: row.get(0)?,
                display_name: row.get(1)?,
                path: row.get(2)?,
                bookmark_data: row.get(3)?,
                tree_uri: row.get(4)?,
                exclusion_globs_json: row.get(5)?,
                last_synced_at: row.get(6)?,
                file_count: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Return a single directory source row by id, or None if not found.
pub fn get_directory_source(
    conn: &Connection,
    id: &str,
) -> Result<Option<DirectorySourceRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, display_name, path, bookmark_data, tree_uri, exclusion_globs,
                last_synced_at, file_count, created_at
         FROM directory_sources WHERE id = ?1",
    )?;
    match stmt.query_row(rusqlite::params![id], |row| {
        Ok(DirectorySourceRow {
            id: row.get(0)?,
            display_name: row.get(1)?,
            path: row.get(2)?,
            bookmark_data: row.get(3)?,
            tree_uri: row.get(4)?,
            exclusion_globs_json: row.get(5)?,
            last_synced_at: row.get(6)?,
            file_count: row.get(7)?,
            created_at: row.get(8)?,
        })
    }) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(PersistenceError::from(e)),
    }
}

/// Delete a directory source row by id. Associated directory_files rows are removed
/// automatically via ON DELETE CASCADE (MIGRATION_V18).
pub fn delete_directory_source(conn: &Connection, id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM directory_sources WHERE id = ?1")?
        .execute(rusqlite::params![id])?;
    Ok(())
}

/// Update `last_synced_at` and `file_count` for a directory source. Called at the
/// end of each successful sync pass.
pub fn update_directory_source_last_synced(
    conn: &Connection,
    id: &str,
    ts: i64,
    file_count: i64,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "UPDATE directory_sources SET last_synced_at = ?2, file_count = ?3 WHERE id = ?1",
    )?
    .execute(rusqlite::params![id, ts, file_count])?;
    Ok(())
}

/// Replace the exclusion glob list (JSON-encoded) for a directory source.
pub fn update_directory_source_exclusions(
    conn: &Connection,
    id: &str,
    globs_json: &str,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE directory_sources SET exclusion_globs = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![id, globs_json])?;
    Ok(())
}

/// Replace the iOS security-scoped bookmark blob for a directory source. Called when
/// the OS returns a refreshed bookmark via `isStale` detection.
pub fn update_directory_source_bookmark(
    conn: &Connection,
    id: &str,
    bookmark: &[u8],
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE directory_sources SET bookmark_data = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![id, bookmark])?;
    Ok(())
}

/// Insert-or-update a file fingerprint for a directory source.
///
/// Uses `ON CONFLICT (source_id, file_path) DO UPDATE` to overwrite `mtime_secs`,
/// `size_bytes`, and `document_id` when a fingerprint for the same path already
/// exists. This is the primary call used by the sync diff pass when it detects a
/// changed file.
pub fn upsert_directory_file(
    conn: &Connection,
    source_id: &str,
    file_path: &str,
    mtime_secs: i64,
    size_bytes: i64,
    document_id: Option<&str>,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO directory_files (source_id, file_path, mtime_secs, size_bytes, document_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(source_id, file_path) DO UPDATE SET
             mtime_secs  = excluded.mtime_secs,
             size_bytes  = excluded.size_bytes,
             document_id = excluded.document_id",
    )?
    .execute(rusqlite::params![
        source_id,
        file_path,
        mtime_secs,
        size_bytes,
        document_id,
    ])?;
    Ok(())
}

/// Return all file fingerprints for a source ordered by file_path ascending.
pub fn list_directory_files_by_source(
    conn: &Connection,
    source_id: &str,
) -> Result<Vec<DirectoryFileRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, source_id, file_path, mtime_secs, size_bytes, document_id
         FROM directory_files WHERE source_id = ?1 ORDER BY file_path ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![source_id], |row| {
            Ok(DirectoryFileRow {
                id: row.get(0)?,
                source_id: row.get(1)?,
                file_path: row.get(2)?,
                mtime_secs: row.get(3)?,
                size_bytes: row.get(4)?,
                document_id: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Delete a single file fingerprint (source_id, file_path) pair.
#[allow(dead_code)]
pub fn delete_directory_file(
    conn: &Connection,
    source_id: &str,
    file_path: &str,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM directory_files WHERE source_id = ?1 AND file_path = ?2")?
        .execute(rusqlite::params![source_id, file_path])?;
    Ok(())
}

/// Count the number of tracked files for a directory source.
pub fn count_directory_files(conn: &Connection, source_id: &str) -> Result<i64, PersistenceError> {
    let count: i64 = conn
        .prepare_cached("SELECT COUNT(*) FROM directory_files WHERE source_id = ?1")?
        .query_row(rusqlite::params![source_id], |row| row.get(0))?;
    Ok(count)
}

// ── Phase 35 — contextvm_tools queries (CTX-03 / CTX-04) ──────────────────────

/// Phase 35 — one row in `contextvm_tools`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextvmToolRow {
    /// Composite key: "<provider_pubkey>:<tool_name>".
    pub id: String,
    pub tool_name: String,
    pub display_name: Option<String>,
    pub description: String,
    pub provider_pubkey: String,
    pub provider_display_name: Option<String>,
    pub schema_json: String,
    pub enabled: bool,
    pub last_seen_at: i64,
}

/// Insert OR REPLACE — used by discovery to refresh announcements without
/// flipping a manually-toggled enabled flag back to 0. Callers MUST pass
/// the existing `enabled` value (read it via `get_contextvm_tool_by_name`
/// first); this helper does NOT preserve enabled across upserts on its
/// own — that responsibility lives in the actor handler.
pub fn upsert_contextvm_tool(
    conn: &Connection,
    row: &ContextvmToolRow,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT OR REPLACE INTO contextvm_tools \
         (id, tool_name, display_name, description, provider_pubkey, \
          provider_display_name, schema_json, enabled, last_seen_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.tool_name,
        row.display_name,
        row.description,
        row.provider_pubkey,
        row.provider_display_name,
        row.schema_json,
        row.enabled as i64,
        row.last_seen_at,
    ])?;
    Ok(())
}

/// Flip the `enabled` flag for a tool row by composite id.
pub fn update_contextvm_tool_enabled(
    conn: &Connection,
    id: &str,
    enabled: bool,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE contextvm_tools SET enabled = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![id, enabled as i64])?;
    Ok(())
}

/// Fetch a single tool row by `tool_name` (unique). Returns `None` if absent.
pub fn get_contextvm_tool_by_name(
    conn: &Connection,
    tool_name: &str,
) -> Result<Option<ContextvmToolRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, tool_name, display_name, description, provider_pubkey, \
                provider_display_name, schema_json, enabled, last_seen_at \
         FROM contextvm_tools WHERE tool_name = ?1 LIMIT 1",
    )?;
    let mut rows = stmt.query(rusqlite::params![tool_name])?;
    if let Some(r) = rows.next()? {
        Ok(Some(ContextvmToolRow {
            id: r.get(0)?,
            tool_name: r.get(1)?,
            display_name: r.get(2)?,
            description: r.get(3)?,
            provider_pubkey: r.get(4)?,
            provider_display_name: r.get(5)?,
            schema_json: r.get(6)?,
            enabled: r.get::<_, i64>(7)? != 0,
            last_seen_at: r.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

/// All currently-enabled tools, newest first by `last_seen_at`.
pub fn list_enabled_contextvm_tools(
    conn: &Connection,
) -> Result<Vec<ContextvmToolRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, tool_name, display_name, description, provider_pubkey, \
                provider_display_name, schema_json, enabled, last_seen_at \
         FROM contextvm_tools WHERE enabled = 1 ORDER BY last_seen_at DESC",
    )?;
    let rows: Result<Vec<_>, _> = stmt
        .query_map([], |r| {
            Ok(ContextvmToolRow {
                id: r.get(0)?,
                tool_name: r.get(1)?,
                display_name: r.get(2)?,
                description: r.get(3)?,
                provider_pubkey: r.get(4)?,
                provider_display_name: r.get(5)?,
                schema_json: r.get(6)?,
                enabled: r.get::<_, i64>(7)? != 0,
                last_seen_at: r.get(8)?,
            })
        })?
        .collect();
    Ok(rows?)
}

/// All known announcements (enabled or not). Used by the Tool Discovery
/// screen to merge fresh announcements with persisted enabled state.
pub fn list_all_contextvm_tools(
    conn: &Connection,
) -> Result<Vec<ContextvmToolRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, tool_name, display_name, description, provider_pubkey, \
                provider_display_name, schema_json, enabled, last_seen_at \
         FROM contextvm_tools ORDER BY last_seen_at DESC",
    )?;
    let rows: Result<Vec<_>, _> = stmt
        .query_map([], |r| {
            Ok(ContextvmToolRow {
                id: r.get(0)?,
                tool_name: r.get(1)?,
                display_name: r.get(2)?,
                description: r.get(3)?,
                provider_pubkey: r.get(4)?,
                provider_display_name: r.get(5)?,
                schema_json: r.get(6)?,
                enabled: r.get::<_, i64>(7)? != 0,
                last_seen_at: r.get(8)?,
            })
        })?
        .collect();
    Ok(rows?)
}

/// Bulk delete — used when the user toggles a tool off and we want to
/// keep the table compact. Optional in v1; included for cleanup paths.
#[allow(dead_code)]
pub fn delete_contextvm_tool(conn: &Connection, id: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("DELETE FROM contextvm_tools WHERE id = ?1")?
        .execute(rusqlite::params![id])?;
    Ok(())
}

/// Phase 36 (CTX36-USED-01) — read all contextvm tool-call agent_steps rows so
/// the caller can aggregate by tool_name. Returns `(action_payload_json, created_at)`
/// per row. The action_payload is a JSON array `[{"id":..,"name":..,"arguments":..}, ...]`
/// and may contain multiple tool invocations per row, hence the parse-and-aggregate
/// step is performed in Rust (see `aggregate_contextvm_tool_usage` in lib.rs).
///
/// Note: agent_steps has no `tool_name` column — the literal CONTEXT D-Area-4 query
/// `SELECT tool_name, COUNT(*), MAX(timestamp) FROM agent_steps GROUP BY tool_name`
/// is not implementable as written; this pull-and-parse approach is the
/// semantically-equivalent v1 implementation (RESEARCH §Common Pitfalls Pitfall 1).
pub fn fetch_contextvm_tool_usage_rows(
    conn: &Connection,
) -> Result<Vec<(String, i64)>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT action_payload, created_at \
         FROM agent_steps \
         WHERE tool_origin = 'contextvm' AND action_type = 'tool_call'",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod directory_tests {
    use super::*;
    use crate::persistence::Database;

    fn sample_source(id: &str, display: &str) -> DirectorySourceRow {
        DirectorySourceRow {
            id: id.to_string(),
            display_name: display.to_string(),
            path: Some(format!("/tmp/{id}")),
            bookmark_data: None,
            tree_uri: None,
            exclusion_globs_json: "[]".to_string(),
            last_synced_at: None,
            file_count: 0,
            created_at: 100,
        }
    }

    #[test]
    fn test_directory_source_queries() {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();

        // Desktop variant
        let desktop = sample_source("s1", "Desktop Vault");
        insert_directory_source(conn, &desktop).unwrap();

        // iOS bookmark variant
        let ios = DirectorySourceRow {
            id: "s2".to_string(),
            display_name: "iOS Vault".to_string(),
            path: None,
            bookmark_data: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            tree_uri: None,
            exclusion_globs_json: "[]".to_string(),
            last_synced_at: None,
            file_count: 0,
            created_at: 200,
        };
        insert_directory_source(conn, &ios).unwrap();

        // Android tree_uri variant
        let android = DirectorySourceRow {
            id: "s3".to_string(),
            display_name: "Android Vault".to_string(),
            path: None,
            bookmark_data: None,
            tree_uri: Some("content://com.android/tree/abc".to_string()),
            exclusion_globs_json: "[]".to_string(),
            last_synced_at: None,
            file_count: 0,
            created_at: 300,
        };
        insert_directory_source(conn, &android).unwrap();

        let listed = list_directory_sources(conn).unwrap();
        assert_eq!(listed.len(), 3, "should list all 3 sources");
        assert_eq!(listed[0].id, "s1");
        assert_eq!(listed[1].id, "s2");
        assert_eq!(listed[2].id, "s3");
        assert_eq!(listed[1].bookmark_data, Some(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        assert_eq!(
            listed[2].tree_uri.as_deref(),
            Some("content://com.android/tree/abc")
        );

        // get_directory_source
        let fetched = get_directory_source(conn, "s2").unwrap().unwrap();
        assert_eq!(fetched.display_name, "iOS Vault");
        assert!(get_directory_source(conn, "missing").unwrap().is_none());

        // update_last_synced
        update_directory_source_last_synced(conn, "s1", 500, 42).unwrap();
        let fetched = get_directory_source(conn, "s1").unwrap().unwrap();
        assert_eq!(fetched.last_synced_at, Some(500));
        assert_eq!(fetched.file_count, 42);

        // delete
        delete_directory_source(conn, "s2").unwrap();
        let listed = list_directory_sources(conn).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|r| r.id != "s2"));
    }

    #[test]
    fn test_directory_file_fingerprints() {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();
        insert_directory_source(conn, &sample_source("src1", "Vault")).unwrap();

        // First insert
        upsert_directory_file(conn, "src1", "notes/a.md", 100, 500, None).unwrap();
        let files = list_directory_files_by_source(conn, "src1").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].mtime_secs, 100);
        assert_eq!(files[0].size_bytes, 500);

        // Upsert same path — should update in place, not duplicate
        upsert_directory_file(conn, "src1", "notes/a.md", 200, 800, Some("doc-abc")).unwrap();
        let files = list_directory_files_by_source(conn, "src1").unwrap();
        assert_eq!(files.len(), 1, "upsert must update, not duplicate");
        assert_eq!(files[0].mtime_secs, 200);
        assert_eq!(files[0].size_bytes, 800);
        assert_eq!(files[0].document_id.as_deref(), Some("doc-abc"));

        // delete_directory_file
        upsert_directory_file(conn, "src1", "notes/b.md", 300, 900, None).unwrap();
        delete_directory_file(conn, "src1", "notes/a.md").unwrap();
        let files = list_directory_files_by_source(conn, "src1").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_path, "notes/b.md");
    }

    #[test]
    fn test_count_directory_files() {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();
        insert_directory_source(conn, &sample_source("src1", "Vault")).unwrap();
        insert_directory_source(conn, &sample_source("src2", "Other")).unwrap();

        assert_eq!(count_directory_files(conn, "src1").unwrap(), 0);

        upsert_directory_file(conn, "src1", "a.md", 1, 10, None).unwrap();
        upsert_directory_file(conn, "src1", "b.md", 2, 20, None).unwrap();
        upsert_directory_file(conn, "src2", "x.md", 3, 30, None).unwrap();

        assert_eq!(count_directory_files(conn, "src1").unwrap(), 2);
        assert_eq!(count_directory_files(conn, "src2").unwrap(), 1);
    }

    #[test]
    fn test_update_exclusions_and_bookmark() {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();
        insert_directory_source(conn, &sample_source("src1", "Vault")).unwrap();

        let new_globs = r#"["*.tmp","/.git/**"]"#;
        update_directory_source_exclusions(conn, "src1", new_globs).unwrap();
        let fetched = get_directory_source(conn, "src1").unwrap().unwrap();
        assert_eq!(fetched.exclusion_globs_json, new_globs);

        let new_bookmark = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        update_directory_source_bookmark(conn, "src1", &new_bookmark).unwrap();
        let fetched = get_directory_source(conn, "src1").unwrap().unwrap();
        assert_eq!(fetched.bookmark_data, Some(new_bookmark));
    }
}
