---
phase: 25-disable-enable-making-memories-in-the-app
plan: 01
subsystem: memory
tags: [rust, uniffi, settings, memory, toggle, persistence]

# Dependency graph
requires:
  - phase: 24-redesign-settings-ux
    provides: brave_api_key_set pattern (field in AppState, set_setting/get_setting, startup load, handler)
provides:
  - memories_enabled bool in AppState with default true
  - SetMemoriesEnabled action in AppAction enum
  - memories_enabled persisted via settings table
  - memory extraction gated behind memories_enabled in StreamDone
  - unit test for round-trip toggle persistence
affects:
  - 25-02 (platform UI plans will dispatch SetMemoriesEnabled to this Rust core)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Boolean setting toggle: get_setting/set_setting with '0'/'1' values, default via unwrap_or(true/false)"
    - "Feature gate in StreamDone: outermost guard checks app_state flag before should_extract"

key-files:
  created: []
  modified:
    - rust/src/lib.rs
    - rust/src/tests/settings.rs

key-decisions:
  - "memories_enabled defaults to true via unwrap_or(true) so existing users are unaffected on upgrade"
  - "Extraction gate placed as outermost condition before should_extract, not nested inside bid block"
  - "Persisted as string '0'/'1' (not boolean) consistent with other settings table entries"

patterns-established:
  - "Boolean settings: store as '0'/'1' string in settings table, load with .map(|v| v != '0').unwrap_or(default)"

requirements-completed:
  - MEM-TOGGLE-01
  - MEM-TOGGLE-02
  - MEM-TOGGLE-03

# Metrics
duration: 2min
completed: 2026-04-05
---

# Phase 25 Plan 01: Memories Toggle Rust Core Summary

**memories_enabled bool added to AppState with persistent toggle via settings table and memory extraction gated behind the flag in StreamDone**

## Performance

- **Duration:** 2 min
- **Started:** 2026-04-05T13:11:48Z
- **Completed:** 2026-04-05T13:13:33Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Added `memories_enabled: bool` to AppState (default `true`)
- Added `SetMemoriesEnabled { enabled: bool }` to AppAction enum
- Persisted toggle in settings table as `"memories_enabled"` key with `"1"`/`"0"` values
- Loads `memories_enabled` from settings at startup with default `true` (upgrade-safe)
- Gated memory extraction in StreamDone with `actor_state.app_state.memories_enabled` as outermost guard
- Added `test_memories_enabled_toggle` unit test — full round-trip: default true -> disable -> re-enable

## Task Commits

Each task was committed atomically:

1. **Task 1: Add memories_enabled to AppState, SetMemoriesEnabled to AppAction, handler, startup load, extraction gate, and unit test** - `48eb175` (feat)

**Plan metadata:** (to be committed with docs)

_Note: TDD task — test written first (RED), then implementation (GREEN). Both in single atomic commit per plan instructions._

## Files Created/Modified
- `rust/src/lib.rs` - AppState field, default, AppAction variant, startup load, handler, extraction gate
- `rust/src/tests/settings.rs` - test_memories_enabled_toggle unit test

## Decisions Made
- `memories_enabled` defaults to `true` so existing users who upgrade continue to have memory extraction enabled
- Extraction gate placed as outermost condition in StreamDone (`memories_enabled && should_extract(...)`) per plan spec — NOT nested inside `if let Some(bid)` block
- String `"0"` means disabled, any other value (including `"1"`) means enabled, consistent with settings table pattern

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Package name is `mango_core` (with underscore) not `mango-core` (with hyphen) — corrected cargo test invocation immediately.

## Next Phase Readiness
- Rust core complete; all three platform UIs (iOS SwiftUI, Android Compose, Desktop iced) can now dispatch `SetMemoriesEnabled` to toggle the behavior
- Plan 25-02 (platform UI wiring) can proceed immediately

---
*Phase: 25-disable-enable-making-memories-in-the-app*
*Completed: 2026-04-05*
