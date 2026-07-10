# Phase 23: Memory Management UI + Agent UI - Research

**Researched:** 2026-04-04
**Domain:** Cross-platform UI (SwiftUI, Jetpack Compose, iced) + Rust core actor extensions
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** Memories displayed as a simple chronological list (newest first), each row showing a content preview (first ~100 chars) and the source conversation title if available
- **D-02:** Follow existing list patterns (ConversationListView on iOS, ChatScreen list on Android, home.rs list on desktop) for visual consistency
- **D-03:** Tap/click a memory row to expand or navigate to a detail view showing full content
- **D-04:** Edit is inline — user modifies text directly and saves with a confirm action; cancel discards changes
- **D-05:** Delete is available from the list view (swipe-to-delete on mobile, delete button on desktop) with confirmation
- **D-06:** AgentStepSummary must be extended with `tool_input: Option<String>` field (first ~200 chars of input payload) to satisfy AUI-02
- **D-07:** Agent session detail view shows each step as: step number, tool name, truncated input, truncated output/result, status badge
- **D-08:** Tool steps with type "final_answer" display the full answer text rather than tool name/input
- **D-09:** Add `Screen::Memories` variant to the Screen enum in lib.rs
- **D-10:** Memory and Agent entries both appear as top-level navigation items alongside Conversations, RAG, and Settings
- **D-11:** Re-enable Agent navigation by uncommenting/restoring the hidden agent nav entries on all platforms
- **D-12:** Add `update_memory(conn, memory_id, new_content)` query in persistence/queries.rs (currently missing — needed for MEM-06)
- **D-13:** Add `AppAction::ListMemories`, `AppAction::DeleteMemory { memory_id }`, `AppAction::UpdateMemory { memory_id, content }` action variants
- **D-14:** Add `AppState.memories: Vec<MemorySummary>` field with a new UniFFI record containing id, content_preview, created_at, conversation_title

### Claude's Discretion

- Memory list empty state messaging
- Exact truncation lengths for content previews and tool input/output snippets
- Whether to show memory count badge in navigation
- Specific swipe gesture vs long-press for delete on mobile
- Agent step visual styling (colors, icons per tool type)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MEM-04 | User can view all stored memories in a dedicated memory management screen | D-09, D-10, D-14: `Screen::Memories`, `AppState.memories`, nav integration on all 3 platforms |
| MEM-05 | User can delete individual memories | D-05, D-13: `AppAction::DeleteMemory`, swipe-to-delete (mobile), delete button (desktop); must remove both SQLite row and usearch vector |
| MEM-06 | User can edit extracted memories to correct or refine them | D-04, D-12, D-13: inline edit flow, `update_memory()` SQL query, `AppAction::UpdateMemory` |
| AUI-01 | Agent UI re-enabled on all platforms with expanded tool set visible | D-11: uncomment hidden agent nav entries; also uncomment desktop `pub mod agents` and `agent_task_input` local state |
| AUI-02 | Agent tool usage displayed step-by-step with tool name, input, output | D-06, D-07, D-08: extend `AgentStepSummary` with `tool_input` field; update detail views on iOS, Android, desktop |
</phase_requirements>

---

## Summary

Phase 23 is a pure UI + thin Rust core extension phase. The heavy lifting (memory extraction pipeline, vector index, agent tool dispatch) is already done in Phases 20–22. This phase adds the read/write UI surface over the existing memory data and re-enables the already-written agent screens.

The Rust side requires four discrete additions: (1) a new `update_memory` SQL query, (2) a `MemorySummary` UniFFI record, (3) three new `AppAction` variants for memory CRUD, and (4) an extension to `AgentStepSummary` with `tool_input`. All existing query infrastructure (`list_memories`, `delete_memory`) is already present in `persistence/queries.rs`. The `VectorIndex::remove(key: u64)` method exists and follows the exact same pattern as `DeleteDocument` — fetch rowid first, then call `remove`, then `save`.

The UI work follows well-established patterns already present in the codebase. All three platforms have a hidden-but-complete agent screen that needs one line to re-enable. Memory screens are new files following the DocumentLibrary pattern (list + empty state + swipe/button delete + inline edit).

