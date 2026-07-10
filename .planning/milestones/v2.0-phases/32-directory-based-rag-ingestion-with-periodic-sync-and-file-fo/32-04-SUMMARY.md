---
phase: 32
plan: 04
subsystem: desktop-ui
tags: [desktop, iced, notify, watcher, rfd, directory-sync, fallback]
requirements: [DIR-02, DIR-05, DIR-06]
dependency_graph:
  requires:
    - "32-03 (AppAction variants, AppState.directory_sources, SyncDirectoryFiles pipeline)"
  provides:
    - "Desktop DirectorySources screen (folder picker + source list + exclusion editor + sync-now + remove-confirm)"
    - "Notify-debouncer-mini watcher (2s) with PollWatcher (60s) ENOSPC fallback"
    - "Tokio 5-minute interval ticker (belt-and-braces sync fallback)"
    - "run_desktop_sync walker pipeline (walk_with_exclusions → diff_files → chunks(50) → SyncDirectoryFiles)"
    - "FfiApp::list_directory_fingerprints sync FFI + DirectoryFingerprint UniFFI Record"
  affects:
    - desktop/iced/src/main.rs
    - desktop/iced/src/views/directory_sources.rs
    - desktop/iced/src/views/mod.rs
    - desktop/iced/src/views/home.rs
    - desktop/iced/Cargo.toml
    - rust/src/lib.rs
tech_stack:
  added:
    - "notify-debouncer-mini 0.4 (desktop/iced)"
    - "tokio 1.x with rt-multi-thread + time + sync + macros features (desktop/iced)"
  patterns:
    - "Shared flume::unbounded::<Vec<PathBuf>> raw-event channel lets RecommendedWatcher and PollWatcher share one consumer thread"
    - "PollHandler struct (impl EventHandler) bridges fallback PollWatcher into the same event channel used by the debouncer"
    - "Per-session dir_watched_paths: Arc<Mutex<HashMap<source_id, path>>> keeps folder handles in-process without leaking bookmark_data/tree_uri across FFI (T-32-I2)"
    - "DirTriggerId(flume::Receiver<Message>) with Hash impl dedupes subscription batch entries across re-runs"
    - "50-file batch ceiling (chunks(50)) enforced in run_desktop_sync — double-defence alongside actor-side ceiling (T-32-DoS1 / D-25)"
    - "UniFFI Record DirectoryFingerprint avoids tuple-return (not supported by proc-macro bindgen)"
key_files:
  created:
    - desktop/iced/src/views/directory_sources.rs
  modified:
    - desktop/iced/src/main.rs
    - desktop/iced/src/views/mod.rs
    - desktop/iced/src/views/home.rs
    - desktop/iced/Cargo.toml
    - rust/src/lib.rs
decisions:
  - "Added FfiApp::list_directory_fingerprints + DirectoryFingerprint Record in the Rust core rather than enumerating from desktop directly. Rationale: the diff logic already lives in directory_sync::diff_files and the native side must not touch the persistence layer; this FFI method mirrors the read_encrypted_image pattern (reply channel + synchronous block_on)."
  - "Dropped new_debouncer_opt<PollWatcher> path in favour of raw PollWatcher + custom PollHandler because Debouncer<INotifyWatcher> and Debouncer<PollWatcher> are distinct generics — they cannot be unified under one match arm. Using a shared event channel keeps the downstream consumer identical for both backends."
  - "Tasks 1 (UI view + wiring) and 2 (watcher + pipeline) were committed together as a single atomic change. Rationale: main.rs modifications for the two tasks are deeply entangled (shared state fields, shared Message variants, shared subscription batch) and splitting them mid-file would have produced a commit that did not build."
  - "Added 'Sources' button to home sidebar between Documents and Settings — plan required the view to be reachable but left the entry-point unspecified."
metrics:
  duration: ~30min
  completed_date: 2026-04-19
  tasks_completed: 3
  commits: 2
---

# Phase 32 Plan 04: Desktop Directory-Sync UI + Watcher Summary

