---
phase: 23
slug: memory-management-ui-agent-ui
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-04
---

# Phase 23 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust integration tests (`#[tokio::test]`) |
| **Config file** | `rust/Cargo.toml` |
| **Quick run command** | `cargo test -p confidential-app-core --lib` |
| **Full suite command** | `cargo test -p confidential-app-core` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p confidential-app-core --lib`
- **After every plan wave:** Run `cargo test -p confidential-app-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 23-01-01 | 01 | 1 | MEM-04 | unit | `cargo test test_list_memories` | Y | pending |
| 23-01-02 | 01 | 1 | MEM-05 | unit | `cargo test test_delete_memory` | Y | pending |
| 23-01-03 | 01 | 1 | MEM-06 | unit | `cargo test test_update_memory` | W0 (23-01 Task 2) | pending |
| 23-01-04 | 01 | 1 | AUI-02 | unit | `cargo test test_agent_step_tool_input` | W0 (23-01 Task 2) | pending |
| 23-01-05 | 01 | 1 | MEM-04 | integration | `cargo test test_list_memories_action` | W0 (23-01 Task 2) | pending |
| 23-01-06 | 01 | 1 | MEM-05 | integration | `cargo test test_delete_memory_action` | W0 (23-01 Task 2) | pending |
| 23-01-07 | 01 | 1 | MEM-06 | integration | `cargo test test_update_memory_action` | W0 (23-01 Task 2) | pending |
| 23-01-08 | 01 | 1 | MEM-04 | integration | `cargo test test_memories_screen_navigation` | W0 (23-01 Task 2) | pending |
| 23-02-01 | 02 | 2 | MEM-04 | manual | UI navigation to memories screen | N/A | pending |
| 23-03-01 | 03 | 3 | AUI-01 | manual | Agent UI visible on all platforms | N/A | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

All Wave 0 tests are created by Plan 23-01, Task 2:

- [x] `test_update_memory` — covers MEM-06 update_memory SQL query
- [x] `test_agent_step_tool_input` — covers AUI-02 tool_input field in AgentStepSummary
- [x] `test_list_memories_action` — covers MEM-04 actor handler populates AppState.memories
- [x] `test_delete_memory_action` — covers MEM-05 actor handler removes from AppState + vector
- [x] `test_update_memory_action` — covers MEM-06 actor handler updates preview in AppState
- [x] `test_memories_screen_navigation` — covers Screen::Memories handled without panic

*Existing `test_insert_and_list_memories` and `test_delete_memory` patterns already exist in persistence test coverage.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Memory list screen renders | MEM-04 | UI rendering requires device/simulator | Navigate to Memories tab, verify list appears |
| Swipe-to-delete on mobile | MEM-05 | Gesture interaction | Swipe left on memory row, confirm deletion |
| Edit memory text and save | MEM-06 | Interactive text editing | Tap memory, edit text, tap save, verify updated |
| Agent nav restored | AUI-01 | UI navigation | Verify Agents button/tab visible on all platforms |
| Tool step detail display | AUI-02 | Visual rendering | Open agent session, verify tool name/input/output shown |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved
