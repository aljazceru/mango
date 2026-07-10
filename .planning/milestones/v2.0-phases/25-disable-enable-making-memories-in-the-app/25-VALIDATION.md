---
phase: 25
slug: disable-enable-making-memories-in-the-app
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-05
---

# Phase 25 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Config file** | rust/Cargo.toml |
| **Quick run command** | `cargo test -p mango-core memories_enabled 2>&1 \| tail -5` |
| **Full suite command** | `cargo test -p mango-core 2>&1 \| tail -10` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mango-core memories_enabled 2>&1 | tail -5`
- **After every plan wave:** Run `cargo test -p mango-core 2>&1 | tail -10`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 25-01-01 | 01 | 0 | memories_enabled field | unit stub | `cargo test -p mango-core memories_enabled 2>&1 \| tail -5` | ❌ W0 | ⬜ pending |
| 25-01-02 | 01 | 1 | AppState field | unit | `cargo test -p mango-core memories_enabled 2>&1 \| tail -5` | ✅ | ⬜ pending |
| 25-01-03 | 01 | 1 | SetMemoriesEnabled action | unit | `cargo test -p mango-core memories_enabled 2>&1 \| tail -5` | ✅ | ⬜ pending |
| 25-01-04 | 01 | 1 | StreamDone guard | unit | `cargo test -p mango-core memories_enabled 2>&1 \| tail -5` | ✅ | ⬜ pending |
| 25-02-01 | 02 | 2 | iOS toggle UI | manual | — | — | ⬜ pending |
| 25-02-02 | 02 | 2 | Android switch UI | manual | — | — | ⬜ pending |
| 25-02-03 | 02 | 2 | Desktop toggler UI | manual | — | — | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `rust/src/tests/memories_enabled_tests.rs` — stubs for memories_enabled toggle behavior

*Existing infrastructure covers all other phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| iOS Toggle visible in Settings MEMORY section | UI | No UI test harness | Open Settings → scroll to MEMORY section → confirm Toggle present and functional |
| Android Switch visible in Settings MEMORY section | UI | No UI test harness | Open Settings → scroll to MEMORY section → confirm Switch present |
| Desktop toggler visible in Settings MEMORY section | UI | No UI test harness | Open Settings → confirm toggler renders in MEMORY section |
| Memories NOT extracted after toggle off + conversation | Behavior | Requires full app run | Toggle off → complete a conversation → check DB for new memories |
| Memories resume after re-enabling | Behavior | Requires full app run | Toggle on → complete a conversation → verify memory extraction occurs |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
