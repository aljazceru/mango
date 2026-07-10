---
phase: 29
slug: wire-vectorindex-dek
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 29 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`#[test]`) |
| **Config file** | `rust/Cargo.toml` |
| **Quick run command** | `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` |
| **Full suite command** | `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1`
- **After every plan wave:** Run full suite
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 29-01-01 | 01 | 1 | ENC-02 | T-29-01 | DEK stored in ActorState on unlock, cleared on lock | unit | `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` | ✅ | ⬜ pending |
| 29-01-02 | 01 | 1 | ENC-02 | T-29-02 | VectorIndex save passes DEK, not None | unit | `cargo test --manifest-path rust/Cargo.toml -- --test-threads=1` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. The Rust test framework is already configured with comprehensive test coverage from prior phases.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Encrypted .usearch file on disk after save | ENC-02 | Requires running app with auth enabled | Enable auth, add document, check embeddings.usearch has MGO1 header |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
