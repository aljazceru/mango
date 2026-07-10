---
phase: 35
plan: 02
type: summary
status: complete
requirements_addressed: [CTX-01, CTX-07, CTX-08]
commits:
  - b01b699  # feat(35-02): add contextvm discovery module + DEFAULT_CONTEXTVM_RELAYS
  - e8cf335  # feat(35-02): activate ctx_07 + ctx_08 stubs; add live discovery test
---

# Plan 35-02 — Wave 1: contextvm discovery service

## What shipped

Pure-Rust read-only Nostr discovery module wrapping `contextvm-sdk` 0.1.0.

### Module structure

```
rust/src/contextvm/
├── mod.rs           # DEFAULT_CONTEXTVM_RELAYS + re-exports
├── discovery.rs     # discover_servers / discover_tools_for_server / discover_all
└── error.rs         # ContextvmError enum + Display + From<anyhow::Error>
```

`mod contextvm` registered in `rust/src/lib.rs` between `attestation` and `crypto`.

### Public API

| Item | Signature | Notes |
|------|-----------|-------|
| `DEFAULT_CONTEXTVM_RELAYS` | `&[&str]` | `["wss://relay.damus.io", "wss://nos.lol", "wss://relay.nostr.net"]` (CTX-07) |
| `default_relays_owned()` | `fn() -> Vec<String>` | Materialises const into the `Vec<String>` shape contextvm-sdk APIs accept |
| `discover_servers` | `async fn(&[String]) -> Result<Vec<DiscoveredServer>, ContextvmError>` | One-shot kind 11316 fetch |
| `discover_tools_for_server` | `async fn(&str, Option<&str>, &[String]) -> Result<Vec<DiscoveredTool>, ContextvmError>` | One-shot kind 11317 fetch via `discover_tools_typed` |
| `discover_all` | `async fn(&[String]) -> Result<Vec<DiscoveredTool>, ContextvmError>` | Convenience sweep — per-server failures downgraded to `log::warn!` |
| `DiscoveredServer` | struct | `{ pubkey_hex, display_name }` |
| `DiscoveredTool` | struct | `{ provider_pubkey_hex, provider_display_name, tool_name, description, schema_json }` |
| `ContextvmError` | enum | Variants: `RelayUnreachable`, `MalformedAnnouncement`, `Timeout`, `JsonRpc`, `Other` |

## Tests

| Test | Status |
|------|--------|
| `ctx_07_default_relay_set_includes_relay_nostr_net` | passing (un-ignored from stub) |
| `ctx_08_graceful_degradation_on_relay_failure` | passing (un-ignored — pins Display contract) |
| `live_discover_servers_against_default_relays` | `#[ignore = "live network"]` — un-ignored by Plan 35-09 |

`cargo test -p mango_core --lib contextvm:: → 4 passed; 0 failed; 7 ignored`

## Build sweep

`cargo build -p mango_core --lib` — green.

`cargo tree -p mango_core | grep openssl-sys` shows ONLY the pre-existing
`rusqlite → libsqlite3-sys → openssl-sys` edge — no new openssl-sys edge
introduced via contextvm-sdk. CTX-01 (pure-Rust, rustls-only) holds.

## Deviations from RESEARCH §A

None functionally. Two minor adjustments:

1. **`nostr_sdk::PublicKey` import**: the plan suggested `use nostr_sdk::PublicKey`,
   but the crate is not a direct dependency. Used the already-public re-export
   `contextvm_sdk::signer::PublicKey` instead. Same type.
2. **`tracing` → `log`**: the plan's reference body used `tracing::warn!`, but
   the crate uses `log` (already in Cargo.toml, no `tracing` dep). Substituted
   `log::warn!`. Behaviour identical for `warn!`.

## Out of scope (handed off)

- Persistence of discovered tools → Plan 35-03.
- Wiring `discover_all` into `AppActions` and `ContextvmDiscoveryState` → Plan 35-05.
- `dispatch_tools` invocation routing → Plan 35-04.
- UniFFI exposure of `DiscoveredTool` to native UIs → Plan 35-08.
