---
phase: 20-memory-core
plan: 02
subsystem: memory
tags: [memory, actor, streaming, sqlite, usearch, background-task]
dependency_graph:
  requires: [memory-module, migration-v15, memory-queries, extraction-function]
  provides: [memory-actor-integration, end-to-end-memory-extraction]
  affects: [lib, llm/streaming]
tech_stack:
  added: []
  patterns: [actor background spawn, InternalEvent round-trip, silent-failure extraction]
key_files:
  created: []
  modified:
    - rust/src/llm/streaming.rs
    - rust/src/lib.rs
decisions:
  - "Captured extraction_backend_id before current_streaming_backend_id.take() to avoid losing the backend reference after the health-tracking block consumes it"
  - "Used core_tx_for_thread (not core_tx) for clone inside StreamDone -- matches existing EmbeddingComplete pattern"
  - "Added continue; in MemoryExtractionComplete handler to skip the default rev+1/emit at end of InternalEvent match -- memories are invisible in Phase 20 UI"
metrics:
  duration: "~10 minutes"
  completed: "2026-04-03"
  tasks: 2
  files: 2
---

# Phase 20 Plan 02: Memory Actor Integration Summary

End-to-end memory extraction pipeline wired into the actor loop -- StreamDone spawns a background LLM extraction task and MemoryExtractionComplete persists facts to SQLite and embeds them in the usearch vector index.

## What Was Built

### rust/src/llm/streaming.rs

- New `MemoryExtractionComplete` variant added to `InternalEvent` enum
- Carries `conversation_id: String` and `memories: Vec<String>`
- Follows doc comment style of `AgentStepComplete` and `EmbeddingComplete`

### rust/src/lib.rs

**StreamDone handler additions:**

- Capture `extraction_backend_id = actor_state.current_streaming_backend_id.clone()` BEFORE the `.take()` call used for health tracking
- After health tracking and before `rev += 1`/`emit()`: spawn background extraction when `memory::extract::should_extract(&messages_snapshot)` returns true
- Extraction task calls `memory::extract::call_extraction_llm` and sends `MemoryExtractionComplete` back to the actor via `core_tx_for_thread`
- Uses `unwrap_or_default()` on the extraction result -- failures are silently swallowed

**MemoryExtractionComplete handler:**

- Iterates extracted memory strings
- For each: generates UUID `id`, random `usearch_key` via `uuid::Uuid::new_v4().as_u128() as i64`
- Calls `persistence::queries::insert_memory` -- failures are silently skipped
- On insert success: calls `actor_state.embedding_provider.embed()` and `actor_state.vector_index.add()`
- Tracks `added_count`; calls `actor_state.vector_index.save()` once after all memories added
- Logs extraction result via `log::info!`
- No `rev += 1` or `emit()` -- memories invisible in Phase 20 UI
- Uses `continue;` to skip default rev/emit at end of match

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Used correct channel variable name `core_tx_for_thread`**
- **Found during:** Task 2 implementation
- **Issue:** Plan specified `core_tx.clone()` inside StreamDone but the variable in scope at that point is `core_tx_for_thread` (as used by EmbeddingComplete and AgentStepComplete patterns)
- **Fix:** Used `core_tx_for_thread.clone()` matching the existing pattern
- **Files modified:** rust/src/lib.rs
- **Commit:** 1e4028f

## Known Stubs

None. All memory extraction paths are fully implemented.

## Pre-existing Test Failures (Out of Scope)

5 persistence tests fail due to Plan 01 adding MIGRATION_V15 (schema version now 15) but tests hardcoding expected version 14:
- `test_migration_idempotent`
- `test_migration_v1_to_v2`
- `test_migration_version_increments`
- `test_migration_v11_seeds_ppq_ai_private_transport`
- `test_migration_v6_version`

These failures were introduced by Plan 01 and are out of scope for Plan 02. `cargo test --lib memory` (the required check) passes with 11/11 tests. These persistence tests should be updated to expect version 15 in a follow-up.

## Verification

- `cargo check` exits 0 (2 dead-code warnings on `list_memories` and `delete_memory` -- expected until Phase 21 wires up the UI)
- `cargo test --lib memory` exits 0, 11 tests passing

## Self-Check: PASSED

Files verified:
- rust/src/llm/streaming.rs: contains `MemoryExtractionComplete {` -- FOUND
- rust/src/lib.rs: contains `memory::extract::should_extract` -- FOUND
- rust/src/lib.rs: contains `memory::extract::call_extraction_llm` -- FOUND
- rust/src/lib.rs: contains `MemoryExtractionComplete` (both spawn site and handler) -- FOUND
- rust/src/lib.rs: contains `insert_memory` call in handler -- FOUND
- rust/src/lib.rs: contains `vector_index.add` call in handler -- FOUND
- rust/src/lib.rs: contains `vector_index.save()` call in handler -- FOUND
- rust/src/lib.rs: contains `runtime.spawn(async move` in StreamDone context -- FOUND

Commits verified:
- 913d8c1: feat(20-02): add MemoryExtractionComplete variant to InternalEvent -- FOUND
- 1e4028f: feat(20-02): wire memory extraction into actor loop -- FOUND
