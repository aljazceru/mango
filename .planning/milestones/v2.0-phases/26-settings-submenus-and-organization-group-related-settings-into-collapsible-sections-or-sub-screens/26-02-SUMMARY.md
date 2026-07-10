---
phase: 26-settings-submenus-and-organization
plan: "02"
subsystem: ui
tags: [ios, android, settings, navigation, refactor]
dependency_graph:
  requires: [26-01]
  provides: [settings-providers-subscreen, settings-defaults-subscreen]
  affects: [ios-settings, android-settings]
tech_stack:
  added: []
  patterns:
    - "Push-screen navigation for Settings sub-screens (same pattern as Memories, Documents, Agents)"
    - "Summary row pattern: tappable row with context label (count/model name) and chevron"
key_files:
  created:
    - ios/Mango/Mango/SettingsProvidersView.swift
    - ios/Mango/Mango/SettingsDefaultsView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsProvidersScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsDefaultsScreen.kt
  modified:
    - ios/Mango/Mango/SettingsView.swift
    - ios/Mango/Mango/ContentView.swift
    - android/app/src/main/java/dev/disobey/mango/ui/SettingsScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
decisions:
  - "Helper functions duplicated into sub-screen files rather than shared to avoid creating a new shared util file (keep it simple for v1)"
  - "Android helper functions in SettingsProvidersScreen.kt use distinct names (healthLabelProviders, etc.) to avoid package-level name conflicts with the removed SettingsScreen.kt helpers"
metrics:
  duration: "7 minutes"
  completed: "2026-04-05"
  tasks_completed: 2
  files_modified: 8
---

# Phase 26 Plan 02: Settings Sub-screens for Providers and Defaults Summary

Extracted Providers and Defaults sections from Settings main screens on iOS and Android into dedicated push-navigation sub-screens, replacing the inline content with tappable summary rows showing contextual info (enabled count / active model name).

## What Was Built

**iOS:**
- `SettingsProvidersView` — full provider card list extracted from `SettingsView.providersSection`, includes `enabledRow`, `disabledRow`, all provider-specific helpers (health, attestation, teeType labels). Navigation via `appManager.dispatch(.popScreen)`.
- `SettingsDefaultsView` — model `Picker` and `TextEditor` for default instructions extracted from `SettingsView.defaultsSection`.
- `SettingsView` — `providersSection` and `defaultsSection` replaced with single-row Buttons dispatching `.pushScreen(screen: .settingsProviders)` and `.pushScreen(screen: .settingsDefaults)`. Removed `presetKeys`, `defaultModel`, `defaultInstructions`, `defaultInstructionsInitialized` state + all provider helpers.
- `ContentView` — added `case .settingsProviders` and `case .settingsDefaults` routing cases.

**Android:**
- `SettingsProvidersScreen` — full provider card `LazyColumn` extracted from `SettingsScreen`, includes all provider-specific helpers suffixed with `Providers` to avoid package-level collisions.
- `SettingsDefaultsScreen` — `ExposedDropdownMenuBox` model picker and default instructions `OutlinedTextField` extracted from `SettingsScreen`.
- `SettingsScreen` — PROVIDERS and DEFAULTS sections replaced with summary `Card` rows dispatching `AppAction.PushScreen(Screen.SettingsProviders)` and `AppAction.PushScreen(Screen.SettingsDefaults)`. Removed `presetKeys`, `defaultModelExp`, `defaultModel`, `defaultInstructions` state + `healthLabel`, `healthColor`, `attestationStyle`, `teeTypeLabel` helpers (kept `parseTeeType` + `teeTypeLabel` for Advanced section).
- `MainApp` — added `is Screen.SettingsProviders` and `is Screen.SettingsDefaults` routing cases.

## Decisions Made

1. **Helper functions duplicated, not shared** — Provider-specific helper functions (`healthLabel`, `healthColor`, `attestationStyle`, `teeTypeLabel`) are duplicated into the sub-screen files rather than extracted to a shared util. Avoids introducing a new utility file for v1; each screen is self-contained.

2. **Android helper names suffixed** — Android Kotlin package-level functions in `SettingsProvidersScreen.kt` use `*Providers` suffix (e.g., `healthLabelProviders`) to avoid package-level name collision with any remaining helpers in the same package. The original `SettingsScreen.kt` helpers were removed since they are now unreferenced.

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — all provider card content and defaults form content are fully wired to live AppState data dispatched via the existing actor/FFI pattern.

## Self-Check: PASSED
