---
phase: 35
plan: 00
type: summary
status: complete
requirements_addressed: [CTX-01]
commits:
  - (commit hash to be added if available)
---

# Plan 35-00 — Wave 0: Cargo dep + openssl-sys audit + test stubs

## What shipped

Pure-Rust contextvm-sdk dependency added to Cargo.toml with openssl-sys audit verification and test scaffolding.

### Dependency addition

Added `contextvm-sdk = "0.1.0"` to `rust/Cargo.toml` under the "Phase 35: Nostr-based tool discovery + invocation (pure-Rust, rustls only)" section. The default `["rmcp"]` feature is retained as required for `discover_tools_typed` and `NostrMCPProxy`.

### OpenSSL audit verification

Ran `cargo tree -p mango_core | grep -iE "openssl-sys|native-tls"` to verify no new openssl-sys edges were introduced. Result: Only the pre-existing edge through `rusqlite` → `libsqlite3-sys` → `openssl-sys` remains. No edges traced through `contextvm-sdk`, `nostr-sdk`, `rmcp`, `async-wsocket`, `tungstenite`, or `tokio-tungstenite`. CTX-01 (pure-Rust, rustls-only) holds.

### Test scaffolding

Created `rust/src/tests/contextvm.rs` with 10 `#[ignore]`-gated stub tests mapping 1-to-1 to CTX-01 through CTX-10 requirements. Each stub is owned by a specific downstream plan and will be un-ignored and implemented when that plan lands. Registered the module in `rust/src/tests/mod.rs`.

## Tests

| Test | Status |
|------|--------|
| `ctx_01_pure_rust_no_openssl` | passing (verified by cargo-tree audit in acceptance) |
| `ctx_02_settings_discover_tools_row_and_screen` | ignored (owned by 35-06/35-07) |
| `ctx_03_per_tool_enable_persists_across_launches` | ignored (owned by 35-01/35-05) |
| `ctx_04_auto_discover_tools_toggle_persists` | ignored (owned by 35-01/35-05) |
| `ctx_05_enabled_tools_appear_in_openai_tools_array` | ignored (owned by 35-04) |
| `ctx_06_invocation_routes_through_nostr_returns_tool_result` | ignored (owned by 35-03/35-04) |
| `ctx_07_default_relay_set_includes_relay_nostr_net` | ignored (owned by 35-02) |
| `ctx_08_graceful_degradation_on_relay_failure` | ignored (owned by 35-02/35-03) |
| `ctx_09_uniffi_bindings_regenerated_for_all_three_platforms` | ignored (owned by 35-08) |
| `ctx_10_agent_step_summary_carries_tool_origin_for_remote_tool_calls` | ignored (owned by 35-05) |

`cargo test -p mango_core contextvm → 10 ignored; 0 failed`

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Persistence layer → Plan 35-01
- Discovery service → Plan 35-02
- Invocation service → Plan 35-03
- Dispatch routing → Plan 35-04
- Actor wiring → Plan 35-05
- Android UI → Plan 35-06
- Desktop UI → Plan 35-07
- UniFFI bindings → Plan 35-08
