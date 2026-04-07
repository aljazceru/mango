---
phase: 27
slug: add-optional-tool-use-to-chat
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-07
---

# Phase 27 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `#[tokio::test]` |
| **Config file** | none (workspace-level cargo test) |
| **Quick run command** | `cargo test -p mango_core chat_tools 2>&1 \| tail -20` |
| **Full suite command** | `cargo test -p mango_core 2>&1 \| tail -40` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mango_core chat_tools 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test -p mango_core 2>&1 | tail -40`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 27-00-01 | 00 | 0 | CHAT-TOOL-01..03 | unit stubs | `grep -c "fn test_" rust/src/tests/chat_tools.rs` | Wave 0 creates | ⬜ pending |
| 27-01-01 | 01 | 1 | CHAT-TOOL-01 | unit | `cargo test -p mango_core test_tools_enabled_persistence` | ✅ (Wave 0) | ⬜ pending |
| 27-01-02 | 01 | 1 | CHAT-TOOL-02 | unit | `cargo test -p mango_core test_update_conversation_tools_enabled` | ✅ (Wave 0) | ⬜ pending |
| 27-01-03 | 01 | 1 | CHAT-TOOL-03 | unit | `cargo test -p mango_core test_build_chat_tools` | ✅ (Wave 0) | ⬜ pending |
| 27-01-04 | 01 | 1 | CHAT-TOOL-03 | unit | `cargo test -p mango_core test_chat_tools_no_brave` | ✅ (Wave 0) | ⬜ pending |
| 27-01-05 | 01 | 1 | CHAT-TOOL-01 | unit | `cargo test -p mango_core test_migration_v16` | ✅ (Wave 0) | ⬜ pending |
| 27-02-01 | 02 | 2 | CHAT-TOOL-04..06 | build | `cargo build -p mango_core` | N/A (compile) | ⬜ pending |
| 27-03-01 | 03 | 3 | CHAT-TOOL-07..08 | grep+build | grep checks + `cargo build -p mango_desktop` | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `rust/src/tests/chat_tools.rs` — 7 stub tests for migration, persistence, and tool subset builder (Plan 27-00)
- [x] `rust/src/tests/mod.rs` — add `mod chat_tools;` (Plan 27-00)

*Wave 0 plan (27-00-PLAN.md) creates these test stubs before Plan 01 implements production code.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| UI tools toggle renders on iOS/Android/Desktop | CHAT-TOOL-07 | Platform-specific UI rendering | Toggle tools on/off in conversation settings, verify toggle state persists across app relaunch |
| Tool call results display in chat bubble | CHAT-TOOL-08 | Visual rendering verification | Enable tools, send query triggering tool use, verify result displays inline |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved
