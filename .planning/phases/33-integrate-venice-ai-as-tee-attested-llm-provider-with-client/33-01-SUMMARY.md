---
phase: 33
plan: 01
subsystem: rust-core/test-scaffold + planning-bookkeeping
tags:
  - venice
  - tee-attestation
  - wave-0
  - bookkeeping
  - red-stubs
dependency-graph:
  requires:
    - .planning/REQUIREMENTS.md (existing structure)
    - rust/Cargo.toml (existing [dependencies] section)
    - rust/src/tests/mod.rs (existing module list)
    - .claude/skills/spike-findings-confidential-app/sources/001-venice-tee-protocol-research/captures/attestation-sample.json
  provides:
    - VEN-01..VEN-09 requirement IDs
    - rust/tests/fixtures/venice/attestation-sample.json (golden capture, on-tree)
    - rust/src/tests/common/venice_fixtures.rs (shared fixture loader)
    - rust/src/tests/{venice,attestation_venice,live_venice}.rs (RED stubs)
    - 33-MRSEAM-RECONCILE.md (verdict: Present)
    - 33-VALIDATION.md (populated, nyquist_compliant: true, wave_0_complete: true)
  affects:
    - rust/Cargo.toml (k256 0.13.4, sha3 0.10.9, urlencoding 2.1 added)
    - .planning/ROADMAP.md (Phase 33 goal/requirements/4-plan list filled)
    - .planning/REQUIREMENTS.md (9 VEN-* checkboxes + 9 traceability rows)
tech-stack:
  added:
    - k256 0.13 (RustCrypto, pure Rust secp256k1 ECDH + ECDSA)
    - sha3 0.10 (Keccak256 for Ethereum-style address binding in REPORTDATA)
    - urlencoding 2.1 (attestation URL builder)
  patterns:
    - "RED stubs gated by `#[ignore = \"RED — Plan NN (VEN-XX)\"]` so default `cargo test` stays green"
    - "Shared fixture loader in `tests/common/venice_fixtures.rs` via `include_str!` — fixture paths never reach into `.claude/skills/`"
    - "Live integration test gated by `VENICE_API_KEY` env-var lookup (panics if missing) — never hard-coded"
key-files:
  created:
    - rust/tests/fixtures/venice/attestation-sample.json
    - rust/src/tests/common/mod.rs
    - rust/src/tests/common/venice_fixtures.rs
    - rust/src/tests/attestation_venice.rs
    - rust/src/tests/venice.rs
    - rust/src/tests/live_venice.rs
    - .planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-MRSEAM-RECONCILE.md
  modified:
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - rust/Cargo.toml
    - Cargo.lock
    - rust/src/tests/mod.rs
    - .planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-VALIDATION.md
decisions:
  - "MRSEAM verdict: Present (matches index 1 of TdxPolicy::default().accepted_mr_seams) — Plan 02 needs no policy change"
  - "RED stubs use `panic!(\"not yet implemented\")` inside `#[ignore]`-gated tests rather than `compile_error!` — keeps build green and lets later plans simply remove the `#[ignore]` and fill the body"
  - "Shared fixture loader extracts `intel_quote`, `nonce`, `signing_public_key`/`signing_key` (handles both spike capture spellings)"
  - "MRSEAM hex recorded as a comment in venice_fixtures.rs so capture rotation forces re-reconcile (sanity sentinel)"
  - "Live test gated solely on VENICE_API_KEY env (Plan 04 will add cassette-based recorded tests if needed)"
metrics:
  duration: ~10 minutes (executor) — bulk of time was cargo dep resolution + extraction round-trip
  completed-date: 2026-04-26
  tasks-completed: 3
  files-created: 7
  files-modified: 6
  commits: 3
---

# Phase 33 Plan 01: Wave 0 — Requirements, Cargo deps, golden capture, RED stubs, MRSEAM reconcile — Summary

**One-liner:** Locked VEN-01..VEN-09 into REQUIREMENTS/ROADMAP, added k256 + sha3 + urlencoding to Cargo.toml, committed the Venice golden capture as an on-tree fixture, scaffolded 14 RED-failing test stubs covering every Phase 33 requirement, reconciled MRSEAM (Present — no policy change needed), and populated VALIDATION.md with a 9-row per-task verification map across all four 33-* plans.

## What was built

