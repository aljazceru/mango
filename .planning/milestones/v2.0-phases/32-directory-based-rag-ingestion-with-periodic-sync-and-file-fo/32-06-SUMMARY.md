---
phase: 32
plan: 06
subsystem: android-ui
tags: [android, compose, saf, workmanager, documentscontract, directory-sync]
requirements: [DIR-02, DIR-05, DIR-06]
dependency_graph:
  requires:
    - "32-03 (AppAction variants, DirectorySourceSummary, DirectoryFileEntry, DirectorySyncStatus)"
    - "32-04 (DirectoryFingerprint + FfiApp::list_directory_fingerprints)"
    - "32-05 (regenerated Kotlin bindings with the above surface)"
  provides:
    - "android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt — OpenDocumentTree launcher + takePersistableUriPermission + bulk DocumentsContract traversal + readFileContent + GlobMatcher + syncDirectory() end-to-end pipeline"
    - "android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt — Compose list UI + Add/Edit/Remove + ExclusionEditorDialog + remove-confirm AlertDialog + LaunchedEffect auto-sync on add"
    - "android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt — CoroutineWorker + PeriodicWorkRequestBuilder(15, MINUTES) + resolveTreeUri via ContentResolver.persistedUriPermissions"
    - "AppManager.listDirectoryFingerprints wrapper (native-side diff)"
    - "MainActivity onCreate → DirectorySyncWorker.enqueue + onResume → foreground-resume sync for every source (locked-screen gated)"
    - "Folders button in home top bar → Screen.DirectorySources route"
  affects:
    - android/app/src/main/java/dev/disobey/mango/AppManager.kt
    - android/app/src/main/java/dev/disobey/mango/MainActivity.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt
tech_stack:
  added: []
  patterns:
    - "takePersistableUriPermission called immediately inside the picker's onResult (D-18 / Pitfall 4) — grant survives reboot"
    - "Bulk DocumentsContract.buildChildDocumentsUriUsingTree + ContentResolver.query per directory level (D-19 / Pitfall 5) — never DocumentFile.listFiles (10-100x slower IPC cost on large vaults)"
    - "COLUMN_LAST_MODIFIED divided by 1000 (ms → secs, D-20); mtime==0 fallback substitutes now-secs so file is always treated as modified (D-08)"
    - "Native-side diff against listDirectoryFingerprints — added/modified/removed computed in Kotlin, keeps tree URI out of UniFFI (T-32-I2 / D-02)"
    - "50-file batching via .chunked(50) — defence-in-depth against the Rust-side handler ceiling (T-32-DoS1)"
    - "Only the FIRST batch carries removedPaths so the cascade fires once; subsequent batches pass an empty list (matches iOS plan 32-05 ordering)"
    - "resolveTreeUri recovers persisted tree URI via ContentResolver.persistedUriPermissions keyed by displayName — no URI ever crosses UniFFI"
    - "PeriodicWorkRequestBuilder(15, MINUTES) + ExistingPeriodicWorkPolicy.KEEP → idempotent 15-minute background schedule (D-23)"
    - "onResume sync skipped when Screen.Locked is active — mirrors iOS ScenePhase gating (plan 32-05 security layering)"
    - "Lightweight local glob validation (bracket balance, non-empty) with authoritative globset re-validation on the Rust side via SetDirectoryExclusions (D-29 / T-32-V5)"
key_files:
  created:
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt
  modified:
    - android/app/src/main/java/dev/disobey/mango/AppManager.kt
    - android/app/src/main/java/dev/disobey/mango/MainActivity.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
decisions:
  - "Files placed under dev.disobey.mango.ui (real package) rather than com.mango (plan path). Project uses dev.disobey.mango throughout; com.mango does not exist. Documented as Deviation 1."
  - "Kotlin bindings NOT regenerated in this plan — plan 32-05 already regenerated android/app/.../mango_core.kt with the full DirectorySource / DirectoryFingerprint / AppAction surface. Plan 06 consumes the existing binding file."
  - "Tree URI resolution at sync time uses ContentResolver.persistedUriPermissions keyed by displayName rather than introducing an Android-specific SharedPreferences cache. Pro: no per-platform storage layer needed. Con: requires the displayName derivation to be stable across picks. Acceptable since the derivation itself is deterministic (last path segment of the tree document id)."
  - "onResume hook dispatches a full sync for every source rather than a TriggerDirectorySync-only nudge — Android's native layer is responsible for enumeration (D-01), so the resume path matches the Sync Now path with zero pipeline divergence."
  - "Folders button added to the home top bar between RAG and Settings — matches iOS plan 32-05 toolbar placement and desktop plan 32-04 sidebar placement. Plan left the entry-point unspecified."
  - "No AndroidManifest changes: SAF persistable grants cover file reads (no extra permissions), WorkManager default init was already active, and no <provider tools:node=\"remove\"> shim existed that would need to stay removed."
