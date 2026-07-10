---
phase: 19
plan: 01
type: summary
status: complete
commits:
  - (implementation completed 2026-03-29, pre-GSD workflow)
---

# Plan 19-01 — Test Coverage Gaps Implementation

## What shipped

Unit tests for three coverage gaps: streaming cancellation, agent failure injection, and attestation cache TTL behavior.

### Streaming cancellation tests (TEST-01)

Added to `rust/src/tests/streaming.rs`:
- `test_stop_generation_cancels_active_stream`: Verifies Tinfoil HPKE transport cancellation path
  - Injects StreamChunk to simulate in-progress stream
  - Dispatches StopGeneration
  - Simulates StreamCancelled event from transport
  - Verifies BusyState::Idle transition
  - Verifies streaming_text preserved after cancel
- `test_stop_generation_cancels_ppq_stream`: Verifies PPQ AES-GCM transport cancellation path
  - Documents PPQ uses same StreamCancelled event as Tinfoil
  - Verifies StopGeneration + StreamCancelled → Idle for PPQ
  - Verifies streaming_text preserved after cancel for PPQ

### Agent failure injection tests (TEST-02)

Added to `rust/src/tests/agent.rs`:
- `test_agent_max_step_enforcement`: Verifies 20-step limit at persistence layer
  - Seeds 20 agent steps in-memory
  - Verifies count equals 20
  - Verifies count >= 20 condition triggers session failure
  - Simulates update_agent_session_status("failed")
  - Verifies exact DB state actor relies on for termination decision

### Attestation cache TTL tests (TEST-03)

Added to `rust/src/tests/attestation_cache.rs`:
- `test_get_latest_for_backend_expiry`: Verifies get_latest_for_backend rejects expired entries
  - Creates expired entry (expires_at = now - 1)
  - Calls get_latest_for_backend
  - Asserts returns None for expired entry
- `test_get_raw_report_bypasses_ttl`: Verifies get_raw_report bypasses TTL by design
  - Creates expired entry (expires_at = now - 1)
  - Calls get_raw_report
  - Asserts returns blob even when TTL expired
  - Asserts raw report bytes match stored value

## Tests

All tests pass:
- `test_stop_generation_cancels_active_stream` — passing
- `test_stop_generation_cancels_ppq_stream` — passing
- `test_agent_max_step_enforcement` — passing
- `test_get_latest_for_backend_expiry` — passing
- `test_get_raw_report_bypasses_ttl` — passing

`cargo test -p mango_core` — full suite green.

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None - implementation matched CONTEXT.md specification.

## Out of scope (handed off)

None. Phase 19 is now complete.
