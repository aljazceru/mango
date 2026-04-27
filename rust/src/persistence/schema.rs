/// Migration v1: creates all 6 tables and seeds initial backend rows.
///
/// Tables: conversations, messages, backends, agent_sessions, agent_steps, attestation_cache.
/// Seeded backends: Tinfoil (IntelTdx, active).
pub const MIGRATION_V1: &str = "
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY NOT NULL,
    title       TEXT NOT NULL,
    model_id    TEXT NOT NULL,
    backend_id  TEXT NOT NULL,
    system_prompt TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    role            TEXT NOT NULL CHECK(role IN ('user','assistant','system')),
    content         TEXT NOT NULL,
    created_at      INTEGER NOT NULL,
    token_count     INTEGER
);

CREATE INDEX IF NOT EXISTS idx_messages_conv_time ON messages(conversation_id, created_at);

CREATE TABLE IF NOT EXISTS backends (
    id            TEXT PRIMARY KEY NOT NULL,
    name          TEXT NOT NULL,
    base_url      TEXT NOT NULL,
    model_list    TEXT NOT NULL DEFAULT '[]',
    tee_type      TEXT NOT NULL DEFAULT 'Unknown',
    display_order INTEGER NOT NULL DEFAULT 0,
    is_active     INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_sessions (
    id         TEXT PRIMARY KEY NOT NULL,
    title      TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'running',
    backend_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_steps (
    id             TEXT PRIMARY KEY NOT NULL,
    session_id     TEXT NOT NULL REFERENCES agent_sessions(id),
    step_number    INTEGER NOT NULL,
    action_type    TEXT NOT NULL,
    action_payload TEXT NOT NULL DEFAULT '{}',
    result         TEXT,
    status         TEXT NOT NULL DEFAULT 'pending',
    created_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS attestation_cache (
    backend_id  TEXT NOT NULL,
    tee_type    TEXT NOT NULL,
    status      TEXT NOT NULL,
    report_blob BLOB,
    verified_at INTEGER NOT NULL,
    expires_at  INTEGER NOT NULL,
    PRIMARY KEY (backend_id, tee_type)
);

INSERT OR IGNORE INTO backends (id, name, base_url, model_list, tee_type, display_order, is_active, created_at)
VALUES
    ('tinfoil', 'Tinfoil', 'https://inference.tinfoil.sh/v1/', '[\"llama3-3-70b\",\"deepseek-r1-0528\",\"kimi-k2-5\"]', 'IntelTdx', 0, 1, strftime('%s','now'));
";

/// Migration v2: add index on agent_steps for ordered step retrieval per session.
///
/// This additive index improves performance of list_agent_steps queries that
/// ORDER BY step_number for a given session_id. Needed for Phase 9 agent
/// orchestration which fetches step history on every agent turn.
pub const MIGRATION_V2: &str = "
CREATE INDEX IF NOT EXISTS idx_agent_steps_session_order
    ON agent_steps(session_id, step_number);
";

/// Migration v3: add settings table for global key-value app configuration.
///
/// Used for: global_system_prompt (default system prompt for all new conversations),
/// and any future app-wide preferences that need SQLite persistence.
pub const MIGRATION_V3: &str = "
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
";

/// Migration v4: add backend_health table for persisting failover state across restarts.
///
/// Stores per-backend exponential-backoff state so that the FailoverRouter can be
/// restored from disk on startup rather than starting fresh after every app restart.
/// The actor loop (Plan 02) loads this table on startup and writes to it on shutdown.
pub const MIGRATION_V4: &str = "
CREATE TABLE IF NOT EXISTS backend_health (
    backend_id          TEXT PRIMARY KEY NOT NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_failure_at     INTEGER,
    state               TEXT NOT NULL DEFAULT 'healthy',
    backoff_until       INTEGER
);
";

/// Migration v5: seed has_completed_onboarding flag in settings table.
///
/// Used by FfiApp::new to detect first launch: if the key is absent or 'false',
/// the app shows the onboarding wizard. CompleteOnboarding sets it to 'true'.
/// INSERT OR IGNORE ensures the row is only created once; subsequent upgrades
/// of existing installs that already completed onboarding are unaffected.
pub const MIGRATION_V5: &str = "
INSERT OR IGNORE INTO settings (key, value) VALUES ('has_completed_onboarding', 'false');
";

/// Migration v6: add documents and chunks tables for local on-device RAG.
///
/// Adds two tables:
/// - `documents`: stores document metadata (id, name, format, size, ingestion date, chunk count)
/// - `chunks`: stores text chunks with their integer rowid used as the usearch vector key
///
/// Key design decisions (per Phase 8 RESEARCH.md):
/// - `chunks.id` is INTEGER PRIMARY KEY AUTOINCREMENT so `last_insert_rowid()` gives us
///   the usearch key directly as a u64-safe integer. No UUID for chunks.
/// - `documents.id` remains TEXT (UUID) for consistency with the rest of the schema.
/// - `ON DELETE CASCADE` on chunks.document_id requires `PRAGMA foreign_keys = ON`
///   which is already enabled in Database::open (Phase 4 decision).
/// - `attached_document_ids` is TEXT (JSON array of document ID strings), nullable.
///   NULL means no documents attached to the conversation. Per D-11.
pub const MIGRATION_V6: &str = "
CREATE TABLE IF NOT EXISTS documents (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    format          TEXT NOT NULL CHECK(format IN ('pdf','txt','md')),
    size_bytes      INTEGER NOT NULL,
    ingestion_date  INTEGER NOT NULL,
    chunk_count     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS chunks (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id     TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index     INTEGER NOT NULL,
    text            TEXT NOT NULL,
    char_offset     INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);

ALTER TABLE conversations
    ADD COLUMN attached_document_ids TEXT DEFAULT NULL;
";

/// Migration v7: update seeded backend model lists to verified TEE-only models.
///
/// Tinfoil: use their actual model IDs (not meta-llama/ prefixed — Tinfoil uses short names).
pub const MIGRATION_V7: &str = "
UPDATE backends SET model_list = '[\"llama3-3-70b\",\"deepseek-r1-0528\",\"kimi-k2-5\"]'
    WHERE id = 'tinfoil';
";

/// Migration v8: remove the redpill backend from existing installs.
///
/// Redpill is no longer a supported provider. This migration cleans up all
/// redpill rows from backends, backend_health, and attestation_cache tables
/// for users who installed the app before this migration was added.
pub const MIGRATION_V8: &str = "
DELETE FROM backends WHERE id = 'redpill';
DELETE FROM backend_health WHERE backend_id = 'redpill';
DELETE FROM attestation_cache WHERE backend_id = 'redpill';
";

/// Migration v9: add vcek_cert_cache table for AMD SEV-SNP VCEK DER certificate caching.
///
/// The VCEK (Versioned Chip Endorsement Key) certificate is fetched from AMD KDS
/// (kdsintf.amd.com) during SEV-SNP attestation verification. It is uniquely keyed
/// by the VCEK URL, which encodes the chip_id and TCB version fields. The certificate
/// is stable for a given chip_id+TCB combination (it only changes when firmware is
/// updated, which changes the TCB fields and thus the URL key).
///
/// Caching avoids repeated KDS requests on each periodic re-attestation run, which
/// would otherwise trigger AMD's rate limiter (HTTP 429) and cause false attestation
/// failures that downgrade a Verified backend to Failed.
///
/// No TTL is enforced here — the URL itself acts as the cache key. If the TCB changes,
/// the new URL is a cache miss and a fresh fetch occurs automatically.
pub const MIGRATION_V9: &str = "
CREATE TABLE IF NOT EXISTS vcek_cert_cache (
    vcek_url    TEXT PRIMARY KEY NOT NULL,
    der         BLOB NOT NULL,
    cached_at   INTEGER NOT NULL
);
";

/// Migration v10: seed PPQ.AI backend with TEE/private models.
///
/// PPQ.AI exposes five AMD SEV-SNP protected private models accessible only with an
/// account API key. Seeded inactive (is_active=0) so existing users are not affected;
/// the user must enter their API key in Settings to activate.
pub const MIGRATION_V10: &str = "
INSERT OR IGNORE INTO backends (id, name, base_url, model_list, tee_type, display_order, is_active, created_at)
VALUES (
    'ppq-ai',
    'PPQ.AI',
    'https://api.ppq.ai/private/v1/',
    '[\"private/kimi-k2-5\",\"private/deepseek-r1-0528\",\"private/gpt-oss-120b\",\"private/llama3-3-70b\",\"private/qwen3-vl-30b\"]',
    'AmdSevSnp',
    1,
    0,
    strftime('%s','now')
);
";

/// Migration v11: switch the seeded PPQ.AI backend to the native private transport base URL.
///
/// Existing installs seeded with the legacy public OpenAI-compatible URL were not actually
/// using PPQ private mode. Preserve any user-customized base_url and only rewrite the
/// untouched seed value.
pub const MIGRATION_V11: &str = "
UPDATE backends
SET base_url = 'https://api.ppq.ai/private/v1/'
WHERE id = 'ppq-ai'
  AND base_url = 'https://api.ppq.ai/v1/';
";

/// Migration v12: add max_concurrent_requests column to backends table.
///
/// Per D-05: concurrency limit is configurable per backend, stored in SQLite,
/// and used to initialize the Tokio Semaphore that enforces the limit in Plan 02.
/// DEFAULT 5 means existing backends get a reasonable limit without user action.
pub const MIGRATION_V12: &str = "
ALTER TABLE backends ADD COLUMN max_concurrent_requests INTEGER NOT NULL DEFAULT 5;
";

/// Migration v13: add supports_tool_use column to backends table.
///
/// Per D-01 (Phase 17): DEFAULT 1 (optimistic) -- all existing backends assumed to support tools.
/// Per D-02: single boolean column, not a JSON blob or capabilities table.
pub const MIGRATION_V13: &str = "
ALTER TABLE backends ADD COLUMN supports_tool_use INTEGER NOT NULL DEFAULT 1;
";

/// Migration v14: seed TEE attestation policy defaults in the settings table.
///
/// Stores the TDX and SNP attestation policy thresholds as JSON so they can
/// be updated at runtime without recompiling the app. The seeded values are
/// identical to the prior compile-time constants, so existing installs receive
/// the same behaviour as before the migration.
///
/// Keys:
/// - `tee_policy_tdx`: TdxPolicy JSON (minimum_tee_tcb_svn, accepted_mr_seams)
/// - `tee_policy_snp`: SnpPolicy JSON (minimum_bootloader, minimum_tee, minimum_snp, minimum_microcode)
pub const MIGRATION_V14: &str = r#"
INSERT OR IGNORE INTO settings (key, value) VALUES (
    'tee_policy_tdx',
    '{"minimum_tee_tcb_svn":"03010200000000000000000000000000","accepted_mr_seams":["476a2997c62bccc78370913d0a80b956e3721b24272bc66c4d6307ced4be2865c40e26afac75f12df3425b03eb59ea7c","7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d","685f891ea5c20e8fa27b151bf34bf3b50fbaf7143cc53662727cbdb167c0ad8385f1f6f3571539a91e104a1c96d75e04","49b66faa451d19ebbdbe89371b8daf2b65aa3984ec90110343e9e2eec116af08850fa20e3b1aa9a874d77a65380ee7e6"]}'
);
INSERT OR IGNORE INTO settings (key, value) VALUES (
    'tee_policy_snp',
    '{"minimum_bootloader":7,"minimum_tee":0,"minimum_snp":14,"minimum_microcode":72}'
);
"#;

/// Migration v15: add memories table for persistent cross-conversation memory.
///
/// Stores memory facts extracted from conversations by the LLM extraction pipeline.
/// Each row links to a conversation, holds the extracted content string, and a
/// usearch_key (UNIQUE INTEGER) that maps to the HNSW vector index entry for
/// semantic recall. The idx_memories_conversation index supports efficient
/// per-conversation memory lookup during context injection (Plan 02).
pub const MIGRATION_V15: &str = "
CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    content         TEXT NOT NULL,
    usearch_key     INTEGER NOT NULL UNIQUE,
    created_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_memories_conversation ON memories(conversation_id);
";

/// Migration v16: add tools_enabled column to conversations table (Phase 27, CHAT-TOOL-01).
///
/// Per-conversation toggle for chat tool use. DEFAULT 0 means tools are off by default,
/// preserving existing behaviour for all pre-Phase-27 conversations. Users opt-in via the
/// SetConversationToolsEnabled action. The column drives build_chat_tools() call selection
/// in do_send_message (Plan 02).
pub const MIGRATION_V16: &str = "
ALTER TABLE conversations ADD COLUMN tools_enabled INTEGER NOT NULL DEFAULT 0;
";

/// Migration v17: add image_path column to messages table (QT-ECE encrypted image persistence).
///
/// Nullable TEXT column stores the absolute on-disk path of the encrypted image file
/// (`{data_dir}/images/{message_id}.jpg.mgo1`). NULL for text-only messages.
/// The file itself is AES-256-GCM encrypted (MGO1 format) via file_crypto::encrypt_file;
/// plaintext image bytes are never written to disk (T-ECE-02).
pub const MIGRATION_V17: &str = "
ALTER TABLE messages ADD COLUMN image_path TEXT;
";

/// Migration v18: add directory_sources and directory_files tables for directory-based RAG.
///
/// Phase 32 (DIR-03, DIR-04): supports adding whole directories (e.g. Obsidian vault) as
/// RAG sources with periodic re-sync. `directory_sources` captures the user-chosen folder
/// plus cross-platform access handle (Desktop path, iOS bookmark_data, Android tree_uri)
/// and JSON-encoded exclusion globs. `directory_files` stores per-file fingerprints
/// (mtime + size) so the sync pass can diff against the on-disk state and emit
/// add/modify/delete events. The FK with ON DELETE CASCADE guarantees file rows are
/// removed atomically when a source is deleted (T-32-I1 mitigation). UNIQUE (source_id,
/// file_path) enforces the invariant that each path is tracked exactly once per source.
pub const MIGRATION_V18: &str = "
CREATE TABLE IF NOT EXISTS directory_sources (
    id              TEXT PRIMARY KEY NOT NULL,
    display_name    TEXT NOT NULL,
    path            TEXT,
    bookmark_data   BLOB,
    tree_uri        TEXT,
    exclusion_globs TEXT NOT NULL DEFAULT '[]',
    last_synced_at  INTEGER,
    file_count      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS directory_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id       TEXT NOT NULL REFERENCES directory_sources(id) ON DELETE CASCADE,
    file_path       TEXT NOT NULL,
    mtime_secs      INTEGER NOT NULL,
    size_bytes      INTEGER NOT NULL,
    document_id     TEXT,
    UNIQUE (source_id, file_path)
);

CREATE INDEX IF NOT EXISTS idx_dirfiles_source ON directory_files(source_id);
";

/// Migration v19: add shape/freshness/orchestrated_components columns to attestation_cache.
///
/// Closes RED-09 / RED-11 by persisting Redpill attestation metadata across app
/// restarts. All three columns are nullable TEXT — non-Redpill backends and
/// pre-V19 rows store NULL. `orchestrated_components` is JSON-encoded
/// `Vec<(String, String)>` (list-of-pairs preserves duplicate labels).
pub const MIGRATION_V19: &str = "
ALTER TABLE attestation_cache ADD COLUMN shape TEXT;
ALTER TABLE attestation_cache ADD COLUMN freshness TEXT;
ALTER TABLE attestation_cache ADD COLUMN orchestrated_components TEXT;
";

/// All migrations in order.
pub const MIGRATIONS: &[&str] = &[
    MIGRATION_V1,
    MIGRATION_V2,
    MIGRATION_V3,
    MIGRATION_V4,
    MIGRATION_V5,
    MIGRATION_V6,
    MIGRATION_V7,
    MIGRATION_V8,
    MIGRATION_V9,
    MIGRATION_V10,
    MIGRATION_V11,
    MIGRATION_V12,
    MIGRATION_V13,
    MIGRATION_V14,
    MIGRATION_V15,
    MIGRATION_V16,
    MIGRATION_V17,
    MIGRATION_V18,
    MIGRATION_V19,
];

#[cfg(test)]
mod tests {
    use crate::persistence::Database;

    #[test]
    fn test_migration_v18_creates_directory_tables() {
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
            tables.contains(&"directory_sources".to_string()),
            "directory_sources table missing after MIGRATION_V18, got: {:?}",
            tables
        );
        assert!(
            tables.contains(&"directory_files".to_string()),
            "directory_files table missing after MIGRATION_V18, got: {:?}",
            tables
        );
    }

    #[test]
    fn test_directory_files_cascade_delete() {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO directory_sources (id, display_name, path, exclusion_globs, file_count, created_at)
             VALUES ('src1', 'My Vault', '/tmp/vault', '[]', 0, 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO directory_files (source_id, file_path, mtime_secs, size_bytes)
             VALUES ('src1', 'a.md', 100, 500), ('src1', 'b.md', 200, 1000)",
            [],
        )
        .unwrap();
        let before: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM directory_files WHERE source_id = 'src1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(before, 2, "two files should exist before cascade delete");
        conn.execute("DELETE FROM directory_sources WHERE id = 'src1'", [])
            .unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM directory_files WHERE source_id = 'src1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after, 0, "files should be cascade-deleted when source removed");
    }

    #[test]
    fn test_migration_v19_adds_three_columns() {
        let db = Database::open(":memory:").expect("open in-memory db");
        let conn = db.conn();
        let mut stmt = conn
            .prepare("PRAGMA table_info('attestation_cache')")
            .expect("pragma");
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(
            cols.contains(&"shape".to_string()),
            "shape column missing; got {:?}",
            cols
        );
        assert!(
            cols.contains(&"freshness".to_string()),
            "freshness column missing; got {:?}",
            cols
        );
        assert!(
            cols.contains(&"orchestrated_components".to_string()),
            "orchestrated_components column missing; got {:?}",
            cols
        );
    }

    #[test]
    fn test_directory_files_unique_source_path() {
        let db = Database::open(":memory:").unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO directory_sources (id, display_name, path, exclusion_globs, file_count, created_at)
             VALUES ('src1', 'My Vault', '/tmp/vault', '[]', 0, 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO directory_files (source_id, file_path, mtime_secs, size_bytes)
             VALUES ('src1', 'dup.md', 100, 500)",
            [],
        )
        .unwrap();
        let dup = conn.execute(
            "INSERT INTO directory_files (source_id, file_path, mtime_secs, size_bytes)
             VALUES ('src1', 'dup.md', 200, 800)",
            [],
        );
        assert!(
            dup.is_err(),
            "second insert with same (source_id, file_path) should fail UNIQUE constraint"
        );
    }
}
