---
phase: 28
plan: "07"
subsystem: ios-ui + android-ui + desktop-iced
tags: [lock-timeout, background-lifecycle, settings, ios, android, desktop, swiftui, compose, iced]
dependency_graph:
  requires: [28-04, 28-05, 28-06]
  provides: []
  affects:
    - ios/Mango/Mango/ContentView.swift
    - android/app/src/main/java/dev/disobey/mango/MainActivity.kt
    - desktop/iced/src/main.rs
    - ios/Mango/Mango/SettingsView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
    - desktop/iced/src/views/settings.rs
tech_stack:
  added: []
  patterns:
    - iOS scenePhase observer for background/foreground detection
    - Android onPause/onResume lifecycle for background time tracking
    - Sentinel -1 for "Never" lock timeout with warning text
    - pick_list / Picker / ExposedDropdownMenuBox for lock timeout selection
key_files:
  created: []
  modified:
    - ios/Mango/Mango/ContentView.swift
    - android/app/src/main/java/dev/disobey/mango/MainActivity.kt
    - desktop/iced/src/main.rs
    - ios/Mango/Mango/SettingsView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
    - desktop/iced/src/views/settings.rs
decisions:
  - iOS uses @Environment(\.scenePhase) onChange observer; records backgroundedAt Date on .background, dispatches lockApp when elapsed > lockTimeoutSeconds on .active
  - Android onPause records System.currentTimeMillis(); onResume checks elapsed vs lockTimeoutSeconds * 1000L, dispatches LockApp
  - Desktop has no background/foreground lifecycle in iced — documented as explicit limitation; app locks on cold launch only
  - Sentinel -1 = Never (no auto-lock); 0 = Immediately (always lock on resume); both platforms handle both correctly
  - All platforms show orange warning text when "Never" (-1) is selected in Settings
  - Lock timeout picker options: Immediately (0s), 1 min (60s), 5 min (300s, default), 15 min (900s), Never (-1)
metrics:
  duration: "~20 minutes"
  completed: "2026-04-09"
  tasks_completed: 2
  tasks_total: 3
  files_changed: 6
---

# Phase 28 Plan 07: Background Lock Timeout and Settings Picker Summary

Background-to-foreground lock timeout wired on iOS and Android via platform lifecycle APIs; lock timeout configuration added to Settings on all three platforms with warning for "Never" selection.

## What Was Built

### Task 1: Background Lock Timeout (iOS + Android + Desktop)

**iOS (`ios/Mango/Mango/ContentView.swift`):**
- Added `@Environment(\.scenePhase) var scenePhase` and `@State private var backgroundedAt: Date?`
- `.onChange(of: scenePhase)` observer on the root `Group`:
  - On `.background`: records `backgroundedAt = Date()`
  - On `.active`: if `backgroundedAt` is set and `elapsed >= Double(lockTimeoutSeconds)` and `lockTimeoutSeconds >= 0`, dispatches `AppAction.lockApp`
  - Clears `backgroundedAt` on any active transition
- Sentinel -1 (Never) correctly skips the lock check

**Android (`android/app/src/main/java/dev/disobey/mango/MainActivity.kt`):**
- Added `private var backgroundedAt: Long = 0` field
- `onPause()` override: records `backgroundedAt = System.currentTimeMillis()`
- `onResume()` override: checks `elapsed >= lockTimeoutSeconds * 1000L` when `lockTimeoutSeconds >= 0`, dispatches `AppAction.LockApp`; clears `backgroundedAt` after check

**Desktop (`desktop/iced/src/main.rs`):**
- iced has no background/foreground lifecycle API — documented explicitly in the subscription method
- Desktop locks on cold launch only (Screen::Locked initial state from Rust core)
- This is the documented desktop limitation per the plan

### Task 2: Lock Timeout Picker in Settings (all platforms)

**Options on all platforms (per D-13):**
| Label | Seconds |
|-------|---------|
| Immediately | 0 |
| 1 minute | 60 |
| 5 minutes | 300 (default) |
| 15 minutes | 900 |
| Never | -1 |

**iOS (`ios/Mango/Mango/SettingsView.swift`):**
- Added `securitySection` private var with "Security" `Section` header
- `Picker` with `ForEach` over `lockTimeoutOptions`, bound to `appState.lockTimeoutSeconds`
- Dispatches `.setLockTimeout(seconds:)` on selection
- Orange warning text shown when `-1` (Never) selected

**Android (`android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt`):**
- Added SECURITY section header + Card in the `LazyColumn` after the MEMORY section
- `LockTimeoutPicker` composable using `ExposedDropdownMenuBox`
- Dispatches `AppAction.SetLockTimeout(seconds = option.seconds)` on selection
- Orange warning text (`Color(0xFFE65100)`) shown for Never selection

**Desktop (`desktop/iced/src/views/settings.rs`):**
- Added `LOCK_TIMEOUT_OPTIONS` const array and `lock_timeout_label()` helper
- `pick_list` dispatches `Message::DispatchAction(AppAction::SetLockTimeout { seconds })`
- SECURITY section header added between MEMORY and TOOLS in the compose column
- Orange warning text (`Color { r: 0.9, g: 0.4, b: 0.1, a: 1.0 }`) shown for Never

## Task Commits

1. **Task 1: Background lock timeout** — `dca028b` (feat)
2. **Task 2: Lock timeout Settings picker** — `eaae900` (feat)

## Deviations from Plan

None — plan executed exactly as written. Desktop limitation (no background/foreground lifecycle in iced) was explicitly anticipated in the plan and documented with a code comment.

## Known Stubs

None. All lock timeout values flow from real `AppState.lockTimeoutSeconds` (loaded from settings DB post-unlock in `load_post_unlock`). The picker dispatches real `SetLockTimeout` actions that persist to the settings table.

## Threat Flags

None — no new network endpoints, auth paths, file access patterns, or schema changes. The lock timeout setting is already in the threat model (T-28-25: stored in encrypted DB, cannot be changed without PIN/biometric).

## Self-Check

### Files Exist

- ios/Mango/Mango/ContentView.swift — FOUND (scenePhase observer added)
- android/app/src/main/java/dev/disobey/mango/MainActivity.kt — FOUND (onResume/onPause added)
- desktop/iced/src/main.rs — FOUND (desktop limitation documented)
- ios/Mango/Mango/SettingsView.swift — FOUND (securitySection added)
- android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt — FOUND (LockTimeoutPicker added)
- desktop/iced/src/views/settings.rs — FOUND (SECURITY section + pick_list added)

### Commits Exist

- dca028b: feat(28-07): wire background lock timeout on iOS, Android, and desktop — FOUND
- eaae900: feat(28-07): add lock timeout picker to Settings on all platforms — FOUND

### Build Verification

- `cargo build -p mango_core`: Finished (no errors)
- `cargo build --bin mango-desktop`: Finished (7 pre-existing warnings only, no errors)

## Self-Check: PASSED