metrics:
  duration: ~12min
  completed_date: 2026-04-19
  tasks_completed: 3
  commits: 2
---

# Phase 32 Plan 06: Android Directory Sync UI + SAF + WorkManager Summary

Ships the Android half of the directory-sync feature: SAF folder picker with persistable read grant, bulk `DocumentsContract` traversal (never `DocumentFile.listFiles`), Compose source list with Sync-now / Edit / Remove-confirm, 15-minute `PeriodicWorkRequest`, and `onResume` foreground-resume sync. Satisfies DIR-02 (real-time sync), DIR-05 (sync pipeline), DIR-06 (cascaded removal UX). Android `:app:assembleDebug` BUILD SUCCESSFUL.

## What Shipped

### `DirectorySourcePicker.kt` (265 lines)
- `rememberDirectoryPicker(onPicked)` — `ActivityResultContracts.OpenDocumentTree()` launcher. On pick: `takePersistableUriPermission(uri, FLAG_GRANT_READ_URI_PERMISSION)` called inline (D-18 / Pitfall 4), display name derived from `DocumentsContract.getTreeDocumentId(uri)` last segment.
- `SafChildEntry(docId, name, lastModifiedMs, sizeBytes, isDirectory)`.
- `traverseTree(context, treeUri, exclusionGlobs)` — BFS stack, bulk `DocumentsContract.buildChildDocumentsUriUsingTree` + `ContentResolver.query` per directory level (D-19 / Pitfall 5 — never `DocumentFile.listFiles`). Projection reads COLUMN_DOCUMENT_ID / DISPLAY_NAME / LAST_MODIFIED / SIZE / MIME_TYPE. `GlobMatcher` filters both directories and files.
- `readFileContent(context, treeUri, docId)` — streams bytes via `openInputStream(buildDocumentUriUsingTree)`.
- `GlobMatcher` — minimal matcher for directory prefixes, extension globs, dotfile literals. Authoritative validation stays on Rust side (D-29).
- `DEFAULT_EXCLUSION_PRESETS = [.obsidian/, .trash/, *.tmp, *.canvas, .git/]` — matches iOS preset set.
- `syncDirectory(context, source, treeUri, dispatch)` — end-to-end: traverse → `AppManager.listDirectoryFingerprints(source.id)` → native diff (added/modified/removed) → 50-file `.chunked(50)` dispatch loop → D-20 `/ 1000` for mtime, D-08 fallback for mtime==0, first-batch-carries-removedPaths ordering, final `isFinalBatch = true`. Empty-changed case sends a single final no-op batch so the actor can clear `IngestionProgress` and update `last_synced_at`.

### `DirectorySourcesScreen.kt` (263 lines)
- `LazyColumn` of `DirectorySourceSummary` rows with Folder icon, `DirectorySyncStatus` badge text (Idle/Syncing/Error), "X files · synced Yh ago" line via `dirRelativeTime()`.
- Floating "+" action → `rememberDirectoryPicker` → dispatches `AppAction.AddDirectorySource(displayName, path=null, bookmarkData=null, treeUri=uri.toString(), exclusionGlobs=DEFAULT_EXCLUSION_PRESETS)`.
- `LaunchedEffect(appState.directorySources.map { it.id to it.fileCount })` — auto-triggers `syncDirectory` for any source whose fileCount is still 0 and whose picked URI we have cached (initial sync after add).
- Per-row `Sync now` / `Edit` / `Remove` outlined buttons.
- `ExclusionEditorDialog` — `OutlinedTextField` monospace multiline, one glob per line, inline invalid-pattern hint, Save disabled until all lines valid. Save dispatches `AppAction.SetDirectoryExclusions`.
- Remove flow: `AlertDialog` with per-source file-count message → destructive-colored Remove button dispatches `AppAction.RemoveDirectorySource` (DIR-06).
- Empty state copy: "No directory sources yet. Add a folder to sync your notes."

