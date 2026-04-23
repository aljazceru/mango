pub mod error;
pub mod queries;
pub mod schema;

pub use error::PersistenceError;
#[allow(unused_imports)]
pub use queries::{
    count_directory_files, delete_backend, delete_backend_health, delete_chunks_for_document,
    delete_conversation, delete_directory_file, delete_directory_source, delete_document,
    delete_message, delete_messages_after, fork_conversation, get_active_backend_id, get_chunk_text_by_rowids,
    get_conversation_attached_docs, get_directory_source, get_setting, insert_agent_session,
    insert_agent_step, insert_backend, insert_chunk, insert_conversation, insert_directory_source,
    insert_document, insert_message, list_agent_sessions, list_agent_steps, list_backend_health,
    list_backends, list_chunks_for_document, list_conversations, list_directory_files_by_source,
    list_directory_sources, list_documents, list_messages, rename_conversation, set_setting,
    update_backend_display_order, update_backend_models, update_conversation_attached_docs,
    update_conversation_backend, update_conversation_model, update_conversation_system_prompt,
    update_conversation_updated_at, update_directory_source_bookmark,
    update_directory_source_exclusions, update_directory_source_last_synced,
    update_document_chunk_count, upsert_backend_health, upsert_directory_file, AgentSessionRow,
    AgentStepRow, BackendHealthRow, BackendRow, ChunkRow, ConversationRow, DirectoryFileRow,
    DirectorySourceRow, DocumentRow, MessageRow,
};

/// Validate that `dek_hex` is exactly 64 lowercase hex characters (32 bytes / 256 bits).
///
/// This check must be performed before embedding the DEK hex in any SQL string or pragma
/// value, to detect programming errors early and prevent malformed SQL (WR-02).
fn validate_dek_hex(dek_hex: &str) -> Result<(), PersistenceError> {
    if dek_hex.len() != 64
        || !dek_hex
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(PersistenceError::DecryptionFailed {
            message: format!(
                "invalid DEK hex: expected 64 lowercase hex chars, got {} chars",
                dek_hex.len()
            ),
        });
    }
    Ok(())
}

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

    /// Return a mutable reference to the underlying `rusqlite::Connection`.
    ///
    /// Needed by helpers that span multiple statements inside a
    /// `rusqlite::Transaction` (e.g. `fork_conversation`). Keep usage narrow;
    /// the actor thread is the only caller.
    pub fn conn_mut(&mut self) -> &mut rusqlite::Connection {
        &mut self.conn
    }

    /// Open a SQLCipher-encrypted database at `path` using a 64-char hex DEK.
    ///
    /// The key pragma is issued as the very first operation after open (per SQLCipher
    /// requirement D-01). WAL mode and foreign keys are enabled after keying.
    /// Runs all pending schema migrations.
    ///
    /// Returns `DecryptionFailed` if the key is wrong or the database is corrupted.
    pub fn open_encrypted(path: &str, dek_hex: &str) -> Result<Self, PersistenceError> {
        // Validate dek_hex before embedding it in the key pragma string.
        // A 256-bit key must be exactly 64 lowercase hex characters (WR-02).
        validate_dek_hex(dek_hex)?;
        let conn = rusqlite::Connection::open(path)?;
        // CRITICAL: key pragma MUST be first operation after open (per D-01)
        conn.pragma_update(None, "key", format!("x'{}'", dek_hex))?;
        // Verify the key is correct by attempting a read. SQLCipher returns an error
        // on the first real DB operation if the key is wrong.
        conn.pragma_query_value::<i32, _>(None, "user_version", |r| r.get(0))
            .map_err(|e| PersistenceError::DecryptionFailed {
                message: format!("wrong key or corrupted database: {}", e),
            })?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Return `true` if the database file at `path` is SQLCipher-encrypted.
    ///
    /// Attempts to read `user_version` without a key. If that fails the DB is
    /// encrypted; if it succeeds it is a plaintext SQLite file.
    pub fn is_encrypted(path: &str) -> bool {
        let Ok(conn) = rusqlite::Connection::open(path) else {
            return false;
        };
        // If we can read user_version without a key, DB is plaintext.
        conn.pragma_query_value::<i32, _>(None, "user_version", |r| r.get(0))
            .is_err()
    }

    /// Migrate a plaintext SQLite database to SQLCipher in-place.
    ///
    /// Uses `sqlcipher_export` to copy the plaintext DB into a new encrypted file,
    /// verifies the new file opens correctly, then replaces the original.
    /// If any step fails the original plaintext file is left untouched.
    ///
    /// Note: `sqlcipher_export` does not copy `PRAGMA user_version`. The source
    /// version is read before export and explicitly written to the encrypted copy
    /// so that `open_encrypted` does not re-apply already-applied migrations.
    pub fn migrate_to_encrypted(path: &str, dek_hex: &str) -> Result<(), PersistenceError> {
        // Validate dek_hex before embedding it in the ATTACH KEY string (WR-02).
        validate_dek_hex(dek_hex)?;
        let enc_path = format!("{}_enc_tmp", path);
        // WR-01: Reject paths containing single-quote characters to prevent SQL injection
        // in the ATTACH DATABASE statement. Single quotes are valid POSIX filename chars
        // but would break or malform the SQL string.
        if enc_path.contains('\'') {
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: format!(
                    "database path contains invalid character (single quote): {:?}",
                    enc_path
                ),
            });
        }
        // Open plaintext DB and read its current user_version
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let src_version: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap_or(0);
        // Export schema + data to encrypted copy
        conn.execute_batch(&format!(
            "ATTACH DATABASE '{}' AS encrypted KEY \"x'{}'\";\
             SELECT sqlcipher_export('encrypted');\
             DETACH DATABASE encrypted;",
            enc_path, dek_hex
        ))?;
        drop(conn);
        // sqlcipher_export does not transfer user_version — set it explicitly so
        // run_migrations skips already-applied migrations.
        {
            let enc_conn = rusqlite::Connection::open(&enc_path)?;
            enc_conn.pragma_update(None, "key", format!("x'{}'", dek_hex))?;
            enc_conn.pragma_update(None, "user_version", src_version)?;
        }
        // Verify the encrypted copy opens with correct key (and no extra migrations)
        let verify = Self::open_encrypted(&enc_path, dek_hex);
        if let Err(e) = verify {
            // Clean up temp file on failure
            let _ = std::fs::remove_file(&enc_path);
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: format!("encrypted DB verification failed: {}", e),
            });
        }
        drop(verify);
        // Atomically replace original with encrypted copy
        std::fs::rename(&enc_path, path).map_err(|e| {
            // Best-effort cleanup: remove temp file since rename failed and the
            // original plaintext DB is still in place.
            let _ = std::fs::remove_file(&enc_path);
            PersistenceError::MigrationFailed {
                version: 0,
                message: format!("rename after encryption: {}", e),
            }
        })?;
        Ok(())
    }
}
