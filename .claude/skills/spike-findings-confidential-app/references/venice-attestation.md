# Venice.ai TEE Attestation

Implementation blueprint for integrating Venice.ai as a TEE-attested LLM provider on parity with ppq (Intel TDX + NVIDIA NRAS). Derived from spike 001.

## Requirements

These are non-negotiable. Inherited from `.planning/spikes/MANIFEST.md`:

- Client must independently verify the Intel TDX quote signature and PCK certificate chain. Never trust Venice's `verified: true` boolean alone.
- Client must independently bind the per-request 32-byte nonce into REPORTDATA before accepting the attestation.
- Client must verify the secp256k1 signing key is bound into REPORTDATA[0..20] so subsequent E2EE handshake keys cannot be substituted.
- Where NVIDIA GPU attestation is present, client must POST `nvidia_payload` to NRAS and verify the response JWT — do not trust `server_verification.nvidia.valid`.

## How to Build It

### 1. Fetch the attestation (no auth)

```rust
// rust/src/llm/venice.rs (sketch)
let nonce: [u8; 32] = rand::random();
let nonce_hex = hex::encode(nonce);
let url = format!(
    "https://api.venice.ai/api/v1/tee/attestation?model={}&nonce={}",
    urlencoding::encode(model_id), nonce_hex,
);
let resp: VeniceAttestation = http.get(url).send().await?.json().await?;
```

The endpoint is **public — no `Authorization` header required**. This means we can attest before the user pays for any inference.

### 2. Response shape (Rust types)

```rust
#[derive(Deserialize)]
struct VeniceAttestation {
    // Hex (no 0x prefix) of raw Intel TDX v4 quote, ~5 KB.
    intel_quote: String,

    // JSON-encoded STRING (must be parsed twice). Contains:
    //   { nonce, evidence_list: [{certificate, evidence, arch}], arch }
    nvidia_payload: Option<String>,

    // Uncompressed secp256k1 pub key, 130 hex chars starting with "04".
    // Field name varies — accept either.
    #[serde(alias = "signing_public_key")]
    signing_key: Option<String>,

    // Ethereum-style address (0x + 40 hex), keccak256(pubkey[1..])[12..32].
    signing_address: String,

    // The nonce we sent, echoed back. Verify byte-equal to what we generated.
    nonce: String,

    model: String,
    tee_provider: String,        // "phala"
    tee_hardware: String,        // "intel-tdx"
    server_verification: ServerVerification, // do not trust, but log
}
```

### 3. Verify the TDX quote (full client-side, not just structural)

Use `dcap-qvl` exactly as the existing ppq path does:

```rust
let quote_bytes = hex::decode(&resp.intel_quote)?;
let collateral = dcap_qvl::collateral::get_collateral_from_intel_pcs(&quote_bytes).await?;
let now = chrono::Utc::now().timestamp() as u64;
let report = dcap_qvl::verify::verify(&quote_bytes, &collateral, now)
    .map_err(AttestationError::TdxVerify)?;

// `report` is a TDReport10 / TDReport15. Extract REPORTDATA (64 bytes).
let report_data: [u8; 64] = report.report.report_data;
```

This checks: quote signature with the PCK leaf, PCK leaf chains to Intel SGX Root CA, TCB level vs Intel TCB-info, QE identity vs Intel QE-identity, CRLs.

### 4. Decode REPORTDATA — Venice-specific layout

```
[ 0..20]  signing_address  (20B)  = keccak256(uncompressed_pubkey[1..65])[12..32]
[20..32]  zero padding     (12B)  = 0x000000000000000000000000
[32..64]  client nonce     (32B)  = byte-equal to the nonce we submitted
```

```rust
fn verify_venice_report_data(
    report_data: &[u8; 64],
    signing_pubkey_hex: &str,   // 130 hex, 04...
    submitted_nonce: &[u8; 32],
) -> Result<(), AttestationError> {
    // Address binding: keccak256(pubkey_xy)[12..32] == report_data[0..20]
    let pubkey = hex::decode(signing_pubkey_hex)?;
    if pubkey.len() != 65 || pubkey[0] != 0x04 {
        return Err(AttestationError::BadSigningKey);
    }
    use sha3::{Digest, Keccak256};
    let mut h = Keccak256::new();
    h.update(&pubkey[1..]);
    let addr20 = &h.finalize()[12..32];
    if addr20 != &report_data[0..20] {
        return Err(AttestationError::SigningKeyNotBound);
    }
    // Padding check (defensive): bytes 20..32 must be zero
    if report_data[20..32].iter().any(|&b| b != 0) {
        return Err(AttestationError::ReportDataPaddingNonZero);
    }
    // Nonce binding: bytes 32..64 == submitted nonce, byte-equal
    if &report_data[32..64] != &submitted_nonce[..] {
        return Err(AttestationError::NonceNotBound);
    }
    Ok(())
}
```

### 5. Verify NVIDIA GPU attestation

Reuse the existing `rust/src/attestation/nvidia.rs` path. Note `nvidia_payload` is a **JSON string** field — parse it before forwarding:

```rust
if let Some(payload_str) = resp.nvidia_payload.as_deref() {
    let payload: NvidiaPayload = serde_json::from_str(payload_str)?;
    // POST {nonce, evidence_list, arch} to https://nras.attestation.nvidia.com/v3/attest/gpu
    // Verify the returned JWT with NRAS signing pubkey (already implemented for ppq).
    nvidia::verify_nras_attestation(&payload).await?;
}
```

