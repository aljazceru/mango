---
phase: 33
plan: 03
subsystem: rust-core/llm
tags:
  - venice
  - e2ee
  - secp256k1-ecdh
  - hkdf-sha256
  - aes-256-gcm
  - text-sse
  - wave-3
dependency-graph:
  requires:
    - 33-02-SUMMARY.md (VerifiedVeniceAttestation, ensure_verified_venice_attestation, invalidate_cached_venice_attestation)
    - rust/src/attestation/venice.rs (Plan 02 attestation orchestrator)
    - rust/src/attestation/policy.rs::TdxPolicy
    - rust/src/llm/{BackendConfig, LlmError}
    - rust/src/llm/streaming.rs (ChatMessage, ChatRole, InternalEvent)
  provides:
    - rust/src/llm/venice.rs::format_attestation_url (VEN-02)
    - rust/src/llm/venice.rs::derive_session_key (ECDH+HKDF)
    - rust/src/llm/venice.rs::seal_message / open_envelope (AES-256-GCM envelope)
    - rust/src/llm/venice.rs::build_venice_chat_body_for_test (test surface)
    - rust/src/llm/venice.rs::model_list_url
    - rust/src/llm/venice.rs::build_http_client
    - rust/src/llm/venice.rs::verify_backend_attestation
    - rust/src/llm/venice.rs::create_chat_completion
    - rust/src/llm/venice.rs::run_streaming_chat_completion
    - rust/src/llm/venice.rs::run_streaming_chat_completion_from_api_messages
  affects:
    - rust/src/llm/mod.rs (`pub mod venice;`)
    - rust/src/tests/venice.rs (5 RED → GREEN)
tech-stack:
  added: []
  patterns:
    - "Per-request EphemeralSecret::random per chat completion (T-33-12: prod has no test-key construction surface; T-33-11: peer pubkey comes from VerifiedVeniceAttestation only)"
    - "Fresh OsRng nonce on every seal_message call — no counter-derived nonce (Pitfall 7 / T-33-05)"
    - "Text-SSE framing via take_sse_event(\\n\\n / \\r\\n\\r\\n) + handle_venice_sse_event mirroring tinfoil_secure (Pitfall 8 / T-33-14: anti-pattern grep clean)"
    - "Inbound envelope nonce read from offsets [65..77] — never derived (Pitfall 8)"
    - "422 with body containing 'stale'/'attestation'/'key' triggers invalidate_cached_venice_attestation + single retry; mirrors PPQ EHBP_KEY_CONFIG_PROBLEM idiom"
    - "create_chat_completion_inner re-stamps stream:false on the serialized body because build_venice_chat_body unconditionally sets stream:true (single body builder for both paths)"
key-files:
  created:
    - rust/src/llm/venice.rs
  modified:
    - rust/src/llm/mod.rs
    - rust/src/tests/venice.rs
decisions:
  - "Used `pub fn` (crate-visible) instead of `pub(super) fn` for derive_session_key/seal_message/open_envelope — needed crate::tests::venice::* test access, and pub(super) only exposes to crate::llm. No external (uniffi) leakage because the parent module crate::llm::venice is not re-exported from mod.rs aside from the `pub mod venice;` line."
  - "build_venice_chat_body_for_test (#[doc(hidden)] pub fn) is the test-only re-export surface for the body builder. The internal build_venice_chat_body remains module-private."
  - "Used backend.models.first() for the attestation model selection in verify_backend_attestation, with hardcoded fallback to 'e2ee-venice-uncensored-24b-p'. BackendConfig has no preferred_model field (the plan stub assumed one). Plan 04 may add a backend-level preferred-model field if needed; this keeps the API surface aligned with the actual struct shape."
  - "AttestationEvent::Verified construction uses the existing field set from attestation/mod.rs (tee_type, report_blob, expires_at, tls_public_key_fp:None, vcek_url:None, vcek_der:None) — Venice has no TLS pinning (per RESEARCH.md 'Don't Hand-Roll' and no SEV VCEK)."
  - "Stream entry signature added an Option<Vec<ChatCompletionTools>> tools parameter (matching the plan's <interfaces> declaration). The simpler run_streaming_chat_completion(messages: Vec<ChatMessage>, ...) bridge passes None for tools."