**Primary recommendation:** Implement in three sequential tasks — (1) Rust core additions, (2) memory screens on all three platforms, (3) re-enable agent nav and extend step display.

---

## Standard Stack

### Core (no new dependencies required)

All libraries needed for this phase are already in `Cargo.toml`. No new crates are required.

| Component | What Is Already Present | Why Sufficient |
|-----------|------------------------|----------------|
| `rusqlite` 0.38 | `list_memories`, `delete_memory`, `insert_memory` in queries.rs | Only need to add `update_memory` SQL update statement |
| `usearch` 2.24 | `VectorIndex::remove(key: u64)` in rag/index.rs | Memory delete follows same pattern as chunk delete in `DeleteDocument` |
| `uniffi` | `AgentStepSummary`, `MemoryRow`, `BackendSummary` UniFFI records | `MemorySummary` follows same `#[derive(uniffi::Record)]` pattern |
| SwiftUI (iOS) | `DocumentLibraryView`, `AgentView`, `ConversationListView` | Memory screen follows `DocumentLibraryView` structure; agent screen exists |
| Jetpack Compose (Android) | `DocumentLibraryScreen`, `AgentScreen`, `ConversationListScreen` | Same patterns apply |
| iced 0.14 (Desktop) | `views/documents.rs`, `views/agents.rs`, `views/home.rs` | Memory view follows documents.rs; agents.rs already written |

---

## Architecture Patterns

### Recommended Project Structure (new files)

```
rust/src/persistence/queries.rs            # add: update_memory(), get_memory_usearch_key()
rust/src/lib.rs                            # add: MemorySummary record, Screen::Memories variant,
                                           #      AppAction::ListMemories / DeleteMemory / UpdateMemory,
                                           #      AppState.memories field, actor handlers
ios/Mango/Mango/MemoryManagementView.swift # new: Memory list + inline edit screen
android/.../ui/MemoryScreen.kt             # new: Memory list + inline edit screen
desktop/iced/src/views/memories.rs         # new: Memory list + edit view following documents.rs pattern
desktop/iced/src/views/mod.rs              # uncomment agents, add memories
```

### Pattern 1: MemorySummary UniFFI Record

**What:** A new display-safe record that carries only what the UI needs (no raw DB row). Follows the established `ConversationSummary`, `DocumentSummary`, `BackendSummary` pattern.

**When to use:** Any time memory data crosses the UniFFI boundary.

```rust
// Source: lib.rs existing pattern (ConversationSummary, DocumentSummary)
#[derive(uniffi::Record, Clone, Debug)]
pub struct MemorySummary {
    pub id: String,
    /// First ~100 chars of the extracted fact
    pub content_preview: String,
    /// Unix timestamp (millis) when memory was created
    pub created_at: i64,
    /// Title of the source conversation, if the conversation still exists
    pub conversation_title: Option<String>,
    /// The usearch_key — needed client-side to pass back for delete
    pub usearch_key: i64,
}
```

Note: `usearch_key` must be included in `MemorySummary` so the actor does not need a secondary DB lookup to find the vector key when the user deletes a memory. The delete handler reads `usearch_key` from `AppState.memories` (already loaded).

### Pattern 2: ListMemories Action — Populates AppState.memories

**What:** `AppAction::ListMemories` triggers a fresh `list_memories()` DB query, maps `MemoryRow` to `MemorySummary` with a JOIN or in-memory lookup for conversation titles, and sets `AppState.memories`. Called when user navigates to `Screen::Memories`.

**When to use:** On `PushScreen { screen: Screen::Memories }` and after successful `DeleteMemory` / `UpdateMemory`.

