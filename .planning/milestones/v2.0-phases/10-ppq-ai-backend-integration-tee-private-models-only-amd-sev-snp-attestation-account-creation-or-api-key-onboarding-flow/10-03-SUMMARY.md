---
phase: 10
plan: 03
type: summary
status: complete
commits:
  - (implementation completed 2026-03-26, pre-GSD workflow)
---

# Plan 10-03 — Database Migrations for PPQ.AI

## What shipped

SQLite migrations V10 and V11 to seed PPQ.AI backend with correct configuration and update to private transport base URL.

### MIGRATION_V10

Added MIGRATION_V10 to `rust/src/persistence/schema.rs`:
- Inserts PPQ.AI backend row into backends table
- id: "ppq-ai"
- name: "PPQ.AI"
- base_url: "https://api.ppq.ai/v1/" (initial public URL)
- tee_type: "AmdSevSnp"
- model_list: NULL (models fetched post-auth)
- display_order: 10
- is_active: 0 (inactive until user enables)
- Comment documents PPQ.AI exposes five AMD SEV-SNP protected private models

### MIGRATION_V11

Added MIGRATION_V11 to `rust/src/persistence/schema.rs`:
- UPDATE statement to switch PPQ.AI to private transport base URL
- Sets base_url to "https://api.ppq.ai/private/v1/"
- WHERE clause: id = "ppq-ai" AND base_url = "https://api.ppq.ai/v1/"
- Preserves any user-customized base_url (only updates if still at default)
- Comment documents switch to PPQ private mode

### Migration registration

Both migrations registered in migrations array in `rust/src/persistence/schema.rs`:
- MIGRATION_V10 added after existing migrations
- MIGRATION_V11 added after MIGRATION_V10
- Applied in order by migration runner

## Tests

Added test in `rust/src/tests/persistence.rs`:
- `test_migration_v11_seeds_ppq_ai_private_transport`: Verifies V11 updates base_url correctly

## Build sweep

`cargo build -p mango_core --lib` — green.

## Deviations from plan

None.

## Out of scope (handed off)

- Attestation routing → Plan 10-04
- UI integration → Plan 10-05
- Additional tests → Plan 10-06
