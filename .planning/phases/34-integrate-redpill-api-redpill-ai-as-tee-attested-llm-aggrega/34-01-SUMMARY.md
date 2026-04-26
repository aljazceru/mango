---
phase: 34
plan: 01
subsystem: attestation
tags:
  - redpill
  - tee-attestation
  - tdx
  - golden-fixtures
  - red-tests
  - wave-0
dependency_graph:
  requires:
    - Phase 33 (Venice TEE provider) — REPORTDATA model-ecdsa decoder reused verbatim
    - Spike 002 — captures + Python decoder are the canonical test design
  provides:
    - rust/tests/fixtures/redpill/ — 4 golden capture JSONs + nonce.txt
    - rust/src/tests/common/redpill_fixtures.rs — fixture loader
    - 26 #[ignore]-gated RED test stubs across 3 test files
    - 34-VALIDATION.md per-task verification map for all 4 plans
  affects:
    - Plan 34-02 (attestation/redpill.rs) — every RED stub becomes a GREEN test
    - Plan 34-03 (llm/redpill.rs) — RED-01, RED-02, RED-10 stubs flip GREEN
    - Plan 34-04 (wiring + UI) — RED-09, RED-11 stubs flip GREEN
tech-stack:
  added: []
  patterns:
    - "include_str! fixture loaders in tests/common/ (mirrors venice_fixtures.rs)"
    - "#[ignore = \"RED — Plan NN (RED-XX) ...\"] panicking stubs (default cargo test stays green)"
    - "Python-decoder assertion → Rust #[test] 1:1 mapping"
key-files:
  created:
    - rust/tests/fixtures/redpill/attestation-phala-pure-raw.json
    - rust/tests/fixtures/redpill/attestation-phala-raw.json
    - rust/tests/fixtures/redpill/attestation-chutes-raw.json
    - rust/tests/fixtures/redpill/attestation-tinfoil-raw.json
    - rust/tests/fixtures/redpill/nonce.txt
    - rust/src/tests/common/redpill_fixtures.rs
    - rust/src/tests/attestation_redpill.rs
    - rust/src/tests/redpill.rs
    - rust/src/tests/live_redpill.rs
    - .planning/phases/34-integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggrega/34-VALIDATION.md
  modified:
    - .planning/ROADMAP.md (Phase 34 Plans block: TBD → 4 plans listed)
    - rust/Cargo.toml (zero-new-deps marker comment)
    - rust/src/tests/mod.rs (3 new mod declarations)
    - rust/src/tests/common/mod.rs (1 new pub mod)
decisions:
  - "Zero new Cargo dependencies — every crate Redpill needs is already present from Phase 33"
  - "Python decoder script's assertions are the canonical test design; each becomes a Rust #[test] stub"
  - "RED stubs are #[ignore]-gated panicking tests, NOT #[should_panic] — default cargo test stays green"
  - "Tinfoil-routed refusal capture included as a fixture (Plan 03 RED-10 dispatches on it)"
  - "Live integration test stubs cover Shape A, B, and C (per CONTEXT D-24)"
metrics:
  duration: ~25 minutes
  completed: 2026-04-26
  tasks: 3
  commits: 3
---

# Phase 34 Plan 01: Redpill Wave 0 — Golden Fixtures + RED Test Stubs Summary

Wave 0 setup for Phase 34 (Redpill TEE-attested aggregator): copied four spike-002 capture JSONs + nonce log into `rust/tests/fixtures/redpill/` byte-identically, scaffolded a fixture loader plus three RED test files (attestation_redpill.rs, redpill.rs, live_redpill.rs) translating every assertion in `decode-report-data.py` 1:1 into `#[ignore]`-gated Rust stubs, confirmed REQUIREMENTS.md already lists RED-01..RED-11, updated ROADMAP.md's Plans block from TBD to 4 plans, verified zero new Cargo dependencies are needed, and populated 34-VALIDATION.md with a 10-row per-task verification map covering all four Phase 34 plans.

## What Shipped

### Golden fixtures (5 files)

Byte-identical copies of the four spike-002 captures plus the submitted-nonce log:

