# Phase 20: Memory Core - Research

**Researched:** 2026-04-03
**Domain:** Rust actor pattern, LLM-driven extraction, SQLite migration, usearch HNSW indexing
**Confidence:** HIGH

## Summary

Phase 20 builds a background memory extraction system that fires automatically after every
conversation turn completes. The app will call an LLM with a structured prompt asking it to
extract facts, preferences, and entities from the completed turn, persist each extracted memory
as a SQLite row (new `memories` table, Migration V15), and embed each memory text using the
existing `EmbeddingProvider` + `VectorIndex` infrastructure — storing the usearch key in the
SQLite row so Phase 21 can retrieve memory text by ANN key.

The codebase already has every primitive needed: `InternalEvent` for async-to-actor delivery,
`runtime.spawn` + `tokio::task::spawn_blocking` for background work, the `EmbeddingProvider`
trait, `VectorIndex::add/save`, `async-openai` for LLM calls, and `rusqlite` for persistence.
This phase is a pure addition — no existing code paths change except the `StreamDone` handler
which gains one non-blocking side-effect (spawning the extraction task).

**Primary recommendation:** Add a new `memory` module under `rust/src/memory/` with
`extract.rs` (LLM call logic), add Migration V15 (memories table), add
`InternalEvent::MemoryExtractionComplete`, and hook into `StreamDone` to spawn the background
task — exactly mirroring how `EmbeddingComplete` is used after `IngestDocument`.

---

## Phase Requirements

<phase_requirements>

| ID | Description | Research Support |
|----|-------------|------------------|
| MEM-01 | App automatically extracts facts, preferences, and entities from completed conversations | Extraction triggers inside `StreamDone` handler; LLM call via async-openai spawned on Tokio runtime |
| MEM-02 | Extracted memories are stored locally in SQLite with vector embeddings in usearch index | Migration V15 adds `memories` table with `usearch_key` column; VectorIndex::add writes the embedding |
| MEM-07 | Memory extraction runs in background without blocking chat flow | `runtime.spawn` returns immediately; `MemoryExtractionComplete` InternalEvent delivers results asynchronously |

</phase_requirements>

---

## Standard Stack

All libraries are already in `Cargo.toml` — Phase 20 adds zero new dependencies.

### Core (already present)

| Library | Version (Cargo.toml) | Role in Phase 20 |
|---------|---------------------|-----------------|
| `async-openai` | 0.33.1 | LLM call for memory extraction (same client, same backend) |
| `rusqlite` (bundled) | 0.39 | Migration V15, CRUD for memories table |
| `usearch` | 2.24.0 | Embed extracted memory text into HNSW index |
| `flume` | 0.11 | Deliver `MemoryExtractionComplete` back to actor loop |
| `tokio` | 1.x | `runtime.spawn` for background extraction task |
| `serde_json` | 1.x | Parse LLM JSON response listing extracted memories |
| `uuid` | 1.x | Generate stable IDs for each memory row |

**No new Cargo.toml entries needed.** Confidence: HIGH (verified against `rust/Cargo.toml`).

---

## Architecture Patterns

### Recommended Project Structure Addition

```
rust/src/
├── memory/
│   ├── mod.rs          # pub mod extract; pub use ...
│   └── extract.rs      # build_extraction_prompt(), call_extraction_llm()
├── persistence/
│   └── schema.rs       # +MIGRATION_V15 (memories table)
│   └── queries.rs      # +insert_memory, list_memories, delete_memory, MemoryRow
└── lib.rs              # +InternalEvent::MemoryExtractionComplete
                        # +StreamDone: spawn extraction task
                        # +ActorState: no new fields needed (reuses vector_index)
```

### Pattern 1: InternalEvent Round-Trip (the canonical async-to-actor bridge)

Every background result in this codebase follows the same pattern: the actor thread spawns a
Tokio task, the task does async work, then sends an `InternalEvent` via `flume::Sender<CoreMsg>`
back to the actor loop. The actor loop handles the event synchronously, writes to SQLite, and
mutates `AppState`.

**Existing examples to copy:**
- `EmbeddingComplete` — spawned from `IngestDocument` handler; delivers chunk embeddings
- `AgentStepComplete` — spawned from agent runner; delivers step results
- `HealthCheckResult` — spawned from health probe; delivers success/failure

