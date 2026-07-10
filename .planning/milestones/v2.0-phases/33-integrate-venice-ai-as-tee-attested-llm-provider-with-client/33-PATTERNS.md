# Phase 33: Venice.ai TEE-Attested Provider — Pattern Map

**Mapped:** 2026-04-25
**Files analyzed:** 8 (5 NEW, 3 MODIFIED)
**Analogs found:** 8 / 8 (all NEW files have a strong in-repo analog; all MODIFIED files extend existing modules verbatim)

---

## File Classification

| New/Modified File | Status | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|--------|------|-----------|----------------|---------------|
| `rust/src/llm/venice.rs` | NEW | provider transport (E2EE wrapper around OpenAI-compatible chat) | streaming + request-response, AES-GCM E2EE | `rust/src/llm/ppq_private.rs` | role-exact (E2EE wrapper); data-flow drift on framing (text-SSE not binary-framed) |
| `rust/src/attestation/venice.rs` | NEW | attestation submodule (Venice REPORTDATA layout decoder + verified-attestation cache) | request-response, in-memory cache | `rust/src/attestation/nvidia.rs` (module shape) + `rust/src/llm/ppq_private.rs::ensure_verified_attestation` (cache pattern) | role-exact (attestation submodule) |
| `rust/src/tests/venice.rs` | NEW | unit + integration tests (E2EE round-trip, request body shape, NRAS double-parse, model echo, debug-bit gate) | test | `rust/src/tests/attestation_nvidia.rs` + `rust/src/tests/attestation_tdx.rs` | role-exact (Rust tests, same crate-internal pattern) |
| `rust/src/tests/attestation_venice.rs` | NEW | REPORTDATA decoder unit tests against golden capture | test | `rust/src/tests/attestation_tdx.rs` | role-exact |
| `rust/src/tests/live_venice.rs` | NEW | gated live integration test (`#[ignore]`) | test (network) | `rust/src/tests/live_ppq_private.rs` (existing) | role-exact |
| `rust/src/llm/transport.rs` | MODIFIED | transport-kind dispatch (add `VeniceE2ee` variant + match arms) | dispatcher | self (extension; existing 115-line file) | exact |
| `rust/src/llm/backend.rs` | MODIFIED | provider preset + ProviderKind enum + transport routing key | config registry | self (extension; existing 160-line file) | exact |
| `rust/src/attestation/tdx.rs` | MODIFIED | parameterise REPORTDATA layout via enum (`NonceFirst32`, `VeniceAddrPadNonce`) | crypto-verify | self (extension of single 131-line function) | exact |
| `rust/src/attestation/mod.rs` | MODIFIED | `pub mod venice;` re-export | module registry | self (extension) | exact |
| `rust/src/llm/mod.rs` | MODIFIED | `pub mod venice;` re-export | module registry | self (extension) | exact |
| `rust/Cargo.toml` | MODIFIED | dependency manifest (add `k256`, `sha3`, `urlencoding`) | manifest | self | exact |

**Note on REPORTDATA layout (D1):** RESEARCH.md recommends parameterising `verify_tdx_quote` with a `ReportDataLayout` enum. Existing function at `rust/src/attestation/tdx.rs:59-131` hard-codes nonce-at-`[..32]`; planner must pick (a) parameterise (one signature change, one new enum, two existing call sites updated to pass `NonceFirst32`) or (b) write a `verify_tdx_quote_venice` sibling. (a) is cleaner; (b) is lower-blast-radius.

---

## Pattern Assignments

### `rust/src/llm/venice.rs` (NEW — provider transport, E2EE wrapper)

**Primary analog:** `rust/src/llm/ppq_private.rs` (module structure, attestation cache, request signing pattern, public API surface)
**Secondary analog (for SSE):** `rust/src/llm/tinfoil_secure.rs::handle_sse_event` (text-SSE parsing — Venice uses text SSE, NOT binary length-prefixed frames)

#### Imports pattern (lines 1-37 of `ppq_private.rs`)
Mirror this block. Drop HPKE/SEV/x509 imports (Venice is TDX+secp256k1 ECDH); add `k256`, `sha3`, `aes_gcm`, `hkdf`, `sha2`. Keep `once_cell::sync::Lazy`, `zeroize::{Zeroize, ZeroizeOnDrop}`, `reqwest::header::*`, `serde::Deserialize`, `flume`, `futures::StreamExt`, `tokio_util::sync::CancellationToken`.

```rust
// rust/src/llm/ppq_private.rs:1-37 (slim down for Venice)
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionTools, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse,
};
use futures::StreamExt;
use hkdf::Hkdf;
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};
```

