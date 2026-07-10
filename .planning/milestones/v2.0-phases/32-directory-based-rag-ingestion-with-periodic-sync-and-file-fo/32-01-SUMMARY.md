---
phase: 32
plan: 01
subsystem: persistence
tags: [rag, persistence, sqlite, migration, directory-sync]
requirements: [DIR-03, DIR-04]
dependency_graph:
  requires: []
  provides:
    - "MIGRATION_V18 (directory_sources + directory_files tables)"
    - "DirectorySourceRow + DirectoryFileRow CRUD layer"
  affects:
    - rust/src/persistence/schema.rs
    - rust/src/persistence/queries.rs
    - rust/src/persistence/mod.rs
    - rust/src/tests/persistence.rs
    - rust/src/tests/rag.rs
tech_stack:
  added: []
  patterns:
    - "Idempotent CREATE TABLE IF NOT EXISTS in MIGRATIONS array"
    - "ON DELETE CASCADE for FK integrity (T-32-I1 mitigation)"
    - "ON CONFLICT DO UPDATE for fingerprint upsert"
key_files:
  created: []
  modified:
    - rust/src/persistence/schema.rs
    - rust/src/persistence/queries.rs
    - rust/src/persistence/mod.rs
    - rust/src/tests/persistence.rs
    - rust/src/tests/rag.rs
decisions:
  - "DirectorySourceRow carries all three platform handles (path, bookmark_data, tree_uri) as nullable columns so the same row shape works across Desktop/iOS/Android without a type-tag column"
  - "upsert_directory_file uses ON CONFLICT(source_id, file_path) DO UPDATE rather than INSERT OR REPLACE to preserve the AUTOINCREMENT id across fingerprint updates"
  - "Added #[allow(dead_code)] to delete_directory_file and count_directory_files since they're consumed by Plan 32-02+; other new fns are re-exported through persistence/mod.rs so the compiler links them without warnings once upstream callers land"
metrics:
  duration: ~12min
  completed_date: 2026-04-19
  tasks_completed: 2
  commits: 3
---

# Phase 32 Plan 01: Persistence Foundation — MIGRATION_V18 + Directory CRUD Summary

SQLite MIGRATION_V18 adds `directory_sources` and `directory_files` tables; the `queries.rs` module exposes 11 CRUD fns (`DirectorySourceRow`, `DirectoryFileRow`) covering source lifecycle, fingerprint upsert, cascade delete, and bookmark/exclusion updates — unblocking Plans 32-02..07.

## Objective

Ground Phase 32 in the persistence layer by adding the two tables and CRUD layer every other plan depends on, before any sync/UI logic is attempted.

## Tasks Completed

### Task 1: Migration V18 schema + registration (TDD)

- **Files:** `rust/src/persistence/schema.rs`, `rust/src/tests/persistence.rs`
- **Commits:**
  - `d6885bf` — test(32-01): failing tests for MIGRATION_V18 directory tables (RED)
  - `3adcf5c` — feat(32-01): MIGRATION_V18 for directory_sources and directory_files (GREEN)
- **What:**
  - Appended `MIGRATION_V18` constant with `directory_sources`, `directory_files`, and `idx_dirfiles_source`.
  - Registered in `MIGRATIONS` array (index 17 → user_version 18).
  - `directory_files.source_id REFERENCES directory_sources(id) ON DELETE CASCADE` — FK already enforced in both `open` and `open_encrypted` via `PRAGMA foreign_keys = ON`.
  - `UNIQUE (source_id, file_path)` prevents duplicate fingerprint rows per source.
  - Added `#[cfg(test)] mod tests` in schema.rs with 3 tests: table existence, cascade delete, unique constraint.
  - Bumped 3 version asserts (17 → 18) in `rust/src/tests/persistence.rs`.

### Task 2: DirectorySource + DirectoryFile CRUD queries (TDD)