**Memory extraction follows the same structure:**

```rust
// In StreamDone handler (rust/src/lib.rs):
// After assistant message is persisted, spawn extraction if messages exist
if let Some(conv_id) = &actor_state.app_state.current_conversation_id {
    let messages_snapshot: Vec<(String, String)> = actor_state
        .app_state
        .messages
        .iter()
        .map(|m| (m.role.clone(), m.content.clone()))
        .collect();

    if messages_snapshot.len() >= 2 {
        let backend = find_active_backend(&actor_state);  // clone for task
        let core_tx_clone = core_tx.clone();
        let conv_id_clone = conv_id.clone();

        actor_state.runtime.spawn(async move {
            let memories = memory::extract::call_extraction_llm(
                &backend,
                &messages_snapshot,
            ).await.unwrap_or_default();

            let _ = core_tx_clone.send(CoreMsg::InternalEvent(Box::new(
                llm::InternalEvent::MemoryExtractionComplete {
                    conversation_id: conv_id_clone,
                    memories,
                },
            )));
        });
    }
}
```

**Source:** Derived from `IngestDocument` handler pattern at `rust/src/lib.rs:3356-3369`.

### Pattern 2: LLM Extraction Prompt

The extraction LLM call uses the same `async-openai` client already used for chat. The key
design decision: call the **same backend that just completed the conversation** (already in
`actor_state.current_streaming_backend_id` or fallback to active backend). This avoids any
new backend-selection logic.

Use a structured system prompt + user message listing the conversation. Request JSON output
(not tool calls — simpler for structured extraction):

```rust
// In memory/extract.rs
const EXTRACTION_SYSTEM: &str = r#"You are a memory extraction assistant.
Extract facts, preferences, and entities from the conversation.
Respond with a JSON array of strings. Each string is one memory fact.
Be concise. Only extract information the user stated or clearly implied.
If nothing is worth remembering, respond with an empty array: []
Example: ["User prefers dark mode", "User's name is Alex", "User works at Acme Corp"]"#;

pub async fn call_extraction_llm(
    backend: &llm::BackendConfig,
    messages: &[(String, String)],  // (role, content) pairs
    model: &str,
) -> anyhow::Result<Vec<String>> {
    // Build conversation transcript for the user message
    let transcript = messages.iter()
        .map(|(role, content)| format!("{}: {}", role, content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let config = async_openai::config::OpenAIConfig::new()
        .with_api_base(&backend.base_url)
        .with_api_key(&backend.api_key);
    let client = async_openai::Client::with_config(config);

    let request = async_openai::types::CreateChatCompletionRequestArgs::default()
        .model(model)
        .max_tokens(512u16)
        .messages(vec![
            async_openai::types::ChatCompletionRequestSystemMessageArgs::default()
                .content(EXTRACTION_SYSTEM)
                .build()?,
            async_openai::types::ChatCompletionRequestUserMessageArgs::default()
                .content(format!("Extract memories from:\n\n{}", transcript))
                .build()?,
        ])
        .build()?;

    let response = client.chat().create(request).await?;
    let text = response.choices.first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    // Parse JSON array; fall back to empty on parse failure
    let memories: Vec<String> = serde_json::from_str(text.trim())
        .unwrap_or_default();
    Ok(memories)
}
```

**Source:** async-openai docs (confirmed from CLAUDE.md high-confidence sources). Pattern
mirrors existing `spawn_streaming_task` but uses non-streaming `create()`.

### Pattern 3: Migration V15 — Memories Table

Follow the exact pattern of all 14 existing migrations in `rust/src/persistence/schema.rs`.

Key design decisions:
- `id` is TEXT PRIMARY KEY (UUID) — consistent with conversations, documents, agent_sessions
- `usearch_key` is INTEGER — the key used to add the embedding to VectorIndex (like chunk rowid)
  BUT memories don't use AUTOINCREMENT because we generate the key ourselves (a random u64).
  **Decision:** use `CAST(ABS(RANDOM()) AS INTEGER)` at insert time, OR generate in Rust using
  `uuid::Uuid::new_v4().as_u128() as u64` and store separately. The latter is cleaner.
- `conversation_id` TEXT — links memory to the source conversation for audit/display
- `content` TEXT — the extracted memory string
- `created_at` INTEGER — Unix milliseconds (consistent with all other tables)