metrics:
  duration: ~25 minutes
  completed-date: 2026-04-25
  tasks-completed: 3
  files-created: 1
  files-modified: 2
  commits: 3
---

# Phase 33 Plan 03: Venice transport / E2EE layer — Summary

**One-liner:** Built `rust/src/llm/venice.rs` (763 lines) — the full Venice provider transport with secp256k1 ECDH (k256), HKDF-SHA256, AES-256-GCM envelope (per-message OsRng nonce — Pitfall 7), text-SSE streaming (mirrors `tinfoil_secure`, NOT PPQ binary frames — Pitfall 8), 422-stale-attestation retry, and exported the public surface needed by Plan 04 to wire transport.rs/backend.rs.

## What was built

### `rust/src/llm/venice.rs` (NEW, 763 lines)

**Public surface (consumed by Plan 04):**
- `pub fn model_list_url(&BackendConfig) -> Result<String, LlmError>` — Venice `/api/v1/models`
- `pub fn build_http_client(Duration) -> Result<reqwest::Client, LlmError>` — verbatim from `ppq_private`
- `pub fn format_attestation_url(model, nonce_hex, base_url) -> String` (VEN-02)
- `pub async fn verify_backend_attestation(&BackendConfig, &TdxPolicy) -> Result<AttestationEvent, AttestationError>` — calls `ensure_verified_venice_attestation`, returns `tee_type:"IntelTdx"`
- `pub async fn create_chat_completion(BackendConfig, model, messages, Option<tools>) -> Result<CreateChatCompletionResponse, LlmError>`
- `pub async fn run_streaming_chat_completion(BackendConfig, model, Vec<ChatMessage>, CancellationToken, Sender<CoreMsg>)`
- `pub async fn run_streaming_chat_completion_from_api_messages(BackendConfig, model, Vec<ChatCompletionRequestMessage>, Option<tools>, CancellationToken, Sender<CoreMsg>)`

**Crypto primitives:**
- `pub fn derive_session_key(&EphemeralSecret, &[u8;65]) -> Result<[u8;32]>` — ECDH(secp256k1) + HKDF-SHA256(info=b"ecdsa_encryption")
- `pub fn seal_message(plaintext, &aes_key, &eph_pub_65) -> Result<String>` — fresh `rand::thread_rng()` 12-byte nonce, `Aes256Gcm::encrypt`, hex-encode `[eph_pub 65 || nonce 12 || ct+tag]`
- `pub fn open_envelope(envelope_hex, &aes_key) -> Result<Vec<u8>>` — parse, fail-closed on AEAD tag mismatch (T-33-13)

**Internal (not exported):**
- `build_venice_chat_body` — encrypts each user/system/tool message content, sets `enable_e2ee:true` + `stream:true`. Multipart `Array` content rejected (D9 deferred).
- `build_venice_chat_body_for_test` — `#[doc(hidden)] pub fn` test re-export.
- `build_venice_headers` / `send_venice_request` — `Authorization: Bearer …`, `x-venice-tee-client-pub-key`, `x-venice-tee-model-pub-key`, `x-venice-tee-signing-algo: ecdsa`.
- `take_sse_event` (newline-separator buffer) + `handle_venice_sse_event` (parses `data: …\n\n`, decrypts `delta.content` via `open_envelope`, emits `StreamChunk`). Returns `Ok(false)` on `[DONE]`.
- `stream_decrypted_venice_sse` — biased `tokio::select!` loop with cancellation handling, partial-frame flush on stream end.
- `run_streaming_inner` / `create_chat_completion_inner` — full pipeline; on 422 with body containing "stale"/"attestation"/"key", calls `invalidate_cached_venice_attestation` and retries once.

### Wire envelope format (for Plan 04 cross-validation)

```
[ uncompressed eph_pub : 65 bytes (0x04 || X(32) || Y(32)) ]
[ AES-GCM nonce        : 12 bytes (fresh OsRng per message) ]
[ AES-GCM ciphertext+tag : N + 16 bytes ]
```

All hex-encoded as a single string. Minimum envelope size: `2 * (65 + 12 + 16) = 186` hex chars (zero-byte plaintext).

### Header set on outbound chat-completion POST

| Header | Value |
|---|---|
| `Content-Type` | `application/json` |
| `Authorization` | `Bearer {api_key}` |
| `x-venice-tee-client-pub-key` | hex(eph_pub uncompressed, 65B) |
| `x-venice-tee-model-pub-key` | hex(verified.signing_pubkey_uncompressed, 65B) |
| `x-venice-tee-signing-algo` | `ecdsa` |

## RED → GREEN tally

| Test | File | Status |
|---|---|---|
| `attestation_url_format` (VEN-02) | tests/venice.rs | RED → **GREEN** |
| `ecdh_aes_round_trip` (VEN-07a) | tests/venice.rs | RED → **GREEN** |
| `envelope_round_trip` (VEN-07b) | tests/venice.rs | RED → **GREEN** |
| `request_body_shape` (VEN-08) | tests/venice.rs | RED → **GREEN** |
| `nvidia_payload_double_parse` (VEN-05) | tests/venice.rs | RED → **GREEN** (Pitfall 2 + 3) |
| `venice_preset_present` (VEN-01) | tests/venice.rs | still `#[ignore]` (Plan 04 scope) |
| `backend_summary_after_add` (VEN-09) | tests/venice.rs | still `#[ignore]` (Plan 04 scope) |

5/7 venice-module RED stubs flipped GREEN this plan, exactly the count specified in the plan's `<output>` section.

## Test counts

```
$ cargo test -p mango_core --lib venice -- --nocapture
running 14 tests
test attestation_venice::tdx_debug_bit_rejected ............. ignored (Plan 04)
test attestation_venice::tdx_verify_golden_capture_signature  ignored (Plan 04)
test live_venice::live_attestation_round_trip ............... ignored (Plan 04)
test venice::backend_summary_after_add ...................... ignored (Plan 04)
test venice::venice_preset_present .......................... ignored (Plan 04)
test venice::attestation_url_format ......................... ok
test venice::ecdh_aes_round_trip ............................ ok
test venice::envelope_round_trip ............................ ok
test venice::nvidia_payload_double_parse .................... ok
test venice::request_body_shape ............................. ok
test attestation_venice::reportdata_layout_ok ............... ok
test attestation_venice::reportdata_address_mismatch ........ ok
test attestation_venice::reportdata_padding_nonzero ......... ok
test attestation_venice::reportdata_nonce_mismatch .......... ok

result: ok. 9 passed; 0 failed; 5 ignored
```

Full crate suite: **347 passed; 0 failed; 15 ignored** (was 342 before — gained 5 GREEN this plan).

`cargo build -p mango_core --release`: clean, 3 dead-code warnings (`decode_quote`, `generate_nonce`, `NonceFirst32::NonceFirst32`) inherited from Plan 02 — all are consumed by tests and other transports.

## Anti-pattern grep (Pitfall 8 verification)

```
$ grep -nE 'frame_len|length_prefix|chunk_counter|try_take_frame' rust/src/llm/venice.rs
(no matches)
```

Counter-derived nonces and binary length-prefixed framing — both PPQ-style anti-patterns — are absent. Venice uses text-SSE only and reads the per-message nonce from the inbound envelope.

## Commits

| # | Task | Type | Hash | Files |
|---|---|---|---|---|
| 1 | Task 1 — crypto primitives + module scaffold (full transport included) + 3 RED→GREEN | feat | `6fe3f96` | `llm/venice.rs` (new), `llm/mod.rs`, `tests/venice.rs` |
| 2 | Task 2a — request_body_shape RED→GREEN | test | `ae13454` | `tests/venice.rs` |
| 3 | Task 2b — nvidia_payload_double_parse RED→GREEN | test | `e08f1bb` | `tests/venice.rs` |

**Note on commit shape:** The plan's three tasks are conceptually separated (T1 = crypto primitives; T2a = body builder + headers + verify_backend_attestation; T2b = streaming + create + 422 retry). For pragmatic reasons all production code landed in the Task 1 commit (`6fe3f96`) — the module compiles only when the full chain is present (the streaming entry points reference `build_venice_chat_body`, `send_venice_request`, and `take_sse_event` as one cohesive unit), and partial commits would have been red. The Task 2a / 2b commits flip the corresponding RED tests, providing per-task verification of the production code's correctness on each acceptance gate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `k256::elliptic_curve::sec1::ToEncodedPoint` trait not in scope**
- **Found during:** Task 1 first compile.
- **Issue:** `eph_secret.public_key().to_encoded_point(false)` failed because `ToEncodedPoint` is the trait that provides the method on `PublicKey<C>`. The plan stub did not import it.
- **Fix:** Added `use k256::elliptic_curve::sec1::ToEncodedPoint;` to both `venice.rs` and the test file.
- **Files modified:** `rust/src/llm/venice.rs`, `rust/src/tests/venice.rs`.
- **Commit:** rolled into `6fe3f96` (Task 1).

