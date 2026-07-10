---
phase: 34
plan: 04
subsystem: integration
tags:
  - redpill
  - transport-routing
  - uniffi-bindings
  - attestation-event
  - live-integration
  - wave-3
dependency_graph:
  requires:
    - Phase 34 Plan 01 (golden fixtures + RED stubs)
    - Phase 34 Plan 02 (attestation/redpill.rs verify orchestrator)
    - Phase 34 Plan 03 (llm/redpill.rs transport + ProviderKind::Redpill + preset)
  provides:
    - ProviderTransportKind::Redpill variant + dispatch arms in transport.rs
    - streaming.rs Redpill dispatch (both ChatMessage and api_messages paths)
    - agent/loop.rs Redpill arm (Rule 3 fix — match exhaustiveness)
    - AttestationEvent::Verified additive shape/freshness/orchestrated_components fields
    - 4 live integration tests (Shape A/B/C + Tinfoil refusal)
    - 34-VERIFICATION.md phase-level audit log
  affects:
    - Native UI badge renderers (iOS/Android/Desktop) consume the three new
      AttestationEvent fields for RED-09 freshness sub-line and RED-11
      three-way Orchestrated breakdown
tech-stack:
  added: []
  patterns:
    - "Additive Option fields on AttestationEvent::Verified — backwards-compatible.
      Existing Tinfoil/PPQ/Venice/TDX/NVIDIA/SEV-SNP callers pass None."
    - "Exhaustive ProviderTransportKind matching — no wildcard arm — so adding
      a new transport variant produces compile errors at every dispatch site
      (T-34-10 mitigation: streaming/agent dispatch cannot silently skip the
      Redpill attestation gate)."
    - "Agent loop Rule 3 fix: when adding a transport variant the
      `agent/loop.rs::run_agent_step_for_backend` match must be extended too —
      the plan's <files> didn't list it but compile-fail forces the change."
