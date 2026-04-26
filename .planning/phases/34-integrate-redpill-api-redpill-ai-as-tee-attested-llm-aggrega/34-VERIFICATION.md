# Phase 34 — Verification Log

**Phase:** 34 — Integrate Redpill (api.redpill.ai) as TEE-attested LLM aggregator
**Status:** Complete (pending live verification by user)
**Date:** 2026-04-26

## Automated Verification

### Full Suite

```
$ cd rust && cargo test -p mango_core --lib

test result: ok. 382 passed; 0 failed; 18 ignored; 0 measured; 0 filtered out; finished in 15.09s
```

Of the 18 ignored tests, **4** are this phase's `#[ignore]`-gated live integration
tests (`tests::live_redpill::*`); the other 14 are pre-existing live/network-gated
tests from Phases 31-33 (Tinfoil/PPQ/Venice live tests).

### Release Build

```
$ cd rust && cargo build -p mango_core --release
   Compiling mango_core v0.2.2 (/home/lio/g/confidential-app/rust)
    Finished `release` profile [optimized] target(s) in 16.51s
```

### RED→GREEN Tally — RED-01..RED-11

| Requirement | Test(s) | Plan | Status |
|-------------|---------|------|--------|
| RED-01 (provider preset) | `tests::redpill::redpill_preset_present` | 34-03 | GREEN |
| RED-02 (attestation URL) | `tests::redpill::attestation_url_format` + `tests::llm::redpill::format_attestation_url_urlencodes_model_id` | 34-03 | GREEN |
| RED-03 (shape dispatcher) | `tests::attestation_redpill::dispatch_shape_a_flat`, `dispatch_shape_b_orchestrated`, `dispatch_shape_c_chutes`, `dispatch_unknown_shape_fails_closed` | 34-02 | GREEN |
| RED-04 (quote_bytes hex/b64) | `tests::attestation_redpill::quote_bytes_hex`, `quote_bytes_base64`, `quote_bytes_strips_0x_prefix` | 34-02 | GREEN |
| RED-05a (Shape A model layout) | `tests::attestation_redpill::shape_a_*` (4 tests) + `shape_b_model_*` | 34-02 | GREEN |
| RED-05b (Shape B gateway ed25519) | `tests::attestation_redpill::shape_b_gateway_reportdata_ok` | 34-02 | GREEN |
| RED-05c (Shape B compose-manager) | `tests::attestation_redpill::shape_b_compose_manager_reportdata_ok` | 34-02 | GREEN |
| RED-05d (Shape C anti-tamper) | `tests::attestation_redpill::shape_c_chutes_anti_tamper_binding_ok` + `shape_c_client_nonce_not_bound` | 34-02 | GREEN |
| RED-06 (three-way AND) | `tests::attestation_redpill::shape_b_three_way_and_gates_session_open` | 34-02 | GREEN |
| RED-07 (NVIDIA NRAS reuse) | `tests::attestation_redpill::nvidia_payload_double_parse_shape_a` + `nvidia_per_gpu_loop_shape_c` (live exercise via 34-04 live tests) | 34-02 | GREEN |
| RED-08 (debug-mode gate) | `tests::attestation_redpill::debug_bit_clear_in_all_captures` + `debug_bit_set_rejected` | 34-02 | GREEN |
| RED-09 (per-enclave freshness UI) | `tests::redpill::redpill_chutes_shape_carries_per_enclave_freshness` + `tests::live_redpill::live_shape_c_chutes_per_enclave_freshness` | 34-04 | GREEN (live: pending sign-off) |
| RED-10 (Tinfoil refusal) | `tests::redpill::tinfoil_route_refused_with_typed_error` + `tests::llm::redpill::tinfoil_user_facing_error_mentions_direct_tinfoil` + `tests::live_redpill::live_tinfoil_route_refused` | 34-03/34-04 | GREEN (live: pending sign-off) |
| RED-11 (Verified badge w/ shape breakdown) | `tests::redpill::backend_summary_after_add` + `tests::redpill::redpill_orchestrated_event_carries_three_components` + `tests::transport::redpill_routes_to_redpill_transport` | 34-04 | GREEN |

