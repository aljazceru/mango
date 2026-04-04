---
phase: 21-memory-retrieval-injection
plan: 01
subsystem: memory
tags: [rust, memory, usearch, sqlite, rag, system-prompt, embedding]

# Dependency graph
requires:
  - phase: 20-memory-core
    provides: memories table with usearch_key, MemoryRow, insert_memory, EmbeddingProvider trait, vector_index on actor_state
  - phase: 8-local-on-device-rag
    provides: usearch HNSW index, build_system_with_context pattern, RAG injection in do_send_message
provides:
  - memory::retrieve module with MemoryResult, DEFAULT_MEMORY_TOP_K=5, build_system_with_memories
  - persistence query get_memory_content_by_usearch_keys resolving usearch keys to memory content
  - Memory injection wired into do_send_message after RAG injection on every message
  - Hoisted query embedding shared between RAG and memory search (computed once)
affects: [22-memory-ui, agent-tools, any future phase touching do_send_message or system prompt construction]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-stage system prompt augmentation: RAG injection then memory injection, each falling through gracefully when no results"
    - "Shared embedding for both RAG and memory search computed once before both blocks"
    - "usearch keys as shared namespace: chunk keys silently ignored when memory lookup returns empty"

key-files:
  created:
    - rust/src/memory/retrieve.rs
  modified:
    - rust/src/memory/mod.rs
    - rust/src/persistence/queries.rs
    - rust/src/tests/memory.rs
    - rust/src/lib.rs

key-decisions:
  - "Reuse the shared usearch HNSW index for memory search (same index as RAG chunks); chunk keys silently fall through via get_memory_content_by_usearch_keys returning empty for non-memory keys"
  - "Hoist query embedding before RAG block so both RAG and memory retrieval share one embed() call"
  - "Mirror rag/context.rs pattern exactly for build_system_with_memories to maintain consistency"

patterns-established:
  - "build_system_with_memories mirrors build_system_with_context: empty slice returns base unchanged, populated slice prepends <memories> XML block"
  - "Persistence queries for vector key lookup follow the same pattern: Vec<(key, content)>, silent omission of missing keys"

requirements-completed: [MEM-03]

# Metrics
duration: 4min
completed: 2026-04-04
---

# Phase 21 Plan 01: Memory Retrieval and Injection Summary

**Semantic memory retrieval wired into do_send_message: relevant past memories inject into system prompt via shared usearch HNSW index and get_memory_content_by_usearch_keys lookup**

## Performance

- **Duration:** 4 min
- **Started:** 2026-04-04T14:46:00Z
- **Completed:** 2026-04-04T14:49:37Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created `rust/src/memory/retrieve.rs` with `MemoryResult`, `DEFAULT_MEMORY_TOP_K=5`, and `build_system_with_memories` that mirrors `rag/context.rs` pattern
- Added `get_memory_content_by_usearch_keys` to persistence/queries.rs for resolving usearch HNSW keys to memory content
- Wired memory injection into `do_send_message` in lib.rs after RAG injection, with hoisted shared embedding
- Added 9 new unit tests (all passing), total test suite: 213 tests, 0 failures

## Task Commits

Each task was committed atomically:

1. **Task 1: Add memory retrieval module and persistence query with tests** - `4d95fc5` (feat)
2. **Task 2: Wire memory injection into do_send_message** - `ece621e` (feat)

**Plan metadata:** (docs commit to follow)

_Note: Task 1 used TDD - tests added first (RED), then implementation (GREEN). All tests passed on first implementation attempt._

## Files Created/Modified
- `rust/src/memory/retrieve.rs` - New module: MemoryResult struct, DEFAULT_MEMORY_TOP_K=5, build_system_with_memories function
- `rust/src/memory/mod.rs` - Added `pub mod retrieve;` declaration
- `rust/src/persistence/queries.rs` - Added get_memory_content_by_usearch_keys for usearch key -> content lookup
- `rust/src/tests/memory.rs` - Added 9 new unit tests for retrieve module and persistence query
- `rust/src/lib.rs` - Hoisted query embedding, renamed RAG result, added Phase 21 memory injection block

## Decisions Made
- Reused the shared usearch HNSW index for memory search: chunk keys and memory keys coexist; `get_memory_content_by_usearch_keys` silently returns empty for non-memory keys (chunk keys), so no filtering is needed
- Hoisted embedding computation before both the RAG block and memory block so `embed()` is called once per message regardless of whether docs are attached
- Mirrored `build_system_with_context` pattern exactly in `build_system_with_memories` for consistency

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Memory retrieval loop is complete: extraction (Phase 20) + injection (Phase 21) both wired
- Ready for Phase 22: Memory Management UI (view/delete/edit memories)
- The `<memories>` XML block will appear in system prompts when relevant memories exist

---
*Phase: 21-memory-retrieval-injection*
*Completed: 2026-04-04*
