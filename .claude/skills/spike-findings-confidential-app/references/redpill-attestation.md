# Redpill TEE Attestation

Implementation blueprint for adding Redpill (`api.redpill.ai`) as a confidential-inference provider with full client-side TEE verification — at parity with Venice and ppq.

## Requirements

These are non-negotiable design decisions established across the spike sessions.

- Client must independently verify the Intel TDX quote signature and PCK certificate chain — never trust the provider's `verified: true` boolean alone (this includes Phala's hosted verifier API that the reference `redpill-verifier` delegates to).
- Client must independently bind the per-request nonce into REPORTDATA where the provider supports a client-supplied nonce.
- Client must verify the per-session signing key is bound into REPORTDATA so subsequent E2EE handshake keys cannot be substituted.
- Where NVIDIA GPU attestation is present, client must POST the GPU evidence payload to NRAS and verify the response JWT, not trust the provider's NVIDIA verification field.
- For aggregator providers that route across multiple backends, the client must dispatch on response shape and verify *all* components of multi-quote attestations (e.g. gateway + model + compose-manager) before opening a session.
- Where a backend embeds an enclave-baked nonce instead of the client's nonce (Redpill→Chutes), the trust UI must downgrade the freshness claim from "per-request" to "per-enclave-instance".

## How to Build It

### 1. Endpoint and dispatch

```
GET https://api.redpill.ai/v1/attestation/report?model=<model_id>&nonce=<64-hex>
   Public, no Authorization header. Same UX gift as Venice: verify before paying.
```

The response has **three possible shapes** dispatched by routing. Detect with this priority order (mirrors `redpill-verifier::js/src/providers/detect.ts`):

```rust
enum RedpillShape {
    /// attestation_type == "chutes" → Shape C
    Chutes,
    /// gateway_attestation present + model_attestations[] → Shape B
    Orchestrated { is_near_ai: bool },
    /// signing_address + intel_quote at top level → Shape A
    Flat,
}

fn detect(json: &Value) -> RedpillShape {
    if json.get("attestation_type").and_then(|v| v.as_str()) == Some("chutes") {
        return RedpillShape::Chutes;
    }
    if json.get("gateway_attestation").is_some() && json.get("model_attestations").is_some() {
        let near_ai = json["model_attestations"][0]
            .get("compose_manager_attestation")
            .is_some();
        return RedpillShape::Orchestrated { is_near_ai: near_ai };
    }
    RedpillShape::Flat
}
```

### 2. REPORTDATA layout decoders

All four layouts cover the 64-byte REPORTDATA field at TDX quote bytes `[568..632]` (header 48 B + body offset 520..584). Decoders verify the nonce/key binding only — the *quote signature itself* is verified separately by `dcap-qvl::verify`.

```
GATEWAY (Shape B, ed25519):
  [ 0..32]  signing_address (raw ed25519 pubkey)
  [32..64]  client nonce — exact bytes submitted

MODEL (Shape A and Shape B, ecdsa secp256k1, NVIDIA-CC):
  [ 0..20]  signing_address = keccak256(uncompressed_pubkey[1..65])[12..32]
  [20..32]  zero padding
  [32..64]  client nonce — exact bytes submitted

COMPOSE_MANAGER (Shape B only):
  [ 0..32]  actions_hash (sha256 over the orchestration commit ledger)
  [32..64]  client nonce — exact bytes submitted

CHUTES (Shape C):
  [ 0..32]  SHA256(nonce_str ++ e2e_pubkey_str)   ← STRING concat, ASCII bytes
  [32..64]  unconstrained — Chutes does NOT bind a client-fresh nonce here
```

The MODEL layout is **byte-identical to Venice's spike-001 finding**. Reuse the Venice decoder verbatim for Shape A and the model component of Shape B.

### 3. TDX quote verification

Every quote across every shape is **TDX v4 ECDSA-P256** (header `04 00 02 00 81 00 00 00`). Quote size 5006 bytes for orchestrated shapes; ~5000 bytes for chutes. Some quotes ship as **base64**, others as **hex** — detect and normalize before parsing:

```rust
fn quote_bytes(s: &str) -> Vec<u8> {
    let s = s.trim_start_matches("0x");
    if s.bytes().all(|b| b.is_ascii_hexdigit()) {
        hex::decode(s).expect("hex quote")
    } else {
        base64::engine::general_purpose::STANDARD.decode(s).expect("b64 quote")
    }
}
```

Then verify with `dcap-qvl::verify(quote_bytes, collateral, ts)` — same path already used for ppq and Venice. No new crates.

### 4. NVIDIA GPU attestation

`nvidia_payload` is present on Shape A and Shape B (always JSON-stringified — double parse), and per-attestation `gpu_evidence[]` on Shape C. POST to `https://nras.attestation.nvidia.com/v3/attest/gpu` and verify the returned JWT — exact same path as `rust/src/attestation/nvidia.rs` for ppq/Venice.

### 5. Composition rules per shape

| Shape | What must verify before session open |
|-------|--------------------------------------|
| **A. Flat (Phala-pure)** | TDX quote sig + PCK chain + REPORTDATA model layout + NVIDIA NRAS JWT |
| **B. Orchestrated** | All three: gateway TDX quote + model TDX quote + compose-manager TDX quote, each with its own REPORTDATA layout. PLUS NVIDIA NRAS for the model. Three-way AND. |
| **C. Chutes** | TDX quote sig + REPORTDATA chutes layout + `td_attributes[0] & 1 == 0` (debug mode disabled) + per-GPU `gpu_evidence` validation. Surface "freshness valid for enclave lifetime" in trust UI. |