### Plan 34-04 Specific Verifications

| Criterion | Result |
|---|---|
| `ProviderTransportKind::Redpill` variant added | PASS — `rust/src/llm/transport.rs` |
| transport.rs match arms (model_list_url, openai_api_base, build_reqwest_client) extended | PASS — three Redpill arms; openai_api_base returns Ok({base}/v1) since Redpill is OpenAI-compatible |
| streaming.rs dispatches Redpill for `spawn_streaming_task` and `spawn_streaming_task_from_api_messages` | PASS — both entry points call `crate::llm::redpill::run_streaming_chat_completion[_from_api_messages]` before generic OpenAI path |
| `agent/loop.rs::run_agent_step_for_backend` includes Redpill arm (Rule 3 fix — match exhaustiveness) | PASS — `crate::llm::redpill::create_chat_completion` |
| `AttestationEvent::Verified` carries shape + freshness + orchestrated_components | PASS — three additive `Option` fields |
| Existing callers (Tinfoil/PPQ/Venice/TDX/NVIDIA/SEV-SNP) pass `None` for new fields | PASS — `endpoint.rs`, `nvidia.rs`, `tdx.rs`, `ppq_private.rs`, `tinfoil_secure.rs`, `venice.rs` |
| Redpill `verify_backend_attestation` populates all three fields from `VerifiedRedpillAttestation` | PASS — `rust/src/llm/redpill.rs::verify_backend_attestation` |
| `lib.rs` actor-loop destructure pattern updated to ignore new fields | PASS — `shape: _, freshness: _, orchestrated_components: _` |
| `tests/live_redpill.rs` covers Shape A, Shape B (three-way AND), Shape C (per-enclave), Tinfoil refusal | PASS — 4 `#[tokio::test] #[ignore]` functions |
| RED-11 `backend_summary_after_add` flipped from `#[ignore]` panic-stub to GREEN | PASS — `tests/redpill.rs` |
| `tests/transport.rs::redpill_routes_to_redpill_transport` regression test added | PASS |
| Full `cargo test -p mango_core --lib`: 0 failures | PASS — 382 passed, 0 failed |
| Existing Tinfoil/PPQ/Venice transport tests still pass — no regressions | PASS — `test_*_transport*` all green |
| `cargo build -p mango_core --release` exits 0 | PASS |
| UniFFI bindings regen (build.rs) — `AttestationEvent` is internal (not UniFFI-exported), additive Option fields are FFI-safe | PASS — release build through build.rs uniffi step succeeds |

## Manual Live Verification (User Action Required)

The four `#[ignore]`-gated live tests in `rust/src/tests/live_redpill.rs` exercise
the four end-to-end attestation paths against `api.redpill.ai`. The attestation
endpoint is **public — NO API key required** for these tests (chat completion
tests would require `REDPILL_API_KEY`, but these only exercise attestation).

1. (Optional, only needed if you later add chat-completion live tests)
   Set `REDPILL_API_KEY=<key>` from <https://redpill.ai>.
2. Run:
   ```
   cd rust && cargo test -p mango_core --lib live_redpill -- --ignored --nocapture
   ```
3. Expected:
   - `live_shape_a_phala_pure` passes (≤ 10s); stderr `[live] shape=Flat freshness=PerRequest`
   - `live_shape_b_orchestrated_three_way_and` passes; stderr shows three component
     hex addresses (gateway/model/compose_manager)
   - `live_shape_c_chutes_per_enclave_freshness` passes; stderr `[live] shape=Chutes freshness=PerEnclave`
   - `live_tinfoil_route_refused` passes (returns Err); stderr shows the
     `RedpillError::TinfoilUnsupported` (or HTTP 502 `Unsupported Tinfoil`) error
