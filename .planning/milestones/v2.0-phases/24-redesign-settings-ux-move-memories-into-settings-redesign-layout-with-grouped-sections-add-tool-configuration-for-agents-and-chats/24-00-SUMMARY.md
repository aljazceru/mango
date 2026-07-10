---
phase: 24-redesign-settings-ux
plan: 00
subsystem: testing
tags: [rust, sqlite, rusqlite, persistence, settings, memories]

# Dependency graph
requires:
  - phase: 22-agent-tools-expansion
    provides: brave_api_key setting storage in settings table
  - phase: 20-memory-core
    provides: memories table with insert_memory / delete_memory / MemoryRow
provides:
  - Failing test stubs for SET-04 (brave_api_key persistence) and SET-06 (memory count)
affects: [24-01-plan]

# Tech tracking
tech-stack:
  added: []
  patterns: [in-memory SQLite Database::open(":memory:") test pattern, direct persistence layer test without actor]

key-files:
  created: []
  modified:
    - rust/src/tests/settings.rs

key-decisions:
  - "MemoryRow.usearch_key field required in test stub (plan's interface snippet was incomplete) -- set to 1 as placeholder"

patterns-established:
  - "Persistence layer tests use Database::open(':memory:') and query functions directly without going through the actor"

requirements-completed: [SET-04, SET-06]

# Metrics
duration: 3min
completed: 2026-04-05
---

# Phase 24 Plan 00: Settings Test Stubs Summary

**Wave 0 unit test stubs for brave_api_key persistence round-trip (SET-04) and memory COUNT(*) accuracy (SET-06) added to settings.rs**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-05T09:31:30Z
- **Completed:** 2026-04-05T09:33:31Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added `test_brave_api_key_persists`: verifies None initial state, set, and overwrite round-trip for `brave_api_key` in the settings table
- Added `test_memory_count`: verifies COUNT(*) returns 0 initially, 1 after `insert_memory`, and 0 again after `delete_memory`
- Both tests compile and pass (GREEN) against the in-memory SQLite persistence layer

## Task Commits

1. **Task 1: Add test_brave_api_key_persists and test_memory_count stubs** - `ed39763` (test)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `rust/src/tests/settings.rs` - Two new test functions appended: `test_brave_api_key_persists` and `test_memory_count`

## Decisions Made

- `MemoryRow.usearch_key` field set to `1` as a placeholder integer in the test -- the plan's interface snippet omitted this field, but the actual struct requires it. This is a minor deviation; the usearch_key is not meaningful for the count test.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] MemoryRow missing usearch_key field**
- **Found during:** Task 1 (Add test stubs)
- **Issue:** Plan's `<interfaces>` snippet showed `MemoryRow` without `usearch_key: i64`, but the actual struct at `queries.rs:811` requires it
- **Fix:** Added `usearch_key: 1` to the `MemoryRow` literal in `test_memory_count`
- **Files modified:** rust/src/tests/settings.rs
- **Verification:** `cargo test -p mango_core test_memory_count` passes
- **Committed in:** ed39763 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug -- incomplete interface in plan)
**Impact on plan:** Minimal -- single field added to test struct literal. No behavioral impact.

## Issues Encountered

None beyond the MemoryRow field deviation above.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Plan 00 stubs are GREEN and committed; Plan 01 implementation can proceed
- Both `test_brave_api_key_persists` and `test_memory_count` verify persistence layer behavior that Plan 01 will wire into AppState handlers

## Known Stubs

None -- tests are complete and passing, not stubs.

---
*Phase: 24-redesign-settings-ux*
*Completed: 2026-04-05*
