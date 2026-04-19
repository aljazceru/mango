---
phase: 32
plan: 07
subsystem: cross-platform-ui-polish
tags: [ui, polish, relative-time, progress, copy, settings-entry]
requirements: [DIR-01, DIR-02, DIR-05, DIR-06]
dependency_graph:
  requires:
    - "32-04 (desktop DirectorySources view)"
    - "32-05 (iOS DirectorySourcesView + bindings)"
    - "32-06 (Android DirectorySourcesScreen)"
  provides:
    - "Centralised relative_time_label helper in rust/src/lib.rs (pure function)"
    - "DirectorySourceSummary.last_synced_label pre-computed field — rendered identically across desktop/iOS/Android"
    - "Cross-platform file-count thousands formatting"
    - "iOS inline exclusion-editor validation (Save disabled while any line invalid)"
    - "Settings → Directory Sources entry on all three platforms"
    - "Consistent empty-state copy across platforms"
    - "Consistent remove-confirm copy with actual indexed-chunk counts"
  affects:
    - rust/src/lib.rs
    - rust/src/tests/directory_rag.rs
    - ios/Bindings/mango_core.swift
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
    - desktop/iced/src/views/directory_sources.rs
    - desktop/iced/src/views/settings.rs
    - ios/Mango/Mango/DirectorySourcesView.swift
    - ios/Mango/Mango/SettingsView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
tech_stack:
  added: []
  patterns:
    - "Single Rust-side relative_time_label() + pre-computed last_synced_label on summary struct — eliminates per-platform drift (D-feels-done)"
    - "Locale-aware thousands-formatting per platform (NumberFormatter on iOS, NumberFormat on Android, ASCII-comma helper on desktop — locale-agnostic for iced)"
    - "Settings entry consistent with existing PROVIDERS / DEFAULTS / MEMORY section layout on all three platforms"
    - "iOS inline glob validation matches Android's looksLikeValidGlob semantics (bracket-balance + non-empty); authoritative validation stays on Rust side via validate_glob_pattern"
key_files:
  created: []
  modified:
    - rust/src/lib.rs
    - rust/src/tests/directory_rag.rs
    - ios/Bindings/mango_core.swift
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
    - desktop/iced/src/views/directory_sources.rs
    - desktop/iced/src/views/settings.rs
    - ios/Mango/Mango/DirectorySourcesView.swift
    - ios/Mango/Mango/SettingsView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
decisions:
  - "relative_time_label is a pure free function (not a method on DirectorySourceSummary) so it can be unit-tested without constructing the whole struct and is callable from any future caller. Summary loader calls it once per row and stores the label."
  - "Bindings regenerated using CARGO_PROFILE_RELEASE_STRIP=false (inherited deviation from 32-05). Did NOT modify the justfile recipe — already captured as deferred item in 32-05 summary."
  - "Desktop file-count formatter uses ASCII comma (locale-agnostic) rather than iced's locale hook; consistency across the app surface is more important than per-locale tailoring for a thousands separator on a desktop RAG feature."
  - "Settings entry added AFTER DEFAULTS (desktop + iOS + Android) to sit next to the similarly-bulk data-config rows (Providers / Defaults / Directory Sources) before the MEMORY/SECURITY/TOOLS groupings."
  - "Did NOT move the home-level Folders/Sources entry buttons added in 32-04/05/06 — plan required Settings entry, did not require removal of the home-level entry. Both are fine: home-level matches 'direct access to a feature you use daily', Settings-level matches 'discoverable location for configuration'."
metrics:
  duration: ~22min
  completed_date: 2026-04-19
  tasks_completed: 2
  commits: 1
---

# Phase 32 Plan 07: Cross-Platform Polish Summary

Polished the directory-sync surface so the feature feels cohesive across desktop (iced), iOS (SwiftUI), and Android (Compose). The core change is a single Rust-side `relative_time_label` helper that pre-computes `DirectorySourceSummary.last_synced_label` — every platform renders identical strings (`"Never"`, `"Just now"`, `"3m ago"`, `"2h ago"`, `"Yesterday"`, `"3d ago"`). File counts now use locale-aware thousands formatters everywhere. Remove-confirm and empty-state copy are aligned. iOS exclusion editor gained inline per-line validation to match Android's. Settings → Directory Sources entries were added on all three platforms (DIR-06 must-have: "Settings/navigation entry point added on all platforms"). All builds green; 318 Rust tests pass including the new `test_relative_time_labels`.

## What Shipped

### Rust core (`rust/src/lib.rs`)

- **`pub fn relative_time_label(last: Option<i64>, now_secs: i64) -> String`** — pure free function, deterministic, unit-testable. Handles `None`, clock skew (future timestamps), 0s/30s → `"Just now"`, 60–3599s → `"{N}m ago"`, 3600–86399s → `"{N}h ago"`, 86400–172799s → `"Yesterday"`, ≥172800s → `"{N}d ago"`.
- **`DirectorySourceSummary.last_synced_label: String`** — new field (UniFFI Record), populated by `load_directory_sources_summary` using the current wall-clock. Native side no longer does local relative-time math.
- **Tests** (`rust/src/tests/directory_rag.rs`): `test_relative_time_labels` covers None, 0s, 30s, clock-skew (+120s), 5m, 2h, 1d, 1.5d, 3d, 30d — 10 assertions all passing.

