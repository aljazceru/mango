/// Bootstrap database for auth parameters.
///
/// Stores a singleton row containing: salt, wrapped DEK, optional duress PIN hash,
/// and KDF parameters. This file is NOT encrypted (only the DEK is wrapped with the
/// KEK, and the raw DEK is never stored here -- T-28-01: AES-256-GCM tag detects tampering).
///
/// The bootstrap DB is separate from the main app DB (mango.db) so auth can be
/// resolved before the main encrypted DB is opened.
use rusqlite::params;

/// A first-run continuation stored while PIN enrollment is still pending.
///
/// Used to route `CompleteOnboarding`/`SkipOnboarding` through the mandatory
/// PIN enrollment gate. The `action` field is `"complete"` or `"skip"`;
/// `conversation_id` is set only for a "complete" continuation, so the first
/// conversation is created exactly once across retries / restarts.
#[derive(Debug, Clone)]
pub struct PendingFirstRun {
    pub action: String,
    pub conversation_id: Option<String>,
}

/// Auth parameters persisted in the bootstrap database.
#[derive(Debug, Clone)]
pub struct AuthParams {
    /// 32-byte random salt for Argon2id key derivation.
    pub salt: Vec<u8>,
    /// DEK wrapped (AES-256-GCM encrypted) with the KEK derived from the user's PIN.
    pub wrapped_dek: Vec<u8>,
    /// PHC-format Argon2id hash of the duress PIN (optional).
    ///
    /// The Argon2id PHC string embeds its own random salt internally; no separate
    /// `duress_salt` column is needed — `verify_pin_hash` re-extracts the salt from
    /// the PHC string at verification time.
    pub duress_hash: Option<String>,
    /// Argon2id memory cost in KiB.
    pub kdf_memory_kib: u32,
    /// Argon2id iteration count.
    pub kdf_iterations: u32,
    /// Argon2id parallelism.
    pub kdf_parallelism: u32,
}

/// Bootstrap database handle.
///
/// This is a plain (unencrypted) SQLite database. Its security relies on the
/// OS file system permissions and the fact that the wrapped_dek cannot be
/// decrypted without the correct PIN (Argon2id + AES-256-GCM).
pub struct BootstrapDb {
    conn: rusqlite::Connection,
}

impl BootstrapDb {
    /// Open the bootstrap database at `path` and initialise the `auth_params` table.
    pub fn open(path: &str) -> Result<Self, anyhow::Error> {
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS auth_params (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                salt            BLOB NOT NULL,
                wrapped_dek     BLOB NOT NULL,
                duress_hash     TEXT,
                kdf_memory_kib  INTEGER NOT NULL,
                kdf_iterations  INTEGER NOT NULL,
                kdf_parallelism INTEGER NOT NULL
            );",
        )?;
        // Quick 260421-bys: add cold_launch_bypass column to existing DBs idempotently.
        // `cold_launch_bypass`: non-sensitive hint. Flipping this to 1 without the
        // corresponding keychain DEK entry is benign — cold-launch code falls back to
        // Screen::Locked when the keychain load returns None.
        conn.execute_batch(
            "ALTER TABLE auth_params ADD COLUMN cold_launch_bypass INTEGER NOT NULL DEFAULT 0;",
        )
        .ok(); // ignore "duplicate column" on existing DBs

