# Roadmap: Confidential App

## Milestones

- ✅ **v1.0 MVP** - Phases 1-10 (shipped 2026-03-27)
- ✅ **v1.1 Mobile Embeddings** - Phase 11 (shipped 2026-03-27)
- ✅ **v1.2 Hardening & Test Coverage** - Phases 12-19 (shipped 2026-03-29)
- 🚧 **v2.0 Memory & Agents** - Phases 20-23 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-10) - SHIPPED 2026-03-27</summary>

- [x] **Phase 1: RMP Foundation** — Actor scaffold, UniFFI bindings, AppState skeleton, native shells
- [x] **Phase 2: Streaming LLM Client** — OpenAI-compatible streaming, backend config, error taxonomy
- [x] **Phase 3: Attestation Verification Core** — Intel TDX/DCAP, NVIDIA CC JWT, nonce binding, TTL cache
- [x] **Phase 4: Persistence Layer** — SQLite schemas, migration runner, platform keychain
- [x] **Phase 5: Chat UI + Conversation Management** — Streaming chat across all three platforms
- [x] **Phase 6: Backend Routing + Settings** — Failover chains, health tracking, settings screen
- [x] **Phase 7: Onboarding Wizard** — First-run wizard with live attestation demo
- [x] **Phase 8: Local On-Device RAG** — On-device embedding, HNSW index, context injection
- [x] **Phase 9: Agent System + Background Execution** — Persisted agents, tool use, background execution
- [x] **Phase 10: PPQ.AI Backend Integration** — AMD SEV-SNP attestation, private model filtering

</details>

<details>
<summary>✅ v1.1 Mobile Embeddings (Phase 11) - SHIPPED 2026-03-27</summary>

- [x] **Phase 11: Mobile ONNX Embedding Pipeline** — Real CoreML/XNNPACK embedding on iOS and Android

</details>

<details>
<summary>✅ v1.2 Hardening & Test Coverage (Phases 12-19) - SHIPPED 2026-03-29</summary>

- [x] **Phase 12: ORT Pin & Stability** — ONNX Runtime version pinning and build stability
- [x] **Phase 13: Panic Elimination** — Remove unwrap/expect from production paths
- [x] **Phase 14: HPKE Key Hygiene** — Key lifecycle and zeroization improvements
- [x] **Phase 15: Embedding Graceful Degradation** — Fallback when embedding model unavailable
- [x] **Phase 16: Rate Limiting & 429 Backoff** — Exponential backoff with provider retry hints
- [x] **Phase 17: Backend Capability Config** — Per-backend feature flags and capability negotiation
- [x] **Phase 18: TEE Runtime Configuration** — Dynamic TEE type configuration at runtime
- [x] **Phase 19: Test Coverage Gaps** — Fill critical test coverage gaps across core modules

</details>

### v2.0 Memory & Agents (In Progress)

**Milestone Goal:** Add persistent cross-conversation memory with automatic fact extraction, and expand the agent system with real-world tools (web search, URL fetching, file operations, calculator).

- [x] **Phase 20: Memory Core** - Rust memory module with SQLite schema, LLM-driven extraction, and background execution (completed 2026-04-03)
- [x] **Phase 21: Memory Retrieval & Injection** - Semantic search over memories and injection into conversation context (completed 2026-04-04)
- [x] **Phase 22: Agent Tools Expansion** - Brave Search, URL fetch, file operations, and calculator tools in ReAct loop (completed 2026-04-04)
- [x] **Phase 23: Memory Management UI + Agent UI** - Memory view/edit/delete screens and agent UI re-enable on all platforms (completed 2026-04-04)

## Phase Details

### Phase 20: Memory Core
**Goal**: The app automatically extracts and stores facts, preferences, and entities from completed conversations as local on-device memories
**Depends on**: Phase 19 (existing RAG + persistence infrastructure)
**Requirements**: MEM-01, MEM-02, MEM-07
**Success Criteria** (what must be TRUE):
  1. After a conversation ends, the app automatically triggers memory extraction without user action
  2. Extracted memories appear in SQLite with text content and usearch vector embeddings
  3. Memory extraction runs in a background task and does not block or delay chat responsiveness
  4. Memory extraction uses the existing EmbeddingProvider trait and usearch index infrastructure
  5. Memories survive app restart and are queryable from the Rust core
**Plans:** 2/2 plans complete
Plans:
- [x] 20-01-PLAN.md — Memory module, migration V15, persistence queries, and unit tests
- [x] 20-02-PLAN.md — Wire extraction into actor loop (StreamDone hook + MemoryExtractionComplete handler)

### Phase 21: Memory Retrieval & Injection
**Goal**: Relevant memories from past conversations are automatically surfaced and injected into new conversation system prompts
**Depends on**: Phase 20
**Requirements**: MEM-03
**Success Criteria** (what must be TRUE):
  1. When a new conversation starts, the system performs semantic search over stored memories
  2. Top-N relevant memories appear in the system prompt without user configuration
  3. Memory injection uses the same context injection pathway as RAG document context
  4. Conversations with no relevant memories proceed normally with no injection artifacts