### 6. td_attributes debug-mode gate (Chutes)

```rust
fn debug_mode_disabled(quote_bytes: &[u8]) -> bool {
    // td_attributes is at body offset 120..128 (quote offset 168..176)
    let attr = quote_bytes[48 + 120];
    (attr & 0x01) == 0
}
```

Chutes verifier flags an enclave running in debug mode as **CRITICAL**. Replicate that gate. For Shapes A and B this check is also worth running defensively, even though those backends should never expose a debug enclave.

### 7. Tinfoil-via-Redpill is currently broken upstream

Probing a Tinfoil-routed model returns HTTP 502:
`Unsupported Tinfoil attestation format: https://tinfoil.sh/predicate/sev-snp-guest/v2`

Treat Redpill→Tinfoil as **unsupported** for now and route Tinfoil models through our existing direct-Tinfoil integration. Re-test the Redpill relay periodically; once they speak SEV-SNP v2 we can fold this back in.

### 8. Captured fixtures

Real wire data for all four backends is preserved in `sources/002-redpill-tee-verification-research/captures/`:

- `attestation-phala-raw.json` — Shape B (NearAI-orchestrated, three TDX quotes)
- `attestation-phala-pure-raw.json` — Shape A (Venice-identical)
- `attestation-chutes-raw.json` — Shape C (base64 quotes, anti-tamper binding)
- `attestation-tinfoil-raw.json` — 502 reproduction (relay limitation)
- `decode-report-data.py` — reference decoder; assertions double as Rust unit-test cases
- `nonce.txt` — submitted nonces for each capture

## What to Avoid

- **Do not trust `verified: true`** anywhere in the response. The reference `redpill-verifier` Light Mode delegates TDX checks to Phala's hosted endpoint — defeating the trust model. We do `dcap-qvl` ourselves.
- **Do not assume one response shape.** Redpill exposes three; missing the dispatcher means one backend silently bypasses verification.
- **Do not assume hex for Chutes quotes.** They're base64. Ref `redpill-verifier::toHexQuote` shows the auto-detect.
- **Do not assume the client's `?nonce=` is honored on Chutes.** It isn't — Chutes uses an enclave-baked nonce. Surfacing this as "verified for this request" is misleading.
- **Do not skip the compose-manager quote on Shape B.** It binds the orchestration ledger into the enclave; ignoring it lets a malicious orchestrator quietly redeploy a different image.
- **Do not depend on Phala's `info.app_cert` envelope.** The raw `intel_quote` is sufficient and self-contained — same simplification as Venice.
- **Do not pull in `dcap-qvl` v0.4 or later without re-verifying** the quote-byte offsets we slice (`[568..632]` for REPORTDATA, `[48+120]` for td_attributes). Lock the version we ship.
- **Do not make on-chain DCAP / Intel Trust Authority / dstack-deep boot-replay required.** They're useful as opt-in audit-receipts; required-on-the-critical-path means an Ethereum RPC dependency on every device, which we don't want.
- **Do not mix Chutes string-concat with byte-concat.** The hash is `SHA256(nonce_str ++ e2e_pubkey_str)` over the **ASCII bytes** of the as-emitted strings, not over their decoded byte representations.

## Constraints

| Constraint | Source | Implication |
|------------|--------|-------------|
| Three response shapes (Flat / Orchestrated / Chutes) | Live captures | Dispatcher required; cannot collapse to one parser |
| Three TDX quotes per Orchestrated request (gateway + model + compose-mgr) | Live capture | Per-request verification cost ~3× a flat response. dcap-qvl is fast enough; budget tens of ms per quote on mobile. |
| TDX v4 ECDSA-P256 across all backends | Live captures | `dcap-qvl` handles all of them; no new TDX crate needed |
| Chutes uses base64 for `intel_quote` | Live capture + chutes.ts | Quote-bytes helper must auto-detect format |
| Chutes ignores client `?nonce=`; embeds enclave-baked nonce | Live capture | Trust UI must not promise per-request freshness for Chutes models |
| Tinfoil-via-Redpill returns 502 on SEV-SNP v2 | Live capture | Use existing direct-Tinfoil integration; not a Redpill blocker |
| Endpoint is public and unauthenticated | Live capture (HTTP 200 with no headers) | Free pre-flight verification before paying |
| Reference `redpill-verifier` Light Mode trusts Phala's hosted API | js/src/verifiers/cloud-api.ts | Do not delegate to Phala; re-verify locally |
| Chutes `td_attributes` debug bit must be 0 | js/src/verifiers/chutes.ts | Replicate as a hard gate |
| Optional secondary verifiers (Automata on-chain DCAP, Intel Trust Authority, Sigstore golden values, dstack-deep) | Reference verifier source | Defer to v2 audit-receipts feature; not on critical path |

## Origin

Synthesized from spikes: 002

Source files available in: `sources/002-redpill-tee-verification-research/`

Companion: `references/venice-attestation.md` — the model-ecdsa REPORTDATA decoder built for Venice is reused verbatim for Shape A and the model component of Shape B.