#### Module-level constants pattern (`ppq_private.rs:39-58`)
Define these at the top of `venice.rs`:

```rust
const ATTESTATION_PATH: &str = "/api/v1/tee/attestation";
const CHAT_COMPLETIONS_PATH: &str = "/api/v1/chat/completions";
const X_VENICE_TEE_CLIENT_PUB_KEY: &str = "x-venice-tee-client-pub-key";
const X_VENICE_TEE_MODEL_PUB_KEY: &str = "x-venice-tee-model-pub-key";
const X_VENICE_TEE_SIGNING_ALGO: &str = "x-venice-tee-signing-algo";
const HKDF_INFO: &[u8] = b"ecdsa_encryption";
const AES_KEY_LEN: usize = 32;
const AES_NONCE_LEN: usize = 12;
const ATTESTATION_TTL_SECS: u64 = 4 * 3600; // matches PPQ pattern; per-session in-memory only

static VERIFIED_ATTESTATIONS: Lazy<Mutex<HashMap<String, VerifiedVeniceAttestation>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
```

#### Verified attestation struct + ZeroizeOnDrop (`ppq_private.rs:60-71`)
```rust
// ppq_private.rs:60-71 — copy structure; replace hpke_public_key with signing_pubkey_uncompressed
#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
struct VerifiedVeniceAttestation {
    #[zeroize(skip)]
    request_base_url: String,             // "https://api.venice.ai/api/v1"
    #[zeroize(skip)]
    model: String,                        // echoed model id, e.g. "e2ee-venice-uncensored-24b-p"
    signing_pubkey_uncompressed: [u8; 65], // attested secp256k1 pubkey (04-prefixed)
    submitted_nonce: [u8; 32],
    #[zeroize(skip)]
    report_blob: Vec<u8>,                  // raw TDX quote bytes
    #[zeroize(skip)]
    expires_at: u64,                       // unix-secs; per-session, NOT persisted
}
```

#### Public API surface — copy these signatures from `ppq_private.rs`
- `pub fn model_list_url(backend: &BackendConfig) -> Result<String, LlmError>` (line 103-105)
- `pub fn build_http_client(timeout: Duration) -> Result<reqwest::Client, LlmError>` (line 107-115)
- `pub async fn verify_backend_attestation(backend: &BackendConfig, tdx_policy: &TdxPolicy) -> Result<AttestationEvent, AttestationError>` (line 117-133, swap `SnpPolicy` for `TdxPolicy`, `tee_type: "IntelTdx"`)
- `pub async fn create_chat_completion(...) -> Result<CreateChatCompletionResponse, LlmError>` (line 135-159)
- `pub async fn run_streaming_chat_completion(backend: BackendConfig, model: String, messages: Vec<ChatMessage>, cancel_token, core_tx) -> ()` (line 161-220) — copy verbatim, swap `build_private_chat_body` → `build_venice_chat_body`, `send_private_request` → `send_venice_request`, `stream_decrypted_sse` → `stream_decrypted_venice_sse`.
- `pub async fn run_streaming_chat_completion_from_api_messages(...)` (line 229-284) — same.

#### Attestation cache + invalidate pattern (`ppq_private.rs:759-796`)
```rust
// ppq_private.rs:759-796 — copy verbatim; the cache shape is identical
async fn ensure_verified_venice_attestation(
    backend: &BackendConfig,
    requested_model: &str,
    tdx_policy: &crate::attestation::TdxPolicy,
) -> Result<VerifiedVeniceAttestation, LlmError> {
    let cache_key = format!("{}|{}", backend.base_url.trim_end_matches('/'), requested_model);
    let now_secs = now_secs();
    {
        let mut cache = VERIFIED_ATTESTATIONS.lock().map_err(|_| LlmError::NetworkError {
            reason: "Attestation cache lock poisoned".to_string(),
        })?;
        if let Some(cached) = cache.get(&cache_key) {
            if cached.expires_at > now_secs { return Ok(cached.clone()); }
            cache.remove(&cache_key);
        }
    }
    let verified = fetch_and_verify_venice_attestation(backend, requested_model, tdx_policy).await?;
    VERIFIED_ATTESTATIONS.lock().map_err(|_| LlmError::NetworkError {
        reason: "Attestation cache lock poisoned".to_string(),
    })?.insert(cache_key, verified.clone());
    Ok(verified)
}

fn invalidate_cached_venice_attestation(backend: &BackendConfig, model: &str) {
    if let Ok(mut cache) = VERIFIED_ATTESTATIONS.lock() {
        cache.remove(&format!("{}|{}", backend.base_url.trim_end_matches('/'), model));
    }
}
```

