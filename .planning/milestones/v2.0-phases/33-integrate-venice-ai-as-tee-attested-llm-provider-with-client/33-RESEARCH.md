# Phase 33: Integrate Venice.ai as TEE-Attested LLM Provider — Research

**Researched:** 2026-04-25
**Domain:** TEE attestation (Intel TDX + NVIDIA NRAS) on Phala dstack substrate, secp256k1 ECDH key agreement, AES-256-GCM E2EE wrapper around OpenAI-compatible chat completions, integration into existing AttestedProvider pipeline
**Confidence:** HIGH (protocol confirmed by live capture in spike 001; integration shape confirmed by reading all three existing provider implementations: tinfoil, ppq.ai, custom endpoint)

---

## Summary

Phase 33 integrates Venice.ai as the third attested provider after Tinfoil and PPQ.AI, but with a meaningfully different attestation+E2EE architecture. Where Tinfoil uses AMD SEV-SNP via Tinfoil's ATC bundle and PPQ.AI uses AMD SEV-SNP via its own bundle endpoint, Venice runs on **Phala dstack** with **Intel TDX + NVIDIA NRAS** and exposes a **public unauthenticated** attestation endpoint that returns a raw DCAP v4 TDX quote and a separate NRAS evidence payload. The TDX REPORTDATA layout is Venice-specific (`[20B keccak-address][12B zero][32B raw nonce]`) — different from the existing TDX path which expects the nonce in `report_data[..32]`.

The good news, validated end-to-end by spike 001 (sample capture in `.claude/skills/spike-findings-confidential-app/sources/001-venice-tee-protocol-research/captures/attestation-sample.json`): every cryptographic check we need is reachable with crates already in our stack (`dcap-qvl`, `jsonwebtoken`, `aes-gcm`, `hkdf`, `reqwest`, `sha2`) plus three small new ones (`k256` for secp256k1 ECDH, `sha3` for keccak256 address binding, `urlencoding` for query-string encoding). No NVIDIA-specific code is new — the existing `attestation/nvidia.rs::fetch_and_verify_nvidia` accepts a payload + nonce_hex and is reused as-is.

The novel work is (a) a Venice-specific REPORTDATA decoder that splits the 64 bytes per the address/pad/nonce schema and verifies the keccak256 address binding to the secp256k1 signing key, and (b) the E2EE handshake — a per-session ECDH(secp256k1) → HKDF-SHA256 → AES-256-GCM wrapper applied to user/system message bodies inside an otherwise-standard OpenAI-compatible request, with three custom request headers and a hex-encoded `[ephemeral_pub|nonce|ct+tag]` envelope per message. This is structurally similar to the existing PPQ.AI HPKE wrapper but uses ECDH on secp256k1 instead of HPKE/X25519 and is per-message rather than per-request.

**Primary recommendation:** Add a new `ProviderKind::Venice` variant + `ProviderTransportKind::VeniceE2ee` and create `rust/src/llm/venice.rs` mirroring the structural shape of `ppq_private.rs`. Keep the TDX cryptographic verification path inside `dcap-qvl` (do not write a custom parser). Introduce a Venice-specific REPORTDATA decoder in `attestation/venice.rs` (or extend `attestation/tdx.rs` with a layout enum). Reuse `attestation/nvidia.rs::fetch_and_verify_nvidia` unmodified. The `verify_tdx_quote` function in `attestation/tdx.rs` is **not** directly reusable because it hard-codes nonce-at-`[..32]`; either parameterise its layout or write a sibling function `verify_tdx_quote_venice`.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Attestation fetch + cryptographic verification (TDX quote, NRAS JWT, REPORTDATA decode) | Rust core (attestation/) | — | Per project core value: "Every inference request is provably confidential" — verification belongs to the actor-owned attestation pipeline; native UI never sees raw quotes. |
| ECDH key agreement, HKDF, AES-GCM session crypto | Rust core (llm/venice.rs) | — | All crypto stays Rust per CLAUDE.md (no OpenSSL, all-Rust crypto via RustCrypto). |
| E2EE message envelope encoding/decoding | Rust core (llm/venice.rs) | — | Mirrors existing pattern for `ppq_private.rs` and `tinfoil_secure.rs`. |
| OpenAI-compatible chat completion request/response shaping | Rust core (llm/venice.rs + async-openai) | — | Same pattern as ppq_private.rs — start from `CreateChatCompletionRequestArgs`, then wrap. |
| Provider preset (UI add-backend form) | Rust core (llm/backend.rs `known_provider_presets`) | iOS/Android/Desktop UI | UniFFI-exported single source of truth; native renders the row. |
| Verification status badge for Venice | Native UI (existing AttestationStatus enum) | Rust core (sets enum variant) | `AttestationStatus` already crosses UniFFI; no new types needed for status surfacing. |
| TEE type display (`Intel TDX + NVIDIA H100 CC`) | Rust core (TeeType::IntelTdx + Phase 18 multi-TEE config if applicable) | Native UI | TeeType enum is already UniFFI-exported. |
| Venice-supplied "verified: true" booleans | Rust core (logged only, NOT trusted) | — | Spike requirement: never trust server self-reports. |

---

## Standard Stack

### Reused (already in Cargo.toml — no version change)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `dcap-qvl` | 0.3 (pinned) | Intel TDX DCAP v4 quote signature + cert chain + TCB + CRL verification | Already used for TDX/NVIDIA-CC paths. Spike 001 confirmed Venice's `intel_quote` is standard DCAP v4 (header `04 00 02 00 81 00 00 00`) — `dcap-qvl::verify::verify` parses it directly. [VERIFIED: spike 001 capture; existing usage in `attestation/tdx.rs`] |
| `jsonwebtoken` | 10 | NRAS JWT verification | Already used by `attestation/nvidia.rs::fetch_and_verify_nvidia` — reused unmodified. [VERIFIED: existing code] |
| `aes-gcm` | 0.10 | AES-256-GCM symmetric cipher for E2EE message bodies | Already in stack (used by ppq_private/tinfoil_secure). [VERIFIED: Cargo.toml] |
| `hkdf` | 0.12 | HKDF-SHA256 key derivation from ECDH shared secret | Already in stack. [VERIFIED: Cargo.toml] |
| `sha2` | 0.10 | SHA-256 inside HKDF, payload hashing | Already in stack. [VERIFIED: Cargo.toml] |
| `reqwest` | 0.12 (rustls-tls-webpki-roots) | HTTPS to `api.venice.ai` and `nras.attestation.nvidia.com` | Already in stack. Project policy: rustls only, never native-tls. [VERIFIED: Cargo.toml] |
| `rand` | 0.8 | 32-byte nonce generation, ephemeral ECDH private key generation, AES-GCM 12-byte nonce | Already in stack. [VERIFIED: Cargo.toml] |
| `hex` | 0.4 | Hex encode/decode of quote bytes, signing keys, addresses, nonces | Already in stack. [VERIFIED: Cargo.toml] |
| `serde` / `serde_json` | 1 | Wire JSON parse including double-parse of `nvidia_payload` string | Already in stack. [VERIFIED: Cargo.toml] |
| `async-openai` | 0.34 | Build the underlying OpenAI-compatible request before E2EE-wrapping its body | Already in stack. [VERIFIED: Cargo.toml] |
| `zeroize` | 1 (with `derive`) | Wipe ephemeral private keys + AES keys + shared secrets on drop | Already used by ppq_private/tinfoil_secure for HPKE keys; same pattern. [VERIFIED: Cargo.toml] |
| `once_cell` | 1 | Lazy static for verified-attestation cache (per-session) | Same pattern as `ppq_private::VERIFIED_ATTESTATIONS`. [VERIFIED: Cargo.toml + existing usage] |
| `tokio` / `flume` / `futures` | existing | Async + actor channel | Standard for the codebase. [VERIFIED: Cargo.toml] |

### New Dependencies

