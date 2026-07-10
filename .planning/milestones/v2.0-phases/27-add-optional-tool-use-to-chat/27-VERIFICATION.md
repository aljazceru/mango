---
phase: 27-add-optional-tool-use-to-chat
verified: 2026-04-07T00:00:00Z
status: human_needed
score: 8/8 must-haves verified
human_verification:
  - test: "Run the desktop app, open a conversation, click the 'Tools' button in the chat header to enable it, send a message, and confirm the response uses tools (look for 'Running tools...' in the busy state)"
    expected: "BusyState shows 'Running tools...' then transitions to streaming; final response reflects tool results if available"
    why_human: "Cannot verify runtime BusyState transitions or actual tool dispatch behavior without a running backend"
  - test: "Toggle 'Tools' on for a conversation, close the app, reopen it, and navigate back to the same conversation"
    expected: "The 'Tools [ON]' button is still active — tools_enabled was persisted to SQLite"
    why_human: "Persistence across app restarts requires running the app end-to-end"
  - test: "Enable tools on Conversation A, switch to Conversation B, then check Conversation B's toolbar"
    expected: "Conversation B shows 'Tools' (OFF) — the toggle is per-conversation, not global"
    why_human: "Per-conversation isolation of toggle state requires live UI testing"
---

# Phase 27: Add Optional Tool Use to Chat Verification Report

**Phase Goal:** Add optional tool use to chat -- allow users to enable tool calling (web search, URL fetch, file operations) in regular chat conversations via a per-conversation toggle, using a single non-streaming first round for tool detection and dispatch on the actor thread.
**Verified:** 2026-04-07
**Status:** human_needed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | tools_enabled column exists on conversations table with DEFAULT 0 | VERIFIED | `MIGRATION_V16` in `rust/src/persistence/schema.rs` line 291; included in `MIGRATIONS` array at line 312 |
| 2 | ConversationSummary.tools_enabled reflects the persisted DB value | VERIFIED | `pub tools_enabled: bool` in ConversationSummary struct (lib.rs line 68); mapped from `row.tools_enabled` at lines 803 and 2404 |
| 3 | SetConversationToolsEnabled action toggles the DB column and updates AppState | VERIFIED | AppAction variant at lib.rs line 541; handler at line 3963 calls `update_conversation_tools_enabled` and `refresh_conversations` |
| 4 | Web search is not invoked when no Brave API key is configured | VERIFIED | `build_chat_tools(has_docs, brave_key_set)` passes `brave_key_set` derived from settings; `build_chat_tools` excludes `web_search` when `brave_api_key_set=false`; test `test_chat_tools_no_brave` passes |
| 5 | Document search tools are absent when no docs are attached | VERIFIED | `has_docs = !current_conversation_attached_docs.is_empty()` checked before build_chat_tools; `build_chat_tools` excludes `search_documents`/`read_document` when `include_doc_search=false`; test `test_chat_tools_with_docs` passes |
| 6 | Non-streaming first round detects tool calls and sends them to actor loop | VERIFIED | `spawn_chat_tool_round` (lib.rs line 1555) spawns async task calling `run_agent_step_for_backend`; result sent as `ChatToolCallsReady` InternalEvent |
| 7 | Tool dispatch runs on actor thread (not inside Tokio task) | VERIFIED | `dispatch_tools` called inside the `ChatToolCallsReady` match arm (lib.rs line 4794), which executes on the actor thread -- not inside the async task that spawns the tool round |
| 8 | Tool messages never appear in AppState.messages | VERIFIED | The `messages` Vec built in `ChatToolCallsReady` handler (pre_tool_messages + assistant tool_calls + tool results) is only passed to `spawn_streaming_task_from_api_messages`; never pushed to `app_state.messages`; `ChatToolNone` injects text into `streaming_text` which routes through `StreamDone` |
| 9 | User can toggle tools on/off in chat toolbar on all three platforms | VERIFIED | iOS: Toggle with wrench.fill in ChatView.swift toolbar (line 141); Android: TextButton in ChatTopBar (ChatScreen.kt line 351); Desktop: tools_btn in chat.rs header row (line 116) |
| 10 | After tool dispatch, streaming follow-up uses full message history with tool results | VERIFIED | `spawn_streaming_task_from_api_messages` called with assembled `messages` that includes pre_tool_messages + assistant turn + tool result messages (lib.rs line 4865) |