```rust
// Source: actor pattern in lib.rs (analogous to refresh_agent_sessions / document list load)
AppAction::ListMemories => {
    let rows = persistence::queries::list_memories(actor_state.db.conn()).unwrap_or_default();
    actor_state.app_state.memories = rows
        .into_iter()
        .map(|row| {
            let preview = row.content.chars().take(100).collect::<String>();
            // conversation_title: look up in actor_state.app_state.conversations
            let title = actor_state.app_state.conversations
                .iter()
                .find(|c| c.id == row.conversation_id)
                .map(|c| c.title.clone());
            MemorySummary {
                id: row.id,
                content_preview: preview,
                created_at: row.created_at,
                conversation_title: title,
                usearch_key: row.usearch_key,
            }
        })
        .collect();
    actor_state.app_state.rev += 1;
    emit_state(&actor_state.app_state, shared, update_tx);
}
```

### Pattern 3: DeleteMemory — Dual Remove (SQLite + usearch)

**What:** Delete is a two-step operation: (1) remove the vector from the usearch HNSW index, (2) delete the SQLite row. Follows the exact pattern established by `DeleteDocument`.

**When to use:** `AppAction::DeleteMemory { memory_id }`.

```rust
// Source: lib.rs DeleteDocument handler (lines 3456-3513) — identical dual-remove pattern
AppAction::DeleteMemory { memory_id } => {
    // Fetch usearch_key from loaded memories in AppState (no extra DB query needed)
    let usearch_key = actor_state.app_state.memories
        .iter()
        .find(|m| m.id == memory_id)
        .map(|m| m.usearch_key);

    if let Some(key) = usearch_key {
        let _ = actor_state.vector_index.remove(key as u64);
        let _ = actor_state.vector_index.save();
    }
    let _ = persistence::queries::delete_memory(actor_state.db.conn(), &memory_id);
    actor_state.app_state.memories.retain(|m| m.id != memory_id);
    actor_state.app_state.rev += 1;
    emit_state(&actor_state.app_state, shared, update_tx);
}
```

### Pattern 4: UpdateMemory — SQL UPDATE (no vector re-embedding)

**What:** `update_memory(conn, memory_id, new_content)` updates the `content` column in SQLite. The vector embedding is NOT re-generated on edit — this is a deliberate simplification (the old embedding becomes stale but the memory is still recalled; re-embedding requires the full EmbeddingProvider pipeline and is deferred). Update the preview in `AppState.memories` in-place.

**When to use:** `AppAction::UpdateMemory { memory_id, content }`.

```rust
// Source: analogous to rename_conversation in queries.rs
pub fn update_memory(
    conn: &Connection,
    memory_id: &str,
    new_content: &str,
) -> Result<(), PersistenceError> {
    conn.prepare_cached("UPDATE memories SET content = ?2 WHERE id = ?1")?
        .execute(rusqlite::params![memory_id, new_content])?;
    Ok(())
}
```

Actor handler: after `update_memory`, find the matching `MemorySummary` in `AppState.memories` and update its `content_preview` in place (truncate to 100 chars), then emit state.

### Pattern 5: AgentStepSummary Extension (D-06)

**What:** Add `tool_input: Option<String>` to the existing `AgentStepSummary` record. The field is populated in `load_agent_steps_for_session` by extracting the first 200 chars of `action_payload` when `action_type == "tool_call"`.

**When to use:** Populating agent step detail view.

```rust
// Source: lib.rs line 91 — existing record, extend in place
#[derive(uniffi::Record, Clone, Debug)]
pub struct AgentStepSummary {
    pub id: String,
    pub step_number: u32,
    pub action_type: String,
    pub tool_name: Option<String>,
    // NEW: first ~200 chars of action_payload for tool_call steps
    pub tool_input: Option<String>,
    pub result_snippet: Option<String>,
    pub status: String,
}
```

The `load_agent_steps_for_session` function (lib.rs around line 2005) populates this from `AgentStepRow.action_payload` truncated to 200 chars, when `action_type == "tool_call"`.

### Pattern 6: Screen::Memories Navigation

**What:** Add `Memories` to the `Screen` UniFFI enum. Navigation follows the `PushScreen { screen: Screen::Documents }` pattern — no additional routing state needed.

```rust
// Source: lib.rs Screen enum (line 320)
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum Screen {
    Home,
    Settings,
    Chat { conversation_id: String },
    Onboarding { step: OnboardingStep },
    Documents,
    Agents,
    Memories,  // NEW
}
```

