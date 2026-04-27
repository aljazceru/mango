# Roadmap: Confidential App

## Milestones

- ✅ **v1.0 MVP** - Phases 1-10 (shipped 2026-03-27)
- ✅ **v1.1 Mobile Embeddings** - Phase 11 (shipped 2026-03-27)
- ✅ **v1.2 Hardening & Test Coverage** - Phases 12-19 (shipped 2026-03-29)
- 🚧 **v2.0 Memory & Agents** - Phases 20-30 (in progress)

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
- [x] **Phase 24: Redesign Settings UX** - Grouped settings sections, Memories in Settings, Brave Search API key in Tools on all platforms (completed 2026-04-05)
- [x] **Phase 25: disable/enable making memories** - Toggle in Settings MEMORY section, persisted, defaults enabled (completed 2026-04-05)
- [x] **Phase 26: settings submenus** - Providers and Defaults as tappable sub-screens, reduced scroll depth on all platforms (completed 2026-04-05)
- [x] **Phase 27: Add optional tool use to chat** - Per-conversation tools toggle, non-streaming tool detection, streaming final response on all platforms (completed 2026-04-07)
- [x] **Phase 28: Local Data Encryption & Authentication** - AES-256-GCM encryption, biometric/PIN unlock, duress PIN wipe on all platforms (completed 2026-04-09)
- [x] **Phase 29: Wire VectorIndex DEK End-to-End** - DEK wired from auth handlers through ActorState to all VectorIndex call sites (completed 2026-04-09)
- [x] **Phase 30: Milestone Verification & Requirements Sync** - Close MEM-03 orphan, regenerate UniFFI bindings, sync REQUIREMENTS.md checkboxes (in progress) (completed 2026-04-20)
- [x] **Phase 31: Multimodal image attachments** - Camera/gallery on iOS+Android, file picker on Desktop, base64 data URL encoding to LLM (completed 2026-04-19)
- [ ] **Phase 32: Directory-based RAG ingestion** - Directory sources with periodic sync, glob exclusions, cross-platform folder permissions (in progress)
- [ ] **Phase 34: Integrate Redpill (api.redpill.ai) as TEE-attested LLM aggregator** - Three response shapes (Phala-flat / Phala-orchestrated 3-quote / Chutes), reusing Venice REPORTDATA decoder, no new crates

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
| 24. Redesign Settings UX | v2.0 | 3/3 | Complete | 2026-04-05 |
| 25. disable/enable making memories | v2.0 | 2/2 | Complete | 2026-04-05 |
| 26. settings submenus | v2.0 | 3/3 | Complete | 2026-04-05 |
| 27. Add optional tool use to chat | v2.0 | 4/4 | Complete | 2026-04-07 |
| 28. Local Data Encryption & Authentication | v2.0 | 8/8 | Complete | 2026-04-09 |
| 29. Wire VectorIndex DEK End-to-End | v2.0 | 1/1 | Complete   | 2026-04-09 |
| 30. Milestone Verification & Requirements Sync | v2.0 | 1/1 | Complete    | 2026-04-20 |
| 31. Multimodal image attachments | v2.0 | 6/6 | Complete | 2026-04-19 |
| 32. Directory-based RAG ingestion | v2.0 | 7/9 | In Progress | — |

### Phase 24: Redesign Settings UX — move memories into settings, redesign layout with grouped sections, add tool configuration for agents and chats

**Goal:** Settings screen redesigned with grouped sections (PROVIDERS/DEFAULTS/MEMORY/TOOLS/APPEARANCE/Advanced), Memories entry point moved from home toolbar into Settings, and Brave Search API key configurable via Tools section -- all on iOS, Android, and Desktop
**Requirements**: SET-01, SET-02, SET-03, SET-04, SET-05, SET-06, SET-07
**Depends on:** Phase 23
**Plans:** 3/3 plans complete

Plans:
- [x] 24-00-PLAN.md — Wave 0: unit test stubs for SET-04 (brave_api_key persistence) and SET-06 (memory_count)
- [x] 24-01-PLAN.md — Rust core: memory_count + brave_api_key_set in AppState, SetBraveApiKey action, memory_count updates in handlers
- [x] 24-02-PLAN.md — Add MEMORY + TOOLS sections to Settings on all 3 platforms, remove Memories from home toolbars
**UI hint**: yes