```sql
-- MIGRATION_V15
CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    content         TEXT NOT NULL,
    usearch_key     INTEGER NOT NULL UNIQUE,
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memories_conversation
    ON memories(conversation_id);
```

**Source:** Pattern from `MIGRATION_V6` (chunks table design) and `MIGRATION_V1` (primary
key conventions). Confidence: HIGH.

### Pattern 4: MemoryExtractionComplete Handler

After the task delivers `MemoryExtractionComplete`, the actor loop:
1. For each extracted memory string: generate UUID id, generate u64 usearch_key
2. Insert into `memories` table via `insert_memory()`
3. Embed the memory text via `spawn_blocking(|| embedding_provider.embed(...))`
4. Add to `vector_index` via `VectorIndex::add(usearch_key, &embedding)`
5. Save the vector index via `VectorIndex::save()`

**Important:** The embedding step inside `MemoryExtractionComplete` handler must also be
async (use another `spawn_blocking` nested inside a `runtime.spawn`), or it can be done
synchronously if the embedding provider is fast (NullEmbeddingProvider is instant; real
providers take 10-50ms per text which is acceptable on the actor thread for 1-5 memories).

**Recommendation:** Do the embedding synchronously in the `MemoryExtractionComplete` handler
on the actor thread (same pattern as RAG query embedding in `do_send_message`). Memory
extraction produces at most ~10 short strings; at 10ms each = 100ms max, well within
acceptable range. Avoids a second round-trip through InternalEvent.

### Pattern 5: usearch Key Management for Memories

The RAG pipeline uses SQLite `INTEGER PRIMARY KEY AUTOINCREMENT` as the usearch key (chunk
rowid). Memories need a different approach since:
- Multiple memories are inserted per extraction
- We can't use AUTOINCREMENT for rowid AND store it in `usearch_key` column simultaneously
  (would need to insert, then get last_insert_rowid, then update — awkward)

**Recommended approach:** Generate a random u64 in Rust before inserting:
```rust
// In Rust, before insert_memory():
let usearch_key: u64 = uuid::Uuid::new_v4().as_u128() as u64;
// Store in memories.usearch_key, use as VectorIndex key
```

This is collision-safe for any realistic number of memories (birthday problem: need ~4 billion
memories for 50% collision probability with 64-bit random keys).

**Source:** Derived from usearch key design (CLAUDE.md HIGH confidence, usearch docs confirm
u64 keys).

### Anti-Patterns to Avoid

- **Blocking the actor thread with LLM calls:** Never call async-openai synchronously on the
  actor thread. Always use `runtime.spawn(async move { ... })`.
- **Saving VectorIndex on every memory insert:** Call `vector_index.save()` once after all
  memories from one extraction batch are added, not once per memory.