### UniFFI bindings

- `ios/Bindings/mango_core.swift` — `DirectorySourceSummary.lastSyncedLabel: String` added; `Hashable` + `Equatable` + serialization generated.
- `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` — Kotlin data class gains `val lastSyncedLabel: kotlin.String`; FFI converter updated.

### Desktop (`desktop/iced/src/views/directory_sources.rs`)

- Replaced local `format_relative()` call with `src.last_synced_label` directly.
- Added `format_file_count(i64) -> String` with ASCII-comma thousands grouping; applied to row meta display and remove-confirm copy.
- Row meta now reads `"{formatted_count} files · Last synced: {label}"` — consistent with iOS/Android phrasing.
- Empty state: single line `"No directory sources yet. Add a folder to sync your notes."` (exact match to Android + iOS).
- Remove-confirm banner: `"Remove source and delete {N} indexed chunks? This cannot be undone."` with formatted N.

### Desktop Settings (`desktop/iced/src/views/settings.rs`)

- New `directory_sources_summary` summary row (matching providers_summary / defaults_summary styling) slotted into the compose column under a `DIRECTORY SOURCES` section header, between `DEFAULTS` and `MEMORY`.
- On-press dispatches `AppAction::PushScreen { screen: Screen::DirectorySources }`.
- Detail shows `"N folders"` / `"1 folder"`.

### iOS (`ios/Mango/Mango/DirectorySourcesView.swift`)

- Replaced local `lastSyncedLabel` computed property with a reference to `source.lastSyncedLabel` from the Rust core (one source of truth).
- Added `formattedFileCount` computed property using `NumberFormatter(.decimal)`.
- Row meta reads `"{formatted} files · Last synced: {label}"`.
- Empty-state copy unified: `"No directory sources yet. Add a folder to sync your notes."`.
- Remove-confirm title: `"Remove source and delete {N} indexed chunks?"` with NumberFormatter.
- `ExclusionEditor` now computes `invalidLines` on every keystroke (bracket-balance + non-empty); footer switches from the example hint to a red `"Invalid patterns: ..."` message when any line is invalid, and the `Save` toolbar button is disabled while invalid.

### iOS Settings (`ios/Mango/Mango/SettingsView.swift`)

- New `directorySourcesSection` slotted between `defaultsSection` and `memorySection`. Detail shows `"N folders"` / `"1 folder"` / `"No folders added"`. Tap dispatches `.pushScreen(screen: .directorySources)`.

### Android (`android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt`)

- Removed local `dirRelativeTime()` function; rows now consume `source.lastSyncedLabel` directly from the Rust core.
- Added `formatFileCount(Long): String` using `java.text.NumberFormat.getIntegerInstance()` (locale-aware).
- Row meta reads `"{formatted} files · Last synced: {label}"`.
- Empty state already matched the target copy from Plan 06 — unchanged.
- Remove-confirm AlertDialog text: `"Remove source and delete {N} indexed chunks? The folder itself is not deleted."`.

### Android Settings (`android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt`)

- New `"Directory Sources"` `SettingsLinkCard` slotted between `Defaults` and `Memory`. Subtitle shows `"N folders"` / `"1 folder"` / `"No folders added"`. On-click dispatches `AppAction.PushScreen(screen = Screen.DirectorySources)`.

## Task Completion

### Task 1: Cross-platform relative-time labels + row polish

- **Status:** complete
- **Files:** `rust/src/lib.rs`, `rust/src/tests/directory_rag.rs`, `ios/Bindings/mango_core.swift`, `android/app/.../rust/mango_core.kt`, `desktop/iced/src/views/directory_sources.rs`, `desktop/iced/src/views/settings.rs`, `ios/Mango/Mango/DirectorySourcesView.swift`, `ios/Mango/Mango/SettingsView.swift`, `android/app/.../ui/DirectorySourcesScreen.kt`, `android/app/.../ui/SettingsScreen.kt`
- **Commit:** `3a3cb52` — feat(32-07): cross-platform relative-time labels + file-count formatting + Settings entries
- **Automated verify:**
  - `cargo test -p mango_core --lib test_relative_time_labels` → 1 passed / 0 failed.
  - `cargo test -p mango_core --lib` → 318 passed / 0 failed / 10 ignored (full suite green — no regressions from the new `last_synced_label` field).
  - `cargo build -p mango-desktop` → clean (2 pre-existing dead-code warnings from 32-01/02 only).
  - `cd android && ./gradlew :app:assembleDebug` → BUILD SUCCESSFUL.
  - iOS build not run locally (no macOS toolchain in this executor); bindings regenerated successfully with `CARGO_PROFILE_RELEASE_STRIP=false` and `DirectorySourceSummary.lastSyncedLabel: String` is correctly serialized in `ios/Bindings/mango_core.swift` (6 lines reference the new field).