All three platform routers (`ContentView.swift`, `MainApp.kt`, `main.rs` view dispatch) must handle this new variant.

### Pattern 7: Re-enabling Hidden Agent Nav (D-11)

The agent navigation was hidden in quick task `260326-pgd`. Three locations need to be un-commented:

**iOS — ContentView.swift:** Add `case .agents:` handler routing to `AgentSessionListView()`, and add "Agents" button to toolbar alongside "Documents" and "Settings".

**Android — MainApp.kt:** Remove `else -> {}` fallback for `Screen.Agents`, add the `is Screen.Agents ->` arm routing to `AgentScreen`. Add `TextButton` for "Agents" in `topBarActions` in the `is Screen.Home` branch.

**Desktop — views/mod.rs:** Change `// AGENTS HIDDEN: pub mod agents;` to `pub mod agents;`. In `main.rs`, un-comment `agent_task_input` in App::Loaded struct, its initialization in `App::new()`, its match arm in `App::update()` for `Message::AgentTaskInputChanged` and `Message::LaunchAgent`, and the `Message::OpenAgents` handler. In the sidebar view, restore the Agents nav button.

### Anti-Patterns to Avoid

- **Re-embedding on edit:** Do NOT call the EmbeddingProvider in `UpdateMemory`. The stale vector is acceptable for v1 — re-embedding is deferred.
- **Extra DB query on delete:** Do NOT add a `SELECT usearch_key FROM memories WHERE id = ?` query. The key is already in `AppState.memories` (loaded by `ListMemories`). Use the in-memory lookup.
- **Separate full-content field in MemorySummary:** Do NOT add a `full_content` field. When the user taps a memory to edit, dispatch `ListMemories` or load on demand. The memory list already carries `content_preview`; if the edit view needs full text, use a `get_memory_full_content(conn, memory_id)` query called lazily, OR store full content in the summary. Given memories are short facts, storing full content in MemorySummary (not just preview) is the simpler path — avoids a round-trip.
- **Blocking swipe-to-delete without confirmation on iOS:** Use `.confirmationDialog` or alert before committing. Document screen does not do this but memory deletion is more consequential.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Swipe-to-delete on iOS | Custom gesture recognizer | SwiftUI `.onDelete(perform:)` on `ForEach` inside `List` |
| Delete confirmation on iOS | Custom modal view | `.confirmationDialog` or `.alert` with destructive action |
| Swipe-to-delete on Android | Manual touch handling | `SwipeToDismiss` from `material3` (used in `ConversationListScreen`) |
| Inline text editing on iOS | Custom overlay | `@State` + `.onTapGesture` toggling between `Text` and `TextField` |
| Inline text editing on Desktop (iced) | Stateful custom widget | Local `App::Loaded` field `memory_edit_state: Option<(String, String)>` following `rename_state` pattern |
| Vector key lookup on delete | New SQL query | Read `usearch_key` from already-loaded `AppState.memories` |

---

## Common Pitfalls

### Pitfall 1: Forgetting to Call `vector_index.save()` After Remove

**What goes wrong:** `VectorIndex::remove()` mutates in-memory state but does not persist. On app restart the deleted memory's vector reappears in search results while the SQLite row is gone, causing phantom recall.

**Why it happens:** `usearch` HNSW index is an in-memory structure serialized to `embeddings.usearch`. Remove without save leaves the file stale.

**How to avoid:** Always call `actor_state.vector_index.save()` after `remove()`. Pattern is established in `DeleteDocument` handler — copy it exactly.

**Warning signs:** Memory appears in conversation context after deletion; `MemoryRow` not found but search returns stale results.

### Pitfall 2: UniFFI Breaking Change from Adding Field to AgentStepSummary

**What goes wrong:** Adding `tool_input: Option<String>` to `AgentStepSummary` changes the UniFFI-generated Swift/Kotlin types. Any existing code that constructs `AgentStepSummary` from Rust literals in tests will fail to compile.

**Why it happens:** UniFFI records require all fields at construction. Existing `AgentStepSummary { id, step_number, ... }` struct literals in lib.rs (line ~2033) and tests will miss the new field.

