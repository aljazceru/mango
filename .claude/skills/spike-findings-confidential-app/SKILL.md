---
name: spike-findings-confidential-app
description: Implementation blueprint from spike experiments. Requirements, proven patterns, and verified knowledge for building confidential-app provider integrations. Auto-loaded during implementation work.
---

<context>
## Project: confidential-app

A multi-platform personal AI platform (RMP architecture: Rust core + native UIs via UniFFI). Users chat with LLMs, run agents, and ground conversations in local RAG — all routed through confidential computing backends with verified TEE attestation. Every inference request is provably confidential: the user can verify via remote attestation that their data never leaves a Trusted Execution Environment.

Spike sessions wrapped: 2026-04-25, 2026-04-26
</context>

<requirements>
## Requirements

Non-negotiable design decisions that emerged from spiking. Every feature reference below must honor these.

- Client must independently verify the Intel TDX quote signature and PCK certificate chain — never trust the provider's `verified: true` boolean alone.
- Client must independently bind the per-request nonce into REPORTDATA before accepting an attestation (where the backend supports a client-supplied nonce).
- Client must verify the per-session signing key is bound into REPORTDATA so subsequent E2EE handshake keys cannot be substituted.
- Where NVIDIA GPU attestation is present, client must POST the GPU evidence payload to NRAS and verify the response JWT, not trust the provider's NVIDIA verification field.
- For aggregator providers that route across multiple backends (e.g. Redpill), the client must dispatch on response shape and verify all components of multi-quote attestations (e.g. gateway + model + compose-manager) before opening a session.
- Where a backend embeds an enclave-baked nonce instead of the client's nonce (Redpill→Chutes), the trust UI must downgrade the freshness claim from "per-request" to "per-enclave-instance".
</requirements>

<findings_index>
## Feature Areas

| Area | Reference | Key Finding |
|------|-----------|-------------|
| Venice.ai TEE attestation | `references/venice-attestation.md` | Public unauthenticated endpoint exposes a raw DCAP v4 TDX quote + NRAS payload; REPORTDATA layout is `[20B addr][12B pad][32B raw nonce]`; verifiable end-to-end with `dcap-qvl` and our existing NRAS path with no new crates |
| Redpill TEE attestation | `references/redpill-attestation.md` | Public unauthenticated `/v1/attestation/report` returns three distinct shapes (Phala-flat / Phala-orchestrated 3-quote / Chutes); model REPORTDATA layout is byte-identical to Venice (decoder reused); orchestrated shape requires three-way AND across gateway+model+compose-manager; Chutes ignores client nonce (enclave-baked); Tinfoil-via-Redpill currently broken upstream — no new crates required |

## Source Files

Original spike source files are preserved in `sources/` for complete reference. Each spike directory contains the live wire captures used as golden fixtures for Rust unit tests, plus a reference Python decoder whose assertions translate directly into test cases.
</findings_index>

<metadata>
## Processed Spikes

- 001-venice-tee-protocol-research
- 002-redpill-tee-verification-research
</metadata>
