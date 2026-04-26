---
phase: 34
plan: 03
subsystem: llm-transport
tags:
  - redpill
  - llm-transport
  - openai-compatible
  - provider-preset
  - tinfoil-refusal
  - wave-2
dependency_graph:
  requires:
    - Phase 34 Plan 01 (golden fixtures + RED test stubs)
    - Phase 34 Plan 02 (attestation/redpill.rs + ensure_verified_redpill_attestation + RedpillError::TinfoilUnsupported)
    - rust/src/llm/venice.rs (transport analog — copied structure, dropped E2EE wrapper)
    - rust/src/llm/backend.rs (ProviderKind enum, known_provider_presets)
    - async_openai 0.34 (OpenAIConfig::with_api_base, Client::with_http_client)
  provides:
    - rust/src/llm/redpill.rs (vanilla OpenAI-compatible transport — no E2EE)
    - ProviderKind::Redpill variant + 'redpill' provider_kind dispatch
    - known_provider_presets() Redpill row (Settings → Providers picker)
    - check_model_routable (Tinfoil-route refusal, T-34-08 gate)
    - map_redpill_error_for_user (UI-friendly error mapping)
  affects:
    - Plan 34-04 (transport.rs/router.rs/streaming.rs wiring + live integration tests)
tech-stack:
  added: []
  patterns:
    - "Vanilla async-openai client with OpenAIConfig::with_api_base + with_http_client — no envelope/HPKE wrapping (CONTEXT D-21: TEE attestation is the v1 confidentiality root)"
    - "Two-phase gate ordering: check_model_routable BEFORE ensure_verified_redpill_attestation BEFORE async-openai POST. Tinfoil-routed models fail closed at the model-list level; no attestation fetch is attempted (T-34-08)."
    - "User-facing error mapping centralized in map_redpill_error_for_user — TinfoilUnsupported variant gets the precise Settings → Providers → direct-Tinfoil hint."
    - "model_list_url and format_redpill_attestation_url tolerate base-url variants (`/v1`, `/v1/`, bare host) by trimming both `/` and `/v1` before re-appending the canonical path. Three #[cfg(test)] cases pin this contract."
key-files:
  created:
    - rust/src/llm/redpill.rs (497 lines)
  modified:
    - rust/src/llm/mod.rs (`pub mod redpill;`)
    - rust/src/llm/backend.rs (ProviderKind::Redpill variant + 'redpill' arm + Redpill preset row)
    - rust/src/attestation/task.rs (ProviderKind::Redpill dispatch arm — Rule 3 auto-fix)
    - rust/src/tests/redpill.rs (3 RED stubs flipped to executable assertions; 1 stub remains for Plan 04)
decisions:
  - "No new ReportDataLayout variant or new Cargo deps — every primitive needed (urlencoding, async-openai, reqwest, futures, serde_json, tokio-util, flume) is already in the workspace from Phase 33."
  - "Streaming uses async-openai's create_stream() directly (CreateChatCompletionStreamResponse). NO custom SSE parsing — Redpill chat completions are vanilla OpenAI-compatible, so the upstream library's SSE handling is sufficient. Cancellation handled via tokio::select! biased on cancel_token."
  - "Tinfoil-route gate wired into BOTH create_chat_completion AND run_streaming_inner (via map_redpill_error_for_user). Defense-in-depth: Plan 02 already detects HTTP 502 'Unsupported Tinfoil' body in fetch_and_verify_redpill_attestation."
  - "verify_backend_attestation populates AttestationEvent::Verified with the full field set from attestation/mod.rs (report_blob: Vec::new(), tls_public_key_fp: None, vcek_url/der: None — these only apply to AMD SEV-SNP backends, not Redpill TDX)."
  - "Inline #[cfg(test)] mod replaces what would otherwise be 3 separate tests files: model_list_url variants, format_redpill_attestation_url URL-encoding, and the user-facing error mapping. Cross-file integration assertions live in tests/redpill.rs."
metrics:
  duration: ~25 minutes
  completed: 2026-04-26
  tasks: 2
  commits: 3
---

# Phase 34 Plan 03: Redpill LLM Transport Summary

