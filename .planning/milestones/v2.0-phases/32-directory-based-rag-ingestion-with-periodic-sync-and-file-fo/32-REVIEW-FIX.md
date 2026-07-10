---
phase: 32-directory-based-rag-ingestion
fixed_at: 2026-04-19
review_path: .planning/phases/32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo/32-REVIEW.md
iteration: 1
findings_in_scope: 10
fixed: 7
skipped: 3
status: partial
---

# Phase 32: Code Review Fix Report

**Fixed at:** 2026-04-19
**Source review:** .planning/phases/32-directory-based-rag-ingestion-with-periodic-sync-and-file-fo/32-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope (High + Medium + Low): 10
- Fixed: 7
- Skipped: 3 (2 architectural — need new FFI surface; 1 non-trivial walker
  refactor deferred to a follow-up)

Note: HI-02, HI-04, and ME-05 were functionally intertwined in the
`SyncDirectoryFiles` handler (same actor arm, overlapping lines). They
were applied as one atomic commit to avoid incoherent intermediate
states; each finding is called out in the commit body.

## Fixed Issues

### HI-02: Actor handler re-reads `directory_files` once per file inside a 50-file batch (O(N²))

**Files modified:** `rust/src/lib.rs`
**Commit:** 86c38a3
**Applied fix:** Pre-fetch every tracked row for the source once before both
the removals loop and the adds/modifies loop, collect into
`HashMap<String, Option<String>>` keyed by `file_path`, and do O(1)
lookups inside the loops. Replaces two per-iteration `list_directory_files_by_source`
SELECTs with a single pre-batch query.

### HI-03: No maximum file-size cap before reading file bytes — OOM risk on mobile

**Files modified:** `desktop/iced/src/main.rs`, `ios/Mango/Mango/DirectorySourcePicker.swift`, `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt`
**Commit:** c84fc1d
**Applied fix:** Introduced `MAX_FILE_BYTES = 32 * 1024 * 1024` on all three
platforms. Each platform now checks the enumerator-reported size before
calling `std::fs::read` / `Data(contentsOf:)` / `readBytes()` and skips +
logs oversized files. Prevents a single 500 MB attachment in an Obsidian
vault from OOM-ing the mobile apps or freezing the actor thread during
embedding.

### HI-04: Embedding failure silently produces zero vectors — orphaned chunks with no searchable entries

**Files modified:** `rust/src/lib.rs`
**Commit:** 86c38a3
**Applied fix:** After the synchronous `embedding_provider.embed(...)` call,
verify returned length matches `texts.len() * EMBEDDING_DIM`. On mismatch,
delete the inserted chunks + document row (and corresponding usearch
entries), skip the `directory_files` upsert so the file retries on the
next sync, and push the error into `batch_errors`. The source now flips
to `DirectorySyncStatus::Error` instead of showing "Idle" while holding
orphaned rows.

**Note:** Requires human verification — logic rollback (chunk/document
deletion) was not exercised by a dedicated unit test; the embed-length
mismatch path is synthetic and the reviewer should confirm behaviour end
to end before the phase proceeds to verification.

### ME-02: iOS and Android disagree on which batch carries `removed_paths`

**Files modified:** `ios/Mango/Mango/DirectorySourcePicker.swift`
**Commit:** 726a05e
**Applied fix:** Changed iOS to attach `diff.removedPaths` to the first
batch (`idx == 0 ? diff.removedPaths : []`) matching desktop + Android.
Removes the partial-sync divergence where a user killing the app
mid-sync saw iOS=nothing-removed and Android/desktop=everything-removed.

### ME-03: iOS `GlobMatcher` literal-match uses `hasPrefix`, diverges from globset semantics

**Files modified:** `ios/Mango/Mango/DirectorySourcePicker.swift`
**Commit:** e451e80
**Applied fix:** Replaced the literal-branch `relPath.hasPrefix(pattern)`
with `relPath == pattern || relPath.hasSuffix("/" + pattern)` so an
exclusion of `foo` no longer drops `foobar.md`. Matches globset's
literal-filename semantics and mirrors the dotfile branch directly above.

