---
phase: 33
plan: 04
subsystem: llm-provider-routing
tags: [venice, transport-routing, provider-preset, integration-test]
requires: [33-01, 33-02, 33-03]
provides: [venice-wired-into-router, venice-live-test, phase-33-verification]
affects:
  - rust/src/llm/transport.rs
  - rust/src/llm/backend.rs
  - rust/src/llm/streaming.rs
  - rust/src/agent/loop.rs
  - rust/src/attestation/task.rs
  - rust/src/attestation/endpoint.rs
  - rust/src/tests/venice.rs
  - rust/src/tests/transport.rs
  - rust/src/tests/live_venice.rs
key-files:
  created:
    - .planning/phases/33-integrate-venice-ai-as-tee-attested-llm-provider-with-client/33-VERIFICATION.md
  modified:
    - rust/src/llm/transport.rs
    - rust/src/llm/backend.rs
    - rust/src/llm/streaming.rs
    - rust/src/agent/loop.rs
    - rust/src/attestation/task.rs
    - rust/src/attestation/endpoint.rs
    - rust/src/tests/venice.rs
    - rust/src/tests/transport.rs
    - rust/src/tests/live_venice.rs
decisions:
  - VeniceE2ee transport selected by ProviderKind::Venice (id == "venice-ai") — no base_url substring sniffing like PPQ
  - Venice live tests skip silently (println, not panic) when VENICE_API_KEY unset — never panic in CI even if --ignored gate is bypassed
  - attestation/endpoint.rs returns Unsupported for VeniceE2ee transport (mirrors PPQ/Tinfoil) — D3, no persisted Venice attestation
  - agent/loop.rs converts empty tools Vec to None for Venice (venice::create_chat_completion takes Option<Vec<ChatCompletionTools>>)
metrics:
  duration: ~12min
  tasks: 2
  files: 9
  completed: 2026-04-25
---

# Phase 33 Plan 04: Wire Venice into router/transport/backend Summary

Venice TEE provider wired into the existing routing pipeline; all 7 venice unit tests + 4 attestation_venice unit tests GREEN; full lib suite (350 tests) passes; live integration tests written and `#[ignore]`-gated awaiting user run with `VENICE_API_KEY`.

## Dispatch Arms Added

| File | Function/Site | Arm |
|------|---------------|-----|
| `llm/transport.rs` | `ProviderTransportKind::for_backend` | `ProviderKind::Venice` → `Self::VeniceE2ee` |
| `llm/transport.rs` | `openai_api_base` | `Self::VeniceE2ee` → `Err(unsupported_venice_transport_error())` |
| `llm/transport.rs` | `model_list_url` | `Self::VeniceE2ee` → `super::venice::model_list_url(backend)` |
| `llm/transport.rs` | `build_reqwest_client` | `Self::VeniceE2ee` → `super::venice::build_http_client(timeout)` |
| `llm/backend.rs` | `ProviderKind` enum | `Venice` variant added |
| `llm/backend.rs` | `provider_kind()` | `"venice-ai" => ProviderKind::Venice` |
| `llm/backend.rs` | `known_provider_presets()` | Venice.ai entry: IntelTdx, base_url `https://api.venice.ai/api/v1/`, description "Intel TDX + NVIDIA H100 CC · E2EE chat" |
| `llm/streaming.rs` | `spawn_streaming_task` | `VeniceE2ee` → `venice::run_streaming_chat_completion` |
| `llm/streaming.rs` | `spawn_streaming_task_from_api_messages` | `VeniceE2ee` → `venice::run_streaming_chat_completion_from_api_messages` |
| `agent/loop.rs` | `run_agent_step_for_backend` | `VeniceE2ee` → `venice::create_chat_completion` |
| `attestation/task.rs` | `run_attestation_task` | `ProviderKind::Venice` → `venice::verify_backend_attestation(&policy.tdx)` |
| `attestation/endpoint.rs` | `verify_attestation_endpoint` | `ProviderKind::Venice` w/ `VeniceE2ee` transport → `Unsupported` (no persisted attestation) |

## RED → GREEN Final Tally