**Note:** Cache key includes `model` because Venice's attestation is per-model (different running TEE instances per model). PPQ caches per `base_url` only.

#### Request body builder (`ppq_private.rs:722-742` — `build_private_chat_body`)
Same shape but instead of swapping the `model` field, encrypt each user/system message body. RESEARCH.md §"Pattern 4" specifies: top-level fields stay plaintext, `enable_e2ee: true` is added, message `content` is replaced with hex envelope.

```rust
// Source: ppq_private.rs:722-742 (structure) + RESEARCH.md Pattern 4 (encryption logic)
fn build_venice_chat_body(
    request: &CreateChatCompletionRequest,
    aes_key: &[u8; 32],
    eph_pub_uncompressed: &[u8; 65],
) -> Result<Vec<u8>, LlmError> {
    let mut value = serde_json::to_value(request).map_err(|e| LlmError::NetworkError { reason: e.to_string() })?;
    let object = value.as_object_mut().ok_or_else(|| LlmError::NetworkError {
        reason: "Invalid chat completion request shape".to_string(),
    })?;
    // Encrypt each user/system message body (see RESEARCH.md Pattern 3 for envelope shape)
    if let Some(Value::Array(messages)) = object.get_mut("messages") {
        for msg in messages.iter_mut() {
            if let Some(Value::Object(m)) = msg.as_object_mut().map(|m| Value::Object(m.clone())).as_mut() {
                // role check: encrypt user/system, leave assistant as-is on outbound
                // ... encrypt content, replace with envelope hex
            }
        }
    }
    object.insert("enable_e2ee".to_string(), Value::Bool(true));
    object.insert("stream".to_string(), Value::Bool(true));
    serde_json::to_vec(&value).map_err(|e| LlmError::NetworkError { reason: e.to_string() })
}
```

#### Header construction + send (`ppq_private.rs:417-493` — `send_private_request`)
Copy the `HeaderMap` construction. Replace PPQ-specific headers (`x-private-model`, `x-query-source`, `x-tinfoil-enclave-url`, `ehbp-encapsulated-key`) with Venice headers (`X-Venice-TEE-Client-Pub-Key`, `X-Venice-TEE-Model-Pub-Key`, `X-Venice-TEE-Signing-Algo`, `Authorization: Bearer <api_key>`). Keep the 422-retry-after-attestation-invalidate idiom (lines 480-491) for any "stale attested key" error Venice returns.

```rust
// ppq_private.rs:432-468 — copy header construction shape
let endpoint = format!("{}{}", verified.request_base_url, path);
let mut headers = HeaderMap::new();
headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
headers.insert(
    HeaderName::from_static("authorization"),
    HeaderValue::from_str(&format!("Bearer {}", backend.api_key))
        .map_err(|e| LlmError::AuthError { reason: e.to_string() })?,
);
headers.insert(
    HeaderName::from_static(X_VENICE_TEE_CLIENT_PUB_KEY),
    HeaderValue::from_str(&hex::encode(eph_pub_uncompressed))
        .map_err(|e| LlmError::NetworkError { reason: e.to_string() })?,
);
headers.insert(
    HeaderName::from_static(X_VENICE_TEE_MODEL_PUB_KEY),
    HeaderValue::from_str(&hex::encode(&verified.signing_pubkey_uncompressed))
        .map_err(|e| LlmError::NetworkError { reason: e.to_string() })?,
);
headers.insert(
    HeaderName::from_static(X_VENICE_TEE_SIGNING_ALGO),
    HeaderValue::from_static("ecdsa"),
);
```

#### SSE handling — text-SSE pattern (CRITICAL DIVERGENCE FROM PPQ)
**Use `tinfoil_secure::handle_sse_event` shape (`tinfoil_secure.rs:342-380`), NOT `ppq_private::stream_decrypted_sse`.**

```rust
// tinfoil_secure.rs:342-380 — text-SSE parsing
fn handle_sse_event(
    raw_event: &str,
    aes_key: &[u8; 32],
    core_tx: &flume::Sender<crate::CoreMsg>,
) -> Result<bool, LlmError> {
    let mut data_lines = Vec::new();
    for line in raw_event.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start());
        }
    }
    if data_lines.is_empty() { return Ok(true); }
    let payload = data_lines.join("\n");
    if payload == "[DONE]" { return Ok(false); }

    let chunk: CreateChatCompletionStreamResponse = serde_json::from_str(&payload)
        .map_err(|e| LlmError::NetworkError { reason: format!("Invalid Venice SSE chunk: {e}") })?;

    if let Some(envelope_hex) = chunk.choices.first().and_then(|c| c.delta.content.as_deref()) {
        // Decrypt: parse [eph_pub 65B][nonce 12B][ct+tag] from hex, decrypt with aes_key
        let plaintext = decrypt_envelope(envelope_hex, aes_key)?;
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            crate::llm::streaming::InternalEvent::StreamChunk { token: plaintext },
        )));
    }
    Ok(true)
}
```

