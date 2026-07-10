# Phase 23: Memory Management UI + Agent UI - Context

**Gathered:** 2026-04-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can view, edit, and delete their stored memories through a dedicated screen on all platforms, and the agent system with its expanded tools (Brave Search, URL fetch, file ops, calculator) is fully accessible on iOS, Android, and Desktop. Agent session detail shows each tool call step with tool name, input, and output.

</domain>

<decisions>
## Implementation Decisions

### Memory List Layout
- **D-01:** Memories displayed as a simple chronological list (newest first), each row showing a content preview (first ~100 chars) and the source conversation title if available
- **D-02:** Follow existing list patterns (ConversationListView on iOS, ChatScreen list on Android, home.rs list on desktop) for visual consistency

### Memory Editing Flow
- **D-03:** Tap/click a memory row to expand or navigate to a detail view showing full content
- **D-04:** Edit is inline — user modifies text directly and saves with a confirm action; cancel discards changes
- **D-05:** Delete is available from the list view (swipe-to-delete on mobile, delete button on desktop) with confirmation

### Agent Tool Step Display
- **D-06:** AgentStepSummary must be extended with `tool_input: Option<String>` field (first ~200 chars of input payload) to satisfy AUI-02
- **D-07:** Agent session detail view shows each step as: step number, tool name, truncated input, truncated output/result, status badge
- **D-08:** Tool steps with type "final_answer" display the full answer text rather than tool name/input

### Navigation Integration
- **D-09:** Add `Screen::Memories` variant to the Screen enum in lib.rs
- **D-10:** Memory and Agent entries both appear as top-level navigation items alongside Conversations, RAG, and Settings
- **D-11:** Re-enable Agent navigation by uncommenting/restoring the hidden agent nav entries on all platforms

### Rust Core Changes
- **D-12:** Add `update_memory(conn, memory_id, new_content)` query in persistence/queries.rs (currently missing — needed for MEM-06)
- **D-13:** Add `AppAction::ListMemories`, `AppAction::DeleteMemory { memory_id }`, `AppAction::UpdateMemory { memory_id, content }` action variants
- **D-14:** Add `AppState.memories: Vec<MemorySummary>` field with a new UniFFI record containing id, content_preview, created_at, conversation_title

### Claude's Discretion
- Memory list empty state messaging
- Exact truncation lengths for content previews and tool input/output snippets
- Whether to show memory count badge in navigation
- Specific swipe gesture vs long-press for delete on mobile
- Agent step visual styling (colors, icons per tool type)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Memory System
- `rust/src/memory/mod.rs` — Memory module root (extract + retrieve submodules)
- `rust/src/memory/extract.rs` — Memory extraction logic (should_extract, call_extraction_llm)
- `rust/src/memory/retrieve.rs` — Memory retrieval and system prompt injection
- `rust/src/persistence/queries.rs` — Memory queries: insert_memory, list_memories, delete_memory, get_memory_content_by_usearch_keys

### Agent System
- `rust/src/agent/mod.rs` — Agent module root
- `rust/src/agent/loop.rs` — ReAct orchestration loop
- `rust/src/agent/tools.rs` — Tool definitions, build_agent_tools, dispatch_tools (Brave Search, URL fetch, file ops, calculator)

### Core Architecture
- `rust/src/lib.rs` — AppState, AppAction, Screen enum, actor loop, AgentStepSummary
- `rust/src/persistence/schema.rs` — SQLite migrations (memories table in V15)

### Existing UI (patterns to follow)
- `ios/Mango/Mango/ContentView.swift` — iOS navigation (agent hidden here)
- `ios/Mango/Mango/AgentView.swift` — Existing iOS agent UI
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — Android navigation (agent hidden here)
- `android/app/src/main/java/dev/disobey/mango/ui/AgentScreen.kt` — Existing Android agent UI
- `desktop/iced/src/views/agents.rs` — Existing desktop agent view
- `desktop/iced/src/views/home.rs` — Desktop home (nav reference)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `persistence::queries::list_memories()` — Already returns Vec<MemoryRow> ordered by created_at DESC
- `persistence::queries::delete_memory()` — Already exists, caller must also remove usearch vector entry
- `AgentView.swift` / `AgentScreen.kt` / `views/agents.rs` — Existing agent UI screens, need tool display enhancement
- `AgentWorker.kt` — Kotlin coroutine for async agent state polling (Android)
- `ConversationListView` pattern — Reusable list-with-swipe-delete pattern on iOS

### Established Patterns
- UniFFI Record types for UI-safe data (e.g., `BackendSummary`, `ConversationSummary`) — follow for `MemorySummary`
- `AppAction` enum dispatch through actor loop — all state changes go through this pattern
- `Screen` enum for navigation — add `Memories` variant
- State emission via `AppReconciler` callback — memory list updates follow same path

### Integration Points
- `lib.rs` actor loop — new action handlers for ListMemories, DeleteMemory, UpdateMemory
- `lib.rs` AppState — new `memories` field populated on screen navigation
- `rag/index.rs` VectorIndex — must remove vector entry when deleting a memory (usearch_key)
- All three platform nav screens — add Memory nav entry, unhide Agent nav entry

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches for memory management UI and agent tool display.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 23-memory-management-ui-agent-ui*
*Context gathered: 2026-04-04*
