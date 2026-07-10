# Milestones

## v2.0 Memory & Agents (Shipped: 2026-07-10)

**Phases completed:** 21 phases, 82 plans, 58 tasks

**Key accomplishments:**

- 1. [Rule 1 - Bug] Fixed async-openai import paths
- StreamDone handler additions:
- Semantic memory retrieval wired into do_send_message: relevant past memories inject into system prompt via shared usearch HNSW index and get_memory_content_by_usearch_keys lookup
- 4 new agent tools (web_search, fetch_url, file, calculate) added to Rust core with scraper + evalexpr deps, extended dispatch_tools signature, and 11 new tests all green
- dispatch_tools call site fully wired with runtime/data_dir/brave_api_key, ActorState.data_dir added, and both agent system prompts updated to list all 7 tools
- MemorySummary UniFFI record, Screen::Memories, three memory AppActions (List/Delete/Update), update_memory SQL query, AgentStepSummary.tool_input field, and Wave 0 test coverage -- complete Rust API surface for Plans 02/03 platform UIs
- Memory management screens on iOS (SwiftUI), Android (Jetpack Compose), and Desktop (iced) with navigation integration, chronological list, swipe/button delete with confirmation, inline edit, and empty states -- all wired to UniFFI MemorySummary types from Plan 01
- Agent navigation restored on iOS/Android/Desktop with tool_input display and final_answer special rendering in all three step detail views
- 1. [Rule 1 - Bug] MemoryRow missing usearch_key field
- memory_count u64 and brave_api_key_set bool added to AppState with startup loading, mutation-triggered refresh, and SetBraveApiKey action handler persisting to settings table
- MEMORY section (Memories count badge, chevron, PushScreen dispatch) and TOOLS section (Brave API key secure field with Save button) added to Settings on Desktop, iOS, and Android; Memories removed from all home toolbars
- memories_enabled bool added to AppState with persistent toggle via settings table and memory extraction gated behind the flag in StreamDone
- 1. [Rule 3 - Blocking] Worktree behind main branch
- One-liner:
- iOS:
- Desktop Settings main screen replaced inline Providers/Defaults content with tappable summary rows; extracted to dedicated sub-screen view modules with back navigation
- 7 failing test stubs in rust/src/tests/chat_tools.rs establish TDD contracts for tools_enabled persistence, MIGRATION_V16, and build_chat_tools subset logic
- One-liner:
- One-liner:
- UniFFI bindings regenerated with toolsEnabled/SetConversationToolsEnabled; tools toggle added to iOS, Android, and Desktop chat UIs
- SQLCipher-backed Database, AES-256-GCM file encryption with MGO1 header, Argon2id DEK/KEK derivation, and BootstrapDb singleton — full crypto foundation for local data encryption
- 1. [Rule 2 - Missing Functionality] Backward-compat auto-open for pre-encryption databases
- 1. [Rule 3 - Blocking] Restored worktree to correct base state
- 1. [Rule 3 - Blocking] Phase 28-02 Rust implementation missing from worktree
- Android lock gate with BiometricPrompt CountDownLatch bridge, Compose LockScreen/PinSetupScreen, and regenerated UniFFI Kotlin bindings with all Phase 28 auth types
- Screen enum:
- iOS (`ios/Mango/Mango/ContentView.swift`):
- File: `rust/src/lib.rs` (SetupPin handler)
- One-liner:
- Phase 21 VERIFICATION.md written (status: passed, MEM-03 SATISFIED); REQUIREMENTS.md synced to 36/36 Complete with ENC-02 ticked and ENC-09 corrected to Phase 28
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- 1. [Rule 3 — Blocking] Stale version assert in `rust/src/tests/rag.rs`
- 1. [Rule 3 - Blocking] Missing FFI method for native-side diff
- 1. [Rule 3 — Blocking] uniffi 0.29.5 panics on `Result<_, String>` as FFI return type
- Targeted FFI accessor `get_directory_bookmark` + AppManager.init rehydration loop that populates `DirectorySyncScheduler.bookmarkCache` from SQLite before the first ScenePhase.active fires, closing VERIFICATION gap HI-01 (truth #12)
- Extends extract_text_from_file from the Phase 8 .pdf + UTF-8 baseline to four mobile-safe pure-Rust extractors (.docx, .epub, .html/.htm, .rtf) and adds a 20 MiB size cap that short-circuits before any parser allocates.
- One-liner:
- One-liner:
- One-liner:
- 1. [Rule 3 - Blocking] Plan referenced non-existent `crate::config::BackendConfig` and `preferred_model` field
- One small adaptation:
- No new variant added.
- One Rule-3 auto-fix:
- One Rule-3 auto-fix:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- One-liner:
- PASS-WITH-NOTES
- 1. [Rule 1 - Bug] Pre-existing `M rust/Cargo.toml` swept into Task 1 commit`
- 1. Aggregation SQL replaced.
- Created:
- 1. chat-tool path does NOT write to `agent_steps`.

---