- **Files:** `rust/src/persistence/queries.rs`, `rust/src/persistence/mod.rs`, `rust/src/tests/rag.rs`
- **Commit:** `b011e88` — feat(32-01): DirectorySource + DirectoryFile CRUD queries (RED+GREEN combined; tests written first and compiled-in same change)
- **What:**
  - `DirectorySourceRow` (9 fields) + `DirectoryFileRow` (6 fields).
  - 11 public fns:
    - `insert_directory_source`, `list_directory_sources`, `get_directory_source`, `delete_directory_source`
    - `update_directory_source_last_synced`, `update_directory_source_exclusions`, `update_directory_source_bookmark`
    - `upsert_directory_file` (ON CONFLICT(source_id, file_path) DO UPDATE), `list_directory_files_by_source`, `delete_directory_file`, `count_directory_files`
  - 4 unit tests in `directory_tests` module: `test_directory_source_queries` (all 3 platform variants, list, get, update_last_synced, delete), `test_directory_file_fingerprints` (upsert idempotency + delete), `test_count_directory_files`, `test_update_exclusions_and_bookmark`.
  - Re-exported all new symbols from `persistence/mod.rs`.

## Verification

```
$ cargo test -p mango_core --lib -- --test-threads 1
test result: ok. 294 passed; 0 failed; 10 ignored; 0 measured
```

All 3 schema tests + 4 CRUD tests green; all pre-existing persistence tests updated to expected user_version 18 and still pass.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Stale version assert in `rust/src/tests/rag.rs`**
- **Found during:** Task 2 full-suite verification
- **Issue:** `test_migration_v6_version` hard-coded `assert_eq!(version, 17)` and failed after MIGRATION_V18 lifted user_version to 18.
- **Fix:** Bumped assert from 17 → 18 with matching comment, consistent with the same fix already applied to 3 asserts in `rust/src/tests/persistence.rs` per the plan's explicit instruction.
- **Commit:** `b011e88`

**2. [Rule 2 — Missing critical functionality] Re-export new CRUD symbols from `persistence/mod.rs`**
- **Found during:** Task 2
- **Issue:** The plan didn't instruct updating `persistence/mod.rs`, but the existing crate convention re-exports every `queries::*` fn from `persistence::` to avoid leaking the submodule path to callers (all current persistence users import from `crate::persistence::...`). Not re-exporting would force Plan 32-02+ to either reach into `crate::persistence::queries::*` (breaking convention) or patch mod.rs mid-plan.
- **Fix:** Added all 11 fns + 2 row structs to the `pub use queries::{...}` block.
- **Files modified:** `rust/src/persistence/mod.rs`
- **Commit:** `b011e88`

### `#[allow(dead_code)]` annotations

`delete_directory_file` and `count_directory_files` were annotated with `#[allow(dead_code)]` because they are genuinely unused until Plan 32-02+ — the crate otherwise emits warnings for functions only referenced by tests + `pub use`. The other 9 fns are exercised by the 4 unit tests and need no annotation. This matches the existing pattern in queries.rs (`update_agent_step_status` uses the same annotation).

## Acceptance Criteria

- [x] grep "MIGRATION_V18" rust/src/persistence/schema.rs returns ≥2 matches (const + array) — verified (5 matches including docstring + tests).
- [x] grep "directory_sources" / "directory_files" / "ON DELETE CASCADE" in schema.rs — verified.
- [x] 3 schema tests pass.
- [x] 11 CRUD fns exist — verified.
- [x] `ON CONFLICT` in upsert — verified at queries.rs:1175.
- [x] 4 directory_tests pass.
- [x] Full `cargo test -p mango_core --lib persistence` suite passes (38 tests).

## Known Stubs

None — every fn has a concrete implementation backed by a `prepare_cached` SQL statement.

## Threat Flags

None — plan introduces no new network/auth/file-access surface beyond the threat model already documented in 32-01-PLAN.md (T-32-I1 FK cascade already mitigated; T-32-V6 accepted at encryption layer).

## Self-Check: PASSED

- FOUND: rust/src/persistence/schema.rs (MIGRATION_V18 present at line ~302, registered in MIGRATIONS array, schema::tests module with 3 tests)
- FOUND: rust/src/persistence/queries.rs (DirectorySourceRow, DirectoryFileRow, 11 fns, directory_tests with 4 tests)
- FOUND: rust/src/persistence/mod.rs (re-exports of all new symbols)
- FOUND: commit d6885bf — RED tests
- FOUND: commit 3adcf5c — MIGRATION_V18 GREEN
- FOUND: commit b011e88 — CRUD layer
- All 294 library tests green.
