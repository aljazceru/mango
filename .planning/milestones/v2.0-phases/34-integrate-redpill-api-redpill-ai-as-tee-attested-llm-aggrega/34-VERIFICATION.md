---
phase: 34-integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggrega
verified: 2026-04-26T00:00:00Z
status: human_needed
score: 9/11 must-haves verified (RED-01..RED-08, RED-10, RED-11 core; RED-09 partial; live tests pending sign-off)
overrides_applied: 0
re_verification:
  previous_status: complete_pending_user_signoff
  previous_score: 11/11 (executor self-report)
  gaps_closed: []
  gaps_remaining:
    - "RED-09 trust-UI rendering of PerEnclave freshness sub-line — data dropped at actor-loop (`shape: _, freshness: _, orchestrated_components: _`); never reaches UniFFI AttestationStatus"
    - "RED-11 three-way Orchestrated breakdown — same data drop at actor-loop; native UI cannot render gateway/model/compose breakdown"
  regressions: []
gaps:
  - truth: "Chutes-routed Redpill models display 'freshness valid for enclave lifetime' in the trust UI (ROADMAP SC #8 / RED-09)"
    status: partial
    reason: |
      Cryptographic path is correct: `verify_backend_attestation` populates
      `AttestationEvent::Verified.freshness = Some(\"PerEnclave\")` for Shape C
      (verified by `redpill_chutes_shape_carries_per_enclave_freshness`).
      However, the actor-loop in `rust/src/lib.rs:7428-7430` destructures and
      DROPS the new fields with `shape: _, freshness: _, orchestrated_components: _`.
      The UniFFI-exported `AttestationStatus` enum (`rust/src/attestation/mod.rs:31`)
      has only `Verified | Unverified | Failed | Expired` variants — no
      freshness sub-field. Data flows into the internal event but is not
      persisted to `AttestationRecord` (SQLite) nor surfaced across the FFI
      boundary, so iOS / Android / Desktop UI layers cannot render the
      "valid for enclave lifetime" sub-line.
    artifacts:
      - path: "rust/src/lib.rs"
        issue: "Actor loop destructures shape/freshness/orchestrated_components as `_`; comment at line 7421-7427 explicitly defers UI surfacing to a future phase"
      - path: "rust/src/attestation/mod.rs"
        issue: "AttestationStatus enum has no freshness/shape variants; AttestationRecord struct has no shape/freshness/orchestrated_components columns"
    missing:
      - "Extend `AttestationStatus` (UniFFI-exported) with freshness + shape sub-fields, OR add a parallel UniFFI struct for badge metadata"
      - "Propagate fields from AttestationEvent::Verified into AppState's backend status entries"
      - "Native UI rendering of 'Verified for this enclave instance' copy on the Redpill row when freshness == PerEnclave"
  - truth: "Orchestrated-shape Redpill models show three-quote verification status (gateway / model / compose-manager) in the badge UI (RED-11 second clause)"
    status: partial
    reason: |
      `verify_backend_attestation` populates `orchestrated_components: Some(vec![(gateway, hex), (model, hex), (compose_manager, hex)])`
      (verified by `redpill_orchestrated_event_carries_three_components`).
      Same actor-loop drop: the field is ignored with `_` at lib.rs:7430 and
      never reaches AttestationStatus / AttestationRecord. Native UI has no
      access to the three component hex addresses.
    artifacts:
      - path: "rust/src/lib.rs"
        issue: "orchestrated_components: _ — dropped at the destructure"
      - path: "android/app/src/main/java/dev/disobey/mango/ui/SettingsProvidersScreen.kt"
        issue: "ProviderStatusPill renders only AttestationStatus (Verified/Failed/etc); no orchestrated component breakdown rendering"
    missing:
      - "Surface orchestrated_components across UniFFI"
      - "Native UI: three-way 'gateway ✓ • model ✓ • compose ✓' sub-row on Redpill provider detail when shape == Orchestrated"