**How to avoid:** Update ALL construction sites when adding the field. Search for `AgentStepSummary {` in the codebase — there is exactly one construction site in `load_agent_steps_for_session` in lib.rs (~line 2033). Add `tool_input: None` or populate appropriately.

### Pitfall 3: Screen::Memories Not Handled in Platform Router

**What goes wrong:** App crashes or shows blank screen when user navigates to Memories.

**Why it happens:** Swift `switch`/Kotlin `when`/Rust `match` on `Screen` are exhaustive. Adding a new variant without updating all three routers causes compile errors on Swift/Kotlin but may silently fall through in Rust `match` with a wildcard arm.

**How to avoid:** After adding `Screen::Memories` to the Rust enum, the Swift and Kotlin generated bindings will add the new case. The iOS `ContentView.swift` switch and Android `MainApp.kt` `when` both need explicit handling. Desktop `main.rs` view dispatch needs a new arm.

### Pitfall 4: ListMemories Not Dispatched on Screen Entry

**What goes wrong:** Memory screen shows empty list even though memories exist.

**Why it happens:** `AppState.memories` starts as `vec![]` (see `AppState::default()`). Unlike `conversations` and `documents` (loaded at startup), memories were intentionally left invisible in Phase 20 (`// No AppState rev increment -- memories are invisible in Phase 20 UI`). The field does not auto-populate.

**How to avoid:** The iOS/Android/Desktop memory screen views must dispatch `AppAction::ListMemories` (or `PushScreen { screen: Screen::Memories }` triggers it in the actor) on `.onAppear` / `LaunchedEffect` / `on_appear` equivalent.

The cleanest pattern: when the actor handles `PushScreen { screen: Screen::Memories }`, immediately call `ListMemories` inline (similar to how `LoadAgentSession` triggers the step load). Alternatively: trigger in the screen's appear handler.

### Pitfall 5: Agent Task Input State Missing on Desktop

**What goes wrong:** Desktop Agents screen has `agent_task_input: &str` parameter but the field is removed from `App::Loaded` (commented out in `// AGENTS HIDDEN: agent_task_input removed`).

**Why it happens:** When agents were hidden, `agent_task_input` was removed from the `App::Loaded` struct and `App::new()`. The `agent_list_view` function in `views/agents.rs` takes it as a parameter.

**How to avoid:** Restore `agent_task_input: String` to `App::Loaded` struct (initialized to `String::new()` in `App::new()`), restore `Message::AgentTaskInputChanged(String)` and `Message::LaunchAgent` match arms in `App::update()`, and restore the `Message::OpenAgents` handler. The view file `views/agents.rs` itself is complete and correct.

---

## Code Examples

### iOS Memory List with Swipe-to-Delete (SwiftUI)

```swift
// Source: DocumentLibraryView.swift pattern + ForEach .onDelete
List {
    ForEach(appManager.appState.memories, id: \.id) { memory in
        MemoryRowView(memory: memory)
            .contentShape(Rectangle())
            .onTapGesture {
                selectedMemoryId = memory.id
            }
    }
    .onDelete { indexSet in
        for index in indexSet {
            let memory = appManager.appState.memories[index]
            memoryToDelete = memory
            showDeleteConfirmation = true
        }
    }
}
.confirmationDialog("Delete Memory?", isPresented: $showDeleteConfirmation) {
    Button("Delete", role: .destructive) {
        if let m = memoryToDelete {
            appManager.dispatch(.deleteMemory(memoryId: m.id))
        }
    }
}
```

### Android Memory Screen (Jetpack Compose)

```kotlin
// Source: ConversationListScreen.kt pattern for swipe-to-dismiss
LazyColumn {
    items(appState.memories, key = { it.id }) { memory ->
        val dismissState = rememberSwipeToDismissBoxState(
            confirmValueChange = { value ->
                if (value == SwipeToDismissBoxValue.EndToStart) {
                    onDispatch(AppAction.DeleteMemory(memoryId = memory.id))
                    true
                } else false
            }
        )
        SwipeToDismissBox(state = dismissState, backgroundContent = { /* red delete bg */ }) {
            MemoryItem(memory = memory, onClick = { /* expand/edit */ })
        }
    }
}
```

