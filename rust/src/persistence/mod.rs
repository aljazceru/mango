pub mod error;
pub mod queries;
pub mod schema;

pub use error::PersistenceError;
#[allow(unused_imports)]
pub use queries::{
    archive_conversations_older_than, count_directory_files, delete_backend, delete_backend_health,
    delete_chunks_for_document, delete_conversation, delete_conversations_older_than,
    delete_directory_file, delete_directory_source, delete_document,
    delete_hybrid_profile, delete_message, delete_messages_after, fork_conversation,
    get_active_backend_id, get_chunk_text_by_rowids, get_conversation_attached_docs,
    get_directory_source, get_setting, insert_agent_session, insert_agent_step, insert_backend,
    insert_chunk, insert_conversation, insert_directory_source, insert_document, insert_message,
    list_active_conversations, list_agent_sessions, list_agent_steps, list_backend_health,
    list_backends, list_archived_conversations, list_chunks_for_document, list_conversations,
    list_directory_files_by_source, list_directory_sources, list_documents, list_hybrid_profiles,
    list_messages,
    rename_conversation, set_conversation_archived, set_setting, update_backend_display_order,
    update_backend_models,
    update_conversation_attached_docs, update_conversation_backend, update_conversation_model,
    update_conversation_system_prompt, update_conversation_updated_at,
    update_directory_source_bookmark, update_directory_source_exclusions,
    update_directory_source_last_synced, update_document_chunk_count, upsert_backend_health,
    upsert_directory_file, upsert_hybrid_profile, upsert_local_backend, AgentSessionRow,
    AgentStepRow, ArchivedConversationRow, BackendHealthRow, BackendRow, ChunkRow,
    ConversationRow, DirectoryFileRow,
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

/// Outcome of `Database::recover_plaintext_after_failed_enrollment`.
///
/// The caller uses this to decide whether to resume enrollment, clear pending auth,
/// or block and retry. No function in this module creates a fresh main DB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentRecovery {
    /// The plaintext main DB is in place and has been verified to open.
    PlaintextReady,
    /// No main, backup, or encrypted candidate files were found. The caller must
    /// decide whether this is a fresh install or data loss. Pending auth is only
    /// written after a real DB exists, so when pending auth is present this means
    /// the database files are gone; the caller must block rather than create a
    /// blank replacement.
    NothingToRecover,
    /// The main DB is encrypted and a pending auth row exists; enrollment can resume.
    EncryptedReady,
    /// The main DB is missing or plaintext, but an encrypted candidate file
    /// (`.enc_tmp` or `.enc_replaced`) survives from an interrupted migration or
    /// rollback. It may be the only copy of the user's data. The caller must
    /// resolve it by verifying the pending-auth DEK (derived from the entered PIN)
    /// via `promote_encrypted_candidate`, or preserve it and block — never delete it.
    EncryptedCandidate,
}

impl Database {
    /// Open the database at `path` and run any pending migrations.
    ///
    /// Pass `":memory:"` for tests; pass an on-disk file path for production.
    pub fn open(path: &str) -> Result<Self, PersistenceError> {
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // NORMAL + WAL is the standard pairing: commits no longer fsync the WAL
        // on every write (big latency win on mobile flash); the durability
        // tradeoff is limited to the last commits on power loss.
        conn.pragma_update(None, "synchronous", "NORMAL")?;
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
        conn.pragma_update(None, "synchronous", "NORMAL")?;
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
        if path == ":memory:" || !std::path::Path::new(path).exists() {
            return false;
        }
        let Ok(conn) = rusqlite::Connection::open(path) else {
            return false;
        };
        // If we can read user_version without a key, DB is plaintext.
        conn.pragma_query_value::<i32, _>(None, "user_version", |r| r.get(0))
            .is_err()
    }

    /// Suffix used for the verified encrypted export of `mango.db`.
    pub const ENCRYPTED_TEMP_SUFFIX: &str = ".enc_tmp";
    /// Suffix used for the plaintext backup kept until auth is committed.
    pub const PLAINTEXT_BACKUP_SUFFIX: &str = ".plain_bak";
    /// Suffix used for the encrypted main file when it is rolled back to a plaintext backup.
    pub const ENCRYPTED_REPLACED_SUFFIX: &str = ".enc_replaced";