| Library | Version (verified) | Purpose | Why Standard |
|---------|-------------------|---------|--------------|
| `k256` | 0.13.x (latest stable; **NOT** 0.14.0-rc.9) | secp256k1 ECDH (`ecdh::diffie_hellman`), public key parse from uncompressed 65-byte form, scalar arithmetic for ephemeral keys | RustCrypto's pure-Rust secp256k1 implementation. Provides `PublicKey::from_sec1_bytes` for the `04`-prefixed 65-byte uncompressed form returned by Venice. ECDH via `elliptic_curve::ecdh::diffie_hellman`. Avoids the C `libsecp256k1` (`secp256k1` crate) which would break iOS/Android builds. [VERIFIED: cargo search returned `k256 = "0.14.0-rc.9"` — the latest stable line is 0.13.x; pin 0.13.x to avoid an rc dep. Confirm via `cargo info k256` before locking version] |
| `sha3` | 0.10.x | Keccak256 for the signing-address binding check (`keccak256(pubkey[1..65])[12..32] == REPORTDATA[0..20]`) | RustCrypto canonical implementation. Spike-confirmed exact algorithm: Ethereum-style address derivation. [VERIFIED: cargo search shows `sha3 = "0.11.0"`; 0.10.x line is also stable and matches RustCrypto digest 0.10 trait API used by sha2 already in the project — pin 0.10.x for trait compatibility] |
| `urlencoding` | 2.1.x | Percent-encode `model_id` in the attestation query string | Tiny single-purpose crate; the spike sketch uses it. Could be replaced with manual encoding — minor. [VERIFIED: cargo search `urlencoding = "2.1.3"`] |

**Version verification note:** Before locking `k256` and `sha3` versions in the plan, run `cargo info k256` and `cargo info sha3` to find the latest stable (non-`-rc`) release lines, then pin to caret. The project pattern (see Cargo.toml) is to use semver ranges (`"0.13"`, not `"=0.13.0"`) except where transitive lockstep is required. [ASSUMED — A1: pinning to 0.13.x for `k256` and 0.10.x for `sha3` is appropriate; verify in plan-time]

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `k256` (RustCrypto) | `secp256k1` (rust-bitcoin, libsecp256k1 FFI) | `secp256k1` is faster and more battle-tested for Bitcoin/Ethereum scale, but it links the C `libsecp256k1` library — breaks reproducible Nix mobile builds (CLAUDE.md §"What NOT to Use" forbids C-linked crypto on iOS App Store builds). `k256` is the right choice for our constraints. |
| Custom keccak256 implementation | `sha3` crate | Hand-rolled Keccak is a textbook landmine. `sha3` is the canonical RustCrypto crate. |
| `urlencoding` | `percent-encoding` (hyper team) | `percent-encoding` requires explicitly defining the AsciiSet which is more boilerplate for a one-liner. Either is fine. |
| Reuse `attestation/tdx.rs::verify_tdx_quote` directly | Add Venice-specific layout decoder | The existing function hard-codes `nonce_in_report = report_data[..32]` (line 109). Venice puts the nonce at `[32..64]`. Either parameterise the existing function with a `ReportDataLayout` enum (Tinfoil/PPQ vs Venice), or add a sibling `verify_tdx_quote_venice`. **Recommendation: parameterise** — single quote-verify function with explicit layout argument is cleaner and forces the planner to think about layout per provider. |
| `dcap-qvl` upgrade to 0.4 | Stay on 0.3 | `dcap-qvl = "0.4.0"` exists [VERIFIED: cargo search]. Upgrading is out of scope for this phase; stay on 0.3 to avoid dragging in a dependency upgrade. Defer to a separate maintenance phase. |

### Installation (Cargo.toml additions)

```toml
# Phase 33: Venice.ai TEE-attested provider — secp256k1 ECDH + Keccak256 address binding
k256 = { version = "0.13", default-features = false, features = ["ecdh", "arithmetic", "std"] }
sha3 = { version = "0.10", default-features = false }
urlencoding = "2.1"
```

[ASSUMED — A2: `default-features = false` keeps build size minimal; `ecdh` + `arithmetic` are the required `k256` features for `EphemeralSecret` + `PublicKey::from_sec1_bytes` + `diffie_hellman`. Verify against `k256` 0.13.x feature list before locking.]

---

## User Constraints

**No CONTEXT.md exists for Phase 33.** The discuss-phase has not been run. This research therefore:

1. Surfaces the design decision space below in the **Design Decisions to Lock** section so the planner (or a discuss-phase) can convert each into a Locked Decision.
2. Treats CLAUDE.md and the spike `<requirements>` block as the implicit constraint set.

### Implicit constraints (from CLAUDE.md and spike findings)

- **No OpenSSL / no native-tls.** All crypto must be pure Rust (RustCrypto family or HPKE crate). secp256k1 must be `k256`, not `secp256k1` crate.
- **RMP architecture.** All Venice protocol logic stays in Rust core; native UI gets only `AttestationStatus`, `BackendSummary`, `TeeType` over UniFFI — same as existing providers.
- **No telemetry / no cloud sync.** Not directly relevant to attestation but means we don't ship Venice attestation results anywhere off-device (already true of all attestation paths).
- **Spike-locked attestation requirements (non-negotiable):**
  1. Verify the Intel TDX quote signature and PCK certificate chain client-side via `dcap-qvl::verify::verify`. Never trust `server_verification.tdx.valid`.
  2. Verify the per-request 32-byte nonce is byte-equal to `REPORTDATA[32..64]` (raw, not hashed). Reject `server_verification.nonceBinding.bound`.
  3. Verify the secp256k1 signing key is bound into `REPORTDATA[0..20]` via `keccak256(pubkey[1..65])[12..32]`. This is what makes the E2EE handshake key trust-rooted.
  4. POST `nvidia_payload` to NRAS and verify the returned JWT independently via `attestation/nvidia.rs`. Never trust `server_verification.nvidia.valid`.

---

## Phase Requirements

**No requirement IDs have been assigned to Phase 33.** REQUIREMENTS.md has no v3.0 / Confidential Compute Phase 33 section. This is a **planning risk** — the planner should either:

- Create a new requirement section (suggested IDs `VEN-01..VEN-08` or `CC-VEN-01..`) before generating plans, OR
- Stub `Requirements: TBD` in plans and circle back to add IDs after the discuss-phase.

Suggested requirement set the planner can use as a starting point:

| Suggested ID | Description | Research Support |
|--------------|-------------|------------------|
| VEN-01 | Add Venice.ai as a known provider preset on the Add Backend form | `known_provider_presets` in `llm/backend.rs` is the single source of truth; same pattern as PPQ.AI added in Phase 10 |
| VEN-02 | Fetch attestation from `GET /api/v1/tee/attestation?model=&nonce=` (public, no API key) | Spike 001 confirmed endpoint is public; capture in spike sources |
| VEN-03 | Verify Intel TDX quote signature, PCK chain, TCB, and CRLs client-side via `dcap-qvl` | Same path as existing TDX in `attestation/tdx.rs` |
| VEN-04 | Decode REPORTDATA with Venice layout (20B keccak-addr / 12B zeros / 32B nonce) and verify all three bindings | Spike findings document layout exactly; decoder is ~60 lines |
| VEN-05 | POST `nvidia_payload` to NRAS, verify returned JWT via existing `attestation/nvidia.rs` | `fetch_and_verify_nvidia` accepts `(payload, nonce_hex, backend_id)` — direct reuse |
| VEN-06 | Reject TDX quote in debug mode (`td_attributes[0] & 0x01 != 0`) | Spike requirement; gate before accepting attestation |
| VEN-07 | Establish ECDH(secp256k1) + HKDF-SHA256(`"ecdsa_encryption"`) + AES-256-GCM E2EE channel using attested signing key | Wire format: `[eph_pub 65B][nonce 12B][ct+tag]` hex; matches venice-cli reference |
| VEN-08 | Send and stream chat completions over the E2EE channel with `enable_e2ee: true` and three `X-Venice-TEE-*` headers | OpenAI-compatible chat completions API at `/api/v1/chat/completions`; SSE chunks contain hex envelopes |
| VEN-09 | Display Venice as a backend in Settings → Providers with `Verified` attestation badge once attestation passes | Reuses existing `AttestationStatus` enum and badge UI from Phase 10 (PPQ.AI integration) |

