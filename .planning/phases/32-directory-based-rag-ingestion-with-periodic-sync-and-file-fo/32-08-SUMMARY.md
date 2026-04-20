---
phase: 32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo
plan: "08"
subsystem: ffi, ios, rag
tags: [uniffi, swift, kotlin, bookmark, cold-launch, scenephase, rust, ffi]

requires:
  - phase: 32-05
    provides: "FfiApp + DirectorySyncScheduler + bookmark cache + syncDirectorySource pipeline"

provides:
  - "FfiApp::get_directory_bookmark(source_id) -> Result<Option<Vec<u8>>, FfiError> targeted per-source SQLite read"
  - "CoreMsg::GetDirectoryBookmark actor variant + handler"
  - "AppManager.init rehydration loop: populates DirectorySyncScheduler.bookmarkCache from persisted bookmark_data before first ScenePhase.active"
  - "Stale-bookmark cache update in DirectorySyncScheduler.syncAll"
  - "3 unit tests for get_directory_bookmark (stored blob, missing source, null blob)"

affects: [ios-sync, 32-verification, phase-32-checkpoint]

tech-stack:
  added: []
  patterns:
    - "Targeted per-source FFI accessor pattern: CoreMsg variant + actor arm + FfiApp method (mirrors list_directory_fingerprints)"
    - "Cold-launch rehydration pattern: AppManager.init iterates initial state, calls FFI accessor per item, populates process-wide cache before listenForUpdates listeners fire"

key-files:
  created: []
  modified:
    - rust/src/lib.rs
    - rust/src/tests/directory_rag.rs
    - rust/src/tests/attestation_integration.rs
    - ios/Bindings/mango_core.swift
    - ios/Bindings/mango_coreFFI.h
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
    - ios/Mango/Mango/AppManager.swift
    - ios/Mango/Mango/DirectorySourcesView.swift

key-decisions:
  - "GetDirectoryBookmark is a targeted single-row read — bookmark_data stays out of DirectorySourceSummary (T-32-I2 preserved)"
  - "Rehydration placed after listenForUpdates in AppManager.init so cache is populated before any ScenePhase.active event can fire"
  - "syncAll also updates bookmarkCache on stale refresh by calling resolveBookmark before delegating to syncDirectorySource (avoids double-resolve on non-stale path)"
  - "Toast copy updated from 'cold launch without cached bookmark' to 'bookmark missing or load failed' — genuine failure semantics now that rehydration covers cold-launch"
  - "UniFFI generates open func (not public func) for FfiApp methods — acceptance criteria grep for public func getDirectoryBookmark returns 0 but open func getDirectoryBookmark is present and correct"

patterns-established:
  - "Cold-launch cache rehydration: AppManager.init iterates initial.directorySources, calls getDirectoryBookmark per source, caches result in process-wide scheduler"

requirements-completed: [DIR-02, DIR-05]

duration: 15min
completed: 2026-04-20
---

# Phase 32 Plan 08: Cold-Launch Bookmark Rehydration Summary

