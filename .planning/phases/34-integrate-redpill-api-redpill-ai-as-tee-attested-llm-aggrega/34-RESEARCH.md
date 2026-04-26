# Phase 34: Integrate Redpill — Research

**Researched:** 2026-04-26
**Domain:** TEE attestation across an aggregator with three response shapes (Phala-pure / Phala-orchestrated 3-quote / Chutes), Intel TDX + NVIDIA NRAS, integration into existing AttestedProvider pipeline
**Confidence:** HIGH (protocol confirmed by live captures across all four backends in spike 002, including a Tinfoil 502 reproduction; integration shape confirmed by the structural analog in Phase 33 / Venice)

---

## Summary

Phase 34 integrates Redpill (`api.redpill.ai`) as a fourth attested provider after Tinfoil, PPQ.AI, and Venice. Redpill is unique among our providers in that it is an **aggregator**: a single attestation endpoint routes across multiple confidential-compute backends (Phala-pure, Phala/NearAI-orchestrated, Chutes, Tinfoil). Spike 002 captured live wire data from each routed backend and proved that **full client-side TEE verification is feasible at parity with — or stronger than — Venice/ppq for the Phala and Chutes backends**, and that the Tinfoil-routed sub-set is gated by Redpill's own relay limitations (HTTP 502 on `sev-snp-guest/v2`).

**Zero new Rust crates are required.** Every cryptographic operation is reachable with crates already in the recommended stack and on the Cargo.toml after Phase 33: `dcap-qvl` for TDX quote verification, the existing `attestation/nvidia.rs` NRAS path for GPU evidence, `sha3::Keccak256` for the model layout's address binding (already pulled in for Venice), `sha2::Sha256` for compose-manager and Chutes anti-tamper bindings, `base64` for Chutes' base64-encoded quote bytes, and `ed25519-dalek`-style raw byte equality for the gateway layout (no signature verification needed beyond the embedded TDX quote sig).

The novel work is (a) the response-shape dispatcher (`Flat | Orchestrated | Chutes`), (b) three new REPORTDATA layout decoders to add alongside the Venice model decoder we already have, (c) the three-way-AND composition rule for Orchestrated responses, and (d) the freshness UI distinction for Chutes (enclave-baked nonce vs client-fresh nonce in Shapes A/B). Everything else is wiring through the existing AttestedProvider pipeline using Phase 33's structural pattern.

**Primary recommendation:** Add a new `ProviderKind::Redpill` variant + `attestation/redpill.rs` (decoder + cache + verify orchestrator) + `llm/redpill.rs` (HTTP transport + response-shape dispatcher) + native UI preset row, mirroring the four-plan wave structure Phase 33 used for Venice. Reuse `attestation/venice.rs::decode_reportdata_model` (or the equivalent Venice helper) for Shape A and the model component of Shape B. Reuse `attestation/nvidia.rs::fetch_and_verify_nvidia` unchanged.

---

## The Spike Is the Research

The deep research for this phase lives in the spike artifacts; this RESEARCH.md is a thin pointer to avoid duplicating 600 lines of detail. **Mandatory reading for the planner and the executor:**

- `.planning/spikes/002-redpill-tee-verification-research/README.md` — full spike report including REPORTDATA decoders for all four layouts, root-of-trust topology table, comparison to Tinfoil/PPQ/Venice, investigation trail, results, and explicit "what we have to build" section
- `.planning/spikes/002-redpill-tee-verification-research/captures/` — four live wire-data JSONs (one per shape, plus the Tinfoil 502 reproduction) + nonce log + Python decoder script whose assertions translate directly into Rust unit tests
- `.claude/skills/spike-findings-confidential-app/references/redpill-attestation.md` — implementation blueprint extracted from the spike, auto-loaded via `Skill("spike-findings-confidential-app")`. This is the canonical "how to build it" reference.

The CONTEXT.md (`34-CONTEXT.md`) for this phase is also derived from the spike findings and locks the implementation decisions (D-01 through D-24).

---

## Architectural Responsibility Map

| Capability | Primary Tier | Rationale |
|------------|-------------|-----------|
| Attestation fetch + cryptographic verification (TDX quote sig, NRAS JWT, REPORTDATA decode for all four layouts) | Rust core (`attestation/redpill.rs`) | Per project core value: verification belongs in the actor-owned attestation pipeline; native UI never sees raw quotes |
| Response-shape dispatch (Flat / Orchestrated / Chutes) | Rust core (`attestation/redpill.rs`) | Single source of truth; native UI only sees the verified result |
| OpenAI-compatible chat completion request/response shaping | Rust core (`llm/redpill.rs` + async-openai) | Same pattern as `llm/venice.rs` — start from `CreateChatCompletionRequestArgs`, no E2EE wrapper for v1 |
| Provider preset (UI add-backend form) | Rust core (`llm/backend.rs::known_provider_presets`) | UniFFI-exported single source of truth |
| Verification status badge (with three-way breakdown for Orchestrated, freshness sub-line for Chutes) | Rust core sets `AttestationStatus::Verified { freshness, components }`; native UI renders | Status enum already crosses UniFFI |
| TTL-cache verified attestations | Rust core (reuse `attestation/cache.rs` from Phase 33) | Same 5-min TTL pattern |
| Tinfoil-routed model gating (refuse, point to direct-Tinfoil) | Rust core | Single decision point; UI surfaces the user-facing message |

---

## Standard Stack

### Reused (already in Cargo.toml — no version change, no new crate)