**Plans:** 1/1 plans complete
Plans:
- [x] 21-01-PLAN.md — Memory retrieval module, persistence query, and do_send_message injection wiring

### Phase 22: Agent Tools Expansion
**Goal**: Agents can search the web, read URLs, manipulate files, and perform precise math — all integrated into the existing ReAct loop with step checkpointing
**Depends on**: Phase 19 (existing agent ReAct loop)
**Requirements**: TOOL-01, TOOL-02, TOOL-03, TOOL-04, TOOL-05
**Success Criteria** (what must be TRUE):
  1. Agent can execute a web search via Brave Search API and incorporate results into its reasoning
  2. Agent can fetch a URL and read its text content (HTML stripped) as a tool result
  3. Agent can create, read, and edit files within the app sandbox directory
  4. Agent can evaluate a mathematical expression and return a precise numeric result
  5. All four tools appear in the existing tool dispatch registry and their steps are checkpointed to SQLite
**Plans:** 2/2 plans complete
Plans:
- [x] 22-01-PLAN.md — Add scraper/evalexpr deps, implement 4 tool schemas and dispatch functions
- [x] 22-02-PLAN.md — Wire dispatch_tools into lib.rs (ActorState.data_dir, call sites, system prompts)

### Phase 23: Memory Management UI + Agent UI
**Goal**: Users can view, edit, and delete their stored memories through a dedicated screen, and the agent system with its expanded tools is fully accessible on all platforms
**Depends on**: Phase 21, Phase 22
**Requirements**: MEM-04, MEM-05, MEM-06, AUI-01, AUI-02
**Success Criteria** (what must be TRUE):
  1. User can navigate to a memory management screen and see a list of all stored memories
  2. User can delete a single memory and it is removed from both SQLite and the usearch index
  3. User can tap a memory to edit its text and save the correction
  4. Agent UI is accessible on iOS, Android, and Desktop with the expanded tool set listed
  5. Agent session detail view shows each tool call step with tool name, input, and output
**Plans:** 3/3 plans complete
Plans:
- [x] 23-01-PLAN.md — Rust core: MemorySummary, Screen::Memories, AppAction variants, actor handlers, AgentStepSummary.tool_input
- [x] 23-02-PLAN.md — Memory management UI screens on iOS, Android, Desktop with navigation wiring
- [x] 23-03-PLAN.md — Re-enable agent navigation and enhance step display with tool_input on all platforms
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 20 → 21 → 22 → 23

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 20. Memory Core | v2.0 | 2/2 | Complete | 2026-04-03 |
| 21. Memory Retrieval & Injection | v2.0 | 1/1 | Complete    | 2026-04-04 |
| 22. Agent Tools Expansion | v2.0 | 2/2 | Complete    | 2026-04-04 |
| 23. Memory Management UI + Agent UI | v2.0 | 3/3 | Complete    | 2026-04-04 |

### Phase 24: Redesign Settings UX — move memories into settings, redesign layout with grouped sections, add tool configuration for agents and chats

**Goal:** Settings screen redesigned with grouped sections (PROVIDERS/DEFAULTS/MEMORY/TOOLS/APPEARANCE/Advanced), Memories entry point moved from home toolbar into Settings, and Brave Search API key configurable via Tools section -- all on iOS, Android, and Desktop
**Requirements**: SET-01, SET-02, SET-03, SET-04, SET-05, SET-06, SET-07
**Depends on:** Phase 23
**Plans:** 3/3 plans complete

Plans:
- [x] 24-00-PLAN.md — Wave 0: unit test stubs for SET-04 (brave_api_key persistence) and SET-06 (memory_count)
- [x] 24-01-PLAN.md — Rust core: memory_count + brave_api_key_set in AppState, SetBraveApiKey action, memory_count updates in handlers
- [ ] 24-02-PLAN.md — Add MEMORY + TOOLS sections to Settings on all 3 platforms, remove Memories from home toolbars
**UI hint**: yes

### Phase 25: disable/enable making memories in the app

**Goal:** User can toggle automatic memory extraction on/off via a switch in the Settings MEMORY section, persisted across app restarts, defaulting to enabled
**Requirements**: MEM-TOGGLE-01, MEM-TOGGLE-02, MEM-TOGGLE-03, MEM-TOGGLE-04
**Depends on:** Phase 24
**Plans:** 2 plans

Plans:
- [ ] 25-01-PLAN.md — Rust core: memories_enabled in AppState, SetMemoriesEnabled action, startup load, extraction gate in StreamDone, unit test
- [ ] 25-02-PLAN.md — Regenerate UniFFI bindings, add toggle to iOS/Android/Desktop Settings MEMORY sections
**UI hint**: yes