**2. [Rule 3 — Blocking] `pub(super) fn` insufficient for cross-module test access**
- **Found during:** Task 1 test compile.
- **Issue:** Plan stubs declared `pub(super) fn derive_session_key/seal_message/open_envelope`, but `crate::tests::venice` is not under `crate::llm` so the visibility was rejected. Test stubs needed direct access.
- **Fix:** Promoted to `pub fn`. The functions are still not externally consumable (uniffi-exposed types live in `lib.rs` and `mod.rs`), and the production callers (`run_streaming_inner`, `create_chat_completion_inner`) are inside the same module.
- **Commit:** rolled into `6fe3f96`.

**3. [Rule 1 — Bug] `BackendConfig.preferred_model` does not exist**
- **Found during:** Task 2a, drafting `verify_backend_attestation`.
- **Issue:** Plan stub used `backend.preferred_model.clone().unwrap_or_else(|| "e2ee-venice-uncensored-24b-p".to_string())`, but `BackendConfig` has no `preferred_model` field — only `models: Vec<String>`.
- **Fix:** Helper `pick_attestation_model` returns `backend.models.first()` falling back to `DEFAULT_VENICE_MODEL`. This matches the actual struct shape and the registered Plan 04 preset will populate `models`.
- **Commit:** rolled into `6fe3f96`.

**4. [Rule 1 — Bug] `AttestationEvent::Verified` field set differs from plan stub**
- **Found during:** Task 2a, drafting `verify_backend_attestation`.
- **Issue:** Plan stub used a placeholder `// ... other fields ...` comment; the actual enum at `attestation/mod.rs:65-80` has `report_blob, tls_public_key_fp, vcek_url, vcek_der`. Without populating all named fields the construction would fail.
- **Fix:** Set `report_blob: verified.report_blob.clone()`, `tls_public_key_fp: None`, `vcek_url: None`, `vcek_der: None`. Venice has no TLS pinning (per RESEARCH.md "Don't Hand-Roll") and no SEV VCEK.
- **Commit:** rolled into `6fe3f96`.

**5. [Rule 2 — Critical functionality] `create_chat_completion` needs `stream:false`**
- **Found during:** Task 2b drafting.
- **Issue:** `build_venice_chat_body` unconditionally inserts `stream:true` (correct for streaming path), but the non-streaming `create_chat_completion` would inherit it and break the response shape.
- **Fix:** After serialization, re-parse → set `stream:false` → re-serialize. A single body builder still owns the message-encryption logic; the stream flag is the only delta between paths.
- **Commit:** rolled into `6fe3f96`.

### Authentication gates
None. Plan was fully autonomous.

## Threat Surface Scan

No new external surface beyond what `<threat_model>` already enumerates (T-33-02, T-33-05, T-33-11, T-33-12, T-33-13, T-33-14). All six are mitigated as designed:

- **T-33-02 (Tampering)** — AES-GCM `decrypt` returns `Err` on any bit flip; `handle_venice_sse_event` propagates the error and `stream_decrypted_venice_sse` aborts the stream. Verified via `envelope_round_trip` wrong-key test and the truncated-envelope test.
- **T-33-05 (Nonce reuse)** — `seal_message` calls `rand::thread_rng().fill_bytes(&mut nonce_12)` on every invocation. Verified via `envelope_round_trip` (`assert_ne!(e1, e2)` for same-key/same-plaintext seals) and `request_body_shape` (system + user envelopes differ).
- **T-33-11 (MitM ECDH spoof)** — peer pubkey comes only from `VerifiedVeniceAttestation.signing_pubkey_uncompressed`, which Plan 02 binds to TDX REPORTDATA[0..20] via keccak256.
- **T-33-12 (Test-key leakage)** — production code constructs `EphemeralSecret::random` only; the only place `SecretKey::random` is called is inside `tests/venice.rs::ecdh_aes_round_trip` (server-side simulation).
- **T-33-13 (Fail-open decrypt)** — `open_envelope` returns `Err(LlmError::NetworkError { reason: "Venice E2EE decrypt failed" })` on AEAD tag failure; the stream loop aborts.
- **T-33-14 (Counter-derived nonce regression)** — anti-pattern grep `frame_len|length_prefix|chunk_counter|try_take_frame` returns nothing in `venice.rs`.

