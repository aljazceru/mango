---
phase: 22-agent-tools-expansion
plan: 02
subsystem: agent
tags: [rust, agent, dispatch_tools, brave-search, file-sandbox, react-loop, system-prompt]

# Dependency graph
requires:
  - phase: 22-agent-tools-expansion/22-01
    provides: Extended dispatch_tools signature with runtime/data_dir/brave_api_key, 4 new tool implementations
  - phase: 9-agent-system-background-execution
    provides: ActorState struct, handle_launch_agent_session, handle_resume_agent_session, ReAct loop
provides:
  - ActorState.data_dir field for agent file sandbox path
  - Proper brave_api_key lookup from SQLite settings before dispatch
  - dispatch_tools wired with runtime/data_dir/brave_api_key (replacing Plan 01 stubs)
  - Updated system prompts listing all 7 tools in both launch and resume code paths
affects: [agent-loop, lib.rs-dispatch-site, 23-agent-ui-re-enable]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - brave_api_key fetched via get_setting() at dispatch time (not cached) for fresh value on each step
    - data_dir initialized from vector_data_dir.clone() — reuses same directory as RAG index

key-files:
  created: []
  modified:
    - rust/src/lib.rs

key-decisions:
  - "Fetch brave_api_key fresh from settings DB at each dispatch_tools call — ensures key changes take effect without restart"
  - "data_dir initialized from vector_data_dir.clone() — both RAG and agent file sandbox share the same app data directory"

patterns-established:
  - "Per-step brave_api_key lookup: get_setting() called inline before dispatch, .unwrap_or(None).unwrap_or_default() for safe empty fallback"

requirements-completed: [TOOL-01, TOOL-02, TOOL-03, TOOL-04, TOOL-05]

# Metrics
duration: 5min
completed: 2026-04-04
---

# Phase 22 Plan 02: Agent Tools Wiring Summary

**dispatch_tools call site fully wired with runtime/data_dir/brave_api_key, ActorState.data_dir added, and both agent system prompts updated to list all 7 tools**

## Performance

- **Duration:** 5 min
- **Started:** 2026-04-04T15:45:00Z
- **Completed:** 2026-04-04T15:50:29Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Added `data_dir: String` field to `ActorState` struct (initialized from `vector_data_dir.clone()`)
- Removed Plan 01 stubs (`""`, `""`) from dispatch_tools call site in `handle_agent_step_complete`
- Added inline `get_setting("brave_api_key")` DB lookup before dispatch for fresh key on each step
- Updated `handle_launch_agent_session` system prompt to enumerate all 7 tools with descriptions
- Updated `handle_resume_agent_session` system prompt to enumerate all 7 tools with descriptions
- All 225 tests pass with zero failures

## Task Commits

Each task was committed atomically:

1. **Task 1: Add data_dir to ActorState and wire dispatch_tools call sites** - `167116d` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `rust/src/lib.rs` - ActorState.data_dir field, dispatch_tools wiring, both system prompts updated

## Decisions Made
- Fetch `brave_api_key` fresh via `get_setting()` at each dispatch call (not cached in ActorState) — ensures key changes from Settings UI take effect without restart
- `data_dir` initialized from `vector_data_dir.clone()` — same app data directory serves both the RAG vector index and the agent file sandbox

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Merged Plan 01 commits from main into worktree before applying changes**
- **Found during:** Task 1
- **Issue:** The worktree branch was created before Plan 01 commits landed on main. The worktree's `dispatch_tools` still had the old 4-parameter signature; applying Plan 02's 7-parameter call would not compile against the old signature.
- **Fix:** Fetched main from the primary repo (`/home/lio/g/confidential-app`) and fast-forward merged into the worktree branch. The stash pop caused a 2-hunk conflict in the dispatch_tools call area (upstream had the Plan 01 stub, stash had the Plan 02 wiring). Resolved by accepting the Plan 02 version (proper wiring) over the Plan 01 stub.
- **Files modified:** rust/src/lib.rs (conflict resolution)
- **Verification:** cargo check passes, all 225 tests pass
- **Committed in:** 167116d (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking)
**Impact on plan:** Required to unblock compilation. The merge brought in the correct dispatch_tools signature from Plan 01 so Plan 02's wiring could compile.

## Issues Encountered
- Worktree branch was 7 commits behind main (Plan 01 commits were only on main). Resolved via `git fetch /home/lio/g/confidential-app main && git merge FETCH_HEAD`.

## Next Phase Readiness
- Phase 22 complete: all 7 agent tools implemented, dispatched, and described in system prompts
- Agent ReAct loop now routes web_search, fetch_url, file, calculate to their implementations
- Phase 23 (agent UI re-enable) can proceed — the backend tool infrastructure is complete
- Brave API key can be set via the existing Settings persistence layer (get_setting/set_setting)

---
*Phase: 22-agent-tools-expansion*
*Completed: 2026-04-04*
