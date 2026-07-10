---
phase: 32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo
verified: 2026-04-19T00:00:00Z
status: human_needed
score: 11/13 must-haves verified (8 automated pass, 3 require physical-device verification, 2 partial/deferred)
overrides_applied: 0
gaps:
  - truth: "File format support extended beyond text to include formats enumerated in phase context"
    status: partial
    reason: "Roadmap goal text promises extended format support, but 32-CONTEXT.md enumerates zero formats, plans 01-07 contain no format-extension work, and rust/src/rag/mod.rs::extract_text_from_file still only handles .pdf (since Phase 8) + UTF-8 text fallback. No docx/epub/rtf/html/etc. extractor added. Plans scope was directory-sync only; the file-format clause in the roadmap goal was never planned into a concrete deliverable."
    artifacts:
      - path: "rust/src/rag/mod.rs"
        issue: "extract_text_from_file unchanged from Phase 8 — only .pdf + UTF-8 lossy; no new format extractors"
    missing:
      - "Either a follow-up phase that enumerates and implements the target formats, or a roadmap-text correction / override acknowledging directory-sync was split from file-format work"
  - truth: "iOS periodic sync works after cold launch (D-22 / phase scope: 'Tracked sources are periodically re-synced ... across launches')"
    status: partial
    reason: "32-REVIEW HI-01 documents that DirectorySyncScheduler.bookmarkCache is process-local; after cold launch ScenePhase.active skips every pre-existing source because no bookmark is cached. Rust has no FFI accessor to read bookmark_data back (excluded from DirectorySourceSummary per T-32-I2). This violates the periodic-sync-across-launches promise in the roadmap goal for iOS specifically. Summary 32-05 explicitly scopes plan to foreground-resume and documents this as a 'known limitation' with future-plan path, but the roadmap goal is not met."
    artifacts:
      - path: "ios/Mango/Mango/DirectorySourcesView.swift"
        issue: "DirectorySyncScheduler.bookmarkCache (static var) never rehydrated on launch; syncAll() skips sources whose bookmark is absent from the in-process cache"
    missing:
      - "get_directory_bookmark(source_id) FFI accessor + AppManager-side rehydration on init, OR accept-as-design via override"
deferred:
  - truth: "Review findings HI-02 (O(N²) directory_files lookups), HI-03 (no file-size cap — OOM risk), HI-04 (silent embedding failure), ME-01/02/03/04/05/06 cross-platform inconsistencies"
    addressed_in: "Follow-up quick/debug phases (not explicitly scheduled)"
    evidence: "32-REVIEW.md logs 4 high + 6 medium + 3 low findings. These are quality/robustness issues rather than blockers of must-have truths; phase goals for add/remove/sync/exclude are still achievable on the happy path. Recommended to open dedicated closure tickets."
human_verification:
  - test: "Desktop end-to-end: add folder, initial sync, modify/delete external files, 2s debounced reindex, remove-confirm"
    expected: "32-04 Task 3 checklist — all 9 steps pass; inotify ENOSPC → PollWatcher fallback banner surfaces"
    why_human: "Requires launching desktop app (cargo run -p mango-desktop) and interacting with filesystem; watcher/debouncer timing not automatable in verifier"
  - test: "iOS physical-device: folder picker, bookmark persistence across force-quit, ScenePhase.active sync, iCloud placeholder skip, stale-bookmark refresh"
    expected: "32-05 Task 3 checklist — all 9 steps pass on an iPhone; note HI-01 gap on cold launch will surface here"
    why_human: "No macOS/Xcode toolchain in this Linux executor; iOS simulator does not enforce sandbox (Pitfall 2). Physical iPhone required for UIDocumentPicker + bookmark + iCloud verification"
  - test: "Android physical-device or emulator: SAF folder picker, persistable permission across reboot, WorkManager 15-min periodic, onResume sync, large-vault (500+ files) perf"
    expected: "32-06 Task 3 checklist — all 9 steps pass on Android device; takePersistableUriPermission survives reboot"
    why_human: "No Android emulator/device in this Linux-only executor; emulator reboot test + long-duration WorkManager test require real device session"