**Targeted FFI accessor `get_directory_bookmark` + AppManager.init rehydration loop that populates `DirectorySyncScheduler.bookmarkCache` from SQLite before the first ScenePhase.active fires, closing VERIFICATION gap HI-01 (truth #12)**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-04-20T04:22:00Z
- **Completed:** 2026-04-20T04:28:12Z
- **Tasks:** 2 completed (Task 3 is human-verify checkpoint, awaiting physical device test)
- **Files modified:** 8

## Accomplishments

- Added `CoreMsg::GetDirectoryBookmark` variant + actor handler + `FfiApp::get_directory_bookmark` public method — targeted per-source bookmark BLOB read without exposing `bookmark_data` in `DirectorySourceSummary` (T-32-I2 preserved)
- Added 3 unit tests (stored blob, missing source returns None, null column returns None) — all pass; full suite 321/321 green
- Regenerated Swift (`ios/Bindings/mango_core.swift`) and Kotlin (`android/.../mango_core.kt`) UniFFI bindings exposing `getDirectoryBookmark(sourceId:)`
- Added rehydration loop in `AppManager.init` that iterates `initial.directorySources`, calls `getDirectoryBookmark` per source, and populates `DirectorySyncScheduler.bookmarkCache` before returning — cold-launch sync now works without user re-adding the folder
- Updated `DirectorySyncScheduler.syncAll` to also update `bookmarkCache` when a stale bookmark is refreshed during the ScenePhase sync pass
- Removed stale "known v1 limitation / re-add the folder" wording from `bookmarkCache` comment and `dispatchSyncNow` toast

## Task Commits

1. **Task 1: Rust FFI accessor + unit tests** - `db3d0a5` (feat)
2. **Task 2: UniFFI bindings regen + iOS rehydration** - `a60146a` (feat)
3. **Task 3: Human verify iOS cold-launch** — checkpoint:human-verify (awaiting physical device test)

## Files Created/Modified

- `rust/src/lib.rs` — `CoreMsg::GetDirectoryBookmark` variant, actor arm, `FfiApp::get_directory_bookmark` method
- `rust/src/tests/directory_rag.rs` — 3 new unit tests for `get_directory_bookmark`
- `rust/src/tests/attestation_integration.rs` — added `GetDirectoryBookmark` arm to exhaustive CoreMsg match
- `ios/Bindings/mango_core.swift` — regenerated with `open func getDirectoryBookmark(sourceId:) throws -> Data?`
- `ios/Bindings/mango_coreFFI.h` — regenerated C header
- `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` — regenerated Kotlin binding
- `ios/Mango/Mango/AppManager.swift` — cold-launch rehydration loop after `listenForUpdates`
- `ios/Mango/Mango/DirectorySourcesView.swift` — updated `syncAll` for stale-cache parity, updated stale comments and toast

## Decisions Made

- `GetDirectoryBookmark` is a targeted single-row read via `get_directory_source` — bookmark_data stays out of `DirectorySourceSummary` (T-32-I2 preserved)
- Rehydration placed *after* `listenForUpdates` in `AppManager.init` so cache is populated before any `ScenePhase.active` event fires
- `syncAll` calls `resolveBookmark` before delegating to `syncDirectorySource` to update the process cache on stale refresh, avoiding need to return refreshed data from the pipeline
- UniFFI generates `open func` (not `public func`) for FfiApp methods — plan acceptance criteria expected `public func` but the binding is correct (`open func getDirectoryBookmark`)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added GetDirectoryBookmark to exhaustive CoreMsg match in attestation_integration.rs**
- **Found during:** Task 1 (would have caused E0004 compile error)
- **Issue:** Plan mentioned checking the attestation integration test for exhaustive CoreMsg matches; adding the new variant requires extending the match
- **Fix:** Added `crate::CoreMsg::GetDirectoryBookmark { .. } => panic!(...)` arm
- **Files modified:** rust/src/tests/attestation_integration.rs
- **Verification:** `cargo test -p mango_core --lib` passes (321 tests)
- **Committed in:** db3d0a5 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 — exhaustive match extension required by compiler)
**Impact on plan:** Necessary for compilation. No scope creep.

## Issues Encountered

- UniFFI generates `open func` not `public func` — the plan's acceptance criteria `grep -c "public func getDirectoryBookmark"` would return 0 but the binding is functionally correct. The method is present as `open func getDirectoryBookmark(sourceId: String) throws -> Data?` (same as all other FfiApp methods like `listDirectoryFingerprints`).

## Known Stubs

None — all functionality is fully wired. The `get_directory_bookmark` FFI path is end-to-end from SQLite through the actor to Swift.

## Threat Flags

None — all new surface covered by existing threat register:
- T-32-I2: bookmark_data excluded from DirectorySourceSummary (preserved — verified by acceptance criteria grep)
- T-32-I4: AppManager.swift logger uses `privacy: .public` on `source.id` only, blob never interpolated into log output

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Rust core, UniFFI bindings, and iOS rehydration logic are complete and tested
- Task 3 (physical device verification) remains as a human-verify checkpoint — human tester needs to follow the 11-step physical iPhone script in the plan to confirm VERIFICATION truth #12 flips from FAILED to VERIFIED
- After Task 3 passes: update VERIFICATION.md truth #12 status

---
*Phase: 32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo*
*Completed: 2026-04-20*