**Score:** 10/10 truths verified (8 required must-haves + 2 additional truths confirmed)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `rust/src/tests/chat_tools.rs` | 7 test stubs for migration, persistence, tool subset | VERIFIED | 7 tests exist; all 7 pass GREEN |
| `rust/src/tests/mod.rs` | `mod chat_tools` registration | VERIFIED | Line 9: `mod chat_tools;` |
| `rust/src/persistence/schema.rs` | MIGRATION_V16 adding tools_enabled column | VERIFIED | Lines 291-312; ALTER TABLE with DEFAULT 0 |
| `rust/src/persistence/queries.rs` | ConversationRow.tools_enabled, insert/list/update queries | VERIFIED | Field at line 36; insert at line 98/109; list at line 117/130; update function at line 141 |
| `rust/src/agent/tools.rs` | `build_chat_tools()` function | VERIFIED | `pub fn build_chat_tools` at line 206; filters `finish`, `web_search`, `search_documents`, `read_document` based on params |
| `rust/src/lib.rs` | ConversationSummary.tools_enabled, SetConversationToolsEnabled action, handler | VERIFIED | Field at line 68; action at line 541; handler at line 3963 |
| `rust/src/llm/streaming.rs` | ChatToolCallsReady and ChatToolNone InternalEvent variants; spawn_streaming_task_from_api_messages | VERIFIED | ChatToolCallsReady at line 80; ChatToolNone at line 91; spawn_streaming_task_from_api_messages at line 235 |
| `ios/Bindings/mango_core.swift` | toolsEnabled field and setConversationToolsEnabled action | VERIFIED | toolsEnabled at line 1852; setConversationToolsEnabled at line 3142 |
| `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` | toolsEnabled and SetConversationToolsEnabled | VERIFIED | toolsEnabled at line 2369; SetConversationToolsEnabled at line 3285 |
| `ios/Mango/Mango/ChatView.swift` | Tools toggle in chat toolbar | VERIFIED | Toggle with toolsEnabled binding at line 143; onSetToolsEnabled callback at line 18 |
| `android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt` | Tools toggle in chat toolbar | VERIFIED | toolsEnabled read at line 351; SetConversationToolsEnabled dispatched at line 356 |
| `desktop/iced/src/main.rs` | ToggleConvToolsEnabled message and handler | VERIFIED | Variant at line 345; handler at line 963 dispatches SetConversationToolsEnabled |
| `desktop/iced/src/views/chat.rs` | Tools toggle button in chat header | VERIFIED | tools_btn at line 116; reads tools_enabled from state at line 110 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `lib.rs (do_send_message)` | `spawn_chat_tool_round` | `current_conv_tools_enabled && backend.supports_tool_use` conditional | WIRED | Branch at lib.rs line 1455; calls spawn_chat_tool_round at line 1508 |
| `spawn_chat_tool_round` | `agent::run_agent_step_for_backend` | async task calls function | WIRED | lib.rs line 1567 |
| `ChatToolCallsReady handler` | `agent::dispatch_tools` | actor thread synchronous call | WIRED | lib.rs line 4794 -- inside InternalEvent match arm (actor thread), not async |
| `ChatToolCallsReady handler` | `llm::spawn_streaming_task_from_api_messages` | spawns follow-up with API messages | WIRED | lib.rs line 4865 |
| `lib.rs (SetConversationToolsEnabled handler)` | `persistence::queries::update_conversation_tools_enabled` | persists toggle to DB | WIRED | lib.rs line 3965 |
| `lib.rs (list_conversations mapping)` | `ConversationSummary.tools_enabled` | maps row.tools_enabled | WIRED | lib.rs lines 803 and 2404 |
| `ios ChatView.swift` | `AppAction.setConversationToolsEnabled` | onSetToolsEnabled callback -> ContentView dispatch | WIRED | ChatView.swift line 18 (callback); ContentView.swift line 44 (dispatch call) |
| `android ChatScreen.kt` | `AppAction.SetConversationToolsEnabled` | onDispatchAction callback | WIRED | ChatScreen.kt line 356 |
| `desktop main.rs` | `AppAction::SetConversationToolsEnabled` | ToggleConvToolsEnabled handler | WIRED | main.rs line 963 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `ChatView.swift` tools toggle | `toolsEnabled` | `ConversationSummary.toolsEnabled` from AppState | Yes -- read from DB via list_conversations | FLOWING |
| `ChatScreen.kt` tools toggle | `toolsEnabled` | `state.conversations.firstOrNull { it.id == state.currentConversationId }?.toolsEnabled` | Yes -- from live AppState | FLOWING |
| `chat.rs` tools button | `tools_enabled` | `state.conversations.iter().find(...)` | Yes -- from live AppState | FLOWING |
| `lib.rs do_send_message` | `current_conv_tools_enabled` | ActorState field loaded in LoadConversation handler | Yes -- loaded from `list_conversations` DB query | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 7 chat_tools unit tests pass | `cargo test -p mango_core chat_tools` | 7 passed, 0 failed | PASS |
| Full test suite passes (no regressions) | `cargo test -p mango_core` | 241 passed, 0 failed | PASS |
| build_chat_tools excludes finish and web_search without brave key | `test_chat_tools_no_brave` | ok | PASS |
| build_chat_tools includes doc tools when include_doc_search=true | `test_chat_tools_with_docs` | ok | PASS |
| MIGRATION_V16 adds tools_enabled column | `test_migration_v16` | ok | PASS |
| tools_enabled persists round-trip through insert/list | `test_tools_enabled_persistence` | ok | PASS |
| update_conversation_tools_enabled toggles the field | `test_update_conversation_tools_enabled` | ok | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CHAT-TOOL-01 | 27-00, 27-01 | Migration V16 adds tools_enabled column with DEFAULT 0 | SATISFIED | `MIGRATION_V16` in schema.rs; `test_migration_v16` passes |
| CHAT-TOOL-02 | 27-00, 27-01 | Per-conversation toggle persisted across restarts | SATISFIED | `update_conversation_tools_enabled` + `SetConversationToolsEnabled` handler; `test_tools_enabled_persistence` passes |
| CHAT-TOOL-03 | 27-00, 27-01 | Chat tool subset excludes finish, conditionally excludes web_search and doc tools | SATISFIED | `build_chat_tools` in tools.rs; 3 subset tests pass |
| CHAT-TOOL-04 | 27-02 | Non-streaming first round detects tool calls | SATISFIED | `spawn_chat_tool_round` calls `run_agent_step_for_backend`; `ChatToolCallsReady` handler wired |
| CHAT-TOOL-05 | 27-02 | Tool dispatch on actor thread | SATISFIED | `dispatch_tools` called inside `ChatToolCallsReady` match arm on actor thread, not inside async task |
| CHAT-TOOL-06 | 27-02 | Streaming follow-up with full message history including tool results | SATISFIED | `spawn_streaming_task_from_api_messages` called with pre_tool_messages + assistant turn + tool results |
| CHAT-TOOL-07 | 27-03 | Tools toggle visible in chat toolbar on iOS, Android, Desktop | SATISFIED (code confirmed; render needs human) | Toggle present in ChatView.swift, ChatScreen.kt, chat.rs |
| CHAT-TOOL-08 | 27-03 | Tool messages never appear in AppState.messages | SATISFIED | Tool messages assembled in local Vec only passed to streaming fn; `app_state.messages.push` only at lines 1308 and 4022 (user and agent-final-answer paths) |

