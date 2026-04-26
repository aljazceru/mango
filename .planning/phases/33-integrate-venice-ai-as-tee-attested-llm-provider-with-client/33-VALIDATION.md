---
phase: 33
slug: integrate-venice-ai-as-tee-attested-llm-provider-with-client
status: wave-0-complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-25
updated: 2026-04-26
---

# Phase 33 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Populated from RESEARCH.md `## Validation Architecture` and the four 33-* plans.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust core) |
| **Config file** | `rust/Cargo.toml` |
| **Quick run command** | `cargo test -p mango_core --lib venice attestation_venice -- --nocapture` |
| **Full suite command** | `cargo test -p mango_core --lib --no-fail-fast` |
| **Estimated runtime** | ≤ 30s for `cargo test -p mango_core --lib venice attestation_venice` |
| **Live integration tests** | `#[ignore]`-gated; run with `VENICE_API_KEY=… cargo test -p mango_core --lib live_venice -- --ignored --nocapture` |

---

## Sampling Rate

- **After every task commit:** Run quick command
- **After every plan wave:** Run full suite
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30s

---

## Per-Task Verification Map

> Every Phase 33 task has an `<automated>` verify command, a Wave 0 dependency, or is flagged manual-only with justification (live API key).

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 33-01-T1 | 33-01 | 1 | VEN-01..09 | — | Requirements + deps + golden fixture in tree | meta | `grep -c '\\*\\*VEN-' .planning/REQUIREMENTS.md` (=9) AND `grep -E '^k256\|^sha3\|^urlencoding' rust/Cargo.toml` (=3) AND `test -f rust/tests/fixtures/venice/attestation-sample.json` | ✅ | ✅ |
| 33-01-T2 | 33-01 | 1 | VEN-01..09 | T-33-01,02,04,07 | RED test stubs compile | unit | `cargo test -p mango_core --lib --no-run` | ✅ | ✅ |
| 33-01-T3 | 33-01 | 1 | VEN-03 | T-33-04 | MRSEAM reconciled vs TdxPolicy::default | meta | `test -f .planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-MRSEAM-RECONCILE.md && grep -q 'Present' .planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-MRSEAM-RECONCILE.md` | ✅ | ✅ |
| 33-02-T1 | 33-02 | 2 | VEN-03 | T-33-01, T-33-04 | TDX `ReportDataLayout` enum (D1) — parameterise `verify_tdx_quote` | unit | `cargo test -p mango_core --lib attestation_tdx::layout` | — | ⬜ |
| 33-02-T2 | 33-02 | 2 | VEN-04, VEN-05, VEN-06 | T-33-01, T-33-02, T-33-04 | REPORTDATA decoder (`attestation/venice.rs`) + per-session cache + `verify_venice_attestation` orchestrator + debug-bit reject | unit | `cargo test -p mango_core --lib attestation_venice` | — | ⬜ |
| 33-03-T1 | 33-03 | 3 | VEN-07a, VEN-07b | T-33-02, T-33-05 | secp256k1 ECDH + HKDF-SHA256("ecdsa_encryption") + AES-256-GCM envelope round-trip | unit | `cargo test -p mango_core --lib venice::ecdh_aes_round_trip venice::envelope_round_trip` | — | ⬜ |
| 33-03-T2 | 33-03 | 3 | VEN-08 | T-33-05 | Request-body builder (`enable_e2ee: true`, encrypted user/system content), three `X-Venice-TEE-*` headers, text-SSE decoder | unit | `cargo test -p mango_core --lib venice::request_body_shape venice::attestation_url_format` | — | ⬜ |
| 33-04-T1 | 33-04 | 4 | VEN-01, VEN-09 | T-33-06 | `transport.rs` + `backend.rs` Venice preset wiring + Verified badge | unit | `cargo test -p mango_core --lib venice::venice_preset_present venice::backend_summary_after_add` | — | ⬜ |
| 33-04-T2 | 33-04 | 4 | VEN-LIVE (cross-cut VEN-02,03,05,07,08) | T-33-04 | Live integration smoke test against api.venice.ai | integration | `VENICE_API_KEY=… cargo test -p mango_core --lib live_venice -- --ignored --nocapture` | ✅ (stub) | ⬜ (manual) |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Sampling continuity check:** Every task has an automated command. The longest gap between automated verifies is one commit (33-04-T2 is the only manual-only task and it is the final task).

---

## Wave 0 Requirements

- [x] Add Cargo deps: `k256` 0.13, `sha3` 0.10, `urlencoding` 2.1 (committed in 33-01-T1)
- [x] Commit golden capture fixture `rust/tests/fixtures/venice/attestation-sample.json` (33-01-T1)
- [x] Stub test files: `rust/src/tests/{venice,attestation_venice,live_venice}.rs` with RED stubs pinning every VEN-* (33-01-T2)
- [x] Shared fixture loader: `rust/src/tests/common/venice_fixtures.rs` (33-01-T2)
- [x] MRSEAM reconciliation document with Present/Absent/Configurable verdict (33-01-T3)
- [x] VALIDATION.md per-task map populated for all 4 plans (this file)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live Venice.ai chat completion round-trip with attested E2EE channel | VEN-LIVE (covers VEN-02, 03, 05, 07, 08 end-to-end) | Requires real `VENICE_API_KEY` and a live TEE endpoint; cannot run in CI without an account | `VENICE_API_KEY=… cargo test -p mango_core --lib live_venice -- --ignored --nocapture` |
| Cold-launch re-attestation UX (banner, retry on transient failure, fail-closed on cryptographic failure) | VEN-09 | Requires manual app launch and observation on iOS/Android UI | Cold-launch app, navigate to Settings → Providers, observe Verified badge after attestation completes; force-disconnect network and observe transient retry; tamper-fixture path is unit-tested |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (golden capture, fixture loader, RED stubs, MRSEAM reconcile)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** Wave 0 complete (33-01 finished 2026-04-26). Plans 02-04 unblocked.