### `DirectorySyncWorker.kt` (98 lines)
- `class DirectorySyncWorker : CoroutineWorker` with `doWork()` iterating `AppManager.getInstance(context).state.directorySources`, resolving each tree URI, calling the shared `syncDirectory()` pipeline. Per-source try/catch — one bad source does not poison the run.
- `companion object.enqueue(context)` — `PeriodicWorkRequestBuilder<DirectorySyncWorker>(15, TimeUnit.MINUTES).build()` + `enqueueUniquePeriodicWork(UNIQUE_NAME, KEEP, req)` (D-23, idempotent).
- `internal fun resolveTreeUri(context, source)` — iterates `ContentResolver.persistedUriPermissions`, derives a displayName from each URI using the same rule the picker uses, matches against `source.displayName`. Keeps tree URI out of UniFFI (T-32-I2).

### `AppManager.kt` (modified)
- `fun listDirectoryFingerprints(sourceId: String): List<DirectoryFingerprint>` wrapper → `ffiApp.listDirectoryFingerprints(sourceId)` (mirrors `readEncryptedImage`).
- Added `directorySources = emptyList()` to the bootstrap `AppState(...)` scaffold so the constructor matches the latest binding shape.

### `MainActivity.kt` (modified)
- `onCreate` calls `DirectorySyncWorker.enqueue(applicationContext)` after `AppManager` init.
- `onResume` dispatches `syncDirectory` for every source, gated on `Screen.Locked` (matches iOS ScenePhase gate from plan 32-05).

### `MainApp.kt` (modified)
- Added `is Screen.DirectorySources -> DirectorySourcesScreen(...)` route.
- Added "Folders" `TextButton` in the home top bar between "RAG" and "Settings".

## Tasks

### Task 1: Regenerate bindings + DirectorySourcePicker (SAF + bulk traverse + batched dispatch)
- **Files:** `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt`, `android/app/src/main/java/dev/disobey/mango/AppManager.kt`
- **Commit:** `350fa81` — feat(32-06): Android SAF directory picker + bulk traversal + batched sync (Task 1)
- Bindings regen skipped — plan 32-05 already regenerated the full directory surface into `android/app/.../rust/mango_core.kt` (grep confirms `DirectoryFileEntry`, `DirectoryFingerprint`, `DirectorySourceSummary`, `DirectorySyncStatus`, 6 AppAction variants, `listDirectoryFingerprints`, `Screen.DirectorySources` all present).