---

## Architecture Patterns

### System Architecture Diagram

```
                            User sends chat message
                                      │
                                      ▼
                  ┌──────────────────────────────────────┐
                  │        ActorState (Rust core)        │
                  │  - Selects backend (router)          │
                  │  - Resolves transport_kind() -> Venice E2ee
                  └──────────────────┬───────────────────┘
                                     │
                                     ▼
        ┌─────────────────── ensure_verified_venice_attestation ──────────────────┐
        │                                                                          │
        │  cache hit & not expired? ──yes──┐                                       │
        │                                  │                                       │
        │       no                         │                                       │
        │        │                         │                                       │
        │        ▼                         │                                       │
        │  generate 32B nonce              │                                       │
        │        │                         │                                       │
        │        ▼                         │                                       │
        │  GET api.venice.ai/api/v1/tee/   │                                       │
        │      attestation?model=&nonce=   │                                       │
        │  (public, unauthenticated)       │                                       │
        │        │                         │                                       │
        │        ▼                         │                                       │
        │  parse JSON: intel_quote,        │                                       │
        │  signing_public_key,             │                                       │
        │  nvidia_payload (string!),       │                                       │
        │  signing_address, nonce echo     │                                       │
        │        │                         │                                       │
        │        ├─ dcap_qvl::verify ──────┼──────► Phala PCCS / Intel PCS         │
        │        │  (TDX sig + chain +     │        (existing path)                │
        │        │   TCB + CRL)            │                                       │
        │        │                         │                                       │
        │        ├─ verify_venice_         │                                       │
        │        │  reportdata:            │                                       │
        │        │   • [0..20] = keccak256 │                                       │
        │        │     (pubkey[1..])[12..] │                                       │
        │        │   • [20..32] = zeros    │                                       │
        │        │   • [32..64] = nonce    │                                       │
        │        │                         │                                       │
        │        ├─ debug-mode reject:     │                                       │
        │        │   td_attributes[0]&0x01 │                                       │
        │        │                         │                                       │
        │        ├─ model echo == requested│                                       │
        │        │                         │                                       │
        │        ├─ parse nvidia_payload   │                                       │
        │        │  (JSON inside string!)  │                                       │
        │        │     │                   │                                       │
        │        │     ▼                   │                                       │
        │        │  fetch_and_verify_nvidia├──────► nras.attestation.nvidia.com    │
        │        │  (existing path)        │        /v3/attest/gpu                 │
        │        │                         │        /.well-known/jwks.json         │
        │        │                         │                                       │
        │        ▼                         ▼                                       │
        │  store VerifiedVeniceAttestation { request_base_url, signing_pubkey,    │
        │            nonce, attested_address, expires_at } in cache                │
        └──────────────────────────────────┬───────────────────────────────────────┘
                                           │
                                           ▼
        ┌──────── E2EE handshake (per chat-completion request) ────────┐
        │                                                                │
        │  generate ephemeral secp256k1 key (k256::ecdh::EphemeralSecret)│
        │  shared_secret = ECDH(eph_priv, attested_signing_pubkey)       │
        │  aes_key = HKDF-SHA256(shared_secret, info=b"ecdsa_encryption", 32) │
        │                                                                │
        │  for each user/system message body:                            │
        │      nonce_12 = rand(12)                                       │
        │      ct_tag   = AES-256-GCM(aes_key, nonce_12, body)           │
        │      envelope = hex([eph_pub_65B || nonce_12 || ct_tag])       │
        │      replace message content with envelope                     │
        │                                                                │
        │  request body = standard chat-completion JSON +                │
        │      "enable_e2ee": true                                       │
        │                                                                │
        │  request headers:                                              │
        │      X-Venice-TEE-Client-Pub-Key: <hex eph_pub 65B>            │
        │      X-Venice-TEE-Model-Pub-Key: <hex attested_pub 65B>        │
        │      X-Venice-TEE-Signing-Algo: ecdsa                          │
        │      Authorization: Bearer <api_key>                           │
        └──────────────────────────────┬───────────────────────────────────┘
                                       │
                                       ▼
        POST api.venice.ai/api/v1/chat/completions  (SSE stream)
                                       │
                                       ▼
        for each SSE delta chunk:
            parse `delta.content` as hex envelope
            split into [eph_pub_65B][nonce_12][ct+tag]
            decrypt ct+tag with same aes_key + chunk-nonce
            forward plaintext token to actor as InternalEvent::StreamChunk
```

### Recommended Project Structure

```
rust/src/
├── attestation/
│   ├── tdx.rs              # EXTEND: parameterise verify_tdx_quote with ReportDataLayout enum
│   │                       #         OR add verify_tdx_quote_venice sibling fn
│   ├── nvidia.rs           # REUSE unmodified — fetch_and_verify_nvidia(payload, nonce_hex, id)
│   ├── venice.rs           # NEW: Venice REPORTDATA layout decoder, address binding check,
│   │                       #      VerifiedVeniceAttestation struct + cache
│   └── mod.rs              # add pub mod venice;
├── llm/
│   ├── venice.rs           # NEW: HTTP transport, E2EE handshake, request/response wrappers,
│   │                       #      streaming SSE decoder. Mirror ppq_private.rs structure.
│   ├── backend.rs          # EXTEND: ProviderKind::Venice, known_provider_presets entry,
│   │                       #         transport_kind() match arm
│   ├── transport.rs        # EXTEND: ProviderTransportKind::VeniceE2ee variant
│   │                       #         + build_reqwest_client + model_list_url arms
│   └── mod.rs              # add pub mod venice;
└── tests/
    ├── attestation_venice.rs   # NEW: REPORTDATA decoder unit tests using golden capture
    │                           #      (.claude/skills/.../captures/attestation-sample.json)
    └── venice.rs               # NEW: ECDH round-trip, AES-GCM envelope round-trip,
                                #      live capture-replay attestation parse
```

### Pattern 1: Venice attestation flow (mirrors ppq_private.rs)

**What:** Single async function that fetches, verifies, and caches the attestation; returns a `VerifiedVeniceAttestation` carrying the trusted signing pubkey for downstream E2EE handshakes.

**When to use:** Called from both the periodic attestation task (`AttestationTick`) and the request path (cache lookup before each request).

**Reference:** `rust/src/llm/ppq_private.rs::ensure_verified_attestation` (lines 759-790) — same shape, different verifier.

```rust
// Source: pattern from ppq_private.rs + spike 001 references/venice-attestation.md
async fn ensure_verified_venice_attestation(
    backend: &BackendConfig,
    requested_model: &str,
    tdx_policy: &crate::attestation::TdxPolicy,
) -> Result<VerifiedVeniceAttestation, LlmError> {
    // 1. Cache lookup with TTL (mirror ppq_private.rs::ensure_verified_attestation)
    // 2. On miss: rand 32B nonce -> hex
    // 3. GET https://api.venice.ai/api/v1/tee/attestation?model=&nonce=
    // 4. Deserialize VeniceAttestation { intel_quote, nvidia_payload, signing_public_key (alias signing_key), signing_address, nonce, model, ... }
    // 5. quote_bytes = hex::decode(intel_quote)
    // 6. dcap_qvl collateral fetch + dcap_qvl::verify::verify (same as attestation/tdx.rs but layout differs at REPORTDATA)
    // 7. extract REPORTDATA, run verify_venice_report_data(report_data, signing_pubkey, &nonce)
    // 8. debug-mode gate: td_attributes[0] & 0x01 == 0
    // 9. model echo gate: resp.model == requested_model
    // 10. parse nvidia_payload (string -> JSON), call fetch_and_verify_nvidia
    // 11. log (NOT trust) server_verification.{tdx,nvidia}.valid
    // 12. cache + return
}
```

### Pattern 2: REPORTDATA decoder (Venice-specific)

