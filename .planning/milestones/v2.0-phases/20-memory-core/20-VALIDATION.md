---
phase: 20
slug: memory-core
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-03
---

# Phase 20 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | rust/Cargo.toml |
| **Quick run command** | `cd rust && cargo test --lib memory` |
| **Full suite command** | `cd rust && cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd rust && cargo test --lib memory`
- **After every plan wave:** Run `cd rust && cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 20-01-01 | 01 | 1 | MEM-01, MEM-02 | unit | `cd rust && cargo test --lib memory` | ❌ W0 | ⬜ pending |
| 20-01-02 | 01 | 1 | MEM-07 | unit | `cd rust && cargo test --lib memory` | ❌ W0 | ⬜ pending |
| 20-01-03 | 01 | 1 | MEM-02 | unit | `cd rust && cargo test --lib memory` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `rust/src/memory/mod.rs` — memory module with extract function
- [ ] `rust/src/tests/memory.rs` — test stubs for MEM-01, MEM-02, MEM-07

*Existing test infrastructure (cargo test) covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Background extraction does not block UI | MEM-07 | Requires runtime observation | Start conversation, complete it, verify chat remains responsive during extraction |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