Built the vanilla OpenAI-compatible LLM transport layer for Redpill in `rust/src/llm/redpill.rs` (497 lines): HTTP client builder, attestation URL formatter, model-list URL helper, attestation-gated `create_chat_completion`, two streaming entry points (ChatMessage and api-message variants) wrapping `async_openai::create_stream`, the `check_model_routable` Tinfoil-route gate, and a centralized user-facing error mapper. Added `ProviderKind::Redpill` + dispatch arm and a Redpill preset row (`https://api.redpill.ai/v1/`, IntelTdx, "Intel TDX aggregator (Phala / NearAI / Chutes) — multi-quote attestation"). Three RED stubs (RED-01 preset, RED-02 attestation URL format, RED-10 typed Tinfoil error) flipped GREEN; full lib suite at 378 passed / 0 failed; release build clean; zero new Cargo deps; zero E2EE crypto.

## What Shipped

### Public API (`rust/src/llm/redpill.rs`)

| Symbol | Kind | Purpose |
|---|---|---|
| `build_http_client(timeout)` | fn | rustls-TLS reqwest client (v1 Cargo profile mirrors Venice) |
| `model_list_url(backend)` | fn | `{base}/v1/models` (trims `/` + `/v1`) |
| `format_redpill_attestation_url(model, nonce_hex, base_url)` | fn | `{base}/v1/attestation/report?model=<urlenc>&nonce=<hex>` (D-02 — no Authorization) |
| `verify_backend_attestation(backend, tdx_policy)` | async fn | Wrapper around `ensure_verified_redpill_attestation` → `AttestationEvent::Verified { tee_type: "IntelTdx", … }` |
| `create_chat_completion(backend, model, messages, tools)` | async fn | check_model_routable → ensure_verified → async-openai POST |
| `run_streaming_chat_completion(backend, model, ChatMessage[], cancel, core_tx)` | async fn | ChatMessage → OpenAI-typed → stream entry |
| `run_streaming_chat_completion_from_api_messages(backend, model, ChatCompletionRequestMessage[], tools, cancel, core_tx)` | async fn | OpenAI-typed direct entry — used by Plan 04's tool-followup wiring |
| `check_model_routable(backend, model)` | async fn | GET `/v1/models` → refuses `providers: ["tinfoil"]` with `RedpillError::TinfoilUnsupported` (T-34-08) |
| `map_redpill_error_for_user(e)` | fn (private) | UX-grade error mapping — Tinfoil variant gets the Settings → Providers → direct-Tinfoil hint |

### ProviderKind extension (`rust/src/llm/backend.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Tinfoil,
    Ppq,
    Venice,
    Redpill,   // ← added
    Custom,
}
```

`BackendConfig::provider_kind()` arm:
```rust
"redpill" => ProviderKind::Redpill,
```

`attestation/task.rs` dispatch arm:
```rust
ProviderKind::Redpill => {
    crate::llm::redpill::verify_backend_attestation(&backend, &policy.tdx).await?
}
```

### Preset entry (verbatim — Plan 04 surfaces this in Settings → Providers)

```rust
ProviderPreset {
    id: "redpill".into(),
    name: "Redpill".into(),
    base_url: "https://api.redpill.ai/v1/".into(),
    tee_type: TeeType::IntelTdx,
    description: "Intel TDX aggregator (Phala / NearAI / Chutes) — multi-quote attestation".into(),
}
```

Listed AFTER Venice in `known_provider_presets()`, matching the Settings UI ordering convention.

### RED → GREEN tally

| Stub | RED commit | GREEN test | Notes |
|---|---|---|---|
| RED-01 — `redpill_preset_present` | 7a6de21 | b8e8727 | asserts name=Redpill, base starts with `https://api.redpill.ai/v1`, IntelTdx, description mentions aggregator/phala/Intel TDX |
| RED-02 — `attestation_url_format` | 7a6de21 | b8e8727 | asserts URL ends with `/v1/attestation/report?model=openai%2Fgpt-oss-20b&nonce=abc123`; both trailing-slash and bare `/v1` base-URL variants |
| RED-10 — `tinfoil_route_refused_with_typed_error` | 7a6de21 | b8e8727 | asserts `RedpillError::TinfoilUnsupported` Display string contains "tinfoil" + "direct" hint |

