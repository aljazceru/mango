# Spike Wrap-Up Summary

**Dates:** 2026-04-25, 2026-04-26
**Spikes processed:** 2
**Feature areas:** venice-attestation, redpill-attestation
**Skill output:** `./.claude/skills/spike-findings-confidential-app/`

## Processed Spikes

| # | Name | Type | Verdict | Feature Area |
|---|------|------|---------|--------------|
| 001 | venice-tee-protocol-research | standard | ✓ VALIDATED | venice-attestation |
| 002 | redpill-tee-verification-research | standard | ✓ VALIDATED | redpill-attestation |

## Key Findings — Venice (spike 001)

- **Venice TEE integration is feasible at parity with ppq.** Public unauthenticated `GET /api/v1/tee/attestation?model=…&nonce=…` returns a raw DCAP v4 TDX quote + NRAS GPU payload. No API key required to attest.
- **REPORTDATA layout (definitive, from live capture):** `[0..20] = signing_address` (Ethereum-style keccak of secp256k1 pubkey), `[20..32] = zero pad`, `[32..64] = the raw 32-byte client nonce`. Server confirms `nonceBinding.method: "raw"`.
- **Substrate:** Phala dstack on Intel TDX + 1× NVIDIA-CC GPU (`tee_provider: "phala"`, `tee_hardware: "intel-tdx"`, image `dstack-nvidia-dev-0.5.5`). The TDX quote can be verified against Intel PCS directly; we don't need any Phala-specific tooling.
- **Reference impl gap:** `veniceai/venice-cli` does only structural quote parsing and trusts the server's `server_verification.tdx.valid` boolean. Our Rust implementation will be the first OpenAI-compatible Venice client to do full client-side cryptographic verification.
- **No new crates required.** `dcap-qvl` (already on the recommended stack for ppq's TDX path) handles the quote; the existing `rust/src/attestation/nvidia.rs` path handles the NRAS payload; `sha3` + `k256`/`secp256k1` for the REPORTDATA address binding and the E2EE handshake.
- **Watch-outs for the build:** `signing_key` vs `signing_public_key` field-name fallback; `nvidia_payload` is a JSON-encoded **string** (double parse); reject TDX debug mode (`td_attributes[0] & 0x01`); use `enable_e2ee: true` on `/chat/completions` (Responses API does not support E2EE).
- **Originally proposed spikes 002 (live probe) and 003 (Rust verify) were dropped** — 002 was completed inside 001 once the endpoint was found to be unauthenticated; 003 has no remaining unknowns and is now an implementation task.

## Key Findings — Redpill (spike 002)

- **Redpill TEE integration is feasible at parity with — or stronger than — Venice/ppq.** Public unauthenticated `GET https://api.redpill.ai/v1/attestation/report?model=…&nonce=…`. No API key required.
- **Three response shapes** dispatched by routing: **Flat** (Phala-pure, Venice-identical), **Orchestrated** (Phala/NearAI: gateway + model + compose-manager — three TDX quotes per request), **Chutes** (`attestation_type: chutes`, base64-encoded quotes, anti-tamper hash binding). A fourth Tinfoil-routed path is currently broken at Redpill's relay (`Unsupported Tinfoil attestation format: sev-snp-guest/v2`) — keep using direct-Tinfoil integration.
- **Model REPORTDATA layout is byte-identical to Venice.** Reuse the spike-001 decoder verbatim for Shape A and the model component of Shape B. Three additional small layouts are needed: ed25519-gateway (`[32B pubkey][32B nonce]`), sha256-compose-manager (`[32B actions_hash][32B nonce]`), chutes-anti-tamper (`SHA256(nonce_str ++ e2e_pubkey_str)` over Chutes' enclave-baked nonce).
- **Chutes does NOT honor the client's `?nonce=`.** It embeds an enclave-baked nonce instead, so freshness is bounded by enclave lifetime, not per-request. Trust UI must reflect this distinction for Chutes-routed models.
- **All quotes are TDX v4 ECDSA-P256** (header `04 00 02 00 81 00 00 00`). `dcap-qvl` parses every shape with no modification. Quote sizes ~5006 bytes; orchestrated requests verify three quotes (~tens of ms each).
- **Detect base64-vs-hex on the quote bytes** — Chutes ships base64, others ship hex. One-line auto-detect (mirrors `redpill-verifier::toHexQuote`).
- **Replicate Chutes' debug-mode gate** — `td_attributes[0] & 1 == 0` at quote body offset 120; the reference verifier flags debug as CRITICAL.
- **Reference `redpill-verifier` Light Mode delegates TDX checks to Phala's hosted API** (`cloud-api.phala.network/api/v1/attestations/verify`). We will be strictly stronger by re-verifying locally with `dcap-qvl`.
- **No new Rust crates required.** `dcap-qvl` + existing `nvidia.rs` + `sha2` + `sha3` cover all four REPORTDATA layouts and the Chutes anti-tamper binding.
- **Compose-manager attestation is a free sovereignty win** — binds an append-only orchestration ledger into the enclave; surface in trust UI as "model image last published by commit X" without extra crypto.
- **Optional secondary verifiers** (Automata on-chain DCAP, Intel Trust Authority, Sigstore golden values, dstack-deep boot replay) are interesting for an audit-receipts feature but **not required** on the critical path. Defer to v2.
