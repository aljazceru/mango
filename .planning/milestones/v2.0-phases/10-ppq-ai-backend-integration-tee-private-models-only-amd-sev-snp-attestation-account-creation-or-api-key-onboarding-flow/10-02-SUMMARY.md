---
phase: 10
plan: 02
type: summary
status: complete
commits:
  - (implementation completed 2026-03-26, pre-GSD workflow)
---

# Plan 10-02 — Transport Kind and TEE Type Extensions

## What shipped

Extended core type system to support PPQ private transport and AMD SEV-SNP TEE type.

### ProviderTransportKind extension

Added `PpqPrivateE2ee` variant to `ProviderTransportKind` enum in `rust/src/llm/transport.rs`:
- Transport kind detection for PPQ.AI backends via `transport_kind()` method
- `openai_api_base()` returns error explaining PPQ uses provider-specific transport
- `model_list_url()` delegates to `ppq_private::model_list_url`
- `build_http_client()` delegates to `ppq_private::build_http_client`
- Error message: "PPQ private E2EE transport does not use the generic OpenAI client path"

### TeeType extension

Added `AmdSevSnp` variant to `TeeType` enum in `rust/src/llm/backend.rs`:
- New variant for AMD SEV-SNP TEE type
- Updated `parse_tee_type()` in `rust/src/lib.rs` to parse "AmdSevSnp" string
- Attestation dispatch in `rust/src/attestation/task.rs` handles `TeeType::AmdSevSnp`

### ProviderKind extension

Added `Ppq` variant to `ProviderKind` enum in `rust/src/llm/backend.rs`:
- New variant for PPQ.AI provider
- Updated `parse_provider_kind()` to handle "ppq-ai" backend ID

### Provider preset

Added PPQ.AI to `known_provider_presets()` in `rust/src/llm/backend.rs`:
- id: "ppq-ai"
- name: "PPQ.AI"
- base_url: "https://api.ppq.ai/private/v1/"
- tee_type: `TeeType::AmdSevSnp`
- description: "AMD SEV-SNP · Private TEE models"

## Tests

Added test in `rust/src/tests/backend_config.rs`:
- `test_known_provider_presets_includes_ppq_ai`: Verifies PPQ preset exists with correct fields
- `test_parse_tee_type_handles_amd_sev_snp`: Verifies AmdSevSnp parsing

Added test in `rust/src/tests/transport.rs`:
- `test_ppq_private_base_url_selects_private_transport`: Verifies transport kind detection

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Database migrations → Plan 10-03
- Attestation routing → Plan 10-04
- UI integration → Plan 10-05
- Additional tests → Plan 10-06