Plus 4 new inline unit tests pinning ancillary contracts:

| Test | Pins |
|---|---|
| `model_list_url_trims_trailing_v1_and_slash` | three base-url variants → one canonical `/v1/models` URL |
| `format_attestation_url_urlencodes_model_id` | URL-encodes `/` in model ID; query-string ordering |
| `tinfoil_user_facing_error_mentions_direct_tinfoil` | UI message contains "tinfoil" + "direct[-tinfoil]" + "settings"/"provider" |
| `non_tinfoil_redpill_error_passes_through_with_prefix` | other RedpillError variants prefixed with `"Redpill:"` |

`tests::redpill::backend_summary_after_add` remains `#[ignore]` — Plan 04 owns RED-11.

## Validation Commands

```bash
# Plan 03 deterministic suite (must pass)
cd rust && cargo test -p mango_core --lib redpill
# → 28 passed; 0 failed; 4 ignored
#   (3 live #[ignore] for Plan 04; 1 RED-11 for Plan 04)

# Full lib (no regressions)
cd rust && cargo test -p mango_core --lib
# → 378 passed; 0 failed; 18 ignored

# Release build
cd rust && cargo build -p mango_core --release
# → exits 0

# Zero E2EE crypto in transport layer
grep -E 'EphemeralSecret|Aes256Gcm|Hkdf|seal_message|open_envelope' rust/src/llm/redpill.rs
# → empty (zero hits)

# Zero new deps
git diff df5cc0985d9498d25caa113d3376eeee6978d903 rust/Cargo.toml
# → empty
```

## Acceptance Criteria — Verified

| Criterion | Result |
|---|---|
| `rust/src/llm/redpill.rs` exists ≥ 200 lines | yes (497 lines) |
| `pub mod redpill` in llm/mod.rs | yes |
| `ProviderKind::Redpill` in backend.rs | yes |
| `"redpill" =>` provider_kind match arm | yes |
| Redpill preset entry (`api.redpill.ai`) in `known_provider_presets()` | yes |
| `pub fn format_redpill_attestation_url` | yes |
| `pub fn model_list_url` | yes |
| `pub async fn verify_backend_attestation` | yes |
| `pub async fn create_chat_completion` | yes |
| `pub async fn run_streaming_chat_completion` | yes |
| `pub async fn run_streaming_chat_completion_from_api_messages` | yes |
| `ensure_verified_redpill_attestation` called before chat (T-34-09) | yes (in `create_chat_completion` and `run_streaming_inner`) |
| `pub async fn check_model_routable` | yes |
| `TinfoilUnsupported` referenced in transport | yes (4 sites) |
| `check_model_routable` used in `create_chat_completion` AND streaming entry points | yes (6 references) |
| "direct" hint in user-facing error path (attestation/redpill.rs and llm/redpill.rs) | yes (1 + 3) |
| `cargo test -p mango_core --lib redpill::redpill_preset_present redpill::attestation_url_format` | both pass |
| `cargo build -p mango_core --release` exits 0 | yes |
| No E2EE crypto in `llm/redpill.rs` | confirmed (grep empty) |
| Zero new Cargo deps | confirmed (rust/Cargo.toml diff empty) |

## Deviations from Plan

**One Rule-3 auto-fix:**

**1. [Rule 3 — Blocking] Add ProviderKind::Redpill arm to `attestation/task.rs::run_attestation_blocking`**
- **Found during:** Task 1 build
- **Issue:** Adding the `ProviderKind::Redpill` enum variant turned the existing `match backend.provider_kind()` in `attestation/task.rs` into a non-exhaustive pattern (E0004). The plan's `<files>` block did not list `attestation/task.rs`, but the change is required for the crate to compile after the variant is added.
- **Fix:** Added the dispatch arm `ProviderKind::Redpill => crate::llm::redpill::verify_backend_attestation(&backend, &policy.tdx).await?` immediately after the Venice arm. Mirrors the Venice integration and uses `policy.tdx` (Redpill is Intel TDX).
- **Files modified:** `rust/src/attestation/task.rs`
- **Commit:** b8e8727

