---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Memory & Agents
status: executing
stopped_at: Completed 32-07-PLAN.md
last_updated: "2026-04-19T18:59:11.318Z"
last_activity: 2026-04-19 -- Phase 32 planning complete
progress:
  total_phases: 13
  completed_phases: 11
  total_plans: 44
  completed_plans: 42
  percent: 95
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-02)

**Core value:** Every inference request is provably confidential -- verified via remote attestation, all data stays local
**Current focus:** Phase 32 — directory-based-rag-ingestion-with-periodic-sync-and-file-fo

## Current Position

Phase: 32 (directory-based-rag-ingestion-with-periodic-sync-and-file-fo) — EXECUTING
Plan: 7 of 7
Status: Ready to execute
Last activity: 2026-04-19 -- Phase 32 planning complete

Progress: [██░░░░░░░░] 25%

## Performance Metrics

**Velocity:**

- Total plans completed: 6
- Average duration: --
- Total execution time: --

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 31 | 6 | - | - |

**Recent Trend:**

- Last 5 plans: --
- Trend: --

*Updated after each plan completion*
| Phase 21-memory-retrieval-injection P01 | 4 | 2 tasks | 5 files |
| Phase 22-agent-tools-expansion P01 | 8min | 2 tasks | 4 files |
| Phase 22-agent-tools-expansion P02 | 5min | 1 tasks | 1 files |
| Phase 23-memory-management-ui-agent-ui P01 | 12 | 2 tasks | 4 files |
| Phase 23-memory-management-ui-agent-ui P02 | 7min | 2 tasks | 8 files |
| Phase 23-memory-management-ui-agent-ui P03 | 8min | 2 tasks | 9 files |
| Phase 24-redesign-settings-ux P00 | 3min | 1 tasks | 1 files |
| Phase 24-redesign-settings-ux P01 | 8min | 1 tasks | 2 files |
| Phase 24-redesign-settings-ux P02 | 15min | 2 tasks | 8 files |
| Phase 25-disable-enable-making-memories-in-the-app P01 | 2min | 1 tasks | 2 files |
| Phase 25-disable-enable-making-memories-in-the-app P25-02 | 15min | 2 tasks | 9 files |
| Phase 26 P01 | 4min | 1 tasks | 3 files |
| Phase 26 P02 | 7min | 2 tasks | 8 files |
| Phase 26 P03 | 15min | 1 tasks | 5 files |
| Phase 27-add-optional-tool-use-to-chat P00 | 5min | 1 tasks | 2 files |
| Phase 27 P01 | 10min | 2 tasks | 8 files |
| Phase 27-add-optional-tool-use-to-chat P03 | 20min | 1 tasks | 8 files |
| Phase 32 P01 | 12min | 2 tasks | 5 files |
| Phase 32 P02 | 4min | 3 tasks | 3 files |
| Phase 32 P03 | 10min | 2 tasks | 3 files |
| Phase 32 P04 | 30min | 3 tasks | 6 files |
| Phase 32 P05 | 16min | 3 tasks | 7 files |
| Phase 32 P06 | 12min | 3 tasks | 6 files |
| Phase 32 P07 | 22min | 2 tasks | 10 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.

Key architectural context for v2.0:

- Memory system reuses EmbeddingProvider trait + usearch HNSW index from Phase 8 RAG
- Memory extraction uses LLM call (same OpenAI-compatible client) on conversation completion
- Agent tools integrate with existing tool dispatch in rust/src/agent/
- Agent UI was hidden in quick task 260326-pgd -- Phase 23 re-enables it
- [Phase 21-memory-retrieval-injection]: Reuse shared usearch HNSW index for memory search; chunk keys silently fall through via get_memory_content_by_usearch_keys returning empty
- [Phase 21-memory-retrieval-injection]: Hoist query embedding before RAG and memory blocks so embed() is called once per message
- [Phase 22-agent-tools-expansion]: pub(crate) visibility for dispatch functions enables direct testing; empty-string sentinel for brave_api_key/data_dir disables tools gracefully
- [Phase 22-agent-tools-expansion]: Fetch brave_api_key fresh from settings DB at each dispatch_tools call to pick up key changes without restart
- [Phase 22-agent-tools-expansion]: ActorState.data_dir initialized from vector_data_dir.clone() - agent file sandbox shares app data directory with RAG index
- [Phase 23-memory-management-ui-agent-ui]: update_memory does NOT re-embed vectors (v1 simplification -- stale HNSW entry acceptable, re-embedding deferred)
- [Phase 23-memory-management-ui-agent-ui]: load_memory_summaries helper extracted to avoid duplicating mapping logic between PushScreen::Memories and ListMemories handlers
- [Phase 23-memory-management-ui-agent-ui]: Desktop uses typed Message variants (MemoryConfirmDelete/MemorySaveEdit) for memory lifecycle; handlers dispatch AppAction and clear memory_edit_state atomically
- [Phase 23-memory-management-ui-agent-ui]: memory_edit_state: Option<(String, String)> in App::Loaded follows established edit_state pattern for chat message editing
- [Phase 23-memory-management-ui-agent-ui]: Agent navigation re-enabled on all three platforms by removing AGENTS HIDDEN guards and restoring routing code
- [Phase 23-memory-management-ui-agent-ui]: final_answer steps skip tool name/input display entirely and show full resultSnippet as primary content (D-08)
- [Phase 24-redesign-settings-ux]: MemoryRow.usearch_key field required in persistence test stubs -- plan interface snippet was incomplete, set to 1 as placeholder integer
- [Phase 24-redesign-settings-ux]: memory_count is re-queried from DB after each mutation rather than in-memory arithmetic to avoid off-by-one errors
- [Phase 24-redesign-settings-ux]: brave_api_key_set bool exposes key presence only, never raw key, across UniFFI boundary (per D-11)
- [Phase 24-redesign-settings-ux]: UniFFI bindings regenerated in Wave 2 plan (not Wave 1) — Rust AppState changes from 24-01 not propagated to Kotlin/Swift binding files; regenerated via 'just bindings-kotlin' and 'just bindings-swift'
- [Phase 24-redesign-settings-ux]: ios/Bindings/ directory committed to repo so Xcode build picks up updated AppState (memoryCount, braveApiKeySet) without requiring local bindings regeneration
- [Phase 25-disable-enable-making-memories-in-the-app]: memories_enabled defaults to true via unwrap_or(true) so existing users are unaffected on upgrade
- [Phase 25-disable-enable-making-memories-in-the-app]: Extraction gate placed as outermost condition before should_extract in StreamDone, not nested inside bid block
- [Phase 25-disable-enable-making-memories-in-the-app]: memories_enabled persisted as '0'/'1' string in settings table consistent with other settings entries
- [Phase 25-disable-enable-making-memories-in-the-app]: Merge main into worktree before regenerating bindings -- worktree was behind main after plan 25-01 landed
- [Phase 26]: SettingsProviders and SettingsDefaults added as unit variants (no associated data) matching pattern of existing navigation screens like Agents and Memories
- [Phase 26]: Desktop sub-screen params stripped from settings::view() and threaded from main.rs directly to sub-screen view calls; custom provider form moved from Advanced section to settings_providers.rs sub-screen
- [Phase 26]: Helper functions duplicated into sub-screen files (not shared util) to keep each screen self-contained for v1
- [Phase 26]: Android provider helpers use Providers suffix in name to avoid package-level collision after SettingsScreen helpers removed
- [Phase 27-add-optional-tool-use-to-chat]: Wave 0 stubs intentionally do not compile until Plan 01 adds tools_enabled field, update_conversation_tools_enabled, and build_chat_tools
- [Phase 27]: ChatCompletionTools::Custom variant handled with pass-through in build_chat_tools filter for forward-compatibility
- [Phase 27]: LoadConversation uses list_conversations lookup for tools_enabled rather than a dedicated single-row query
- [Phase 27-add-optional-tool-use-to-chat]: iOS ChatView receives onSetToolsEnabled callback; ContentView.swift wires dispatch to setConversationToolsEnabled (consistent with callback pattern)
- [Phase 27-add-optional-tool-use-to-chat]: Android onDispatchAction: (AppAction) -> Unit threaded through ChatScreen -> ChatTopBar to avoid per-action callback proliferation (27-03)
- [Phase 27-add-optional-tool-use-to-chat]: Desktop Tools button uses accent background when active showing Tools [ON], surface background when off - consistent with docs_btn visual pattern (27-03)
- [Phase quick/260419-ece]: Reused file_crypto exclusively for image encryption — MGO1 format, no parallel AES path
- [Phase quick/260419-ece]: Desktop thumbnails use Task::perform + image_cache HashMap (iced pattern, not async in view)
- [Phase 32]: DirectorySourceRow carries all 3 platform handles (path/bookmark_data/tree_uri) as nullable columns — single row shape across Desktop/iOS/Android
- [Phase 32]: upsert_directory_file uses ON CONFLICT DO UPDATE (preserves AUTOINCREMENT id) rather than INSERT OR REPLACE
- [Phase 32]: Plan 02: diff_files is pure (no DB/FS); walk_with_exclusions uses ignore::OverrideBuilder with ! prefix — desktop-only. validate_glob_pattern (globset) exposed cross-platform for mobile UI validation over UniFFI.
- [Phase 32]: Plan 02: Path-traversal globs scoped to walk root by ignore crate (T-32-V5); test canonicalises every emitted path and asserts starts_with(canonical_root).
- [Phase 32]: Plan 03: No UDL file in project (proc-macro UniFFI) — new types wired via derive macros instead of UDL entries
- [Phase 32]: Plan 03: SyncDirectoryFiles embeds synchronously inside actor loop to preserve per-batch VectorIndex flush semantics (deviates from IngestDocument's spawn_blocking)
- [Phase 32]: Plan 03: 50-file batch ceiling enforced at SyncDirectoryFiles handler entry (T-32-DoS1 mitigation)
- [Phase 32]: Added FfiApp::list_directory_fingerprints + DirectoryFingerprint Record so native side can diff without crossing persistence/bookmark boundary (T-32-I2)
- [Phase 32]: PollWatcher fallback uses raw watcher + custom EventHandler (not debouncer_opt) — shared flume channel unifies both backends
- [Phase 32]: Plan 05: FfiError enum replaces Result<_, String> in FfiApp; uniffi 0.29.5 strict about throws types
- [Phase 32]: Plan 05: Release profile strip=true hides UNIFFI_META_* symbols; bindgen needs CARGO_PROFILE_RELEASE_STRIP=false
- [Phase 32]: Plan 05: Bookmark cache is in-process only; cold-launch requires re-add. Deferred bookmark-read FFI to future plan
- [Phase 32]: Plan 06: Android directory-sync files placed under dev.disobey.mango.ui (plan referenced non-existent com.mango package)
- [Phase 32]: Plan 06: resolveTreeUri uses ContentResolver.persistedUriPermissions keyed by displayName — keeps tree URI out of UniFFI (T-32-I2)
- [Phase 32]: Plan 06: AppState.directorySources init was missing from AppManager bootstrap after plan 32-05 binding regen — fixed under Rule 1
- [Phase 32]: Plan 07: Centralised relative_time_label in Rust core + pre-computed last_synced_label field; native layers no longer compute relative-time locally (D-feels-done)
- [Phase 32]: Plan 07: Settings → Directory Sources entry added between Defaults and Memory on all three platforms; home-level Folders/Sources buttons preserved from 32-04/05/06

### Roadmap Evolution

- Phase 24 added: Redesign Settings UX — move memories into settings, redesign layout with grouped sections, add tool configuration for agents and chats
- Phase 25 added: disable/enable making memories in the app
- Phase 26 added: settings submenus and organization — group related settings into collapsible sections or sub-screens
- Phase 27 added: Add optional tool use to chat — extend chat streaming with inline tool calling, reusing agent tool infrastructure
- Phase 28 added: Local Data Encryption & Authentication — encrypt all local data (SQLite, vector indices, documents) with platform hardware (Keychain/Keystore/TPM), biometric/PIN unlock, duress PIN for data wipe, graceful degradation on older devices
- Phase 32 added: Directory-based RAG ingestion with periodic sync and file/folder exclusion — add whole directories (e.g. Obsidian vault) as RAG sources with glob exclusions, automatic periodic re-sync (add/modify/delete diff), cross-platform folder permissions (iOS security-scoped bookmarks, Android persistable tree URIs, Desktop), UI for source list + exclusion editor + manual sync-now

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260403-ft1 | Add Default Instructions setting in Settings (iOS + desktop) for global_system_prompt | 2026-04-03 | f91690e | [260403-ft1-add-default-instructions-setting-in-sett](./quick/260403-ft1-add-default-instructions-setting-in-sett/) |
| 260419-ece | encrypted image persistence: store sent/received image JPEG encrypted with DEK + thumbnail rendering on Android/iOS/desktop | 2026-04-19 | a7c204b | [260419-ece-encrypted-image-persistence-store-sent-r](./quick/260419-ece-encrypted-image-persistence-store-sent-r/) |
| 260419-jz5 | Allow changing chat title manually from chat top bar (iOS/Android/Desktop) | 2026-04-19 | 52fc9f8 | [260419-jz5-feature-request-allow-changing-chat-titl](./quick/260419-jz5-feature-request-allow-changing-chat-titl/) |

## Session Continuity

Last session: 2026-04-19T18:17:28.222Z
Stopped at: Completed 32-07-PLAN.md
Resume file: None