```rust
// Source: spike 001 references/venice-attestation.md §4 (verbatim from spike)
fn verify_venice_report_data(
    report_data: &[u8; 64],
    signing_pubkey_hex: &str,   // 130 hex, "04..."
    submitted_nonce: &[u8; 32],
) -> Result<(), AttestationError> {
    // Address binding: keccak256(pubkey_xy)[12..32] == report_data[0..20]
    let pubkey = hex::decode(signing_pubkey_hex)?;
    if pubkey.len() != 65 || pubkey[0] != 0x04 {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not in uncompressed secp256k1 form".into()
        });
    }
    use sha3::{Digest, Keccak256};
    let mut h = Keccak256::new();
    h.update(&pubkey[1..]);
    let addr20 = &h.finalize()[12..32];
    if addr20 != &report_data[0..20] {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not bound to TDX REPORTDATA[0..20]".into()
        });
    }
    if report_data[20..32].iter().any(|&b| b != 0) {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice REPORTDATA[20..32] padding non-zero".into()
        });
    }
    if &report_data[32..64] != &submitted_nonce[..] {
        return Err(AttestationError::NonceMismatch {
            expected: hex::encode(submitted_nonce),
            actual: hex::encode(&report_data[32..64]),
        });
    }
    Ok(())
}
```

### Pattern 3: ECDH + HKDF + AES-GCM session establishment

```rust
// Source: spike 001 references/venice-attestation.md §7 + venice-cli/src/lib/e2ee.ts
use k256::{ecdh::EphemeralSecret, EncodedPoint, PublicKey};
use hkdf::Hkdf;
use sha2::Sha256;
use aes_gcm::{Aes256Gcm, Nonce, aead::{Aead, KeyInit}};
use rand::thread_rng;

let eph_secret = EphemeralSecret::random(&mut thread_rng());
let eph_pub = eph_secret.public_key();
let attested_pub = PublicKey::from_sec1_bytes(&attested_pub_bytes_65)
    .map_err(|_| LlmError::NetworkError { reason: "invalid Venice signing pubkey".into() })?;
let shared = eph_secret.diffie_hellman(&attested_pub);
let mut aes_key = [0u8; 32];
Hkdf::<Sha256>::new(None, shared.raw_secret_bytes())
    .expand(b"ecdsa_encryption", &mut aes_key)?;

// Per-message:
let cipher = Aes256Gcm::new_from_slice(&aes_key)?;
let mut nonce_12 = [0u8; 12]; rand::thread_rng().fill(&mut nonce_12);
let ct_tag = cipher.encrypt(Nonce::from_slice(&nonce_12), plaintext.as_bytes())?;
let eph_pub_uncompressed = eph_pub.to_encoded_point(false);  // 65 bytes, "04..."
let mut envelope = Vec::with_capacity(65 + 12 + ct_tag.len());
envelope.extend_from_slice(eph_pub_uncompressed.as_bytes());
envelope.extend_from_slice(&nonce_12);
envelope.extend_from_slice(&ct_tag);
let envelope_hex = hex::encode(&envelope);
```

### Pattern 4: Wrapping the OpenAI-compatible chat completion

The Venice path is **not** a custom request — it's a standard OpenAI chat completion with two wrappers:
1. **Body-level:** Replace each user/system message's `content` (or each text part of a multipart `Array`) with the hex envelope. Add `"enable_e2ee": true` to the top-level body. Set `"stream": true` for SSE.
2. **Header-level:** Add the three `X-Venice-TEE-*` headers + `Authorization: Bearer <api_key>`.

This is structurally analogous to `ppq_private::build_private_chat_body` (which only swaps the model name) — extend the same idea to encrypt the message contents instead.

**SSE response decoding:** Each `data: { ... }` chunk's `delta.content` field contains a hex envelope of the same `[eph_pub|nonce|ct+tag]` shape. Decrypt with the same `aes_key` (and use the per-message nonce embedded in the envelope, NOT a chunk-counter scheme like ppq_private/tinfoil_secure use — Venice's framing is per-message, server-generated nonces).

### Anti-Patterns to Avoid

- **Trusting `server_verification.tdx.valid` / `.nvidia.valid` / `.nonceBinding.bound`.** These booleans are fine to log and surface in UI as a soft hint but the client MUST re-verify everything. Spike-locked.
- **Hashing the nonce.** Venice uses `nonceBinding.method: "raw"`. Don't sha256/keccak the nonce before comparing.
- **Treating `nvidia_payload` as a JSON object.** It's a JSON-encoded **string** — must `serde_json::from_str` the string before forwarding to NRAS.
- **Forgetting the field-name fallback.** Live API returns `signing_public_key`; some docs/tools call it `signing_key`. Use `#[serde(alias = "signing_public_key")]` on a `signing_key` field, or vice versa.
- **Skipping the debug-mode check.** `td_attributes[0] & 0x01 != 0` means TDX is in debug — zero confidentiality. Reject.
- **Building a custom DCAP parser.** `dcap-qvl::verify::verify` already does signature + cert chain + TCB + CRL.
- **Reusing `verify_tdx_quote` from `attestation/tdx.rs` directly.** It hard-codes `nonce_in_report = report_data[..32]`. Either parameterise or fork.
- **Reusing PPQ.AI's chunk-counter SSE decryption pattern.** Venice's per-message envelope already includes its own nonce; the framing is different.
- **Caching attestation across application restarts.** Per-request nonce model means each fresh session must re-attest. The cache should live for the lifetime of an unbroken E2EE session only.
- **Treating the `info.app_cert` X.509 envelope as the verification root.** It embeds the same TDX quote but in a Phala-dstack-specific custom extension. The raw `intel_quote` field is sufficient — don't add a Phala dstack dep.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Intel TDX DCAP v4 quote signature + cert chain + TCB + CRL verification | Custom parser from quote header offsets | `dcap-qvl::verify::verify` (already in stack) | Spike confirmed Venice quotes are standard DCAP v4. Custom parsers have a long history of CVE-grade bugs. |
| NVIDIA NRAS JWT signature verification | Manual JWT base64-split + RSA verify | `attestation/nvidia.rs::fetch_and_verify_nvidia` (already in code) | Algorithm pinning + issuer pinning + JWKS fetch is already implemented and reviewed. |
| secp256k1 ECDH | Custom curve arithmetic | `k256` crate (RustCrypto) `ecdh::EphemeralSecret::diffie_hellman` | Constant-time, audited, no FFI. |
| AES-256-GCM | OpenSSL AEAD | `aes-gcm` crate (already in stack) | Pure Rust, RustCrypto, already used by Tinfoil/PPQ paths. |
| HKDF-SHA256 | Manual HMAC chaining | `hkdf` crate (already in stack) | Already used in PPQ path. |
| Keccak256 | Hand-rolled Keccak | `sha3` crate | Keccak is hard to implement correctly. |
| Server-side attestation policy / TCB minimum thresholds | Hard-code in venice.rs | Reuse existing `TdxPolicy` from `attestation/policy.rs` | Phase 18 already runtime-configures TDX policy via the `settings` table. Apply the same policy to Venice quotes. |
| TLS pinning to attested cert | Custom TLS config | Existing `pinned_reqwest_client` in `net/tls.rs` (used by transport.rs) | Already integrated into the transport pipeline. **Note:** Venice doesn't pin TLS to an attested cert — the trust root is the secp256k1 signing key in REPORTDATA, not the TLS cert. Don't try to pin TLS for Venice. |

**Key insight:** Custom crypto in this domain is the highest-risk code in the entire app. Every primitive Venice needs is already in our crate set or sits behind a one-line `cargo add`.

---

## Common Pitfalls

### Pitfall 1: REPORTDATA layout mismatch silently passing
**What goes wrong:** Reusing `attestation/tdx.rs::verify_tdx_quote` for Venice will compare `report_data[..32]` to the nonce. Venice puts the nonce at `[32..64]` and the **address** at `[..32]` (well, at `[..20]` with `[20..32]` zeros). The address bytes differ from the nonce bytes, so verification will fail with `NonceMismatch` — but a future refactor that normalises the comparison could mask the bug.
**Why it happens:** Different TEE substrates / providers use different REPORTDATA conventions, and there is no "canonical" layout enforced by Intel TDX itself — it's 64 free-form bytes the workload chooses how to fill.
**How to avoid:** Make the REPORTDATA layout an explicit, named parameter to the TDX verify function. Use a `ReportDataLayout` enum (e.g., `NonceFirst32`, `VeniceAddrPadNonce`) so you cannot call the function without choosing one.
**Warning signs:** Tests pass with hand-rolled fixtures but fail against the live capture; the live capture's `[32..64]` bytes match the submitted nonce but `[..32]` bytes look "random" (they're an Ethereum address).