        // Enrollment crash-recovery tables. Both are keyed as singletons (id=1)
        // so their presence is atomic and easy to inspect at startup.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pending_auth (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                salt            BLOB NOT NULL,
                wrapped_dek     BLOB NOT NULL,
                duress_hash     TEXT,
                kdf_memory_kib  INTEGER NOT NULL,
                kdf_iterations  INTEGER NOT NULL,
                kdf_parallelism INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS pending_first_run (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                action          TEXT NOT NULL,
                conversation_id TEXT
            );",
        )?;

        Ok(Self { conn })
    }

    /// Write (or replace) the singleton auth params row.
    ///
    /// Uses an explicit `INSERT ... ON CONFLICT DO UPDATE` upsert so that
    /// independent columns such as `cold_launch_bypass` are *preserved* on an
    /// existing row (fresh inserts still default to `0`). This prevents
    /// `SetDuressPin` and enrollment updates from silently resetting the
    /// "Never" auto-lock preference.
    pub fn write_auth_params(&self, params: &AuthParams) -> Result<(), anyhow::Error> {
        self.conn.execute(
            "INSERT INTO auth_params
                (id, salt, wrapped_dek, duress_hash,
                 kdf_memory_kib, kdf_iterations, kdf_parallelism)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                salt = excluded.salt,
                wrapped_dek = excluded.wrapped_dek,
                duress_hash = excluded.duress_hash,
                kdf_memory_kib = excluded.kdf_memory_kib,
                kdf_iterations = excluded.kdf_iterations,
                kdf_parallelism = excluded.kdf_parallelism",
            params![
                params.salt,
                params.wrapped_dek,
                params.duress_hash,
                params.kdf_memory_kib,
                params.kdf_iterations,
                params.kdf_parallelism,
            ],
        )?;
        Ok(())
    }

    /// Read the singleton auth params row. Returns `None` if not yet initialised.
    pub fn read_auth_params(&self) -> Result<Option<AuthParams>, anyhow::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT salt, wrapped_dek, duress_hash,
                    kdf_memory_kib, kdf_iterations, kdf_parallelism
             FROM auth_params WHERE id = 1",
        )?;
        let result = stmt.query_row([], |row| {
            Ok(AuthParams {
                salt: row.get(0)?,
                wrapped_dek: row.get(1)?,
                duress_hash: row.get(2)?,
                kdf_memory_kib: row.get::<_, u32>(3)?,
                kdf_iterations: row.get::<_, u32>(4)?,
                kdf_parallelism: row.get::<_, u32>(5)?,
            })
        });
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Return `true` if a singleton auth params row exists.
    ///
    /// Used to distinguish first launch (no params) from returning user (params present).
    pub fn has_auth_params(&self) -> bool {
        self.conn
            .query_row("SELECT COUNT(*) FROM auth_params WHERE id = 1", [], |r| {
                r.get::<_, i32>(0)
            })
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    /// Delete all auth params (used for duress wipe, per D-15).
    ///
    /// After this call `has_auth_params()` returns `false` and the encrypted
    /// main DB is permanently inaccessible (DEK is gone). Also clears any
    /// pending enrollment / first-run continuations so a duress wipe leaves a
    /// clean auth state.
    pub fn delete_all(&self) -> Result<(), anyhow::Error> {
        self.conn.execute("DELETE FROM auth_params", [])?;
        self.conn.execute("DELETE FROM pending_auth", [])?;
        self.conn.execute("DELETE FROM pending_first_run", [])?;
        Ok(())
    }

    // ── Quick 260421-bys: cold-launch bypass flag ─────────────────────────────
    //
    // `cold_launch_bypass` is a non-sensitive hint stored alongside auth_params.
    // Setting it to 1 without the corresponding keychain DEK entry is benign —
    // the cold-launch code in lib.rs falls back to Screen::Locked when the
    // keychain load returns None.
    //
    // NOTE: `write_auth_params` uses an explicit `INSERT ... ON CONFLICT DO
    // UPDATE` upsert that does NOT touch `cold_launch_bypass`, so the
    // PIN-setup / SetDuressPin paths cannot accidentally reset the flag.

    /// Read the cold-launch bypass flag. Returns `false` on any error or if no row exists.
    pub fn read_cold_launch_bypass(&self) -> Result<bool, anyhow::Error> {
        let val: i32 = self
            .conn
            .query_row(
                "SELECT cold_launch_bypass FROM auth_params WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        Ok(val != 0)
    }

    /// Persist the cold-launch bypass flag. Call after writing the keychain DEK
    /// (or after evicting it) so the two sources of truth stay in sync.
    pub fn write_cold_launch_bypass(&self, on: bool) -> Result<(), anyhow::Error> {
        self.conn.execute(
            "UPDATE auth_params SET cold_launch_bypass = ?1 WHERE id = 1",
            params![if on { 1i32 } else { 0i32 }],
        )?;
        Ok(())
    }

    // ── Pending auth (recoverable enrollment) ──────────────────────────────────

    /// Write (or replace) the singleton pending auth row.
    ///
    /// Only wrapped key material is stored here — never the raw DEK or PIN.
    /// This row is a recovery marker used to finish or roll back a first-time
    /// enrollment that is interrupted before the active auth row is promoted.
    pub fn write_pending_auth(&self, params: &AuthParams) -> Result<(), anyhow::Error> {
        self.conn.execute(
            "INSERT INTO pending_auth
                (id, salt, wrapped_dek, duress_hash,
                 kdf_memory_kib, kdf_iterations, kdf_parallelism)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                salt = excluded.salt,
                wrapped_dek = excluded.wrapped_dek,
                duress_hash = excluded.duress_hash,
                kdf_memory_kib = excluded.kdf_memory_kib,
                kdf_iterations = excluded.kdf_iterations,
                kdf_parallelism = excluded.kdf_parallelism",
            params![
                params.salt,
                params.wrapped_dek,
                params.duress_hash,
                params.kdf_memory_kib,
                params.kdf_iterations,
                params.kdf_parallelism,
            ],
        )?;
        Ok(())
    }

    /// Read the singleton pending auth row, if any.
    pub fn read_pending_auth(&self) -> Result<Option<AuthParams>, anyhow::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT salt, wrapped_dek, duress_hash,
                    kdf_memory_kib, kdf_iterations, kdf_parallelism
             FROM pending_auth WHERE id = 1",
        )?;
        let result = stmt.query_row([], |row| {
            Ok(AuthParams {
                salt: row.get(0)?,
                wrapped_dek: row.get(1)?,
                duress_hash: row.get(2)?,
                kdf_memory_kib: row.get::<_, u32>(3)?,
                kdf_iterations: row.get::<_, u32>(4)?,
                kdf_parallelism: row.get::<_, u32>(5)?,
            })
        });
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Return `true` if a pending auth row exists.
    pub fn has_pending_auth(&self) -> bool {
        self.conn
            .query_row("SELECT COUNT(*) FROM pending_auth WHERE id = 1", [], |r| {
                r.get::<_, i32>(0)
            })
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    /// Remove the pending auth row.
    pub fn clear_pending_auth(&self) -> Result<(), anyhow::Error> {
        self.conn.execute("DELETE FROM pending_auth", [])?;
        Ok(())
    }

    /// Atomically promote the pending auth row to the active `auth_params` row,
    /// preserving any existing `cold_launch_bypass` value, then delete pending.
    ///
    /// Returns `true` if a pending row was found and promoted, `false` otherwise.
    pub fn promote_pending_auth(&self) -> Result<bool, anyhow::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute(
            "INSERT INTO auth_params
                (id, salt, wrapped_dek, duress_hash,
                 kdf_memory_kib, kdf_iterations, kdf_parallelism)
             SELECT 1, salt, wrapped_dek, duress_hash,
                    kdf_memory_kib, kdf_iterations, kdf_parallelism
             FROM pending_auth WHERE id = 1
             ON CONFLICT(id) DO UPDATE SET
                salt = excluded.salt,
                wrapped_dek = excluded.wrapped_dek,
                duress_hash = excluded.duress_hash,
                kdf_memory_kib = excluded.kdf_memory_kib,
                kdf_iterations = excluded.kdf_iterations,
                kdf_parallelism = excluded.kdf_parallelism",
            [],
        )?;
        if n == 0 {
            tx.rollback()?;
            return Ok(false);
        }
        tx.execute("DELETE FROM pending_auth WHERE id = 1", [])?;
        tx.commit()?;
        Ok(true)
    }

    // ── Pending first-run continuation (Finding 2) ─────────────────────────────

    /// Persist the post-enrollment first-run continuation.
    pub fn set_pending_first_run(
        &self,
        action: &str,
        conversation_id: Option<&str>,
    ) -> Result<(), anyhow::Error> {
        self.conn.execute(
            "INSERT INTO pending_first_run (id, action, conversation_id)
             VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET
                action = excluded.action,
                conversation_id = excluded.conversation_id",
            params![action, conversation_id],
        )?;
        Ok(())
    }

    /// Read the pending first-run continuation, if any.
    pub fn read_pending_first_run(&self) -> Result<Option<PendingFirstRun>, anyhow::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT action, conversation_id FROM pending_first_run WHERE id = 1")?;
        let result = stmt.query_row([], |row| {
            Ok(PendingFirstRun {
                action: row.get(0)?,
                conversation_id: row.get(1)?,
            })
        });
        match result {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Remove the pending first-run continuation.
    pub fn clear_pending_first_run(&self) -> Result<(), anyhow::Error> {
        self.conn.execute("DELETE FROM pending_first_run", [])?;
        Ok(())
    }
}