### Bookkeeping (Task 1)
- `.planning/REQUIREMENTS.md`: new `### Venice.ai TEE-Attested Provider` section with 9 checkboxes (VEN-01..VEN-09) + 9 rows in the Traceability table + Coverage block updated to `45 total / 36 complete / 9 pending (Phase 33)`.
- `.planning/ROADMAP.md`: Phase 33 entry — Goal filled in, `Requirements: VEN-01..VEN-09`, `Plans: 4 plans`, four-plan breakdown listed.
- `rust/Cargo.toml`: three new deps under a Phase 33 banner:
  - `k256 = { version = "0.13", default-features = false, features = ["ecdh", "arithmetic", "std"] }` → resolves to **0.13.4**
  - `sha3 = { version = "0.10", default-features = false }` → resolves to **0.10.9** (cargo notes 0.11.0 is available but 0.10 was pinned per RESEARCH.md to avoid breaking-change cascade)
  - `urlencoding = "2.1"`
- `rust/tests/fixtures/venice/attestation-sample.json`: byte-identical copy of the spike-findings golden capture (`diff -q` passes).
- `cargo check -p mango_core` passes cleanly with three new deps + transitive `keccak v0.1.6`.

### Test scaffold (Task 2)
- `rust/src/tests/common/mod.rs` + `rust/src/tests/common/venice_fixtures.rs`: `GOLDEN_CAPTURE_JSON` constant via `include_str!`, plus helpers `golden_capture()`, `golden_intel_quote_bytes()`, `golden_nonce_32()`, `golden_signing_pubkey_hex()` (handles both `signing_key` and `signing_public_key` spellings observed in the capture).
- `rust/src/tests/attestation_venice.rs`: 6 `#[ignore]`-gated stubs (`reportdata_layout_ok`, `reportdata_address_mismatch`, `reportdata_nonce_mismatch`, `reportdata_padding_nonzero`, `tdx_debug_bit_rejected`, `tdx_verify_golden_capture_signature`) covering VEN-03/04/06.
- `rust/src/tests/venice.rs`: 7 `#[ignore]`-gated stubs (`venice_preset_present`, `attestation_url_format`, `nvidia_payload_double_parse`, `ecdh_aes_round_trip`, `envelope_round_trip`, `request_body_shape`, `backend_summary_after_add`) covering VEN-01/02/05/07/08/09.
- `rust/src/tests/live_venice.rs`: 1 `#[ignore]`-gated `#[tokio::test]` reading `VENICE_API_KEY` env (live integration smoke).
- `rust/src/tests/mod.rs`: registered `attestation_venice`, `pub mod common`, `live_venice`, `venice`.
- **Compile check:** `cargo test -p mango_core --lib --no-run` exits 0; default `cargo test` does not run RED stubs (all gated by `#[ignore]`).
- **VEN-* coverage:** every requirement VEN-01 through VEN-09 has at least one named test stub.

### MRSEAM reconcile + VALIDATION map (Task 3)
- Wrote a temporary `tests/mrseam_dump.rs` `#[ignore]`-gated test that called `dcap_qvl::quote::Quote::parse` on the golden capture, printed `Report::TD10` fields, then deleted the temp file (reverted both the file and its `mod` registration).
- Captured fields:
  - **MRSEAM (48 B):** `7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d`
  - `td_attributes` = `0000001000000000` (debug bit clear — production TDX, satisfies VEN-06 expectation)
  - `tee_tcb_svn` = `0b010300000000000000000000000000` (above default minimum `03010200…`)
  - Quote header `version` = `4` (TDX 1.0 / DCAP v4 → `Report::TD10`)
- **Verdict: ✅ Present** — Venice MRSEAM matches **index 1** of `TdxPolicy::default().accepted_mr_seams` in `rust/src/attestation/policy.rs`. **Plan 02 requires no policy change.**
- `33-MRSEAM-RECONCILE.md`: full reconciliation document with sibling fields and explicit Plan 02 actions.
- `venice_fixtures.rs`: MRSEAM hex recorded as a top-of-file sentinel comment so capture rotation triggers re-reconciliation.
- `33-VALIDATION.md`: replaced placeholder row with **9 task rows** spanning all 4 plans (33-01-T1..T3, 33-02-T1..T2, 33-03-T1..T2, 33-04-T1..T2). Frontmatter set: `nyquist_compliant: true`, `wave_0_complete: true`, status `wave-0-complete`. Sampling continuity verified — every task has an automated verify command except 33-04-T2 (live integration, manual-only).

## Commits

| # | Task | Type | Hash    | Files |
|---|------|------|---------|-------|
| 1 | Task 1 — bookkeeping + Cargo deps + golden capture | chore | `c2e05c3` | REQUIREMENTS.md, ROADMAP.md, Cargo.toml, Cargo.lock, fixture |
| 2 | Task 2 — RED test stubs + fixture loader | test | `cee7b2a` | 6 test files (5 new + tests/mod.rs) |
| 3 | Task 3 — MRSEAM reconcile + VALIDATION.md | docs | `22b3944` | 33-MRSEAM-RECONCILE.md, 33-VALIDATION.md, venice_fixtures.rs (sentinel) |

