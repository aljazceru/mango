---
phase: 21
slug: memory-retrieval-injection
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-04
---

# Phase 21 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) |
| **Config file** | rust/Cargo.toml |
| **Quick run command** | `cargo test -p mango_core --lib memory` |
| **Full suite command** | `cargo test -p mango_core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mango_core --lib memory`
- **After every plan wave:** Run `cargo test -p mango_core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 21-01-01 | 01 | 1 | MEM-03 | unit | `cargo test -p mango_core --lib memory::retrieve` | ❌ W0 | ⬜ pending |
| 21-01-02 | 01 | 1 | MEM-03 | integration | `cargo test -p mango_core --lib test_memory_injection` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `rust/src/memory/retrieve.rs` — memory retrieval module with tests
- [ ] Test fixtures for memory search with pre-seeded memories

*Existing test infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Memory context visible in conversation | MEM-03 | Requires running app with real LLM | Send message in conversation with prior memories, verify system prompt includes memory context |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