| Test | Before | After |
|------|--------|-------|
| `tests::venice::venice_preset_present` (VEN-01) | RED #[ignore] | GREEN |
| `tests::venice::backend_summary_after_add` (VEN-09) | RED #[ignore] | GREEN |
| `tests::transport::venice_routes_to_venice_e2ee` | absent | GREEN (added Plan 04) |
| `tests::live_venice::live_venice_attestation_verifies` | RED #[ignore] panic stub | IMPL #[ignore]-gated, awaits VENICE_API_KEY |
| `tests::live_venice::live_venice_chat_completion_e2ee` | absent | IMPL #[ignore]-gated, awaits VENICE_API_KEY |

Other VEN tests (VEN-02, VEN-04a..d, VEN-05, VEN-07a/b, VEN-08) were already GREEN from Plan 02/03.
`tests::attestation_venice::tdx_verify_golden_capture_signature` (VEN-03) and `tests::attestation_venice::tdx_debug_bit_rejected` (VEN-06) remain `#[ignore]` (covered by live VEN-LIVE).

## Full-Suite Result

```
test result: ok. 350 passed; 0 failed; 14 ignored; 0 measured; 0 filtered out; finished in 15.10s
```

`cargo build -p mango_core` exits 0 with only pre-existing dead_code warnings.

## Manual Live Test Command

User runs after sign-off:

```bash
VENICE_API_KEY=<key> cargo test -p mango_core --lib live_venice -- --ignored --nocapture
```

Expected: both tests pass; `[live-venice] decrypted reply: …` line shows non-empty plaintext after E2EE round-trip.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Plan referenced non-existent `crate::config::BackendConfig` and `preferred_model` field**
- **Found during:** Task 2 (live_venice.rs implementation)
- **Issue:** Plan pseudocode used `crate::config::{BackendConfig, TeeType}` and a `preferred_model: Option<String>` field that doesn't exist on `BackendConfig`.
- **Fix:** Used `crate::llm::{BackendConfig, TeeType}` and constructed `BackendConfig` with the fields actually present (`max_concurrent_requests`, `supports_tool_use`, no `preferred_model`). Mirrored the `live_ppq_private.rs` shape for consistency.
- **Files modified:** `rust/src/tests/live_venice.rs`

**2. [Rule 2 - Critical functionality] Plan only listed `router.rs` for dispatch, but actual dispatch sites are spread across `streaming.rs`, `agent/loop.rs`, `attestation/task.rs`, `attestation/endpoint.rs`**
- **Found during:** Task 1 (read_first scan)
- **Issue:** `rust/src/llm/router.rs` is a `FailoverRouter` health/selection module; transport-kind dispatch happens in `streaming.rs::spawn_streaming_task[_from_api_messages]`, `agent/loop.rs::run_agent_step_for_backend`, `attestation/task.rs::run_attestation_task`, and `attestation/endpoint.rs::verify_attestation_endpoint`. Skipping any site would silently route Venice traffic through the OpenAI-compatible path, bypassing E2EE (T-33-16).
- **Fix:** Added `VeniceE2ee` arms to every match site. Verified by exhaustive enum match (compile-time enforcement).
- **Files modified:** all four files above.

**3. [Rule 2 - Critical functionality] agent/loop.rs `tools: Vec<ChatCompletionTools>` vs venice api `Option<Vec<…>>`**
- **Found during:** Task 1 build
- **Issue:** `crate::llm::venice::create_chat_completion` takes `Option<Vec<ChatCompletionTools>>` (parity with ppq_private), but agent loop has `Vec<>`.
- **Fix:** Convert empty Vec to None at the call site so Venice receives the same "no tools" signal as PPQ would for an empty tool list.
- **Files modified:** `rust/src/agent/loop.rs`

## Threat Closure

T-33-06, T-33-15, T-33-16 mitigated as documented in plan threat_model and verified in `33-VERIFICATION.md`. No new threat surface introduced beyond what was already analyzed in Plans 02/03.

## Self-Check: PASSED

- File `rust/src/tests/live_venice.rs` exists.
- File `.planning/phases/33-.../33-VERIFICATION.md` exists.
- Commit `806913e` (Task 1) present in git log.
- Commit `c2f45c4` (Task 2) present in git log.
- 350/0/14 lib test result confirmed in `/tmp/phase33-final.log`.
