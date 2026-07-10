---
phase: 35
plan: 03
type: summary
status: complete
requirements_addressed: [CTX-06, CTX-08]
commits:
  - (commit hash to be added if available)
---

# Plan 35-03 — Wave 1: Invocation service via NostrMCPProxy + persistent Nostr key + truncation/timeout/error formatting

## What shipped

Async invocation primitive for calling contextvm tools via Nostr MCP protocol with persistent secret key, 15s timeout, 16 KiB result truncation, and comprehensive error formatting.

### Invocation module

Created `rust/src/contextvm/invocation.rs` with:
- `invoke_tool`: async function taking secret_key_hex, provider_pubkey, tool_name, args_str; returns String result
- `NostrMCPProxy` wrapper around `contextvm_sdk::nostr_mcp::NostrMCPProxy` with EncryptionMode::Optional
- `load_or_create_secret_key`: persists hex-encoded Nostr secret key in settings table, reuses on subsequent calls
- `INVOCATION_TIMEOUT_SECS: u64 = 15` constant
- `MAX_TOOL_RESULT_BYTES: usize = 16_384` constant
- `truncate_result`: caps result at 16 KiB with "... [truncated]" marker
- `format_timeout`: returns "Error: tool '{name}' timed out (15s)"
- `format_jsonrpc_error`: returns "Error: {code}: {message}"

Registered module in `rust/src/contextvm/mod.rs`.

### Tests

Added unit tests in `rust/src/tests/contextvm.rs`:
- `test_truncate_result_under_limit_unchanged`: verifies no truncation under limit
- `test_truncate_result_over_limit_appends_marker`: verifies truncation with marker
- `test_format_timeout_locked_copy`: verifies timeout error format
- `test_format_jsonrpc_error_locked_copy`: verifies JSON-RPC error format
- `test_load_or_create_secret_key_creates_then_returns_same`: verifies key persistence

The `ctx_06_invocation_routes_through_nostr_returns_tool_result` stub remains ignored (owned by 35-04 for dispatch wiring). The `ctx_08_graceful_degradation_on_relay_failure` stub remains ignored (owned by 35-02 for discovery wiring).

## Tests

| Test | Status |
|------|--------|
| `test_truncate_result_under_limit_unchanged` | passing |
| `test_truncate_result_over_limit_appends_marker` | passing |
| `test_format_timeout_locked_copy` | passing |
| `test_format_jsonrpc_error_locked_copy` | passing |
| `test_load_or_create_secret_key_creates_then_returns_same` | passing |

`cargo test -p mango_core` — full suite green.

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Dispatch routing → Plan 35-04
- Actor wiring → Plan 35-05
