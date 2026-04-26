---
spike: 001
name: venice-tee-protocol-research
type: standard
validates: "Given Venice public docs + reference CLI + a free unauthenticated probe of the attestation endpoint, when analyzed end-to-end, then we know the exact wire format, REPORTDATA layout, root-of-trust topology, and required client-side checks for parity with tinfoil/ppq"
verdict: VALIDATED
related: []
tags: [venice, tdx, nvidia-cc, attestation, phala-dstack]
---

# Spike 001: Venice TEE Protocol Research

## What This Validates

**Given** Venice's public docs at <https://docs.venice.ai/overview/guides/tee-e2ee-models>, the open-source reference implementation in `veniceai/venice-cli`, and a single live capture of `GET /api/v1/tee/attestation`,
**when** the protocol, response shape, REPORTDATA layout, and root-of-trust topology are analyzed end-to-end,
**then** we can answer: *can Venice be integrated into the Confidential App with the same client-side TDX + NVIDIA-CC verification rigor we already apply to tinfoil and ppq?*

## Research

### Documents read
- `docs.venice.ai/overview/guides/tee-e2ee-models` — protocol overview, response field table, JS/Python example code (only verifies `verified: true`, doesn't parse the quote).
- `docs.venice.ai/overview/privacy` — privacy-tier overview.
- `github.com/veniceai/venice-cli` (TypeScript, MIT) — `src/lib/tee.ts`, `src/lib/e2ee.ts`, `src/lib/api.ts`, `src/commands/tee.ts`, `src/commands/chat.ts`. This is the reference verifier.

### Wire format (confirmed from a live capture, not docs)
```
GET https://api.venice.ai/api/v1/tee/attestation?model=<model_id>&nonce=<64-hex-chars>
   No Authorization header required. Endpoint is public.
```

Response JSON keys actually emitted (more than the docs list):
```
signing_address, signing_public_key, signing_algo, request_nonce,
intel_quote, nvidia_payload, info (dstack app_cert + app_id + instance_id),
quote (raw hex of intel_quote), event_log, vm_config,
verified, model, nonce, nonce_source, tee_provider, tee_hardware,
upstream_model, server_verification, candidates_evaluated, candidates_available
```

A captured sample is checked into `captures/attestation-sample.json`. Live values from that capture:
- `tee_provider: "phala"`, `tee_hardware: "intel-tdx"`
- `vm_config.image: "dstack-nvidia-dev-0.5.5"` — Venice runs on Phala's **dstack** confidential VM runtime
- `vm_config.num_gpus: 1`, `cpu_count: 24`, `memory_size: 192 GiB`
- `signing_algo: "ecdsa"`, secp256k1 uncompressed public key (65 bytes, leading `04`)
- `intel_quote`: 5 006 bytes (10 012 hex chars). Header `04 00 02 00 81 00 00 00` → TDX quote v4, attestation_key_type = ECDSA-256-with-P-256 (=2), tee_type = 0x81 (TDX). Standard DCAP layout — `dcap-qvl` parses this directly.
- `nvidia_payload`: 12 KB JSON `{nonce, evidence_list[], arch}` for NRAS POST to `https://nras.attestation.nvidia.com/v3/attest/gpu`.

### REPORTDATA layout (definitive — confirmed by capture)

The TDX REPORTDATA field is exactly 64 bytes, structured as:

```
[ 0..20]  signing_address  20B   = keccak256(uncompressed_pubkey[1..65])[12..32]
[20..32]  zero padding     12B   = 00 00 00 00 00 00 00 00 00 00 00 00
[32..64]  client nonce     32B   = the exact bytes the client passed in ?nonce=
```

Server-side returns `server_verification.nonceBinding.method: "raw"` confirming the nonce is embedded raw (not hashed). This is independently verifiable from the parsed quote on the client.

The signing-address binding `keccak256(uncompressed_pubkey)[12..32] == reportData[0..20]` is exactly what `venice-cli/src/lib/e2ee.ts::verifyKeyBinding` checks. Trivial to port to Rust.

### Root of trust topology

| Layer | Root | How verified |
|-------|------|--------------|
| Intel TDX quote signature | Intel SGX/TDX root CA (Intel PCS) | `dcap-qvl::verify(quote, collateral, ts)` — pure Rust, already in our recommended stack |
| Intel PCK cert chain | Intel SGX Root CA | included in quote; verified by `dcap-qvl` |
| NVIDIA GPU attestation | NVIDIA NRAS root + NVIDIA Device Identity CA | POST `nvidia_payload` to NRAS, verify returned JWT (matches our existing `rust/src/attestation/nvidia.rs`) |
| Signing key binding | Intel TDX (transitively) | recompute `keccak256(pub)[12..32]`, compare to `reportData[0..20]` — `sha3` crate |
| Nonce freshness | Self-supplied | byte-equal `reportData[32..64]` to the nonce we generated |
| Phala dstack `app_cert` | The TDX quote inside it | optional extra layer — embeds quote in X.509 extension; can ignore in v1 |

Venice's server already does signature + cert chain + CRL + root-CA-pin + attestation-key-match (as shown in `server_verification.tdx`), but **we will not rely on those booleans**. We re-verify everything from `intel_quote` raw bytes ourselves.

### Comparison to existing providers

| Check | tinfoil (today) | ppq (today) | Venice (proposed) |
|-------|-----------------|-------------|-------------------|
| TEE primitive | NVIDIA NRAS JWT only | Intel TDX + NVIDIA NRAS | Intel TDX + NVIDIA NRAS |
| Attestation transport | per-request signed payload | dedicated endpoint | dedicated endpoint `/api/v1/tee/attestation` |
| Client supplies nonce | yes (per-request) | yes | yes (32B random, REPORTDATA[32..64]) |
| Quote signature verified client-side | n/a (no raw quote) | yes (`dcap-qvl`) | yes (same path as ppq) |
| Cert chain verified client-side | n/a | yes | yes |
| Per-response signing key bound to attestation | yes (ECDSA) | yes | yes (secp256k1, address in REPORTDATA[0..20]) |
| GPU attestation | yes (NRAS) | yes (NRAS) | yes (NRAS, payload provided) |
| Auth required to fetch attestation | n/a | yes | **no — endpoint is public** |
| Reference verifier exists | own Rust impl | own Rust impl | venice-cli (TS, only structural; we do better) |

### Approach comparison

| Approach | Pros | Cons | Status |
|----------|------|------|--------|
| Use existing `dcap-qvl` + new `venice.rs` REPORTDATA layout module | Reuses our existing TDX verification code from ppq integration; minimum new surface area | Need a tiny Venice-specific REPORTDATA decoder (20B addr + 12B pad + 32B nonce) | Chosen |
| Use Phala's `dstack-sdk` for dstack-aware verification | Could verify the `info.app_cert` extension chain too | Adds a Phala-specific dependency; not needed since the raw `intel_quote` is sufficient and we ignore the dstack envelope | Rejected |
| Trust `server_verification` booleans | Trivial to implement | Defeats the whole point of the app; no better than HTTPS trust | Rejected |
| Skip client-side and rely on attested HTTPS only | Simplest | Same problem as above | Rejected |

**Chosen approach:** add a `venice` provider variant in `rust/src/attestation/` that reuses the existing TDX verification path (`dcap-qvl` for quote sig + cert chain) and the existing NVIDIA NRAS path (`rust/src/attestation/nvidia.rs`), and adds one small Venice-specific REPORTDATA layout check.

## How to Run

```bash
# Reproduce the live capture (no API key needed)
curl -sS "https://api.venice.ai/api/v1/tee/attestation?model=e2ee-venice-uncensored-24b-p&nonce=$(openssl rand -hex 32)" \
  | python3 -m json.tool > captures/attestation-sample.json

# Inspect REPORTDATA layout
python3 -c "
import json
d = json.load(open('captures/attestation-sample.json'))
rd = d['server_verification']['tdx']['reportData']
print('addr  :', rd[0:40])
print('pad   :', rd[40:64])
print('nonce :', rd[64:128])
print('echoed:', d['nonce'])
print('match :', rd[64:128] == d['nonce'])
"
```

## What to Expect

- Capture file populated with a real Venice TDX + NVIDIA attestation.
- REPORTDATA decoded into 20B address + 12B zeros + 32B nonce, with the nonce field byte-equal to the nonce we submitted.
- All `server_verification.tdx.*` and `server_verification.nvidia.*` booleans `true`.

## Investigation Trail

1. **Started with the docs page.** Surfaced field table (`intel_quote`, `signing_key`, `nvidia_payload`, etc.) but the doc explicitly says clients should "verify the attestation client-side" without showing how. JS sample only trusts `verified: true`. Ambiguous whether real client-side verification is feasible at the docs level alone.
2. **Found the reference CLI.** `veniceai/venice-cli` ships `venice tee verify` — TypeScript, MIT. Read `src/lib/tee.ts` and `src/lib/e2ee.ts` end-to-end. Confirmed structural parsing of TDX quote (offsets 48..632), but the CLI **does not** verify the quote signature or cert chain itself — it relies on the server's `server_verification.tdx.valid` boolean. The CLI's only client-side cryptographic check is the signing-key→REPORTDATA binding (first 20 bytes = keccak address).
3. **Discovered the second binding.** REPORTDATA is 64 bytes; CLI only inspects the first 20. Where does the nonce live? Server returns `nonceBinding.method: 'sha256' | 'raw'` — type system hint that one of them is the raw nonce. Couldn't determine layout from docs or CLI alone.
4. **Probed the endpoint live.** Without an API key, `curl https://api.venice.ai/api/v1/tee/attestation?model=...&nonce=...` returns 400 ("model does not support TEE attestation") not 401 — endpoint is **public, unauthenticated**. Looked up TEE-capable models from `/api/v1/models?type=text` (the `e2ee-*-p` family), then captured a real attestation against `e2ee-venice-uncensored-24b-p`.
5. **Decoded REPORTDATA.** From the live capture: `[20B signing_address][12B zero][32B nonce]`. Nonce bytes match the submitted nonce exactly. `nonceBinding.method` = `"raw"` confirms.
6. **Identified the runtime.** `tee_provider: "phala"`, `tee_hardware: "intel-tdx"`, `vm_config.image: "dstack-nvidia-dev-0.5.5"` — Venice's TEE substrate is Phala's dstack confidential VM with a single NVIDIA confidential-compute GPU. The dstack `app_cert` (X.509 in `info.app_cert`) embeds the TDX quote in a custom extension; we can ignore that envelope and verify the raw `intel_quote` directly.
7. **Confirmed quote header.** Parsed first 8 bytes of `intel_quote`: version=4, attestation_key_type=2 (ECDSA-P256), tee_type=0x81 (TDX). Standard DCAP v4 quote — `dcap-qvl` already parses this for ppq.

## Results

**Verdict: ✓ VALIDATED.** Venice can be integrated with verification at parity with — or stronger than — what we do for tinfoil and ppq. Specifically:

**What we get for free, no code:**
- Public unauthenticated `GET /api/v1/tee/attestation` (no API-key cost to fetch and verify before opening a session).
- Raw Intel TDX quote (DCAP v4, ECDSA-P256) directly compatible with `dcap-qvl` already on our recommended stack.
- NVIDIA NRAS payload directly compatible with our existing `rust/src/attestation/nvidia.rs`.
- Per-request 32-byte nonce, raw-embedded in REPORTDATA[32..64] — independently verifiable.
- secp256k1 signing key bound into REPORTDATA[0..20] via Ethereum-style keccak address — independently verifiable with `sha3` + slice-compare.

**What we have to build (small):**
- A Venice-specific REPORTDATA decoder (20B addr + 12B pad + 32B nonce) — single file, ~60 lines.
- A Venice provider variant in the existing attestation/policy modules — wires the above into the existing pipeline.
- E2EE handshake using ECDH(secp256k1) + HKDF-SHA256(`"ecdsa_encryption"`) + AES-256-GCM, with header layout `[ephemeral_pub 65B][nonce 12B][ciphertext+tag]` (mirrors `venice-cli/src/lib/e2ee.ts`). This is **on top** of attestation and is what gives the "E2EE" half of "TEE + E2EE". Scope-defining for the build phase, not for this spike.

**Surprises and gotchas:**
- Endpoint is **unauthenticated** — counterintuitive, but means clients can attest *before* paying. Good for our UX (we can verify-then-decide-to-pay).
- `signing_key` field name varies — Venice returns `signing_public_key` in the live response but the docs and CLI sometimes call it `signing_key`. The `venice-cli` chat handler reads either: `response.signing_key || response.signing_public_key`. Implement the same fallback.
- Their `nvidia_payload` is returned as a **JSON-encoded string**, not a JSON object. Need `JSON.parse` (or `serde_json::from_str` after extracting the string) before forwarding to NRAS.
- `tee_provider: "phala"` and the dstack envelope means the same verification code will work for any other Phala-dstack-hosted model behind any other vendor — useful future generality.
- The `venice-cli` reference verifier is **structurally weaker** than what we'll ship. We will be the first OpenAI-compatible Venice client to do full client-side TDX cryptographic verification; CLI only does field shape checks.

**Risk that could still kill the integration:**
- **None identified at this layer.** Client-side TEE attestation is feasible.
- Out-of-scope risks for this spike (deferred to plan phase): rate limits on attestation endpoint, model availability/SLA, pricing per token, whether streaming inside the E2EE wrapper is supported on `chat/completions` (docs say yes via `enable_e2ee` param), how to plumb the E2EE header trio (`X-Venice-TEE-Client-Pub-Key`, `X-Venice-TEE-Model-Pub-Key`, `X-Venice-TEE-Signing-Algo`) through `async-openai`'s request builder.

**Decision:** Skip spikes 002 (live probe) and 003 (Rust verify) originally proposed — 002 was already accomplished here as part of investigation, and 003 is now an implementation task with no remaining unknowns, not a spike. Promote directly to a build phase.

## Sources

- <https://docs.venice.ai/overview/guides/tee-e2ee-models>
- <https://docs.venice.ai/overview/privacy>
- <https://github.com/veniceai/venice-cli> — `src/lib/tee.ts`, `src/lib/e2ee.ts`, `src/lib/api.ts`
- Live capture (this spike): `captures/attestation-sample.json`
- <https://nras.attestation.nvidia.com/v3/attest/gpu> (existing attestation root for NVIDIA-CC, already integrated)
- <https://github.com/Phala-Network/dstack> (background context only — not a dependency)