### Phase 25: disable/enable making memories in the app

**Goal:** User can toggle automatic memory extraction on/off via a switch in the Settings MEMORY section, persisted across app restarts, defaulting to enabled
**Requirements**: MEM-TOGGLE-01, MEM-TOGGLE-02, MEM-TOGGLE-03, MEM-TOGGLE-04
**Depends on:** Phase 24
**Plans:** 2/2 plans complete

Plans:
- [x] 25-01-PLAN.md — Rust core: memories_enabled in AppState, SetMemoriesEnabled action, startup load, extraction gate in StreamDone, unit test
- [x] 25-02-PLAN.md — Regenerate UniFFI bindings, add toggle to iOS/Android/Desktop Settings MEMORY sections
**UI hint**: yes

### Phase 26: settings submenus and organization — group related settings into collapsible sections or sub-screens

**Goal:** Providers and Defaults settings sections reorganized into dedicated sub-screens accessible via tappable summary rows on the main Settings screen, reducing scroll depth and matching platform-native Settings app patterns -- on iOS, Android, and Desktop
**Requirements**: TBD
**Depends on:** Phase 25
**Plans:** 3/3 plans complete

Plans:
- [x] 26-01-PLAN.md — Add SettingsProviders + SettingsDefaults Screen variants to Rust, regenerate UniFFI bindings
- [x] 26-02-PLAN.md — iOS + Android: extract Providers/Defaults into sub-screens, add summary rows and routing
- [x] 26-03-PLAN.md — Desktop: extract Providers/Defaults into sub-screen view modules, add summary rows and routing
**UI hint**: yes

### Phase 27: Add optional tool use to chat

**Goal:** Chat conversations can optionally use LLM tool calling (web search, URL fetch, calculator, file ops) via a per-conversation toggle, reusing agent tool infrastructure with a non-streaming first round for tool detection and streaming follow-up for the final response
**Requirements**: CHAT-TOOL-01, CHAT-TOOL-02, CHAT-TOOL-03, CHAT-TOOL-04, CHAT-TOOL-05, CHAT-TOOL-06, CHAT-TOOL-07, CHAT-TOOL-08
**Depends on:** Phase 26
**Plans:** 4/4 plans complete

Plans:
- [x] 27-00-PLAN.md — Wave 0: test stubs for migration, persistence, and tool subset builder
- [x] 27-01-PLAN.md — Migration V16, persistence queries, build_chat_tools, SetConversationToolsEnabled action, unit tests
- [x] 27-02-PLAN.md — InternalEvent variants, spawn_chat_tool_round, do_send_message tool branch, ChatToolCallsReady handler
- [x] 27-03-PLAN.md — UniFFI bindings regeneration, tools toggle UI on iOS/Android/Desktop
**UI hint**: yes

### Phase 28: Local Data Encryption & Authentication

**Goal:** All local data (SQLite database, usearch vector indices, cached documents) is encrypted at rest using platform hardware capabilities. Users authenticate via biometrics or PIN/password to unlock. Duress PIN triggers full data wipe. Graceful degradation on devices without biometric hardware. All three platforms: iOS, Android, Desktop.
**Requirements**: ENC-01, ENC-02, ENC-03, ENC-04, ENC-05, ENC-06, ENC-07, ENC-08, ENC-09, ENC-10, ENC-11, ENC-12, ENC-13, ENC-14
**Depends on:** Phase 27
**Plans:** 8/8 plans complete

Plans:
- [x] 28-01-PLAN.md — Crypto foundation: SQLCipher, AES-256-GCM file encryption, Argon2id key derivation, bootstrap DB
- [x] 28-02-PLAN.md — Actor restructure: Screen::Locked, BiometricProvider, deferred DB open, auth AppActions
- [x] 28-03-PLAN.md — Encrypted VectorIndex save/load with DEK
- [x] 28-04-PLAN.md — iOS: UniFFI bindings, BiometricProviderImpl, LockScreen, PinSetupScreen
- [x] 28-05-PLAN.md — Android: BiometricProviderImpl (BiometricPrompt+CountDownLatch), LockScreen, PinSetupScreen
- [x] 28-06-PLAN.md — Desktop: PIN-only LockScreen, PinSetupScreen
- [x] 28-07-PLAN.md — Background lock timeout on all platforms, lock timeout Settings picker
**UI hint**: yes

