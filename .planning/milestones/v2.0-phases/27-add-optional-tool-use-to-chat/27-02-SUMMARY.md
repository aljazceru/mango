---
phase: 27-add-optional-tool-use-to-chat
plan: "02"
subsystem: rust-core
tags: [streaming, tools, chat, actor, tool-calling]
dependency_graph:
  requires: [27-01]
  provides: [CHAT-TOOL-04, CHAT-TOOL-05, CHAT-TOOL-06]
  affects: [rust/src/llm/streaming.rs, rust/src/lib.rs, rust/src/agent/mod.rs]
tech_stack:
  added: []
  patterns:
    - spawn_chat_tool_round delegates to run_agent_step_for_backend then sends InternalEvent
    - run_streaming_with_api_messages shared inner function avoids code duplication between spawn_streaming_task variants
    - ChatToolCallsReady conv_id race guard follows AgentStepComplete session_id guard pattern
    - Tool dispatch on actor thread (not async task) to avoid runtime.block_on panic
    - spawn_streaming_task_from_api_messages avoids lossy ChatMessage conversion for Tool/AssistantWithToolCalls
key_files:
  created: []
  modified:
    - rust/src/llm/streaming.rs
    - rust/src/llm/mod.rs
    - rust/src/lib.rs
    - rust/src/agent/mod.rs
decisions:
  - "Refactored spawn_streaming_task and spawn_streaming_task_from_api_messages to share run_streaming_with_api_messages inner function, eliminating code duplication"
  - "Tinfoil/PPQ backends in spawn_streaming_task_from_api_messages do best-effort API-to-ChatMessage conversion since they take ChatMessage; Tool-role messages become assistant context"
  - "ChatToolNone handler injects text into streaming_text and sends StreamDone so standard message-persist path handles it without duplication"
  - "build_chat_tools re-exported from agent module to make it accessible in lib.rs"
metrics:
  duration: "~15 minutes"
  completed: "2026-04-07T15:31:41Z"
  tasks: 2
  files: 4
---

# Phase 27 Plan 02: Chat Tool Round-Trip Runtime Wiring Summary

**One-liner:** Non-streaming tool detection round with actor-thread dispatch and streaming follow-up using spawn_streaming_task_from_api_messages for lossless API message passing.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | InternalEvent variants and spawn_streaming_task_from_api_messages | 0086b5b | streaming.rs, llm/mod.rs |
| 2 | do_send_message tool branch and ChatToolCallsReady/ChatToolNone handlers | 9f5915d | lib.rs, agent/mod.rs |

## What Was Built

### InternalEvent Variants (streaming.rs)

Two new variants added to `InternalEvent`:
- `ChatToolCallsReady { conv_id, tool_calls, pre_tool_messages, backend_id, model }` — delivered when the non-streaming round returns tool calls. Actor dispatches tools synchronously then spawns streaming follow-up.
- `ChatToolNone { conv_id, text }` — delivered when model answers without calling any tool. Actor injects text into streaming_text and triggers StreamDone.

### spawn_streaming_task_from_api_messages (streaming.rs)

New public function accepting `Vec<ChatCompletionRequestMessage>` directly instead of `Vec<ChatMessage>`. Avoids the lossy conversion of Tool-role and Assistant-with-tool-calls messages through the simpler ChatMessage type.

Implementation shares a common `run_streaming_with_api_messages` async inner function with the existing `spawn_streaming_task`, eliminating code duplication. For Tinfoil/PPQ backends (which require `ChatMessage`), a best-effort `api_messages_to_chat_messages` conversion is applied — Tool messages become assistant context strings.

### spawn_chat_tool_round (lib.rs)

New function that spawns a Tokio async task calling `run_agent_step_for_backend` with the chat tool set. Result dispatched as either `ChatToolCallsReady` or `ChatToolNone`. Errors become `StreamError`. Tool dispatch itself is NOT done inside this async task — it happens in the actor loop handler.

### do_send_message tool branch (lib.rs)

After building chat_messages and before spawning streaming, a new branch checks:
1. `actor_state.current_conv_tools_enabled` (set by Plan 01)
2. `backend.supports_tool_use`
3. `agent::build_chat_tools(has_docs, brave_key_set)` returns non-empty