---

# Phase 32: Directory-based RAG Ingestion Verification Report

**Phase Goal:** Users can designate directories on disk as ambient RAG sources. The app periodically syncs them (adds new files, removes deleted ones, re-indexes modified ones) without requiring manual per-file upload, across iOS, Android, and Desktop. File format support extended beyond text to include the formats enumerated in the phase context.

**Verified:** 2026-04-19
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| - | ----- | ------ | -------- |
| 1 | User can add a directory as RAG source (folder picker on all 3 platforms) | VERIFIED (automated) | `DirectorySourcePicker` present on iOS (UIDocumentPicker) + Android (OpenDocumentTree) + Desktop (rfd); AddDirectorySource handler in lib.rs:6236 validates globs, inserts DirectorySourceRow; AppState.directory_sources populated (lib.rs:300) |
| 2 | User can remove a directory source and its indexed chunks cascade-delete | VERIFIED (automated) | RemoveDirectorySource handler lib.rs:6542 enumerates directory_files, deletes each document (chunks + usearch keys), deletes source row; FK CASCADE on directory_files confirmed by test_directory_files_cascade_delete; cross-platform confirmation dialogs present |
| 3 | Incremental sync picks up added / modified / removed files via mtime+size fingerprints | VERIFIED (automated) | `diff_files` in rag/directory_sync.rs:60 partitions added/modified/removed; 6 dedicated unit tests cover each bucket; SyncDirectoryFiles handler calls delete+re-insert for modified (lib.rs:6287) |
| 4 | Sync runs in batched 50-file chunks with per-batch VectorIndex flush | VERIFIED (automated) | Handler enforces `files.len() > 50` ceiling (lib.rs:6296); test_sync_directory_files_batching_flushes_vector_index asserts 50+30 split + vector_index.save per batch |
| 5 | Exclusion globs stored + validated + applied to walk | VERIFIED (automated) | `walk_with_exclusions` + `validate_glob_pattern` + test_walk_excludes_obsidian_dir, test_walk_excludes_tmp_glob, test_validate_exclusion_glob_*, test_walk_path_traversal_inert; SetDirectoryExclusions re-validates before persist |
| 6 | Desktop real-time sync via notify watcher (2s debounce) with PollWatcher fallback + 5-min interval | VERIFIED (automated) | desktop/iced/src/main.rs: notify_debouncer_mini, PollWatcher, Duration::from_secs(300), fallback warning path all present (31 matches across watcher/pipeline/chunks(50)) |
| 7 | iOS bookmark lifecycle: .minimalBookmark + isStale refresh + security-scoped reads | VERIFIED (automated) | DirectorySourcePicker.swift: 5×.minimalBookmark, 14×startAccessing/stopAccessing, bookmarkDataIsStale + UpdateDirectorySourceBookmark dispatch; iCloud `ubiquitousItemDownloadingStatus == .notDownloaded` skip |
| 8 | Android SAF lifecycle: takePersistableUriPermission + bulk DocumentsContract.query (not DocumentFile.listFiles) | VERIFIED (automated) | DirectorySourcePicker.kt: OpenDocumentTree + takePersistableUriPermission inline in result callback + buildChildDocumentsUriUsingTree (2×); no DocumentFile.listFiles in production code (only docstring warnings); COLUMN_LAST_MODIFIED `/ 1000` (D-20); .chunked(50) |
| 9 | Android WorkManager 15-min periodic + onResume foreground sync | VERIFIED (automated) | DirectorySyncWorker.kt: PeriodicWorkRequestBuilder<DirectorySyncWorker>(15, MINUTES) + enqueueUniquePeriodicWork; MainActivity.kt: onResume dispatches syncDirectory per source |
| 10 | Cross-platform UI polish: relative-time label, empty state copy, remove-confirm, settings entry | VERIFIED (automated) | `relative_time_label` fn + last_synced_label field (lib.rs:856/840); 10-assertion test_relative_time_labels; Settings entries on all 3 platforms; consistent empty-state copy |
| 11 | Migration V18 + CRUD layer backing all of the above | VERIFIED (automated) | MIGRATION_V18 + directory_sources/directory_files tables + 11 CRUD fns; 3 schema tests + 4 CRUD tests + 9 integration tests + 318-test full suite pass |
| 12 | iOS periodic sync across launches (cold-start rehydration) | FAILED | HI-01: DirectorySyncScheduler.bookmarkCache is process-local static; cold launch leaves it empty so ScenePhase.active skips every pre-existing source. Summary 32-05 flags this as "known limitation"; roadmap goal requires "periodically re-synced ... across launches" |
| 13 | File format support extended beyond text | FAILED / OUT-OF-SCOPE | `rust/src/rag/mod.rs::extract_text_from_file` handles only .pdf (Phase 8) + UTF-8 text; CONTEXT.md enumerates zero formats; no plan in 01-07 added any extractor. Roadmap goal sentence is not satisfied, but was never planned into a deliverable |

