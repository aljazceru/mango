---
created: 2026-04-25
---

# Spike Manifest

## Idea

Determine whether Venice.ai's TEE-protected models can be integrated into the Confidential App with the same client-side attestation rigor we apply to tinfoil and ppq — i.e., the user can independently verify, on-device, that the model is running inside a genuine Intel TDX enclave with NVIDIA Confidential Computing GPU support, before any user message is sent.

## Requirements

(emergent — populated as spikes surface non-negotiables)

- Client must independently verify the Intel TDX quote signature and PCK certificate chain — never trust Venice's `verified: true` boolean alone.
- Client must independently bind the per-request nonce into REPORTDATA before accepting the attestation.
- Client must verify the secp256k1 signing key is bound into REPORTDATA so subsequent E2EE handshake keys cannot be substituted.
- Where NVIDIA GPU attestation is present, client must POST the `nvidia_payload` to NRAS and verify the response JWT, not trust Venice's NVIDIA verification field.
- For aggregator providers that route across multiple backends (e.g. Redpill), the client must dispatch on response shape and verify all components of multi-quote attestations (e.g. gateway + model + compose-manager) before opening a session.
- Where a backend embeds an enclave-baked nonce instead of the client's nonce (e.g. Redpill→Chutes), the trust UI must downgrade the freshness claim from "per-request" to "per-enclave-instance".

## Spikes

| # | Name | Type | Validates | Verdict | Tags |
|---|------|------|-----------|---------|------|
| 001 | venice-tee-protocol-research | standard | Given Venice public docs + reference CLI + a free unauthenticated probe of the attestation endpoint, when analyzed end-to-end, then we know the exact wire format, REPORTDATA layout, root-of-trust topology, and required client-side checks for parity with tinfoil/ppq | ✓ VALIDATED | venice, tdx, nvidia-cc, attestation, phala-dstack |
| 002 | redpill-tee-verification-research | standard | Given Redpill's open-source verifier + live unauthenticated probes across each routed backend, when analyzed end-to-end, then we know whether full client-side TEE verification is feasible at parity with Venice/ppq across all three Redpill response shapes | ✓ VALIDATED | redpill, tdx, nvidia-cc, attestation, phala-dstack, chutes, near-ai, multi-backend |
