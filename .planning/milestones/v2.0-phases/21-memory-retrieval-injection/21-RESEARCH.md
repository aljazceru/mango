# Phase 21: Memory Retrieval & Injection - Research

**Researched:** 2026-04-04
**Domain:** Rust actor pattern, semantic memory search, system prompt injection, usearch ANN
**Confidence:** HIGH

## Summary

Phase 21 completes the memory loop by wiring the stored memories (written in Phase 20) into
new conversations. When a user sends a message, the actor embeds the message text, queries the
shared usearch HNSW index, resolves matching memory keys back to their content via SQLite, and
prepends a `<memories>` block to the system prompt — before or alongside the existing RAG
`<context>` block.

The codebase already has every primitive needed. The injection point is `do_send_message` in
`rust/src/lib.rs` (around line 1273) where RAG context is already injected. Memory injection
follows the exact same pattern: embed query, call `vector_index.search()`, resolve keys to
content via a new `get_memory_content_by_usearch_keys()` persistence query, call a new
`build_system_with_memories()` helper, and combine with any RAG context.

The key engineering challenge is **key disambiguation**: Phase 20 stores memories with large
random u64 keys (cast from UUID bits), and Phase 8 RAG stores chunks with small sequential i64
rowids. When `vector_index.search()` returns keys, the handler must determine which keys belong
to memories vs. document chunks. The recommended approach is a separate SQLite lookup:
search all returned keys against `memories.usearch_key` — hits are memories, misses are chunks.

**Primary recommendation:** Add `get_memory_content_by_usearch_keys()` to persistence queries,
add a `build_system_with_memories()` helper to `rust/src/memory/`, and extend `do_send_message`
to inject memories after RAG context — all without breaking the existing RAG injection path.

---

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MEM-03 | Relevant memories are injected into new conversation system prompts via semantic search | Inject at `do_send_message` using same embedding + usearch pathway as RAG; new `get_memory_content_by_usearch_keys` query resolves content; `build_system_with_memories` adds `<memories>` block |

</phase_requirements>

---

## Standard Stack

All libraries are already in `Cargo.toml`. Phase 21 adds zero new dependencies.

### Core (already present)

| Library | Version (Cargo.toml) | Role in Phase 21 |
|---------|---------------------|-----------------|
| `usearch` | 2.24.0 | `vector_index.search()` for ANN memory lookup |
| `rusqlite` (bundled) | 0.39 | New `get_memory_content_by_usearch_keys()` query |
| `serde_json` | 1.x | Not needed (content is plain text) |
| `tokio` | 1.x | No new async work — injection is synchronous on actor thread |

**No new Cargo.toml entries needed.** Confidence: HIGH (verified against `rust/Cargo.toml` via Phase 20 research).

---

## Architecture Patterns

### Injection Point in do_send_message

The memory injection slots into `do_send_message` in `rust/src/lib.rs` at approximately line
1273, right after the RAG context injection block and before the system prompt is pushed to
`chat_messages`. The RAG block already computes `system_prompt` from `base_system_prompt`.
Memory injection further augments that result.

The RAG flow today is:

```
base_system_prompt (from conversation or global default)
  ↓ RAG block (if docs attached)
system_prompt (= base_system_prompt with optional <context> prepended)
  ↓ pushed to chat_messages
```

After Phase 21 the flow becomes:

```
base_system_prompt
  ↓ RAG block (if docs attached)  → system_prompt_after_rag
  ↓ Memory block (always runs)    → system_prompt_final
  ↓ pushed to chat_messages
```

Memory injection runs unconditionally (no "attached memories" gate) — it silently returns the
unchanged prompt if no relevant memories exist.

### Recommended Project Structure Additions

```
rust/src/
├── memory/
│   ├── mod.rs          # + pub mod retrieve;
│   └── retrieve.rs     # build_system_with_memories(), DEFAULT_MEMORY_TOP_K
├── persistence/
│   └── queries.rs      # + get_memory_content_by_usearch_keys()
└── lib.rs              # do_send_message: memory injection block
```

No new `InternalEvent` variant is needed — memory search is synchronous (same as RAG query
embedding in `do_send_message`, not a background task).

### Pattern 1: Memory Retrieval Query (mirrors get_chunk_text_by_rowids)

The existing `get_chunk_text_by_rowids` function in `persistence/queries.rs` (line 685)
demonstrates the lookup pattern. The memory equivalent is:

```rust
/// Return the content of memories whose usearch_key is in `keys`.
///
/// Returns Vec<(usearch_key_as_i64, content)> pairs.
/// Missing keys are silently omitted (memory may have been deleted after indexing).
pub fn get_memory_content_by_usearch_keys(
    conn: &Connection,
    keys: &[i64],
) -> Result<Vec<(i64, String)>, PersistenceError> {
    if keys.is_empty() {
        return Ok(vec![]);
    }
    let placeholders: String = keys
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT usearch_key, content FROM memories WHERE usearch_key IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = keys
        .iter()
        .map(|k| k as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

**Source:** Derived directly from `get_chunk_text_by_rowids` at `rust/src/persistence/queries.rs:685`. HIGH confidence.

### Pattern 2: Build System with Memories (mirrors build_system_with_context)

Add to `rust/src/memory/retrieve.rs`:

```rust
/// Default number of top-k memories to retrieve for injection.
pub const DEFAULT_MEMORY_TOP_K: usize = 5;

/// A retrieved memory with its similarity score.
#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub content: String,
    pub score: f32,
}

/// Prepend a <memories> block to `current_system` if relevant memories exist.
///
/// If `memories` is empty, returns `current_system` unchanged (no injection artifact).
/// Otherwise prepends:
///
/// ```text
/// <memories>
/// [1] first memory content
///
/// [2] second memory content
///
/// </memories>
///
/// {current_system}
/// ```
pub fn build_system_with_memories(current_system: &str, memories: &[MemoryResult]) -> String {
    if memories.is_empty() {
        return current_system.to_owned();
    }
    let mut out = String::from("<memories>\n");
    for (i, mem) in memories.iter().enumerate() {
        out.push_str(&format!("[{}] {}\n\n", i + 1, mem.content));
    }
    out.push_str("</memories>\n\n");
    out.push_str(current_system);
    out
}
```

**Source:** Derived from `build_system_with_context` at `rust/src/rag/context.rs:35`. HIGH confidence.

### Pattern 3: Key Disambiguation (memories vs. chunks in shared index)

The shared `vector_index` stores both RAG chunks (small sequential i64 rowids, max in
thousands/millions) and memory keys (large random u64 values cast from UUID bits — typically
near u64::MAX/2). When `vector_index.search()` returns results, the returned keys are raw u64.

**Disambiguation approach:** After `vector_index.search()`, cast keys to i64 and query
`get_memory_content_by_usearch_keys()`. Any keys that hit the memories table are memories; any
that miss are chunks (which were already handled by the RAG block, or are irrelevant if no docs
are attached).

This works because:
- Chunk rowids are SQLite AUTOINCREMENT i64, bounded by i64::MAX (~9.2e18), but practically in
  the low thousands/millions for any real corpus.
- Memory keys are `uuid::Uuid::new_v4().as_u128() as i64` — effectively random in the full i64
  range including negative values (high bit of u128 wraps to negative i64).
- The ranges are distinguishable in principle, but SQLite lookup is simpler and more robust than
  a range check.

**Implementation note:** The memory injection block in `do_send_message` runs a standalone
`vector_index.search()` call dedicated to memory retrieval — separate from the RAG block's
search. This avoids any need to split results and is cleaner.

```rust
// Phase 21: Memory injection block in do_send_message
// (runs after base_system_prompt is resolved, before system_prompt is finalized)
let system_prompt = {
    // Step 1: RAG injection (existing code, produces system_prompt_after_rag)
    let system_prompt_after_rag = /* ... existing RAG block ... */;

    // Step 2: Memory injection
    let query_emb = actor_state.embedding_provider.embed(vec![final_text.clone()]);
    let system_with_memories = if !query_emb.is_empty() {
        match actor_state.vector_index.search(&query_emb, memory::retrieve::DEFAULT_MEMORY_TOP_K) {
            Ok(results) => {
                let keys: Vec<i64> = results.iter().map(|(k, _)| *k as i64).collect();
                let memory_hits = persistence::queries::get_memory_content_by_usearch_keys(
                    actor_state.db.conn(),
                    &keys,
                ).unwrap_or_default();

                if !memory_hits.is_empty() {
                    let mem_results: Vec<memory::retrieve::MemoryResult> = memory_hits
                        .into_iter()
                        .zip(results.iter())
                        .map(|((_, content), (_, score))| memory::retrieve::MemoryResult {
                            content,
                            score: *score,
                        })
                        .collect();
                    memory::retrieve::build_system_with_memories(&system_prompt_after_rag, &mem_results)
                } else {
                    system_prompt_after_rag
                }
            }
            Err(_) => system_prompt_after_rag,
        }
    } else {
        system_prompt_after_rag
    };

    system_with_memories
};
```

**Source:** Derived from existing RAG block at `rust/src/lib.rs:1277–1329`. HIGH confidence.

### Pattern 4: Handling Both RAG and Memory Together

When both RAG chunks and memories are relevant, the final system prompt structure is:

```
<memories>
[1] memory content
[2] memory content
</memories>

