# Phase 27: Add Optional Tool Use to Chat - Research

**Researched:** 2026-04-07
**Domain:** Rust async-openai streaming with tool call accumulation + per-conversation settings + chat UI extension
**Confidence:** HIGH

## Summary

Phase 27 extends the chat path with optional LLM tool use — the model can call web_search, fetch_url, calculate, and file tools mid-conversation just like agents do, but without launching a full ReAct loop. The user opts in per conversation via a toggle in the chat toolbar/bottom sheet (matching the existing "Instructions" pattern).

The key technical challenge is that the current `spawn_streaming_task` in `rust/src/llm/streaming.rs` knows nothing about tools. Tools require a different LLM flow: either (a) non-streaming request (`client.chat().create()` with `.tools()`) followed by tool dispatch and a second streaming request for the final answer, or (b) streaming with mid-stream tool call chunk accumulation. Both approaches have been implemented in the ecosystem; option (a) is dramatically simpler to integrate with the current codebase because it reuses `run_agent_step_for_backend` from `agent/loop.rs` and the existing `dispatch_tools` from `agent/tools.rs`.

All agent tool infrastructure (`build_agent_tools`, `dispatch_tools`, tool dispatch functions) is already present in `rust/src/agent/tools.rs` and is `pub(crate)`. The phase adds: (1) a per-conversation `tools_enabled` flag, (2) a new `InternalEvent::ChatToolRound` event so the actor loop can handle a tool round-trip without forking into a full agent session, (3) a settings toggle in the chat toolbar on all three platforms, and (4) a DB migration adding `tools_enabled` to the conversations table.

**Primary recommendation:** Use the non-streaming first-round approach — detect `FinishReason::ToolCalls` from a non-streaming request, dispatch tools, then stream the follow-up for the final visible response. This reuses existing `agent/loop.rs` and `agent/tools.rs` code without modifying `streaming.rs`.

## Project Constraints (from CLAUDE.md)

- Architecture: Rust core owns all business logic; native layers are thin UI + capability bridges only
- All LLM calls must use OpenAI-compatible chat completions API
- No OpenSSL — `reqwest` must use `rustls-tls`
- UniFFI boundary: never expose raw API keys or tool results directly; expose only display-safe data
- Platforms: iOS 17+, Android API 28+, Desktop (iced)
- Build system: Nix flake / `just` / UniFFI
- async-openai 0.33.1 is the locked version

## Standard Stack

### Core (already in Cargo.toml — no new dependencies needed)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| `async-openai` | 0.33.1 | Non-streaming tool call requests + streaming follow-up | `client.chat().create()` with `.tools()` for first round; `create_stream()` for final answer |
| `agent/tools.rs` | in-repo | `build_agent_tools()`, `dispatch_tools()` — already implement all 4 chat-relevant tools | Reuse as-is; no new tool schemas needed |
| `agent/loop.rs` | in-repo | `run_agent_step_for_backend()` — already handles tool-calling non-streaming request | Reuse directly in the new chat tool round handler |
| `rusqlite` | 0.39 | `tools_enabled` column on conversations table (new migration V16) | `ALTER TABLE conversations ADD COLUMN tools_enabled INTEGER NOT NULL DEFAULT 0` |
| `flume` | 0.11 | `InternalEvent::ChatToolRound` — new event variant for tool dispatch from async task | Follows established InternalEvent pattern |

**No new Cargo dependencies required.**

### Tool Subset for Chat

The agent has 7 tools including `search_documents`, `read_document`, and `finish`. Chat tool use should expose a subset — remove `finish` (no-op in chat context) and conditionally include `search_documents`/`read_document` only if RAG docs are attached. Recommended default subset:

| Tool | Include in Chat | Condition |
|------|-----------------|-----------|
| `web_search` | Yes | Always (if brave_api_key set) |
| `fetch_url` | Yes | Always |
| `calculate` | Yes | Always |
| `file` | Yes | Always (uses existing sandbox) |
| `search_documents` | Optional | Only if conversation has attached docs |
| `read_document` | Optional | Only if conversation has attached docs |
| `finish` | No | Not meaningful in chat context |

## Architecture Patterns

### Recommended: Non-Streaming First Round + Streaming Final Response

This is the simplest correct approach that reuses existing code:

```
User sends message
  -> do_send_message() checks actor_state.current_conv_tools_enabled
  -> if enabled: spawn async task that calls run_agent_step_for_backend()
     (non-streaming, same as agent loop, returns ToolCalls or FinalAnswer)
  -> if ToolCalls: dispatch_tools() synchronously on actor thread
     (same as agent loop step handling)
  -> spawn second streaming task with tool results in message history
     (same spawn_streaming_task as normal chat)
  -> StreamChunk / StreamDone as normal
```

vs. the alternative (streaming accumulation):

```
Streaming with tool_calls chunk accumulation:
  stream.next() -> delta.tool_calls chunks -> accumulate by index
  -> on FinishReason::ToolCalls: all chunks collected
  -> dispatch tools
  -> stream second request
```

The non-streaming first round is recommended because:
- `run_agent_step_for_backend()` already exists and handles all transport variants (Tinfoil, PPQ, standard)
- No modification to `spawn_streaming_task()` required
- Tool call accumulation from streaming chunks is complex (index-keyed partial JSON concatenation)
- One extra HTTP round trip is invisible to the user (< 500ms for tool dispatch)

### Actor Loop Event Flow

New `InternalEvent` variant needed:

```rust
// In streaming.rs InternalEvent enum
/// Tool call round completed for a chat conversation (Phase 27).
/// Delivered after non-streaming tool detection + dispatch.
/// Actor loop re-injects tool results into message history and spawns streaming task.
ChatToolRoundComplete {
    conv_id: String,
    /// The assistant message with tool_calls (to append to history before tool results)
    assistant_tool_calls_json: String,
    /// (tool_call_id, tool_name, result_text) triples for the tool message
    tool_results: Vec<(String, String, String)>,
    /// Whether the backend used supports streaming for the follow-up
    backend_id: String,
    model: String,
},
```

The actor loop handler for `ChatToolRoundComplete`:
1. Build the `ChatCompletionRequestMessage` history with tool results appended
2. Call `spawn_streaming_task()` with updated history
3. The streaming task fires `StreamChunk` / `StreamDone` as normal

### Project Structure Impact

```
rust/src/
├── llm/
│   └── streaming.rs     # Add ChatToolRoundComplete variant to InternalEvent
├── agent/
│   └── tools.rs         # Add build_chat_tools() (subset of build_agent_tools())
└── lib.rs               # +ConversationSummary.tools_enabled field
                         # +AppAction::SetConversationToolsEnabled
                         # +ActorState.current_conv_tools_enabled
                         # +handle_chat_tool_round() helper
                         # +ChatToolRoundComplete handler in event loop

rust/src/persistence/
└── schema.rs            # MIGRATION_V16: tools_enabled column

ios/Mango/Mango/
└── ChatView.swift       # Toggle in chat toolbar / system prompt sheet

android/.../ui/
└── ChatScreen.kt        # Toggle in ChatTopBar or system prompt bottom sheet

desktop/iced/src/views/
└── chat.rs              # Toggle in chat toolbar (follows show_system_prompt_input pattern)
```

### Per-Conversation Storage

Add `tools_enabled INTEGER NOT NULL DEFAULT 0` via Migration V16. Default OFF — user must explicitly enable per conversation. Load at `LoadConversation`; carry in `ActorState` (not in `AppState` — same pattern as other actor-internal state like `pending_rag_doc_count`). Expose as `ConversationSummary.tools_enabled: bool` so the UI can render the toggle state.

**Migration:**
```sql
ALTER TABLE conversations ADD COLUMN tools_enabled INTEGER NOT NULL DEFAULT 0;
```

