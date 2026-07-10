---
phase: 33
plan: 02
subsystem: rust-core/attestation
tags:
  - venice
  - tdx
  - reportdata-layout
  - keccak256-binding
  - nvidia-nras
  - attestation
  - wave-2
dependency-graph:
  requires:
    - 33-01-SUMMARY.md (k256/sha3/urlencoding deps, golden capture, RED stubs, MRSEAM verdict "Present")
    - rust/src/attestation/tdx.rs (existing verify_tdx_quote at line 59)
    - rust/src/attestation/nvidia.rs (existing fetch_and_verify_nvidia)
    - rust/src/attestation/policy.rs (TdxPolicy::default with Venice MRSEAM at index 1)
    - rust/src/llm/{BackendConfig, LlmError}
  provides:
    - rust/src/attestation/tdx.rs::ReportDataLayout enum (NonceFirst32, VeniceAddrPadNonce)
    - rust/src/attestation/venice.rs (REPORTDATA decoder, response struct, orchestrator, cache)
    - VerifiedVeniceAttestation handle (consumed by Plan 03 transport)
    - ensure_verified_venice_attestation (consumed by Plan 03 transport)
    - invalidate_cached_venice_attestation (consumed by Plan 03 transport on retryable failures)
  affects:
    - rust/src/attestation/mod.rs (registers nonce, nvidia, tdx, venice modules — previously orphaned)
    - rust/src/tests/mod.rs (registers attestation_tdx, attestation_nvidia — previously orphaned)
    - rust/src/tests/attestation_tdx.rs (single caller updated to NonceFirst32)
    - rust/src/tests/attestation_venice.rs (4 RED → GREEN; 2 stay #[ignore] with explicit deferral)
tech-stack:
  added: []
  patterns:
    - "ReportDataLayout enum parameterises verify_tdx_quote (D1: parameterise, not fork)"
    - "Cache pattern reused from llm/ppq_private.rs::ensure_verified_attestation (Lazy<Mutex<HashMap>>, in-memory only, ZeroizeOnDrop on eviction)"
    - "NRAS double-parse: nvidia_payload typed as Option<String>, inner content sanity-parsed then forwarded to fetch_and_verify_nvidia (Pitfall 2)"
    - "Decoder is pure: takes &[u8; 64] + pubkey hex + nonce — fully unit-testable against golden capture without network"
key-files:
  created:
    - rust/src/attestation/venice.rs
  modified:
    - rust/src/attestation/mod.rs
    - rust/src/attestation/tdx.rs
    - rust/src/tests/mod.rs
    - rust/src/tests/attestation_tdx.rs
    - rust/src/tests/attestation_venice.rs
decisions:
  - "Per Wave 0 MRSEAM reconcile: Venice MRSEAM already present at TdxPolicy::default index 1 — policy.rs UNCHANGED (no-op as expected)"
  - "Discovered attestation::{tdx, nvidia, nonce} modules were orphaned (no pub mod declarations) and the matching test files (attestation_tdx, attestation_nvidia) were not in tests/mod.rs — registered both as Rule 3 blocking work, otherwise venice.rs cannot reach super::tdx::ReportDataLayout and the existing-tests-still-pass acceptance criterion is unverifiable"
  - "Used crate::llm::BackendConfig (not crate::config::BackendConfig as the plan stub suggested) — verified via mod.rs export at rust/src/llm/mod.rs:11"
  - "Tests requiring synthetic TDX quote construction or live PCCS collateral (tdx_debug_bit_rejected, tdx_verify_golden_capture_signature) remain #[ignore] with explicit deferral to Plan 04 live integration — the plan body authorised this when 'constructing a quote from scratch is too heavy'"
  - "TTL = 4h matches existing PPQ private cache convention; D3/Pitfall 5 enforced by static Lazy<Mutex<HashMap>> with no SQLite write-path"
metrics:
  duration: ~12 minutes
  completed-date: 2026-04-26
  tasks-completed: 2
  files-created: 1
  files-modified: 5
  commits: 2
---

# Phase 33 Plan 02: Venice attestation layer — Summary

**One-liner:** Built Venice's TDX REPORTDATA decoder + full attestation orchestrator with in-memory 4h cache, parameterised `verify_tdx_quote` via a `ReportDataLayout` enum so the same TDX verifier drives both Tinfoil/PPQ (`NonceFirst32`) and Venice/Phala dstack (`VeniceAddrPadNonce`), and flipped 4 of 6 Wave 0 RED stubs to GREEN against the golden capture.

## What was built

### Task 1 — `verify_tdx_quote` parameterised by REPORTDATA layout

`rust/src/attestation/tdx.rs`:
- New `pub enum ReportDataLayout { NonceFirst32, VeniceAddrPadNonce }`.
- `verify_tdx_quote` gains a fourth `layout: ReportDataLayout` parameter.
- Nonce comparison branches on the layout: `NonceFirst32` keeps the existing `report_data[..32]` slice (byte-identical to pre-refactor); `VeniceAddrPadNonce` reads `report_data[32..64]` and surfaces an explicit `QuoteVerification` error if the report is shorter than 64 bytes.

Single existing caller (`tests/attestation_tdx.rs::test_verify_tdx_quote_short_input`) updated to pass `ReportDataLayout::NonceFirst32`.

**Side cleanup (Rule 3 — blocking):** `attestation::{tdx, nvidia, nonce}` were orphaned modules (no `pub mod` declarations in `attestation/mod.rs`) and their test files (`attestation_tdx`, `attestation_nvidia`) were not in `tests/mod.rs`. Without registering both, (a) `venice.rs` cannot reach `super::tdx::ReportDataLayout`, and (b) the plan's acceptance criterion "existing TDX tests still pass" is unverifiable. Added `pub mod` lines and test registrations.

### Task 2 — `attestation/venice.rs` (373 lines, new)

Public surface (consumed by Plan 03):
- `VerifiedVeniceAttestation` — `#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]` handle holding `signing_pubkey_uncompressed: [u8; 65]`, `submitted_nonce: [u8; 32]`, `report_blob`, `expires_at`. The pubkey + nonce are zeroed on drop.
- `VeniceAttestationResponse` — wire format with `#[serde(alias = "signing_public_key")]` (Pitfall 3) and `nvidia_payload: Option<String>` (Pitfall 2 — JSON-as-string).
- `verify_venice_report_data(&[u8; 64], pk_hex, &[u8; 32]) -> Result<(), AttestationError>` — the four-check decoder (uncompressed form, keccak-256 address binding at `[0..20]`, zero pad at `[20..32]`, nonce echo at `[32..64]`).
- `fetch_and_verify_venice_attestation` — full orchestrator: fresh 32-byte nonce → URL-encoded `GET /api/v1/tee/attestation?model=…&nonce=…` (no Authorization header per D14, 30s timeout) → `verify_tdx_quote(..., VeniceAddrPadNonce)` (signature + Phala PCCS collateral + nonce-at-32 via existing path) → re-parse to gate `td_attributes[0] & 0x01 == 0` (Pitfall 6 / VEN-06) → `verify_venice_report_data` → model echo gate → `nvidia_payload` JSON-as-string sanity-parse → forward to `super::nvidia::fetch_and_verify_nvidia` → build handle with `expires_at = now + 4h`.
- `ensure_verified_venice_attestation` — `Lazy<Mutex<HashMap<"{base_url}|{model}", VerifiedVeniceAttestation>>>` cache; expired entries evicted (ZeroizeOnDrop wipes secret material). **In-memory only** — no SQLite write path (`rg 'attestation_records.*[Vv]enice'` returns nothing).
- `invalidate_cached_venice_attestation` — manual eviction hook for transport-layer 401 / signing failures.

Module registered as `pub mod venice;` in `attestation/mod.rs`.

### Test results

`cargo test -p mango_core --lib attestation_venice`:
```
running 6 tests
test reportdata_address_mismatch ........ ok    (RED → GREEN)
test reportdata_padding_nonzero ......... ok    (RED → GREEN)
test reportdata_nonce_mismatch .......... ok    (RED → GREEN)
test reportdata_layout_ok ............... ok    (RED → GREEN)
test tdx_debug_bit_rejected ............. ignored (deferred to Plan 04 live)
test tdx_verify_golden_capture_signature  ignored (deferred to Plan 04 live)
result: ok. 4 passed; 0 failed; 2 ignored
```

`cargo test -p mango_core --lib attestation_tdx`:
```
running 6 tests
test test_decode_quote_base64 ........... ok
test test_decode_quote_too_short ........ ok
test test_decode_quote_garbage .......... ok
test test_nonce_uniqueness .............. ok
test test_decode_quote_hex .............. ok
test test_verify_tdx_quote_short_input .. ok    (passes after layout-enum refactor)
result: ok. 6 passed
```

Full suite: **342 passed; 0 failed; 20 ignored** (was 329/0/24 — gained 6 attestation_tdx + 5 attestation_nvidia + 4 venice GREEN; net `#[ignore]` count down by 4).

## Tests that remain `#[ignore]` (with reason)

| Test | Reason |
|------|--------|
| `attestation_venice::tdx_debug_bit_rejected` | Constructing a synthetic minimal TDX quote with `td_attributes[0] |= 0x01` is heavyweight; debug-bit gate path is exercised by Plan 04 live integration test (VEN-06). The branch itself is straight-line code at `venice.rs:233-237`. |
| `attestation_venice::tdx_verify_golden_capture_signature` | Requires a fresh Phala PCCS collateral round-trip against the recorded golden capture; collateral expires and the golden quote may not verify offline against current TCB updates. Covered by Plan 04 live test (VEN-03). |
| 7 stubs in `tests/venice.rs` | Reserved for Plan 03 (transport: ECDH, AES round-trip, envelope, request body, attestation URL builder) and Plan 04 (preset, backend summary). |
| 1 stub in `tests/live_venice.rs` | Plan 04 live integration smoke (gated on `VENICE_API_KEY`). |

## MRSEAM action

**No `policy.rs` change.** Wave 0 reconcile (`33-MRSEAM-RECONCILE.md`) verified that the captured Venice MRSEAM `7bf063280e94fb051f5dd7b1fc59ce9aac42bb961df8d44b709c9b0ff87a7b4df648657ba6d1189589feab1d5a3c9a9d` already sits at index 1 of `TdxPolicy::default().accepted_mr_seams`. The default policy accepts Venice as-is.

## RED → GREEN tally

| Source | Pre-Plan-02 | Post-Plan-02 |
|--------|-------------|--------------|
| `attestation_venice` deterministic stubs (VEN-04*) | 4 RED `#[ignore]` | **4 GREEN** |
| `attestation_venice` heavy stubs (VEN-03, VEN-06) | 2 RED `#[ignore]` | 2 still `#[ignore]` (deferred to Plan 04) |
| `attestation_tdx` (regression — was orphaned) | 0 running | **6 GREEN** |
| `attestation_nvidia` (regression — was orphaned) | 0 running | **5 GREEN** |

## Commits

| # | Task | Type | Hash | Files |
|---|------|------|------|-------|
| 1 | Task 1 — ReportDataLayout enum + caller updates + module registration | refactor | `e75d330` | `attestation/mod.rs`, `attestation/tdx.rs`, `tests/mod.rs`, `tests/attestation_tdx.rs` |
| 2 | Task 2 — attestation/venice.rs + RED→GREEN test bodies | feat | `7018eb2` | `attestation/venice.rs` (new), `attestation/mod.rs`, `tests/attestation_venice.rs` |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `attestation::{tdx, nvidia, nonce}` were orphaned modules; matching test files were orphaned too**
- **Found during:** Task 1 (initial `cargo test attestation_tdx` returned `running 0 tests`).
- **Issue:** `rust/src/attestation/mod.rs` did not declare `pub mod tdx; pub mod nvidia; pub mod nonce;`, and `rust/src/tests/mod.rs` did not include `mod attestation_tdx; mod attestation_nvidia;`. The files existed and contained code, but were never compiled. Without these registrations, `venice.rs` cannot resolve `super::tdx::ReportDataLayout` and the plan's acceptance criterion "existing TDX tests still pass after layout-enum refactor" cannot be observed.
- **Fix:** Added the three `pub mod` lines and two `mod` lines.
- **Files modified:** `rust/src/attestation/mod.rs`, `rust/src/tests/mod.rs`.
- **Commit:** `e75d330` (rolled into Task 1).

**2. [Rule 1 — Bug] Plan-suggested `crate::config::BackendConfig` path doesn't exist**
- **Found during:** Task 2 initial compile.
- **Issue:** Plan's stub used `use crate::config::BackendConfig;` but no `rust/src/config.rs` exists; `BackendConfig` lives in `crate::llm::BackendConfig` (re-exported from `llm/backend.rs` via `llm/mod.rs:11`).
- **Fix:** Used `use crate::llm::BackendConfig; use crate::llm::LlmError;`.
- **Commit:** `7018eb2` (Task 2).

**3. [Rule 1 — Bug] `sha3::Keccak256::new()` in test helper required `Digest` trait in scope**
- **Found during:** First test compile.
- **Issue:** `Keccak256::new()` resolves to `CoreWrapper<…>::new` only when the `Digest` trait is imported; the helper used unqualified `sha3::Digest::update(&mut h, …)` which compiled but `::new()` did not.
- **Fix:** Added `use sha3::Digest;` inside the `make_valid_report_data` helper and switched to method-call form.
- **Commit:** `7018eb2` (Task 2).

### Authentication gates
None. Plan was fully autonomous.

## Threat Surface Scan

No new external surface introduced beyond what the plan's `<threat_model>` already enumerates (T-33-01, T-33-04, T-33-08, T-33-09, T-33-10). All five threats are mitigated as designed:
- **T-33-01** (REPORTDATA replay across providers): `ReportDataLayout` enum is an explicit named parameter — silent layout drift now requires a recompile, not a runtime accident. Address binding + zero-pad checks in `verify_venice_report_data` add defence-in-depth.
- **T-33-04** (TDX debug mode): explicit `td_attributes[0] & 0x01 != 0` gate at `venice.rs:233-237`, after `dcap_qvl::verify` succeeds.
- **T-33-08** (stale quote replay): per-request 32-byte nonce; cache TTL 4h, in-memory only, zero-on-drop.
- **T-33-09** (mocked signing keys): pubkey rejected unless 65 bytes starting with `0x04`; address binding ties pubkey to TDX REPORTDATA.
- **T-33-10** (NRAS replay/forge): reuses existing `attestation::nvidia::fetch_and_verify_nvidia` (pinned RS256 + issuer + JWKS); inner JSON sanity-parsed before forwarding.

No `## Threat Flags` section — nothing new outside the plan's register.

## Known Stubs

- `attestation_venice::tdx_debug_bit_rejected` (`#[ignore]`) — synthetic-quote test deferred to Plan 04 live coverage of VEN-06.
- `attestation_venice::tdx_verify_golden_capture_signature` (`#[ignore]`) — golden-capture signature verify deferred to Plan 04 live coverage of VEN-03.

These are documented `#[ignore]`-gated tests, not behavioural stubs. The production code paths they exercise (`venice.rs::fetch_and_verify_venice_attestation`'s debug-bit branch and the underlying `verify_tdx_quote` call) are wired and will be exercised end-to-end by Plan 04.

## Self-Check: PASSED

Files verified to exist:
- `rust/src/attestation/venice.rs` (373 lines) — present
- `rust/src/attestation/mod.rs` — `pub mod venice` present
- `rust/src/attestation/tdx.rs` — `pub enum ReportDataLayout` + `NonceFirst32` + `VeniceAddrPadNonce` present
- `rust/src/tests/attestation_venice.rs` — 4 GREEN + 2 `#[ignore]`
- `rust/src/tests/attestation_tdx.rs` — single caller updated to `ReportDataLayout::NonceFirst32`

Commits verified to exist:
- `e75d330` (Task 1) — verified via `git log --oneline`
- `7018eb2` (Task 2) — verified via `git log --oneline`

Acceptance grep checks (all 1 for "present", 0 for "absent"):
- `pub enum ReportDataLayout` in tdx.rs: 1
- `pub mod venice` in attestation/mod.rs: 1
- `pub fn verify_venice_report_data` in venice.rs: 1
- `pub async fn ensure_verified_venice_attestation` in venice.rs: 1
- `fetch_and_verify_nvidia` references in venice.rs: 3 (use comment + import + call)
- `VeniceAddrPadNonce` references in venice.rs: 2
- `td_attributes[0] & 0x01` in venice.rs: 2 (gate + error reason)
- `serde(alias = "signing_public_key")` in venice.rs: 1
- `nvidia_payload: Option<String>` in venice.rs: 1
- `attestation_records.*[Vv]enice` anywhere in `rust/src/`: 0 (D3/Pitfall 5 honoured)

Test counts:
- `cargo test -p mango_core --lib attestation_venice`: 4 passed, 2 ignored
- `cargo test -p mango_core --lib attestation_tdx`: 6 passed
- `cargo test -p mango_core --lib`: 342 passed, 0 failed, 20 ignored

`cargo build -p mango_core`: clean (3 dead-code warnings on `decode_quote`, `generate_nonce`, `NonceFirst32` — all expected, all consumed by tests / Plan 03 transport).
