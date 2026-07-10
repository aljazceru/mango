---
phase: 35
plan: 05
type: summary
status: complete
requirements_addressed: [CTX-02, CTX-06, CTX-08, CTX-10]
commits:
  - (commit hash to be added if available)
---

# Plan 35-05 — Wave 2: AppActions (4) + handlers + AppState fields + ActorState hydration + auto-discover hook + AgentStepSummary.tool_origin + UniFFI types

## What shipped

Rust core API for Phase 35 with 4 AppActions, AppState fields, ActorState hydration, auto-discover hook, tool_origin provenance tracking, and UniFFI type exports.

### AppActions

Added 4 AppAction variants to `rust/src/lib.rs`:
- `DiscoverContextvmTools`: triggers background discovery
- `SetContextvmToolEnabled { tool_id, enabled }`: toggles tool enablement
- `SetAutoDiscoverTools { enabled }`: toggles auto-discover setting
- `RetryContextvmDiscovery`: retries failed discovery

### AppState extensions

Added fields to `AppState`:
- `contextvm_tools: Vec<DiscoverableTool>`: cached tool list
- `auto_discover_tools_enabled: bool`: auto-discover toggle state

Added `DiscoverableTool` UniFFI record:
- id, name, description, provider_pubkey, provider_display_name, enabled fields

### ActorState extensions

Added field to `ActorState`:
- `current_conv_contextvm_tools: HashMap<String, ContextvmToolDescriptor>`: per-conversation tool map

### InternalEvent

Added `InternalEvent::ContextvmDiscoveryComplete(Result<Vec<DiscoveredTool>, String>)` for async discovery result handling.

### Handler wiring

Added AppAction handlers in `rust/src/lib.rs`:
- `DiscoverContextvmTools`: spawns background discovery task
- `SetContextvmToolEnabled`: updates DB and refreshes AppState
- `SetAutoDiscoverTools`: updates settings key
- `RetryContextvmDiscovery`: re-triggers discovery
- `InternalEvent::ContextvmDiscoveryComplete`: updates AppState with results

### Actor integration

Added auto-discover hook in `do_send_message`:
- When `current_conv_contextvm_tools` is empty and `auto_discover_tools_enabled` is true, triggers discovery before first tool round

Added tool_origin tracking in agent loop:
- `tool_origin: Some("contextvm")` for remote tools, `Some("local")` for local tools
- Propagated to `AgentStepSummary` for provenance display

Added `load_enabled_descriptors` and `row_to_discoverable_tool` helper functions.

### UniFFI exports

Added UniFFI exports in `rust/src/lib.rs`:
- `DiscoverableTool` record
- `ContextvmDiscoveryState` enum (Loading, Loaded, Error)
- AppAction variants for contextvm

### Hydration

Added hydration on unlock:
- `auto_discover_tools_enabled` loaded from settings
- `contextvm_tools` loaded from DB via `list_all_contextvm_tools`

### Tests

Un-ignored CTX stubs in `rust/src/tests/contextvm.rs`:
- `ctx_02_settings_discover_tools_row_and_screen`: verifies Screen::ToolDiscovery exists
- `ctx_06_invocation_routes_through_nostr_returns_tool_result`: verifies dispatch_map routing
- `ctx_08_graceful_degradation_on_relay_failure`: verifies Error variant
- `ctx_10_agent_step_summary_carries_tool_origin_for_remote_tool_calls`: verifies tool_origin field

Removed all `// TODO(Phase 35-05)` placeholders from dispatch_tools call sites.

## Tests

| Test | Status |
|------|--------|
| `ctx_02_settings_discover_tools_row_and_screen` | passing (no longer ignored) |
| `ctx_06_invocation_routes_through_nostr_returns_tool_result` | passing (no longer ignored) |
| `ctx_08_graceful_degradation_on_relay_failure` | passing (no longer ignored) |
| `ctx_10_agent_step_summary_carries_tool_origin_for_remote_tool_calls` | passing (no longer ignored) |

`cargo test -p mango_core` — full suite green.

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Android UI → Plan 35-06
- Desktop UI → Plan 35-07
- UniFFI binding regeneration → Plan 35-08
