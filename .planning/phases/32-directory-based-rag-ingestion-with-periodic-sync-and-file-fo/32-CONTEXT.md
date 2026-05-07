# Phase 32: Directory-based RAG Ingestion with Periodic Sync — Context

**Gathered:** 2026-04-19
**Status:** Ready for planning
**Mode:** auto (decisions selected from 32-RESEARCH.md recommendations)

<domain>
## Phase Boundary

Extend the existing single-file RAG ingestion (Phases 8/31) so a user can register a **whole directory** (e.g. an Obsidian vault) as a RAG source. The phase delivers:

- Tracked directory sources persisted across launches on iOS (security-scoped bookmarks), Android (persistable SAF tree URIs), and Desktop (absolute paths).
- Glob-based exclusion rules per source (gitignore-style `!` semantics).
- Incremental re-sync (added / modified / deleted files) triggered periodically and on manual "Sync now".
- UI across all three native layers: source list with last-synced time, add/remove source, exclusion editor, manual sync-now.

**Out of scope (deferred):** bi-directional sync / writes back to the folder, cloud-native connectors (Dropbox/Drive/iCloud remote indexing), full-text preview UI, per-file delete from index while keeping on disk, search-result deep-links back into source folder.

</domain>

<decisions>
## Implementation Decisions

### Architecture / RMP Boundary
- **D-01:** Rust core owns persistence, diffing, chunking, embedding, and index writes. Native layers own permission lifecycle (pick → persist → resolve), filesystem/SAF enumeration, and byte reads. [auto: research §Summary, §Pattern 1]
- **D-02:** Native layer **diffs first** against fingerprints it already has (either cached locally or requested from Rust) and streams only changed file content to Rust via a batched AppAction. Full 10k-file listings never cross the UniFFI boundary. [auto: research §Pattern 1, §Pitfall 8]
- **D-03:** New AppActions: `AddDirectorySource`, `SyncDirectoryFiles { source_id, files, removed_paths }`, `RemoveDirectorySource`, `SetDirectoryExclusions`, `TriggerDirectorySync { source_id }`, `UpdateDirectorySourceBookmark { source_id, bookmark_data }` (iOS stale-refresh). [auto: research §Pattern 1, §Pitfall 7]

### Persistence (Migration V18)
- **D-04:** Add two tables per research schema: `directory_sources` (id, display_name, path, bookmark_data BLOB, tree_uri, exclusion_globs JSON, last_synced_at, file_count, created_at) and `directory_files` (source_id FK ON DELETE CASCADE, file_path, mtime_secs, size_bytes, document_id, UNIQUE(source_id, file_path)). [auto: research §Pattern schema]
- **D-05:** `file_path` semantics: **Desktop** = absolute path; **iOS** = path relative to bookmark root; **Android** = SAF document ID (opaque, stable). Stored as TEXT. [auto: research §Pattern 5]
- **D-06:** On `RemoveDirectorySource` cascade to `directory_files`, and also delete owned `documents`, `chunks`, and corresponding `usearch` vector keys for those files. Reuse existing single-document delete path per-file.

### Change Detection
- **D-07:** Use **mtime + size** fingerprints, not content hashes. Content hashing is explicitly rejected for cost on large vaults. [auto: research §Pattern 2, §Anti-Patterns]
- **D-08:** Special case for Android SAF: if `COLUMN_LAST_MODIFIED == 0` (third-party provider quirk), fall back to **always re-read** that file (treat as modified every sync). [auto: research Open Q5]

### Desktop Walking & Watching
- **D-09:** Use `ignore` crate 0.4 (`WalkBuilder` + `OverrideBuilder`) with `.hidden(false)`, `.git_ignore(false)`, `.standard_filters(false)` — only user-supplied exclusion globs apply. Exclusion globs are stored raw; the `!` prefix is added when passing to `OverrideBuilder`. [auto: research §Pattern 3]
- **D-10:** Desktop real-time FS watching via `notify` 8.x + `notify-debouncer-mini` with **2-second debounce**; watcher only emits a `TriggerDirectorySync` nudge, it does not send file data directly. [auto: research §Pattern 6]
- **D-11:** On `inotify` watch-limit error (Linux `ENOSPC`), fall back to `PollWatcher` at 60s interval and surface a warning in the UI: "File watching unavailable; syncing on schedule." [auto: research §Pitfall 6]
- **D-12:** Desktop folder picker via `rfd::FileDialog::pick_folder()` (verify API at plan time — if different, adapt).

