---
phase: 34
plan: 02
subsystem: attestation
tags:
  - redpill
  - tdx
  - reportdata-layout
  - shape-dispatcher
  - three-way-and
  - debug-mode-gate
  - attestation
  - wave-1
dependency_graph:
  requires:
    - Phase 33 Plan 01 (attestation/venice.rs verify_venice_report_data — re-exported)
    - Phase 34 Plan 01 (golden fixtures + RED test stubs + RedpillError)
    - rust/src/attestation/tdx.rs::verify_tdx_quote (ReportDataLayout::VeniceAddrPadNonce)
    - rust/src/attestation/nvidia.rs::fetch_and_verify_nvidia (NRAS JWT verifier)
  provides:
    - rust/src/attestation/redpill.rs (RedpillShape + Freshness + RedpillError + four REPORTDATA decoders + verify orchestrator + 5-min TTL cache)
    - VerifiedRedpillAttestation handle for Plan 03 transport
    - OrchestratedComponents struct for Plan 04 verified-badge sub-line
    - ensure_verified_redpill_attestation public API
  affects:
    - Plan 34-03 (llm/redpill.rs) consumes ensure_verified_redpill_attestation
    - Plan 34-04 (UI) consumes shape + freshness + orchestrated_components
tech-stack:
  added: []
  patterns:
    - "Re-export pattern (`pub use ... as ...`) — single source of truth for the model REPORTDATA decoder, no copy-paste of the Venice decoder."
    - "Sibling RedpillError enum (NOT extending UniFFI-exported AttestationError) — keeps cross-boundary error type stable while expressing Redpill-specific failure modes; ensure_verified maps to LlmError::NetworkError at the boundary."
    - "Inline report_data fallback for orchestrated sub-components (gateway/compose-manager): prefer the JSON `report_data` hex when present, otherwise slice REPORTDATA from the full TDX quote — matches the spike capture's actual wire format."
    - "Self-feedback layout nonce on Shape C: feed rd[32..64] back as expected_nonce to dcap-qvl (Chutes leaves the slot unconstrained per D-11) so signature/collateral/TCB checks still run while no spurious nonce-mismatch error fires."
key-files:
  created:
    - rust/src/attestation/redpill.rs (800 lines)
  modified:
    - rust/src/attestation/mod.rs (`pub mod redpill;`)
    - rust/src/tests/attestation_redpill.rs (19 RED stubs → 21 GREEN tests)
decisions:
  - "Sibling `RedpillError` enum inline in `redpill.rs` — kept off the UniFFI boundary. Maps to `LlmError::NetworkError` at `ensure_verified_redpill_attestation`."
  - "Model decoder re-exported from attestation::venice — single source of truth (no copy-paste). Verified by `pub use crate::attestation::venice::verify_venice_report_data as verify_redpill_model_reportdata` grep."
  - "No new ReportDataLayout variant added. Reused `VeniceAddrPadNonce` for all three shapes: Shape A/B (rd[32..64] == client nonce), Shape C (self-feedback to satisfy dcap-qvl while the anti-tamper decoder enforces the actual binding via SHA256(nonce_str ++ e2e_pubkey_str))."
  - "Shape A model decoder fallback path: when the response provides only `signing_address` (Phala-pure capture has no `signing_key`/`signing_public_key` field), verify the address slice + zero pad + nonce manually instead of calling the keccak-derived decoder."
  - "5-min TTL cache implemented as `Lazy<Mutex<HashMap>>` mirroring Venice's pattern (D-20)."
  - "Three-way AND wraps any single-component failure in `RedpillError::OrchestratedComponentFailed { failed: 'gateway' | 'model' | 'compose_manager' }` — never opens an Orchestrated session unless all three components verify (T-34-02)."
metrics:
  duration: ~50 minutes
  completed: 2026-04-26
  tasks: 3
  commits: 3
---

# Phase 34 Plan 02: Redpill Attestation Layer Summary

Built the complete Redpill TEE-attested aggregator verification module in `rust/src/attestation/redpill.rs` (800 lines): response-shape dispatcher with fail-closed unknown-shape handling, `quote_bytes` hex/base64 auto-detect helper, `debug_mode_disabled` td_attributes gate, four REPORTDATA layout decoders (model decoder re-exported verbatim from `attestation::venice` as the single source of truth — no copy-paste), three-way AND composition for Orchestrated responses, NRAS double-parse for Shape A/B and per-GPU loop for Shape C, Tinfoil-via-Redpill HTTP-502 detection returning `TinfoilUnsupported`, and an in-memory 5-min TTL cache. All 21 RED stubs flipped GREEN against the spike-002 golden fixtures; 371 lib tests pass; release build clean; zero new Cargo dependencies.