No `## Threat Flags` section — nothing new outside the plan's register.

## Known Stubs

- `venice::venice_preset_present` (`#[ignore]`) — Plan 04 scope (VEN-01 backend preset).
- `venice::backend_summary_after_add` (`#[ignore]`) — Plan 04 scope (VEN-09 summary).
- `live_venice::live_attestation_round_trip` (`#[ignore]`) — Plan 04 live integration.
- D7 (tool calling) and D9 (multipart vision content) are deferred to a future phase; multipart bodies are explicitly rejected in `build_venice_chat_body` with a clear `LlmError::NetworkError` rather than silently sending plaintext.

These are documented `#[ignore]`-gated deferrals, not behavioural stubs. The production paths they will exercise (`verify_backend_attestation`, `run_streaming_chat_completion`, header construction) are already wired and unit-tested at the primitive level.

## TDD Gate Compliance

The plan declares each task `tdd="true"`. Per `execute-plan.md` plan-level TDD gate enforcement, the gate sequence requires a `test(...)` commit followed by a `feat(...)` commit. In this plan the production code and the first batch of GREEN tests landed together in `6fe3f96` (`feat`), with subsequent `test(33-03):` commits flipping the remaining RED stubs (`ae13454`, `e08f1bb`). The RED stubs already existed in the repository from Plan 02 (the venice test file was created as a Wave-0 RED scaffold), so the RED gate was satisfied by inherited state rather than a new commit in this plan. This matches the spirit of the plan's `<must_haves>` requirement that "All 5 RED stubs in tests/venice.rs covering VEN-02/07/08 flip GREEN" — the RED tests pre-existed and we flipped them; we did not create new RED tests in this plan.

## Self-Check: PASSED

Files verified to exist:
- `rust/src/llm/venice.rs` (763 lines) — present, `pub mod venice` registered in `rust/src/llm/mod.rs`
- `rust/src/tests/venice.rs` — 5 GREEN, 2 still `#[ignore]` (Plan 04 scope)

Commits verified to exist (via `git log --oneline`):
- `6fe3f96` (Task 1 — feat)
- `ae13454` (Task 2a — test)
- `e08f1bb` (Task 2b — test)

Acceptance grep checks (all 1 for "present", 0 for "absent"):
- `pub mod venice` in `llm/mod.rs`: 1
- `fn derive_session_key` in `venice.rs`: 1
- `fn seal_message` in `venice.rs`: 1
- `fn open_envelope` in `venice.rs`: 1
- `pub fn format_attestation_url` in `venice.rs`: 1
- `b"ecdsa_encryption"` in `venice.rs`: 1
- `EphemeralSecret` in `venice.rs`: 4 (import + 2 `random` call sites + signature)
- `rand::thread_rng().fill_bytes` in `venice.rs`: 1
- `pub fn build_http_client` / `pub fn model_list_url` / `pub async fn verify_backend_attestation` / `pub async fn create_chat_completion` / `pub async fn run_streaming_chat_completion` / `pub async fn run_streaming_chat_completion_from_api_messages`: all 1
- `invalidate_cached_venice_attestation` in `venice.rs`: 2 (streaming + create paths)
- `enable_e2ee` in `venice.rs`: 1
- `take_sse_event` / `handle_venice_sse_event` in `venice.rs`: 1 / 1
- Anti-pattern grep `frame_len|length_prefix|chunk_counter|try_take_frame`: 0

Test counts:
- `cargo test -p mango_core --lib venice -- --nocapture`: 9 passed (5 venice + 4 attestation_venice), 5 ignored
- `cargo test -p mango_core --lib`: 347 passed, 0 failed, 15 ignored
- `cargo build -p mango_core --release`: clean (3 inherited dead-code warnings)

Plan 04 hand-off: the public surface listed in `<interfaces>` is fully exported; Plan 04 wires it into `transport.rs`/`backend.rs` as a `VeniceE2ee` `ProviderTransportKind` variant.
