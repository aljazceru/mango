---
phase: 10
plan: 06
type: summary
status: complete
commits:
  - (implementation completed 2026-03-26, pre-GSD workflow)
---

# Plan 10-06 — Test Coverage for PPQ

## What shipped

Comprehensive test coverage for PPQ transport, provider presets, attestation verification, and streaming cancellation.

### Live PPQ attestation tests

Created `rust/src/tests/live_ppq_private.rs`:
- `live_ppq_private_attestation_verifies`: Verifies PPQ attestation with test API key, returns VerifiedPpqAttestation
- `live_ppq_private_rejects_invalid_api_key`: Verifies invalid API keys are rejected with appropriate error
- Uses actual PPQ.AI endpoint for live integration tests
- Tests require PPQ_API_KEY env var for live execution

### PPQ preset tests

Added to `rust/src/tests/backend_config.rs`:
- `test_known_provider_presets_includes_ppq_ai`: Verifies PPQ preset exists with correct id, name, base_url, tee_type, and description
- `test_ppq_ai_supports_tool_use`: Verifies PPQ.AI backend supports tool use with correct configuration
- `test_parse_tee_type_handles_amd_sev_snp`: Verifies parse_tee_type("AmdSevSnp") returns TeeType::AmdSevSnp

### PPQ transport tests

Added to `rust/src/tests/transport.rs`:
- `test_ppq_private_base_url_selects_private_transport`: Verifies PPQ private URL selects PpqPrivateE2ee transport kind
- `test_ppq_private_transport_errors_on_openai_api_base`: Verifies PPQ transport returns error for OpenAI API base path
- `test_ppq_private_model_list_url`: Verifies model list URL returns correct PPQ endpoint

### Migration tests

Added to `rust/src/tests/persistence.rs`:
- `test_migration_v11_seeds_ppq_ai_private_transport`: Verifies V11 updates base_url to https://api.ppq.ai/private/v1/
- Queries ppq-ai row directly and verifies tee_type is "AmdSevSnp"

### Streaming cancellation test

Added to `rust/src/tests/streaming.rs`:
- `test_stop_generation_cancels_ppq_stream`: Documents PPQ coverage of cancellation path
- Tests that StopGeneration + StreamCancelled transitions to Idle for PPQ
- Verifies streaming_text is preserved after cancel for PPQ path

### Test module registration

Updated `rust/src/tests/mod.rs`:
- Added `mod live_ppq_private;` to register new test module

## Tests

All tests pass:
- Live PPQ attestation tests require API key but verify core functionality
- Preset tests verify PPQ configuration
- Transport tests verify routing
- Migration tests verify database state
- Streaming test verifies cancellation path

`cargo test -p mango_core` — full suite green.

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

None. Phase 10 is now complete.
