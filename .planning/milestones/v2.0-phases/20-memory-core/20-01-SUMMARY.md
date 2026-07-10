---
phase: 20-memory-core
plan: 01
subsystem: memory
tags: [memory, persistence, extraction, sqlite, llm]
dependency_graph:
  requires: []
  provides: [memory-module, migration-v15, memory-queries, extraction-function]
  affects: [persistence, lib]
tech_stack:
  added: []
  patterns: [async-openai chat completions non-streaming, rusqlite prepare_cached CRUD]
key_files:
  created:
    - rust/src/memory/mod.rs
    - rust/src/memory/extract.rs
    - rust/src/tests/memory.rs
  modified:
    - rust/src/persistence/schema.rs
    - rust/src/persistence/queries.rs
    - rust/src/lib.rs
    - rust/src/tests/mod.rs
decisions:
  - "Used Database::open(':memory:') in tests instead of a free-function run_migrations (which does not exist in schema.rs)"
  - "Used ChatCompletionRequestSystemMessageArgs / ChatCompletionRequestUserMessageArgs pattern from agent/loop.rs -- not the Args builder from plan spec (which does not exist in async-openai 0.33)"
metrics:
  duration: "~15 minutes"
  completed: "2026-04-03"
  tasks: 2
  files: 7
---

# Phase 20 Plan 01: Memory Module Foundation Summary

Foundational memory extraction module with LLM call, SQLite migration, and CRUD queries -- no actor integration yet (Plan 02 wires these in).

## What Was Built

### rust/src/memory/extract.rs

- `EXTRACTION_SYSTEM` const: system prompt instructing LLM to return a JSON array of fact strings
- `MIN_EXTRACTION_CHARS: usize = 100`: threshold constant
- `should_extract(messages: &[(String, String)]) -> bool`: gate that requires >= 2 messages AND >= 100 total content chars
- `call_extraction_llm(backend, messages, model) -> anyhow::Result<Vec<String>>`: builds transcript, calls the backend via async-openai non-streaming, parses JSON response with `unwrap_or_default` fallback

### rust/src/persistence/schema.rs

- `MIGRATION_V15`: creates `memories` table with `id TEXT PK`, `conversation_id TEXT`, `content TEXT`, `usearch_key INTEGER UNIQUE`, `created_at INTEGER` plus `idx_memories_conversation` index
- Appended `MIGRATION_V15` to `MIGRATIONS` slice (now 15 entries)

### rust/src/persistence/queries.rs

- `MemoryRow` struct: mirrors the memories table schema
- `insert_memory`: INSERT with prepare_cached
- `list_memories`: SELECT ORDER BY created_at DESC
- `delete_memory`: DELETE by id

### rust/src/tests/memory.rs

11 unit tests covering the full surface:
- Migration creates table (count query on in-memory DB)
- Insert + list round-trip with DESC ordering
- Delete removes row
- UNIQUE constraint on usearch_key enforced
- should_extract gating (empty, 1 msg, short, sufficient)
- JSON parsing: valid array, invalid prose, empty array

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed async-openai import paths**
- **Found during:** Task 1 cargo check
- **Issue:** Plan specified `async_openai::types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role}` but in async-openai 0.33 these types live under `async_openai::types::chat` (not `types`), and `ChatCompletionRequestMessageArgs` does not exist -- the actual builder types are `ChatCompletionRequestSystemMessageArgs` and `ChatCompletionRequestUserMessageArgs`
- **Fix:** Used `async_openai::types::chat::{ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs}` matching the pattern in `rust/src/agent/loop.rs`
- **Files modified:** rust/src/memory/extract.rs
- **Commit:** 263a035

**2. [Rule 1 - Bug] Used Database::open(":memory:") instead of non-existent run_migrations free function**
- **Found during:** Task 2 test authoring
- **Issue:** Plan specified `persistence::schema::run_migrations(&conn)` but this free function does not exist -- migrations run via `Database::open()` which calls the private `run_migrations` method
- **Fix:** Used `Database::open(":memory:")` consistent with `rust/src/tests/rag.rs` and `rust/src/tests/persistence.rs` patterns
- **Files modified:** rust/src/tests/memory.rs
- **Commit:** cf44f7d

## Known Stubs

None. All functions are fully implemented. The `insert_memory`, `list_memories`, and `delete_memory` functions produce dead-code warnings at this stage (Plan 02 will wire them into the actor loop).

## Verification

- `cargo check` exits 0 (4 dead-code warnings only -- expected, Plan 02 wires these)
- `cargo test --lib memory` exits 0 with 11 tests passing

## Self-Check: PASSED

Files verified:
- rust/src/memory/mod.rs: FOUND
- rust/src/memory/extract.rs: FOUND
- rust/src/tests/memory.rs: FOUND
- rust/src/persistence/schema.rs: contains MIGRATION_V15 and memories table
- rust/src/persistence/queries.rs: contains MemoryRow, insert_memory, list_memories, delete_memory
- rust/src/lib.rs: contains `pub mod memory`
- rust/src/tests/mod.rs: contains `mod memory`

Commits verified:
- 263a035: feat(20-01) -- FOUND
- cf44f7d: test(20-01) -- FOUND
