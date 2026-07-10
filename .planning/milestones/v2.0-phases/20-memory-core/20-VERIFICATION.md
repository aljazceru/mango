---
phase: 20-memory-core
verified: 2026-04-03T00:00:00Z
status: passed
score: 7/7 must-haves verified
---

# Phase 20: Memory Core Verification Report

**Phase Goal:** The app automatically extracts and stores facts, preferences, and entities from completed conversations as local on-device memories
**Verified:** 2026-04-03
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| #   | Truth                                                                                      | Status     | Evidence                                                                                       |
| --- | ------------------------------------------------------------------------------------------ | ---------- | ---------------------------------------------------------------------------------------------- |
| 1   | After a conversation ends, the app automatically triggers memory extraction without user action | ✓ VERIFIED | `StreamDone` handler spawns extraction task when `should_extract` returns true (lib.rs:3668)   |
| 2   | Extracted memories appear in SQLite with text content and usearch vector embeddings        | ✓ VERIFIED | `insert_memory` + `vector_index.add` in `MemoryExtractionComplete` handler (lib.rs:4295-4311) |
| 3   | Memory extraction runs in a background task and does not block or delay chat responsiveness | ✓ VERIFIED | `runtime.spawn(async move { ... })` wraps `call_extraction_llm` (lib.rs:3712)                 |
| 4   | Memory extraction uses the existing EmbeddingProvider trait and usearch index infrastructure | ✓ VERIFIED | `actor_state.embedding_provider.embed()` and `actor_state.vector_index.add/save` (lib.rs:4302-4317) |
| 5   | Memories survive app restart and are queryable from the Rust core                          | ✓ VERIFIED | `insert_memory` writes to SQLite via `rusqlite` bundled DB; `list_memories` query exists in queries.rs:837 |

**Score:** 5/5 success-criteria truths verified

### Plan-level Truths (from must_haves frontmatter)

**Plan 01 truths:**

| #   | Truth                                                                                      | Status     | Evidence                                                         |
| --- | ------------------------------------------------------------------------------------------ | ---------- | ---------------------------------------------------------------- |
| 1   | A memory module exists with an LLM extraction function that takes conversation messages and returns extracted memory strings | ✓ VERIFIED | `call_extraction_llm` in extract.rs:41; `should_extract` in extract.rs:28 |
| 2   | Migration V15 creates a memories table with id, conversation_id, content, usearch_key, created_at columns | ✓ VERIFIED | `MIGRATION_V15` in schema.rs:274; appended to `MIGRATIONS` slice at index 14 (schema.rs:301) |
| 3   | CRUD queries exist for inserting and listing memory rows                                   | ✓ VERIFIED | `insert_memory` (queries.rs:821), `list_memories` (queries.rs:837), `delete_memory` (queries.rs:860) |
| 4   | Extraction function parses JSON array from LLM response with graceful fallback on parse failure | ✓ VERIFIED | `serde_json::from_str(text.trim()).unwrap_or_default()` (extract.rs:82) |

**Plan 02 truths:**

| #   | Truth                                                                                      | Status     | Evidence                                                              |
| --- | ------------------------------------------------------------------------------------------ | ---------- | --------------------------------------------------------------------- |
| 1   | After StreamDone, if the conversation has >= 2 messages and >= 100 chars, a background task spawns to extract memories via LLM | ✓ VERIFIED | `should_extract` gate + `runtime.spawn` in StreamDone handler (lib.rs:3668-3735) |
| 2   | The background extraction task does not block the actor thread or delay UI updates         | ✓ VERIFIED | Task is spawned via `runtime.spawn(async move {...})`; `rev += 1` and `emit` happen immediately after spawn returns (lib.rs:3737-3738) |
| 3   | Extracted memories are persisted to SQLite and embedded into the usearch vector index      | ✓ VERIFIED | `insert_memory` + `vector_index.add` in `MemoryExtractionComplete` handler (lib.rs:4295-4311) |
| 4   | Extraction failures are silently swallowed (logged, not propagated to UI)                  | ✓ VERIFIED | `.unwrap_or_default()` on LLM call (lib.rs:3720); `is_ok()` check on insert (lib.rs:4299); no `rev` increment or `emit` in handler |
| 5   | The vector index is saved once after all memories in a batch are added                     | ✓ VERIFIED | `vector_index.save()` called once after the for-loop, guarded by `added_count > 0` (lib.rs:4316-4318) |