- **Ignoring extraction failures:** If the LLM returns unparseable JSON or an error, silently
  skip (log a warning, don't propagate to UI). Memory extraction is best-effort.
- **Extracting from trivial conversations:** Add a guard — skip extraction if the conversation
  has fewer than 2 messages or fewer than ~50 total characters. Prevents wasted LLM calls on
  one-liner exchanges.
- **Storing raw embedding in SQLite:** Do NOT store the f32 embedding in the `memories` table.
  The embedding lives in the usearch index on disk; the `usearch_key` links them. (Same design
  as RAG chunks — no `embedding` column in `chunks` table.)

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| LLM extraction client | Custom HTTP client for extraction | `async-openai` (already in use) | Same client, same config, same auth |
| ANN vector store for memories | Separate usearch index | Existing `VectorIndex` in `ActorState` | One index, one file, Phase 21 reads from it |
| Background task delivery | Custom channel | `flume::Sender<CoreMsg>` + `InternalEvent` | Already the established actor bridge pattern |
| Text embedding | Direct ONNX calls | `EmbeddingProvider` trait (already registered) | Handles platform differences, already initialized |
| Random key generation | Counter or timestamp | `uuid::Uuid::new_v4().as_u128() as u64` | Collision-free, already using uuid crate |

---

## Common Pitfalls

### Pitfall 1: rusqlite::Connection is not Send

**What goes wrong:** Attempting to pass `actor_state.db.conn()` into a `runtime.spawn(async move {...})` task. The compiler will reject it — `rusqlite::Connection` is `!Send`.

**Why it happens:** The extraction task runs on a Tokio thread, but the database lives on the actor thread.

**How to avoid:** All SQLite writes happen in the `MemoryExtractionComplete` handler on the actor thread, not inside the spawned task. The task only does the async LLM call and sends results back via `InternalEvent`.

**Warning signs:** Compiler error "cannot be sent between threads safely" on `conn`.

### Pitfall 2: LLM Returns Non-JSON or Partial JSON

**What goes wrong:** The extraction LLM occasionally returns prose instead of `["...", "..."]`, especially with smaller models or when the system prompt is partially ignored.

**Why it happens:** Not all models strictly follow JSON-output instructions, and no streaming is used (non-streaming `create()` still has model variation).

**How to avoid:** Wrap `serde_json::from_str()` in `.unwrap_or_default()`. Add a fallback: if parsing fails, try to extract quoted strings with a simple regex or just return empty vec.

**Warning signs:** Tests passing but memories table always empty in manual testing.

### Pitfall 3: VectorIndex Key Collision Between Memories and Chunks

**What goes wrong:** If memory usearch keys happen to collide with chunk rowids in the same VectorIndex, Phase 21 semantic search returns mixed results (memories + document chunks).

**Why it happens:** Chunk rowids are small sequential integers (1, 2, 3...); random u64 memory keys are large (~2^63 range). With random u64 generation this is astronomically unlikely, but a naive counter starting from 1 would collide immediately.

**How to avoid:** Use `uuid::Uuid::new_v4().as_u128() as u64` for memory keys. The high-bit range ensures no overlap with SQLite autoincrement rowids which are bounded by i64 max (2^63-1), but practically max out in the thousands/millions.

**Warning signs:** Phase 21 search returns document chunk text when querying for memories.

### Pitfall 4: Extraction Fires on Every StreamDone Including Agent Steps

**What goes wrong:** Agent step completions also emit `StreamDone`-equivalent events. If the extraction hook is placed inside the generic streaming done path, agent intermediate steps trigger spurious memory extraction.

**Why it happens:** The actor loop handles agent completions via `AgentStepComplete`, not `StreamDone`. But adding extraction to `StreamDone` is safe as long as we verify `current_conversation_id` is set (agent sessions have session IDs, not conversation IDs).

**How to avoid:** In the `StreamDone` handler, check that `current_conversation_id.is_some()` before spawning extraction. Agent step streaming resolves through `AgentStepComplete`, not `StreamDone`, so this is naturally isolated.

**Warning signs:** Memories being inserted with empty or "agent" conversation IDs.

### Pitfall 5: Extraction Uses Wrong Backend or Wrong Model

**What goes wrong:** The backend used for extraction may not be the same as the one that just completed the conversation (especially if failover occurred). Also, the model stored on the conversation might not be the best extraction model.

**How to avoid:** Use the `current_streaming_backend_id` captured at `StreamDone` time — it's the backend that actually completed the response. The same model used for chat is fine for extraction (it already understands the context). Clone the full `BackendConfig` before spawning.

---

## Code Examples

### New InternalEvent Variant

```rust
// In rust/src/llm/streaming.rs, inside pub enum InternalEvent:
/// Memory extraction completed for a conversation turn (Phase 20, MEM-01, MEM-07).
///
/// Delivered from the Tokio runtime.spawn extraction task back to the actor loop.
/// The actor inserts memories into SQLite and adds embeddings to the vector index.
MemoryExtractionComplete {
    conversation_id: String,
    /// Each string is one extracted memory fact. Empty vec means nothing to store.
    memories: Vec<String>,
},
```

### New MemoryRow and insert_memory Query

```rust
// In rust/src/persistence/queries.rs

/// A row from the `memories` table.
#[derive(Debug, Clone)]
pub struct MemoryRow {
    pub id: String,
    pub conversation_id: String,
    pub content: String,
    pub usearch_key: i64,  // stored as i64 in SQLite (INTEGER), reinterpreted as u64 for usearch
    pub created_at: i64,
}

pub fn insert_memory(
    conn: &Connection,
    row: &MemoryRow,
) -> Result<(), PersistenceError> {
    conn.prepare_cached(
        "INSERT INTO memories (id, conversation_id, content, usearch_key, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(rusqlite::params![
        row.id,
        row.conversation_id,
        row.content,
        row.usearch_key,
        row.created_at,
    ])?;
    Ok(())
}

pub fn list_memories(conn: &Connection) -> Result<Vec<MemoryRow>, PersistenceError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, conversation_id, content, usearch_key, created_at
         FROM memories ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(MemoryRow {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                content: row.get(2)?,
                usearch_key: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

### MemoryExtractionComplete Handler Sketch

```rust
// In rust/src/lib.rs, inside the InternalEvent match arm:
llm::InternalEvent::MemoryExtractionComplete { conversation_id, memories } => {
    if memories.is_empty() {
        // Nothing extracted — no state change needed
    } else {
        let now = now_secs();
        for content in &memories {
            let id = new_uuid();
            // Generate collision-safe usearch key from UUID bits
            let usearch_key = uuid::Uuid::new_v4().as_u128() as i64;

            let row = persistence::queries::MemoryRow {
                id: id.clone(),
                conversation_id: conversation_id.clone(),
                content: content.clone(),
                usearch_key,
                created_at: now,
            };
            if persistence::queries::insert_memory(actor_state.db.conn(), &row).is_ok() {
                // Embed and add to vector index
                let embedding = actor_state.embedding_provider.embed(vec![content.clone()]);
                if embedding.len() == crate::embedding::EMBEDDING_DIM {
                    let _ = actor_state.vector_index.add(usearch_key as u64, &embedding);
                }
            }
        }
        // Save index once after all memories are added
        let _ = actor_state.vector_index.save();
        log::info!(target: "memory", "[memory] extracted {} memories from conv={}", memories.len(), conversation_id);
        // No UI state change needed for Phase 20 (Phase 23 adds memory list UI)
    }
    // No rev increment needed -- no visible AppState change in Phase 20
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Mem0 Python library | Local Rust extraction with LLM prompt | Phase 20 decision | Full privacy, no external service |
| Separate vector store per entity type | Single usearch HNSW index for all embedded text | Phase 8 decision | Simpler ops, Phase 21 searches everything |

---

## Open Questions

1. **Minimum conversation length threshold for extraction**
   - What we know: extraction on every turn wastes LLM tokens for trivial exchanges
   - What's unclear: exact threshold (2 messages? 100 chars? 1 user+1 assistant turn?)
   - Recommendation: gate on `messages.len() >= 2` AND total content chars > 100; planner can tune

2. **Which model for extraction**
   - What we know: extraction uses the same backend as the conversation
   - What's unclear: whether to use the conversation model or a smaller/cheaper model
   - Recommendation: use the same model for simplicity; extraction prompt is small, latency < 2s

3. **Extraction frequency: every turn vs. conversation close**
   - What we know: requirements say "after a conversation ends" (MEM-01, success criteria 1)
   - What's unclear: "conversation ends" = user navigates away? explicit close? stream done?
   - Recommendation: trigger on `StreamDone` (each assistant turn), which naturally fires when
     the user's "conversation session" produces a complete turn; simpler than detecting navigation

4. **VectorIndex key space collision (memories vs. chunks)**
   - What we know: chunks use small sequential i64 rowids; random u64 for memories is safe
   - What's unclear: whether Phase 21 needs to distinguish memory vs. chunk results from search
   - Recommendation: add a `source` column in Phase 21 search or keep separate indices;
     Phase 20 only needs to store — defer the query-time distinction to Phase 21 research

---

## Environment Availability

All dependencies are already in `Cargo.toml`. No new external tools required.

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All compilation | Yes | rustc 1.93.0 | — |
| `cargo` | Build | Yes | bundled | — |
| `async-openai` | LLM extraction call | Yes (Cargo.toml) | 0.33.1 | — |
| `rusqlite` | Migration V15, memory CRUD | Yes (Cargo.toml) | 0.39 | — |
| `usearch` | Vector embedding storage | Yes (Cargo.toml) | 2.24.0 | — |

**No missing dependencies.** Phase 20 is purely additive Rust code using the existing stack.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`cargo test`) |
| Config file | none — Cargo.toml `[[test]]` implicit |
| Quick run command | `cargo test -p confidential_app_core memory -- --nocapture` |
| Full suite command | `cargo test -p confidential_app_core` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MEM-01 | Memories are extracted after stream done | Integration | `cargo test -p confidential_app_core memory::test_extraction_fires_after_stream_done` | No — Wave 0 |
| MEM-02 | Memories appear in SQLite + usearch after extraction | Unit | `cargo test -p confidential_app_core memory::test_memory_persisted_with_usearch_key` | No — Wave 0 |
| MEM-02 | Migration V15 creates memories table | Unit | `cargo test -p confidential_app_core memory::test_migration_v15_creates_memories_table` | No — Wave 0 |
| MEM-07 | Extraction does not block StreamDone → UI unblocked | Integration | `cargo test -p confidential_app_core memory::test_extraction_non_blocking` | No — Wave 0 |
| MEM-02 | Memories survive restart (usearch load + SQLite persist) | Integration | `cargo test -p confidential_app_core memory::test_memories_survive_restart` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p confidential_app_core memory`
- **Per wave merge:** `cargo test -p confidential_app_core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `rust/src/tests/memory.rs` — covers MEM-01, MEM-02, MEM-07
- [ ] `rust/src/memory/mod.rs` — new module file
- [ ] `rust/src/memory/extract.rs` — extraction logic

---

## Project Constraints (from CLAUDE.md)

The following directives from `CLAUDE.md` apply directly to this phase and the planner MUST honor them:

| Directive | Impact on Phase 20 |
|-----------|-------------------|
| Rust core owns all business logic; native layers are thin UI bridges | Memory module lives entirely in `rust/src/memory/`; no Swift/Kotlin logic |
| All document storage and vector indices must remain on-device | Memories stored in SQLite + local usearch — no cloud calls for storage |
| All backend integrations must use OpenAI-compatible chat completions API | Extraction LLM call uses `async-openai` with `create()` (non-streaming) |
| No `native-tls` / OpenSSL; use `rustls-tls` | `async-openai` already uses `reqwest` with `rustls-tls` — no new HTTP code needed |
| `rusqlite` with `bundled` feature; do not mix with `sqlx` | Migration V15 follows same pattern as V1–V14; no sqlx |
| Actor model: `rusqlite::Connection` is `!Send`, must stay on actor thread | All SQLite writes in `MemoryExtractionComplete` handler, not in spawned task |
| GSD workflow enforcement: use `/gsd:execute-phase` entry point | Must not make direct repo edits outside GSD workflow |
| No telemetry, no cloud sync in v1 | Extracted memories never leave device |

---

## Sources

### Primary (HIGH confidence)
- `rust/src/lib.rs` (read directly) — actor pattern, `StreamDone` handler, `IngestDocument`/`EmbeddingComplete` round-trip pattern, `ActorState` structure
- `rust/src/llm/streaming.rs` (read directly) — `InternalEvent` enum, all existing variants
- `rust/src/persistence/schema.rs` (read directly) — all 14 migrations, table conventions, key design decisions
- `rust/src/persistence/queries.rs` (read directly) — row types, query functions, rusqlite patterns
- `rust/src/embedding/mod.rs` (read directly) — `EmbeddingProvider` trait, `EMBEDDING_DIM`
- `rust/src/rag/index.rs` (read directly) — `VectorIndex::add/search/remove/save`, key semantics
- `CLAUDE.md` (read directly) — technology stack, version pins, architectural constraints
- `.planning/REQUIREMENTS.md` (read directly) — MEM-01, MEM-02, MEM-07 definitions
- `.planning/ROADMAP.md` (read directly) — Phase 20 success criteria

### Secondary (MEDIUM confidence)
- async-openai non-streaming `create()` — from CLAUDE.md source list citing docs.rs/async-openai 0.33.1

### Tertiary (LOW confidence)
- None — all findings verified against actual codebase or CLAUDE.md HIGH-confidence sources

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified against Cargo.toml; all libraries already present
- Architecture: HIGH — patterns extracted directly from existing working code in lib.rs
- Migration design: HIGH — follows established patterns from 14 existing migrations
- Pitfalls: HIGH — derived from actual `!Send` constraints and existing error-handling patterns
- LLM extraction prompt: MEDIUM — prompt design is project-specific; success depends on model behavior in testing

**Research date:** 2026-04-03
**Valid until:** 2026-05-03 (stable codebase; no fast-moving dependencies added)
