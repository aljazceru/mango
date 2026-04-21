/// Bootstrap database for auth parameters.
///
/// Stores a singleton row containing: salt, wrapped DEK, optional duress PIN hash,
/// and KDF parameters. This file is NOT encrypted (only the DEK is wrapped with the
/// KEK, and the raw DEK is never stored here -- T-28-01: AES-256-GCM tag detects tampering).
///
/// The bootstrap DB is separate from the main app DB (mango.db) so auth can be
/// resolved before the main encrypted DB is opened.
use rusqlite::params;

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
        Ok(Self { conn })
    }

    /// Write (or replace) the singleton auth params row.
    pub fn write_auth_params(&self, params: &AuthParams) -> Result<(), anyhow::Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO auth_params
                (id, salt, wrapped_dek, duress_hash,
                 kdf_memory_kib, kdf_iterations, kdf_parallelism)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
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
    /// main DB is permanently inaccessible (DEK is gone).
    pub fn delete_all(&self) -> Result<(), anyhow::Error> {
        self.conn.execute("DELETE FROM auth_params", [])?;
        Ok(())
    }

    // ── Quick 260421-bys: cold-launch bypass flag ─────────────────────────────
    //
    // `cold_launch_bypass` is a non-sensitive hint stored alongside auth_params.
    // Setting it to 1 without the corresponding keychain DEK entry is benign —
    // the cold-launch code in lib.rs falls back to Screen::Locked when the
    // keychain load returns None.
    //
    // NOTE: `write_auth_params` uses INSERT OR REPLACE and does NOT include this
    // column, so the PIN-setup path cannot accidentally reset the flag.

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
}