### Desktop Memory View (iced — follows documents.rs pattern)

```rust
// Source: desktop/iced/src/views/documents.rs pattern
pub fn view(state: &AppState, memory_edit_state: &Option<(String, String)>, is_dark: bool)
    -> Element<'_, Message>
{
    // header + back button (same pattern as documents.rs)
    // list of memory rows with delete button (X) and edit/expand on click
    // empty state: "No memories yet." when state.memories.is_empty()
}
```

### load_agent_steps_for_session — Adding tool_input

```rust
// Source: lib.rs ~line 2005, extend existing function
AgentStepSummary {
    id: row.id.clone(),
    step_number: row.step_number as u32,
    action_type: row.action_type.clone(),
    tool_name: if row.action_type == "tool_call" {
        extract_tool_name(&row.action_payload)
    } else {
        None
    },
    // NEW: first 200 chars of action_payload for tool_call steps
    tool_input: if row.action_type == "tool_call" {
        Some(row.action_payload.chars().take(200).collect())
    } else {
        None
    },
    result_snippet: row.result.as_deref().map(|r| r.chars().take(200).collect()),
    status: row.status.clone(),
}
```

---

## Runtime State Inventory

> This phase does not rename, refactor, or migrate existing data. No runtime state inventory is needed.

---

## Environment Availability

> Step 2.6: This phase is purely code changes with no new external dependencies. All required tools (Rust toolchain, Swift, Kotlin, iced) are already confirmed by preceding phases completing successfully.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `cargo test` |
| Config file | none — inline `#[test]` modules in `rust/src/tests/` |
| Quick run command | `cd rust && cargo test memory -- --nocapture` |
| Full suite command | `cd rust && cargo test -- --nocapture` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MEM-04 | `list_memories()` returns all rows newest-first | unit | `cd rust && cargo test test_insert_and_list_memories` | ✅ `tests/memory.rs` |
| MEM-05 | `delete_memory()` removes SQLite row; `VectorIndex::remove()` removes vector | unit | `cd rust && cargo test test_delete_memory` | ✅ `tests/memory.rs` |
| MEM-06 | `update_memory()` mutates content column correctly | unit | `cd rust && cargo test test_update_memory` | ❌ Wave 0 — test does not exist yet |
| MEM-04 | `AppAction::ListMemories` populates `AppState.memories` | integration | `cd rust && cargo test test_list_memories_action` | ❌ Wave 0 |
| MEM-05 | `AppAction::DeleteMemory` removes from AppState + vector | integration | `cd rust && cargo test test_delete_memory_action` | ❌ Wave 0 |
| MEM-06 | `AppAction::UpdateMemory` updates preview in AppState | integration | `cd rust && cargo test test_update_memory_action` | ❌ Wave 0 |
| AUI-02 | `AgentStepSummary.tool_input` populated from action_payload | unit | `cd rust && cargo test test_agent_step_tool_input` | ❌ Wave 0 |
| AUI-01 | `Screen::Memories` handled by actor without panic | smoke | `cd rust && cargo test test_memories_screen_navigation` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cd rust && cargo test memory -- --nocapture`
- **Per wave merge:** `cd rust && cargo test -- --nocapture`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `rust/src/tests/memory.rs` — add `test_update_memory`, `test_list_memories_action`, `test_delete_memory_action`, `test_update_memory_action`
- [ ] `rust/src/tests/agent.rs` — add `test_agent_step_tool_input` verifying `tool_input` is populated when `action_type == "tool_call"`
- [ ] `rust/src/tests/routing.rs` or `memory.rs` — add `test_memories_screen_navigation` verifying `Screen::Memories` round-trips through actor

---

## Project Constraints (from CLAUDE.md)

All applicable to this phase:

- **Architecture:** Rust core owns all logic. Native layers are thin UI + dispatch only. All memory CRUD logic lives in `lib.rs` actor loop — no business logic in Swift/Kotlin/iced.
- **Privacy:** Memories are local-only (already in SQLite + usearch on-device). This phase adds no network calls.
- **UniFFI:** `MemorySummary` must be a `#[derive(uniffi::Record)]`. `Screen::Memories` must be added to the `#[derive(uniffi::Enum)]` Screen enum. `AppAction` variants must be `uniffi::Enum` variants.
- **No OpenSSL / no native-tls:** Not applicable — this phase has no new HTTP.
- **Build system:** Nix flake + `just`. No Cargo.toml changes expected (no new dependencies).