`take_sse_event` helper for buffering newline-separated frames is at `tinfoil_secure.rs:700-710`:

```rust
// tinfoil_secure.rs:700-710 — copy verbatim
fn take_sse_event(buffer: &mut String) -> Option<String> {
    let separators = ["\n\n", "\r\n\r\n"];
    for separator in separators {
        if let Some(index) = buffer.find(separator) {
            let event = buffer[..index].to_string();
            buffer.drain(..index + separator.len());
            return Some(event);
        }
    }
    None
}
```

#### ECDH + HKDF + AES-GCM (NEW; spec from RESEARCH.md Pattern 3)
No analog in repo for secp256k1 ECDH. RESEARCH.md §"Pattern 3" gives the canonical sequence verbatim — use it directly:

```rust
// RESEARCH.md Pattern 3 (line 326-355) — canonical ECDH/HKDF/AES-GCM bring-up
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
    .expand(HKDF_INFO, &mut aes_key)?;
```

**Per-message envelope:** generate fresh 12B nonce from `OsRng`, encrypt body, concatenate `[eph_pub 65B || nonce 12B || ct+tag]`, hex-encode. No counter-derived nonces (Pitfall 7).

#### Error handling (`ppq_private.rs:1227+` — `llm_to_attestation_error`)
Reuse the same translator. The cryptographic failures (REPORTDATA mismatch, etc.) raised by `attestation/venice.rs` will already be `AttestationError`; LLM-layer errors stay `LlmError`. Use `AttestationError::QuoteVerification { reason }` for layout/binding failures and `AttestationError::NonceMismatch` only for the raw nonce comparison at `report_data[32..64]`.

---

### `rust/src/attestation/venice.rs` (NEW — REPORTDATA decoder + Venice cache types)

**Primary analog:** `rust/src/attestation/nvidia.rs` (module shape, error mapping, `tee_type: "IntelTdx"` event construction)

#### File header pattern (`nvidia.rs:1-7`)
```rust
//! Venice.ai TEE attestation: REPORTDATA layout decoder + secp256k1 binding check.
//!
//! Per spike 001: Venice runs on Phala dstack with Intel TDX + NVIDIA H100 CC.
//! REPORTDATA layout: [20B keccak-address][12B zero-pad][32B raw nonce].
//! The 20B address binds the per-session secp256k1 signing key into the TDX quote.

use super::error::AttestationError;
use super::AttestationEvent;
```

#### REPORTDATA decoder (verbatim from RESEARCH.md §"Pattern 2", lines 290-323)
This is the canonical implementation — copy as-is, adapting error variants to project's `AttestationError` enum (see `rust/src/attestation/error.rs:8-37`). Use `QuoteVerification { reason }` for shape failures and `NonceMismatch { expected, actual }` for the nonce comparison at offsets `[32..64]`.