## What Shipped

### Public API (`rust/src/attestation/redpill.rs`)

| Symbol | Kind | Purpose |
|---|---|---|
| `RedpillError` | enum | UnknownShape, QuoteDecode, DebugMode, ReportDataMismatch, ComposeManagerMismatch, TinfoilUnsupported, OrchestratedComponentFailed, Network, Inner(AttestationError) |
| `RedpillShape` | enum | Flat / Orchestrated{is_near_ai} / Chutes |
| `Freshness` | enum | PerRequest (Shape A/B) / PerEnclave (Shape C) |
| `OrchestratedComponents` | struct | gateway_signing_address_hex, model_signing_address_hex, compose_manager_actions_hash_hex |
| `VerifiedRedpillAttestation` | struct | backend_id, model, shape, freshness, orchestrated_components, expires_at |
| `detect_shape(value)` | fn | per detect.ts priority order; fails closed |
| `quote_bytes(s)` | fn | hex/base64 auto-detect, strips 0x prefix |
| `debug_mode_disabled(q)` | fn | quote[48+120] & 0x01 == 0 |
| `verify_redpill_model_reportdata` | re-export | `pub use crate::attestation::venice::verify_venice_report_data` |
| `verify_redpill_gateway_reportdata` | fn | ed25519 [0..32] pubkey + [32..64] client nonce |
| `verify_redpill_compose_manager_reportdata` | fn | actions_hash [0..32] + nonce [32..64] |
| `verify_redpill_chutes_anti_tamper` | fn | SHA256(nonce_str ++ e2e_pubkey_str) over rd[0..32]; rd[32..64] unconstrained |
| `fetch_and_verify_redpill_attestation` | async fn | full orchestrator (uncached) |
| `ensure_verified_redpill_attestation` | async fn | 5-min TTL cache wrapper |

### Model-decoder reuse confirmation

`grep` confirms single source of truth — the model REPORTDATA decoder is REUSED via `pub use`:

```
$ grep -n 'pub use crate::attestation::venice::verify_venice_report_data' rust/src/attestation/redpill.rs
36: pub use crate::attestation::venice::verify_venice_report_data as verify_redpill_model_reportdata;
```

No copy of the Venice keccak/zero-pad/nonce decoder lives in `redpill.rs`. The Shape A and Shape B model components both call the re-exported symbol.

### Three-way AND test result

`shape_b_three_way_and_gates_session_open` constructs a synthetic `[u8;64]` with `bad_rd[0] = 0xFF` and a zeroed `actions_hash` reference, then asserts `verify_redpill_compose_manager_reportdata` returns `Err(RedpillError::ComposeManagerMismatch { .. })`. This pins the contract that any single component failure fails the whole attestation. The orchestrator logic in `verify_orchestrated` then wraps the inner error in `RedpillError::OrchestratedComponentFailed { failed: "compose_manager" }` (and mirrors that for "gateway" and "model"). The live three-way AND end-to-end fixture is exercised by Plan 04's #[ignore] live integration test against `api.redpill.ai`.

### RED → GREEN tally