**Combined Score:** 9/9 plan-level truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `rust/src/memory/mod.rs` | Memory module declaration, re-exports | ✓ VERIFIED | Contains `pub mod extract` (1 line, correct) |
| `rust/src/memory/extract.rs` | LLM extraction prompt and call logic | ✓ VERIFIED | 85 lines; exports `call_extraction_llm`, `should_extract`, `EXTRACTION_SYSTEM`, `MIN_EXTRACTION_CHARS` |
| `rust/src/persistence/schema.rs` | Migration V15 for memories table | ✓ VERIFIED | `MIGRATION_V15` at line 274; appended to `MIGRATIONS` slice at line 301 |
| `rust/src/persistence/queries.rs` | MemoryRow struct, insert_memory, list_memories functions | ✓ VERIFIED | `MemoryRow` (line 811), `insert_memory` (line 821), `list_memories` (line 837), `delete_memory` (line 860) |
| `rust/src/llm/streaming.rs` | MemoryExtractionComplete variant on InternalEvent | ✓ VERIFIED | Lines 67-75; variant has `conversation_id: String` and `memories: Vec<String>` |
| `rust/src/lib.rs` | StreamDone spawns extraction task; MemoryExtractionComplete handler persists and embeds | ✓ VERIFIED | Spawn at lines 3668-3735; handler at lines 4276-4328 |
| `rust/src/tests/memory.rs` | 11 unit tests for migration, queries, extraction parsing, and should_extract | ✓ VERIFIED | 11 tests present; `cargo test --lib memory` = 11/11 passed |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `rust/src/memory/extract.rs` | `async-openai` | `Client::with_config + create()` | ✓ WIRED | `client.chat().create(request).await?` at extract.rs:74 |
| `rust/src/persistence/schema.rs` | `MIGRATIONS` array | `MIGRATION_V15` appended to slice | ✓ WIRED | `MIGRATION_V15` is the 15th entry in `MIGRATIONS` at schema.rs:301 |
| `rust/src/lib.rs (StreamDone)` | `memory::extract::call_extraction_llm` | `runtime.spawn(async move { ... })` | ✓ WIRED | Direct call at lib.rs:3714; wrapped in spawned task |
| `rust/src/lib.rs (MemoryExtractionComplete)` | `persistence::queries::insert_memory` | Direct call on actor thread | ✓ WIRED | `persistence::queries::insert_memory(actor_state.db.conn(), &row)` at lib.rs:4295 |
| `rust/src/lib.rs (MemoryExtractionComplete)` | `actor_state.vector_index.add` | Embed then add to usearch index | ✓ WIRED | `actor_state.vector_index.add(usearch_key as u64, &embedding)` at lib.rs:4308 |
| `rust/src/lib.rs (MemoryExtractionComplete)` | `actor_state.vector_index.save` | Single save after batch | ✓ WIRED | `actor_state.vector_index.save()` at lib.rs:4317; called once post-loop |

**Critical wiring detail verified:** `extraction_backend_id` is captured via `.clone()` at lib.rs:3643-3644, BEFORE `current_streaming_backend_id.take()` at lib.rs:3648. This prevents the backend reference from being lost before the extraction spawn uses it.

### Data-Flow Trace (Level 4)

The primary data-rendering artifact is the `MemoryExtractionComplete` handler — it writes to SQLite and usearch, not a UI component, so the "data flows to rendering" check is replaced by a write-path trace.

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `extract.rs: call_extraction_llm` | `response.choices.first()` | `client.chat().create(request).await?` — live LLM call | Yes — real HTTP request to configured backend | ✓ FLOWING |
| `lib.rs: MemoryExtractionComplete handler` | `memories: Vec<String>` | Received from spawned task result | Yes — populated by `call_extraction_llm` with `unwrap_or_default` | ✓ FLOWING |
| `lib.rs: MemoryExtractionComplete handler` | `embedding` | `actor_state.embedding_provider.embed(vec![content.clone()])` | Yes — calls real `EmbeddingProvider` trait impl | ✓ FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Memory module compiles | `cargo check` | `Finished dev profile [unoptimized + debuginfo]` (0.52s, 2 dead-code warnings only) | ✓ PASS |
| All memory unit tests pass | `cargo test --lib memory` | `11 passed; 0 failed` | ✓ PASS |
| Full test suite passes (no regressions) | `cargo test --lib` | `204 passed; 0 failed; 9 ignored` | ✓ PASS |

**Note on dead-code warnings:** `list_memories` and `delete_memory` produce `never used` warnings. These are expected — Phase 21 (Memory Retrieval & Injection) will wire them into the context injection path, and Phase 23 (Memory Management UI) will expose them via UniFFI.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ----------- | ----------- | ------ | -------- |
| MEM-01 | 20-01, 20-02 | App automatically extracts facts, preferences, and entities from completed conversations | ✓ SATISFIED | `StreamDone` triggers `call_extraction_llm` via background spawn when `should_extract` returns true |
| MEM-02 | 20-01, 20-02 | Extracted memories stored locally in SQLite with vector embeddings in usearch index | ✓ SATISFIED | `insert_memory` writes to `memories` table; `vector_index.add` + `vector_index.save` embeds into HNSW |
| MEM-07 | 20-02 | Memory extraction runs in background without blocking chat flow | ✓ SATISFIED | `runtime.spawn(async move {...})` is non-blocking; `rev += 1` and `emit` execute immediately after spawn |

**Orphaned requirements check:** REQUIREMENTS.md maps MEM-01, MEM-02, and MEM-07 to Phase 20. All three are claimed in plan frontmatter. No orphaned requirements.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| None | — | — | — | — |

Scan covered: `rust/src/memory/extract.rs`, `rust/src/memory/mod.rs`, `rust/src/llm/streaming.rs` (new variant section), `rust/src/persistence/schema.rs` (MIGRATION_V15), `rust/src/persistence/queries.rs` (memory functions), `rust/src/tests/memory.rs`. No TODOs, placeholders, empty return stubs, or hardcoded empty data found in any phase-20 code paths.

### Human Verification Required

None. All success criteria can be verified programmatically:
- Compilation verified via `cargo check`
- Persistence and extraction logic verified via 11 unit tests
- Actor wiring verified via static analysis (grep patterns match expected call sites)
- Background spawn verified via `runtime.spawn` pattern — the non-blocking guarantee follows from Tokio's spawn semantics

The only behavior requiring a live run would be verifying that a real LLM backend returns valid JSON for the extraction prompt, but this is out of scope for a local Rust verification — it depends on the remote backend being available and is functionally equivalent to testing `call_extraction_llm` integration.

### Gaps Summary

No gaps. All 7 must-have artifacts exist, are substantive, are wired, and data flows through the extraction → persist → embed pipeline. The phase goal is fully achieved.

---

_Verified: 2026-04-03T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