If all conditions met: sets `BusyState::Loading { "Running tools..." }`, converts ChatMessages to API message types, calls `spawn_chat_tool_round`, and returns early (skipping normal streaming).

If tools empty or conditions not met: falls through to existing streaming path unchanged.

### ChatToolCallsReady handler (lib.rs)

Runs on actor thread inside the InternalEvent match. Flow:
1. **Race guard**: if `app_state.current_conversation_id != conv_id`, drops event and resets to Idle
2. **dispatch_tools**: called synchronously on actor thread (safe — no Tokio context panic)
3. **Message assembly**: wraps tool_calls in `ChatCompletionMessageToolCalls::Function`, appends assistant turn + tool result messages to pre_tool_messages
4. **Backend lookup**: finds backend by `backend_id`, resolves API key from keychain if not inline
5. **Streaming follow-up**: transitions to `BusyState::Streaming`, calls `spawn_streaming_task_from_api_messages` with full API message history

### ChatToolNone handler (lib.rs)

For the case where model answers without calling tools: pushes text into `streaming_text`, sends `StreamDone` internally, so the existing StreamDone handler persists the assistant message to SQLite and clears busy state.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ChatCompletionMessageToolCalls enum wrapper required for .tool_calls() builder**
- **Found during:** Task 2 compilation
- **Issue:** `ChatCompletionRequestAssistantMessageArgs::tool_calls()` expects `Vec<ChatCompletionMessageToolCalls>`, not `Vec<ChatCompletionMessageToolCall>`. Each call must be wrapped in `ChatCompletionMessageToolCalls::Function`.
- **Fix:** Added `.map(|c| ChatCompletionMessageToolCalls::Function(c.clone()))` before passing to builder. Follows the same pattern used in `handle_agent_step_complete`.
- **Files modified:** rust/src/lib.rs
- **Commit:** 9f5915d

**2. [Rule 2 - Missing] spawn_streaming_task refactored to share inner function**
- **Found during:** Task 1
- **Issue:** Plan suggested considering a shared inner function to avoid code duplication between `spawn_streaming_task` and `spawn_streaming_task_from_api_messages`. 
- **Fix:** Extracted `run_streaming_with_api_messages` async inner function. `spawn_streaming_task` now converts ChatMessage → API message types then calls it. `spawn_streaming_task_from_api_messages` calls it directly. Significantly reduces code.
- **Files modified:** rust/src/llm/streaming.rs
- **Commit:** 0086b5b

**3. [Rule 3 - Missing] build_chat_tools not exported from agent module**
- **Found during:** Task 2
- **Issue:** `agent::build_chat_tools` was defined in `rust/src/agent/tools.rs` but not re-exported from `rust/src/agent/mod.rs`.
- **Fix:** Added `build_chat_tools` to the `pub use tools::{...}` line in `agent/mod.rs`.
- **Files modified:** rust/src/agent/mod.rs
- **Commit:** 9f5915d

**4. [Rule 3 - Missing] Plan 01 commits not yet in worktree**
- **Found during:** Start of execution
- **Issue:** This worktree was behind main — Plan 01 commits (68ece65, 329c1ed, ef3fcc3) were in the main repo but not in the worktree branch.
- **Fix:** Fetched main from local repo (`git fetch /home/lio/g/confidential-app main:refs/remotes/local-main`) and fast-forwarded the worktree branch.
- **Commit:** Merge fast-forward only

## Known Stubs

None — the chat tool round-trip is fully wired. Tool messages never appear in `AppState.messages` (they exist only in the intermediate API message vec). BusyState transitions correctly between Loading (tool round) and Streaming (follow-up).

## Self-Check: PASSED

Files exist:
- rust/src/llm/streaming.rs: contains ChatToolCallsReady (2 occurrences), spawn_streaming_task_from_api_messages (2 occurrences)
- rust/src/lib.rs: contains spawn_chat_tool_round (2 occurrences), dispatch_tools in ChatToolCallsReady handler

Commits exist:
- 0086b5b: feat(27-02): add ChatToolCallsReady/ChatToolNone variants and spawn_streaming_task_from_api_messages
- 9f5915d: feat(27-02): wire chat tool round-trip in do_send_message and InternalEvent handlers

All 241 tests pass: `cargo test -p mango_core` — 0 failures.
