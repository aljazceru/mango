---
quick_id: 260421-fij
date: 2026-04-21
---

# Summary — 260421-fij

Added `#[allow(dead_code)]` to `DirectoryFileRow.id` and `DirectoryFileRow.source_id` in `rust/src/persistence/queries.rs`. Fields remain populated by `list_directory_files_by_source` for future callers; the attributes silence `dead_code` without removing schema-mirroring fields.

Verified with `cargo build -p mango_core --release` — clean build, warning gone.