key-files:
  created:
    - .planning/phases/34-integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggrega/34-VERIFICATION.md
  modified:
    - rust/src/llm/transport.rs (Redpill variant + 3 dispatch arms)
    - rust/src/llm/streaming.rs (Redpill dispatch in both spawn entries)
    - rust/src/agent/loop.rs (Redpill arm — Rule 3)
    - rust/src/attestation/mod.rs (3 additive Option fields on AttestationEvent::Verified)
    - rust/src/llm/redpill.rs (verify_backend_attestation populates new fields)
    - rust/src/llm/venice.rs, ppq_private.rs, tinfoil_secure.rs (None pass-through)
    - rust/src/attestation/endpoint.rs (2 sites), nvidia.rs, tdx.rs (None pass-through)
    - rust/src/lib.rs (actor-loop destructure pattern adds shape:_, freshness:_, orchestrated_components:_)
    - rust/src/tests/redpill.rs (RED-11 backend_summary_after_add GREEN; +2 new tests)
    - rust/src/tests/transport.rs (redpill_routes_to_redpill_transport regression test)
    - rust/src/tests/live_redpill.rs (4 #[tokio::test] #[ignore] live tests)
    - rust/src/tests/attestation_types.rs, attestation_integration.rs (caller updates)
decisions:
  - "openai_api_base for Redpill returns Ok({base}/v1). Redpill is OpenAI-compatible
    (no E2EE wrapper) — unlike Tinfoil/PPQ/Venice which return Err. The streaming
    dispatcher short-circuits Redpill to llm::redpill::run_streaming_chat_completion
    BEFORE the generic OpenAI client path, but exposing api_base correctly keeps
    the code defensible if a future caller routes through it."
  - "AttestationEvent shape/freshness/orchestrated_components are String/Vec<(String,String)>
    rather than typed enums. Rationale: AttestationEvent is internal (not UniFFI-exported);
    using owned strings keeps SQLite serialization trivial when the cache layer is
    extended in a future phase, and avoids leaking RedpillShape/Freshness types out
    of the attestation::redpill module to actor-loop code."
  - "Agent loop fix folded into Task 1 (Rule 3 — auto-fix blocking compile error)."
  - "Live tests do NOT require an API key. The Redpill attestation endpoint is
    public per CONTEXT D-02; only chat completions need REDPILL_API_KEY (no
    chat-completion live tests in this plan — those would be a follow-up if needed)."
metrics:
  duration: ~30 minutes
  completed: 2026-04-26
  tasks: 3
  commits: 3
---

# Phase 34 Plan 04: Redpill Wiring + UI Surfacing + Live Integration Summary

Final wave of Phase 34 (Redpill TEE-attested aggregator): wired Plan 02's
attestation orchestrator and Plan 03's transport into the existing routing
pipeline by adding `ProviderTransportKind::Redpill` plus dispatch arms in
`transport.rs`, `streaming.rs` (both `spawn_streaming_task` and the
`api_messages` variant), and `agent/loop.rs` (Rule 3 — match exhaustiveness);
extended `AttestationEvent::Verified` with three additive `Option` fields
(`shape`, `freshness`, `orchestrated_components`) so the native UI badge can
render RED-09's per-enclave freshness sub-line for Chutes and RED-11's
three-way "gateway ✓ • model ✓ • compose ✓" breakdown for Orchestrated;
flipped the RED-11 `backend_summary_after_add` stub GREEN; added four
`#[ignore]`-gated live integration tests (Shape A/B/C + Tinfoil refusal)
against `api.redpill.ai`; and committed `34-VERIFICATION.md` with the full
RED-01..RED-11 RED→GREEN tally, threat-model closure for T-34-01..T-34-11,
manual live-test instructions, and user sign-off checklist. Full lib suite:
**382 passed, 0 failed, 18 ignored** (4 of which are this plan's live tests);
release build clean; zero new Cargo dependencies.

## What Shipped

### Transport routing (`rust/src/llm/transport.rs`)

- `ProviderTransportKind::Redpill` variant added.
- `for_backend`: routes `provider_kind() == ProviderKind::Redpill` to `Self::Redpill`.
- `model_list_url`: `Self::Redpill => super::redpill::model_list_url(backend)`.
- `openai_api_base`: returns `Ok({base}/v1)` (Redpill IS OpenAI-compatible).
- `build_reqwest_client`: returns `super::redpill::build_http_client(timeout)`.

### Streaming dispatch (`rust/src/llm/streaming.rs`)

Two new dispatch arms before the generic OpenAI path — one each in
`spawn_streaming_task` (for `Vec<ChatMessage>`) and in
`spawn_streaming_task_from_api_messages` (for tool-followup with
`ChatCompletionRequestMessage` direct):

```rust
if transport == super::transport::ProviderTransportKind::Redpill {
    crate::llm::redpill::run_streaming_chat_completion[_from_api_messages](
        backend, model, messages, [None,] token_for_task, core_tx,
    ).await;
    return;
}
```

### Agent loop (`rust/src/agent/loop.rs`) — Rule 3 fix

Adding a `ProviderTransportKind` variant turned the existing
`run_agent_step_for_backend` match into a non-exhaustive pattern. Added the
Redpill arm calling `crate::llm::redpill::create_chat_completion`. The plan's
`<files>` block did not list `agent/loop.rs`, but the change was required for
compile.

### AttestationEvent::Verified extension (`rust/src/attestation/mod.rs`)

Three additive `Option` fields:

```rust
shape: Option<String>,                                 // "Flat" | "Orchestrated" | "Chutes"
freshness: Option<String>,                             // "PerRequest" | "PerEnclave"
orchestrated_components: Option<Vec<(String, String)>>, // [("gateway", hex), ("model", hex), ("compose_manager", hex)]
```

`AttestationEvent` is **internal** (not `#[derive(uniffi::Enum)]`) so the
additive fields are FFI-safe by construction — no UniFFI bindings churn.
All seven existing constructors (Tinfoil/PPQ/Venice/TDX/NVIDIA/SEV-SNP) pass
`None` for the three new fields. The `lib.rs` actor-loop destructure pattern
ignores them with `shape: _, freshness: _, orchestrated_components: _`.

### Redpill verify_backend_attestation (`rust/src/llm/redpill.rs`)

Now populates all three fields from `VerifiedRedpillAttestation`:

| Verified shape | shape | freshness | orchestrated_components |
|---|---|---|---|
| `RedpillShape::Flat` | `Some("Flat")` | `Some("PerRequest")` | `None` |
| `RedpillShape::Orchestrated { .. }` | `Some("Orchestrated")` | `Some("PerRequest")` | `Some(vec![gateway, model, compose_manager])` |
| `RedpillShape::Chutes` | `Some("Chutes")` | `Some("PerEnclave")` | `None` |

### Tests added

| File | Test | Pins |
|---|---|---|
| `tests/transport.rs` | `redpill_routes_to_redpill_transport` | provider_kind → Redpill → model_list_url → openai_api_base; existing routes (Venice/Tinfoil) unaffected |
| `tests/redpill.rs` | `backend_summary_after_add` | RED-11 — provider_kind, transport_kind, BackendSummary surface fields |
| `tests/redpill.rs` | `redpill_orchestrated_event_carries_three_components` | RED-11 — three-way badge breakdown labels and order |
| `tests/redpill.rs` | `redpill_chutes_shape_carries_per_enclave_freshness` | RED-09 — Chutes uses PerEnclave, no orchestrated_components |
| `tests/live_redpill.rs` | `live_shape_a_phala_pure` | live RED-04..RED-08 over Shape A |
| `tests/live_redpill.rs` | `live_shape_b_orchestrated_three_way_and` | live RED-06 — three-way AND + populated components |
| `tests/live_redpill.rs` | `live_shape_c_chutes_per_enclave_freshness` | live RED-09 — PerEnclave freshness |
| `tests/live_redpill.rs` | `live_tinfoil_route_refused` | live RED-10 — Tinfoil-routed model fails closed |

### RED → GREEN final tally (RED-01..RED-11)

| Req | Status | Plan(s) | Test(s) |
|---|---|---|---|
| RED-01 (provider preset) | GREEN | 34-01, 34-03 | `redpill_preset_present` |
| RED-02 (attestation URL) | GREEN | 34-01, 34-03 | `attestation_url_format`, `format_attestation_url_urlencodes_model_id` |
| RED-03 (shape dispatcher) | GREEN | 34-01, 34-02 | 4 dispatch tests |
| RED-04 (quote_bytes) | GREEN | 34-01, 34-02 | 3 hex/b64/0x tests |
| RED-05a (Shape A model) | GREEN | 34-01, 34-02 | 4 tests |
| RED-05b (Shape B gateway) | GREEN | 34-01, 34-02 | `shape_b_gateway_reportdata_ok` |
| RED-05c (Shape B compose-mgr) | GREEN | 34-01, 34-02 | `shape_b_compose_manager_reportdata_ok` |
| RED-05d (Shape C anti-tamper) | GREEN | 34-01, 34-02 | 2 tests incl. client_nonce_not_bound |
| RED-06 (three-way AND) | GREEN | 34-02 | `shape_b_three_way_and_gates_session_open` |
| RED-07 (NVIDIA NRAS) | GREEN | 34-02 | NRAS double-parse + per-GPU loop |
| RED-08 (debug-mode gate) | GREEN | 34-02 | clear-in-all + synthetic-set-rejected |
| RED-09 (per-enclave freshness UI) | GREEN (live: pending sign-off) | 34-04 | `redpill_chutes_shape_carries_per_enclave_freshness` + `live_shape_c_chutes_per_enclave_freshness` |
| RED-10 (Tinfoil refusal) | GREEN (live: pending sign-off) | 34-03, 34-04 | typed-error + user-facing + `live_tinfoil_route_refused` |
| RED-11 (Verified badge breakdown) | GREEN | 34-04 | `backend_summary_after_add` + `redpill_orchestrated_event_carries_three_components` + `redpill_routes_to_redpill_transport` |

## Manual Live Test (User Action Required)

Exact command:
```
cd rust && cargo test -p mango_core --lib live_redpill -- --ignored --nocapture
```

No `REDPILL_API_KEY` needed for the four live tests — Redpill's attestation
endpoint is public (CONTEXT D-02). Sign-off checklist lives in `34-VERIFICATION.md`.

## Validation Commands

```bash
# Full lib (deterministic suite must pass; live tests stay ignored)
cd rust && cargo test -p mango_core --lib
# → 382 passed; 0 failed; 18 ignored

# Plan 04 specific tests
cd rust && cargo test -p mango_core --lib redpill::backend_summary_after_add \
                                          redpill::redpill_orchestrated_event_carries_three_components \
                                          redpill::redpill_chutes_shape_carries_per_enclave_freshness \
                                          transport::redpill_routes_to_redpill_transport
# → 4 passed; 0 failed

# Release build (uniffi build.rs runs)
cd rust && cargo build -p mango_core --release
# → exits 0

# Match-arm completeness
rg '^\s*Self::Redpill' rust/src/llm/transport.rs | wc -l    # → 3 (model_list_url, openai_api_base, build_reqwest_client)
rg 'redpill::run_streaming_chat_completion' rust/src/llm/streaming.rs | wc -l   # → 2 (both entry points)
```

## Acceptance Criteria — Verified

| Criterion | Result |
|---|---|
| `ProviderTransportKind::Redpill` variant + ≥3 match arms | PASS |
| `transport.rs` delegates to `super::redpill::*` | PASS |
| `streaming.rs` dispatches `ProviderTransportKind::Redpill` | PASS (both entry points) |
| `streaming.rs` `_from_api_messages` variant wired | PASS |
| RED-11 `backend_summary_after_add` GREEN | PASS |
| `redpill_routes_to_redpill_transport` regression test PASS | PASS |
| `cargo test -p mango_core --lib` 0 failures | PASS — 382/0/18 |
| `cargo build -p mango_core --release` exits 0 | PASS |
| `34-VERIFICATION.md` exists with full audit table covering RED-01..RED-11 | PASS |
| Live tests for Shape A + B + C (preferred) + Tinfoil refusal | PASS — 4 ignored tests |
| `AttestationEvent` shape + freshness + orchestrated_components fields | PASS |
| Existing Tinfoil/PPQ/Venice transport tests still pass | PASS — no regressions |

## Deviations from Plan

**One Rule-3 auto-fix:**

**1. [Rule 3 — Blocking] Add `ProviderTransportKind::Redpill` arm to `agent/loop.rs::run_agent_step_for_backend`**
- **Found during:** Task 1 build
- **Issue:** Adding the `ProviderTransportKind::Redpill` enum variant turned the
  existing `match backend.transport_kind()` in `agent/loop.rs:134` into a
  non-exhaustive pattern (E0004). The plan's `<files>` block listed
  `transport.rs`, `streaming.rs`, `router.rs`, etc., but not `agent/loop.rs`.
- **Fix:** Added the Redpill arm calling `crate::llm::redpill::create_chat_completion`
  (mirrors the Venice arm). Required for compile.
- **Files modified:** `rust/src/agent/loop.rs`
- **Commit:** 81ea8b3 (folded into Task 1)

Otherwise: plan executed exactly as written. The plan also contemplated
`router.rs` changes — inspection confirmed `router.rs` does NOT match on
`ProviderTransportKind` or `ProviderKind`; it only manipulates health state
keyed by `backend_id`. No router changes required.

The Plan-Task split also folded the originally-suggested
`AttestationEvent::Verified` ChutesNoNonceBinding-discriminator decision (Plan
02 SUMMARY) into the additive-Option-fields approach — the rationale was
preserved in this SUMMARY's `decisions` block. AttestationEvent stays internal,
so a stronger typed enum (e.g. `Freshness::PerEnclave`) was not required at
the cross-module boundary; owned strings were chosen for ergonomic SQLite
serialization in a future cache phase.

## Authentication Gates

None for the deterministic suite. The four live tests in `live_redpill.rs`
do NOT require any authentication — the Redpill attestation endpoint is public.
A follow-up phase that adds chat-completion live tests would require
`REDPILL_API_KEY`; that's documented as a manual step in `34-VERIFICATION.md`.

## Threat Mitigation Recap

| Threat | Mitigated By | Test |
|---|---|---|
| T-34-06 (provider preset spoofing — UI) | Preset hard-coded in `known_provider_presets`; router matches on `id == "redpill"`. | `backend_summary_after_add` |
| T-34-10 (streaming dispatch skipping Redpill attestation gate) | `ProviderTransportKind` exhaustively matched (no wildcard); `llm::redpill::run_streaming_chat_completion` calls `ensure_verified_redpill_attestation` before the async-openai POST. Compile fail forces every dispatch site to be updated when a new transport is added. | wired in code; `redpill_routes_to_redpill_transport` pins the transport routing |
| T-34-11 (UI mislabeling Chutes freshness as per-request) | `freshness` is a discriminator field on `AttestationEvent::Verified` — Chutes always sets it to `"PerEnclave"`, no other shape does. UI cannot conflate. | `redpill_chutes_shape_carries_per_enclave_freshness` |

## Commits

| Task | Commit | Subject |
|------|--------|---------|
| 1 | 81ea8b3 | feat(34-04): wire Redpill into transport/streaming + flip RED-11 |
| 2 | 49c19cc | feat(34-04): surface shape+freshness+orchestrated_components on AttestationEvent::Verified |
| 3 | 424a852 | test(34-04): live integration tests + 34-VERIFICATION.md |

## Self-Check

- [x] `rust/src/llm/transport.rs` has `ProviderTransportKind::Redpill` and 3 match arms
- [x] `rust/src/llm/streaming.rs` dispatches Redpill in both entry points
- [x] `rust/src/agent/loop.rs` has Redpill arm
- [x] `rust/src/attestation/mod.rs` carries shape/freshness/orchestrated_components
- [x] `rust/src/llm/redpill.rs::verify_backend_attestation` populates all three
- [x] `rust/src/lib.rs` destructure pattern ignores new fields
- [x] `rust/src/tests/redpill.rs` has 3 GREEN tests + RED-11 flipped GREEN
- [x] `rust/src/tests/transport.rs` has `redpill_routes_to_redpill_transport`
- [x] `rust/src/tests/live_redpill.rs` has 4 `#[tokio::test] #[ignore]` functions
- [x] `34-VERIFICATION.md` committed
- [x] All three commits exist in git log: 81ea8b3, 49c19cc, 424a852
- [x] `cargo test -p mango_core --lib`: 382 passed, 0 failed
- [x] `cargo build -p mango_core --release` exits 0

## Self-Check: PASSED
