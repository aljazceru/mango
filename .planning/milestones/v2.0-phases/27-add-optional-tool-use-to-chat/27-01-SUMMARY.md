---
phase: 27-add-optional-tool-use-to-chat
plan: "01"
subsystem: rust-core
tags: [persistence, tools, chat, migration]
dependency_graph:
  requires: [27-00]
  provides: [CHAT-TOOL-01, CHAT-TOOL-02, CHAT-TOOL-03]
  affects: [rust/src/persistence, rust/src/agent, rust/src/lib.rs]
tech_stack:
  added: []
  patterns:
    - MIGRATION_V16 ALTER TABLE pattern (same as V12/V13)
    - build_chat_tools() delegates to build_agent_tools() and filters
    - SetConversationToolsEnabled follows SetSystemPrompt handler pattern
key_files:
  created: []
  modified:
    - rust/src/persistence/schema.rs
    - rust/src/persistence/queries.rs
    - rust/src/agent/tools.rs
    - rust/src/lib.rs
    - rust/src/tests/chat_tools.rs
    - rust/src/tests/persistence.rs
    - rust/src/tests/rag.rs
    - rust/src/tests/settings.rs
decisions:
  - "ChatCompletionTools::Custom variant handled with pass-through (return true) in build_chat_tools filter"
  - "LoadConversation reuses list_conversations to find tools_enabled rather than a dedicated query"
  - "SetConversationToolsEnabled updates current_conv_tools_enabled only when conversation_id matches active"
metrics:
  duration: "~10 minutes"
  completed: "2026-04-07T15:17:54Z"
  tasks: 2
  files: 8
---

# Phase 27 Plan 01: Migration V16, Persistence, build_chat_tools, and SetConversationToolsEnabled Summary

**One-liner:** MIGRATION_V16 adds tools_enabled column, ConversationRow/Summary carry it, build_chat_tools() filters agent tools for chat, SetConversationToolsEnabled persists toggle per-conversation.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Migration V16, persistence queries, and build_chat_tools | 68ece65 | schema.rs, queries.rs, tools.rs, chat_tools.rs, persistence.rs, rag.rs, settings.rs |
| 2 | ConversationSummary.tools_enabled and SetConversationToolsEnabled action | 329c1ed | lib.rs |

## What Was Built

### Migration V16
Added `tools_enabled INTEGER NOT NULL DEFAULT 0` to the conversations table via `MIGRATION_V16` in `schema.rs`. DEFAULT 0 means all pre-Phase-27 conversations have tools disabled, preserving existing behaviour.

### Persistence Layer
- `ConversationRow` gained `pub tools_enabled: bool`
- `insert_conversation` extended to column 8 (`tools_enabled`)
- `list_conversations` extended to select and map column 7 to bool
- New `update_conversation_tools_enabled(conn, conversation_id, enabled, updated_at)` query function

### build_chat_tools()
New public function in `rust/src/agent/tools.rs` that delegates to `build_agent_tools()` and filters:
- Excludes `finish` always (not meaningful in chat)
- Excludes `search_documents`/`read_document` unless `include_doc_search = true`
- Excludes `web_search` unless `brave_api_key_set = true`
- `ChatCompletionTools::Custom(_)` is passed through (forward-compatible)

### UniFFI Surface
- `ConversationSummary.tools_enabled: bool` added so UI can render the toggle without extra queries
- `AppAction::SetConversationToolsEnabled { conversation_id, enabled }` added to the enum
- Handler persists to DB, updates `ActorState.current_conv_tools_enabled` if active conversation, and calls `refresh_conversations`

### ActorState
- New field `current_conv_tools_enabled: bool` (default false on startup)
- `LoadConversation` handler loads from DB via `list_conversations` lookup
- `NewConversation` handler resets to false
- Available in `do_send_message` for Plan 02 to consult

## Wave 0 Tests
All 7 unit tests in `rust/src/tests/chat_tools.rs` pass GREEN after this plan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ChatCompletionTools has Custom variant not covered by match**
- **Found during:** Task 1
- **Issue:** The `build_chat_tools` match expression only covered `Function`, but `ChatCompletionTools` has a `Custom` variant. Compiler error E0004.
- **Fix:** Added `ChatCompletionTools::Custom(_) => return true` arm to pass through custom tools.
- **Files modified:** rust/src/agent/tools.rs
- **Commit:** 68ece65

**2. [Rule 1 - Bug] Test import path for ChatCompletionTools was wrong**
- **Found during:** Task 1 (Wave 0 test file from plan 27-00)
- **Issue:** Tests used `async_openai::types::ChatCompletionTools` but the correct path is `async_openai::types::chat::ChatCompletionTools`.
- **Fix:** Updated import and match arms to use filter_map with wildcard for `Custom` variant.
- **Files modified:** rust/src/tests/chat_tools.rs
- **Commit:** 68ece65

**3. [Rule 1 - Bug] Hardcoded user_version assertions expected 15 but got 16**
- **Found during:** Task 1 full suite regression check
- **Issue:** Multiple tests in persistence.rs and rag.rs had `assert_eq!(version, 15, ...)` which now correctly fails because V16 exists.
- **Fix:** Updated all assertions to 16 with updated messages.
- **Files modified:** rust/src/tests/persistence.rs, rust/src/tests/rag.rs
- **Commit:** 68ece65

## Known Stubs

None -- `tools_enabled` is fully wired through DB and exposed in `ConversationSummary`. Plan 02 will consume `ActorState.current_conv_tools_enabled` in `do_send_message`.

## Self-Check

Files created/modified exist:
- rust/src/persistence/schema.rs: contains MIGRATION_V16
- rust/src/persistence/queries.rs: contains tools_enabled in struct, insert, list, update
- rust/src/agent/tools.rs: contains build_chat_tools
- rust/src/lib.rs: contains SetConversationToolsEnabled

Commits:
- 68ece65: feat(27-01): migration V16, tools_enabled persistence, and build_chat_tools
- 329c1ed: feat(27-01): ConversationSummary.tools_enabled and SetConversationToolsEnabled action

## Self-Check: PASSED