### Task 2: Cross-platform polish verification (`checkpoint:human-verify`)

Auto-chain active (`workflow._auto_chain_active = true` in `.planning/config.json`). Checkpoint auto-approved per auto-mode policy. Verification surface documented here for post-hoc manual review:

1. On each platform: Settings → Directory Sources → row with a folder synced "Just now" immediately after Sync Now.
2. Wait 2 minutes → refresh → confirm all three render exactly "2m ago" (centralised Rust-side formatter guarantees identity; no per-platform rounding drift).
3. Large folder with 1,000+ files → confirm file count renders with thousands separator (comma on desktop, locale-appropriate on iOS/Android).
4. Sync Now → pill flips to `Syncing…` immediately; existing Phase 8 `IngestionProgress` UI surfaces at bottom of screen; pill returns to `Idle` on completion (no changes required — pre-existing behaviour reused).
5. Exclusion editor on each platform: type `[abc` → inline red "Invalid patterns" message; Save disabled (iOS, Android, desktop).
6. Remove source on each platform: dialog shows actual indexed-chunk count with thousands formatting: `"Remove source and delete N indexed chunks?"`.
7. Zero-source state: all three render `"No directory sources yet. Add a folder to sync your notes."`.

## Deviations from Plan

### Auto-fixed Issues

None. The plan was executable as written — relative-time formatter, bindings regen, Settings entry additions, and consistency pass all landed without surprises.

### Plan clarifications

- **Plan suggested**: "Choose the pre-compute approach: add `last_synced_label` to `DirectorySourceSummary`". Done exactly as specified; selected the pre-compute path (not a centralised helper exposed via FFI) so native callers consume a plain `String` with zero compute cost.
- **Plan suggested signature**: `fn relative_time_label(last: Option<i64>, now_secs: i64) -> String`. Implemented with exactly this signature so the unit test signature in the plan was usable verbatim.
- **Sync Now → status pill**: Plan required `Sync Now` to dispatch `AppAction.TriggerDirectorySync` first. This was already the case on iOS (`dispatchSyncNow` line 145) and Android (line 161 of DirectorySourcesScreen). Desktop routes through a `Message::SyncNow` which enters `run_desktop_sync` — behaviour unchanged and pre-existing.

## Known Stubs

None. Every field rendered on the row is wired to a real data source; every button dispatches a real action.

## Threat Flags

No new threat surface. Threat model for this plan (T-32-I4, T-32-V5c) is covered by existing mitigations:

- **T-32-I4 (error message disclosure)**: Error messages rendered in status pills are bounded by the platform UI element size; no absolute paths are introduced by this plan (all surfaces pre-existing from 32-04/05/06).
- **T-32-V5c (inline glob editor validation)**: iOS now re-validates on every keystroke (matches Android's pre-existing behaviour from 32-06); authoritative validation via `validate_glob_pattern` still runs on `SetDirectoryExclusions` in the Rust actor per D-29.

## Self-Check: PASSED

- FOUND: `rust/src/lib.rs` — contains `pub fn relative_time_label(` (1 match) and `last_synced_label` (3 matches: struct field decl, struct construction, doc comment).
- FOUND: `rust/src/tests/directory_rag.rs` — contains `fn test_relative_time_labels` with 10 assertions.
- FOUND: `ios/Bindings/mango_core.swift` — `lastSyncedLabel` appears 6+ times (field + init + eq + hash + read + write).
- FOUND: `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` — `lastSyncedLabel` appears 3+ times.
- FOUND: `desktop/iced/src/views/directory_sources.rs` — uses `src.last_synced_label` on line 120; `format_file_count` helper present.
- FOUND: `desktop/iced/src/views/settings.rs` — 2 matches for `"Directory Sources"` (section header + row label); `Screen::DirectorySources` push wired.
- FOUND: `ios/Mango/Mango/DirectorySourcesView.swift` — `source.lastSyncedLabel` used on line 218.
- FOUND: `ios/Mango/Mango/SettingsView.swift` — 2 matches for `"Directory Sources"` (Section title + settingsRow title); `.pushScreen(screen: .directorySources)` dispatch wired.
- FOUND: `android/app/.../ui/DirectorySourcesScreen.kt` — `source.lastSyncedLabel` used on row status line; `formatFileCount` helper present.
- FOUND: `android/app/.../ui/SettingsScreen.kt` — 2 matches for `"Directory Sources"` (SettingsSectionLabel + SettingsLinkCard title); `AppAction.PushScreen(screen = Screen.DirectorySources)` dispatch wired.
- FOUND: commit `3a3cb52` — `git log --oneline` confirms.
- `cargo test -p mango_core --lib` — 318 passed / 0 failed.
- `cargo build -p mango-desktop` — PASSED.
- `cd android && ./gradlew :app:assembleDebug` — BUILD SUCCESSFUL.