**Query updates needed:**
- `insert_conversation()` — add `tools_enabled` column
- `list_conversations()` — add `tools_enabled` to SELECT
- New: `update_conversation_tools_enabled(conn, conv_id, enabled)` — same shape as `update_conversation_system_prompt()`

### UI Pattern: Chat Toolbar Toggle

Follow the exact pattern of the "Instructions" (SetSystemPrompt) toggle, which already exists on all three platforms:

- **iOS**: Button in the chat navigation toolbar → sheet or inline toggle (use `@State private var showToolsSheet = false` + bottom sheet, or a simpler toggle pill in the existing system prompt sheet)
- **Android**: New item in `ChatTopBar`'s overflow dropdown (or in `SystemPromptSheet`), dispatches `AppAction.SetConversationToolsEnabled`
- **Desktop**: New `Message::ToggleConvToolsEnabled` variant; toggle button in the chat header row alongside the Instructions toggle

The simplest UX: add a "Tools" toggle switch inside the existing system prompt / instructions sheet on all platforms. This avoids adding another button to the toolbar.

### Non-Streaming Tool Round — Spawning Pattern

The async task for the first tool-round follows the agent step pattern:

```rust
// Source: modeled on handle_launch_agent_session + spawn_agent_step_task patterns in lib.rs
fn spawn_chat_tool_round(
    runtime: &tokio::runtime::Runtime,
    backend: &BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,  // full history including user message
    tools: Vec<ChatCompletionTools>,
    conv_id: String,
    brave_api_key: String,
    data_dir: String,
    core_tx: flume::Sender<CoreMsg>,
) {
    let backend = backend.clone();
    let model = model.to_string();
    runtime.spawn(async move {
        let result = agent::run_agent_step_for_backend(
            &backend, &model, messages.clone(), tools,
        ).await;
        match result {
            Ok(AgentStepResult::ToolCalls(calls)) => {
                // Dispatch tools synchronously -- but we're in async context.
                // Solution: send back to actor loop via ChatToolRoundComplete,
                // let actor thread call dispatch_tools (same pattern as agent loop).
                let calls_json = serde_json::to_string(&calls).unwrap_or_default();
                let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
                    InternalEvent::ChatToolCallsReady {
                        conv_id,
                        tool_calls_json: calls_json,
                        messages_json: ...,
                        backend_id: backend.id.clone(),
                        model,
                    }
                )));
            }
            Ok(AgentStepResult::FinalAnswer(text)) | Ok(AgentStepResult::FinishTool(text)) => {
                // No tools called -- treat as a completed streaming response
                let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
                    InternalEvent::StreamDone
                )));
                // But we need to put 'text' into streaming_text first...
                // Alternative: emit StreamChunk(text) then StreamDone
            }
            Err(e) => { /* emit StreamError */ }
        }
    });
}
```

**Design decision for the planner:** The tool calls cannot be dispatched inside the async task (dispatch_tools is synchronous and uses `runtime.block_on()` internally for web_search/fetch_url, which would panic when called from within a runtime thread). The actor loop must receive the raw tool call list and dispatch them on the actor thread (same as the agent step loop does). Two InternalEvent variants are needed:
- `ChatToolCallsReady { conv_id, tool_calls: Vec<ChatCompletionMessageToolCall>, pre_tool_messages: ... }` — actor dispatches tools then spawns streaming
- `ChatToolRoundNone { text }` — model answered without tools; actor emits as normal StreamDone

### Message History Construction for Follow-up

After tool dispatch, the message history for the streaming follow-up must include:

```rust
// 1. All prior messages (system + conversation history)
// 2. The assistant message with tool_calls (required by OpenAI API)
// 3. One ChatCompletionRequestToolMessage per tool result
```

The `ChatCompletionRequestMessage` types needed:
- `ChatCompletionRequestAssistantMessage { content: None, tool_calls: Some(vec![...]) }` 
- `ChatCompletionRequestToolMessage { tool_call_id, content }` (one per result)

These types are already imported and used in `agent/loop.rs`.

### Anti-Patterns to Avoid

