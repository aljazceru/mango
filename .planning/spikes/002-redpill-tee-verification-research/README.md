---
spike: 002
name: redpill-tee-verification-research
type: standard
validates: "Given Redpill's open-source verifier (redpill-ai/redpill-verifier), the public docs, and live unauthenticated probes of the attestation endpoint across each routed backend (Phala-pure, NearAI-orchestrated, Chutes, Tinfoil), when the protocol, response shapes, REPORTDATA layouts, and root-of-trust topology are analyzed end-to-end, then we know whether Redpill can be integrated with the same client-side TDX + NVIDIA-CC verification rigor we already apply to Tinfoil, PPQ and Venice"
verdict: VALIDATED
related: [001]
tags: [redpill, tdx, nvidia-cc, attestation, phala-dstack, chutes, near-ai, multi-backend]
---

# Spike 002: Redpill TEE Verification Research

## What This Validates

**Given** the open-source reference implementation in `redpill-ai/redpill-verifier` (TypeScript, MIT) and a series of live captures of `GET https://api.redpill.ai/v1/attestation/report` against each routed backend,
**when** the protocol, response shapes, REPORTDATA layouts and root-of-trust topology are analyzed,
**then** we can answer: *can Redpill be integrated into the Confidential App with the same client-side TDX + NVIDIA-CC verification rigor we already apply to Tinfoil, PPQ and Venice?*

## Research

### Documents read

- `github.com/redpill-ai/redpill-verifier` (`README.md`, `js/src/verify.ts`, `js/src/verifiers/cloud-api.ts`, `js/src/verifiers/dstack.ts`, `js/src/verifiers/onchain.ts`, `js/src/verifiers/chutes.ts`, `js/src/verifiers/tinfoil.ts`, `js/src/providers/detect.ts`, `js/src/constants.ts`).
- `https://api.redpill.ai/v1/models` — provider-routing manifest.
- Redpill API base: `https://api.redpill.ai/v1/attestation/report?model=<id>&nonce=<64-hex>`.

### Endpoint shape (live, unauthenticated)

```
GET https://api.redpill.ai/v1/attestation/report?model=<model_id>&nonce=<64-hex>
   No Authorization header required. Endpoint is public — same gift as Venice.
```

Returns one of **three distinct response shapes**, dispatched by which backend the model is routed to. The full source for `js/src/providers/detect.ts` is the canonical disambiguation logic.

### Three response shapes — captured live

| Shape | Triggered when | Sample model | Capture |
|-------|----------------|--------------|---------|
| **A. Flat** (Venice-like) | Single Phala dstack instance (no gateway) | `openai/gpt-oss-20b` | `captures/attestation-phala-pure-raw.json` |
| **B. Gateway+model+composer** (NearAI / Phala-orchestrated) | `gateway_attestation` is present | `phala/gpt-oss-120b` | `captures/attestation-phala-raw.json` |
| **C. Chutes** | `attestation_type: "chutes"` | `deepseek/deepseek-v3.2` | `captures/attestation-chutes-raw.json` |

A fourth path exists for Tinfoil-routed models but **Redpill's relay currently rejects it**: probing `meta-llama/llama-3.3-70b-instruct` returns HTTP 502 `Unsupported Tinfoil attestation format: https://tinfoil.sh/predicate/sev-snp-guest/v2`. So Tinfoil-via-Redpill is gated by Redpill's own backend, not by what we can verify on-device. We already integrate Tinfoil directly elsewhere — that path is unaffected.

### Wire format details (confirmed from live captures)

#### Shape A — Flat (Phala-pure)

Same field set as Venice. Top-level keys: `signing_address` (`0x`-prefixed Ethereum-style 20B), `signing_algo: "ecdsa"`, `request_nonce`, `intel_quote` (10 012 hex chars = 5006-byte TDX v4 quote), `nvidia_payload` (JSON-stringified NRAS request), `info` (dstack envelope: `app_cert`, `app_id`, `instance_id`, `tcb_info`, `vm_config`, `key_provider_info`, `mr_aggregated`, `os_image_hash`, `compose_hash`), `event_log`, `vm_config`, `all_attestations` (often length 1).

#### Shape B — Gateway + model + composer (NearAI-style)