    /// Path to the verified encrypted temp copy of `path`.
    pub fn encrypted_temp_path(path: &str) -> String {
        format!("{}{}", path, Self::ENCRYPTED_TEMP_SUFFIX)
    }

    /// Path to the plaintext backup copy of `path`.
    pub fn plaintext_backup_path(path: &str) -> String {
        format!("{}{}", path, Self::PLAINTEXT_BACKUP_SUFFIX)
    }

    /// Path to the encrypted main file when it is parked during rollback.
    pub fn replaced_encrypted_path(path: &str) -> String {
        format!("{}{}", path, Self::ENCRYPTED_REPLACED_SUFFIX)
    }

    /// Checkpoint the WAL into the main file and remove the WAL sidecars.
    ///
    /// Call before any file operation (rename, copy, finalize) that must treat the
    /// database file as self-contained. Returns an error if SQLite reports a busy
    /// checkpoint, which means the caller should retry or block instead of moving
    /// files around.
    pub fn checkpoint_truncate(&self) -> Result<(), PersistenceError> {
        let busy: i32 = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))
            .unwrap_or(1);
        if busy != 0 {
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: "WAL checkpoint busy; database not self-contained".into(),
            });
        }
        Ok(())
    }

    /// Migrate a plaintext SQLite database to SQLCipher in-place.
    ///
    /// Uses `sqlcipher_export` to copy the plaintext DB into a new encrypted file,
    /// verifies the new file opens correctly, then replaces the original. The
    /// original is kept as a `path.plain_bak` until the caller commits the active
    /// auth params; that backup is the only recoverable copy if promotion fails.
    ///
    /// The protocol is:
    ///   1. Checkpoint the plaintext WAL into the main file.
    ///   2. Export to `path.enc_tmp` and set `user_version`.
    ///   3. Verify `path.enc_tmp` opens with the DEK and checkpoint it.
    ///   4. Move `path` to `path.plain_bak`.
    ///   5. Move `path.enc_tmp` to `path`.
    ///
    /// Note: `sqlcipher_export` does not copy `PRAGMA user_version`. The source
    /// version is read before export and explicitly written to the encrypted copy
    /// so that `open_encrypted` does not re-apply already-applied migrations.
    pub fn migrate_to_encrypted(path: &str, dek_hex: &str) -> Result<(), PersistenceError> {
        if path == ":memory:" {
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: "cannot migrate an in-memory database".into(),
            });
        }
        if !std::path::Path::new(path).exists() {
            // Do not create a blank database in the name of migration.
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: "source database does not exist; cannot migrate".into(),
            });
        }
        // Validate dek_hex before embedding it in the ATTACH KEY string (WR-02).
        validate_dek_hex(dek_hex)?;
        // WR-01: Reject paths containing single-quote characters to prevent SQL injection
        // in the ATTACH DATABASE statement. Single quotes are valid POSIX filename chars
        // but would break or malform the SQL string.
        if path.contains('\'') {
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: format!(
                    "database path contains invalid character (single quote): {:?}",
                    path
                ),
            });
        }

        let enc_path = Self::encrypted_temp_path(path);
        let bak_path = Self::plaintext_backup_path(path);

        // Remove an abandoned encrypted temp from a previous attempt. Do NOT remove
        // `bak_path` here — if a previous run got as far as renaming the original
        // aside but crashed before auth promotion, that backup is the only plaintext
        // copy and must survive for recovery.
        remove_sqlite_files(&enc_path)?;

        // 1. Open the plaintext source, make it self-contained, and read its user_version.
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Checkpoint WAL into the main file before we detach it; if SQLite is still
        // busy, fail rather than risk incomplete data in the backup/encrypted copy.
        let busy: i32 = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))
            .unwrap_or(1);
        if busy != 0 {
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: "plain text DB checkpoint failed; migration aborted".into(),
            });
        }
        let src_version: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap_or(0);

        // 2. Export schema + data to encrypted copy.
        conn.execute_batch(&format!(
            "ATTACH DATABASE '{}' AS encrypted KEY \"x'{}'\";\
             SELECT sqlcipher_export('encrypted');\
             DETACH DATABASE encrypted;",
            enc_path, dek_hex
        ))?;
        drop(conn);

        // 3. sqlcipher_export does not transfer user_version — set it explicitly so
        //    run_migrations skips already-applied migrations.
        {
            let enc_conn = rusqlite::Connection::open(&enc_path)?;
            enc_conn.pragma_update(None, "key", format!("x'{}'", dek_hex))?;
            enc_conn.pragma_update(None, "user_version", src_version)?;
        }

        // 4. Verify the encrypted copy opens with the correct key and is self-contained.
        let verify = Self::open_encrypted(&enc_path, dek_hex);
        match verify {
            Ok(verify_db) => {
                if let Err(e) = verify_db.checkpoint_truncate() {
                    drop(verify_db);
                    remove_sqlite_files(&enc_path)?;
                    return Err(PersistenceError::MigrationFailed {
                        version: 0,
                        message: format!("encrypted DB checkpoint failed: {}", e),
                    });
                }
                drop(verify_db);
            }
            Err(e) => {
                // Abandon the temp file; original is untouched and retryable.
                remove_sqlite_files(&enc_path)?;
                return Err(PersistenceError::MigrationFailed {
                    version: 0,
                    message: format!("encrypted DB verification failed: {}", e),
                });
            }
        }
        remove_sqlite_sidecars(&enc_path)?;

        // 5. Replace original. The plaintext sidecars are empty after checkpoint
        //    but could still conflict with the encrypted DB landing at `path`,
        //    so remove them before the rename. Do NOT remove `path` itself — it
        //    is the source of the rename-aside to the plaintext backup. Any stale
        //    backup at `bak_path` is from a previous interrupted attempt and is safe
        //    to remove now that we are about to write a fresh one.
        remove_sqlite_files(&bak_path)?;
        remove_sqlite_sidecars(path)?;

        let main_exists = std::path::Path::new(path).exists();
        if main_exists {
            std::fs::rename(path, &bak_path).map_err(|e| {
                // The rename aside failed; the original is still at `path` and
                // `enc_path` is valid and self-contained. Remove `enc_path` so the
                // next attempt starts clean, but leave the original untouched.
                let _ = remove_sqlite_files(&enc_path).ok();
                PersistenceError::MigrationFailed {
                    version: 0,
                    message: format!("rename plaintext aside failed: {}", e),
                }
            })?;
        }

        if let Err(e) = std::fs::rename(&enc_path, path) {
            // The encrypted copy did not land at `path`. Best-effort restore the
            // plaintext backup if we have it; otherwise the encrypted temp is still
            // usable at `enc_path` (caller can recover via pending auth on restart).
            if main_exists {
                let _ = std::fs::rename(&bak_path, path);
            }
            let _ = remove_sqlite_files(&enc_path).ok();
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: format!("rename encrypted DB into place failed: {}", e),
            });
        }

        Ok(())
    }

    /// Finalize an in-place migration by removing the now-obsolete plaintext backup
    /// and any leftover enrollment artifacts.
    ///
    /// Call *after* the active auth params have been committed and the encrypted
    /// DB has been verified to open. The function is now fallible so the caller can
    /// surface a retryable error and the cleanup will be attempted again on the next
    /// verified open.
    pub fn finalize_encrypted_storage(path: &str) -> Result<(), PersistenceError> {
        if path == ":memory:" {
            return Ok(());
        }
        remove_sqlite_files(&Self::encrypted_temp_path(path))?;
        remove_sqlite_files(&Self::plaintext_backup_path(path))?;
        remove_sqlite_files(&Self::replaced_encrypted_path(path))?;
        Ok(())
    }

    /// Roll an interrupted migration back to the plaintext backup.
    ///
    /// The encrypted `path` is moved to `path.enc_replaced` and the plaintext backup
    /// at `path.plain_bak` is moved to `path`. The operation is verified: if the
    /// plaintext backup does not exist, cannot be moved, or cannot be opened, the
    /// original encrypted file is left untouched.
    ///
    /// The caller is responsible for ensuring the encrypted main is self-contained
    /// (no WAL sidecars) before calling this function, because the rename only moves
    /// the main file.
    pub fn rollback_encrypted_to_plaintext(path: &str) -> Result<(), PersistenceError> {
        if path == ":memory:" {
            return Ok(());
        }
        let bak_path = Self::plaintext_backup_path(path);
        if !std::path::Path::new(&bak_path).exists() {
            // Without a backup we cannot roll back. Do not destroy the encrypted main.
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: "no plaintext backup available for rollback".into(),
            });
        }

        // Verify the backup opens before moving any files. A corrupt backup must
        // not cause the encrypted main to be parked or the restore attempted.
        Self::open(&bak_path)?;
        let _ = remove_sqlite_sidecars(&bak_path).ok();

        // No encrypted main — just restore the backup.
        if !std::path::Path::new(path).exists() {
            std::fs::rename(&bak_path, path).map_err(|e| PersistenceError::MigrationFailed {
                version: 0,
                message: format!("restore plaintext backup failed: {}", e),
            })?;
            let _ = Self::open(path)?;
            return Ok(());
        }

        let replaced_path = Self::replaced_encrypted_path(path);
        // Remove any stale fallback from a previous interrupted rollback so the
        // rename does not fail.
        let _ = remove_sqlite_files(&replaced_path).ok();

        // Move the encrypted main aside. Its sidecars, if any, were the caller's
        // responsibility to checkpoint/remove before calling this function.
        std::fs::rename(path, &replaced_path).map_err(|e| PersistenceError::MigrationFailed {
            version: 0,
            message: format!("park encrypted main failed: {}", e),
        })?;

        // Move the plaintext backup into place.
        if let Err(e) = std::fs::rename(&bak_path, path) {
            // Try to restore the encrypted main.
            let _ = std::fs::rename(&replaced_path, path);
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: format!("rollback rename backup into place failed: {}", e),
            });
        }

        // Verify the restored plaintext opens.
        if let Err(e) = Self::open(path) {
            // Undo: move the backup back and restore the encrypted main.
            let _ = std::fs::remove_file(path);
            let _ = std::fs::rename(&replaced_path, path);
            return Err(PersistenceError::MigrationFailed {
                version: 0,
                message: format!("rollback verify plaintext failed: {}", e),
            });
        }

        // Plaintext is in place and verified. The encrypted fallback is no longer
        // needed, but its removal is best-effort so a permission problem here does
        // not roll back a successful restore. Finalize will try again later.
        let _ = remove_sqlite_files(&replaced_path).ok();
        Ok(())
    }

    /// Recover the pre-enrollment plaintext state after an interrupted migration.
    ///
    /// This function never creates a fresh main DB and never deletes an encrypted
    /// file that could be the only surviving copy of the user's data. It inspects
    /// the filesystem and returns an `EnrollmentRecovery` outcome:
    ///
    ///   - `PlaintextReady`: the main file is plaintext (or was restored from a
    ///     verified backup) and has been verified to open.
    ///   - `EncryptedReady`: the main file is encrypted; the caller should resume
    ///     enrollment with the pending auth.
    ///   - `EncryptedCandidate`: the main file is missing, but `.enc_tmp` or
    ///     `.enc_replaced` survives. These can only be verified with the
    ///     pending-auth DEK; the caller must resolve them via
    ///     `promote_encrypted_candidate` or preserve them and block.
    ///   - `NothingToRecover`: no main, backup, or candidate files were found.
    ///
    /// On any error the function returns `Err` and the caller must not clear
    /// pending auth or create a new database.
    pub fn recover_plaintext_after_failed_enrollment(
        path: &str,
    ) -> Result<EnrollmentRecovery, PersistenceError> {
        if path == ":memory:" {
            return Ok(EnrollmentRecovery::NothingToRecover);
        }
        let enc_path = Self::encrypted_temp_path(path);
        let replaced_path = Self::replaced_encrypted_path(path);
        let bak_path = Self::plaintext_backup_path(path);
        let main_exists = std::path::Path::new(path).exists();
        let bak_exists = std::path::Path::new(&bak_path).exists();
        let enc_exists = std::path::Path::new(&enc_path).exists();
        let replaced_exists = std::path::Path::new(&replaced_path).exists();

        // Case 1: main file is encrypted. A pending auth row is required for this
        // state to make sense; the caller should resume enrollment.
        if main_exists && Self::is_encrypted(path) {
            return Ok(EnrollmentRecovery::EncryptedReady);
        }

        // Case 2: main is plaintext (the encrypted case returned above). Verify it
        // opens, then it is authoritative: any stale backup or encrypted candidates
        // are redundant copies and can be removed safely.
        if main_exists {
            Self::open(path)?;
            if bak_exists {
                remove_sqlite_files(&bak_path)?;
            }
            if enc_exists {
                remove_sqlite_files(&enc_path)?;
            }
            if replaced_exists {
                remove_sqlite_files(&replaced_path)?;
            }
            return Ok(EnrollmentRecovery::PlaintextReady);
        }

        // Case 3: no main, but a plaintext backup exists. Verify the backup,
        // restore it, and verify the restored main.
        if bak_exists {
            Self::open(&bak_path)?;
            let _ = remove_sqlite_sidecars(&bak_path).ok();
            std::fs::rename(&bak_path, path).map_err(|e| PersistenceError::MigrationFailed {
                version: 0,
                message: format!("restore plaintext backup failed: {}", e),
            })?;
            Self::open(path)?;
            return Ok(EnrollmentRecovery::PlaintextReady);
        }

        // Case 4: no main and no backup, but an encrypted candidate survives. It
        // may be the only copy of the user's data and can only be verified with
        // the pending DEK — never delete it here.
        if enc_exists || replaced_exists {
            return Ok(EnrollmentRecovery::EncryptedCandidate);
        }

        // Case 5: nothing on disk at all.
        Ok(EnrollmentRecovery::NothingToRecover)
    }

    /// Promote a surviving encrypted candidate (`.enc_tmp` or `.enc_replaced`) to
    /// the canonical main path after verifying it opens with `dek_hex` (derived
    /// from the pending-auth PIN).
    ///
    /// Returns `true` if a candidate was verified and promoted into place. A
    /// candidate that fails verification is preserved untouched — it may be the
    /// only surviving copy of the user's data — and the next candidate is tried.
    /// Returns `false` when the main file already exists or no candidate verifies.
    pub fn promote_encrypted_candidate(
        path: &str,
        dek_hex: &str,
    ) -> Result<bool, PersistenceError> {
        if path == ":memory:" || std::path::Path::new(path).exists() {
            return Ok(false);
        }
        // Prefer `enc_replaced` (a parked live main, which may hold post-migration
        // writes) over `enc_tmp` (the pre-rename verified export).
        for candidate in [
            Self::replaced_encrypted_path(path),
            Self::encrypted_temp_path(path),
        ] {
            if !std::path::Path::new(&candidate).exists() {
                continue;
            }
            let verified = match Self::open_encrypted(&candidate, dek_hex) {
                Ok(_) => true,
                Err(e) => {
                    log::warn!(
                        "[auth] encrypted candidate {candidate} did not open with the pending DEK: {e}"
                    );
                    false
                }
            };
            if !verified {
                continue;
            }
            let _ = remove_sqlite_sidecars(&candidate).ok();
            std::fs::rename(&candidate, path).map_err(|e| PersistenceError::MigrationFailed {
                version: 0,
                message: format!("promote encrypted candidate failed: {}", e),
            })?;
            Self::open_encrypted(path, dek_hex)?;
            return Ok(true);
        }
        Ok(false)
    }
}

fn remove_file_if_exists(path: &str) -> Result<(), PersistenceError> {
    if path.is_empty() || path == ":memory:" {
        return Ok(());
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PersistenceError::IoError {
            message: format!("failed to remove file {}: {}", path, e),
        }),
    }
}

fn remove_sqlite_files(base_path: &str) -> Result<(), PersistenceError> {
    remove_file_if_exists(base_path)?;
    remove_sqlite_sidecars(base_path)?;
    Ok(())
}

fn remove_sqlite_sidecars(base_path: &str) -> Result<(), PersistenceError> {
    remove_file_if_exists(&format!("{base_path}-wal"))?;
    remove_file_if_exists(&format!("{base_path}-shm"))?;
    remove_file_if_exists(&format!("{base_path}-journal"))?;
    Ok(())
}