Desktop (iced) directory-source screen with rfd folder picker, inline exclusion editor, sync-now, and remove-confirm — backed by a notify-debouncer-mini watcher that falls back to PollWatcher on ENOSPC and a Tokio 5-minute belt-and-braces ticker. All filesystem changes flow through `walk_with_exclusions → list_directory_fingerprints → diff_files → chunks(50) → SyncDirectoryFiles`, satisfying DIR-02 (real-time sync), DIR-05 (sync pipeline), and DIR-06 (cascaded removal UX). Clean `cargo build -p mango-desktop`.

## What Shipped

### UI (`desktop/iced/src/views/directory_sources.rs`, 496 lines)
- **Header:** "Add folder" button (triggers `rfd::AsyncFileDialog::pick_folder`).
- **Fallback warning banner** (T-32-V4): rendered when `dir_watcher_warning` is set (e.g., "Real-time watching unavailable — polling every 60s").
- **Source rows:** path label + file-count + last-sync + sync status (`Idle|Syncing|Error`), plus Sync, Edit, Remove buttons.
- **Inline exclusion editor:** textarea (one glob per line) with `validate_glob_pattern` per line; Save disabled until all lines valid.
- **Default presets:** `default_exclusion_presets()` returns `[".git/**", "node_modules/**", "target/**", "*.tmp", ".DS_Store"]` offered on first edit.
- **Remove-confirm modal** (DIR-06): "Remove N documents and all embeddings?" → Cancel / Remove.

### Home integration (`desktop/iced/src/views/home.rs`)
- New "Sources" button in sidebar navigation → `Message::OpenDirectorySources`.

### Watcher & pipeline (`desktop/iced/src/main.rs`, +735 lines net)
- `spawn_directory_sync_workers(manager, trigger_tx, watched_paths)`:
  - **Thread 1 (watcher):** `new_debouncer(2s, ...)` with a `notify_debouncer_mini::Debouncer<RecommendedWatcher>`. On ENOSPC or watch-limit error from either `new_debouncer` or a subsequent `watch()`, switches to `PollWatcher` with `NotifyConfig::default().with_poll_interval(60s)`. Both backends push `Vec<PathBuf>` into a shared `flume::unbounded()` channel via the `PollHandler` bridge.
  - **Thread 2 (ticker):** Tokio multi-thread runtime driving `tokio::time::interval(Duration::from_secs(300))`. Emits `Message::DirSyncIntervalTick` on every tick.
  - **Thread 3 (state-subscriber):** Reads AppState via `manager.state_rx`, keeps `dir_watched_paths` in sync with current sources, re-builds `watch()` calls when sources are added/removed.
- `run_desktop_sync(manager, source_id, source_path, exclusions, done_tx)`:
  1. `walk_with_exclusions(path, &exclusions)` → current file fingerprints.
  2. `FfiApp::list_directory_fingerprints(source_id)` → stored fingerprints.
  3. `diff_files(&stored, &current)` → `{added, modified, removed}`.
  4. Added+modified read into `(relative_path, mtime, size, bytes)` tuples.
  5. `.chunks(50)` → one `AppAction::SyncDirectoryFiles` dispatch per chunk.
  6. Final `SyncDirectoryFiles` with the removed list for cascaded deletion.
- **Manual "Sync now"** button dispatches the same `run_desktop_sync` call synchronously (via a worker thread) — zero pipeline divergence.
- **Subscription batching:** `DirTriggerId(flume::Receiver<Message>)` + `dir_triggers` stream merged with the existing subscription batch via `iced::Subscription::batch`.

### Rust core FFI (`rust/src/lib.rs`, already committed as `71f4eb6` pre-summary)
- `DirectoryFingerprint` UniFFI Record: `{relative_path, mtime_secs, size_bytes}`.
- `CoreMsg::ListDirectoryFingerprints { source_id, reply }` + handler that converts `DirectoryFileRow` → `DirectoryFingerprint`.
- `FfiApp::list_directory_fingerprints(source_id) -> Result<Vec<DirectoryFingerprint>, String>`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Missing FFI method for native-side diff**
- **Found during:** Task 2 (pipeline wiring).
- **Issue:** `diff_files` requires a `&[StoredFingerprint]`, but `AppState` did not expose stored fingerprints across UniFFI.
- **Fix:** Added `DirectoryFingerprint` UniFFI Record + `CoreMsg::ListDirectoryFingerprints` variant + `FfiApp::list_directory_fingerprints` method (mirroring the `read_encrypted_image` synchronous-FFI pattern).
- **Files modified:** `rust/src/lib.rs`.
- **Commit:** `71f4eb6`.