### iOS Access Model
- **D-13:** `UIDocumentPickerViewController(forOpeningContentTypes: [.folder], asCopy: false)` for the picker. [auto: research §Pattern 4]
- **D-14:** Persistence via `URL.bookmarkData(options: .minimalBookmark, …)` — **not** `.withSecurityScope` (that's macOS-only). [auto: research §Pitfall 1]
- **D-15:** On every sync attempt: resolve bookmark, check `bookmarkDataIsStale`; if stale, re-create bookmark from resolved URL and dispatch `UpdateDirectorySourceBookmark`. [auto: research §Pitfall 7]
- **D-16:** Wrap every read in `startAccessingSecurityScopedResource()` / `defer stopAccessingSecurityScopedResource()`. Balance strictly. [auto: research §Pitfall 2]
- **D-17:** iCloud handling — check `URLResourceValues.ubiquitousItemDownloadingStatus`; **skip** files with status `.notDownloaded` this sync cycle and surface them as "not downloaded locally" in UI. Do **not** block on `startDownloadingUbiquitousItem`. [auto: research §Pitfall 3]

### Android Access Model
- **D-18:** `ActivityResultContracts.OpenDocumentTree()` for the picker; immediately call `takePersistableUriPermission(uri, FLAG_GRANT_READ_URI_PERMISSION)` in the result callback before storing the URI. [auto: research §Pitfall 4]
- **D-19:** Traverse with direct `DocumentsContract.buildChildDocumentsUriUsingTree` + `contentResolver.query` bulk queries — never `DocumentFile.listFiles()`. [auto: research §Pitfall 5]
- **D-20:** `COLUMN_LAST_MODIFIED` is millis; divide by 1000 before storing as `mtime_secs`. Store the SAF `docId` as `file_path`. [auto: research §Pattern 5]

### Scheduling Strategy (per platform)
- **D-21:** **Desktop** — `notify` watcher with debounce + Tokio 5-minute interval as belt-and-braces fallback.
- **D-22:** **iOS** — foreground-resume scan only. Trigger `SyncAllDirectories` when `ScenePhase` becomes `.active`. No BGTaskScheduler. [auto: research §Pattern 7]
- **D-23:** **Android** — `PeriodicWorkRequest` via WorkManager at the 15-minute minimum with `NETWORK_NOT_REQUIRED`, plus a foreground-resume trigger on `onResume`. [auto: research §Pattern 7]
- **D-24:** Manual "Sync now" button on every platform dispatches the same sync pipeline directly, bypassing the scheduler.

### Throughput / Batching
- **D-25:** `SyncDirectoryFiles` is dispatched in **batches of 50 files** per call. The native layer iterates; the actor processes each batch end-to-end (chunk → embed → usearch add) and emits `IngestionProgress` before taking the next batch. This bounds memory and makes progress visible. [auto: research §Pitfall 8]
- **D-26:** Embedding reuses the existing `fastembed` (desktop) / `MobileEmbeddingProvider` (iOS/Android) paths — no new embedding code.
- **D-27:** `VectorIndex` save is flushed after each 50-file batch so a crash mid-sync doesn't lose progress.

### Exclusions
- **D-28:** Exclusion patterns stored as JSON array of raw gitignore-style strings (e.g. `[".obsidian/", "*.tmp", "*.canvas"]`). UI editor gives a sensible default set for Obsidian vaults (`.obsidian/`, `.trash/`, `*.tmp`) but the user can edit freely. [auto: research §Pattern 3]
- **D-29:** Exclusions validated at save time via `OverrideBuilder::add()`; surface errors inline in the editor. Mobile platforms apply the same glob semantics client-side using a small shared glob matcher (or call into Rust via a `ValidateExclusionGlob` helper — plan-phase decides whether to FFI or reimplement matching).

### UI Scope (shared across platforms)
- **D-30:** **Source list screen** shows: display name, item count, last-synced relative time ("3m ago" / "never"), sync-status pill (idle / syncing / error), per-row overflow → "Sync now", "Edit exclusions", "Remove".
- **D-31:** **Add source flow**: native folder picker → optional exclusion editor (can be skipped; defaults apply) → save → kicks off first sync with progress bar.
- **D-32:** **Exclusion editor** = plain text editor, one glob per line, with "Restore defaults" button; live-validates each line.
- **D-33:** **Remove source** shows confirmation ("Remove source and delete N indexed chunks?"). Irreversible; no soft delete.
- **D-34:** **Progress surface** during sync reuses existing `IngestionProgress` UI from Phase 8 (no new component).

### Security / Validation
- **D-35:** Exclusion globs are scoped to the walk root by `OverrideBuilder` design — path-traversal patterns like `../../etc/passwd` are inert. No additional sanitization needed beyond the `OverrideBuilder::add()` validation. [auto: research §Security]
- **D-36:** Only read paths produced by enumerating the resolved bookmark/tree root — never construct arbitrary paths from user input. [auto: research §Security]
- **D-37:** Indexed chunk storage reuses existing ENC-02 encryption path via the DEK already threaded through `ActorState` (Phase 28/29). No new crypto work.

### File Format Support (added 2026-04-19 via gap-closure plan 32-09)

- **D-38:** `extract_text_from_file` dispatch (rust/src/rag/mod.rs) supports, beyond the Phase 8 `.pdf` + UTF-8 baseline, the following formats for directory-sync ingestion: `.docx` (docx-rs), `.epub` (epub crate), `.html` / `.htm` (scraper + html2text), `.rtf` (rtf-parser). `.md`, `.txt`, `.org` continue via the UTF-8 lossy fallback branch. [gap-closure: VERIFICATION truth #13]
- **D-39:** Extractor crate selection rule: pure-Rust, no OpenSSL, no native C deps — verified by `cargo tree | grep -iE "openssl-sys|native-tls"` returning empty after the deps are added, and by successful `cargo build --target aarch64-apple-ios` / `cargo ndk -t arm64-v8a build`.
- **D-40:** Size cap: `MAX_EXTRACT_INPUT_BYTES = 20 MiB` short-circuits extract_text_from_file before any parser runs (also addresses VERIFICATION HI-03 file-size OOM concern for the extract path; full-pipeline HI-03 coverage at the file-read sites remains deferred).

### Claude's Discretion
- Exact batch size (50) is a starting point — may be tuned empirically in execute.
- Native-side glob matching implementation (small bespoke matcher vs. FFI into Rust `globset`) — planner picks based on binary size / code-reuse tradeoff.
- Default exclusion preset list content — planner may refine beyond `.obsidian/`, `.trash/`, `*.tmp`.
- UI copy, iconography, empty-state wording.

### Folded Todos
None — no pending todos matched this phase.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase inputs
- `.planning/phases/32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo/32-RESEARCH.md` — full research; patterns, pitfalls, versions, code examples.
- `.planning/ROADMAP.md` §"Phase 32" — scope statement.
- `.planning/REQUIREMENTS.md` — project-wide constraints (on-device, no telemetry, OpenAI-compatible API).
- `.planning/PROJECT.md` — architectural principles.

### Rust crate docs (external, for reference during implementation)
- `https://docs.rs/ignore/latest/ignore/` — `WalkBuilder`, `OverrideBuilder`.
- `https://docs.rs/ignore/latest/ignore/overrides/struct.OverrideBuilder.html` — `!` prefix semantics.
- `https://docs.rs/notify/8.2.0/notify/` — watcher platform matrix.
- `https://developer.android.com/training/data-storage/shared/documents-files` — SAF + `takePersistableUriPermission`.
- `https://developer.android.com/topic/libraries/architecture/workmanager` — 15-min minimum.
- `https://adam.garrett-harris.com/2021-08-21-providing-access-to-directories-in-ios-with-bookmarks/` — iOS bookmark lifecycle.
- `https://commonsware.com/blog/2019/12/14/scoped-storage-stories-listfiles-woe.html` — avoid `DocumentFile.listFiles()`.

### In-repo patterns to mirror
- `rust/src/lib.rs` — `AppAction::IngestDocument` handler (~line 5013) is the direct template for per-file sync.
- `rust/src/persistence/schema.rs` — migration pattern (V6 referenced in research).
- `rust/src/persistence/queries.rs` — CRUD query style.
- `rust/src/rag/` — existing `chunker.rs`, `context.rs`, `index.rs`; add `directory_sync.rs`.
- `android/.../DocumentLibraryScreen.kt` — existing SAF pattern reference.
- `ios/.../DocumentLibraryView.swift` — existing security-scoped resource pattern reference.
- `desktop/iced/src/main.rs` — rfd picker usage (lines 773, 1056).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppAction::IngestDocument` — per-file chunk + embed + index-insert pipeline. Directory sync calls into the same pipeline per file inside the batch loop.
- `IngestionProgress` state + UI — already wired across platforms; surfaces progress for directory sync with no new component.
- `rfd::FileDialog` — already used on desktop; extend with `pick_folder()`.
- `file_crypto` + DEK on `ActorState` — chunk encryption path (Phases 28/29).
- `usearch` index — single shared HNSW; directory chunks use the same chunk-rowid keyspace.

### Established Patterns
- Rust actor on a dedicated thread receives `AppAction`, blocks on SQLite + embedding, emits `AppState` snapshots via flume.
- Migrations are numbered (`MIGRATION_V17` etc.); add `MIGRATION_V18` for the two new tables.
- Platform-specific native code is thin: picker + permission + file reads; all logic in Rust.

### Integration Points
- New `AddDirectorySource` is dispatched from iOS / Android / Desktop UI after the native folder picker returns.
- `SceneDelegate` / `ScenePhase` (iOS) and `onResume` (Android) already exist as lifecycle hooks — just add sync trigger.
- WorkManager init block on Android needs a new `PeriodicWorkRequest` registration.
- Desktop main loop spawns tokio tasks — add long-lived `notify` watcher thread and 5-minute interval task.

</code_context>

<specifics>
## Specific Ideas

- Obsidian-vault-via-git-sync is the canonical mental model — the sync loop must handle a sudden mass change (git pull touching hundreds of files) without spiking memory or UI-freezing. This is what drives D-10 (debounce), D-25 (50-file batches), D-27 (flush after each batch).
- Last-synced time in the source list is a first-class UX signal — users will look at it to confirm their vault is fresh.
- "Sync now" must feel immediate: dispatch and show progress within one tick.

</specifics>

<deferred>
## Deferred Ideas

- Writing back to the source folder (e.g. saving model-generated notes into the vault). Out of scope — read-only sync keeps the security model simple.
- Cloud-native connectors (Dropbox, Google Drive, OneDrive remote directory indexing without downloading). Separate phase; requires OAuth plus cloud-file provider protocols.
- Automatic download of iCloud-evicted files. D-17 explicitly skips them; a future phase could add an opt-in "materialize cloud files before sync".
- Full-text result → "Open in Obsidian" deep links. Separate phase.
- Per-file "exclude this file" from the search UI. Separate phase — would require additional per-file-skip schema.
- File-type allowlist (e.g. only `.md`). Exclusion globs can approximate this today; a dedicated allowlist UI is a later UX polish.
- Bi-directional change detection with content hash for files on storage providers that lie about mtime. D-08 is the interim fallback.

</deferred>

---

*Phase: 32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo*
*Context gathered: 2026-04-19 (auto mode)*