human_verification:
  - test: "Live Shape A — Phala-pure attestation"
    expected: "`cargo test -p mango_core --lib live_redpill -- --ignored --nocapture` produces `[live] shape=Flat freshness=PerRequest`; test passes"
    why_human: "Hits public network (api.redpill.ai); requires fresh nonce + working Intel PCS collateral; cannot run inside a deterministic sandbox"
  - test: "Live Shape B — Orchestrated three-way AND"
    expected: "Test passes; stderr shows three component hex addresses (gateway / model / compose_manager); failure of any one component fails the whole attestation"
    why_human: "Live network + cryptographic flow against api.redpill.ai"
  - test: "Live Shape C — Chutes per-enclave freshness"
    expected: "Test passes; stderr shows `[live] shape=Chutes freshness=PerEnclave`"
    why_human: "Live network + Chutes anti-tamper SHA-256 binding against fresh capture"
  - test: "Live Tinfoil refusal"
    expected: "Test returns Err with `RedpillError::TinfoilUnsupported` (or HTTP 502 'Unsupported Tinfoil') for `meta-llama/llama-3.3-70b-instruct`"
    why_human: "Depends on Redpill's current `/v1/models` provider mapping AND the relay's 502 behaviour; both are live"
  - test: "Settings → Providers shows Redpill row on at least one platform"
    expected: "Redpill appears with TEE badge after Add Backend; Verified badge renders once API key is set and attestation succeeds"
    why_human: "Visual UI verification on iOS / Android / Desktop; programmatic check is limited to the preset existing in `known_provider_presets`"
  - test: "(Optional) End-to-end chat completion via Redpill backend"
    expected: "With REDPILL_API_KEY set, send a message to a Phala-pure model; response arrives; AttestationEvent::Verified observed"
    why_human: "Live API key + paid usage; user must opt in"
deferred:
  - truth: "On-chain DCAP receipts via Automata"
    addressed_in: "Future phase (post-34)"
    evidence: "34-CONTEXT.md <deferred>: 'On-chain DCAP receipts via Automata smart contracts (deferred)'"
  - truth: "Intel Trust Authority secondary appraisal"
    addressed_in: "Future phase (post-34)"
    evidence: "34-CONTEXT.md <deferred>: 'Intel Trust Authority secondary appraisal (deferred)'"
  - truth: "dstack-deep boot-replay"
    addressed_in: "v3 (out of mobile sandbox)"
    evidence: "34-CONTEXT.md <deferred>: 'dstack-deep boot-replay verification (requires Docker, not viable on mobile)'"
  - truth: "Tinfoil-via-Redpill integration"
    addressed_in: "Quarterly re-probe + future phase"
    evidence: "34-CONTEXT.md <deferred>: 'Tinfoil-via-Redpill integration (broken at Redpill's relay; existing direct-Tinfoil integration covers SEV-SNP)' — D-17"
  - truth: "E2EE handshakes (Chutes HPKE / Phala ECDSA)"
    addressed_in: "Follow-up phase (deferred)"
    evidence: "34-CONTEXT.md <deferred>: 'E2EE handshakes ... TEE attestation is the confidentiality root, HTTPS handles transport.'"
---

# Phase 34 — Verifier Audit Report

**Phase:** 34 — Integrate Redpill (api.redpill.ai) as TEE-attested LLM aggregator
**Audited:** 2026-04-26 (re-verification of executor-authored VERIFICATION.md)
**Status:** human_needed (live integration tests + visual UI + 2 partial gaps)
**Verifier:** goal-backward audit; does not blindly trust SUMMARY claims

## Goal Achievement

ROADMAP goal: *"Redpill is selectable as a fourth TEE-attested LLM provider; every chat completion goes through fully verified TDX (and NRAS where present) attestation, with the client correctly dispatching the three Redpill response shapes (Phala-flat, Phala-orchestrated 3-quote, Chutes anti-tamper) and reusing the Venice REPORTDATA decoder for the model component. No new Rust crates. Attestation failures fail-closed. Redpill→Tinfoil routes are explicitly unsupported until Redpill upstream upgrades its relay."*

### Observable Truths (ROADMAP Success Criteria + RED-01..RED-11)