### Phase 29: Wire VectorIndex DEK End-to-End

**Goal:** Wire the Data Encryption Key (DEK) from authentication handlers through ActorState to all VectorIndex call sites, so usearch vector index files are actually encrypted at rest — closing the gap between the AES-256-GCM capability built in Phase 28-03 and its runtime invocation
**Requirements**: ENC-02
**Depends on:** Phase 28
**Gap Closure:** Closes ENC-02 partial, VectorIndex DEK integration gap, encryption E2E flow gap from v2.0 milestone audit
**Plans:** 1/1 plans complete

Plans:
- [x] 29-01-PLAN.md — DEK field in ActorState, auth handler wiring, save call site plumbing

### Phase 30: Milestone Verification & Requirements Sync

**Goal:** Close the MEM-03 orphan by verifying Phase 21, regenerate UniFFI bindings for biometric_authenticated field, and sync all stale REQUIREMENTS.md checkboxes and traceability entries to reflect verified implementation status
**Requirements**: MEM-03 (verification), ENC-09 (UX bindings)
**Depends on:** Phase 29
**Gap Closure:** Closes MEM-03 orphan, ENC-09 UX gap, stale documentation from v2.0 milestone audit
**Plans:** 1/1 plans complete

Plans:
- [x] 30-01-PLAN.md — Phase 21 VERIFICATION.md + REQUIREMENTS.md ENC-02/ENC-09 sync

### Phase 31: Multimodal image attachments across all platforms — extend Rust core AttachmentInfo to carry image bytes/URI + MIME, wire vision-capable image_url parts into OpenAI-compatible chat completions (base64 data URLs), update UniFFI bindings. Android: camera capture via FileProvider + TakePicture and gallery picker via PickVisualMedia. iOS: UIImagePickerController camera + photo library with privacy usage strings. Desktop (iced): native file picker scoped to image MIME types. All platforms send photos through the updated AttachmentInfo pipeline so the model actually sees the image, not a placeholder.

**Goal:** Users can attach photos from camera or gallery on Android, from camera or photo library on iOS, and via native file picker on desktop; the Rust core encodes each image as a base64 data URL and sends a multipart ChatCompletionRequestUserMessageContent::Array so the model actually sees the image
**Requirements**: IMG-01, IMG-02, IMG-03, IMG-04, IMG-05, IMG-06
**Depends on:** Phase 30
**Plans:** 6/6 plans complete

Plans:
- [x] 31-00-PLAN.md — Wave 0: add image 0.25 dep + 4 failing unit tests (RED) pinning IMG-01..04
- [x] 31-01-PLAN.md — Rust core: AttachImage action, PendingImageAttachment, prepare_image_for_api, multipart builder, do_send_message branch
- [x] 31-02-PLAN.md — Regenerate UniFFI bindings (iOS Bindings/ + Android kt)
- [x] 31-03-PLAN.md — Android camera (FileProvider + TakePicture) + gallery (PickVisualMedia) + action sheet
- [x] 31-04-PLAN.md — iOS UIImagePickerController + PhotosPicker + Info.plist usage strings + action sheet
- [x] 31-05-PLAN.md — Desktop iced: rfd image filter + AttachImage dispatch branch

### Phase 32: Directory-based RAG ingestion with periodic sync and file/folder exclusion. Users can add a whole directory (e.g. an Obsidian vault / PKB synced to the device) as a RAG source instead of ingesting files one-by-one. Support excluding specific subdirectories and files (glob patterns). Tracked sources are periodically re-synced so added/modified/deleted files in the source folder are reflected in the index automatically (e.g. when the Obsidian vault is updated via git sync). Must work on iOS (security-scoped folder bookmarks), Android (persistable ACTION_OPEN_DOCUMENT_TREE URIs) and Desktop. UI: source list with last-synced time, add/remove source, exclusion editor, manual sync-now.