<context>
[1] document chunk
[2] document chunk
</context>

base_system_prompt
```

This is achieved naturally by the two-step injection: RAG runs first and wraps `base_system_prompt`
in a `<context>` block; then memory injection wraps that result in a `<memories>` block. The LLM
sees memories outermost (first) and RAG context innermost (second), both before the base prompt.

The ordering `<memories>` before `<context>` is intentional — memories are about the user, while
document context is about the current task. Placing user context first is a common RAG+memory
pattern.

### Pattern 5: Embedding Reuse

The embedding call in the memory block (`actor_state.embedding_provider.embed(vec![final_text.clone()])`)
is identical to the RAG block's embedding call. If both RAG and memory injection are active, the
embed is called twice for the same text.

**Optimization (optional):** Compute the query embedding once before both blocks. The RAG block
currently embeds inside its `if !attached_docs.is_empty()` guard; the memory block always
embeds. To avoid double-embedding when docs are attached, refactor to compute `query_emb` once
before the RAG block and reuse it in both.

**Recommendation for Phase 21:** Compute `query_emb` once before both blocks. This is a simple
refactor (hoist the `embed()` call above the RAG if-block) and avoids wasted inference time
when both RAG and memories are active.

### Anti-Patterns to Avoid

- **Separate usearch index for memories:** Do NOT create a second `VectorIndex` for memories.
  The shared index already holds memory keys from Phase 20. A separate index would double the
  embedding storage and require loading two files.

- **Searching inside a new InternalEvent:** Memory search is fast (< 5ms for HNSW on a
  consumer device with thousands of entries). Do it synchronously on the actor thread, same as
  RAG query embedding. No need for a background task or new InternalEvent.

- **Filtering by negative/large keys to find memories:** The key ranges overlap in theory
  (uuid-based i64 can be negative). Always use SQLite lookup for disambiguation, not key range.

- **Injecting memories on every message in an existing conversation:** The success criteria
  say "when a new conversation starts" — but in practice `do_send_message` runs for every
  user turn. Memory injection on every turn is correct behavior and is consistent with how RAG
  injection works. The success criteria description is slightly misleading; the planner should
  inject on every `do_send_message` call, not only the first.

- **Blocking on empty index:** If `vector_index.size() == 0`, the search returns an empty
  result immediately — no special guard needed. The `build_system_with_memories` function
  already handles empty results by returning the prompt unchanged.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Memory key → content lookup | Custom cache or in-memory map | `get_memory_content_by_usearch_keys()` SQL query | SQLite is the source of truth; in-memory map goes stale |
| Second vector store for memories | New `VectorIndex` instance | Existing shared `actor_state.vector_index` | Phase 20 already writes there; one file, one index |
| Similarity threshold filtering | Custom score comparison logic | Return top-N results, let LLM decide relevance | Threshold tuning is fragile; top-5 with `build_system_with_memories` is sufficient |
| Memory system prompt format | Custom XML/JSON template | `build_system_with_memories()` matching `build_system_with_context()` style | Consistent format; LLM already trained on similar XML context blocks |

---

## Common Pitfalls

### Pitfall 1: Embedding Called Twice When RAG + Memory Both Active

**What goes wrong:** When a conversation has attached documents AND memories exist, the query
embedding is computed twice — once for RAG search, once for memory search.

**Why it happens:** The RAG block embeds inside `if !attached_docs.is_empty()`, and the memory
block embeds unconditionally. Two separate `embed()` calls on the actor thread.

**How to avoid:** Hoist `let query_emb = actor_state.embedding_provider.embed(vec![final_text.clone()])` 
before both blocks. Pass it into both the RAG block and the memory block. The `NullEmbeddingProvider`
makes this zero-cost in test mode; real providers save 5–50ms.

**Warning signs:** Test log shows `[embed]` twice per `SendMessage` call.

### Pitfall 2: Memory Keys Returned by Search Include Stale Keys

**What goes wrong:** A memory is deleted from SQLite (via `delete_memory`) but its embedding
key remains in the usearch index (Phase 23 will handle deletion from index). After deletion,
`vector_index.search()` may return keys that no longer exist in the memories table.

**Why it happens:** Phase 20 has no memory deletion path. Phase 23 adds it. In Phase 21,
only insertion exists.

**How to avoid:** `get_memory_content_by_usearch_keys()` naturally handles missing keys —
it returns only rows that exist. The `memory_hits` result will simply be shorter than the
search results list. No explicit guard needed.

**Warning signs:** Result count from `vector_index.search()` is higher than `memory_hits.len()`.

### Pitfall 3: Search Returns Chunk Keys When No Memories Exist Yet

**What goes wrong:** On a fresh install with documents indexed but no memories yet,
`vector_index.search()` returns chunk keys. `get_memory_content_by_usearch_keys()` gets
chunk rowids (small integers like 1, 2, 3) and correctly finds 0 matches in the memories
table. The prompt is returned unchanged — correct behavior.

**Why it happens:** The shared index holds both chunk and memory keys.

**How to avoid:** No action needed. The SQLite lookup naturally filters to memory keys only.

**Warning signs:** None — this is correct behavior and requires no special handling.

### Pitfall 4: Memory Injection Artifacts When No Memories Exist

**What goes wrong:** An empty `<memories>` block is appended to the system prompt even when
there are no relevant memories. The LLM receives `<memories>\n</memories>\n\n` followed by the
base prompt.

**Why it happens:** `build_system_with_memories()` called with an empty slice.

**How to avoid:** `build_system_with_memories()` must check `memories.is_empty()` and return
`current_system` unchanged — mirroring `build_system_with_context()`. Include this check in the
success criteria and unit tests.

**Warning signs:** Tests show `<memories>` in the prompt when memories list is empty.

### Pitfall 5: do_send_message Refactor Breaks RetryLastMessage / EditMessage

**What goes wrong:** `do_send_message` is called by `SendMessage`, `RetryLastMessage`, and
`EditMessage`. Changes to the system prompt construction block affect all three paths.

**Why it happens:** All three call `do_send_message` with the same function signature.

**How to avoid:** The memory injection block is a pure addition — it does not remove or change
any existing code. Adding it before the final `if !system_prompt.is_empty()` guard is safe
for all callers. Test `RetryLastMessage` path in validation.

---

## Code Examples

### New: get_memory_content_by_usearch_keys

```rust
// Source: derived from rust/src/persistence/queries.rs:685 (get_chunk_text_by_rowids)
pub fn get_memory_content_by_usearch_keys(
    conn: &Connection,
    keys: &[i64],
) -> Result<Vec<(i64, String)>, PersistenceError> {
    if keys.is_empty() {
        return Ok(vec![]);
    }
    let placeholders: String = keys
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT usearch_key, content FROM memories WHERE usearch_key IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = keys
        .iter()
        .map(|k| k as &dyn rusqlite::types::ToSql)
        .collect();
    let rows = stmt
        .query_map(params.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

### New: memory/retrieve.rs

```rust
// Source: mirrors rust/src/rag/context.rs:build_system_with_context
pub const DEFAULT_MEMORY_TOP_K: usize = 5;

pub struct MemoryResult {
    pub content: String,
    pub score: f32,
}

pub fn build_system_with_memories(current_system: &str, memories: &[MemoryResult]) -> String {
    if memories.is_empty() {
        return current_system.to_owned();
    }
    let mut out = String::from("<memories>\n");
    for (i, mem) in memories.iter().enumerate() {
        out.push_str(&format!("[{}] {}\n\n", i + 1, mem.content));
    }
    out.push_str("</memories>\n\n");
    out.push_str(current_system);
    out
}
```

### Modified: do_send_message — query_emb hoisted, memory block added

```rust
// Source: rust/src/lib.rs:1282-1329 (existing RAG block, adapted)

// Hoist embedding computation (used by both RAG and memory blocks)
let query_emb = actor_state.embedding_provider.embed(vec![final_text.clone()]);

// Phase 8: RAG context injection (unchanged, but uses hoisted query_emb)
let mut rag_doc_count: Option<u32> = None;
let system_prompt_after_rag = if !actor_state.app_state.current_conversation_attached_docs.is_empty() {
    if !query_emb.is_empty() {
        match actor_state.vector_index.search(&query_emb, rag::DEFAULT_TOP_K) {
            Ok(results) => {
                // ... existing chunk lookup and build_system_with_context call ...
            }
            Err(_) => base_system_prompt,
        }
    } else {
        base_system_prompt
    }
} else {
    base_system_prompt
};

// Phase 21: Memory injection (MEM-03)
let system_prompt = if !query_emb.is_empty() {
    match actor_state.vector_index.search(&query_emb, memory::retrieve::DEFAULT_MEMORY_TOP_K) {
        Ok(results) => {
            let keys: Vec<i64> = results.iter().map(|(k, _)| *k as i64).collect();
            let memory_hits = persistence::queries::get_memory_content_by_usearch_keys(
                actor_state.db.conn(),
                &keys,
            ).unwrap_or_default();
            if !memory_hits.is_empty() {
                let mem_results: Vec<memory::retrieve::MemoryResult> = memory_hits
                    .into_iter()
                    .zip(results.iter())
                    .map(|((_, content), (_, score))| memory::retrieve::MemoryResult {
                        content,
                        score: *score,
                    })
                    .collect();
                memory::retrieve::build_system_with_memories(&system_prompt_after_rag, &mem_results)
            } else {
                system_prompt_after_rag
            }
        }
        Err(_) => system_prompt_after_rag,
    }
} else {
    system_prompt_after_rag
};
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-turn memory search (external service) | On-device usearch ANN search | Phase 20/21 design | Full privacy; no network call for retrieval |
| Separate memory index file | Shared `embeddings.usearch` with RAG chunks | Phase 20 design | One index file, simpler ops |
| Memory stored externally (Mem0, Zep) | Local SQLite + HNSW | Architecture decision | Data never leaves device |

**Deprecated/outdated:**
- Separate vector stores per entity type: replaced by shared usearch index (Phase 8 decision, documented in Phase 20 research)

---

## Open Questions

1. **Injection ordering: memories before or after RAG context?**
   - What we know: both `<memories>` and `<context>` blocks prepend to base system prompt
   - What's unclear: whether memories-first or context-first is better for LLM attention
   - Recommendation: memories-first (outermost), then RAG context, then base prompt. User
     facts are global context; document chunks are task-specific. Most LLMs attend to early
     tokens more heavily.

2. **Top-K for memory retrieval (DEFAULT_MEMORY_TOP_K)**
   - What we know: RAG uses DEFAULT_TOP_K = 4 (verified at `rag/context.rs:8`)
   - What's unclear: optimal N for memories; more memories = more context tokens used
   - Recommendation: 5 is a reasonable starting default; enough to be useful, small enough
     not to bloat the context. Planner can tune.

3. **Score threshold filtering**
   - What we know: usearch returns cosine distance (0 = identical, 2 = opposite); lower = more relevant
   - What's unclear: whether to filter results below a relevance threshold (e.g., skip if
     distance > 0.7) to avoid injecting irrelevant memories
   - Recommendation: start without threshold filtering; return top-N and let the LLM handle
     weak relevance. Add threshold in a follow-up if test results show noisy injection.

4. **Dead-code warnings for list_memories / delete_memory**
   - What we know: Phase 20 Plan 02 summary notes `list_memories` and `delete_memory` trigger
     dead-code warnings because nothing calls them yet
   - What's unclear: Phase 21 does not call `list_memories` (memory UI is Phase 23)
   - Recommendation: Phase 21 adds `get_memory_content_by_usearch_keys` (new function); the
     existing dead-code warnings remain until Phase 23. Do not add `#[allow(dead_code)]` — the
     warnings are informational and correct.

---

## Environment Availability

Step 2.6: SKIPPED (no external dependencies identified — Phase 21 is pure Rust additions using
already-present libraries and no new external tools).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | none — Cargo.toml `[[test]]` implicit |
| Quick run command | `cargo test -p confidential_app_core memory` |
| Full suite command | `cargo test -p confidential_app_core` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MEM-03 | Semantic search over memories returns relevant results | Unit | `cargo test -p confidential_app_core memory::test_memory_search_returns_relevant` | No — Wave 0 |
| MEM-03 | Top-N memories appear in system prompt | Unit | `cargo test -p confidential_app_core memory::test_build_system_with_memories_prepends_block` | No — Wave 0 |
| MEM-03 | Uses same injection pathway as RAG context | Unit | `cargo test -p confidential_app_core memory::test_memory_injection_uses_rag_pathway` | No — Wave 0 |
| MEM-03 | No injection when no relevant memories | Unit | `cargo test -p confidential_app_core memory::test_no_injection_when_empty_memories` | No — Wave 0 |
| MEM-03 | get_memory_content_by_usearch_keys returns correct rows | Unit | `cargo test -p confidential_app_core memory::test_get_memory_content_by_usearch_keys` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p confidential_app_core memory`
- **Per wave merge:** `cargo test -p confidential_app_core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `rust/src/memory/retrieve.rs` — new file: `DEFAULT_MEMORY_TOP_K`, `MemoryResult`, `build_system_with_memories`
- [ ] `rust/src/tests/memory.rs` — extend with retrieval and injection tests (file already exists from Phase 20)

---

## Project Constraints (from CLAUDE.md)

| Directive | Impact on Phase 21 |
|-----------|-------------------|
| Rust core owns all business logic; native layers are thin UI bridges | Memory retrieval logic lives entirely in `rust/src/memory/retrieve.rs`; no Swift/Kotlin logic |
| All document storage and vector indices must remain on-device | Memory search uses local usearch index; no network call for retrieval |
| No `native-tls` / OpenSSL | No new HTTP code; retrieval is local-only |
| `rusqlite` with `bundled` feature; do not mix with `sqlx` | New query `get_memory_content_by_usearch_keys` follows existing `rusqlite` patterns |
| Actor model: `rusqlite::Connection` is `!Send`, must stay on actor thread | Memory search is synchronous in `do_send_message` — no async spawn needed |
| GSD workflow enforcement: use `/gsd:execute-phase` entry point | Must not make direct repo edits outside GSD workflow |
| No telemetry, no cloud sync in v1 | Retrieved memories never leave device |

---

## Sources

### Primary (HIGH confidence)

- `rust/src/rag/context.rs` (read directly) — `build_system_with_context`, `ChunkResult`, `DEFAULT_TOP_K` — exact pattern to mirror for memory injection
- `rust/src/lib.rs:1261–1339` (read directly) — existing RAG injection block in `do_send_message`; exact location and logic for memory injection insertion
- `rust/src/persistence/queries.rs:685–709` (read directly) — `get_chunk_text_by_rowids` pattern for new memory lookup function
- `rust/src/persistence/queries.rs:806–864` (read directly) — existing `MemoryRow`, `insert_memory`, `list_memories`, `delete_memory`; confirmed `usearch_key` column structure
- `rust/src/rag/index.rs` (read directly) — `VectorIndex::search()` return type `Vec<(u64, f32)>`; cosine distance semantics
- `rust/src/memory/extract.rs` (read directly) — Phase 20 extraction function; `should_extract` guard
- `rust/src/memory/mod.rs` (read directly) — current module structure (`pub mod extract;` only)
- `.planning/phases/20-memory-core/20-02-SUMMARY.md` (read directly) — key decisions: `core_tx_for_thread`, usearch_key as `uuid::Uuid::new_v4().as_u128() as i64`, `continue;` in handler
- `CLAUDE.md` (read directly) — technology stack, version pins, architectural constraints
- `.planning/REQUIREMENTS.md` (read directly) — MEM-03 definition
- `.planning/ROADMAP.md` (read directly) — Phase 21 success criteria

### Secondary (MEDIUM confidence)

- None — all findings are derived directly from the existing codebase.

### Tertiary (LOW confidence)

- None — no WebSearch or unverified sources used. All claims verified against actual code.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in Cargo.toml; verified by reading Phase 20 research and codebase
- Architecture: HIGH — injection point identified by reading actual `do_send_message` source; patterns derived from working RAG code
- Persistence query: HIGH — derived directly from `get_chunk_text_by_rowids` (same author, same module)
- Pitfalls: HIGH — derived from actual code structure and Phase 20 decisions
- Top-K and ordering recommendations: MEDIUM — reasonable defaults, tunable in practice

**Research date:** 2026-04-04
**Valid until:** 2026-05-04 (stable codebase; no external dependencies added)