- **Tool dispatch inside Tokio task:** `dispatch_tools` calls `runtime.block_on()` for web/URL tools. Calling this from inside a `runtime.spawn()` task panics. Always dispatch on the actor thread.
- **Adding tools to streaming request directly:** The current `spawn_streaming_task` builds a `CreateChatCompletionRequest` without `.tools()`. Adding tools to the streaming request changes nothing without also handling `FinishReason::ToolCalls` in the stream consumer — the stream would just stop with no content and send `StreamDone`, losing the tool calls silently.
- **Exposing tool results in AppState.messages:** Tool call/result messages are internal conversation-history entries, not user-visible chat bubbles. They should NOT be pushed to `AppState.messages` (which drives the UI bubble list). Only the final streaming response becomes a visible message.
- **Global tools_enabled flag:** Tool use must be per-conversation, not global. Different conversations may need different behavior. Use the conversations table column approach, not the settings table.
- **Using `finish` tool in chat context:** The `finish` tool signals agent session termination. In chat context, `FinalAnswer` from `run_agent_step_for_backend` is sufficient — no `finish` tool needed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Tool schemas for chat | New JSON schema definitions | `build_agent_tools()` from `agent/tools.rs` (filter out `finish`) |
| Web search dispatch | New reqwest calls | `dispatch_web_search()` from `agent/tools.rs` |
| URL fetch dispatch | New HTML parsing | `dispatch_fetch_url()` from `agent/tools.rs` |
| Non-streaming tool request | New async-openai client setup | `run_agent_step_for_backend()` from `agent/loop.rs` |
| Tool call type accumulation | Manual streaming chunk stitching | Use non-streaming first round (avoids all accumulation complexity) |
| Per-conversation tool persistence | Settings table key | Migration V16 column on conversations table (matches `system_prompt` pattern) |

## Common Pitfalls

### Pitfall 1: dispatch_tools Inside Tokio Task
**What goes wrong:** `dispatch_web_search` and `dispatch_fetch_url` call `runtime.block_on()` internally. Calling `dispatch_tools` inside a `runtime.spawn()` task triggers "Cannot start a runtime from within a runtime" panic.
**Why it happens:** The async task is already running on the Tokio thread pool. Calling `block_on` from within a pool thread panics.
**How to avoid:** The async task sends `ChatToolCallsReady` to the actor loop. The actor thread (which is NOT a Tokio thread) calls `dispatch_tools` with its owned `&runtime`. This is exactly how the agent loop works: the `AgentStepComplete` handler in the actor loop calls `dispatch_tools`, not the async step task.
**Warning signs:** "Cannot start a runtime from within a runtime" panic at runtime.

### Pitfall 2: Tool Messages in UI Bubble List
**What goes wrong:** Pushing `ChatCompletionRequestToolMessage` or the assistant's `tool_calls` message into `AppState.messages` causes the UI to render internal protocol messages as visible chat bubbles.
**Why it happens:** `AppState.messages` is a display list, not a full conversation protocol history.
**How to avoid:** Keep the tool-augmented message history only in the `ChatToolCallsReady` event payload and in the spawned streaming task's `messages` argument. Never insert tool messages into SQLite or AppState.messages.

### Pitfall 3: tools_enabled Toggle When Backend Doesn't Support Tools
**What goes wrong:** If the conversation's backend has `supports_tool_use = false` (some custom backends), enabling chat tools will cause the API to return an error or silently ignore tools.
**Why it happens:** Not all OpenAI-compatible backends implement function calling.
**How to avoid:** In `do_send_message`, only use the tool-enabled path if both `conv_tools_enabled` AND `backend.supports_tool_use` are true. Show a toast if user enables tools but backend doesn't support them.

### Pitfall 4: Streaming Final Response Without Tool Results History
**What goes wrong:** The streaming follow-up request omits the assistant tool_calls message and tool result messages from history. The model has no context about what tools were called and produces incoherent output.
**Why it happens:** Easy to forget the intermediate messages when building the `chat_messages` vector for `spawn_streaming_task`.
**How to avoid:** The `ChatToolCallsReady` handler must construct the full message history: prior messages + assistant tool_calls message + tool result messages, then pass all of them to `spawn_streaming_task`.

