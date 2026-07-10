---
phase: 10
plan: 01
type: summary
status: complete
commits:
  - (implementation completed 2026-03-26, pre-GSD workflow)
---

# Plan 10-01 — Core PPQ Private Transport Module

## What shipped

Complete PPQ private transport module with EHBP encryption, AMD SEV-SNP attestation verification, and SSE streaming support.

### Module structure

Created `rust/src/llm/ppq_private.rs` (1264 lines) with:
- EHBP protocol constants and key lengths
- `VerifiedPpqAttestation` struct with zeroization for caching
- `AttestationBundle`, `AttestationDoc`, `ProblemDetails` structs for parsing PPQ responses
- HPKE encryption using X25519HkdfSha256 KEM, AesGcm256 AEAD, HkdfSha256 KDF
- Request/response nonce handling with 32-byte nonces
- AES-256-GCM encryption for request bodies
- HKDF key derivation for response keys and nonces
- AMD SEV-SNP report parsing and verification using `sev` crate
- X.509 enclave certificate parsing using `x509-parser`
- VCEK extraction and validation
- Attestation caching with TTL using Lazy<Mutex<HashMap>>
- Error types for PPQ-specific failures

### Functions implemented

- `create_chat_completion`: Non-streaming chat completion with HPKE encryption
- `run_streaming_chat_completion`: SSE streaming from async-openai messages
- `run_streaming_chat_completion_from_api_messages`: SSE streaming from API messages
- `verify_backend_attestation`: AMD SEV-SNP attestation verification
- `build_http_client`: HTTP client with PPQ-specific configuration
- `model_list_url`: PPQ model list endpoint
- Helper functions for encryption, decryption, nonce handling, and error formatting

### Security features

- All sensitive materials zeroized using `zeroize` crate
- HPKE key encapsulation with forward secrecy
- Per-request nonces bound to attestation reports
- Response key derivation with unique labels
- Encrypted response chunks with sequence numbers
- Attestation caching with TTL to prevent replay attacks

## Tests

Live PPQ attestation tests created in `rust/src/tests/live_ppq_private.rs`:
- `live_ppq_private_attestation_verifies`: Verifies PPQ attestation with test API key
- `live_ppq_private_rejects_invalid_api_key`: Rejects invalid API keys

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None - implementation matched design.

## Out of scope (handed off)

- Transport kind integration → Plan 10-02
- TEE type integration → Plan 10-02
- Database migrations → Plan 10-03
- Attestation routing → Plan 10-04
- UI integration → Plan 10-05
- Additional tests → Plan 10-06
