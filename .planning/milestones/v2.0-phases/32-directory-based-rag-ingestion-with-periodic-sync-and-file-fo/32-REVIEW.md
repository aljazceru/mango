---
phase: 32-directory-based-rag-ingestion
reviewed: 2026-04-19T00:00:00Z
depth: standard
iteration: 2
files_reviewed: 10
files_reviewed_list:
  - rust/src/lib.rs
  - rust/src/persistence/schema.rs
  - rust/src/persistence/queries.rs
  - rust/src/rag/directory_sync.rs
  - desktop/iced/src/views/directory_sources.rs
  - desktop/iced/src/main.rs
  - ios/Mango/Mango/DirectorySourcePicker.swift
  - ios/Mango/Mango/DirectorySourcesView.swift
  - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt
  - android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
status: clean
---

# Phase 32: Code Review Report (Iteration 2 — Post-Fix Re-Review)

**Reviewed:** 2026-04-19
**Depth:** standard
**Status:** clean (within scope of re-review)
**Prior iteration:** `32-REVIEW.md` iteration 1 → 13 findings; `32-REVIEW-FIX.md` resolved 7 of the 10 in-scope findings, deferred 3 (HI-01, ME-01, ME-04) as architectural follow-ups.

## Summary

All seven applied fixes (commits 86c38a3, c84fc1d, 726a05e, e451e80, 8790313, 45234b5, fe6cd66) were inspected against their respective original findings and the surrounding code. Each fix is correctly applied, semantically matches the review's recommended remedy, and does not introduce visible regressions.

The three skipped findings (HI-01 iOS bookmark cold-launch rehydration, ME-01 Android displayName collision, ME-04 Android glob path-vs-name) remain deferred per `32-REVIEW-FIX.md`. They are not re-flagged as blockers in this pass; they are genuine architectural follow-ups (new FFI accessors / non-trivial walker refactor) and the fix report's rationale is sound.

No new issues were detected.

## Fix Verification

### HI-02 — actor O(N²) DB reads (commit 86c38a3)
Verified at `rust/src/lib.rs:6318-6378`. `existing_by_path: HashMap<String, Option<String>>` is populated once before the removals loop from a single `list_directory_files_by_source` call and re-used in both the removals loop and the adds/modifies loop via `.get(...)`. The two prior per-iteration SELECTs are gone. `HashMap` import is already present at `rust/src/lib.rs:1`. Correct.

### HI-03 — 32 MiB file-size cap (commit c84fc1d)
Verified on all three platforms:
- `desktop/iced/src/main.rs:2121` — `const MAX_FILE_BYTES: i64 = 32 * 1024 * 1024;` checked before `std::fs::read`.
- `ios/Mango/Mango/DirectorySourcePicker.swift:249` — `let MAX_FILE_BYTES: Int64 = 32 * 1024 * 1024` checked inside the batch loop before `readFileBytes`.
- `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt:134` — `const val MAX_FILE_BYTES: Long = 32L * 1024L * 1024L` checked before `readFileContent`.

All three platforms log and skip oversized files rather than attempting the read. Cap value is consistent. Correct.

### HI-04 — embed-length mismatch rollback (commit 86c38a3)
Verified at `rust/src/lib.rs:6478-6509`. After `embed(...)`, `expected_len = texts.len() * dim` is compared against `embeddings.len()`; on mismatch the code deletes chunks (with usearch rowid removal), deletes the document row, pushes an error into `batch_errors`, and `continue`s — correctly bypassing the subsequent `upsert_directory_file` (line 6521) and the `DocumentSummary.push` (line 6531), leaving `directory_files.document_id = None` so the next sync retries. The `if !texts.is_empty()` guard correctly avoids spurious rollback when a file has zero chunks. Correct.

Note (informational, not a finding): as flagged in `32-REVIEW-FIX.md`, this rollback path has no dedicated unit test; recommend adding one before verification signs off. Not a blocker for the re-review.

### ME-02 — iOS removals on first batch (commit 726a05e)
Verified at `ios/Mango/Mango/DirectorySourcePicker.swift:356`. `let removedForThisBatch = idx == 0 ? diff.removedPaths : []` now matches desktop (`main.rs:2144`) and Android (`DirectorySourcePicker.kt:264`). The `isFinal` flag is still used for `isFinalBatch`, so no dead variable. Correct.

### ME-03 — iOS literal glob semantics (commit e451e80)
Verified at `ios/Mango/Mango/DirectorySourcePicker.swift:146-153`. The `hasPrefix(pattern)` that previously mis-matched `foobar.md` for exclusion `foo` has been replaced with `relPath == pattern || relPath.hasSuffix("/" + pattern)`, mirroring the dotfile branch and matching globset literal-filename semantics. Correct.

### ME-05 — accumulating batch errors (commit 86c38a3)
Verified at `rust/src/lib.rs:6314` (declaration as `Vec<String>`), `6402` and `6434` (push on extract/insert failures), `6502` (push on embed-length mismatch), and `6565-6572` (join with "; " before flipping source to `DirectorySyncStatus::Error`). All error sites now accumulate rather than overwrite. Correct.

### ME-06 — explicit expect on globs JSON (commit 8790313)
Verified at `rust/src/lib.rs:6259` (`AddDirectorySource`) and `rust/src/lib.rs:6641` (`SetDirectoryExclusions`). Both sites now `.expect("Vec<String> serialises to JSON")`. Silent "[]" fallback is gone. Correct.

### LO-01 — migration source ordering (commit 45234b5)
Verified at `rust/src/persistence/schema.rs`. `MIGRATION_V17` now appears before `MIGRATION_V18` in the source. The `MIGRATIONS` slice ordering was already correct; no behaviour change. Correct.

### LO-02 — stale allow(dead_code) (commit fe6cd66)
Verified at `rust/src/persistence/queries.rs:1205`. The `#[allow(dead_code)]` attribute on `count_directory_files` is removed. The function is live (called from `rust/src/lib.rs:6547`). Correct.

## Regression Scan

- `HashMap` usage in `SyncDirectoryFiles` is sound (owned `String` keys, `Option<String>` values cloned via `.and_then(|d| d.clone())`).
- The HI-04 `continue` correctly targets the per-file `for entry in &files` loop, so subsequent files in the same batch are still processed; per-batch `vector_index.save` at line 6543 still runs; `count_directory_files` + `last_synced` bookkeeping at line 6546 still runs on `is_final_batch`.
- ME-02 does not break the "empty changed, removals only" path (`if changedChunks.isEmpty` branch at iOS line 326 still sends removals with `isFinalBatch: true`).
- ME-03 does not alter the dotfile/extension/directory-prefix branches; only the `else` literal branch was modified.
- LO-01 is text-only; compiled output is unchanged.

## Skipped (Deferred) — Not Re-Flagged

- **HI-01** iOS bookmark cold-launch rehydration — requires new `get_directory_bookmark` FFI accessor.
- **ME-01** Android `resolveTreeUri` displayName collision — requires new `get_directory_tree_uri` FFI accessor.
- **ME-04** Android GlobMatcher path-vs-name — requires threading relative path through the BFS walker plus cross-platform test coverage.

These are acknowledged architectural follow-ups per `32-REVIEW-FIX.md` and are intentionally not raised as new findings in this iteration.

---

_Reviewed: 2026-04-19_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
_Iteration: 2 (post-fix)_