| File | Shape | Contains |
|------|-------|----------|
| `attestation-phala-pure-raw.json` | A — Flat (Phala-pure / Venice-identical) | `intel_quote` |
| `attestation-phala-raw.json` | B — Orchestrated (gateway + model + compose-manager) | `gateway_attestation`, `model_attestations[]` |
| `attestation-chutes-raw.json` | C — Chutes anti-tamper (base64 quotes, enclave-baked nonce) | `attestation_type: "chutes"` |
| `attestation-tinfoil-raw.json` | D (refusal) — Tinfoil-via-Redpill HTTP 502 body | `Unsupported Tinfoil` |
| `nonce.txt` | n/a | five client-submitted nonces (phala, chutes, tinfoil, etc.) |

All five verified byte-identical to spike sources via `diff -q`.

### Test scaffolding (4 source files)

| File | Lines | RED Stubs | Coverage |
|------|-------|-----------|----------|
| `rust/src/tests/common/redpill_fixtures.rs` | 60 | n/a (loader) | `include_str!` for 5 fixtures + nonce parsing + `quote_bytes_for_test` + `slice_reportdata` helpers |
| `rust/src/tests/attestation_redpill.rs` | 156 | **19** | RED-03 (4), RED-05a (4), RED-05b (1), RED-05c (1), RED-06 (1), RED-05d (2), RED-04 (3), RED-08 (2) |
| `rust/src/tests/redpill.rs` | 33 | **4** | RED-01, RED-02, RED-10, RED-11 |
| `rust/src/tests/live_redpill.rs` | 27 | **3** | live Shape A, B, C (RED-09) |
| **Total** | | **26 RED stubs** | every Python decoder assertion has a Rust #[test] |

### Wave 0 Documentation

- `34-VALIDATION.md` — 10-row per-task verification map covering all four Phase 34 plans, with explicit RED-01..RED-11 → task and T-34-01..T-34-06 → mitigation cross-references. Frontmatter: `nyquist_compliant: true`, `wave_0_complete: true`.

## Zero-New-Deps Verification

`grep` confirmed all 23 needed crates are present in `rust/Cargo.toml` from earlier phases:
`dcap-qvl 0.3, sha2 0.10, sha3 0.10, base64 0.22, hex 0.4, reqwest 0.12 rustls, async-openai 0.34, serde 1, serde_json 1, hkdf 0.12, aes-gcm 0.10, k256 0.13, urlencoding 2.1, zeroize 1, once_cell 1, rand 0.8, tokio, tokio-util, futures, log, chrono, uuid, jsonwebtoken`.

A marker comment was added to `Cargo.toml` to document the zero-add decision for Phase 34 (no actual dependency lines added). `git diff rust/Cargo.toml` shows comment-only changes.

## Validation Commands

Plans 02–04 will track RED→GREEN flip with this exact invocation:

```bash
# Default test run (RED stubs all ignored — must stay green throughout the phase)
cd rust && cargo test -p mango_core --lib tests::redpill tests::attestation_redpill

# RED→GREEN tracking (re-run after each Plan-NN task; counts how many remain RED)
cd rust && cargo test -p mango_core --lib tests::redpill tests::attestation_redpill -- --include-ignored 2>&1 | grep -c 'not yet implemented'
# At Wave 0 close: 23 (live test stubs excluded; they remain #[ignore] for live runs)

# Live integration (Plan 04, manual gate)
cd rust && cargo test -p mango_core --lib tests::live_redpill -- --ignored --nocapture
```

## Acceptance Criteria — Verified