```rust
// RESEARCH.md Pattern 2 (lines 290-323) — verbatim
fn verify_venice_report_data(
    report_data: &[u8; 64],
    signing_pubkey_hex: &str,
    submitted_nonce: &[u8; 32],
) -> Result<(), AttestationError> {
    let pubkey = hex::decode(signing_pubkey_hex)
        .map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Venice signing pubkey hex decode failed: {e}"),
        })?;
    if pubkey.len() != 65 || pubkey[0] != 0x04 {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not in uncompressed secp256k1 form".into(),
        });
    }
    use sha3::{Digest, Keccak256};
    let mut h = Keccak256::new();
    h.update(&pubkey[1..]);
    let addr20 = &h.finalize()[12..32];
    if addr20 != &report_data[0..20] {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice signing key not bound to TDX REPORTDATA[0..20]".into(),
        });
    }
    if report_data[20..32].iter().any(|&b| b != 0) {
        return Err(AttestationError::QuoteVerification {
            reason: "Venice REPORTDATA[20..32] padding non-zero".into(),
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

#### Wire-format struct with field aliasing (Pitfall 3)
```rust
// RESEARCH.md Pitfall 3 — `signing_public_key` vs `signing_key` drift
#[derive(Debug, serde::Deserialize)]
struct VeniceAttestationResponse {
    intel_quote: String,                                // hex
    #[serde(alias = "signing_public_key")]
    signing_key: String,                                // hex, 130 chars (04 + 64-byte X || Y)
    nvidia_payload: Option<String>,                     // STRING containing JSON — see Pitfall 2
    signing_address: String,                            // 0x-prefixed Ethereum-style address
    nonce: String,                                      // hex echo of client-submitted nonce
    model: String,                                      // server echo for sanity gate
    #[serde(default)]
    server_verification: Option<serde_json::Value>,     // logged only, NEVER trusted
}
```

#### NRAS double-parse (Pitfall 2)
```rust
// nvidia_payload arrives as String containing JSON; must from_str the inner string
let nvidia_payload_str = resp.nvidia_payload.ok_or_else(|| AttestationError::QuoteVerification {
    reason: "Venice response missing nvidia_payload".into(),
})?;
// Forward the parsed string (NOT the outer Value) to NRAS:
let nvidia_evt = crate::attestation::nvidia::fetch_and_verify_nvidia(
    &nvidia_payload_str,
    &nonce_hex,
    &backend.id,
).await?;
```

#### Debug-bit gate (RESEARCH.md Pitfall 6 + VEN-06)
```rust
// After dcap_qvl::verify::verify(...) succeeds, gate on td_attributes
let td_attributes = match &report.report {
    dcap_qvl::quote::Report::TD10(td) => td.td_attributes,
    dcap_qvl::quote::Report::TD15(td) => td.base.td_attributes,
    _ => return Err(AttestationError::QuoteVerification {
        reason: "Venice quote is not a TDX TD10/TD15 report".into(),
    }),
};
if td_attributes[0] & 0x01 != 0 {
    return Err(AttestationError::QuoteVerification {
        reason: "Venice TDX is in debug mode (td_attributes[0] & 0x01 != 0)".into(),
    });
}
```

---

### `rust/src/attestation/tdx.rs` (MODIFIED — D1: parameterise REPORTDATA layout)

**Self-extension.** Existing function at lines 59-131. Add layout enum, change signature, update existing call sites in `attestation/endpoint.rs` (or wherever `verify_tdx_quote` is called) to pass `ReportDataLayout::NonceFirst32`.

#### Pattern: extend in place
```rust
// Add to tdx.rs above verify_tdx_quote
#[derive(Clone, Copy, Debug)]
pub enum ReportDataLayout {
    /// First 32 bytes are the raw client nonce; remainder ignored. (Tinfoil/PPQ via TDX path.)
    NonceFirst32,
    /// Venice/Phala dstack layout: [20B keccak-addr][12B zero-pad][32B raw nonce].
    /// Address and pad checks live in attestation/venice.rs; this enum only changes
    /// where verify_tdx_quote looks for the nonce-comparison bytes.
    VeniceAddrPadNonce,
}

// Change signature at line 59:
pub async fn verify_tdx_quote(
    quote_bytes: &[u8],
    expected_nonce: &[u8; 32],
    backend_id: &str,
    layout: ReportDataLayout,        // NEW parameter
) -> Result<AttestationEvent, AttestationError> { ... }