### Pitfall 5: UniFFI Type Boundary for Tool Calls
**What goes wrong:** Trying to pass `Vec<ChatCompletionMessageToolCall>` across the UniFFI boundary (native → Rust) isn't possible; these types aren't UniFFI records.
**Why it happens:** Tool call state must stay entirely in Rust core.
**How to avoid:** The toggle (`tools_enabled: bool`) is the only thing that crosses the UniFFI boundary for this feature. All tool dispatch happens inside Rust.

## Code Examples

### Tool Subset Builder for Chat Context
```rust
// Source: modeled on build_agent_tools() in rust/src/agent/tools.rs
// Filter agent tools to the chat-appropriate subset
pub fn build_chat_tools(
    include_doc_search: bool,
    brave_api_key_set: bool,
) -> Vec<ChatCompletionTools> {
    let all = build_agent_tools();
    all.into_iter()
        .filter(|tool| {
            let name = match tool {
                ChatCompletionTools::Function(t) => t.function.name.as_str(),
            };
            match name {
                "finish" => false,  // not meaningful in chat context
                "search_documents" | "read_document" => include_doc_search,
                "web_search" => brave_api_key_set,
                _ => true, // calculate, fetch_url, file always included
            }
        })
        .collect()
}
```

### Migration V16
```rust
// rust/src/persistence/schema.rs
pub const MIGRATION_V16: &str = "
ALTER TABLE conversations ADD COLUMN tools_enabled INTEGER NOT NULL DEFAULT 0;
";
```

### AppAction Variant
```rust
// rust/src/lib.rs AppAction enum
/// Enable or disable tool use for a specific conversation.
/// Persisted in conversations.tools_enabled column.
SetConversationToolsEnabled { conversation_id: String, enabled: bool },
```

### ConversationSummary Extension
```rust
// rust/src/lib.rs ConversationSummary record
// Add field:
pub tools_enabled: bool,
```

### New InternalEvent Variants
```rust
// rust/src/llm/streaming.rs InternalEvent enum
/// Tool calls returned from non-streaming first round (Phase 27).
/// Actor thread dispatches the tool calls and spawns streaming follow-up.
ChatToolCallsReady {
    conv_id: String,
    tool_calls: Vec<async_openai::types::chat::ChatCompletionMessageToolCall>,
    /// Full message history up to and including the assistant tool_calls message.
    /// Actor appends tool results, then spawns streaming task with this history.
    messages_before_results: Vec<async_openai::types::chat::ChatCompletionRequestMessage>,
    backend_id: String,
    model: String,
},
```

### AppState.messages — Tool Call Indicator (Optional)
If the UI should show "Used web search, calculator" below the response bubble, add an optional field to `UiMessage`:
```rust
// Optional enhancement:
pub tools_used: Vec<String>,  // e.g. ["web_search", "calculate"]
```
This is not required for MVP — the response text makes tool use obvious.

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|------------------|-------|
| Streaming with tool chunk accumulation | Non-streaming first round + streaming final | Non-streaming is simpler; accumulation required for real-time tool call display (not needed here) |
| Per-provider tool schemas | OpenAI function calling standard | All 9 backends already support this protocol |

## Open Questions

1. **Which tools to include in chat subset?**
   - What we know: `finish` must be excluded; `file` raises sandboxing UX questions in chat context
   - What's unclear: Should `file` be included by default? Users may not expect file I/O from chat
   - Recommendation: Include `file` (sandbox is already set up); it's opt-in per conversation

2. **Show tool activity in the UI bubble?**
   - What we know: Agents show tool steps in a detail view; chat has no step view
   - What's unclear: Should chat show a brief "Searched the web..." indicator during the tool round?
   - Recommendation: Show `BusyState::Loading { message: "Running tools..." }` during the tool dispatch round, then transition to `Streaming` for the final answer — no persistent UI artifacts

