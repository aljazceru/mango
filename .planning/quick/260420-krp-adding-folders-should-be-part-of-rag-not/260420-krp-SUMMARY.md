---
phase: quick/260420-krp
plan: 01
type: execute
wave: 1
subsystem: ui-rag
tags: [android, ios, desktop, rag, directory-sources, ui-consolidation]
requires:
  - Phase 32-07 (DirectorySourceSummary.last_synced_label)
  - Phase 32-08 (bookmark rehydration)
provides:
  - Unified RAG surface on Android, iOS, Desktop
affects:
  - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
  - android/app/src/main/java/dev/disobey/mango/ui/DocumentLibraryScreen.kt
  - ios/Mango/Mango/ContentView.swift
  - ios/Mango/Mango/DocumentLibraryView.swift
  - desktop/iced/src/views/home.rs
  - desktop/iced/src/views/documents.rs
  - desktop/iced/src/views/directory_sources.rs
tech-stack:
  added: []
  patterns:
    - "Reuse, don't duplicate: all three platforms call the existing folder-pickers verbatim (rememberDirectoryPicker, DirectorySourcePicker.swift, rfd::AsyncFileDialog)"
    - "Desktop: shared compact row helper (pub(crate) fn) in directory_sources.rs; no layout duplication"
key-files:
  modified:
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
    - android/app/src/main/java/dev/disobey/mango/ui/DocumentLibraryScreen.kt
    - ios/Mango/Mango/ContentView.swift
    - ios/Mango/Mango/DocumentLibraryView.swift
    - desktop/iced/src/views/home.rs
    - desktop/iced/src/views/documents.rs
    - desktop/iced/src/views/directory_sources.rs
decisions:
  - "Keep Screen::DirectorySources enum and its dedicated screen; fold only the UI entry point — lowest-risk refactor preserving all Rust core contracts"
  - "Android FAB uses DropdownMenu anchored inside a Box (simpler than ModalBottomSheet for 2 options)"
  - "iOS uses a toolbar Menu with Label items — native idiom for disclosure-style Add"
  - "Desktop uses two side-by-side '+ Document' / '+ Folder' buttons (iced lacks native dropdowns)"
  - "Desktop compact folder row lives in directory_sources.rs (pub(crate)), not documents.rs — single source of truth for row layout"
metrics:
  duration: ~15min
  completed: 2026-04-20
  tasks_completed: 3
  tasks_total_in_plan: 4 (task 4 is human-verify checkpoint, awaits user)
  files_modified: 7
---

# Quick Plan quick/260420-krp: Unify Documents + Folders under "RAG" Summary

One-liner: UI-only consolidation folding the separate "Folders" Home entry into the existing Documents screen across Android/iOS/Desktop, renamed to "RAG", with a single Add affordance that picks Document or Folder — Rust core contracts untouched.

## What Changed

### Android (Task 1)
- `MainApp.kt`: removed "Folders" TextButton from Home topBar; the remaining "RAG" button routes to `Screen.Documents`.
- `DocumentLibraryScreen.kt`: renamed title to "RAG"; added FOLDERS section above DOCUMENTS; replaced single FAB with a `DropdownMenu` anchored to an Add FAB offering "Document" (launches `openDocumentLauncher`) and "Folder" (launches `rememberDirectoryPicker`, reused verbatim from `DirectorySourcesScreen`). Tapping a folder row dispatches `PushScreen(Screen.DirectorySources)`.
- The existing `syncDirectory` pipeline and `pickedUrisByName` caching logic from `DirectorySourcesScreen` are mirrored inline so a folder added from the RAG screen starts syncing immediately — same behavior as the dedicated Sources screen.