| Library | Purpose | Why Standard |
|---------|---------|--------------|
| `dcap-qvl` | TDX quote signature + PCK chain + TCB + CRL verification across all three Redpill shapes | Already in stack; same path used by ppq.ai (Phase 10) and Venice (Phase 33). No modification needed — Redpill quotes are standard DCAP v4 ECDSA-P256, header `04 00 02 00 81 00 00 00`. |
| `reqwest` (rustls-tls) | HTTP fetch of `/v1/attestation/report` and chat completions | Already in stack for every provider |
| `async-openai` | OpenAI-compatible chat completions request/response shaping | Already in stack |
| `serde` / `serde_json` | Parse the three Redpill response shapes; double-parse `nvidia_payload` (JSON-stringified) | Already in stack |
| `sha2` | Compose-manager binding (`reportData[0..32] == actions_hash`); Chutes anti-tamper binding (`SHA256(nonce_str ++ e2e_pubkey_str)`) | Already in stack |
| `sha3` (Keccak256) | Model layout address binding (`keccak256(uncompressed_pubkey[1..65])[12..32] == reportData[0..20]`) | Already in stack since Phase 33 (Venice) |
| `base64` | Chutes ships `intel_quote` as base64; auto-detect via `quote_bytes()` helper | Already in stack |
| `hex` | Parse hex-encoded quotes for Shapes A and B; encode client nonce for the URL | Already in stack |
| `tokio` | Async runtime | Always |
| Existing `attestation/nvidia.rs::fetch_and_verify_nvidia` | NRAS JWT verification — `nvidia_payload` for Shapes A/B (one call), `gpu_evidence[]` for Shape C (one call per GPU) | Built in PPQ phase; reused unchanged for Venice; reused unchanged for Redpill |
| Existing `attestation/cache.rs` | TTL-cache verified attestations | Built in earlier phases |
| Existing `attestation/nonce.rs` | Generate cryptographic nonce | Built in Phase 3 |
| Existing `attestation/venice.rs::decode_reportdata_model` (or equivalent Venice helper) | Model layout decoder — byte-identical between Venice and Redpill | Built in Phase 33 |

### Optional / Not Needed

| Library | Why Not |
|---------|---------|
| `ed25519-dalek` | Gateway layout uses byte equality on the raw pubkey, not signature verification. The signature on the embedded TDX quote is verified by `dcap-qvl`. |
| `viem`-equivalent / on-chain RPC | On-chain DCAP via Automata is deferred (D-deferred section in CONTEXT) |
| `dstack-sdk` | We verify the raw `intel_quote` directly — no need for the Phala dstack envelope |
| Any new HPKE crate | E2EE is deferred to a future phase; HTTPS + TEE attestation is the v1 confidentiality root |

---

## Validation Architecture

(For Nyquist VALIDATION.md — see separate `34-VALIDATION.md`)

**Layer 1 — Unit:** Each REPORTDATA decoder is independently unit-testable against the corresponding live capture in `.planning/spikes/002-redpill-tee-verification-research/captures/`. The Python decoder script's assertions translate directly:

- Model layout decoder ↔ `attestation-phala-pure-raw.json` (Shape A) AND `attestation-phala-raw.json::model_attestations[0]` (Shape B model component)
- Gateway layout decoder ↔ `attestation-phala-raw.json::gateway_attestation`
- Compose-manager layout decoder ↔ `attestation-phala-raw.json::model_attestations[0].compose_manager_attestation`
- Chutes layout decoder ↔ `attestation-chutes-raw.json::all_attestations[*]` (all 4 entries should pass)
- Shape dispatcher ↔ each of the four captured JSONs returns the expected variant
- Tinfoil refusal ↔ `attestation-tinfoil-raw.json` body returns `RedpillError::TinfoilUnsupported`
- `quote_bytes()` helper ↔ both hex (Shapes A/B) and base64 (Shape C) inputs round-trip correctly
- Debug-mode gate ↔ all captured quotes have debug bit clear; a synthetic quote with the bit set must be rejected

**Layer 2 — Integration:** Two `#[ignore]`-gated live tests against `api.redpill.ai`:
- `test_live_redpill_phala_orchestrated()` — fetch + verify against `phala/gpt-oss-120b` (Shape B)
- `test_live_redpill_chutes()` — fetch + verify against `deepseek/deepseek-v3.2` (Shape C)
- Optional: `test_live_redpill_phala_pure()` — fetch + verify against `openai/gpt-oss-20b` (Shape A)

**Layer 3 — End-to-end:** Manual smoke test: add Redpill backend in Settings → run a chat with an Orchestrated model, observe the three-way verified badge. Run a chat with a Chutes model, observe the per-enclave freshness badge. Add a Tinfoil-routed Redpill model, observe the typed error.

---

## Out-of-Scope for This Phase

- E2EE (Chutes HPKE handshake; Phala per-response signing-key wrapper) — defer to a v2 phase if needed
- On-chain DCAP receipts (Automata)
- Intel Trust Authority secondary appraisal
- dstack-deep boot-replay (Docker + QEMU; not mobile-viable)
- Tinfoil-via-Redpill (broken upstream; existing direct-Tinfoil integration unaffected)
- Sigstore golden-values check (audit-receipts feature)
- `/v1/signature/{chatId}` after-the-fact response binding

---

*Phase: 34-integrate-redpill-api-redpill-ai-as-tee-attested-llm-aggrega*
*Research: 2026-04-26 — derived directly from spike-002 findings; minimal new investigation required*