| #   | Truth (Source) | Status | Evidence |
| --- | -------------- | ------ | -------- |
| 1   | Redpill appears as a known provider preset in Add Backend (SC #1, RED-01) | ✓ VERIFIED | `rust/src/llm/backend.rs:170-176` ProviderPreset entry; `rust/src/llm/backend.rs:15` ProviderKind::Redpill; test `tests::redpill::redpill_preset_present` GREEN |
| 2   | Client fetches `GET /v1/attestation/report?model=&nonce=` without API key, response verified before any chat (SC #2, RED-02) | ✓ VERIFIED | `attestation/redpill.rs:358 fetch_and_verify_redpill_attestation`, `llm/redpill.rs:78 format_redpill_attestation_url`, attestation gate at `llm/redpill.rs:166` BEFORE chat POST; tests `attestation_url_format` + `format_attestation_url_urlencodes_model_id` GREEN |
| 3   | Client correctly identifies Flat / Orchestrated / Chutes and applies right REPORTDATA decoder (SC #3, RED-03, RED-05) | ✓ VERIFIED | `attestation/redpill.rs:121 detect_shape` + 4 decoders (`verify_redpill_model_reportdata` re-exports Venice; `verify_redpill_gateway_reportdata`, `verify_redpill_compose_manager_reportdata`, `verify_redpill_chutes_anti_tamper`); 8+ unit tests GREEN against golden captures |
| 4   | Orchestrated three-way AND: all three TDX quotes verify or fail-closed (SC #4, RED-06) | ✓ VERIFIED | `attestation/redpill.rs:519 verify_orchestrated`; `RedpillError::OrchestratedComponentFailed` variant; test `shape_b_three_way_and_gates_session_open` GREEN |
| 5   | TDX quote signature/PCK/TCB/CRL verified locally via `dcap-qvl` (SC #5, RED-04) | ✓ VERIFIED | `quote_bytes()` helper auto-detects hex/b64; uses same `dcap-qvl` path as Venice/PPQ — no new crates per `Cargo.toml`; tests `quote_bytes_hex_round_trip`, `quote_bytes_base64_round_trip`, `quote_bytes_strips_0x_prefix` GREEN |
| 6   | NVIDIA NRAS verification reused unchanged for Shape A/B + per-GPU loop for Shape C (SC #6, RED-07) | ✓ VERIFIED | `attestation/redpill.rs` re-uses `attestation/nvidia.rs::fetch_and_verify_nvidia`; tests `nvidia_payload_double_parse_shape_a` + `nvidia_per_gpu_loop_shape_c` GREEN; live exercise gated `#[ignore]` |
| 7   | TDX debug-mode bit rejected across all shapes (SC #7, RED-08) | ✓ VERIFIED | `attestation/redpill.rs:169 debug_mode_disabled` + checked in all three verify paths (`verify_flat`/`verify_orchestrated`/`verify_chutes`); tests `debug_bit_clear_in_all_captures` + `debug_bit_set_rejected` GREEN |
| 8   | Chutes models display "freshness valid for enclave lifetime"; Flat/Orchestrated display per-request (SC #8, RED-09) | ⚠️ PARTIAL | Cryptographic side correct: `Freshness::PerEnclave` populated for Shape C, `Freshness::PerRequest` for A/B (test `redpill_chutes_shape_carries_per_enclave_freshness`). **GAP:** actor-loop drops the field (`lib.rs:7429 freshness: _`); UniFFI `AttestationStatus` has no freshness sub-field; UI cannot render. Documented as deferred to "future cache columns" in lib.rs:7421-7427. |
| 9   | Tinfoil-routed Redpill models surface clear error pointing to direct-Tinfoil (SC #9, RED-10) | ✓ VERIFIED | `llm/redpill.rs:366 check_model_routable` refuses on `providers: ["tinfoil"]`; `RedpillError::TinfoilUnsupported`; user-facing error message contains "tinfoil"+"direct"; HTTP 502 fallback in `fetch_and_verify` orchestrator; tests `tinfoil_route_refused_with_typed_error` + `tinfoil_user_facing_error_mentions_direct_tinfoil` GREEN; live test `live_tinfoil_route_refused` ignored |
| 10  | E2E live integration test passes against `api.redpill.ai` for at least one model per shape (SC #10) | ? HUMAN | 4 `#[ignore]`-gated tests in `tests/live_redpill.rs` (Shape A/B/C + Tinfoil refusal). Sandbox cannot hit live network. Requires user sign-off. |
| 11  | Redpill row appears in Settings → Providers with Verified badge after attestation; orchestrated-shape three-quote breakdown (RED-11) | ⚠️ PARTIAL | Row appearance: ✓ — `knownProviderPresets()` UniFFI is single source of truth, Android `SettingsProvidersScreen.kt` iterates presets. Verified badge: ✓ — `AttestationStatus::Verified` propagates after `verify_backend_attestation` runs. **GAP:** three-quote breakdown — `orchestrated_components` populated in AttestationEvent but dropped at actor loop (`lib.rs:7430 orchestrated_components: _`); UI has no data path to render gateway/model/compose breakdown. |

**Score:** 9/11 verified, 2 partial, 1 human-needed

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `rust/src/attestation/redpill.rs` | Shape dispatcher + 4 REPORTDATA decoders + debug-mode gate + orchestrator | ✓ VERIFIED | 800 lines; 13 public functions; pub use re-export of Venice decoder for single source of truth |
| `rust/src/llm/redpill.rs` | Provider transport: model_list_url, format_attestation_url, verify_backend_attestation, create_chat_completion, streaming entries, check_model_routable | ✓ VERIFIED | 527 lines; attestation gate runs before every chat POST |
| `rust/src/llm/transport.rs` | ProviderTransportKind::Redpill + 3 dispatch arms | ✓ VERIFIED | Variant at line 19; `for_backend`/`model_list_url`/`build_reqwest_client` arms present |
| `rust/src/llm/streaming.rs` | Redpill dispatch in both streaming entry points before generic OpenAI path | ✓ VERIFIED | Lines 200, 351 — both `spawn_streaming_task` and `_from_api_messages` short-circuit to `redpill::run_streaming_chat_completion[_from_api_messages]` |
| `rust/src/agent/loop.rs` | Redpill arm in `run_agent_step_for_backend` (Rule 3 fix) | ✓ VERIFIED | Line 158 — `ProviderTransportKind::Redpill` arm calls `crate::llm::redpill::create_chat_completion` |
| `rust/src/attestation/mod.rs` | AttestationEvent::Verified extended with shape/freshness/orchestrated_components | ✓ VERIFIED | Lines 84, 89, 94 — three additive `Option` fields |
| `rust/src/lib.rs` | Actor loop destructure ignores new fields (deferred surfacing) | ⚠️ ORPHANED | Lines 7428-7430 — `shape: _, freshness: _, orchestrated_components: _`. Code-comment at lines 7421-7427 explicitly defers UI surfacing. The fields exist but are **structurally orphaned at the actor loop boundary**. |
| `rust/src/tests/redpill.rs` | RED-11 backend_summary_after_add + 2 RED-09/RED-11 tests | ✓ VERIFIED | 178 lines; 5 `#[test]` functions, all GREEN |
| `rust/src/tests/attestation_redpill.rs` | Decoder + dispatcher + debug-mode + three-way AND tests against captures | ✓ VERIFIED | 346 lines; ~16 `#[test]` functions covering all REPORTDATA layouts and gates |
| `rust/src/tests/live_redpill.rs` | 4 `#[tokio::test] #[ignore]` for Shape A/B/C + Tinfoil refusal | ✓ VERIFIED | 137 lines; 4 functions matching ROADMAP SC #10 expectation |
| `rust/src/tests/transport.rs::redpill_routes_to_redpill_transport` | Regression test pinning transport routing | ✓ VERIFIED | Mentioned in test results; passes |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `create_chat_completion` (Redpill) | `ensure_verified_redpill_attestation` | direct call before async-openai POST | ✓ WIRED | `llm/redpill.rs:166` — attestation gate runs first; T-34-09 mitigated |
| `run_streaming_chat_completion[_from_api_messages]` | `ensure_verified_redpill_attestation` | direct call before stream POST | ✓ WIRED | `llm/redpill.rs:292` — gate runs first; T-34-10 mitigated by exhaustive ProviderTransportKind match |
| `streaming.rs::spawn_streaming_task` | `redpill::run_streaming_chat_completion` | dispatch on `ProviderTransportKind::Redpill` | ✓ WIRED | `streaming.rs:200-201` |
| `agent/loop.rs::run_agent_step_for_backend` | `redpill::create_chat_completion` | match arm on `ProviderTransportKind::Redpill` | ✓ WIRED | `agent/loop.rs:158-160` |
| `verify_backend_attestation` | `AttestationEvent::Verified { shape, freshness, orchestrated_components }` | populates from `VerifiedRedpillAttestation` | ✓ WIRED | `llm/redpill.rs:91-141` |
| `AttestationEvent::Verified` (Redpill new fields) | `AttestationStatus` (UniFFI) / `AttestationRecord` (SQLite) / native UI | actor loop destructure | ✗ NOT_WIRED | `lib.rs:7428-7430` — fields ignored with `_`; no propagation to UniFFI status, no SQLite columns, no native renderer. **This is the gap behind RED-09 and RED-11 partial status.** |
| `check_model_routable` (Tinfoil refusal) | `RedpillError::TinfoilUnsupported` → `LlmError` user-facing | `map_redpill_error_for_user` | ✓ WIRED | `llm/redpill.rs:160-162` (chat path), `:286-288` (stream path) |
| `verify_orchestrated` | three-way AND fail-closed | `RedpillError::OrchestratedComponentFailed` raised on any single failure | ✓ WIRED | Tested via `shape_b_three_way_and_gates_session_open` |
| Attestation cache | `attestation/cache.rs` 5-min TTL | per `(model, response_hash)` | ✓ WIRED | `ATTESTATION_TTL_SECS = 5*60` const at `attestation/redpill.rs:38` |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
| -------- | ------------- | ------ | ------------------ | ------ |
| `verify_backend_attestation` event | `shape`, `freshness`, `orchestrated_components` | `VerifiedRedpillAttestation` (live cryptographic verification) | Yes — string discriminant + Vec<(label, hex)> | ✓ FLOWING (into AttestationEvent) |
| Actor loop / AppState backend status | `freshness` UI sub-line / `orchestrated_components` breakdown | `AttestationEvent::Verified` | No — destructured to `_` and dropped at `lib.rs:7428-7430` | ✗ DISCONNECTED |
| Native UI (Android `ProviderStatusPill`) | freshness copy / 3-component badge | `AppState.attestationStatuses[i]` | No — `AttestationStatus` enum has only Verified/Unverified/Failed/Expired; no shape or freshness | ✗ HOLLOW_PROP |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Full lib suite passes | `cargo test -p mango_core --lib` | 382 passed; 0 failed; 18 ignored | ✓ PASS |
| Release build clean | `cargo build -p mango_core --release` (executor-reported, file timestamps consistent) | exit 0 | ✓ PASS |
| Redpill attestation gate present in chat path | `grep -n "ensure_verified_redpill_attestation" rust/src/llm/redpill.rs` | 4 hits — chat + 2 streaming entries + verify_backend_attestation | ✓ PASS |
| ProviderTransportKind exhaustively matched (no wildcard) | `grep -n "Self::Redpill\|Self::Tinfoil\|Self::Ppq\|Self::Venice" rust/src/llm/transport.rs` | All variants covered in 3 match arms | ✓ PASS |
| Tinfoil-route refusal wired in user-facing error | `grep -n "tinfoil.*direct\|direct.*tinfoil" rust/src/llm/redpill.rs` | error variant message + map_redpill_error_for_user | ✓ PASS |
| Live network tests | `cargo test -p mango_core --lib live_redpill -- --ignored` | requires public api.redpill.ai | ? SKIP (live net) |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
| ----------- | ----------- | ----------- | ------ | -------- |
| RED-01 | 34-03 | Provider preset on Add Backend | ✓ SATISFIED | `redpill_preset_present` GREEN; UniFFI `knownProviderPresets()` is the single source consumed by Android/iOS/Desktop |
| RED-02 | 34-03 | Public attestation endpoint URL format | ✓ SATISFIED | `attestation_url_format`, `format_attestation_url_urlencodes_model_id` GREEN; no Authorization header (verified by inspection of `fetch_and_verify_redpill_attestation`) |
| RED-03 | 34-02 | Shape dispatcher (Flat / Orchestrated / Chutes / fail-closed) | ✓ SATISFIED | 4 dispatch tests including `dispatch_unknown_shape_fails_closed` |
| RED-04 | 34-02 | TDX quote verification via `dcap-qvl`; auto-detect hex/b64 | ✓ SATISFIED | 3 quote_bytes tests + reused dcap-qvl path identical to Venice |
| RED-05 | 34-02 | Four REPORTDATA layouts (model / gateway / compose / chutes anti-tamper) | ✓ SATISFIED | One test per layout; Venice decoder re-exported via `pub use` |
| RED-06 | 34-02 | Three-way AND on Orchestrated | ✓ SATISFIED | `shape_b_three_way_and_gates_session_open` |
| RED-07 | 34-02 | NVIDIA NRAS reuse (Shape A/B nvidia_payload + Shape C gpu_evidence per-GPU loop) | ✓ SATISFIED | `nvidia_payload_double_parse_shape_a`, `nvidia_per_gpu_loop_shape_c` |
| RED-08 | 34-02 | Reject debug bit on all shapes | ✓ SATISFIED | `debug_bit_clear_in_all_captures`, `debug_bit_set_rejected` |
| RED-09 | 34-04 | "Freshness valid for enclave lifetime" UI surface for Chutes | ⚠️ PARTIAL | Cryptographic side complete; UI surfacing blocked at actor-loop drop (lib.rs:7428-7430). Executor explicitly deferred to future cache phase. |
| RED-10 | 34-03/34-04 | Tinfoil-via-Redpill refusal with hint | ✓ SATISFIED | typed-error + user-facing error message + live test |
| RED-11 | 34-04 | Backend appears with Verified badge; orchestrated three-quote breakdown | ⚠️ PARTIAL | Provider row + basic Verified badge: complete. Three-quote breakdown: data populated but dropped at actor loop; UI has no path. |

**Orphaned requirements:** None. All RED-01..RED-11 are claimed by ≥1 plan.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `rust/src/lib.rs` | 7428-7430 | `shape: _, freshness: _, orchestrated_components: _` — additive AttestationEvent fields ignored at actor loop with explicit "future cache columns" deferral comment | ⚠️ Warning | Blocks RED-09 and RED-11 trust-UI surfacing. Cryptographic verification still passes; UI just cannot render the freshness sub-line or three-quote breakdown that the requirements call for. Executor flagged this as deferred but the deferral is not documented in 34-CONTEXT.md `<deferred>` block. |
| `rust/src/attestation/redpill.rs` | 15 | `#![allow(dead_code)]` module-wide allow | ℹ️ Info | Suppresses warnings for currently-unused public surface (e.g. helper constants exposed for future test/cache work). Acceptable but worth tracking. |

No TODO/FIXME/PLACEHOLDER strings found in the four Redpill files.
No stub returns (`return null`, `return []`, `unimplemented!()`) found in production paths.

### Human Verification Required

1. **Live Shape A — Phala-pure attestation**
   - Test: `cargo test -p mango_core --lib live_redpill::live_shape_a_phala_pure -- --ignored --nocapture`
   - Expected: passes; stderr `[live] shape=Flat freshness=PerRequest model=openai/gpt-oss-20b`
   - Why human: live network call to api.redpill.ai

2. **Live Shape B — Orchestrated three-way AND**
   - Test: `cargo test -p mango_core --lib live_redpill::live_shape_b_orchestrated_three_way_and -- --ignored --nocapture`
   - Expected: passes; stderr shows three component hex addresses; failure of any one component would fail the whole attestation
   - Why human: live network + cryptographic flow

3. **Live Shape C — Chutes per-enclave freshness (RED-09)**
   - Test: `cargo test -p mango_core --lib live_redpill::live_shape_c_chutes_per_enclave_freshness -- --ignored --nocapture`
   - Expected: passes; stderr `[live] shape=Chutes freshness=PerEnclave`
   - Why human: live network

4. **Live Tinfoil refusal (RED-10)**
   - Test: `cargo test -p mango_core --lib live_redpill::live_tinfoil_route_refused -- --ignored --nocapture`
   - Expected: returns Err with `RedpillError::TinfoilUnsupported` (or HTTP 502)
   - Why human: depends on Redpill's current `/v1/models` mapping AND relay's 502 behaviour

5. **Settings → Providers shows Redpill row**
   - Test: launch app on iOS / Android / Desktop, navigate to Settings → Providers, confirm "Redpill" row with TEE badge
   - Expected: row appears; Add Backend → Redpill flow accepts an API key; once attestation succeeds, Verified badge renders
   - Why human: visual UI verification

6. **(Optional) End-to-end chat completion via Redpill backend**
   - Test: with `REDPILL_API_KEY` set, send a message to a Phala-pure model
   - Expected: response arrives; AttestationEvent::Verified observed in logs
   - Why human: live API key, paid usage

### Gaps Summary

The cryptographic and routing core of Phase 34 is **solid and well-tested**:
- All four REPORTDATA decoders implemented against golden captures
- Shape dispatcher fails closed on unknown shapes
- Three-way AND for Orchestrated; debug-mode gate everywhere
- Attestation gate fires BEFORE every chat POST and stream POST
- Tinfoil-routed models refused with typed error pointing to direct-Tinfoil
- Single source of truth for the Venice model decoder via `pub use` (no copy-paste)
- 382 tests pass; 0 failures; 18 ignored (4 belong to this phase as live tests)
- No new Cargo crates added — `dcap-qvl` and `nvidia.rs` reused unchanged
- Release build clean

**Two partial gaps**, both stemming from the same actor-loop drop:

1. **RED-09 trust-UI freshness sub-line**: `AttestationEvent::Verified.freshness = Some("PerEnclave")` is populated correctly for Shape C, but `lib.rs:7428-7430` destructures `freshness: _` — the field is dropped before reaching `AttestationStatus` (UniFFI), `AttestationRecord` (SQLite), or any native UI. The executor explicitly documented this as deferred via inline code comment ("RED-09/RED-11 surfacing happens via AttestationStatus + future cache columns") but the deferral is **not documented** in 34-CONTEXT.md `<deferred>`. ROADMAP success criterion #8 ("Chutes-routed models display 'freshness valid for enclave lifetime' in the trust UI") is not yet satisfied at the UI layer.

2. **RED-11 three-quote Orchestrated breakdown**: same root cause. `orchestrated_components: Some(vec![(gateway, hex), (model, hex), (compose_manager, hex)])` is populated correctly but dropped at the same actor-loop destructure. RED-11's second clause ("orchestrated-shape models show three-quote verification status") cannot be rendered.

The basic RED-11 row + Verified badge IS verified (preset list flows through UniFFI; AttestationStatus::Verified does propagate). Only the **breakdown** is missing.

**Recommendation:** Either (a) close these gaps now in a small follow-up plan that extends `AttestationStatus` (UniFFI) with `freshness` + `orchestrated_components` sub-fields and wires native UI rendering, or (b) explicitly add them to ROADMAP / REQUIREMENTS as a deferred follow-up phase ("Redpill UI surfacing — orchestrated breakdown + per-enclave freshness label") with traceability rows updated. The executor's choice to ship the cryptographic core fully verified while deferring UI surfacing is **defensible** given the security primitives are correct (the user is protected against attestation bypass; they just don't see the rich breakdown yet) — but it should be made explicit instead of relying on an inline code comment.

### Threat Model Closure (carried forward from executor's report, audited)

| Threat | Disposition | Verified |
|--------|-------------|----------|
| T-34-01 (REPORTDATA layout spoofing) | mitigate | ✓ — 8 RED stubs across all four decoders; byte-slice asserts against golden captures |
| T-34-02 (three-way AND bypass) | mitigate | ✓ — `shape_b_three_way_and_gates_session_open` |
| T-34-03 (Chutes per-enclave freshness misrepresentation) | mitigate | ✓ — `shape_c_client_nonce_not_bound` pins that the client `?nonce=` is NOT bound in REPORTDATA[32..64] for Shape C |
| T-34-04 (TDX debug-mode bit) | mitigate | ✓ — clear-in-all + synthetic-set-rejected |
| T-34-05 (provider preset spoofing — backend) | mitigate | ✓ — preset hard-coded in `known_provider_presets`; `redpill_routes_to_redpill_transport` regression |
| T-34-06 (provider preset spoofing — UI) | mitigate | ✓ — `backend_summary_after_add` + UniFFI single source of truth |
| T-34-07 (stale TDX quote replay) | mitigate | ✓ — per-request nonce in REPORTDATA[32..64] for A/B; 5-min TTL cache |
| T-34-08 (Tinfoil-via-Redpill bypassing direct-Tinfoil) | mitigate | ✓ — `check_model_routable` + HTTP 502 detection + live test |
| T-34-09 (chat sent before attestation verifies) | mitigate | ✓ — `ensure_verified_redpill_attestation` precedes every chat POST and stream POST in all three entry points (chat / streaming / agent) |
| T-34-10 (streaming dispatch skipping attestation gate) | mitigate | ✓ — exhaustive `ProviderTransportKind` match (no wildcard); compile failure if a new transport is added without Redpill arm |
| T-34-11 (UI mislabeling Chutes freshness as per-request) | partial | ⚠️ — discriminator field exists at AttestationEvent level, BUT same drop-at-actor-loop issue means the UI layer cannot CURRENTLY mislabel either way; it just cannot render at all. Risk re-emerges if the deferred surfacing is later wired without preserving the discriminator. |

### Plan Commit Log (carried forward, verified against git log)

| Plan | Commits | Subject |
|------|---------|---------|
| 34-01 | 1181aa5, 038af19, 53f0547 | Wave 0 — fixtures + RED stubs + VALIDATION |
| 34-02 | 3b8ef7f, a12bd07, 95b6dea | Attestation layer + decoders + orchestrator |
| 34-03 | 7a6de21, b8e8727, b1cc106 | LLM transport + preset + Tinfoil refusal |
| 34-04 | 81ea8b3, 49c19cc, 424a852, f77d3f5 | Wiring + AttestationEvent fields + live tests + plan-redpill-tee-attested-aggregator-integration commit |

### Verifier Notes

- The executor's self-authored VERIFICATION.md was thorough and largely accurate. The audit confirms the RED→GREEN tally is honest at the test level.
- The audit's only material disagreement is the **scope of RED-09 and RED-11**: the executor marked both GREEN based on AttestationEvent-level data presence and unit tests on the structs. ROADMAP SC #8 explicitly says "**display in the trust UI**" — that requires data crossing the FFI boundary and rendering in native code. The current actor-loop `_` drop means neither happens. Marking these PARTIAL is the goal-backward-correct verdict.
- All other items pass goal-backward inspection: cryptographic primitives, attestation gate, refusal path, three-way AND, debug-mode rejection, NRAS reuse, single-source-of-truth Venice decoder, no new crates.
- Live integration tests genuinely exist as `#[ignore]`-gated tokio tests (4 of them) — not stubs. They require user sign-off to verify against the live network.

### Status After User Sign-Off

If the live tests pass and the partial gaps are accepted as deferred (with REQUIREMENTS.md updated to reflect "RED-09/RED-11 UI surfacing deferred to phase 35-N"):
- Mark RED-01..RED-08, RED-10 as `[x]` Complete in `.planning/REQUIREMENTS.md`
- Mark RED-09, RED-11 as `[~]` Partial OR open a follow-up plan to close them
- Update Traceability rows accordingly
- Mark Phase 34 as `[x]` Complete in `.planning/ROADMAP.md` only after the partial-gap disposition is decided

---

*Audited: 2026-04-26 — verifier (goal-backward, not blind-trust)*
*Re-verification of executor's 34-VERIFICATION.md*