**Goal:** Users can add a whole directory (e.g. an Obsidian vault) as a RAG source with glob-based exclusion patterns, automatic incremental re-sync across launches (added/modified/deleted files picked up via mtime+size fingerprints), and cross-platform folder-permission lifecycle (iOS security-scoped bookmarks, Android persistable SAF tree URIs, Desktop paths with notify watcher + 5-min fallback). UI delivers source list with relative last-synced time, add/remove source with confirmation, exclusion editor with live validation, and manual Sync Now.
**Requirements**: DIR-01, DIR-02, DIR-03, DIR-04, DIR-05, DIR-06
**Depends on:** Phase 31
**Plans:** 7/7 plans complete

Plans:
- [x] 32-01-PLAN.md — Migration V18 (directory_sources + directory_files) + CRUD queries
- [x] 32-02-PLAN.md — Directory walk + exclusion globs (ignore crate) + diff_files algorithm
- [x] 32-03-PLAN.md — 6 AppActions + actor handlers (sync pipeline with 50-file batching, cascaded removal, UniFFI)
- [x] 32-04-PLAN.md — Desktop iced UI + notify watcher + PollWatcher fallback + 5-min Tokio interval
- [x] 32-05-PLAN.md — iOS SwiftUI + UIDocumentPickerViewController + .minimalBookmark lifecycle + ScenePhase sync + iCloud placeholder skip
- [x] 32-06-PLAN.md — Android Compose + OpenDocumentTree + takePersistableUriPermission + bulk DocumentsContract traversal + WorkManager 15-min + onResume sync
- [x] 32-07-PLAN.md — Cross-platform UX polish: relative-time labels, sync-status pills, settings entry, reused IngestionProgress

### Phase 33: Integrate Venice.ai as TEE-attested LLM provider with client-side TDX + NVIDIA NRAS verification and ECDH+AES-GCM E2EE handshake

**Goal:** Venice.ai is selectable as a third TEE-attested LLM provider; every chat completion goes through verified TDX+NRAS attestation and a per-request ECDH(secp256k1)+HKDF+AES-256-GCM E2EE channel rooted in the attested signing key. No TLS pinning. Attestation failures fail-closed.
**Requirements**: VEN-01, VEN-02, VEN-03, VEN-04, VEN-05, VEN-06, VEN-07, VEN-08, VEN-09
**Depends on:** Phase 32
**Plans:** 4 plans

Plans:
- [x] 33-01-PLAN.md — Wave 0: REQUIREMENTS+Cargo deps+golden fixture+failing test stubs+MRSEAM reconcile
- [x] 33-02-PLAN.md — Attestation layer: TDX layout enum + attestation/venice.rs (REPORTDATA decoder + cache + verify orchestrator)
- [x] 33-03-PLAN.md — llm/venice.rs: ECDH+HKDF+AES-GCM E2EE crypto + HTTP transport + text-SSE streaming
- [x] 33-04-PLAN.md — Wiring (transport.rs, backend.rs, mod.rs) + integration tests + live #[ignore] test

### Phase 34: Integrate Redpill (api.redpill.ai) as TEE-attested LLM aggregator with client-side TDX + NVIDIA NRAS verification across three response shapes

