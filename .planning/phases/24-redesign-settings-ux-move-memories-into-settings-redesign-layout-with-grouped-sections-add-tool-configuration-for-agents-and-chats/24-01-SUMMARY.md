---
phase: 24-redesign-settings-ux
plan: 01
subsystem: settings
tags: [rust, uniffi, sqlite, settings, memory, brave-search]

# Dependency graph
requires:
  - phase: 23-memory-management-ui-agent-ui
    provides: memories table, DeleteMemory/MemoryExtractionComplete handlers, MemorySummary type

provides:
  - AppState.memory_count u64 field loaded at startup and refreshed on delete/extraction
  - AppState.brave_api_key_set bool field loaded at startup, updated by SetBraveApiKey
  - SetBraveApiKey AppAction variant with handler following SetGlobalSystemPrompt pattern
  - Unit tests for both new fields (test_brave_api_key_persists, test_memory_count)

affects:
  - 24-02-android-settings-redesign
  - 24-03-ios-settings-redesign
  - Any platform UI that needs memory badge count or Brave key status

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "brave_api_key_set bool exposes key presence without leaking raw key across UniFFI boundary"
    - "memory_count re-queried via COUNT(*) after mutation (avoids off-by-one vs. decrement)"

key-files:
  created: []
  modified:
    - rust/src/lib.rs
    - rust/src/tests/settings.rs

key-decisions:
  - "memory_count is re-queried from DB (SELECT COUNT(*) FROM memories) after each mutation rather than incrementing/decrementing in-memory to avoid off-by-one errors"
  - "brave_api_key_set bool never exposes the raw API key across the UniFFI boundary; key is written to settings table only"
  - "SetBraveApiKey follows SetGlobalSystemPrompt handler pattern exactly"

patterns-established:
  - "Phase 24 bool-sentinel pattern: expose key presence (brave_api_key_set) not raw value for cross-FFI safety"

requirements-completed: [SET-04, SET-06]

# Metrics
duration: 8min
completed: 2026-04-05
---

# Phase 24 Plan 01: AppState Phase 24 Fields Summary

**memory_count u64 and brave_api_key_set bool added to AppState with startup loading, mutation-triggered refresh, and SetBraveApiKey action handler persisting to settings table**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-05T09:26:00Z
- **Completed:** 2026-04-05T09:34:52Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Added `memory_count: u64` and `brave_api_key_set: bool` to `AppState` with proper doc comments
- Added `SetBraveApiKey { api_key: String }` action variant following the `SetGlobalSystemPrompt` pattern
- Startup loading: both fields populated from SQLite at actor init (COUNT(*) for memories, get_setting for brave_api_key)
- Mutation tracking: `memory_count` refreshed in `DeleteMemory` and `MemoryExtractionComplete` handlers
- Unit tests `test_brave_api_key_persists` and `test_memory_count` added to `tests/settings.rs`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add AppState fields, SetBraveApiKey action, and startup loading** - `34f1ddb` (feat)

**Plan metadata:** pending docs commit

## Files Created/Modified
- `rust/src/lib.rs` - AppState new fields, Default impl, AppAction variant, startup loading, handler updates
- `rust/src/tests/settings.rs` - Added test_brave_api_key_persists and test_memory_count integration tests

## Decisions Made
- `memory_count` uses DB re-query on each mutation (not in-memory arithmetic) to avoid off-by-one bugs (per D-04)
- `brave_api_key_set` bool never crosses the FFI with the raw key value (per D-11)
- `SetBraveApiKey` clears the in-memory flag when given an empty string, but does not delete the DB row (consistent with SetGlobalSystemPrompt)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added unit tests for new fields**
- **Found during:** Task 1 (verification step)
- **Issue:** Plan's acceptance criteria required `test_brave_api_key_persists` and `test_memory_count` to pass, but no tests existed in the codebase
- **Fix:** Added both tests to `rust/src/tests/settings.rs` with actor integration (make_app/wait helpers)
- **Files modified:** rust/src/tests/settings.rs
- **Verification:** Both tests pass: `cargo test -p mango_core test_brave_api_key_persists` and `cargo test -p mango_core test_memory_count`
- **Committed in:** 34f1ddb (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (missing critical: unit tests required by acceptance criteria)
**Impact on plan:** Auto-fix necessary for acceptance criteria. No scope creep.

## Issues Encountered
- `cargo check -p mango-core` (hyphen) fails with package ID error; correct invocation is `cargo check -p mango_core` (underscore). No code issue.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `AppState.memory_count` and `AppState.brave_api_key_set` are ready for Android/iOS platform UI to consume
- `SetBraveApiKey` action is wired and persists; platform UIs can call it from the Tools settings section
- No blockers for 24-02 or 24-03

---
*Phase: 24-redesign-settings-ux*
*Completed: 2026-04-05*
