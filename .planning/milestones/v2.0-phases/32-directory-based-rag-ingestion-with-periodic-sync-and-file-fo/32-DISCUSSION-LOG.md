# Phase 32: Directory-based RAG Ingestion — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in 32-CONTEXT.md. This log records that discuss-phase ran in `--auto` mode.

**Date:** 2026-04-19
**Phase:** 32-directory-based-rag-ingestion-with-periodic-sync-and-file-folder-exclusion
**Mode:** `--auto` (no interactive questioning; all decisions taken from recommended options in 32-RESEARCH.md)
**Areas covered:** Architecture/RMP boundary, Persistence schema, Change detection, Desktop walking/watching, iOS access model, Android access model, Scheduling, Throughput/batching, Exclusions, UI scope, Security

---

## Auto-selection rationale

In `--auto` mode each gray area is resolved by selecting the approach explicitly recommended in `32-RESEARCH.md`. Concretely:

| Gray area | Options considered (from research) | Auto-selected |
|-----------|------------------------------------|---------------|
| Change detection fingerprint | mtime+size / SHA-256 content hash / Merkle tree | **mtime+size** (recommended; only viable for 10k-file vaults) |
| Desktop walk | manual `std::fs::read_dir` / `walkdir` / `ignore::WalkBuilder` | **`ignore::WalkBuilder` + `OverrideBuilder`** (recommended) |
| Desktop real-time watching | `notify` + debounce / polling only / none | **`notify` 8.x + `notify-debouncer-mini` 2s debounce + 5-min poll fallback** (recommended) |
| inotify limit handling | fail / log-only / fall back to `PollWatcher` | **Fall back to `PollWatcher` 60s + UI warning** (recommended) |
| iOS bookmark option | `.withSecurityScope` / `.minimalBookmark` / none | **`.minimalBookmark`** (recommended; `.withSecurityScope` is macOS-only) |
| iOS scheduling | BGTaskScheduler / foreground-resume / timer-only | **Foreground-resume scan** (recommended; industry pattern) |
| iOS iCloud placeholders | block + download / skip + surface / read anyway | **Skip `.notDownloaded` + surface in UI** (recommended) |
| Android traversal | `DocumentFile.listFiles()` / `DocumentsContract.query` | **`DocumentsContract.query` bulk** (recommended; 10-100× faster) |
| Android scheduling | AlarmManager / JobScheduler / WorkManager | **WorkManager 15-min + onResume** (recommended) |
| Sync batching | single giant action / file-by-file / 50-file batches | **50-file batches with flush after each** (recommended) |
| Exclusion syntax | regex / custom DSL / gitignore-style globs | **gitignore-style globs via `OverrideBuilder`** (recommended) |
| Persistence scope | in-memory / SQLite / separate file | **SQLite (`directory_sources` + `directory_files` V18 migration)** (recommended) |
| Native-vs-Rust enumeration split | full listing to Rust / native diff + changed-only | **Native diff, send changed-only** (recommended; avoids 10k-file UniFFI payloads) |
| Removal semantics | soft-delete / hard-delete with cascade | **Hard-delete with `ON DELETE CASCADE` and index key removal** |

## Claude's Discretion
- Batch size tuning (50-file starting point).
- Native-side glob matcher implementation detail (bespoke vs. FFI into `globset`).
- Default exclusion preset contents.
- UI copy and empty-state wording.

## Deferred Ideas
Captured in `32-CONTEXT.md` `<deferred>` section: bi-directional sync, cloud-native connectors, iCloud auto-download, deep-link to source app, per-result exclude, file-type allowlist, content-hash fallback for unreliable mtime providers.