**Goal:** Redpill is selectable as a fourth TEE-attested LLM provider; every chat completion goes through fully verified TDX (and NRAS where present) attestation, with the client correctly dispatching the three Redpill response shapes (Phala-flat, Phala-orchestrated 3-quote, Chutes anti-tamper) and reusing the Venice REPORTDATA decoder for the model component. No new Rust crates. Attestation failures fail-closed. Redpill→Tinfoil routes are explicitly unsupported until Redpill upstream upgrades its relay.
**Requirements**: RED-01, RED-02, RED-03, RED-04, RED-05, RED-06, RED-07, RED-08, RED-09, RED-10, RED-11
**Depends on:** Phase 33
**Plans:** 4 plans
Plans:
- [x] 34-01-PLAN.md — Wave 0: golden fixtures + RED test stubs + REQUIREMENTS sanity + zero-new-deps verification
- [x] 34-02-PLAN.md — Attestation layer: attestation/redpill.rs (shape dispatcher + 4 REPORTDATA decoders + quote_bytes helper + debug-mode gate + verify orchestrator + cache integration)
- [x] 34-03-PLAN.md — LLM transport: llm/redpill.rs (HTTP fetch, OpenAI-compatible chat completions, Tinfoil-route refusal); ProviderKind::Redpill + preset
- [x] 34-04-PLAN.md — Wiring: transport/router/streaming dispatch + UniFFI bindings + Settings preset surfacing + verified-badge with shape breakdown + live integration tests
**Success Criteria** (what must be TRUE):
  1. Redpill appears as a known provider preset in Add Backend on all three platforms
  2. The client fetches `GET https://api.redpill.ai/v1/attestation/report?model=&nonce=` without an API key and the response is verified end-to-end before any chat request is sent
  3. The client correctly identifies the response shape (Flat / Orchestrated / Chutes) and applies the right REPORTDATA layout decoder(s) for each component
  4. For Orchestrated responses, all three TDX quotes (gateway + model + compose-manager) verify before the session opens; failure of any one fails the whole attestation
  5. TDX quote signature, PCK chain, TCB, and CRLs are verified locally via `dcap-qvl` against fresh Intel collateral — never via Phala's hosted verifier
  6. NVIDIA GPU attestation (`nvidia_payload` for Shapes A/B; `gpu_evidence[]` for Shape C) is verified through the existing `attestation/nvidia.rs` NRAS path
  7. TDX debug-mode bit is rejected (`td_attributes[0] & 0x01 != 0` fails closed)
  8. Chutes-routed models display "freshness valid for enclave lifetime" in the trust UI; Flat and Orchestrated display per-request freshness
  9. Tinfoil-routed Redpill models surface a clear error pointing the user to the existing direct-Tinfoil integration
 10. End-to-end live integration test (`#[ignore]`) passes against `api.redpill.ai` for at least one model per supported shape
**Source spike:** `.planning/spikes/002-redpill-tee-verification-research/` (VALIDATED 2026-04-26) — captures usable as golden fixtures
**Findings skill:** `Skill("spike-findings-confidential-app")` → `references/redpill-attestation.md`

### Phase 34.1: close RED-09 and RED-11 actor-loop drop — plumb shape/freshness/orchestrated_components through UniFFI AttestationStatus, AttestationRecord, and native trust UI (INSERTED)

**Goal:** Plumb shape/freshness/orchestrated_components from AttestationEvent::Verified through the actor-loop, UniFFI AttestationStatus (struct-variant promotion + new OrchestratedComponent record), SQLite V19 cache columns, and native trust UI on Android/iOS/Desktop iced — closing the RED-09 (PerEnclave freshness sub-line) and RED-11 (Orchestrated three-quote breakdown) PARTIAL gaps from Phase 34
**Requirements**: RED-09, RED-11 (downstream wiring; cryptographic verification already complete in Phase 34)
**Depends on:** Phase 34
**Plans:** 7 plans

Plans:
- [ ] 34.1-01-PLAN.md — Actor-loop wiring: extract map_event_to_record_and_status helper, extend AttestationRecord with three new Option fields, stop dropping the values, unit test threading per shape
- [ ] 34.1-02-PLAN.md — UniFFI AttestationStatus::Verified promoted to struct variant + OrchestratedComponent record + compile-driven sweep across ~15 Rust sites + regenerate Kotlin/Swift bindings
- [ ] 34.1-03-PLAN.md — SQLite MIGRATION_V19 (three nullable TEXT columns) + AttestationCache put/get extensions + round-trip and pre-V19 backward-compat tests
- [ ] 34.1-04-PLAN.md — Android: render freshness + orchestrated breakdown sub-lines in SettingsProvidersScreen.kt (UI-SPEC locked copy); compile-fix sweep in AttestationBadge/ChatScreen/OnboardingScreen
- [ ] 34.1-05-PLAN.md — iOS: mirror in SettingsProvidersView.swift; compile-fix sweep in AttestationBadgeView/AppColors/ChatView/ModelPickerView/OnboardingView
- [ ] 34.1-06-PLAN.md — Desktop iced: mirror in views/settings_providers.rs; compile-fix sweep in widgets/attestation_badge / views/chat / views/onboarding
- [ ] 34.1-07-PLAN.md — Cleanup: confirm stale comment is gone, update Phase 34 SUMMARY with FULL closure note, update STATE.md
**UI hint**: yes
