---
quick_id: 260421-fij
type: execute
status: complete
created: 2026-04-21
---

# Quick 260421-fij — Silence dead_code warnings on DirectoryFileRow

**Request:** Fix `dead_code` build warnings on `DirectoryFileRow.id` and `DirectoryFileRow.source_id` in `rust/src/persistence/queries.rs`.

## Scope

Single file, single fix. Fields are populated by `list_directory_files_by_source` but no current caller reads them. Keep the fields — they mirror the DB schema and will be wanted by future callers (deletion-by-id, source joins). Suppress the warning with targeted `#[allow(dead_code)]` attributes.

## Task

<task type="auto">
  <name>Add #[allow(dead_code)] on the two unused fields</name>
  <files>rust/src/persistence/queries.rs</files>
  <action>Annotate `id: i64` and `source_id: String` on `DirectoryFileRow` with `#[allow(dead_code)]`.</action>
  <verify><automated>cargo build -p mango_core --release</automated></verify>
  <done>Release build completes without the two dead_code warnings.</done>
</task>