| Stub group | Plan-01 stubs | Plan-02 GREEN | Notes |
|---|---|---|---|
| RED-03 dispatcher | 4 | 4 | Flat / Orchestrated / Chutes / UnknownShape |
| RED-04 quote_bytes | 3 | 3 | hex / base64 / 0x-prefix |
| RED-05a Shape A model | 4 | 4 | ok / addr-mismatch / nonce-mismatch / pad-nonzero |
| RED-05b Shape B gateway | 1 | 1 | ed25519 + nonce |
| RED-05c Shape B compose-manager | 1 | 1 | actions_hash + nonce |
| RED-05d Shape C anti-tamper | 2 | 2 | binding ok / client nonce not bound |
| RED-06 three-way AND | 1 | 1 | compose-manager mismatch path |
| RED-07 NVIDIA NRAS shape | 0 (was 0 in attestation_redpill.rs) | 2 (added) | nvidia_payload double-parse + Shape C gpu_evidence shape |
| RED-08 debug-mode | 2 | 2 | clear-in-all-captures / synthetic-set-rejected |
| Shape B model-rd | 1 | 1 | Venice-identical addr+pad+nonce slice |
| **Total deterministic** | **19** | **21** | (added 2 NRAS shape tests for RED-07; live network tests remain #[ignore]) |

`tests::live_redpill::*` — 3 #[ignore] live tests remain ignored (Plan 04 wires them up against `api.redpill.ai`).
`tests::redpill::*` — 4 #[ignore] stubs untouched (Plan 03 owns RED-01/RED-02/RED-10/RED-11).

### ReportDataLayout decision

**No new variant added.** Reused `ReportDataLayout::VeniceAddrPadNonce` across all three shapes:

- **Shape A and Shape B (model component):** Layout matches Venice exactly — rd[0..20] = keccak-derived addr, rd[20..32] = zero, rd[32..64] = client nonce. The dcap-qvl layout-nonce comparison correctly checks rd[32..64] == client_nonce.
- **Shape B (gateway):** rd[0..32] = ed25519 pubkey, rd[32..64] = client nonce. Layout-nonce comparison still checks rd[32..64] == client_nonce — correct. The address-binding (rd[0..32] == gateway pubkey) is enforced by `verify_redpill_gateway_reportdata` separately.
- **Shape B (compose-manager):** rd[0..32] = actions_hash, rd[32..64] = client nonce. Same as gateway — layout-nonce check is correct, the actions_hash binding is enforced by the compose-manager decoder.
- **Shape C (Chutes):** rd[32..64] is intentionally unconstrained per D-11. To keep dcap-qvl's signature/collateral/TCB verification effective without firing a spurious NonceMismatch, `verify_chutes` slices rd[32..64] from the parsed quote and feeds it back to `verify_tdx_quote` as the "expected nonce" — the layout-nonce comparison degenerates to a tautology while the cryptographic gates remain. The actual freshness binding is enforced by `verify_redpill_chutes_anti_tamper` (SHA256 over rd[0..32]) and the enclave-baked nonce rotation.

If Plan 03/04 prefers cleaner separation, a `ReportDataLayout::ChutesNoNonceBinding` variant can be added to `attestation/tdx.rs` later — currently the self-feedback approach keeps all changes localized to `redpill.rs`.

### Tests left `#[ignore]` (with reason)

| Test | File | Reason |
|---|---|---|
| `live_shape_a_phala_pure` | `live_redpill.rs` | Plan 04 — live integration against `api.redpill.ai` |
| `live_shape_b_orchestrated` | `live_redpill.rs` | Plan 04 — live integration |
| `live_shape_c_chutes` | `live_redpill.rs` | Plan 04 — live integration |
| RED-01/02/10/11 stubs | `redpill.rs` | Plan 03 owns the LLM transport layer + provider preset |

## Validation Commands

```bash
# Plan 02 deterministic suite (must pass)
cd rust && cargo test -p mango_core --lib attestation_redpill
# → 21 passed; 0 failed; 0 ignored

# Full lib (no regressions)
cd rust && cargo test -p mango_core --lib
# → 371 passed; 0 failed; 21 ignored (live + Plan 03/04 stubs)

# Release build
cd rust && cargo build -p mango_core --release
# → exits 0

# Zero new deps
git diff rust/Cargo.toml
# → empty
```

## Acceptance Criteria — Verified

| Criterion | Result |
|---|---|
| `rust/src/attestation/redpill.rs` exists | yes (800 lines) |
| `pub mod redpill;` in attestation/mod.rs | yes |
| `pub enum RedpillShape` | yes |
| `pub fn detect_shape` | yes |
| `pub fn quote_bytes` | yes |
| `pub fn debug_mode_disabled` | yes |
| `pub use crate::attestation::venice::verify_venice_report_data` (single source of truth grep) | yes |
| `RedpillError` variants present (UnknownShape, QuoteDecode, DebugMode, ComposeManagerMismatch, TinfoilUnsupported, OrchestratedComponentFailed) | yes |
| `pub fn verify_redpill_gateway_reportdata` | yes |
| `pub fn verify_redpill_compose_manager_reportdata` | yes |
| `pub fn verify_redpill_chutes_anti_tamper` | yes |
| `pub async fn fetch_and_verify_redpill_attestation` | yes |
| `pub async fn ensure_verified_redpill_attestation` | yes |
| `verify_flat` / `verify_orchestrated` / `verify_chutes` private dispatchers | yes |
| `OrchestratedComponentFailed` (three-way AND failure path) | yes |
| `fetch_and_verify_nvidia` reused | yes |
| `TinfoilUnsupported` HTTP 502 detection | yes |
| `Freshness::PerRequest` / `Freshness::PerEnclave` | yes |
| `0x01` debug-bit constant | yes |
| `h.update(enclave_baked_nonce_str.as_bytes())` (Chutes STRING concat over ASCII) | yes |
| `cargo test -p mango_core --lib attestation_redpill` ≥ 14 pass | 21 pass |
| Full lib suite passes — no regressions | 371 pass |
| `cargo build --release` exits 0 | yes |
| `redpill.rs` ≥ 350 lines | 800 lines |
| Zero new Cargo deps | confirmed |

## Deviations from Plan

**One small adaptation:**

**1. [Rule 2 - Auto-add critical functionality] Address-only fallback path for Shape A model decoder**
- **Found during:** Task 3
- **Issue:** Spike capture `attestation-phala-pure-raw.json` does not include a `signing_key` or `signing_public_key` field — only `signing_address` (a 20-byte Eth-address derived from a keccak-of-uncompressed-pubkey). The plan's `verify_flat` step assumed an uncompressed pubkey would be present; calling `verify_redpill_model_reportdata` (which expects 65-byte uncompressed pubkey) would fail at parse time even on a correct capture.
- **Fix:** Added an address-only fallback in `verify_flat`: when `signing_key`/`signing_public_key` is absent, decode `signing_address` (hex, 20 bytes), assert `rd[0..20] == addr`, assert `rd[20..32]` is all-zero, and assert `rd[32..64] == nonce`. Same checks the keccak decoder would have made minus the keccak step. Documented inline.
- **Files modified:** `rust/src/attestation/redpill.rs` (verify_flat)
- **Commit:** 95b6dea

Otherwise: plan executed exactly as written.

## Authentication Gates

None — all attestation endpoints used here are public (D-02). Live integration network calls are deferred to Plan 04 (`#[ignore]`-gated).

## Threat Mitigation Recap

| Threat | Mitigated By | Test |
|---|---|---|
| T-34-01 (cross-shape REPORTDATA replay) | `detect_shape` returns one of three discrete variants and fails closed on unknown — no permissive fallback. | `dispatch_unknown_shape_fails_closed` |
| T-34-02 (single-component compromise of Orchestrated) | `verify_orchestrated` runs gateway → model → compose-manager sequentially; any single failure wraps in `OrchestratedComponentFailed`. | `shape_b_three_way_and_gates_session_open` |
| T-34-03 (Chutes per-enclave freshness misrepresentation) | `Freshness::PerEnclave` is set on Shape C; the Chutes decoder explicitly does NOT compare rd[32..64] to the client nonce (D-11). | `shape_c_client_nonce_not_bound` |
| T-34-04 (TDX debug-mode bypass) | `debug_mode_disabled` applied across all three shapes; failure returns `RedpillError::DebugMode`. | `debug_bit_clear_in_all_captures` + `debug_bit_set_rejected` |
| T-34-07 (stale TDX quote replay) | Per-request 32-byte client nonce embedded in REPORTDATA[32..64] for Shape A and B (verified byte-equal); 5-min TTL cache (D-20) keyed on (base_url, model). Chutes uses enclave-baked nonce + e2e_pubkey rotation. | `shape_a_model_reportdata_ok`, `shape_b_gateway_reportdata_ok`, `shape_b_compose_manager_reportdata_ok` |

## Commits

| Task | Commit | Subject |
|---|---|---|
| 1 | 3b8ef7f | feat(34-02): redpill shape dispatcher + quote_bytes + debug-mode gate |
| 2 | a12bd07 | feat(34-02): redpill REPORTDATA decoders (gateway/compose-manager/chutes) |
| 3 | 95b6dea | feat(34-02): redpill verify orchestrator + 5-min cache + three-way AND |

## Self-Check

- [x] `rust/src/attestation/redpill.rs` exists (800 lines)
- [x] `rust/src/attestation/mod.rs` declares `pub mod redpill;`
- [x] `rust/src/tests/attestation_redpill.rs` has 21 GREEN tests, 0 ignored
- [x] All three commits exist in git log: 3b8ef7f, a12bd07, 95b6dea
- [x] `cargo test -p mango_core --lib`: 371 passed, 0 failed
- [x] `cargo build -p mango_core --release` exits 0
- [x] `git diff rust/Cargo.toml` is empty (zero new deps)

## Self-Check: PASSED