// At line 109, replace:
//   let nonce_in_report = &report_data[..32.min(report_data.len())];
// with:
let nonce_in_report = match layout {
    ReportDataLayout::NonceFirst32 => &report_data[..32.min(report_data.len())],
    ReportDataLayout::VeniceAddrPadNonce => &report_data[32..64.min(report_data.len())],
};
```

**Existing callers to update:** Search for `verify_tdx_quote(` — every call needs `, ReportDataLayout::NonceFirst32` appended. Existing test at `rust/src/tests/attestation_tdx.rs:53-63` calls it with `(short_bytes, nonce, "test-backend")` — update.

---

### `rust/src/llm/transport.rs` (MODIFIED — add `VeniceE2ee` variant)

**Self-extension.** File is 115 lines; the change set is mechanical. RESEARCH.md §"Example 3" (lines 487-503) gives the exact additions:

```rust
// transport.rs:14-18 — add variant
pub enum ProviderTransportKind {
    OpenAiCompatible,
    TinfoilSecure,
    PpqPrivateE2ee,
    VeniceE2ee,                       // NEW
}

// transport.rs:21-34 — add for_backend arm
impl ProviderTransportKind {
    pub fn for_backend(backend: &BackendConfig) -> Self {
        if backend.provider_kind() == super::backend::ProviderKind::Tinfoil {
            return Self::TinfoilSecure;
        }
        if backend.provider_kind() == super::backend::ProviderKind::Venice {
            return Self::VeniceE2ee;
        }
        // ... existing PPQ branch ...
    }
}

// transport.rs:36-52 — add error/url arms in openai_api_base, model_list_url
//   Self::VeniceE2ee => Err(unsupported_venice_transport_error()),
//   Self::VeniceE2ee => super::venice::model_list_url(backend),
//   Self::VeniceE2ee => Ok((super::venice::build_http_client(timeout)?, false)),
```

Pattern for the unsupported-transport error helper exists at `transport.rs:111-115` (`unsupported_private_transport_error`) — clone with Venice wording.

---

### `rust/src/llm/backend.rs` (MODIFIED — add `Venice` to ProviderKind enum + preset)

**Self-extension.** File is 160 lines.

```rust
// backend.rs:11-15 — add variant
pub enum ProviderKind {
    Tinfoil,
    Ppq,
    Venice,                           // NEW
    Custom,
}

// backend.rs:99-105 — add provider_kind arm
pub fn provider_kind(&self) -> ProviderKind {
    match self.id.as_str() {
        "tinfoil" => ProviderKind::Tinfoil,
        "ppq-ai" => ProviderKind::Ppq,
        "venice-ai" => ProviderKind::Venice,    // NEW
        _ => ProviderKind::Custom,
    }
}

// backend.rs:143-160 — add to known_provider_presets
ProviderPreset {
    id: "venice-ai".into(),
    name: "Venice.ai".into(),
    base_url: "https://api.venice.ai/api/v1/".into(),
    tee_type: TeeType::IntelTdx,
    description: "Intel TDX + NVIDIA H100 CC \u{00b7} E2EE chat".into(),
},
```

---

### `rust/src/tests/attestation_venice.rs` (NEW)

**Analog:** `rust/src/tests/attestation_tdx.rs` (verbatim shape).

#### Imports + test layout pattern
```rust
// attestation_tdx.rs:1-7 — copy structure
//! Unit tests for Venice REPORTDATA decoder + binding checks.
//! Covers VEN-04a..VEN-04d.

use crate::attestation::error::AttestationError;
use crate::attestation::venice::{verify_venice_report_data, VeniceAttestationResponse};
```

#### Test pattern (`attestation_tdx.rs:8-37`)
```rust
// attestation_tdx.rs:8-37 — synchronous unit test with constructed input
#[test]
fn reportdata_layout_ok() {
    // Use golden capture from .claude/skills/.../captures/attestation-sample.json
    let report_data: [u8; 64] = /* parsed from capture */;
    let pubkey_hex = "04..."; /* from capture */
    let nonce: [u8; 32] = /* from capture */;
    let result = verify_venice_report_data(&report_data, pubkey_hex, &nonce);
    assert!(result.is_ok(), "golden-capture report_data must validate");
}

#[test]
fn reportdata_address_mismatch() {
    let mut report_data = [0u8; 64];
    report_data[0] = 0xFF; // tamper address
    let result = verify_venice_report_data(&report_data, "04...", &[0u8; 32]);
    assert!(matches!(result, Err(AttestationError::QuoteVerification { .. })));
}

#[test]
fn reportdata_padding_nonzero() {
    let mut report_data = /* valid layout */;
    report_data[20] = 0xAA; // tamper padding
    let result = verify_venice_report_data(&report_data, pubkey_hex, &nonce);
    assert!(matches!(result, Err(AttestationError::QuoteVerification { reason }) if reason.contains("padding non-zero")));
}

#[test]
fn reportdata_nonce_mismatch() {
    let result = verify_venice_report_data(&golden_report_data, pubkey_hex, &[0u8; 32]);
    assert!(matches!(result, Err(AttestationError::NonceMismatch { .. })));
}
```

#### Golden capture loading
RESEARCH.md §"Wave 0 Gaps" recommends `include_str!` from skill path. Pattern:
```rust
const GOLDEN: &str = include_str!("../../../.claude/skills/spike-findings-confidential-app/sources/001-venice-tee-protocol-research/captures/attestation-sample.json");
```

---

### `rust/src/tests/venice.rs` (NEW)

**Analog:** `rust/src/tests/attestation_nvidia.rs` (claims-deserialize, error path) plus `tests/attestation_tdx.rs` for `#[tokio::test]` shape.

#### Patterns to copy
- `#[test] fn deserialize_response_ok` — from `attestation_nvidia.rs:7-25` shape (parse JSON literal, assert fields)
- `#[test] fn nvidia_payload_double_parse` — verify the inner-string `serde_json::from_str` works
- `#[test] fn ecdh_aes_round_trip` — generate two `EphemeralSecret`s, compute symmetric AES key, encrypt/decrypt round-trip
- `#[test] fn envelope_round_trip` — concat `[eph_pub | nonce | ct]`, hex-encode, decode, decrypt
- `#[test] fn request_body_shape` — call `build_venice_chat_body` with a fixed key and assert `enable_e2ee: true` is set, `messages[].content` is hex
- `#[tokio::test] async fn tdx_debug_bit_rejected` — feed a constructed quote with `td_attributes[0] |= 0x01` and assert rejection
- `#[test] fn attestation_url_format` — assert `format_attestation_url("e2ee-...", "abc...")` produces `?model=...&nonce=...` with proper percent-encoding

---

### `rust/src/tests/live_venice.rs` (NEW, gated)

**Analog:** `rust/src/tests/live_ppq_private.rs` (existing). Mark with `#[ignore]`; manually run via `cargo test -p mango_core --lib live_venice -- --ignored`.

```rust
#[tokio::test]
#[ignore = "live integration test against api.venice.ai; requires VENICE_API_KEY env"]
async fn live_attestation_round_trip() { /* ... */ }
```

---

## Shared Patterns

### Authentication (Bearer token via async-openai or manual headers)
**Source:** `rust/src/llm/ppq_private.rs:436-442`
**Apply to:** All `venice.rs` outbound HTTP calls to chat completions endpoint.

```rust
headers.insert(
    HeaderName::from_static("authorization"),
    HeaderValue::from_str(&format!("Bearer {}", backend.api_key))
        .map_err(|e| LlmError::AuthError { reason: e.to_string() })?,
);
```

**Note:** Venice attestation endpoint (`GET /api/v1/tee/attestation`) is PUBLIC — no Authorization header needed (RESEARCH.md D14 + VEN-02).

### Error handling (LlmError + AttestationError taxonomy)
**Source:** `rust/src/attestation/error.rs:8-37` and `rust/src/llm/ppq_private.rs:1227+` (`llm_to_attestation_error`)
**Apply to:** All cryptographic operations in `attestation/venice.rs` and all transport operations in `llm/venice.rs`.

| Failure | Variant | Notes |
|---------|---------|-------|
| Hex decode of `intel_quote` | `AttestationError::QuoteVerification { reason }` | wrap source error in `reason` |
| `dcap_qvl::collateral::get_collateral` failure | `AttestationError::CollateralFetch { reason }` | matches existing TDX path |
| `dcap_qvl::verify::verify` failure | `AttestationError::QuoteVerification { reason }` | matches existing TDX path |
| Address binding mismatch | `AttestationError::QuoteVerification { reason: "...not bound to TDX REPORTDATA..." }` | new, Venice-specific |
| Nonce mismatch (REPORTDATA[32..64]) | `AttestationError::NonceMismatch { expected, actual }` | hex-encoded both sides |
| TDX debug bit set | `AttestationError::QuoteVerification { reason: "...debug mode..." }` | new, Venice-specific |
| NRAS network failure | `AttestationError::NetworkError { reason }` | reused unchanged via `fetch_and_verify_nvidia` |
| Reqwest send failure | `LlmError::NetworkError { reason }` | matches existing PPQ path |
| API key empty / Authorization header build failure | `LlmError::AuthError { reason }` | matches PPQ pattern |
| AES-GCM decrypt failure on inbound chunk | `LlmError::NetworkError { reason: "Venice E2EE decrypt failed" }` | new |
| Hex decode of inbound envelope | `LlmError::NetworkError { reason: ... }` | new |

**Transient classification (`is_transient`, error.rs:53-58):** `NetworkError` and `CollateralFetch` are transient → preserve `Verified`. All cryptographic failures (`QuoteVerification`, `NonceMismatch`, `JwtVerification`) are non-transient → downgrade `Verified -> Failed`. Per RESEARCH.md D11.

### TLS / HTTP client construction
**Source:** `rust/src/llm/ppq_private.rs:107-115` (`build_http_client`) — uses default reqwest with `hickory_dns(false)` and timeout. **No TLS pinning for Venice** — RESEARCH.md §"Don't Hand-Roll" makes this explicit: trust root for content is the attested signing key, not the TLS leaf.

```rust
// ppq_private.rs:107-115 — copy verbatim
pub fn build_http_client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .hickory_dns(false)
        .timeout(timeout)
        .build()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })
}
```

### Cache pattern (`once_cell::Lazy<Mutex<HashMap>>`)
**Source:** `rust/src/llm/ppq_private.rs:57-58, 759-796`
**Apply to:** `attestation/venice.rs` cache + `llm/venice.rs` cache (only one is needed; recommend put it in `attestation/venice.rs` and have `llm/venice.rs` call into it).

### Zeroize / ZeroizeOnDrop on key material
**Source:** `rust/src/llm/ppq_private.rs:60-71`
**Apply to:** Any struct holding AES keys, ECDH shared secrets, ephemeral private keys. Mark non-secret display strings with `#[zeroize(skip)]`.

### UniFFI status surfacing (`AttestationStatus`, `AttestationEvent`)
**Source:** `rust/src/attestation/mod.rs` (existing types).
**Apply to:** `verify_backend_attestation` in `llm/venice.rs` returns `AttestationEvent::Verified { tee_type: "IntelTdx", ... }` (NVIDIA-CC verification result is folded in but the surfaced `tee_type` matches existing tinfoil pattern). The badge wording is rendered native-side from `TeeType::IntelTdx` + the description string from `ProviderPreset`.

### Logging conventions
**Source:** `rust/src/attestation/tdx.rs:64, 82, 87, 95, 111` and `nvidia.rs:105, 115, 128, 130`
**Pattern:** `log::debug!(target: "attestation", "[attestation] <action> backend={} <field>={}", ...)` for steady-state; `log::warn!` for failures. Use `target: "attestation"` for `attestation/venice.rs`. Use no special target for `llm/venice.rs` (matches `ppq_private.rs`).

### Validation (per-field length checks)
**Source:** RESEARCH.md §"Security Domain V5" — validate every wire-format field length:
- `intel_quote` decoded ≥ 48 bytes
- `signing_key` decoded == 65 bytes with `[0] == 0x04`
- REPORTDATA exactly 64 bytes
- nonce exactly 32 bytes (`[u8; 32]`)
- AES-GCM nonce exactly 12 bytes (`[u8; 12]`)

Existing precedent for this discipline in `rust/src/attestation/tdx.rs:37-45` (quote-too-short check).

---

## No Analog Found

| File / concern | Why no analog | Planner reference |
|----------------|---------------|-------------------|
| secp256k1 ECDH (`k256::ecdh::EphemeralSecret`) | First use of secp256k1 in repo (existing `sev` SEV-SNP path uses p384) | RESEARCH.md §"Pattern 3" lines 326-355 — verbatim canonical |
| Keccak256 address derivation (`sha3::Keccak256` on `pubkey[1..65]`) | First use of sha3 in repo (existing `sha2::Sha256` is for HKDF/cert hashing) | RESEARCH.md §"Pattern 2" lines 290-323 — verbatim canonical |
| `urlencoding` of model id in attestation URL query | First use of percent-encoding helper | RESEARCH.md §"Standard Stack" — recommend `urlencoding::encode(model_id)` one-liner |
| Per-message AES-GCM with embedded server-generated nonce | PPQ uses counter-derived nonce on client side; Venice receives server-generated nonces in the inbound envelope | RESEARCH.md §"Pitfall 8" + Pattern 4 |

For all three, RESEARCH.md provides verbatim canonical code that is ready to inline.

---

## Metadata

**Analog search scope:**
- `rust/src/llm/` (all files; primary analogs: `ppq_private.rs`, `tinfoil_secure.rs`)
- `rust/src/attestation/` (all files; primary analogs: `tdx.rs`, `nvidia.rs`, `error.rs`, `mod.rs`)
- `rust/src/tests/` (test harness shape: `attestation_tdx.rs`, `attestation_nvidia.rs`, `live_ppq_private.rs`)

**Files scanned:** 11 source + 3 test = 14 in-repo files; 1 spike skill file; 1 RESEARCH.md.

**Pattern extraction date:** 2026-04-25

**Planner pre-flight checklist:**
1. Decide D1 (parameterise `verify_tdx_quote` vs sibling fn). Recommendation: parameterise — cleaner, forces explicit layout choice.
2. Confirm `k256` 0.13.x and `sha3` 0.10.x exist as stable lines (`cargo info k256 && cargo info sha3`).
3. Confirm golden capture path resolves at compile time via `include_str!` (relative path from `rust/src/tests/`).
4. Decide whether `attestation/venice.rs` exposes the cache directly or whether `llm/venice.rs` owns its own `VERIFIED_ATTESTATIONS` static (recommend the latter to mirror `ppq_private.rs`).
5. Write a small Wave-0 task to parse the golden capture's MRSEAM and reconcile against `TdxPolicy::accepted_mr_seams` — RESEARCH.md A3/A4 are MEDIUM-risk assumptions.