**Folded Task 2 into Task 1's commit + a follow-up test commit:**

The plan's Task 2 (Tinfoil-routed gate) was naturally implemented in Task 1 because the Tinfoil-route check sits inside `create_chat_completion` and `run_streaming_inner` — splitting it across two commits would have required either a transient broken state or an artificial helper. Instead:
- Task 1 commit (b8e8727) ships `check_model_routable` + `map_redpill_error_for_user` + the gate ordering, AND flips RED-01/RED-02/RED-10 GREEN.
- A separate Task 2 commit (b1cc106) adds two unit tests that pin the user-facing error contract (`tinfoil_user_facing_error_mentions_direct_tinfoil`, `non_tinfoil_redpill_error_passes_through_with_prefix`).

The HTTP-502 defense-in-depth path (D-16) was already wired in Plan 02 — `attestation/redpill.rs::fetch_and_verify_redpill_attestation` returns `RedpillError::TinfoilUnsupported` when the response is HTTP 502 with body "Unsupported Tinfoil". Verified by `grep 'Unsupported Tinfoil' rust/src/attestation/redpill.rs` → 2 sites (doc comment + body match).

Otherwise: plan executed as written.

## Authentication Gates

None. The attestation endpoint is public per CONTEXT D-02; the chat completions endpoint accepts the user-supplied API key on the existing `BackendConfig::api_key` field. Live network tests are deferred to Plan 04 (`#[ignore]`-gated).

## Threat Mitigation Recap

| Threat | Mitigated By | Test |
|---|---|---|
| T-34-05 (provider preset spoofing) | Preset hard-coded in `known_provider_presets()`; router matches on `id == "redpill"` literal; user enters API key only. | `redpill_preset_present` |
| T-34-08 (Tinfoil-routed model bypassing direct-Tinfoil's stronger SEV-SNP path) | `check_model_routable` returns `RedpillError::TinfoilUnsupported` BEFORE attestation fetch + chat. Defense-in-depth: Plan 02's HTTP-502 detection inside `fetch_and_verify_redpill_attestation` returns the same typed error. | `tinfoil_route_refused_with_typed_error` + `tinfoil_user_facing_error_mentions_direct_tinfoil` |
| T-34-09 (chat sent before attestation verifies) | `create_chat_completion` and `run_streaming_inner` call `ensure_verified_redpill_attestation` BEFORE constructing the async-openai request. Attestation failure short-circuits with `LlmError::NetworkError` — no chat is sent. | gated by Plan 02's attestation tests; live exercise in Plan 04. |

## Commits

| Phase | Commit | Subject |
|---|---|---|
| TDD RED | 7a6de21 | test(34-03): flip RED-01/RED-02/RED-10 stubs to executable assertions |
| TDD GREEN (Task 1) | b8e8727 | feat(34-03): llm/redpill.rs transport + ProviderKind::Redpill + preset |
| TDD GREEN (Task 2) | b1cc106 | test(34-03): pin Tinfoil-route user-facing error contract (RED-10 mitigation) |

## Self-Check

- [x] `rust/src/llm/redpill.rs` exists (497 lines)
- [x] `rust/src/llm/mod.rs` declares `pub mod redpill;`
- [x] `rust/src/llm/backend.rs` has `ProviderKind::Redpill` + `"redpill"` arm + Redpill preset entry
- [x] `rust/src/attestation/task.rs` has `ProviderKind::Redpill` dispatch arm
- [x] `rust/src/tests/redpill.rs` has 3 GREEN tests + 1 #[ignore] (RED-11 for Plan 04)
- [x] All three commits exist in git log: 7a6de21, b8e8727, b1cc106
- [x] `cargo test -p mango_core --lib redpill`: 28 passed, 0 failed, 4 ignored
- [x] `cargo test -p mango_core --lib`: 378 passed, 0 failed
- [x] `cargo build -p mango_core --release` exits 0
- [x] `grep -E 'EphemeralSecret|Aes256Gcm|Hkdf|seal_message|open_envelope' rust/src/llm/redpill.rs` is empty
- [x] `git diff df5cc09 rust/Cargo.toml` is empty (zero new deps)

## Self-Check: PASSED