### Pitfall 2: Double-JSON-parse of `nvidia_payload`
**What goes wrong:** `serde_json` parses Venice's response and returns `nvidia_payload: Option<Value>`. The value is a `Value::String("{\"nonce\":...,\"evidence_list\":[...]}")` — a JSON string containing JSON, not a nested object. Forwarding the `Value::String` directly to NRAS sends the literal string `"{\"nonce\":...}"` (with escaped quotes) and NRAS rejects it.
**Why it happens:** Venice's serializer wraps `nvidia_payload` in JSON.stringify() once before embedding it. Their reference CLI does `JSON.parse(payload)` before forwarding.
**How to avoid:** Type the field as `Option<String>`, then do `serde_json::from_str::<NvidiaPayload>(payload_str)?` to get the parsed struct.
**Warning signs:** NRAS returns 400 with "invalid evidence format" or similar; forwarded body contains escaped JSON.

### Pitfall 3: `signing_key` vs `signing_public_key` field-name drift
**What goes wrong:** Docs and the venice-cli switch between two field names. Live API returns `signing_public_key`; some docs say `signing_key`. Hard-coding either name will break against the other.
**Why it happens:** Field-name rename in flight or unsynchronised docs.
**How to avoid:** `#[serde(alias = "signing_public_key")]` on the Rust struct field. The spike sketch already does this.
**Warning signs:** Deserialization fails with "missing field signing_key" against a live response that has `signing_public_key`, or vice versa.

### Pitfall 4: `enable_e2ee: true` only via chat completions API, not Responses API
**What goes wrong:** Venice has a newer Responses API (alpha) that does NOT support E2EE. Using it silently sends plaintext.
**Why it happens:** OpenAI added the Responses API; Venice followed. E2EE wasn't ported.
**How to avoid:** Use only `/api/v1/chat/completions`. Never wire Venice into a Responses-API code path.
**Warning signs:** Server returns plaintext SSE (no hex envelope) — but the request appeared to set `enable_e2ee: true`.

### Pitfall 5: Caching attestation across cold launches
**What goes wrong:** Persisting the attestation result + signing key across an app restart, then resuming an "encrypted" session. The client supplies a fresh nonce each call, and the `[32..64]` REPORTDATA region binds to that nonce — re-using a stale attestation against a fresh server instance breaks confidentiality.
**Why it happens:** Tempting to avoid the ~1 second extra latency of a fresh attestation on every cold launch.
**How to avoid:** Per-session in-memory cache only (`Lazy<Mutex<HashMap<...>>>`). Do not write Venice attestation results to SQLite. Re-attest on any reconnect or app foreground after >1 minute background.
**Warning signs:** Cache survives an app cold launch; persisted attestation_records SQLite rows for Venice backend.

### Pitfall 6: `td_attributes[0] & 0x01` debug bit ignored
**What goes wrong:** A debug TDX VM provides zero confidentiality guarantee — a debugger can read all enclave memory. Accepting an attestation with the debug bit set defeats the entire feature.
**Why it happens:** The bit is buried inside the parsed report struct; easy to miss.
**How to avoid:** Explicit gate after `dcap_qvl::verify::verify` succeeds. Spike provides the exact check.
**Warning signs:** A "test" Venice deployment streams content with debug enabled; client accepts it without warning.

### Pitfall 7: AES-GCM nonce reuse
**What goes wrong:** AES-GCM is catastrophically broken if the same `(key, nonce)` pair is reused. If we reuse `aes_key` across requests in a session and seed `nonce_12` from a counter or fixed value, an adversary can XOR ciphertexts and recover plaintext.
**Why it happens:** Tempting to derive `nonce` from a counter for stream framing.
**How to avoid:** Generate `nonce_12` fresh from `OsRng` per message. The spike-confirmed wire format embeds the nonce in the envelope, so the server always sees the right nonce.
**Warning signs:** Same `nonce` byte-equal across two messages with the same session key.

### Pitfall 8: Streaming SSE chunk parsing diverges from PPQ pattern
**What goes wrong:** Copying `ppq_private::stream_decrypted_sse` verbatim — that path uses a length-prefixed binary frame format and a counter-derived chunk nonce. Venice's SSE chunks are standard text `data: {...}\n\n` frames whose `delta.content` field carries a per-message hex envelope with its own nonce.
**Why it happens:** Plausible code-reuse trap.
**How to avoid:** Mirror the SSE plumbing of `tinfoil_secure::handle_sse_event` (text-based SSE) rather than `ppq_private::try_take_frame` (binary length-prefixed). Decryption nonce comes from the envelope, not a counter.
**Warning signs:** Decryption fails with `aead::Error` on every chunk; counter-based nonce computation appears in the code.

---

## Code Examples

### Example 1: Verified Venice attestation cache entry

```rust
// Source: shape from ppq_private::VerifiedPpqAttestation, fields from spike 001
#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
struct VerifiedVeniceAttestation {
    #[zeroize(skip)]
    request_base_url: String,             // "https://api.venice.ai/api/v1"
    #[zeroize(skip)]
    model: String,                         // "e2ee-venice-uncensored-24b-p"
    signing_pubkey_uncompressed: [u8; 65], // attested secp256k1 pubkey, 04-prefixed
    submitted_nonce: [u8; 32],
    #[zeroize(skip)]
    report_blob: Vec<u8>,                  // raw TDX quote bytes
    #[zeroize(skip)]
    expires_at: u64,                       // unix-secs; per-session, NOT persisted
}
```

### Example 2: Provider preset registration

```rust
// Source: extend rust/src/llm/backend.rs::known_provider_presets
ProviderPreset {
    id: "venice-ai".into(),
    name: "Venice.ai".into(),
    base_url: "https://api.venice.ai/api/v1/".into(),
    tee_type: TeeType::IntelTdx,  // primary; NVIDIA-CC verified separately like Tinfoil
    description: "Intel TDX + NVIDIA H100 CC \u{00b7} E2EE chat".into(),
}
```

### Example 3: ProviderTransportKind extension

```rust
// Source: extend rust/src/llm/transport.rs::ProviderTransportKind
pub enum ProviderTransportKind {
    OpenAiCompatible,
    TinfoilSecure,
    PpqPrivateE2ee,
    VeniceE2ee,  // NEW
}

impl ProviderTransportKind {
    pub fn for_backend(backend: &BackendConfig) -> Self {
        if backend.provider_kind() == ProviderKind::Tinfoil { return Self::TinfoilSecure; }
        if backend.provider_kind() == ProviderKind::Venice { return Self::VeniceE2ee; }
        // ... existing PPQ branch ...
    }
}
```

---

## Design Decisions to Lock

These are choices the planner (or a discuss-phase) MUST resolve before writing tasks. Each has a research-supported recommendation but is not yet user-locked.