**2. [Rule 1 - Bug] Debouncer type-mismatch in fallback path**
- **Found during:** Task 2.
- **Issue:** `new_debouncer_opt::<PollWatcher>` returns `Debouncer<PollWatcher>` which is a different type from `Debouncer<INotifyWatcher>` — unifying them under one `match` arm fails `E0308`.
- **Fix:** Introduced raw `PollWatcher` + custom `PollHandler` (`impl EventHandler`) that pushes into the same `flume::unbounded::<Vec<PathBuf>>` channel the debouncer uses. Downstream consumer thread is backend-agnostic.
- **Files modified:** `desktop/iced/src/main.rs`.
- **Commit:** `627145c`.

**3. [Rule 1 - Bug] StoredFingerprint field name (`relative_path` → `file_path`)**
- **Found during:** Task 2 build verification.
- **Issue:** `E0560`: `StoredFingerprint` has field `file_path`, not `relative_path`.
- **Fix:** One-line rename in the `map(|f| StoredFingerprint { ... })` call in `run_desktop_sync`.
- **Files modified:** `desktop/iced/src/main.rs`.
- **Commit:** `627145c`.

**4. [Rule 2 - Missing functionality] Home sidebar entry-point**
- **Found during:** Task 1.
- **Issue:** Plan required `DirectorySources` to be reachable but did not specify the entry-point; `OpenDirectorySources` was a dead-code variant with no emitter.
- **Fix:** Added "Sources" button to the home sidebar between Documents and Settings.
- **Files modified:** `desktop/iced/src/views/home.rs`.
- **Commit:** `627145c`.

### Task-merge

- Tasks 1 and 2 were committed together (`627145c`). Splitting the main.rs diff would have yielded a non-building intermediate commit since the UI view references Message variants defined and handled by the watcher/pipeline code. Per execution-flow guidance, commits must build — so atomic integration was preferred.

## Checkpoint: Task 3 (human-verify)

Auto-mode active (`workflow._auto_chain_active = true`). Task 3 `checkpoint:human-verify` auto-approved with the following verification surface documented for post-hoc review:

1. **Build:** `cargo build -p mango-desktop` → clean (1 pre-existing dead-code warning in `mango_core`).
2. **Launch:** `cargo run -p mango-desktop`.
3. **Add folder:** Click "Sources" in sidebar → "Add folder" → pick a directory with ≥1 text file.
4. **Initial sync:** Click "Sync" on the new source row → row status transitions `Idle → Syncing → Idle`, file count populates.
5. **Real-time watch:** Touch a new file inside the directory → within ~2s, a new `SyncDirectoryFiles` dispatch occurs (visible in logs).
6. **Exclusion edit:** Click Edit → add `*.log` → Save → recognized and persisted (globs validated inline).
7. **Remove:** Click Remove → confirm → row disappears, `RemoveDirectorySource` fires (cascade documented in 32-03 handler).
8. **Fallback banner:** On a system with inotify limit hit (`sysctl -w fs.inotify.max_user_watches=1`), banner renders and polling continues at 60s cadence.

## Known Stubs

None. All UI surfaces are wired to real AppState / AppAction paths.

## Deferred Issues

- `DirectoryFileRow.id` + `source_id` fields warn as dead-code in `mango_core` (pre-existing from 32-01/02 — not introduced by this plan). Out of scope for 32-04 auto-fix.

## Self-Check: PASSED

- `desktop/iced/src/views/directory_sources.rs` — FOUND (496 lines, exceeds plan's 250 minimum).
- `desktop/iced/src/main.rs` — MODIFIED (+735 lines for watcher + pipeline + state + handlers).
- Commit `71f4eb6` — FOUND (DirectoryFingerprint FFI).
- Commit `627145c` — FOUND (UI + watcher + pipeline).
- `cargo build -p mango-desktop` — PASSED (no errors).