## Cargo invocation Plans 02-04 will use to track RED→GREEN

```bash
# Run all Venice unit tests (default = ignored stubs skipped, GREEN tests run)
cargo test -p mango_core --lib venice attestation_venice -- --nocapture

# Force-run a specific RED stub once it has been un-ignored:
cargo test -p mango_core --lib venice::ecdh_aes_round_trip -- --exact --nocapture

# Force-run all (including currently-ignored RED) to inventory remaining RED stubs:
cargo test -p mango_core --lib venice attestation_venice -- --include-ignored --nocapture

# Live integration (Plan 04 acceptance):
VENICE_API_KEY=… cargo test -p mango_core --lib live_venice -- --ignored --nocapture
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `mod mrseam_dump` registration required for one-shot extractor**
- **Found during:** Task 3
- **Issue:** Cargo would not run a `#[test]` inside `rust/src/tests/mrseam_dump.rs` without the module being declared in `rust/src/tests/mod.rs`. Initial run reported "0 tests".
- **Fix:** Added `#[cfg(test)] mod mrseam_dump;` registration, ran the dump, then **removed both the file and the module declaration** (no temp artifact left in the tree per acceptance criteria).
- **Files modified:** `rust/src/tests/mod.rs` (transient — reverted), `rust/src/tests/mrseam_dump.rs` (created and deleted)
- **Commit:** none (workflow-internal)

**2. [Rule 3 — Blocking] `dcap_qvl::quote::Report` enum field discovery**
- **Found during:** Task 3
- **Issue:** The plan suggested `quote.report.as_td10()` accessor, but `dcap_qvl 0.3.x` exposes `Report::TD10(TDReport10)` / `Report::TD15(TDReport15Combined)` as enum variants requiring a `match`.
- **Fix:** Used `match &quote.report { Report::TD10(r) => …, Report::TD15(r) => r.base.mr_seam, … }`. The capture is `TD10`.
- **Files modified:** `rust/src/tests/mrseam_dump.rs` (transient)
- **Commit:** none (workflow-internal)

**3. [Rule 3 — Blocking] `.planning/` is in `.gitignore`**
- **Found during:** Task 1 commit
- **Issue:** `git add .planning/REQUIREMENTS.md` failed with "ignored by .gitignore" — the project's `.planning/` is git-ignored by default.
- **Fix:** Used `git add -f` for `.planning/` paths. Pattern reused for Task 3.
- **Files modified:** none — workflow change only
- **Commit:** captured in c2e05c3 / 22b3944

### Authentication gates
None. Plan was fully autonomous.

## Threat Surface Scan

No new runtime threat surface introduced — Wave 0 only adds metadata, build-time Cargo deps, in-tree test fixtures, and ignore-gated test stubs. The three new Cargo deps (`k256`, `sha3`, `urlencoding`) **will** become runtime trust roots in Plans 02/03, but in Plan 01 they are compile-only — no code in `src/` calls them yet.

No `## Threat Flags` section needed.

## Known Stubs

By design — every test in `attestation_venice.rs`, `venice.rs`, `live_venice.rs` is a `panic!("not yet implemented")` body inside `#[ignore]`. These are **intentional Wave 0 RED markers**, each annotated with the future plan + VEN-* requirement that resolves it. Plans 02/03/04 must un-ignore and implement these (this is the contract — see "Cargo invocation" block above).

## Self-Check: PASSED

Files verified to exist:
- `rust/tests/fixtures/venice/attestation-sample.json` ✅ (98 KB, jq parses, intel_quote present)
- `rust/src/tests/common/venice_fixtures.rs` ✅ (sentinel comment present, all helpers present)
- `rust/src/tests/attestation_venice.rs` ✅ (6 `#[ignore]` markers)
- `rust/src/tests/venice.rs` ✅ (7 `#[ignore]` markers, all VEN-* covered)
- `rust/src/tests/live_venice.rs` ✅ (1 `#[ignore]` marker)
- `.planning/phases/33-…/33-MRSEAM-RECONCILE.md` ✅ (Present verdict)
- `.planning/phases/33-…/33-VALIDATION.md` ✅ (`nyquist_compliant: true`, 9 task rows)

Commits verified to exist:
- `c2e05c3` ✅ (chore: requirements + Cargo deps + fixture)
- `cee7b2a` ✅ (test: RED stubs)
- `22b3944` ✅ (docs: MRSEAM reconcile + VALIDATION.md)

`cargo check -p mango_core` ✅ (clean)
`cargo test -p mango_core --lib --no-run` ✅ (clean)
Temporary dump artifact removed ✅