| # | Decision | Recommendation | Open Variants |
|---|----------|----------------|---------------|
| D1 | TDX REPORTDATA layout abstraction | Parameterise `attestation/tdx.rs::verify_tdx_quote` with a `ReportDataLayout` enum (`NonceFirst32`, `VeniceAddrPadNonce`); refactor existing call sites to pass the layout explicitly. | (a) parameterise (recommended), (b) fork into `verify_tdx_quote_venice` sibling function. |
| D2 | Where REPORTDATA decoder lives | New `attestation/venice.rs` module — symmetric with `attestation/nvidia.rs` and easier to find. | (a) new module (recommended), (b) inline in `llm/venice.rs`. |
| D3 | Whether to persist Venice attestation results to the SQLite `attestation_records` table | NO — per-session only (in-memory `Lazy<Mutex<HashMap>>`). Aligns with PPQ pattern and the spike's per-request-nonce design. | (a) in-memory only (recommended), (b) persist with short TTL — risks Pitfall 5. |
| D4 | secp256k1 crate choice | `k256` (RustCrypto, pure Rust). | (a) `k256` (recommended), (b) `secp256k1` (libsecp256k1 FFI — breaks mobile builds, rejected). |
| D5 | E2EE-encrypt scope per request | User and system message bodies only (per spike + venice-cli). Tools/tool-results encrypted on the same pattern if present. Top-level fields (`model`, `temperature`, `enable_e2ee`, etc.) plaintext. | (a) bodies-only (recommended); (b) whole-body opaque encryption — diverges from spec. |
| D6 | Where ephemeral key lifetime ends | Per-request (new `EphemeralSecret` for each chat completion), not per-session. Safer against compromise; spike does not require session reuse. | (a) per-request (recommended), (b) per-session (small latency win). |
| D7 | Whether to support tool calling on Venice in v1 | Yes for plaintext fields (`tools` JSON, function names) — tool message bodies that contain user content go through E2EE same as user messages. Verify via test against Venice. **Risk:** unverified — spike did not test tool calling. | (a) defer (safe), (b) include in v1 (more upside; needs live verification). |
| D8 | Whether the chat tool-use toggle path (Phase 27) is in scope for Venice | Defer to a follow-up phase. Phase 33 ships streaming chat only. | (a) defer (recommended), (b) integrate now — adds scope. |
| D9 | Multimodal (Phase 31 image attachments) on Venice | Defer; treat as a follow-up. Many TEE-attested models are text-only; spike did not validate vision. | (a) defer (recommended), (b) include — needs Venice support confirmation. |
| D10 | Streaming `[DONE]` framing | Same as Tinfoil text-SSE pattern (`take_sse_event` over `\n\n` separators). | (a) text-SSE (recommended); (b) length-prefixed binary — wrong shape per spike. |
| D11 | Attestation status downgrade rules | Treat Venice attestation failures as **non-transient** for cryptographic failures (REPORTDATA mismatch, NRAS JWT invalid, signing-key-not-bound) — downgrade `Verified -> Failed`. Treat collateral fetch / NRAS network errors as transient — preserve `Verified`. Mirrors existing `AttestationEvent::Failed { is_transient }` discipline. | (a) follow existing pattern (recommended); (b) custom rules per provider — unnecessary complexity. |
| D12 | UI badge wording for Venice | "Verified — Intel TDX + NVIDIA H100 CC + E2EE" (or similar). | (a) reuse Tinfoil wording with E2EE suffix (recommended); (b) introduce a new badge variant — unneeded. |
| D13 | Settings UI placement | Goes into Settings → Providers (sub-screen added in Phase 26). Same row pattern as PPQ.AI. | (a) Providers sub-screen (recommended). |
| D14 | API key required? | NO for attestation (public endpoint). YES for chat completions (Bearer token). User configures the key in the standard provider key form. | Locked by spike — endpoint shape is fixed. |
| D15 | TDX policy applied to Venice quotes | Reuse existing `TdxPolicy` from `attestation/policy.rs`. Confirm Phala dstack measurements are in `accepted_mr_seams` or extend the seed list. **ACTION:** parse the live capture's MRSEAM, compare to the existing seeded list. If absent, decide whether to add or to ship a Venice-specific policy. | (a) reuse existing policy (recommended; verify MRSEAM coverage); (b) new VenicePolicy struct. |

---

## Runtime State Inventory

> Greenfield phase — no rename / refactor / migration. Section omitted.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` (Rust toolchain) | All Rust changes | ✓ | per `flake.nix` pin | — |
| Venice.ai live attestation endpoint (`api.venice.ai`) | Live integration tests, manual verification | ✓ (spike confirmed live capture available) | n/a | golden capture in `.claude/skills/spike-findings-confidential-app/sources/001-venice-tee-protocol-research/captures/attestation-sample.json` for offline tests |
| Phala PCCS (`pccs.phala.network`) — TDX collateral | TDX quote verification | ✓ (existing path uses it) | n/a | Intel PCS direct (`get_collateral_from_intel_pcs`) — slower, rate-limited |
| Intel PCS — TDX collateral fallback | Fallback when Phala PCCS unreachable | ✓ | n/a | none — both must be unreachable to fail |
| NVIDIA NRAS (`nras.attestation.nvidia.com`) | NVIDIA GPU attestation | ✓ (existing path uses it) | n/a | none — NRAS is the canonical NVIDIA attestation root |
| `k256` crate | E2EE handshake | ✗ (not yet in Cargo.toml) | n/a (target 0.13.x) | — |
| `sha3` crate | Address binding | ✗ (not yet in Cargo.toml) | n/a (target 0.10.x) | — |
| `urlencoding` crate | URL query encoding | ✗ (not yet in Cargo.toml) | n/a (target 2.1.x) | manual percent-encoding |
| Venice TEE-capable model availability | Live testing | ✓ (`e2ee-venice-uncensored-24b-p`, `e2ee-glm-4-7-p`, `e2ee-qwen3-30b-a3b-p` listed in spike) | n/a | use captured response for unit tests; live tests can run offline |
| Venice API key for chat completions | Live integration tests of E2EE chat path | ⚠ (user-supplied, not in CI) | n/a | unit tests only via mock endpoints + golden capture |

**Missing dependencies with fallback:** All three new crates have `cargo add` as the install path; manual percent-encoding is a viable fallback for `urlencoding`. No blockers.

**Missing dependencies with no fallback:** None. The Venice live attestation endpoint is required for live integration tests, but unit/contract tests against the golden capture cover ~all verification logic.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` + `tokio-test` (already in use across `rust/src/tests/`) |
| Config file | None — Cargo workspace |
| Quick run command | `cargo test -p mango_core --lib venice` |
| Full suite command | `cargo test -p mango_core` |

### Phase Requirements → Test Map

(Using the suggested VEN-* IDs from the §Phase Requirements section; planner may renumber.)

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| VEN-01 | Venice preset present in `known_provider_presets()` | unit | `cargo test -p mango_core --lib backend::tests::venice_preset_present` | ❌ Wave 0 |
| VEN-02 | Attestation URL builder produces correct `?model=&nonce=` query string with hex nonce | unit | `cargo test -p mango_core --lib venice::tests::attestation_url_format` | ❌ Wave 0 |
| VEN-03 | TDX quote from golden capture verifies via `dcap-qvl` (using mocked collateral) | integration | `cargo test -p mango_core --lib venice::tests::tdx_verify_golden_capture` | ❌ Wave 0 |
| VEN-04a | REPORTDATA decoder accepts the golden-capture layout | unit | `cargo test -p mango_core --lib venice::tests::reportdata_layout_ok` | ❌ Wave 0 |
| VEN-04b | REPORTDATA decoder rejects mismatched address | unit | `cargo test -p mango_core --lib venice::tests::reportdata_address_mismatch` | ❌ Wave 0 |
| VEN-04c | REPORTDATA decoder rejects mismatched nonce | unit | `cargo test -p mango_core --lib venice::tests::reportdata_nonce_mismatch` | ❌ Wave 0 |
| VEN-04d | REPORTDATA decoder rejects non-zero padding | unit | `cargo test -p mango_core --lib venice::tests::reportdata_padding_nonzero` | ❌ Wave 0 |
| VEN-05 | NRAS payload extracted from JSON-string field and forwarded correctly | unit (mock NRAS server) | `cargo test -p mango_core --lib venice::tests::nvidia_payload_double_parse` | ❌ Wave 0 |
| VEN-06 | TDX debug-mode bit triggers rejection | unit | `cargo test -p mango_core --lib venice::tests::tdx_debug_bit_rejected` | ❌ Wave 0 |
| VEN-07a | ECDH-derived AES key matches across encrypt/decrypt round-trip | unit | `cargo test -p mango_core --lib venice::tests::ecdh_aes_round_trip` | ❌ Wave 0 |
| VEN-07b | AES-GCM envelope round-trip with deterministic test vectors | unit | `cargo test -p mango_core --lib venice::tests::envelope_round_trip` | ❌ Wave 0 |
| VEN-08 | Chat completion request body has `enable_e2ee: true` and message bodies are hex envelopes | unit (build + inspect) | `cargo test -p mango_core --lib venice::tests::request_body_shape` | ❌ Wave 0 |
| VEN-09 | Provider preset surfaces in `BackendSummary` after add | integration | `cargo test -p mango_core --lib venice::tests::backend_summary` | ❌ Wave 0 |
| VEN-LIVE | Live attestation against `api.venice.ai` (manual / skipped in CI) | integration (gated) | `cargo test -p mango_core --lib live_venice -- --ignored` | ❌ Wave 0 (mark `#[ignore]`) |