### ME-05: `batch_error` overwrites instead of accumulating, hiding multiple errors

**Files modified:** `rust/src/lib.rs`
**Commit:** 86c38a3
**Applied fix:** Replaced `batch_error: Option<String>` with
`batch_errors: Vec<String>`, pushing each per-file failure
(`extract_text`, `insert_document`, and new embed-length mismatch), then
joining on "; " when flipping the source to Error. User now sees every
failed path for a partial batch, not just the last one.

### ME-06: `.unwrap_or_else(|_| "[]".into())` silently hides serialization failures for exclusion globs

**Files modified:** `rust/src/lib.rs`
**Commit:** 8790313
**Applied fix:** Replaced `.unwrap_or_else(|_| "[]".into())` in both
`AddDirectorySource` and `SetDirectoryExclusions` with
`.expect("Vec<String> serialises to JSON")`. Crashes loudly on a
programming error instead of quietly wiping user-supplied exclusions.

### LO-01: Migration source-order interleaves V17 and V18 declarations

**Files modified:** `rust/src/persistence/schema.rs`
**Commit:** 45234b5
**Applied fix:** Reordered the constant declarations so
`MIGRATION_V17` precedes `MIGRATION_V18` in the file. Pure cosmetic —
execution order through `MIGRATIONS` is unchanged — but source order now
matches execution order, removing a trap for the next contributor.

### LO-02: `allow(dead_code)` on `count_directory_files` is unnecessary — it *is* used

**Files modified:** `rust/src/persistence/queries.rs`
**Commit:** fe6cd66
**Applied fix:** Removed the stale `#[allow(dead_code)]` attribute.
`count_directory_files` is called from the `SyncDirectoryFiles` handler
in `rust/src/lib.rs` so the allow was silencing a warning that could no
longer fire.

## Skipped Issues

### HI-01: iOS bookmark cache is process-local and never rehydrated — periodic sync is broken after cold launch

**File:** `ios/Mango/Mango/DirectorySourcesView.swift:359-391`
**Reason:** skipped — requires new FFI surface. The review explicitly
flags this as "an architectural gap, not just a code omission": a new
`get_directory_bookmark(source_id) -> Result<Option<Vec<u8>>, FfiError>`
accessor has to be added to `FfiApp`, wired through `uniffi.toml`,
regenerated into Swift, and then called from `MangoApp.init` /
`AppManager.init`. That touches the stable FFI surface and the T-32-I2
threat-model decision (bookmark bytes deliberately excluded from
`DirectorySourceSummary`). Belongs in a dedicated small phase with its
own plan entry, not a review-fix hotfix.

### ME-01: Android `resolveTreeUri` matches by displayName — two folders with the same name collide

**File:** `android/app/src/main/java/dev/disobey/mango/ui/DirectorySyncWorker.kt:78-101`
**Reason:** skipped — same architectural shape as HI-01. The correct fix
(option (a) in the review) is a new FFI accessor
`get_directory_tree_uri(source_id)`; option (b) (smuggling the URI into
the display_name field) is called out as "less clean" by the reviewer
and would regress UI presentation. Defensive log-and-skip on duplicate
derived names is a partial mitigation but does not fix the underlying
ambiguity. Deferring with HI-01 into the same follow-up phase so both
FFI accessors land together.

### ME-04: Android `GlobMatcher` only sees the file's display name, not relative path

**File:** `android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt:152-161, 112`
**Reason:** skipped — non-trivial refactor deferred to a follow-up. The
fix described by the reviewer (thread the accumulated relative path
through the BFS stack, then split patterns into anchored/unanchored and
apply accordingly to match `ignore::OverrideBuilder`) crosses both the
`traverseTree` walker and `GlobMatcher` class boundaries and needs
matching test-coverage updates to assert `projects/drafts/` vs top-level
`drafts/` behaviour across platforms. The review itself notes "On
balance this works today but is fragile" — it's a latent correctness
issue, not a live bug. Tracking as a follow-up rather than forcing a
rushed change to the matcher.

---

_Fixed: 2026-04-19_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