| Criterion | Result |
|-----------|--------|
| `grep -c '\*\*RED-' .planning/REQUIREMENTS.md` ≥ 11 | ✅ 11 |
| ROADMAP.md Phase 34 entry lists 4 plans + RED-01..RED-11 | ✅ |
| 4 golden fixtures + nonce.txt byte-identical to spike | ✅ `diff -q` clean on all 5 |
| `jq -e '.intel_quote'` Shape A | ✅ |
| `jq -e '.gateway_attestation'` Shape B | ✅ |
| `jq -r '.attestation_type'` Shape C → `chutes` | ✅ |
| `cargo check -p mango_core` exits 0 | ✅ (3 unrelated pre-existing dead-code warnings) |
| `cargo test -p mango_core --lib --no-run` exits 0 | ✅ |
| `grep -c '#\[ignore' attestation_redpill.rs` ≥ 16 | ✅ 19 |
| `grep -c '#\[ignore' redpill.rs` ≥ 4 | ✅ 4 |
| `grep -c '#\[ignore' live_redpill.rs` ≥ 2 | ✅ 3 |
| `grep -c 'RED-0[4-8]' attestation_redpill.rs` ≥ 10 | ✅ 20 |
| Default `cargo test` for redpill modules: 0 failures | ✅ 0 passed, 0 failed, 23 ignored |
| Zero new Cargo dependency lines | ✅ comment-only diff |
| `34-VALIDATION.md` ≥ 9 task rows + every RED-01..RED-11 present | ✅ 10 rows + all 11 covered |

## Deviations from Plan

**One small adaptation:**

**1. [Rule 3 - Blocking] Force-add for `.planning/` and `rust/tests/fixtures/redpill/`**
- **Found during:** Task 1 commit
- **Issue:** Repo `.gitignore` excludes `.planning/` and `rust/tests/fixtures/redpill/` (the latter via the inherited Cargo `target` filter pattern). `git add` rejected the paths.
- **Fix:** Used `git add -f` to force-add the planned tracked files. This matches how prior Phase 33 fixtures (`rust/tests/fixtures/venice/attestation-sample.json`) and ROADMAP.md updates have always been committed in this repo (verified via `git log -- .planning/ROADMAP.md`).
- **Files modified:** none (process-only)
- **Commit:** 1181aa5

Otherwise: plan executed exactly as written. The Python decoder script's `assert` count maps cleanly to the requirement-byte-count (RED-04 = 3 stubs for hex/b64/0x-prefix; RED-05a = 4 stubs incl. negative paths; RED-08 = 2 stubs incl. synthetic-quote rejection).

## Authentication Gates

None — this is a pure test-scaffolding plan with no network access.

## Threat Mitigation Recap

| Threat | Mitigated By |
|--------|--------------|
| T-34-01 (REPORTDATA layout spoofing) | 8 RED stubs across all four shape decoders pin the byte-slice asserts; Plan 02 cannot ship without flipping each one GREEN |
| T-34-02 (three-way AND bypass) | `shape_b_three_way_and_gates_session_open` stub forces Plan 02 to enforce the AND on Orchestrated |
| T-34-03 (Chutes freshness misrepresentation) | `shape_c_client_nonce_not_bound` stub explicitly documents that rd[32..64] is unconstrained on Chutes; Plan 04 UI is goal-backward to this |
| T-34-04 (TDX debug-mode bit) | `debug_bit_clear_in_all_captures` (positive) + `debug_bit_set_rejected` (negative) stubs gate Plan 02's verify orchestrator |

## Commits

| Task | Commit | Subject |
|------|--------|---------|
| 1 | 1181aa5 | chore(34-01): copy Redpill golden fixtures and update ROADMAP plans block |
| 2 | 038af19 | test(34-01): scaffold Redpill RED test stubs translating decode-report-data.py |
| 3 | 53f0547 | docs(34-01): add 34-VALIDATION.md per-task verification map |

## Self-Check

- [x] All four golden fixtures + nonce.txt at `rust/tests/fixtures/redpill/` (5 files, byte-identical)
- [x] `rust/src/tests/common/redpill_fixtures.rs` (loader, 60 lines)
- [x] `rust/src/tests/attestation_redpill.rs` (19 RED stubs, 156 lines)
- [x] `rust/src/tests/redpill.rs` (4 RED stubs, 33 lines)
- [x] `rust/src/tests/live_redpill.rs` (3 live stubs, 27 lines)
- [x] `34-VALIDATION.md` (10 task rows, all RED-* covered)
- [x] All three commits exist in git log

## Self-Check: PASSED