### Sampling Rate
- **Per task commit:** `cargo test -p mango_core --lib venice` (~1-3s)
- **Per wave merge:** `cargo test -p mango_core` (~30-60s)
- **Phase gate:** Full suite green before `/gsd-verify-work`; live test (`VEN-LIVE`) run manually and noted in VERIFICATION.md.

### Wave 0 Gaps
- [ ] `rust/src/tests/venice.rs` — covers VEN-01..09 (unit + integration with golden capture)
- [ ] `rust/src/tests/attestation_venice.rs` — covers REPORTDATA decoder edge cases against captured fixture
- [ ] `rust/src/tests/live_venice.rs` (with `#[ignore]`) — gated live test against `api.venice.ai`
- [ ] Add golden capture as test fixture: copy `attestation-sample.json` into `rust/tests/fixtures/venice-attestation-sample.json` (or load via `include_str!` from skill path)
- [ ] Mock NRAS HTTP server pattern: existing tests do not appear to mock NRAS (search for `mockito` / `wiremock` shows none in Cargo.toml). Decision: either (a) add `wiremock` dev-dependency, or (b) test the JWT verification path with a hand-crafted JWT signed by a test RSA key. Recommend (b) — simpler, no new dep.

**What CAN be validated automatically:**
- All cryptographic decoding/binding logic (against golden capture)
- ECDH/HKDF/AES-GCM round-trip
- Request shape (envelope, headers, body)
- Error paths (debug mode, bad nonce, bad address, malformed JSON)

**What requires real Venice endpoints:**
- `dcap-qvl::collateral::get_collateral_from_intel_pcs` against a fresh capture (collateral has freshness windows)
- NRAS JWT signed by NVIDIA's actual RSA key
- End-to-end chat completion E2EE round-trip

**What CANNOT be validated without hardware:**
- TCB level meeting `TdxPolicy::minimum_tee_tcb_svn` against a particular Phala dstack image
- `accepted_mr_seams` containing the live Venice MRSEAM (must be checked against the capture and updated if needed)

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | yes | API key in `Authorization: Bearer` header (existing pattern). Attestation endpoint is public — no auth. |
| V3 Session Management | yes | Per-request ephemeral ECDH key (D6); per-session in-memory attestation cache (D3). No persistent session state. |
| V4 Access Control | partial | Backend gating in router; user enables/disables provider via Settings. No user-level RBAC needed. |
| V5 Input Validation | yes | Validate every wire-format field length (intel_quote min 48B, signing key 65B with `04` prefix, REPORTDATA exactly 64B, nonce exactly 32B). |
| V6 Cryptography | yes | All primitives via RustCrypto crates; no hand-rolled crypto. zeroize on key material. AES-GCM nonce uniqueness enforced (Pitfall 7). |
| V7 Error Handling | yes | Map every cryptographic failure to `AttestationError` taxonomy (existing). Never leak key material into log strings. |
| V8 Data Protection | yes | DEK / app-data encryption is independent (Phase 28). E2EE channel encrypts in-transit message bodies. |
| V9 Communication | yes | rustls-tls only (CLAUDE.md mandate). No TLS pinning required for Venice — trust root is the attested signing key, not the TLS leaf. |

### Known Threat Patterns for {Intel TDX + NVIDIA NRAS + secp256k1 ECDH + AES-256-GCM}

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Replay of stale attestation against a different / compromised model instance | Spoofing | Per-request nonce + REPORTDATA[32..64] binding (VEN-04c); per-session cache only (D3) |
| Substitution of attestation for a different model | Spoofing / Tampering | Verify `resp.model == requested_model` (VEN-VEN-06 sanity gate from spike §6) |
| Substitution of E2EE handshake key not rooted in the TEE | Spoofing | REPORTDATA[0..20] = keccak256(signing_pubkey)[12..32] binding (VEN-04a); use only the attested `signing_public_key` for ECDH (D5) |
| Use of TDX in debug mode (zero confidentiality) | Information Disclosure | `td_attributes[0] & 0x01 == 0` check (VEN-06) |
| Trust of provider-supplied "verified: true" booleans | Spoofing | Spike-locked: never trust; always re-verify |
| AES-GCM nonce reuse | Information Disclosure / Tampering | Fresh `OsRng` 12B nonce per message (Pitfall 7) |
| TLS MITM | Information Disclosure | rustls + webpki-roots; trust root for content is the attested signing key not TLS — TLS MITM cannot decrypt E2EE bodies |
| Forged NRAS JWT (algorithm confusion) | Spoofing | Existing `attestation/nvidia.rs` pins algorithm to RS256 and issuer to `https://nras.attestation.nvidia.com` — reused unmodified |
| Stale NRAS JWT replay | Spoofing | NRAS JWT TTL = 1 hour (existing `fetch_and_verify_nvidia` policy); `eat_nonce` matches submitted nonce |
| Collateral freshness drift / TCB downgrade | Tampering | `dcap-qvl` validates collateral against current timestamp; `TdxPolicy::minimum_tee_tcb_svn` floor enforced |
| Attestation cache TTL bypass | Spoofing / Replay | Per-session in-memory cache (D3); evict + re-fetch on any reconnect / extended background |
| Server-supplied padding bytes used as a covert channel | Information Disclosure | Reject any `REPORTDATA[20..32]` that is non-zero (VEN-04d) |

### Project-specific notes

- The spike's `<requirements>` list IS the security spec for this phase. All four bullets are non-negotiable and verifiable against the golden capture.
- The existing `AttestationStatus::Failed { reason }` enum is sufficient for surfacing every cryptographic failure mode to the UI without leaking key material — verify in the planner that error reason strings never include raw secret bytes.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Trust provider's `verified: true` boolean | Independently re-verify quote signature, cert chain, REPORTDATA bindings, NRAS JWT | Phase 3 (existing TDX/NVIDIA path) | Foundational; Phase 33 inherits this discipline |
| Custom DCAP parser | `dcap-qvl` (Flashbots, pure Rust) | Phase 3 | Eliminates per-provider parser code |
| C `libsecp256k1` (FFI) | `k256` (RustCrypto, pure Rust) | Phase 33 (this phase) | Mobile-buildable, no OpenSSL/C linking |
| Per-provider HPKE config | Provider-specific transport modules under `llm/` (tinfoil_secure, ppq_private, venice) | Phase 10 (PPQ.AI added) | Each provider owns its E2EE shape |

**Deprecated/outdated:**
- Trusting `server_verification.*` booleans for any cryptographic property — the spike explicitly rejects this for Venice.
- The Phala dstack `info.app_cert` envelope path — embeds the same TDX quote in a custom X.509 extension; we use the raw `intel_quote` field directly and ignore the envelope.

---

## Assumptions Log