4. Sign off below:
   - [ ] Live Shape A (Phala-pure) passed
   - [ ] Live Shape B (Orchestrated three-way AND) passed
   - [ ] Live Shape C (Chutes per-enclave freshness) passed
   - [ ] Live Tinfoil refusal passed
   - [ ] Settings → Providers shows Redpill row; verified badge renders correctly
   - [ ] (Optional) `REDPILL_API_KEY` set, end-to-end chat completion via Redpill backend works

## Threat Model Closure

All threats T-34-01 through T-34-11 are mitigated or accepted across the four
plan threat models. Summary:

| Threat | Disposition | Verified By |
|--------|-------------|-------------|
| T-34-01 (REPORTDATA layout spoofing) | mitigate | Plan 02 — 8 RED stubs across all four shape decoders pin byte-slice asserts |
| T-34-02 (three-way AND bypass) | mitigate | Plan 02 — `shape_b_three_way_and_gates_session_open` |
| T-34-03 (Chutes per-enclave freshness misrepresentation) | mitigate | Plan 02 — `shape_c_client_nonce_not_bound`; Plan 04 — `redpill_chutes_shape_carries_per_enclave_freshness` |
| T-34-04 (TDX debug-mode bit) | mitigate | Plan 02 — `debug_bit_clear_in_all_captures` + `debug_bit_set_rejected` |
| T-34-05 (provider preset spoofing) | mitigate | Plan 03 — preset hard-coded; Plan 04 — `redpill_routes_to_redpill_transport` |
| T-34-06 (provider preset spoofing — UI) | mitigate | Plan 04 — `backend_summary_after_add` |
| T-34-07 (stale TDX quote replay) | mitigate | Plan 02 — per-request nonce embedded in REPORTDATA[32..64] for Shape A and B; 5-min TTL cache |
| T-34-08 (Tinfoil-routed bypassing direct-Tinfoil) | mitigate | Plan 03 — `check_model_routable` + Plan 02 HTTP 502 detection; Plan 04 live test `live_tinfoil_route_refused` |
| T-34-09 (chat sent before attestation verifies) | mitigate | Plan 03 — `ensure_verified_redpill_attestation` precedes any chat POST |
| T-34-10 (streaming dispatch skipping attestation gate) | mitigate | Plan 04 — `ProviderTransportKind` exhaustively matched; missing arm causes compile failure; Redpill streaming entry points call `ensure_verified_redpill_attestation` first |
| T-34-11 (UI mislabeling Chutes freshness as per-request) | mitigate | Plan 04 — `freshness` is a discriminator field on `AttestationEvent::Verified`; UI cannot conflate; `redpill_chutes_shape_carries_per_enclave_freshness` test pins it |

## Phase 34 Plan Commit Log

| Plan | Commits | Subject |
|---|---|---|
| 34-01 | 1181aa5, 038af19, 53f0547 | Wave 0 — fixtures + RED stubs + VALIDATION |
| 34-02 | 3b8ef7f, a12bd07, 95b6dea | Attestation layer + decoders + orchestrator |
| 34-03 | 7a6de21, b8e8727, b1cc106 | LLM transport + preset + Tinfoil refusal |
| 34-04 | (this plan — see git log for hashes) | Wiring + AttestationEvent fields + live tests |

## Status After User Sign-Off

On user sign-off above:
- Mark RED-01..RED-11 as `[x]` Complete in `.planning/REQUIREMENTS.md`
- Update Traceability rows from Pending to Complete
- Mark Phase 34 as `[x]` Complete in `.planning/ROADMAP.md`
- Phase 34 closes; the deferred items (E2EE handshakes, Automata receipts, ITA
  appraisal, dstack-deep, Tinfoil-via-Redpill quarterly re-probe) carry over
  to follow-up phases per `34-CONTEXT.md`'s `<deferred>` block.