3. **Multi-tool-round (tool calls → more tool calls)?**
   - What we know: Agent loop iterates up to N steps; chat tool round is single-step in this phase
   - Recommendation: Limit to exactly one tool round for Phase 27. Multi-round (agentic) behavior is the agent system's job.

4. **tools_enabled default for existing conversations?**
   - Migration V16 uses `DEFAULT 0` so all existing conversations start with tools disabled
   - This is correct: opt-in, not opt-out

## Environment Availability

Step 2.6: SKIPPED — Phase 27 is a pure code/config change. All dependencies (async-openai, rusqlite, evalexpr, scraper, reqwest) are already in Cargo.toml and working.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` |
| Config file | none (workspace-level cargo test) |
| Quick run command | `cargo test -p mango_core chat_tools 2>&1 \| tail -20` |
| Full suite command | `cargo test -p mango_core 2>&1 \| tail -40` |

### Phase Requirements → Test Map

Phase 27 requirements are TBD, but based on the feature scope, expected behaviors are:

| Behavior | Test Type | Automated Command | File Exists? |
|----------|-----------|-------------------|-------------|
| `tools_enabled` persists to conversations table | unit | `cargo test -p mango_core test_tools_enabled_persistence` | ❌ Wave 0 |
| `SetConversationToolsEnabled` action updates DB | unit | `cargo test -p mango_core test_set_conv_tools_enabled` | ❌ Wave 0 |
| `build_chat_tools` excludes `finish` | unit | `cargo test -p mango_core test_build_chat_tools` | ❌ Wave 0 |
| `build_chat_tools` excludes `web_search` when no brave key | unit | `cargo test -p mango_core test_chat_tools_no_brave` | ❌ Wave 0 |
| `ConversationSummary.tools_enabled` propagates from DB | unit | `cargo test -p mango_core test_conv_summary_tools_enabled` | ❌ Wave 0 |
| Migration V16 runs cleanly on existing DB | unit | `cargo test -p mango_core test_migration_v16` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p mango_core 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p mango_core 2>&1 | tail -40`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `rust/src/tests/chat_tools.rs` — covers tool subset builder, tools_enabled persistence, migration V16
- [ ] `rust/src/tests/mod.rs` — add `mod chat_tools;`

## Sources

### Primary (HIGH confidence)
- `rust/src/agent/tools.rs` (in-repo) — `build_agent_tools()`, `dispatch_tools()`, all dispatch functions verified
- `rust/src/agent/loop.rs` (in-repo) — `run_agent_step_for_backend()` reuse pattern verified
- `rust/src/llm/streaming.rs` (in-repo) — no tool handling present; `InternalEvent` extension point confirmed
- `rust/src/lib.rs` (in-repo) — `AppAction`, `ConversationSummary`, `ActorState`, `do_send_message`, `StreamDone` handler all inspected
- `rust/src/persistence/schema.rs` (in-repo) — migration V15 is latest; V16 slot is available; `system_prompt` column pattern confirmed
- async-openai 0.33.1 docs (docs.rs) — `ChatCompletionStreamResponseDelta.tool_calls`, `ChatCompletionMessageToolCallChunk` verified
- async-openai tool-call-stream example (github.com/64bit/async-openai) — tool chunk accumulation pattern and `FinishReason::ToolCalls` usage confirmed

### Secondary (MEDIUM confidence)
- `async-openai` GitHub examples tree — `tool-call` and `tool-call-stream` examples verified to exist
- OpenAI streaming docs — `FinishReason::ToolCalls` in streaming confirmed

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in use; no new deps
- Architecture: HIGH — non-streaming first round pattern is well-established and matches existing agent code
- Pitfalls: HIGH — runtime.block_on pitfall is verified by inspecting dispatch_tools source; tool message UI pitfall is verified by reading StreamDone handler

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable — async-openai 0.33.1 is pinned, no external service changes)