> Claims tagged `[ASSUMED]` that the planner / discuss-phase should validate with the user.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `k256` 0.13.x and `sha3` 0.10.x are appropriate version pins | Standard Stack | Low — easily corrected at plan time via `cargo info`; if `k256` 0.13 lacks a needed feature, fall back to a more recent line |
| A2 | `k256` features `ecdh, arithmetic, std` (default-features=false) suffice for `EphemeralSecret::diffie_hellman` + `PublicKey::from_sec1_bytes` | Standard Stack — Installation | Low — feature-set is verifiable via `cargo info k256`; cost is one feature-flag adjustment |
| A3 | Phala dstack MRSEAM measurement(s) for Venice's running image are already in the existing `TdxPolicy::accepted_mr_seams` seed list | Design Decisions D15 | MEDIUM — if not, attestation will fail until the policy is updated; verify against the golden capture as part of Wave 0 |
| A4 | Live capture-extracted MRSEAM matches what Venice runs in production today | Design Decisions D15 | MEDIUM — Phala dstack image versions update; production MRSEAM may differ from spike capture |
| A5 | Venice's tool-calling support over E2EE is functional in v1 | D7, VEN-08 | MEDIUM — spike did not validate; plan should include a live verification step or defer tools to a follow-up |
| A6 | Venice's vision (multimodal) support over E2EE matches OpenAI multipart shape | D9 | LOW (deferred); not in v1 scope per recommendation |
| A7 | Venice attestation per-session caching (in-memory only, not persisted) does not violate any product UX expectation | D3 | LOW — re-attestation is ~1s; no UX regression vs cold launch of any other provider |
| A8 | `chrono::Utc::now().timestamp()` clock is monotonic enough for collateral freshness checks (no NTP drift > 4h) | Pattern 1 (cache TTL) | LOW — same assumption as existing TDX path |
| A9 | Venice will not change the REPORTDATA layout in a way that breaks the spike-confirmed `[20B addr][12B 0][32B nonce]` schema | All REPORTDATA logic | LOW — schema is keccak-bound to a published Ethereum-style address derivation; changing it would break their own CLI |
| A10 | The existing `verify_tdx_quote` callers in `attestation/tdx.rs` can be refactored to take a `ReportDataLayout` parameter without breaking Tinfoil/PPQ tests | D1 | LOW — refactor is mechanical; existing tests cover the `NonceFirst32` path |

---

## Open Questions (RESOLVED)

1. **Does Venice's Phala dstack image MRSEAM appear in the project's existing `accepted_mr_seams` list?**
   - What we know: Spike capture exposes the live MRSEAM via `dcap-qvl` parse of `intel_quote`.
   - What's unclear: Whether that exact bytes appears in `attestation/policy.rs::TdxPolicy::default()::accepted_mr_seams`.
   - Recommendation: Wave 0 task — write a one-shot test that parses the golden capture and prints MRSEAM hex; compare to the seed list. If absent, decide between extending the global list or shipping a Venice-scoped policy.
   - RESOLVED: Plan 01 Task 3 reconciles MRSEAM against the golden capture and writes the verdict to `33-MRSEAM-RECONCILE.md` (extend `accepted_mr_seams` or ship a Venice-scoped policy as the reconcile output dictates).

2. **Does Venice support tool calling and/or vision over the E2EE path?**
   - What we know: OpenAI-compatible API supports both at the wire level; Venice docs and `venice-cli` reference do not exercise either through E2EE.
   - What's unclear: Live behavior under `enable_e2ee: true`.
   - Recommendation: Defer both to follow-up phases (D7, D9). Live-verify in a spike before re-opening.
   - RESOLVED: Deferred per discretion D7 (tools) and D9 (vision/multimodal) — out of scope for Phase 33. `build_venice_chat_body` rejects multipart content with an explicit `LlmError::NetworkError` so any caller attempting vision-over-E2EE fails closed.

3. **What's the rate limit on `GET /api/v1/tee/attestation`?**
   - What we know: Endpoint is public/unauthenticated.
   - What's unclear: Per-IP throttling.
   - Recommendation: Plan should cache aggressively (per-session), and surface a clear retry strategy on 429 reusing existing `LlmError::RateLimited`.
   - RESOLVED: Addressed via the 4h in-memory attestation cache (D3) — at most one attestation fetch per provider key per 4h window. 429 responses surface through the existing `LlmError::RateLimited` mapping in the transport layer; no new rate-limit machinery required.

4. **Does the chat tool toggle (Phase 27) need to gate Venice differently?**
   - What we know: Phase 27 introduced per-conversation tools-enabled bool; `build_chat_tools` filters tools.
   - What's unclear: Whether tool messages over E2EE work end-to-end.
   - Recommendation: Defer (D8). Phase 33 ships streaming chat; tools defer.
   - RESOLVED: Deferred per discretion D8 — out of scope for Phase 33. Phase 33 ships streaming chat only; the existing Phase 27 toggle continues to gate tool exposure and Venice tool support is a follow-up phase.

5. **Is there a per-conversation continuation expectation (resume an E2EE session across app cold launch)?**
   - What we know: Spike says "re-attest on any reconnect."
   - What's unclear: User UX expectation — do users feel a re-handshake?
   - Recommendation: Re-attest on every cold launch; the latency is ~1-2s which matches Tinfoil/PPQ behavior; no UX regression.
   - RESOLVED: Re-attest on every cold launch (matches Tinfoil/PPQ behavior); the verified attestation is cached per provider key for 4h within a single launch (D3). No persisted cross-launch session state.

---

## Sources

### Primary (HIGH confidence)
- **Spike 001 — Venice TEE Protocol Research:** `.claude/skills/spike-findings-confidential-app/sources/001-venice-tee-protocol-research/README.md` and `references/venice-attestation.md` — protocol shape, REPORTDATA layout, root-of-trust topology, gotchas. Contains live capture in `captures/attestation-sample.json`.
- **Existing project source:** `rust/src/llm/ppq_private.rs` (PPQ E2EE pattern), `rust/src/llm/tinfoil_secure.rs` (Tinfoil E2EE pattern), `rust/src/attestation/tdx.rs` (TDX verify), `rust/src/attestation/nvidia.rs` (NRAS JWT), `rust/src/llm/transport.rs` (provider transport routing), `rust/src/llm/backend.rs` (provider preset registration).
- **Project Cargo.toml** — confirmed available crates and version pins.
- **CLAUDE.md** — non-negotiable architecture and crypto constraints (no OpenSSL, all-Rust, RMP architecture, mobile build constraints).

### Secondary (MEDIUM confidence)
- **`cargo search` output (2026-04-25):** `k256 = "0.14.0-rc.9"`, `sha3 = "0.11.0"`, `urlencoding = "2.1.3"`, `dcap-qvl = "0.4.0"`, `jsonwebtoken = "10.3.0"`, `aes-gcm = "0.11.0-rc.3"`, `hkdf = "0.13.0"`, `rand_core = "0.10.1"` — used to inform recommended pin lines (with stable-line caveats noted as assumptions A1, A2).

### Tertiary (LOW confidence)
- None. All claims about Venice's wire format are derived from the spike capture (HIGH); all integration claims are derived from reading source (HIGH); only the version pinning recommendations are softened to MEDIUM by the assumption that 0.13.x `k256` and 0.10.x `sha3` exist as stable lines (verifiable in seconds at plan time).

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH for reused crates (verified against Cargo.toml); MEDIUM-HIGH for new crates (need plan-time `cargo info` to lock versions).
- Architecture: HIGH (mirrors verified existing patterns in ppq_private.rs and tinfoil_secure.rs).
- Pitfalls: HIGH (every pitfall is from the spike's verified `What to Avoid` list or from reading the existing codebase).
- Validation: HIGH (golden capture exists and covers all cryptographic-logic test paths).
- Security domain: HIGH (mirrors existing project security discipline).

**Research date:** 2026-04-25
**Valid until:** 2026-05-25 (30 days for this stable area; spike capture is dated 2026-04-25; revalidate live capture before phase merge if attempting > 30 days later).

**Planning risks flagged for the planner:**
1. No phase requirement IDs assigned — must be created (suggested VEN-01..VEN-09) before plans are usable.
2. No CONTEXT.md / discuss-phase artifacts — D1..D15 design decisions are recommended but not user-locked. Either run `/gsd-discuss-phase 33` first or have the planner adopt the recommendations and surface them as "Claude's discretion" in the plans.
3. MRSEAM coverage in `TdxPolicy` is unverified against live capture (A3, A4) — Wave 0 task should parse the golden capture and reconcile.
4. The existing `verify_tdx_quote` function needs a small refactor (D1) to accept a layout parameter or be forked. The planner must decide which.
5. NRAS-side mocking strategy for unit tests is not yet established in this codebase — recommend hand-crafted RSA test key over adding `wiremock` dependency.