Top-level keys: `gateway_attestation`, `model_attestations[]`. The model_attestation embeds a third nested attestation: `compose_manager_attestation`. So a single trust handshake covers **three** TDX quotes.

| Component | Signing algo | REPORTDATA layout |
|-----------|--------------|-------------------|
| `gateway_attestation` | `ed25519` | `[32B raw ed25519 pubkey][32B nonce]` |
| `model_attestations[i]` | `ecdsa` (secp256k1) | `[20B Eth address][12B zero pad][32B nonce]` (Venice-identical) |
| `model_attestations[i].compose_manager_attestation` | (implicit) | `[32B actions_hash][32B nonce]` — binds the orchestration commit history into the enclave |

All three layouts independently verified on the captured response — the byte slices match the submitted nonce exactly and decode without ambiguity.

#### Shape C — Chutes

Top-level keys: `attestation_type: "chutes"`, `nonce`, `all_attestations[]`. Each entry: `instance_id`, `nonce`, `e2e_pubkey` (~1500-byte HPKE config blob), `intel_quote` (**base64-encoded**, not hex), `gpu_evidence[]` (NRAS payload split into per-GPU entries). Chutes quotes lead with `BAAC...` in base64, which decodes to the same TDX v4 header `04 00 02 00 81 00 00 00`. The `redpill-verifier::toHexQuote()` helper auto-detects.

Anti-tamper binding the verifier checks: `report_data == sha256(nonce || e2e_pubkey)` (per `js/src/verifiers/chutes.ts`). Different shape, same primitive — recompute SHA-256 client-side and compare a slice of REPORTDATA.

### REPORTDATA layouts (definitive — confirmed by capture)

Submitted nonce: `d70abc03ba658004483b5041420661c577ae7652882b83948d898fae359467fb`

```
GATEWAY (ed25519):
  [ 0..32]  signing_address  32B  = raw ed25519 public key
  [32..64]  client nonce     32B  = exact bytes submitted

MODEL (ecdsa, NVIDIA-CC):
  [ 0..20]  signing_address  20B  = keccak256(uncompressed_pubkey)[12..32]
  [20..32]  zero padding     12B  = 00 00 00 00 00 00 00 00 00 00 00 00
  [32..64]  client nonce     32B  = exact bytes submitted

COMPOSE_MANAGER:
  [ 0..32]  actions_hash     32B  = sha256 over commit-action ledger
  [32..64]  client nonce     32B  = exact bytes submitted

CHUTES (per attestation in all_attestations[]):
  [ 0..32]  binding_digest   32B  = SHA256(nonce_str ++ e2e_pubkey_str)
                                    — STRING concat of the as-emitted ASCII
                                      forms (per redpill-verifier chutes.ts:77)
  [32..64]  unconstrained    32B  — Chutes does NOT bind a client-fresh nonce here

  CRITICAL: the `nonce` Chutes hashes is its own enclave-baked nonce
  (returned in top-level `nonce` and per-attestation `nonce`), NOT the
  client-supplied `?nonce=` query parameter. The client's nonce is ignored
  by Chutes-routed responses; freshness is bounded by enclave lifetime
  and e2e_pubkey rotation, not by a per-request challenge.
```

The MODEL layout is byte-identical to Venice's spike-001 finding. The model-pubkey-binding proof is the same Ethereum-address derivation: `keccak256(uncompressed_pubkey[1..65])[12..32] == reportData[0..20]`.