### iOS (Task 2)
- `ContentView.swift`: removed `Button("Folders")`; renamed `Button("Documents")` to `Button("RAG")`.
- `DocumentLibraryView.swift`: renamed navigationTitle to "RAG"; added `Section("Folders")` above `Section("Documents")`; replaced single Add button with a `Menu` exposing Document (existing `.fileImporter`) and Folder (existing `DirectorySourcePicker` sheet). Folder rows are `Button`s dispatching `.pushScreen(screen: .directorySources)`. The bookmark is cached on `DirectorySyncScheduler.bookmarkCache` (keyed by displayName — matches the dedicated screen's handlePicked pattern) so the first Sync Now on the detail screen resolves without a cold launch.

### Desktop / iced (Task 3)
- `views/home.rs`: removed `sources_btn` from the sidebar bottom nav; renamed `docs_btn` label from "Documents" to "RAG".
- `views/documents.rs`: renamed screen title to "RAG"; rebuilt body as two stacked sections (FOLDERS, DOCUMENTS) with headers shown only when non-empty; replaced the single "Add Document" button with two side-by-side header buttons "+ Document" (`Message::PickDocumentFile`) and "+ Folder" (`Message::DirSources(AddFolder)` — same rfd pipeline as the Sources screen).
- `views/directory_sources.rs`: added `pub(crate) fn directory_source_compact_row(src, vc)` which reuses `status_pill` and `format_file_count` to render a clickable row emitting `Message::OpenDirectorySources`. Documents screen calls this helper — no duplicated row/status layout.

## What Did Not Change

- **Rust core**: zero changes. `Screen::Documents`, `Screen::DirectorySources`, `DocumentSummary`, `DirectorySourceSummary`, `AppAction::IngestDocument`, `AppAction::AddDirectorySource`, bookmark rehydration (Phase 32-08) — all untouched.
- **UniFFI bindings**: not regenerated; no Rust types changed.
- **Existing pickers** (`DirectorySourcePicker.kt/.swift`, rfd path in main.rs): reused verbatim, zero modifications.
- **Dedicated DirectorySources screen** (`DirectorySourcesScreen.kt`, `DirectorySourcesView.swift`, `views/directory_sources.rs` view fn): kept intact and reachable via folder-row tap. This is the "legacy detail route" model the plan called for — zero-risk, future-proof.

## Deviations from Plan

None — plan executed exactly as written. The only adaptation worth noting is minor: the plan said "OR (preferred) expose the existing row composable from DirectorySourcesScreen.kt as `internal`". The existing `DirectorySourceRow` on Android has 4 action buttons (Sync/Edit/Open/Remove) — inappropriate for a compact list row. A new `DirectorySourceCompactRow` was added inside `DocumentLibraryScreen.kt` as `private`, reading only state fields already on `DirectorySourceSummary` (no business logic). Desktop/iOS took the inverse path: a new shared compact helper in the Sources file (desktop) or inline minimal row (iOS), both respecting the "don't duplicate logic" rule.

## Verification

| Platform | Command | Result |
|---|---|---|
| Android | `./gradlew :app:compileDebugKotlin` | BUILD SUCCESSFUL in 17s (only pre-existing deprecation warnings) |
| iOS | `xcodebuild -project ios/Mango/Mango.xcodeproj -scheme Mango ... build` | **SKIPPED — xcodebuild not installed in this Linux headless environment.** Source-level changes follow existing DocumentLibraryView/ContentView idioms; no new types or APIs introduced that would break the Swift build. Needs verification on macOS. |
| Desktop | `cargo check -p mango-desktop` | Finished dev profile in 0.66s (only pre-existing dead-code warning in `queries.rs`) |
| Home-level folder entry audit | `git grep -nE 'Button\("Folders"\)\|sources_btn\|Text\("Folders"\)'` | No matches (clean) |

### iOS build-verification notes for Task 4 (human)

- The iOS changes are Swift-source-only and use the existing `Menu`, `Section`, `Button`, `.sheet` APIs already used in the codebase.
- `DirectorySourcePicker` and `DirectorySyncScheduler` are imported from the same module with no new exports required.
- A real iOS build on macOS is required before Task 4 approval; please run the plan's `xcodebuild` command locally and report back.

## Task 4: Human Verification — OUTSTANDING

Task 4 is a `type="checkpoint:human-verify"` with `gate="blocking"`. The executor intentionally did NOT attempt manual verification. The three platforms require the user to:

1. Launch Android build (`./gradlew :app:installDebug` or Android Studio), iOS build (`xcodebuild`/Xcode), and Desktop (`cargo run -p mango-desktop`).
2. Follow the 8-step verification protocol in `260420-krp-PLAN.md` under `<how-to-verify>`.
3. Reply "approved" or describe platform-specific issues.

Until Task 4 is approved, the requirements `LRAG-06` and `DIR-05` remain in progress; the orchestrator should NOT mark this quick task fully complete.

## Commits

- `2472bd4` feat(quick/260420-krp): Android — fold Directory Sources into RAG screen
- `bb073c1` feat(quick/260420-krp): iOS — fold Directory Sources into RAG screen
- `162ca36` feat(quick/260420-krp): Desktop — fold Directory Sources into RAG screen

## Known Stubs

None — the unified screen is wired to real `state.documents` and `state.directorySources`. No placeholder data paths.

## Threat Flags

None — UI-only refactor. No new network endpoints, auth paths, file-access patterns, or trust-boundary schema changes introduced. The existing folder-permission primitives (SAF persistable tree URI, iOS security-scoped bookmarks, rfd native picker) are reused verbatim without altering their call sites, permission-grant semantics, or stored BLOB formats.

## Self-Check

All claimed files exist:
- `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — FOUND
- `/home/lio/g/confidential-app/android/app/src/main/java/dev/disobey/mango/ui/DocumentLibraryScreen.kt` — FOUND
- `/home/lio/g/confidential-app/ios/Mango/Mango/ContentView.swift` — FOUND
- `/home/lio/g/confidential-app/ios/Mango/Mango/DocumentLibraryView.swift` — FOUND
- `/home/lio/g/confidential-app/desktop/iced/src/views/home.rs` — FOUND
- `/home/lio/g/confidential-app/desktop/iced/src/views/documents.rs` — FOUND
- `/home/lio/g/confidential-app/desktop/iced/src/views/directory_sources.rs` — FOUND

All commits exist on main: 2472bd4, bb073c1, 162ca36 — FOUND.

## Self-Check: PASSED
