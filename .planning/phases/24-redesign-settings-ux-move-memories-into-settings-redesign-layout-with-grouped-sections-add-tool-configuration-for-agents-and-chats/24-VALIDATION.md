---
phase: 24
slug: redesign-settings-ux
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-05
---

# Phase 24 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (cargo test) |
| **Config file** | none — inline `#[test]` modules |
| **Quick run command** | `cargo test -p mango_core settings 2>&1 | tail -10` |
| **Full suite command** | `cargo test -p mango_core 2>&1 | tail -10` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p mango_core && cargo check -p mango-desktop`
- **After every plan wave:** Run `cargo test -p mango_core 2>&1 | tail -5`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 24-00-01 | 00 | 0 | SET-04, SET-06 | unit | `cargo test -p mango_core test_brave_api_key_persists test_memory_count` | rust/src/tests/settings.rs | pending |
| 24-01-01 | 01 | 1 | SET-04 | unit + build | `cargo check -p mango-core && cargo test -p mango_core test_brave_api_key_persists test_memory_count` | rust/src/tests/settings.rs | pending |
| 24-02-01 | 02 | 2 | SET-01 | manual-only | -- | N/A | pending |
| 24-02-02 | 02 | 2 | SET-02 | manual-only | -- | N/A | pending |
| 24-02-03 | 02 | 2 | SET-03 | manual-only | -- | N/A | pending |
| 24-02-04 | 02 | 2 | SET-05 | manual-only | -- | N/A | pending |
| 24-02-05 | 02 | 2 | SET-07 | build | `cargo check -p mango-desktop` | exists | pending |

---

## Wave 0 Requirements

- [x] `rust/src/tests/settings.rs` -- Plan 24-00 adds `test_brave_api_key_persists` (SET-04) and `test_memory_count` (SET-06) test functions

*Existing infrastructure covers build verification (SET-07). Wave 0 adds unit test coverage for Rust core additions.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Memory section row navigates to Memories screen | SET-01 | UI navigation test, no headless runner | Open Settings > tap Memory row > verify Memories screen appears |
| Home toolbar no longer shows Memories button | SET-02 | Visual absence check | Open home screen > verify no Memories button in toolbar |
| Tools section renders Brave API key field | SET-03 | UI render check | Open Settings > scroll to Tools section > verify input field present |
| Sections ordered and labeled correctly | SET-05 | Visual layout check | Open Settings > verify section headers: Providers, Defaults, Memory, Tools, Appearance, Advanced |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
