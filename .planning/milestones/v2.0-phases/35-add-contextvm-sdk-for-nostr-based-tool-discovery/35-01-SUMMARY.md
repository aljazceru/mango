---
phase: 35
plan: 01
type: summary
status: complete
requirements_addressed: [CTX-03, CTX-04]
commits:
  - (commit hash to be added if available)
---

# Plan 35-01 — Wave 1: MIGRATION_V20 (contextvm_tools + agent_steps.tool_origin) + 6 CRUD queries

## What shipped

SQLite migration V20 and six CRUD query helpers for contextvm tool persistence, covering per-tool enable persistence and auto-discover toggle persistence.

### Migration V20

Added `MIGRATION_V20` to `rust/src/persistence/schema.rs`:
- Creates `contextvm_tools` table with columns: id (TEXT PRIMARY KEY), tool_name (TEXT UNIQUE), display_name (TEXT), description (TEXT), provider_pubkey (TEXT), provider_display_name (TEXT), schema_json (TEXT), enabled (INTEGER), last_seen_at (INTEGER)
- Creates unique index `idx_contextvm_tools_name` on tool_name
- Creates index `idx_contextvm_tools_enabled` on enabled
- Alters `agent_steps` table to add `tool_origin` (TEXT) column

### Query helpers

Added six query functions to `rust/src/persistence/queries.rs`:
- `upsert_contextvm_tool`: INSERT OR REPLACE into contextvm_tools
- `update_contextvm_tool_enabled`: UPDATE enabled flag by id
- `get_contextvm_tool_by_name`: SELECT single row by tool_name
- `list_enabled_contextvm_tools`: SELECT enabled=1 rows ordered by last_seen_at DESC
- `list_all_contextvm_tools`: SELECT all rows ordered by last_seen_at DESC
- `delete_contextvm_tool`: DELETE by id

Added `ContextvmToolRow` struct matching the table schema.

### Tests

Added four persistence tests in `rust/src/tests/persistence.rs`:
- `test_round_trip_enabled_contextvm_tool`: verifies upsert and retrieval
- `test_list_enabled_skips_disabled_rows_and_orders_by_last_seen_desc`: verifies filtering and ordering
- `test_update_contextvm_tool_enabled_persists_after_reopen`: verifies persistence across DB reopens
- `test_auto_discover_tools_setting_round_trips`: verifies settings key persistence

Un-ignored `ctx_03_per_tool_enable_persists_across_launches` and `ctx_04_auto_discover_tools_toggle_persists` in `rust/src/tests/contextvm.rs`.

## Tests

| Test | Status |
|------|--------|
| `test_round_trip_enabled_contextvm_tool` | passing |
| `test_list_enabled_skips_disabled_rows_and_orders_by_last_seen_desc` | passing |
| `test_update_contextvm_tool_enabled_persists_after_reopen` | passing |
| `test_auto_discover_tools_setting_round_trips` | passing |
| `ctx_03_per_tool_enable_persists_across_launches` | passing (no longer ignored) |
| `ctx_04_auto_discover_tools_toggle_persists` | passing (no longer ignored) |

`cargo test -p mango_core` — full suite green.

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Discovery service → Plan 35-02
- Invocation service → Plan 35-03
- Dispatch routing → Plan 35-04
- Actor wiring → Plan 35-05