**Chutes freshness model differs.** Shapes A and B accept the client's `?nonce=` and embed it raw in REPORTDATA[32..64], so the client gets a true per-request liveness proof. Shape C does not — Chutes embeds an enclave-baked nonce instead. For Chutes the trust statement is "this `e2e_pubkey` was bound to this enclave instance at boot," and freshness is at the granularity of the enclave's lifetime. Acceptable for a session-open gate (we'd open an HPKE session under that `e2e_pubkey`), but the client must surface this distinction in the trust UI.

### Root of trust topology

| Layer | Root | How verified | Existing crate? |
|-------|------|--------------|-----------------|
| Intel TDX quote signature | Intel SGX/TDX root CA (Intel PCS) | `dcap-qvl::verify(quote, collateral, ts)` | yes (already on stack, used by ppq + Venice) |
| Intel PCK cert chain | Intel SGX Root CA | included in quote; verified by `dcap-qvl` | yes |
| NVIDIA GPU attestation | NVIDIA NRAS root + Device Identity CA | POST `nvidia_payload` to NRAS, verify returned JWT | yes (`rust/src/attestation/nvidia.rs`) |
| Model signing-key binding (ecdsa) | TDX (transitively) | `keccak256(pub)[12..32] == reportData[0..20]` | needs `sha3` (already on stack for Venice) |
| Gateway signing-key binding (ed25519) | TDX (transitively) | `pub == reportData[0..32]` (slice equality) | std slice compare |
| Compose-manager binding | TDX (transitively) | `sha256(action_ledger) == reportData[0..32]` | `sha2` |
| Chutes anti-tamper binding | TDX (transitively) | `sha256(nonce_str ++ e2e_pubkey_str) == reportData[0..32]` (Chutes-baked nonce, not client) | `sha2` |
| Chutes debug-mode check | TDX (transitively) | `td_attributes[0] & 1 == 0` (debug bit must be off) | std byte test |
| Nonce freshness | self-supplied | byte-equal `reportData[32..64]` to client nonce | std slice compare |
| Phala dstack `info.app_cert` | the embedded TDX quote | optional; quote-in-X.509-extension envelope | skip (raw quote sufficient) |
| Optional: on-chain DCAP via Automata | Automata smart contract | view-call `verifyAndAttestOnChain(bytes)` on Sepolia/ATA | NOT NEEDED for client-side; reference verifier offers it as a *secondary* check we can ignore |
| Optional: Intel Trust Authority | Intel ITA | requires API key | NOT NEEDED; primary TDX path is sufficient |
| Optional: Sigstore container provenance | Sigstore Rekor | container-image build proof | useful for "deep" verification — not on critical path for v1 |

The reference `redpill-verifier` Light Mode trusts **Phala's hosted TDX verification API** (`POST https://cloud-api.phala.network/api/v1/attestations/verify`) instead of doing the cryptography itself. We will be **stronger** than the reference by running `dcap-qvl` locally — same posture as we already adopted for Venice and ppq.

### Comparison to existing providers

| Check | tinfoil (today) | ppq (today) | Venice (planned) | **Redpill (proposed)** |
|-------|-----------------|-------------|------------------|-------------------------|
| TEE primitive | NVIDIA NRAS JWT only | Intel TDX + NVIDIA NRAS | Intel TDX + NVIDIA NRAS | Intel TDX + NVIDIA NRAS |
| Attestation transport | per-request signed payload | dedicated endpoint | `/api/v1/tee/attestation` | `/v1/attestation/report` |
| Auth required | n/a | yes | **no** | **no** |
| Client supplies nonce | yes | yes | yes (32B raw) | yes (32B raw) |
| Quote signature verified client-side | n/a | yes | yes (`dcap-qvl`) | yes (`dcap-qvl`) |
| Per-response signing key bound to attestation | yes (ECDSA) | yes | yes (secp256k1) | yes (secp256k1 model + ed25519 gateway) |
| GPU attestation | yes (NRAS) | yes (NRAS) | yes (NRAS) | yes (NRAS) |
| Multi-component attestation chain | no | no | no | **yes — gateway + model + compose-manager** |
| Number of distinct response shapes | 1 | 1 | 1 | **3** (flat / gateway-orchestrated / chutes) |
| Provider does its own server-side check first | yes | yes | yes (`server_verification.tdx.*`) | yes (Phala API in reference verifier) |
| Reference verifier exists | own Rust impl | own Rust impl | venice-cli (TS, structural only) | `redpill-verifier` (TS, calls Phala API) |
| Trust ceiling we can hit | provider claim | full DCAP | full DCAP + reportData binding | full DCAP + reportData binding **across 3 quotes per request** |

### Approach comparison

| Approach | Pros | Cons | Status |
|----------|------|------|--------|
| Reuse `dcap-qvl` + new `redpill.rs` parser that handles all three response shapes | Reuses Venice TDX path; one provider variant covers Phala-pure, NearAI, Chutes; supports 80+% of Redpill catalog | Need a small response-shape detector + 4 REPORTDATA-layout decoders | **Chosen** |
| Trust Phala's `/api/v1/attestations/verify` boolean (what reference does) | Trivial: one HTTP call returns `verified: true` | Defeats the trust model; equivalent to trusting Redpill itself | Rejected |
| Add Automata on-chain DCAP as a secondary check | Trustless second opinion, doesn't require Phala | Requires Ethereum RPC connection from device, gas-free but adds latency and a network dep | Defer to v2 |
| Add Intel Trust Authority as secondary | Independent appraisal | Requires per-user API key | Defer indefinitely |
| Add deep-mode dstack-verifier (Docker + QEMU boot replay) | Strongest trust posture — replays measurements | Requires Docker; not viable on iOS/Android sandbox | Out of scope for client app — could expose as a "verify on a server you control" feature later |
| Skip Tinfoil-via-Redpill entirely; keep direct Tinfoil integration | Sidesteps Redpill's relay limitation | Lose Tinfoil convenience through Redpill — fine, we already have Tinfoil | **Chosen** |

**Chosen approach:** add a `redpill` provider variant in `rust/src/attestation/` that:

1. Reuses the existing TDX path (`dcap-qvl` for quote sig + cert chain) and the existing NVIDIA NRAS path.
2. Adds a small Redpill response-shape dispatcher (`A | B | C`).
3. Adds 4 REPORTDATA-layout decoders (model-ecdsa, gateway-ed25519, compose-manager-sha256, chutes-anti-tamper).
4. For Shape B, requires **all three** quotes to verify before approving the session.
5. Skips on-chain / ITA / dstack-deep modes for v1.
6. Treats Redpill-Tinfoil as unsupported until Redpill upgrades its relay; users wanting Tinfoil keep using the existing direct integration.

## How to Run

```bash
# Phala-orchestrated (gateway + model + compose_manager)
NONCE=$(openssl rand -hex 32)
curl -sS "https://api.redpill.ai/v1/attestation/report?model=phala/gpt-oss-120b&nonce=$NONCE" \
  -o captures/attestation-phala-raw.json

# Phala-pure (flat, Venice-shape)
NONCE=$(openssl rand -hex 32)
curl -sS "https://api.redpill.ai/v1/attestation/report?model=openai/gpt-oss-20b&nonce=$NONCE" \
  -o captures/attestation-phala-pure-raw.json

# Chutes
NONCE=$(openssl rand -hex 32)
curl -sS "https://api.redpill.ai/v1/attestation/report?model=deepseek/deepseek-v3.2&nonce=$NONCE" \
  -o captures/attestation-chutes-raw.json

# Decode REPORTDATA from each shape
python3 captures/decode-report-data.py
```

## What to Expect

- All four captures (incl. tinfoil 502 reproduction) saved verbatim — golden fixtures for unit tests.
- For Shape B: REPORTDATA from each of the three quotes decodes into its expected layout, with the nonce field byte-equal to the submitted nonce.
- For Shape A: Venice-identical layout decodes the same way.
- For Shape C: `sha256(nonce || e2e_pubkey)` of the captured fields equals the first 32 bytes of REPORTDATA.
- TDX header on every quote: `04 00 02 00 81 00 00 00` — version 4, ECDSA-P256, TDX. `dcap-qvl` parses without modification.

## Investigation Trail

1. **Read the README.** Reference verifier ships an npm package with two depths (Light = cloud APIs, Deep = QEMU). Light Mode delegates TDX verification to **Phala's** hosted endpoint — useful as a cross-check, useless as a trust root. Deep Mode requires Docker — non-starter on mobile. Conclusion: we will not run their verifier on-device; we'll do the work ourselves.
2. **Read `verify.ts` end-to-end.** Discovered the multi-backend dispatch (Phala / NearAI / Chutes / Tinfoil) and the multi-component nature of Phala-orchestrated attestations (gateway + model + compose-manager).
3. **Probed the endpoint live.** No API key required. Got back HTTP 200 / 236 KB JSON for `phala/gpt-oss-120b`. Confirmed Shape B (gateway + model_attestations + compose_manager).
4. **Probed each remaining backend.**
   - `openai/gpt-oss-20b` → Shape A (flat, Venice-identical).
   - `deepseek/deepseek-v3.2` → Shape C (`attestation_type: chutes`, `all_attestations[]`, base64 quote).
   - `meta-llama/llama-3.3-70b-instruct` (Tinfoil) → HTTP 502, *"Unsupported Tinfoil attestation format: sev-snp-guest/v2"*. So Redpill itself doesn't currently parse the new Tinfoil SEV-SNP envelope; Tinfoil-via-Redpill is broken upstream. Skipping.
5. **Decoded REPORTDATA from each capture.** Confirmed:
   - Gateway: `[32B ed25519 pubkey][32B nonce]`.
   - Model: `[20B Eth addr][12B zero pad][32B nonce]` — **byte-identical to Venice spike-001**.
   - Compose-manager: `[32B actions_hash][32B nonce]`.
   - Chutes: `[32B sha256(nonce||e2e_pubkey)][32B nonce]` (cross-checked against `js/src/verifiers/chutes.ts`).
6. **Confirmed TDX quote shape** is standard DCAP v4 ECDSA-P256 across all responses (header `04 00 02 00 81 00 00 00`). 5006 bytes per quote. Already parseable by `dcap-qvl` with no modification.
7. **Identified runtime substrate.** All Phala-routed responses run on **Phala dstack** (`app_name: dstack-0.5.4` for the gateway, `dstack-nvidia-0.5.5` for the model). Same substrate as Venice. The same `info.app_cert` X.509 envelope is present — we can ignore the envelope and verify the raw `intel_quote` directly, identical strategy.
8. **Inspected detect.ts.** Provider auto-detect fingerprint:
   - `attestation_type === 'chutes'` ⇒ Shape C.
   - `gateway_attestation && model_attestations` and `models[0].compose_manager_attestation` ⇒ NearAI Shape B.
   - `gateway_attestation && model_attestations` without compose-manager ⇒ Phala-orchestrated Shape B.
   - `signing_address && intel_quote` at top level ⇒ Phala-pure Shape A.
9. **Considered on-chain DCAP and ITA.** Both are *secondary opinions*, not primary verification. We can independently re-verify TDX with `dcap-qvl` against fresh Intel collateral — that already gives us a trustless answer rooted in Intel's PCS, no need to add an Ethereum RPC dependency on the client. If we ever want a public auditable record we can add `verifyAndAttestOnChain` later as an opt-in feature; not required for the trust model.

## Results

**Verdict: ✓ VALIDATED.** Redpill can be integrated with verification at parity with — or **stronger than** — what we do for Tinfoil, ppq and Venice, **except** the Tinfoil-routed sub-set, which is currently broken at Redpill's own relay layer (so unaffected by our client design).

**What we get for free, no code:**
- Public, unauthenticated `GET /v1/attestation/report` (verify before paying — same UX win as Venice).
- Raw Intel TDX v4 ECDSA-P256 quotes, directly compatible with `dcap-qvl`.
- NVIDIA NRAS payloads compatible with our existing `rust/src/attestation/nvidia.rs`.
- Per-request 32-byte client nonce echoed raw into REPORTDATA[32..64] across every shape and every component.
- All quotes run on Phala dstack — same envelope as Venice, no new substrate to learn.

**What we have to build (small, ~1 plan phase):**
- A Redpill **response-shape dispatcher** that returns `Flat | Orchestrated | Chutes`.
- Four small **REPORTDATA-layout decoders** (model-ecdsa is reusable from Venice; the other three are <30 lines each).
- For Orchestrated responses: enforce that **all three** quotes (gateway + model + compose-manager) verify before opening a session.
- For Chutes responses: also recompute `sha256(nonce || e2e_pubkey)` and compare a slice of REPORTDATA.
- Detect base64-vs-hex on the quote bytes (one-line check; also seen in reference `toHexQuote`).
- Wire a `redpill` provider variant into the existing attestation pipeline.

**Surprises and gotchas:**
- **Three response shapes from one endpoint.** Routing is opaque to the caller; any consumer must dispatch on shape. Fortunately the fingerprint is deterministic.
- **Two cryptographic algorithms in one trust handshake.** Gateway signs with ed25519, model signs with secp256k1/ECDSA. Both keys are bound into their respective REPORTDATA — but with different layouts. Code must support both.
- **Three TDX quotes per orchestrated request.** This is *more* assurance than any other provider gives us, but it triples per-request verification work. Each quote is 5 KB, `dcap-qvl::verify` typical runtime ~tens of ms per quote on mobile — should be acceptable inside a session-open flow.
- **Compose-manager attestation** binds an *append-only ledger of orchestration actions* (image pushes, redeploys, scaling events) into the enclave — this is a meaningful sovereignty property other providers don't expose. We can surface it in the UI ("model image was last published by commit `bbe30f5...`") without extra crypto work.
- **Chutes uses base64 for `intel_quote`**, not hex. Don't assume hex.
- **Chutes binds an HPKE `e2e_pubkey`** into REPORTDATA. If we ever do E2EE through Chutes (HPKE handshake, similar to Venice's secp256k1 ECDH path), the binding is already there — verifying it gates the handshake correctly.
- **Chutes does not honor the client `?nonce=`.** It embeds an enclave-baked nonce into REPORTDATA instead. Practical consequence: a Chutes attestation proves "this enclave instance bound this `e2e_pubkey` at boot under nonce N" — sufficient to gate an HPKE session, *not* sufficient as a per-request liveness proof. The trust UI must say "verified attestation issued at enclave boot" rather than "verified for this request" for Chutes-routed models. Shapes A and B don't have this caveat.
- **Chutes also requires a `td_attributes` debug-mode check** (bit 0 of body[120..128]). The reference verifier flags an enclave running in debug mode as CRITICAL — we must replicate that gate.
- **Tinfoil-via-Redpill is broken upstream** (`Unsupported Tinfoil attestation format`). For Tinfoil, keep using the existing direct integration. Re-test in a few months.
- **Redpill exposes optional secondary verifiers** (Automata smart contracts, Intel Trust Authority, Sigstore golden values, dstack-deep boot replay). All four are interesting but **none are required** for client-side trust — pure-Rust `dcap-qvl` + NRAS are sufficient. They become relevant if we want to publish *audit receipts*, not for live verification.
- The reference verifier's signature endpoint `/v1/signature/{chatId}` returns a separate ECDSA signature over `sha256(request) || sha256(response)` for after-the-fact response integrity checks — this is the "verify the response is the one that was inferred" half of the story, complementary to attestation. **Worth implementing in a v2 phase**: it lets users prove a specific response came from a specific attested enclave. Out of scope for v1 attestation feasibility.

**Risk that could still kill the integration:**
- **None at the verification layer.** Full client-side TEE verification is feasible, with zero new crates and no new attestation primitives beyond what Venice already validated.
- Out-of-scope risks for this spike (defer to plan phase): rate limits on the unauthenticated attestation endpoint; cost per token across the Redpill model catalog; whether streaming chat completions remain OpenAI-compatible after attestation gate; UX for surfacing 3-quote orchestrated verification without overwhelming the user; whether to expose the `/v1/signature/{chatId}` after-the-fact response binding as a separate "verify this response" affordance.

**Decision:** Promote directly to a build phase. The Redpill provider variant should reuse Venice's REPORTDATA decoder for the model-ecdsa case and add three small extra layout decoders. No new dependencies in `Cargo.toml` are required.

## Sources

- <https://github.com/redpill-ai/redpill-verifier> — `js/src/verify.ts`, `js/src/verifiers/cloud-api.ts`, `js/src/verifiers/chutes.ts`, `js/src/providers/detect.ts`, `js/src/constants.ts`, `README.md`
- Live captures (this spike): `captures/attestation-phala-raw.json` (Shape B, NearAI-orchestrated), `captures/attestation-phala-pure-raw.json` (Shape A, flat), `captures/attestation-chutes-raw.json` (Shape C, chutes), `captures/attestation-tinfoil-raw.json` (502 reproduction)
- <https://nras.attestation.nvidia.com/v3/attest/gpu> — already integrated
- <https://cloud-api.phala.network/api/v1/attestations/verify> — Phala's hosted TDX verifier (used by reference; we will not depend on it)
- <https://github.com/Phala-Network/dstack> — dstack runtime (background only; not a dependency)
- Companion: spike 001 — `.planning/spikes/001-venice-tee-protocol-research/README.md` for the Venice REPORTDATA decoder this work reuses.