---

## Open Questions

1. **Should `AppState.memories` be loaded at startup (like conversations/documents) or only when navigating to Memories?**
   - What we know: Memories were explicitly hidden from AppState in Phase 20 (`// No AppState rev increment`). Conversations and documents load at startup. Loading all memories at startup has minimal cost (most users have < 200 memories) but consumes AppState snapshot size.
   - What's unclear: Is there a performance concern with large memory counts?
   - Recommendation: Load lazily on `Screen::Memories` navigation (handle `PushScreen { screen: Screen::Memories }` in the actor to auto-dispatch `ListMemories`). Avoids startup overhead for users who never visit the memory screen.

2. **Does `UpdateMemory` need to re-embed the new content for semantic recall?**
   - What we know: The vector stored in usearch corresponds to the original extracted fact. After editing, the vector becomes stale. Memory will still be recalled when the embedding of the query happens to match the old vector.
   - What's unclear: How significant is the semantic drift for typical edits?
   - Recommendation: Skip re-embedding in this phase per existing decision in CONTEXT.md. If the user corrects a factual error in a memory (e.g., "prefers Python" → "prefers Rust"), the recall quality drops but does not break. Document this limitation in code comments.

3. **conversation_title in MemorySummary — JOIN vs in-memory lookup?**
   - What we know: `MemoryRow.conversation_id` is a foreign key to `conversations`. The actor already holds `AppState.conversations: Vec<ConversationSummary>`. A DB JOIN is not necessary.
   - Recommendation: In-memory lookup against `actor_state.app_state.conversations` in the `ListMemories` handler. O(n*m) but n (memories) and m (conversations) are both small.

---

## Sources

### Primary (HIGH confidence)

- Direct code inspection of `/home/lio/g/confidential-app/rust/src/lib.rs` — AppState fields, AppAction variants, Screen enum, AgentStepSummary struct, actor loop patterns
- Direct code inspection of `/home/lio/g/confidential-app/rust/src/persistence/queries.rs` — list_memories, delete_memory, insert_memory, MemoryRow, existing query patterns
- Direct code inspection of `/home/lio/g/confidential-app/rust/src/rag/index.rs` — VectorIndex::remove(key: u64), save() pattern
- Direct code inspection of `/home/lio/g/confidential-app/ios/Mango/Mango/AgentView.swift` — existing iOS agent screen (complete, needs tool_input extension)
- Direct code inspection of `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/AgentScreen.kt` — existing Android agent screen
- Direct code inspection of `/home/lio/g/confidential-app/desktop/iced/src/views/agents.rs` — existing desktop agent view
- Direct code inspection of `/home/lio/g/confidential-app/desktop/iced/src/views/documents.rs` — reference pattern for memory view
- Direct code inspection of `/home/lio/g/confidential-app/desktop/iced/src/main.rs` — Message enum, App::Loaded struct, AGENTS HIDDEN comments
- Direct code inspection of `/home/lio/g/confidential-app/ios/Mango/Mango/ContentView.swift` — AGENTS HIDDEN comment location

### Secondary (MEDIUM confidence)

- CONTEXT.md D-01 through D-14 — user decisions
- REQUIREMENTS.md — requirement definitions
- STATE.md — accumulated phase context

---

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — all libraries already in use; research is reading existing code, not discovering new libs
- Architecture patterns: HIGH — all patterns are copies or minor extensions of existing actor/UniFFI patterns in the codebase
- Pitfalls: HIGH — pitfalls derived from direct code inspection of where AGENTS HIDDEN comments exist and how VectorIndex/UniFFI work

**Research date:** 2026-04-04
**Valid until:** 2026-05-04 (stable APIs, in-house codebase)