**Note on REQUIREMENTS.md discrepancy:** CHAT-TOOL-04, 05, 06 are marked `[ ]` (unchecked) in REQUIREMENTS.md while CHAT-TOOL-01, 02, 03, 07, 08 are marked `[x]`. The traceability table shows all 8 as "Planned" rather than "Complete." The code fully implements all 8 requirements. This is a documentation update that was not completed -- the REQUIREMENTS.md file was not updated to mark these as done after Plan 02 executed.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | - | - | - | - |

No TODOs, FIXMEs, placeholders, empty returns, or hardcoded empty data found in any phase-27-modified files.

### Human Verification Required

#### 1. Tools Toggle Renders and Functions on Desktop

**Test:** Run `cargo run -p mango-desktop`, open a conversation, click "Tools" button in chat header, verify it toggles to "Tools [ON]" with accent color, send a message to confirm the busy state transitions to "Running tools..."
**Expected:** Button toggles visually; on send, BusyState briefly shows "Running tools..." then transitions to streaming
**Why human:** Runtime BusyState transitions and visual rendering cannot be verified by static analysis

#### 2. Per-Conversation Toggle Persistence Across Restarts

**Test:** Enable tools on Conversation A, close and reopen the app, navigate back to Conversation A
**Expected:** "Tools [ON]" button is still active -- state was persisted to SQLite and loaded on startup
**Why human:** End-to-end persistence requires running the app and restarting it

#### 3. Per-Conversation Toggle Isolation

**Test:** Enable tools on Conversation A, switch to Conversation B, check the toolbar
**Expected:** Conversation B shows "Tools" (OFF); switching back to A shows "Tools [ON]"
**Why human:** Multi-conversation state isolation requires live UI interaction

#### 4. Actual Tool Invocation End-to-End

**Test:** Enable tools on a conversation backed by a tool-use-capable backend, ask a question that would require web search (e.g., "What is today's news?"), observe the response
**Expected:** If Brave API key is configured, web_search is called; response cites results. If no Brave key, tools are excluded and response falls through to normal streaming
**Why human:** Requires a live backend with tool-use support (supports_tool_use=true) and optionally a Brave API key

### Gaps Summary

No gaps found. All 8 phase requirements are implemented and verified in code. The only remaining items are human-verifiable UI behaviors (visual rendering, live interaction, runtime state transitions) that cannot be confirmed through static analysis.

**REQUIREMENTS.md documentation gap (non-blocking):** CHAT-TOOL-04, 05, 06 remain marked `[ ]` in REQUIREMENTS.md and the traceability table still shows "Planned" for all 8 requirements. This is a stale documentation issue and does not affect goal achievement.

---

_Verified: 2026-04-07_
_Verifier: Claude (gsd-verifier)_