**Score:** 11/13 verified (8 fully automated, 3 pending human-device verification, 2 partial)

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `rust/src/persistence/schema.rs` | MIGRATION_V18 + registration | VERIFIED | 23 hits for MIGRATION_V18/directory_sources/directory_files; ON DELETE CASCADE present |
| `rust/src/persistence/queries.rs` | 11 CRUD fns | VERIFIED | All 11 fns present (insert/list/get/delete + 3 updates + upsert/list-by-source/delete/count for files) |
| `rust/src/rag/directory_sync.rs` | diff_files, walk_with_exclusions, validators, FileDiff | VERIFIED | All entry points present; 14 unit tests green |
| `rust/src/lib.rs` | 6 AppAction variants + DirectorySourceSummary + DirectoryFileEntry + DirectorySyncStatus + DirectoryFingerprint + relative_time_label + load_directory_sources_summary | VERIFIED | All types and variants present; handlers at lib.rs:6236-6621; load_directory_sources_summary at lib.rs:3148 |
| `rust/src/tests/directory_rag.rs` | 9 integration tests + test_relative_time_labels | VERIFIED | Test file present, full suite 318 pass |
| `desktop/iced/src/views/directory_sources.rs` | Source list + add/edit/remove + sync-now view | VERIFIED | File exists (~496 lines per summary); integrates with main.rs watcher + pipeline |
| `ios/Mango/Mango/DirectorySourcePicker.swift` | Picker + bookmark lifecycle + enumerator + 50-file batching | VERIFIED | 349 lines; all required APIs present and correctly wired |
| `ios/Mango/Mango/DirectorySourcesView.swift` | SwiftUI source list + ExclusionEditor + ScenePhase scheduler | VERIFIED | 379 lines; DirectorySyncScheduler present |
| `android/app/.../ui/DirectorySourcePicker.kt` | SAF picker + traverseTree + readFileContent + batched dispatch | VERIFIED | 265 lines; all anti-patterns (DocumentFile.listFiles) avoided |
| `android/app/.../ui/DirectorySourcesScreen.kt` | Compose source list + dialogs | VERIFIED | 263 lines |
| `android/app/.../ui/DirectorySyncWorker.kt` | CoroutineWorker + 15-min periodic | VERIFIED | 98 lines; PeriodicWorkRequestBuilder(15, MINUTES) correct |
| `rust/src/rag/mod.rs` | Extended file-format extractor beyond Phase 8 | NOT DELIVERED | Unchanged from Phase 8 — only .pdf + UTF-8 text; see truth #13 |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| Desktop rfd picker | AppAction::AddDirectorySource | pick_folder → dispatch | WIRED | desktop/iced/src/main.rs dispatches AddDirectorySource with path + defaults |
| iOS UIDocumentPicker | AppAction::AddDirectorySource | bookmarkData(options: .minimalBookmark) → dispatch | WIRED | DirectorySourcesView.swift handlePicked |
| Android OpenDocumentTree | AppAction::AddDirectorySource | takePersistableUriPermission → dispatch | WIRED | DirectorySourcesScreen.kt + rememberDirectoryPicker |
| SyncDirectoryFiles handler | VectorIndex.save | actor_state.dek at end of each batch | WIRED | lib.rs batch loop flushes vector_index.save after each 50-file chunk |
| SyncDirectoryFiles handler | IngestDocument-equivalent pipeline | extract_text_from_file → chunk_text → embed → usearch.add → insert_document | WIRED | Per-file logic in lib.rs:6287+ |
| RemoveDirectorySource | delete_document (cascade chunks + usearch) + delete_directory_source | list_directory_files_by_source → per-row delete | WIRED | lib.rs:6542+ confirmed by test_remove_directory_source_cascades |
| notify watcher | AppAction::TriggerDirectorySync | debouncer callback → flume | WIRED | desktop/iced/src/main.rs spawn_directory_sync_workers |
| iOS ScenePhase .active | syncAll | DirectorySyncScheduler | PARTIAL | Wired but bookmarkCache rehydration missing (HI-01 / truth #12) |
| Android WorkManager → SyncDirectoryFiles | DirectorySyncWorker.doWork → syncDirectory per source | resolveTreeUri → dispatch | WIRED (with caveat ME-01: displayName collision) |
| DirectorySourceSummary.last_synced_label | UI rendering | rust-side relative_time_label | WIRED | All 3 platforms consume `source.lastSyncedLabel` from Rust |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| DirectorySourcesView/Screen rows | source.directory_sources | AppState.directory_sources populated by load_directory_sources_summary from SQLite | YES — reads directory_sources table | FLOWING |
| DirectorySourceRow sync_status pill | source.sync_status | SyncDirectoryFiles handler mutates AppState | YES — actor updates status during sync pipeline | FLOWING |
| IngestionProgress during sync | AppState.ingestion_progress | Actor emits progress per batch | YES — set at batch start, cleared on is_final_batch | FLOWING |
| Native-side diff input | FfiApp.listDirectoryFingerprints | CoreMsg::ListDirectoryFingerprints → list_directory_files_by_source | YES — real DB query | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Full Rust library test suite passes | `cargo test -p mango_core --lib -- --test-threads=1` | 318 passed / 0 failed / 10 ignored | PASS |
| Directory-sync unit + integration tests | included in above run (schema + queries + directory_sync + directory_rag modules) | All green | PASS |
| Desktop build | `cargo build -p mango-desktop` (per 32-07 summary) | Clean build | PASS (per summary evidence; not re-run in this session) |
| Android debug build | `./gradlew :app:assembleDebug` (per 32-06/07 summaries) | BUILD SUCCESSFUL | PASS (per summary evidence; not re-run) |
| iOS build | `xcodebuild -scheme Mango ...` | N/A | SKIP — no macOS toolchain in this Linux executor |

### Requirements Coverage

DIR-01..DIR-06 are declared in PLAN frontmatter only. `.planning/REQUIREMENTS.md` does not define DIR-* entries — the plan-side IDs are the authoritative spec. Coverage below is derived from the plans' own stated intents.

| Requirement | Source Plan(s) | Description (from plan/context) | Status | Evidence |
| ----------- | -------------- | ------------------------------- | ------ | -------- |
| DIR-01 | 32-02, 32-07 | Incremental diff against fingerprints (added/modified/removed) | SATISFIED | rag/directory_sync.rs:diff_files + 6 unit tests |
| DIR-02 | 32-02, 32-04, 32-05, 32-06, 32-07 | Glob-based exclusions with validation + real-time change pickup | SATISFIED | walk_with_exclusions + validate_glob_pattern + desktop notify watcher + iOS ScenePhase + Android WorkManager/onResume |
| DIR-03 | 32-01 | Persistence layer for directory sources (CRUD) | SATISFIED | MIGRATION_V18 + 7 source-level CRUD fns + 4 unit tests |
| DIR-04 | 32-01 | Fingerprint upsert / stale detection | SATISFIED | upsert_directory_file (ON CONFLICT DO UPDATE) + list/delete/count + tests |
| DIR-05 | 32-03, 32-04, 32-05, 32-06, 32-07 | Sync pipeline: chunk → embed → index → persist with 50-file batching | SATISFIED | SyncDirectoryFiles handler + 9 integration tests + batching ceiling + VectorIndex flush |
| DIR-06 | 32-03, 32-04, 32-05, 32-06, 32-07 | Cascaded removal: source row + files + documents + chunks + usearch keys | SATISFIED | RemoveDirectorySource handler + FK CASCADE + test_remove_directory_source_cascades + remove-confirm UI on all 3 platforms |

**ORPHANED:** None. No `.planning/REQUIREMENTS.md` DIR-* entries exist that aren't claimed by a plan.

### Anti-Patterns Found

Surveyed via 32-REVIEW.md (standard-depth code review). Findings classified per reviewer severity; none rise to blocker for the roadmap goal on the happy path.

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `ios/Mango/Mango/DirectorySourcesView.swift` | 359-391 | Process-local bookmark cache never rehydrated | HIGH (HI-01) | Blocks truth #12 — cold-launch periodic sync silently skipped |
| `rust/src/lib.rs` | 6318, 6358 | O(N²) `list_directory_files_by_source` inside batch loop | HIGH (HI-02) | Performance degrades on large vaults; functionally correct |
| `ios/`, `android/`, `desktop/` | file-read sites | No file-size cap before read-all-to-memory | HIGH (HI-03) | OOM risk on large PDFs in vault; not tested |
| `rust/src/lib.rs` | 6464-6478 | Embedding failure → silent zero-vector write, chunks persisted unindexed | HIGH (HI-04) | User sees "Idle" on a sync that produced no searchable content |
| `android/app/.../DirectorySyncWorker.kt` | 78-101 | Tree URI resolution by displayName collides on duplicate folder names | MEDIUM (ME-01) | Wrong folder → wrong source on ambiguous names |
| `ios` vs `android` SyncDirectoryFiles dispatch | — | Removals on last batch (iOS) vs first batch (Android); desktop uses first | MEDIUM (ME-02) | Cross-platform drift; mid-sync cancellation behaviour diverges |
| `ios/.../DirectorySourcePicker.swift` | 146-150 | GlobMatcher literal uses hasPrefix — `foo` matches `foobar.md` | MEDIUM (ME-03) | False-positive exclusion |
| `android/.../DirectorySourcePicker.kt` | 112, 152-161 | Matcher sees display name, not relative path; patterns unanchored vs desktop-anchored | MEDIUM (ME-04) | Diverges from ignore-crate semantics on desktop |
| `rust/src/lib.rs` | 6393, 6425 | batch_error overwrites — only last failure surfaces | MEDIUM (ME-05) | Partial-failure visibility poor |
| `rust/src/lib.rs` | 6259, 6598 | `serde_json::to_string(globs).unwrap_or_else("[]".into())` silent fallback | MEDIUM (ME-06) | Masks hypothetical serialization bugs |
| `rust/src/persistence/schema.rs` | 295-339 | V18 const declared before V17 in source order | LOW (LO-01) | Cosmetic; MIGRATIONS slice order is correct |
| `rust/src/persistence/queries.rs` | 1206 | Stale `#[allow(dead_code)]` on count_directory_files (now live-used) | LOW (LO-02) | Masks truly-dead helpers in future |
| `desktop/iced/src/views/directory_sources.rs` | 65-80 | `format_file_count` correctness not locked by test | LOW (LO-03) | Tested manually by reviewer; suggest adding unit test |

### Human Verification Required

**This phase executed on a Linux workstation with no Xcode / iOS simulator / Android emulator available.** The three Task 3 checkpoints (32-04, 32-05, 32-06) were auto-approved under auto-chain policy and remain to be manually walked through. Items:

#### 1. Desktop end-to-end (iced)

**Test:** Follow 32-04 Task 3 steps 1-9 (launch `cargo run -p mango-desktop`; add folder containing `.md` files + `.obsidian/config.json`; modify/delete/exclude files; test notify debounce + PollWatcher fallback; remove source with confirm).
**Expected:** All steps succeed; inotify ENOSPC fallback banner appears when `max_user_watches` is set low.
**Why human:** Requires running the app + making filesystem changes + observing debouncer timing.

#### 2. iOS physical-device end-to-end

**Test:** Build on macOS, install on iPhone; follow 32-05 Task 3 steps 1-9 (pick folder; force-quit/relaunch; move folder to exercise stale-bookmark refresh; iCloud offload; exclusion edit; remove).
**Expected:** Picker + bookmark persistence + ScenePhase sync + iCloud skip + stale refresh + remove all work. **Note:** Step 4 (post-relaunch Sync Now) is expected to show the HI-01 "re-add required" fallback toast — this is the cold-launch bookmark-cache gap that truth #12 flags.
**Why human:** No macOS / Xcode toolchain in this executor; iOS simulator does not enforce sandbox (Pitfall 2) so physical device is required.

#### 3. Android device/emulator end-to-end

**Test:** Build + install debug APK; follow 32-06 Task 3 steps 1-9 (pick folder via SAF; reboot device; test WorkManager 15-min; onResume sync; 500+ file large-vault perf; exclusion edit; remove).
**Expected:** takePersistableUriPermission survives reboot; bulk DocumentsContract completes large vault in <5s; WorkManager and onResume both drive syncs.
**Why human:** No Android emulator or device in this Linux-only executor; reboot-persistence and 15-min WorkManager cycles cannot be simulated in verifier.

### Gaps Summary

Two gaps prevent a clean pass beyond the three human-verification items:

1. **iOS cold-launch periodic sync (truth #12 / HI-01):** The roadmap goal text promises periodic sync across launches. On iOS this is not delivered because `DirectorySyncScheduler.bookmarkCache` is process-local and there's no FFI accessor to rehydrate it from `directory_sources.bookmark_data`. Summary 32-05 acknowledges this as a known limitation with a future-plan path, but it is a genuine goal gap, not just a polish item. **Proposed closure:** add `FfiApp::get_directory_bookmark(source_id) -> Result<Option<Vec<u8>>, FfiError>` + AppManager.init-time rehydration. Alternatively: add an override to VERIFICATION.md frontmatter accepting the limitation and planning the fix in a named later phase.

2. **File format support (truth #13):** The roadmap goal sentence "File format support extended beyond text to include the formats enumerated in the phase context" has no corresponding work product. CONTEXT.md enumerates no formats; plans 01-07 contain no extractor work; `extract_text_from_file` remains at Phase 8 capabilities (.pdf + UTF-8 text fallback). This appears to be a roadmap-text / planning mismatch rather than an execution gap. **Proposed closure:** (a) new phase scoped specifically to format support (e.g., .docx via docx-rs, .epub via epub, .html via scraper, .rtf via rtf-parser), OR (b) roadmap-text correction / override recording that directory-sync was split from file-formats.

The ten 32-REVIEW findings (HI-02/03/04 + ME-01..06 + LO-01..03) are documented as deferred — they degrade robustness and cross-platform consistency but do not block the must-have truths on the happy path. Recommended to open a follow-up debug/quick ticket grouping them.

---

_Verified: 2026-04-19_
_Verifier: Claude (gsd-verifier)_