### Task 2: DirectorySourcesScreen + DirectorySyncWorker + MainActivity onResume + Manifest review
- **Files:** `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt`, `android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt`, `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt`, `android/app/src/main/java/dev/disobey/mango/MainActivity.kt`
- **Commit:** `036c1ea` — feat(32-06): DirectorySourcesScreen + DirectorySyncWorker + onResume sync (Task 2)
- AndroidManifest.xml inspected — no changes needed (SAF grants cover file reads, no `<provider tools:node="remove" ...>` disabling WorkManager's default init, no `POST_NOTIFICATIONS` required since the worker posts no notifications).

### Task 3: Human-verify on device (auto-approved)

Auto-chain is active (`workflow._auto_chain_active = true`). This Linux-only executor cannot run an Android emulator or device test; Task 3 `checkpoint:human-verify` is auto-approved per auto-mode policy. The verification surface is documented here for post-hoc manual review on a physical device:

1. Build and install the debug APK (build currently green: `./gradlew :app:assembleDebug` → BUILD SUCCESSFUL).
2. Home → Folders → "+" FAB → pick an Obsidian-style folder via SAF.
3. Confirm initial sync indexes `.md` files and skips `.obsidian/` / `.trash/`.
4. **Reboot device.** Reopen app → Folders → tap Sync now → confirm sync still succeeds (SAF persistable permission survived reboot — Pitfall 4 covered by inline `takePersistableUriPermission`).
5. Modify a file via another app → background Mango for ≥15 min → reopen → change reflected (caught by `DirectorySyncWorker`).
6. Modify a file → background < 15s → foreground → `onResume` triggers the full sync pass and reindexes the modified file.
7. Large-vault perf: folder with 500+ files should complete initial traversal in <~5s on a mid-range device (bulk `DocumentsContract.query` per directory level).
8. Edit exclusions → save → next sync honours the new rules; malformed `[abc` glob shows the inline "Invalid patterns" hint and disables Save.
9. Remove source → confirmation dialog shows file count → Remove → all chunks cascade-delete (Rust handler from plan 32-03 handles the cascade).

## Deviations from Plan

### 1. [Rule 3 — Blocking] Plan references `com.mango` package; actual Android package is `dev.disobey.mango`

- **Found during:** Task 1, first attempt to read `android/app/src/main/java/com/mango/bindings/mango.kt`.
- **Issue:** The plan's `files_modified` and `<action>` blocks both reference paths under `android/app/src/main/java/com/mango/`. No such directory exists — the project's Android package is `dev.disobey.mango` (set in `android/app/build.gradle.kts` as `applicationId = "dev.disobey.mango"` and `namespace = "dev.disobey.mango"`; bindings are generated under `dev.disobey.mango.rust`, not `com.mango.bindings`).
- **Fix:** All three new files placed under `android/app/src/main/java/dev/disobey/mango/ui/` next to the existing `DocumentLibraryScreen.kt`, `AgentWorker.kt`, `MainApp.kt`, etc. Package declaration is `package dev.disobey.mango.ui`. Imports reference `dev.disobey.mango.rust.*`. The binding file referenced by the plan as `android/app/src/main/java/com/mango/bindings/mango.kt` is actually `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` — already regenerated by plan 32-05.
- **Files affected:** Path choice only; semantics identical to the plan.
- **Commit:** `350fa81` / `036c1ea`.

### 2. [Rule 2 — Missing functionality] `FfiApp::listDirectoryFingerprints` not exposed on AppManager

- **Found during:** Task 1, `syncDirectory` needs stored fingerprints for native diff.
- **Issue:** `AppManager.ffiApp` is `private`; only `readEncryptedImage` had a public wrapper. Without a wrapper the picker could not call `listDirectoryFingerprints`.
- **Fix:** Added `fun listDirectoryFingerprints(sourceId: String): List<DirectoryFingerprint>` to `AppManager` next to `readEncryptedImage` (same pattern: single-line delegation to `ffiApp`). Import for `DirectoryFingerprint` added.
- **Files modified:** `android/app/src/main/java/dev/disobey/mango/AppManager.kt`.
- **Commit:** `350fa81`.

### 3. [Rule 1 — Bug] `AppState` constructor in `AppManager` missing the new `directorySources` argument

- **Found during:** Task 1 build — `e: AppManager.kt:92:13 No value passed for parameter 'directorySources'.`
- **Issue:** Plan 32-05's binding regen added the `directorySources: List<DirectorySourceSummary>` field to `AppState` but never updated the bootstrap scaffold in `AppManager.init`. The compile broke the moment anything else in the module was touched.
- **Fix:** Added `directorySources = emptyList()` to the initial `AppState(...)` scaffold. Pre-existing gap surfaced by the first unrelated edit — noted as a bug in plan 32-05's regen scope but fixed here under Rule 1.
- **Files modified:** `android/app/src/main/java/dev/disobey/mango/AppManager.kt`.
- **Commit:** `350fa81`.

### 4. [Rule 1 — Bug] Name collision with existing `relativeTime(epochMillis: Long)`

- **Found during:** Task 2 build — `e: DirectorySourcesScreen.kt: Overload resolution ambiguity between candidates: fun relativeTime(epochMillis: Long): String; fun relativeTime(epochSecs: Long): String`.
- **Issue:** `MemoryScreen.kt` (or another existing file in `dev.disobey.mango.ui`) already defines a private top-level `relativeTime(epochMillis: Long)` that resolves in this package; my new one has the same signature.
- **Fix:** Renamed my function to `dirRelativeTime(epochSecs: Long)` and updated the one call site.
- **Files modified:** `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt`.
- **Commit:** `036c1ea`.

### 5. [Rule 2 — Missing functionality] Home top-bar entry point for directory sources

- **Found during:** Task 2 — plan required `Screen.DirectorySources` to be reachable but left the entry-point unspecified.
- **Fix:** Added "Folders" `TextButton` to the `ConversationListScreen`-hosted top bar (wired in `MainApp.kt`) between "RAG" and "Settings". Matches desktop plan 32-04 "Sources" sidebar button and iOS plan 32-05 "Folders" toolbar button for cross-platform consistency.
- **Files modified:** `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt`.
- **Commit:** `036c1ea`.

## Acceptance Criteria

- [x] `grep "ActivityResultContracts.OpenDocumentTree" DirectorySourcePicker.kt` = 1.
- [x] `grep "takePersistableUriPermission" DirectorySourcePicker.kt` = 4 (≥1 required; docstrings + the actual call + FLAG constant reference).
- [x] `grep "buildChildDocumentsUriUsingTree" DirectorySourcePicker.kt` = 2 (≥1 required).
- [x] `grep -R "DocumentFile.listFiles" android/app/src/main/java/dev/disobey/mango/` = only 2 matches and both are in DirectorySourcePicker.kt docstring comments telling you NEVER to use it (Pitfall 5 avoided in actual code).
- [x] `grep "COLUMN_LAST_MODIFIED\|lastModifiedMs" DirectorySourcePicker.kt` = 4 (≥1 required).
- [x] `grep "/ 1000" DirectorySourcePicker.kt` = 2 (≥1 required — D-20).
- [x] `grep "chunked(50)" DirectorySourcePicker.kt` = 1.
- [x] `grep "AddDirectorySource\|SyncDirectoryFiles" android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` = 12 (≥2 required — bindings already regenerated by plan 32-05).
- [x] File `DirectorySourcesScreen.kt` exists.
- [x] File `DirectorySyncWorker.kt` exists.
- [x] `grep "PeriodicWorkRequestBuilder<DirectorySyncWorker>(15" DirectorySyncWorker.kt` = 1.
- [x] `grep "enqueueUniquePeriodicWork" DirectorySyncWorker.kt` = 1.
- [x] `grep "onResume" MainActivity.kt` with syncDirectory dispatch = 5 matches (imports + override + inner call).
- [x] `grep "AlertDialog" DirectorySourcesScreen.kt` = multiple (remove-confirm + exclusion editor both use AlertDialog).
- [x] Android build succeeds: `cd android && ./gradlew :app:assembleDebug` → BUILD SUCCESSFUL (8 executed, 29 up-to-date).

## Known Stubs

None. All three UI artefacts are wired to real AppState / AppAction paths and real SAF + WorkManager APIs. The one simplification is the lightweight glob validation (bracket balance + non-empty) in `ExclusionEditorDialog` — the authoritative `validate_glob_pattern` runs server-side on `SetDirectoryExclusions` per D-29, which will reject anything the lightweight check misses.

## Known Limitations

- **Tree URI resolution uses displayName matching.** `resolveTreeUri()` iterates `ContentResolver.persistedUriPermissions` and matches by the displayName rule the picker uses. If two folders happen to share the same last path segment, the resolver may pick the wrong one. Acceptable for v1 (Obsidian vaults have unique names); a future plan can add a SharedPreferences `id → uri` cache.
- **No bindings regen step in this plan.** Plan 32-05 left the Kotlin bindings already regenerated with the full directory surface. If bindings drift in a future plan without a regen here, `DirectorySourcePicker.syncDirectory` will fail to compile — acceptable signal.

## Threat Flags

No new surface beyond the plan's `<threat_model>`:

- **T-32-V4 (access control):** `takePersistableUriPermission(uri, FLAG_GRANT_READ_URI_PERMISSION)` called immediately in the picker result callback; URI scope enforced by Android OS per UID.
- **T-32-V4c (multi-user device):** Android OS enforces SAF grants per UID — no mitigation needed at app layer.
- **T-32-DoS4 (large vault traversal):** Bulk `DocumentsContract.query` per directory level avoids the 10-100x IPC cost of `DocumentFile.listFiles` (Pitfall 5); `.chunked(50)` + 50-file batch ceiling cap memory across the UniFFI boundary (T-32-DoS1 defence-in-depth).
- **T-32-DoS5 (aggressive polling):** `PeriodicWorkRequestBuilder(15, MINUTES)` is the platform minimum (D-23); no network constraint means no radio wake.
- **T-32-I2 (summary leak prevention):** Tree URI never crosses UniFFI. It lives in `ContentResolver.persistedUriPermissions` and is recovered at sync time via `resolveTreeUri()` by matching displayName only. `DirectorySourceSummary` carries no URI / bookmark / path.

## Self-Check: PASSED

- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt` (265 lines, plan required ≥180).
- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt` (263 lines, plan required ≥200).
- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt` (98 lines, plan required ≥60).
- FOUND: `android/app/src/main/java/dev/disobey/mango/AppManager.kt` (modified — listDirectoryFingerprints + directorySources init).
- FOUND: `android/app/src/main/java/dev/disobey/mango/MainActivity.kt` (modified — enqueue + onResume).
- FOUND: `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` (modified — Screen.DirectorySources route + Folders button).
- FOUND: commit `350fa81` — Task 1 DirectorySourcePicker + AppManager wrapper.
- FOUND: commit `036c1ea` — Task 2 DirectorySourcesScreen + DirectorySyncWorker + onResume + Folders button.
- `cd android && ./gradlew :app:assembleDebug` — BUILD SUCCESSFUL.
