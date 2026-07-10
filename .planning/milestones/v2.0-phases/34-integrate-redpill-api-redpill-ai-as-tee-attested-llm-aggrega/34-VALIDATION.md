---
phase: 34
nyquist_compliant: true
wave_0_complete: true
---

# Phase 34: Redpill TEE-Attested Aggregator — Validation Map

This document maps every task across all four Phase 34 plans to:
- the requirement(s) it satisfies (RED-01..RED-11)
- the threat(s) it mitigates (T-34-01..T-34-06)
- the secure behavior it locks in
- the test type and the exact automated command that verifies it

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|--------|
| 34-01-T1 | 34-01 | 0 | RED-01..11 | T-34-01 | requirements + fixtures + zero new deps | meta | `for f in attestation-phala-pure-raw.json attestation-phala-raw.json attestation-chutes-raw.json attestation-tinfoil-raw.json nonce.txt; do diff -q rust/tests/fixtures/redpill/$f .planning/spikes/002-redpill-tee-verification-research/captures/$f; done` | ✅ |
| 34-01-T2 | 34-01 | 0 | RED-04, RED-05a..d, RED-06, RED-08 | T-34-01..04 | RED test stubs compile and skip cleanly | unit | `cd rust && cargo test -p mango_core --lib --no-run` | ✅ |
| 34-01-T3 | 34-01 | 0 | RED-01..11 | — | per-task map populated; every RED-* traceable | meta | `grep -c '34-0[1234]-T' .planning/phases/34-integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggrega/34-VALIDATION.md` | ✅ |
| 34-02-T1 | 34-02 | 1 | RED-03, RED-04 | T-34-01 | shape dispatcher + quote_bytes() helper | unit | `cargo test -p mango_core --lib tests::attestation_redpill::dispatch tests::attestation_redpill::quote_bytes` | ⬜ |
| 34-02-T2 | 34-02 | 1 | RED-05a, RED-05b, RED-05c, RED-05d | T-34-01..03 | four REPORTDATA decoders (model reuses Venice import) | unit | `cargo test -p mango_core --lib tests::attestation_redpill::shape_` | ⬜ |
| 34-02-T3 | 34-02 | 1 | RED-06, RED-07, RED-08 | T-34-02, T-34-04 | three-way AND + NRAS double-parse + debug-mode gate + verify orchestrator + cache | unit | `cargo test -p mango_core --lib tests::attestation_redpill` | ⬜ |
| 34-03-T1 | 34-03 | 2 | RED-01, RED-02 | T-34-05 | llm/redpill.rs HTTP fetch + chat completions + preset wiring | unit | `cargo test -p mango_core --lib tests::redpill::redpill_preset tests::redpill::attestation_url` | ⬜ |
| 34-03-T2 | 34-03 | 2 | RED-10 | T-34-05 | Tinfoil-route refusal with typed error | unit | `cargo test -p mango_core --lib tests::redpill::tinfoil_route_refused` | ⬜ |
| 34-04-T1 | 34-04 | 3 | RED-09, RED-11 | T-34-06 | UniFFI bindings + Settings preset surfacing + verified badge with shape breakdown | unit | `cargo test -p mango_core --lib tests::redpill::backend_summary` | ⬜ |
| 34-04-T2 | 34-04 | 3 | RED-09 (live) | — | end-to-end live attestation against api.redpill.ai (Shape A + B [+ C]) | integration | `cargo test -p mango_core --lib tests::live_redpill -- --ignored` | ⬜ (manual) |

## Requirement → Task Coverage (RED-01..RED-11)

| Req | Covered By |
|-----|------------|
| RED-01 (provider preset) | 34-01-T1, 34-03-T1 |
| RED-02 (attestation URL) | 34-01-T1, 34-03-T1 |
| RED-03 (shape dispatcher) | 34-01-T2, 34-02-T1 |
| RED-04 (quote_bytes hex/b64) | 34-01-T2, 34-02-T1 |
| RED-05a (Shape A model layout) | 34-01-T2, 34-02-T2 |
| RED-05b (Shape B gateway ed25519) | 34-01-T2, 34-02-T2 |
| RED-05c (Shape B compose-manager) | 34-01-T2, 34-02-T2 |
| RED-05d (Shape C anti-tamper) | 34-01-T2, 34-02-T2 |
| RED-06 (three-way AND) | 34-01-T2, 34-02-T3 |
| RED-07 (NVIDIA NRAS reuse) | 34-02-T3 |
| RED-08 (debug-mode gate) | 34-01-T2, 34-02-T3 |
| RED-09 (per-enclave freshness UI) | 34-04-T1, 34-04-T2 |
| RED-10 (Tinfoil refusal) | 34-01-T2, 34-03-T2 |
| RED-11 (Verified badge w/ shape breakdown) | 34-04-T1 |

## Threat → Mitigation Coverage

| Threat | Mitigation Task |
|--------|-----------------|
| T-34-01 Spoofing — TDX REPORTDATA layout | 34-02-T2 (decoders), 34-01-T2 (RED stubs) |
| T-34-02 Tampering — Three-way AND | 34-02-T3 (verify orchestrator) |
| T-34-03 Info Disclosure — Chutes per-enclave freshness | 34-04-T1 (UI sub-line), 34-01-T2 (`shape_c_client_nonce_not_bound` stub) |
| T-34-04 Info Disclosure — TDX debug-mode bit | 34-02-T3 (debug-mode gate), 34-01-T2 (`debug_bit_*` stubs) |
| T-34-05 Spoofing — fixtures leaking into prod | All RED stubs `#[ignore]`-gated; no prod code reads fixture path |
| T-34-06 Trust UI misrepresentation | 34-04-T1 (UniFFI badge surfacing), 34-04-T2 (live integration) |