### 6. Sanity gates before accepting attestation

```rust
assert_eq!(report.report.tee_type, 0x81, "not a TDX quote");
let td_attr_lsb = report.report.td_attributes[0];
if td_attr_lsb & 0x01 != 0 {
    return Err(AttestationError::TdxDebugMode); // debug = no confidentiality
}
if resp.model != requested_model_id {
    return Err(AttestationError::ModelMismatch);
}
```

### 7. E2EE handshake (separate from attestation, but required to use the model)

Once attestation passes, derive an ephemeral channel keyed to the attested signing key:

- ECDH on **secp256k1**: ephemeral client key × attested model pub key.
- HKDF-SHA256 over the shared secret, `info = b"ecdsa_encryption"`, output 32 bytes (AES-256 key).
- AES-256-GCM, 12-byte random nonce per message.
- Wire format per message: `[ephemeral_pub 65B][nonce 12B][ciphertext+16B GCM tag]`, hex-encoded.
- Request headers (every chat completion in E2EE mode):
  - `X-Venice-TEE-Client-Pub-Key: <hex of our ephemeral 65B pubkey>`
  - `X-Venice-TEE-Model-Pub-Key: <hex of attested 65B model pubkey>`
  - `X-Venice-TEE-Signing-Algo: ecdsa`

Encrypt **only** the user/system message bodies, not the whole request. Server returns SSE chunks where each chunk's text content is the same hex-encoded `[eph_pub|nonce|ct+tag]` envelope.

Reference impl: `veniceai/venice-cli` → `src/lib/e2ee.ts` (preserved in `sources/001-venice-tee-protocol-research/`). We can re-implement directly with `k256` + `hkdf` + `aes-gcm` Rust crates (or `secp256k1` if we already use it for ECDSA elsewhere).

## What to Avoid

- **Trusting `server_verification.tdx.valid` / `.nvidia.valid` / `.nonceBinding.bound`.** These are server self-reports. They're fine to surface in UI as a sanity hint, but the client must independently re-verify everything. The reference `venice-cli` is structurally weaker than what we ship — it does only structural quote parsing and trusts these booleans.
- **Trusting `verified: true` at the response root.** Same problem.
- **Forgetting the field-name fallback.** Live API returns `signing_public_key`; docs and CLI sometimes call it `signing_key`. Use `serde(alias)` for both.
- **Forgetting the double JSON parse on `nvidia_payload`.** It's a string containing JSON, not a nested object.
- **Building a Venice-specific TDX parser from scratch.** `dcap-qvl` already parses DCAP v4 quotes correctly; reusing it gives us cert-chain + CRL + TCB checks for free.
- **Using the dstack `info.app_cert` X.509 envelope as the verification path.** It embeds the TDX quote in a custom extension, but the raw `intel_quote` field is already provided alongside it. Don't add a Phala dstack dependency just for this.
- **Hashing the nonce.** Venice uses `nonceBinding.method: "raw"` — the nonce is byte-embedded at REPORTDATA[32..64]. Don't sha256/keccak it before comparing.
- **Skipping the debug-mode check.** `td_attributes` LSB & 0x01 indicates debug TDX, which provides zero confidentiality guarantees. Reject.
- **Pinning Intel collateral too tightly.** Fetch fresh from Intel PCS each session (with a short on-device cache that respects collateral validity windows) — same policy as our ppq path.

## Constraints

- **Endpoint:** `GET https://api.venice.ai/api/v1/tee/attestation?model=<id>&nonce=<64hex>`. Public, unauthenticated, returns JSON.
- **Required nonce length:** exactly 32 bytes / 64 hex chars. Server enforces this and 400s otherwise.
- **TDX quote version:** v4 (`bytes[0..2] = 04 00`), attestation key type ECDSA-P256 (`bytes[2..4] = 02 00`), tee_type TDX (`bytes[4..8] = 81 00 00 00`). Stable as of Apr 2026.
- **Quote size:** ~5 KB (10 012 hex chars in the captured sample). DCAP v4 with full PCK cert chain.
- **NRAS endpoint:** `POST https://nras.attestation.nvidia.com/v3/attest/gpu` with `{nonce, evidence_list, arch}`. Same as ppq.
- **Substrate:** Phala dstack on Intel TDX + NVIDIA-CC. dstack image identifier `dstack-nvidia-dev-0.5.5` in the captured sample. We don't depend on this — it's just useful context for diagnosing failures.
- **TEE-capable model IDs:** prefixed `e2ee-*-p` (e.g. `e2ee-venice-uncensored-24b-p`, `e2ee-glm-4-7-p`, `e2ee-qwen3-30b-a3b-p`). Listed via `GET /api/v1/models?type=text` filtered by `model_spec.capabilities.supportsE2EE == true`.
- **Chat completions API:** standard OpenAI-compatible at `/api/v1/chat/completions`. Set `enable_e2ee: true` in the request body to opt into the E2EE wrapper. The Responses API (alpha) does NOT support E2EE — must use chat completions.
- **Attestation freshness:** per-request nonce. Cache the attestation only for the lifetime of an unbroken E2EE session; re-attest on any reconnect.

## Origin

Synthesized from spikes: 001
Source files available in: `sources/001-venice-tee-protocol-research/`

Live capture: `sources/001-venice-tee-protocol-research/captures/attestation-sample.json` — full real attestation response (TDX quote, NRAS payload, dstack app_cert, server verification details). Reference for unit tests and golden-path assertions.
