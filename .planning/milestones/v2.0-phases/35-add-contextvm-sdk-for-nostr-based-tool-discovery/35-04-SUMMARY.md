---
phase: 35
plan: 04
type: summary
status: complete
requirements_addressed: [CTX-05]
commits:
  - (commit hash to be added if available)
---

# Plan 35-04 — Wave 2: Dispatch routing (ContextvmToolDescriptor, RESERVED_LOCAL_NAMES, 8-tool cap, 500-char description cap, build_chat_tools_with_contextvm, dispatch_tools fallback arm)

## What shipped

Dispatch routing extension for contextvm tools with local-tool collision filtering, 8-tool cap, 500-char description cap, and integration into existing dispatch_tools function.

### Descriptor types

Added `ContextvmToolDescriptor` struct in `rust/src/contextvm/dispatch.rs`:
- tool_name, description, schema (serde_json::Value)
- provider_pubkey_hex, provider_display_name
- last_seen_at
- `from_row`: conversion from ContextvmToolRow
- `DESCRIPTION_CAP_CHARS: usize = 500` constant
- `RESERVED_LOCAL_NAMES: &[&str]` with local tool names
- `MAX_REMOTE_TOOLS_PER_TURN: usize = 8` constant

Added helper functions:
- `finalise_for_turn`: filters reserved names, caps at 8, sorts by last_seen_at DESC, caps descriptions at 500 chars
- `descriptors_to_chat_tools`: converts descriptors to OpenAI ChatCompletionTool format

### Dispatch integration

Extended `dispatch_tools` in `rust/src/agent/tools.rs`:
- Added `contextvm_map: &HashMap<String, ContextvmToolDescriptor>` parameter
- Added `contextvm_secret_key: &str` parameter
- Added fallback match arm for remote tools: calls `crate::contextvm::invoke_tool`
- Added `// TODO(Phase 35-05)` placeholders at all call sites (to be consumed by 35-05)

Added `build_chat_tools_with_contextvm` in `rust/src/agent/tools.rs`:
- Extension of existing `build_chat_tools` that appends remote descriptors
- Backward-compatible: accepts empty slice for callers without contextvm

### Tests

Added unit tests in `rust/src/tests/contextvm.rs`:
- `test_build_chat_tools_with_contextvm_appends_remote`: verifies remote tools appended
- `test_build_chat_tools_with_contextvm_caps_at_8_via_finalise`: verifies 8-tool cap
- `test_finalise_for_turn_filters_reserved_local_names`: verifies collision filtering
- `test_finalise_for_turn_caps_at_8_sorted_desc_by_last_seen`: verifies sorting and capping
- `test_finalise_for_turn_caps_description_at_500`: verifies description truncation

Un-ignored `ctx_05_enabled_tools_appear_in_openai_tools_array` in `rust/src/tests/contextvm.rs`.

## Tests

| Test | Status |
|------|--------|
| `test_build_chat_tools_with_contextvm_appends_remote` | passing |
| `test_build_chat_tools_with_contextvm_caps_at_8_via_finalise` | passing |
| `test_finalise_for_turn_filters_reserved_local_names` | passing |
| `test_finalise_for_turn_caps_at_8_sorted_desc_by_last_seen` | passing |
| `test_finalise_for_turn_caps_description_at_500` | passing |
| `ctx_05_enabled_tools_appear_in_openai_tools_array` | passing (no longer ignored) |

`cargo test -p mango_core` — full suite green.

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Actor wiring (populate contextvm_map and secret_key) → Plan 35-05
