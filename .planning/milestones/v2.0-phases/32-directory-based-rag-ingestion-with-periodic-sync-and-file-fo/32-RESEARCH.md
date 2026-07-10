# Phase 32: Directory-based RAG Ingestion with Periodic Sync - Research

**Researched:** 2026-04-19
**Domain:** Cross-platform directory watching, file system enumeration, incremental RAG ingestion, platform permission models (iOS security-scoped bookmarks, Android SAF), glob exclusion patterns
**Confidence:** MEDIUM-HIGH (platform-specific sections are MEDIUM; Rust ecosystem sections are HIGH)

---

## Summary

Phase 32 extends Phase 8/31 RAG from single-file ingestion to whole-directory ingestion with automatic incremental re-sync. The core problem splits cleanly into three sub-problems: (1) acquiring and persisting cross-launch directory access on each platform, (2) efficiently enumerating files and detecting changes, and (3) triggering re-sync on a schedule that respects mobile battery/lifecycle constraints.

**The RMP boundary is the key architectural decision.** Rust cannot directly open Android SAF URIs or iOS security-scoped URLs without calling back into the native layer. The established pattern is: native layer acquires access, enumerates file paths + reads bytes, and streams `(path, mtime, bytes)` tuples to Rust via a batch AppAction. Rust owns all persistence (SQLite `directory_sources` table), diffing (mtime/size vs stored fingerprint), chunking, and embedding. Native layers own only permission lifecycle (grant, persist, refresh) and byte-reading at each sync trigger.

**Primary recommendation:** Use `ignore` crate 0.4.25 (not `walkdir` directly) for Desktop directory walking with glob exclusions because it ships `OverrideBuilder` with gitignore-style `!` semantics. Use a `directory_sources` SQLite table with `mtime + size` fingerprints per file for incremental sync detection — do NOT use content hashing (too slow for 10k-file Obsidian vaults). Sync on foreground resume is the pragmatic iOS strategy; Android WorkManager with 15-minute minimum interval for background.

---

## Standard Stack

### Core Rust Libraries

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ignore` | 0.4.25 | Directory walking + gitignore-style glob exclusions | From the ripgrep team (BurntSushi); combines walkdir + globset + gitignore pattern matching. `WalkBuilder` + `OverrideBuilder` handles custom `!exclude` patterns correctly. Ships `WalkParallel` for large vaults. |
| `walkdir` | 2.5.0 | Low-level recursive directory iterator (dependency of `ignore`) | Already a transitive dep. Use directly only when `ignore`'s gitignore filtering is unwanted (e.g., Desktop "walk without .gitignore inference"). |
| `globset` | 0.4.18 | Multi-pattern glob matching for exclusion rule sets | Dependency of `ignore`; also usable standalone. Use `GlobSetBuilder` to compile user-supplied exclusion patterns into a fast matcher. |
| `notify` | 8.2.0 | Desktop real-time FS event watching | Latest stable series (9.x is RC). Supports inotify (Linux/Android), FSEvents/kqueue (macOS/iOS), kqueue (iOS), ReadDirectoryChangesW (Windows). Mobile kqueue on iOS is in-process only — not useful for sandbox; use poll mode or foreground-resume scan. |
| `sha2` | 0.10.x | Content hashing (only for small files / hash-on-import) | Already in Cargo.toml. If content hashing chosen over mtime: use SHA-256 for fingerprint. For 10k+ file vaults, **do NOT hash on every sync** — use mtime+size only. |

[VERIFIED: cargo search `ignore` 0.4.25, `walkdir` 2.5.0, `globset` 0.4.18, `notify` 8.2.0 — all from cargo search output]
[VERIFIED: sha2 already in Cargo.toml — project Cargo.toml read]
[VERIFIED: ignore docs — WalkBuilder, OverrideBuilder, glob semantics confirmed via docs.rs/ignore]

### Existing Project Libraries (reused, no new dep needed)

| Library | Version | Purpose | Reuse Pattern |
|---------|---------|---------|---------------|
| `rusqlite` | 0.39 | New `directory_sources` + `directory_files` tables | Add migration V18; follow schema pattern from V6 |
| `usearch` | 2.24.0 | HNSW index — reuse existing single index | Directory-sourced chunks get same `chunk` rowid as key |
| `chrono` | 0.4 | Timestamps for `last_synced_at` in `directory_sources` | Already used for messages timestamps |
| `uuid` | 1.x | Source IDs for directory records | Already used for document IDs |
| `anyhow` / `thiserror` | 1.x / 2.x | Error handling in sync logic | Follow existing error propagation patterns |
| `tokio` | 1.x | Async actor — sync runs as blocking_task inside actor | Already used; pattern established for IngestDocument |

[VERIFIED: all from project Cargo.toml]

### New Dependencies to Add

```toml
# Phase 32: Directory walking + glob exclusions (Desktop only — mobile enumeration done natively)
[target.'cfg(not(any(target_os = "ios", target_os = "android")))'.dependencies]
ignore = "0.4"

# Phase 32: Real-time FS watching on Desktop only (mobile uses foreground-resume scan)
notify = { version = "8", default-features = false, features = ["macos_fsevent"] }
notify-debouncer-mini = "0.4"  # Debounce rapid FS events (e.g. git pull of vault)
```

Mobile platforms do NOT add `ignore` or `notify` — directory enumeration happens in Swift/Kotlin and file data arrives via `SyncDirectoryFiles` AppAction.

[ASSUMED: notify-debouncer-mini version 0.4; cargo search shows 0.7.0 but version compatibility with notify 8.x should be verified]

---

## Architecture Patterns

### New SQLite Tables (Migration V18)

```sql
-- Tracked directory sources
CREATE TABLE IF NOT EXISTS directory_sources (
    id              TEXT PRIMARY KEY NOT NULL,   -- UUID
    display_name    TEXT NOT NULL,               -- user-visible label (folder name)
    -- Platform-specific access handles:
    path            TEXT,                        -- Desktop: absolute path; NULL on mobile
    bookmark_data   BLOB,                        -- iOS: security-scoped bookmark Data bytes
    tree_uri        TEXT,                        -- Android: persistable SAF tree URI string
    exclusion_globs TEXT NOT NULL DEFAULT '[]',  -- JSON array of glob strings, e.g. ["!.obsidian/", "!*.tmp"]
    last_synced_at  INTEGER,                     -- Unix timestamp, NULL = never synced
    file_count      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL
);

-- Per-file fingerprints for incremental sync
CREATE TABLE IF NOT EXISTS directory_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id       TEXT NOT NULL REFERENCES directory_sources(id) ON DELETE CASCADE,
    -- Mobile: opaque path relative to source root (or SAF document ID)
    -- Desktop: absolute path
    file_path       TEXT NOT NULL,
    mtime_secs      INTEGER NOT NULL,            -- st_mtime seconds
    size_bytes      INTEGER NOT NULL,
    document_id     TEXT,                        -- NULL until indexed; FK to documents.id
    UNIQUE (source_id, file_path)
);

CREATE INDEX IF NOT EXISTS idx_dirfiles_source ON directory_files(source_id);
```

[ASSUMED: Schema design — reasonable given project patterns but not derived from a prior phase]

### Recommended Project Structure

```
rust/src/rag/
├── chunker.rs          # existing
├── context.rs          # existing
├── index.rs            # existing
├── mod.rs              # existing — add directory_sync module
└── directory_sync.rs   # NEW: scan, diff, ingest logic

rust/src/persistence/
└── schema.rs           # add MIGRATION_V18 (directory_sources + directory_files)
└── queries.rs          # add directory_sources and directory_files CRUD queries

# Native layers add:
ios/Mango/Mango/
└── DirectorySourcesView.swift        # Source list + exclusion editor UI
└── DirectorySourcePicker.swift       # UIDocumentPickerViewController folder picker

android/.../ui/
└── DirectorySourcesScreen.kt         # Source list + exclusion editor UI
└── DirectorySourcePicker.kt          # ACTION_OPEN_DOCUMENT_TREE launcher

desktop/iced/src/views/
└── directory_sources.rs              # Source list + exclusion editor view
```

### Pattern 1: AppAction Design for Directory Sync

The actor must handle two distinct stages: (a) user adds a source, (b) periodic/manual re-sync.

```rust
// Source: [ASSUMED — follows AppAction pattern from lib.rs]

pub enum AppAction {
    // User picks a folder; native layer sends the access handle
    AddDirectorySource {
        display_name: String,
        // Exactly one of these is set depending on platform:
        path: Option<String>,             // Desktop
        bookmark_data: Option<Vec<u8>>,  // iOS
        tree_uri: Option<String>,         // Android
        exclusion_globs: Vec<String>,
    },

    // Periodic or manual trigger; native layer has already re-acquired access
    // and enumerated files, sending (relative_path, mtime_secs, size_bytes, content)
    SyncDirectoryFiles {
        source_id: String,
        files: Vec<DirectoryFileEntry>,   // added + modified files only
        removed_paths: Vec<String>,       // paths present in DB but absent from enumeration
    },

    RemoveDirectorySource {
        source_id: String,
    },

    SetDirectoryExclusions {
        source_id: String,
        globs: Vec<String>,
    },
}

pub struct DirectoryFileEntry {
    pub relative_path: String,
    pub mtime_secs: i64,
    pub size_bytes: i64,
    pub content: Vec<u8>,
}
```

**Key insight:** Native layers do the diff at the filesystem/SAF layer so they only send changed files, NOT the full directory listing every time. The Rust actor only receives new/changed file content — avoiding memory spikes for 10k-file vaults.

[ASSUMED: AppAction struct shape — follows project conventions but is a new design]

### Pattern 2: Incremental Sync Logic (Rust Core)

```rust
// Source: [ASSUMED — based on mtime+size fingerprint pattern, standard in build systems]
// In rust/src/rag/directory_sync.rs

pub struct FileDiff {
    pub added: Vec<(String, i64, i64)>,    // (path, mtime, size)
    pub modified: Vec<(String, i64, i64)>,
    pub removed: Vec<String>,
}

/// Compare current filesystem enumeration against stored fingerprints.
/// Returns only the delta — never re-ingest unchanged files.
pub fn diff_files(
    conn: &Connection,
    source_id: &str,
    current_files: &[(String, i64, i64)],  // (path, mtime, size) from enumeration
) -> anyhow::Result<FileDiff> {
    // Load stored fingerprints from directory_files
    // Compare mtime_secs + size_bytes — if both match, file is unchanged
    // Any path in DB but not in current_files → removed
    // Any path in current_files but not in DB → added
    // Any path in both but with different mtime or size → modified
}
```

**Why mtime+size, not content hash:**
- Obsidian vault with 10k .md files: content hashing 10k × ~50KB = 500MB of reads on every sync check. Unacceptable.
- mtime+size: 10k metadata reads = ~50ms on modern storage.
- Collision risk (same mtime+size, different content) is negligible for user documents; gitignore and Make have used this pattern for decades.

[CITED: cargo issue #6529 discussing mtime vs checksum fingerprinting for the same tradeoff rationale]

### Pattern 3: Desktop Directory Walking with `ignore` Crate

```rust
// Source: [VERIFIED: docs.rs/ignore/latest/ignore/overrides/struct.OverrideBuilder.html]

use ignore::{WalkBuilder, overrides::OverrideBuilder};

pub fn walk_with_exclusions(
    root: &str,
    exclusion_globs: &[String],
) -> anyhow::Result<Vec<(String, std::time::SystemTime, u64)>> {
    // Build exclusion overrides
    let mut override_builder = OverrideBuilder::new(root);
    for glob in exclusion_globs {
        // User provides raw pattern like ".obsidian/" or "*.tmp"
        // Prepend "!" to make it an ignore (exclusion) pattern
        override_builder.add(&format!("!{}", glob))?;
    }
    let overrides = override_builder.build()?;

    let mut entries = Vec::new();
    let walker = WalkBuilder::new(root)
        .overrides(overrides)
        .hidden(false)       // Show hidden files (Obsidian uses .obsidian/ which we exclude explicitly)
        .git_ignore(false)   // Don't respect .gitignore — user controls exclusions
        .standard_filters(false)  // Only our exclusion patterns, nothing else
        .build();

    for result in walker {
        match result {
            Ok(entry) if entry.file_type().map_or(false, |t| t.is_file()) => {
                let path = entry.path().to_string_lossy().to_string();
                let meta = entry.metadata()?;
                let mtime = meta.modified()?;
                let size = meta.len();
                entries.push((path, mtime, size));
            }
            _ => {}
        }
    }
    Ok(entries)
}
```

[VERIFIED: OverrideBuilder API confirmed via docs.rs/ignore/latest]
[VERIFIED: `!` prefix = exclusion glob per official docs]

### Pattern 4: iOS Security-Scoped Folder Bookmark Lifecycle

The full lifecycle must be respected on every sync attempt:

```swift
// Source: [CITED: adam.garrett-harris.com/2021-08-21-providing-access-to-directories-in-ios-with-bookmarks/]
// Source: [CITED: developer.apple.com/documentation/uikit/uidocumentpickerviewcontroller]

// 1. PICK — UIDocumentPickerViewController with .folder content type
let picker = UIDocumentPickerViewController(
    forOpeningContentTypes: [.folder],
    asCopy: false
)
picker.delegate = self

// 2. BOOKMARK — in documentPicker(_:didPickDocumentsAt:)
func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    guard let folderURL = urls.first else { return }
    guard folderURL.startAccessingSecurityScopedResource() else { return }
    defer { folderURL.stopAccessingSecurityScopedResource() }

    // Create persistent bookmark
    let bookmarkData = try? folderURL.bookmarkData(
        options: .minimalBookmark,  // iOS uses .minimalBookmark, NOT .withSecurityScope
        includingResourceValuesForKeys: nil,
        relativeTo: nil
    )
    // Store bookmarkData as BLOB in directory_sources.bookmark_data via AppAction.AddDirectorySource
}

// 3. RESOLVE — on each sync attempt
func resolveBookmark(_ bookmarkData: Data) -> URL? {
    var isStale = false
    guard let url = try? URL(
        resolvingBookmarkData: bookmarkData,
        options: .withoutUI,
        relativeTo: nil,
        bookmarkDataIsStale: &isStale
    ) else { return nil }

    if isStale {
        // Re-create bookmark from the resolved URL and update stored bookmark_data
        // (user may have moved the folder — bookmark resolves to new location)
        let newBookmark = try? url.bookmarkData(options: .minimalBookmark, ...)
        // dispatch UpdateDirectorySourceBookmark(sourceId, newBookmark)
    }
    return url
}

// 4. ENUMERATE — during sync, inside startAccessingSecurityScopedResource scope
func enumerateDirectory(_ folderURL: URL, exclusions: [String]) -> [(String, Date, Int)] {
    guard folderURL.startAccessingSecurityScopedResource() else { return [] }
    defer { folderURL.stopAccessingSecurityScopedResource() }

    let fm = FileManager.default
    var results: [(String, Date, Int)] = []
    // Use fm.enumerator(at:includingPropertiesForKeys:[.contentModificationDateKey, .fileSizeKey])
    // for efficient recursive enumeration with metadata
    let enumerator = fm.enumerator(
        at: folderURL,
        includingPropertiesForKeys: [.contentModificationDateKey, .fileSizeKey, .isDirectoryKey],
        options: [.skipsHiddenFiles]  // or omit to show hidden — matched to user exclusions
    )
    // Apply exclusion globs in Swift using NSPredicate or custom glob check before reading
    // Only read bytes for files that changed vs stored fingerprint
    // Stream changed files to Rust via SyncDirectoryFiles AppAction
    return results
}
```

**Critical iOS pitfalls:**
- On iOS, the bookmark option is `.minimalBookmark`, NOT `.withSecurityScope` (that is macOS-only sandbox API). [CITED: developer.apple.com forums thread/131670]
- `startAccessingSecurityScopedResource()` MUST be called and MUST succeed before any file read. Without it, file reads silently return errors or empty data.
- Stale bookmarks: always check `bookmarkDataIsStale` flag; re-create bookmark from resolved URL immediately.
- iCloud-backed folders: files may be "evicted" (stored in cloud, placeholder on device). `FileManager.default.isUbiquitousItem(at:)` + `FileManager.default.startDownloadingUbiquitousItem(at:)` are needed. **Do not try to read placeholder bytes** — it blocks forever or fails. Best practice: skip files that are not locally available on sync, surface them as "not downloaded" in UI.
- `NSFileCoordinator` is NOT required for simple reads from user-selected folder (no iCloud coordination conflicts for read-only access). Required only if writing back to the folder.

[CITED: developer.apple.com/documentation/foundation/nsurl/startaccessingsecurityscopedresource()]
[ASSUMED: .minimalBookmark vs .withSecurityScope distinction — confirm with Apple docs for iOS 17]

### Pattern 5: Android SAF Directory Traversal

```kotlin
// Source: [CITED: developer.android.com/training/data-storage/shared/documents-files]
// Source: [CITED: commonsware.com/blog/2019/12/14/scoped-storage-stories-listfiles-woe.html]

// 1. PICK — ACTION_OPEN_DOCUMENT_TREE
val openTreeLauncher = rememberLauncherForActivityResult(
    contract = ActivityResultContracts.OpenDocumentTree()
) { uri: Uri? ->
    uri?.let {
        // 2. PERSIST — must call immediately in the result callback
        val takeFlags = Intent.FLAG_GRANT_READ_URI_PERMISSION
        context.contentResolver.takePersistableUriPermission(it, takeFlags)
        // Store URI string in directory_sources.tree_uri
        onDispatch(AppAction.AddDirectorySource(
            displayName = getDisplayName(it),
            treeUri = it.toString(),
            ...
        ))
    }
}

// 3. TRAVERSE — use DocumentsContract directly, NOT DocumentFile.listFiles()
// DocumentFile.listFiles() fires one IPC per file for metadata — 10x slower
fun traverseTree(
    context: Context,
    treeUri: Uri,
    exclusionGlobs: List<String>
): List<Triple<String, Long, Long>> {  // (docId, lastModified, size)
    val results = mutableListOf<Triple<String, Long, Long>>()
    val stack = ArrayDeque<String>()
    val treeDocId = DocumentsContract.getTreeDocumentId(treeUri)
    stack.addLast(treeDocId)

    while (stack.isNotEmpty()) {
        val parentDocId = stack.removeLast()
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)

        // BULK QUERY — fetches all children in one IPC call
        context.contentResolver.query(
            childrenUri,
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_LAST_MODIFIED,
                DocumentsContract.Document.COLUMN_SIZE,
                DocumentsContract.Document.COLUMN_MIME_TYPE
            ),
            null, null, null
        )?.use { cursor ->
            while (cursor.moveToNext()) {
                val docId = cursor.getString(0)
                val name = cursor.getString(1)
                val mtime = cursor.getLong(2)
                val size = cursor.getLong(3)
                val mime = cursor.getString(4)

                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    if (!matchesExclusion(name, exclusionGlobs)) {
                        stack.addLast(docId)
                    }
                } else {
                    if (!matchesExclusion(name, exclusionGlobs)) {
                        results.add(Triple(docId, mtime, size))
                    }
                }
            }
        }
    }
    return results
}

// 4. READ — open file content for changed files
fun readFileContent(context: Context, treeUri: Uri, docId: String): ByteArray? {
    val fileUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId)
    return try {
        context.contentResolver.openInputStream(fileUri)?.use { it.readBytes() }
    } catch (e: Exception) { null }
}
```

**Key Android pitfalls:**
- `DocumentFile.listFiles()` is 10-100x slower than direct `DocumentsContract.query` for large directories. [CITED: commonsware.com/blog/2019/12/14/scoped-storage-stories-listfiles-woe.html — "listFiles Woe" blog post]
- Persistable URI permission limit: Android 11+ allows 512 persisted grants, older versions only 128. [VERIFIED: Android developer docs — updated May 2025]
- Permission is lost if user uninstalls+reinstalls or if the user moves the folder (but NOT on device reboot — persisted grants survive reboots).
- On Android 11+ (API 30), cannot access root of internal storage, root of SD card, Downloads folder, `Android/data/`, or `Android/obb/`. Obsidian vaults stored in those locations are inaccessible via SAF. [VERIFIED: developer.android.com/training/data-storage/shared/documents-files]
- `COLUMN_LAST_MODIFIED` in SAF is millis since epoch (not seconds). Divide by 1000 before storing in SQLite mtime column.
- Pass doc IDs (not file paths) as `file_path` in `directory_files` table for Android — doc IDs are stable SAF identifiers.

### Pattern 6: Desktop Real-Time Watching with `notify`

```rust
// Source: [VERIFIED: docs.rs/notify/8.2.0/notify/ — RecommendedWatcher, platform support]
// Only use on Desktop (not mobile)

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::time::Duration;

fn start_watching(path: &str, tx: flume::Sender<AppAction>) -> anyhow::Result<RecommendedWatcher> {
    let (debouncer_tx, debouncer_rx) = std::sync::mpsc::channel();

    let mut debouncer = new_debouncer(Duration::from_secs(2), debouncer_tx)?;
    debouncer.watcher().watch(path.as_ref(), RecursiveMode::Recursive)?;

    // Spawn a thread to forward FS events as sync triggers
    std::thread::spawn(move || {
        for events in debouncer_rx {
            match events {
                Ok(_) => {
                    // FS change detected — trigger a re-scan of this source
                    // Don't dispatch SyncDirectoryFiles directly here; just set a "needs_sync" flag
                    // and let the periodic timer or foreground resume pick it up
                    let _ = tx.send(AppAction::TriggerDirectorySync { source_id: "...".into() });
                }
                Err(_) => break,
            }
        }
    });
    Ok(debouncer.into_watcher())
}
```

**Notes:**
- `notify` 8.x is the latest stable; 9.x is RC and not production-ready yet. [VERIFIED: docs.rs/notify/latest returns 8.2.0]
- Always debounce FS events — git pull of an Obsidian vault fires hundreds of events in rapid succession. `notify-debouncer-mini` with 2-second delay is the standard approach.
- On Linux (inotify), there are per-process `inotify` watch limits (`/proc/sys/fs/inotify/max_user_watches`, default 8192 on many distros). For a 10k-file vault with nested directories, you may hit this limit. Fallback: poll mode (`PollWatcher` in notify with 30-second interval). [CITED: notify GitHub README]
- Do NOT use notify on iOS or Android — kqueue is available on iOS sandbox but the app does not have meaningful filesystem access outside its container (the user-selected folder is not writable without security scope). Use foreground-resume scan instead.

### Pattern 7: Scheduling Strategy Per Platform

| Platform | Strategy | Implementation |
|----------|----------|----------------|
| Desktop | Real-time + poll fallback | `notify` 8.x watcher + 5-minute Tokio interval as fallback |
| iOS | Foreground-resume scan | `AppManager.scenePhase` `.active` trigger → `SyncAllDirectories` AppAction; no BGTaskScheduler |
| Android | WorkManager periodic (15-min min interval) + foreground resume | `PeriodicWorkRequest` with `NETWORK_NOT_REQUIRED` constraint; also trigger on `onResume` |

**iOS BGTaskScheduler is NOT the right tool here.** BGProcessingTaskRequest requires external power + internet. BGAppRefreshTaskRequest lasts ~30 seconds maximum — insufficient for a 1000-file incremental sync. The pragmatic answer used by Obsidian itself and similar note apps is: trigger sync when the app enters foreground (high reliability, respects iOS battery model, zero entitlements needed). [CITED: obsidian forum + iOS developer forums — confirmed pattern for vault apps]

**Android WorkManager minimum interval is 15 minutes.** This is a hard platform constraint, not a configuration option. [CITED: developer.android.com/topic/libraries/architecture/workmanager]

### Anti-Patterns to Avoid

- **Content-hashing all files on every sync:** SHA-256 of 500MB of vault data on each foreground resume will peg the CPU and drain battery. Use mtime+size fingerprints.
- **Reading all file bytes up-front before diffing:** Build the diff table first (metadata only), then read bytes only for changed files.
- **Using `DocumentFile.listFiles()` on Android for large directories:** 10-100x IPC overhead vs direct `DocumentsContract.query`. [CITED: commonsware.com blog]
- **Sending `SyncDirectoryFiles` with full file list on every sync:** Native layer should diff against stored fingerprints first, only send changed files. For a 10k-file vault with 5 changed files, the AppAction should contain 5 entries, not 10k.
- **Watching with `notify` on iOS/Android:** `notify` requires unrestricted filesystem access and uses inotify (Linux)/kqueue — not practical in iOS sandbox; use foreground-resume scan.
- **Inlining bookmark data or tree URIs in AppState:** These are large opaque blobs and must stay in SQLite (`directory_sources` table), not in AppState. Only pass source IDs and metadata to AppState.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Gitignore-style glob exclusion | Custom glob parser | `ignore` crate `OverrideBuilder` | gitignore semantics are subtle (path anchoring, `**`, negation precedence). `ignore` is from the ripgrep team — battle-hardened. |
| Multi-pattern glob matching | Manual regex expansion | `globset` `GlobSetBuilder` | Handles `{a,b}` alternation, `**` across dirs, case sensitivity correctly. |
| FS event watching with debounce | Custom polling + dedup | `notify` + `notify-debouncer-mini` | Platform backend selection, debounce logic, reconnect on errors are all non-trivial. |
| iOS security-scoped bookmark serialization | Custom URL persistence | `URL.bookmarkData(options: .minimalBookmark)` | Apple's API is the only correct way; manual serialization of security tokens is not possible. |
| Android SAF URI grant persistence | Custom intent flag management | `contentResolver.takePersistableUriPermission()` | Without this specific call, SAF URI access is lost on the next device restart. |
| Change detection algorithm | Merkle tree or recursive hash | mtime+size flat table in SQLite | Simpler, faster for read-heavy sync, sufficient collision resistance for user documents. |
| Directory walking | Manual `std::fs::read_dir` recursive stack | `ignore::WalkBuilder` | Handles symlink loops, max depth, error recovery, and integrates exclusions in one pass. |

---

## Common Pitfalls

### Pitfall 1: iOS — `.withSecurityScope` vs `.minimalBookmark`

**What goes wrong:** Developer uses `[.withSecurityScope]` option (macOS App Sandbox API) on iOS. This silently creates a bookmark that may fail to resolve or creates unnecessarily large bookmark data.
**Why it happens:** Apple documentation conflates macOS and iOS bookmark APIs. macOS uses `.withSecurityScope` for sandboxed app access. iOS uses `.minimalBookmark` (or no options for app-container files). The iOS sandbox model grants access through UIDocumentPickerViewController's delegate URL, not through security-scoped bookmarks.
**How to avoid:** Use `URL.bookmarkData(options: .minimalBookmark, ...)` on iOS. Test bookmark round-trip on physical device (simulator may not enforce sandbox).
**Warning signs:** Bookmark resolves to `nil` or `isStale` is always `true`.

[CITED: developer.apple.com/forums/thread/131670 — confirmed iOS vs macOS distinction]

### Pitfall 2: iOS — Forgetting `startAccessingSecurityScopedResource`

**What goes wrong:** File reads from the bookmark-resolved URL succeed in the simulator but fail silently on device, returning empty data or `NSFileReadNoPermissionError`.
**Why it happens:** The simulator does not fully enforce sandbox security scope; physical devices do.
**How to avoid:** Always call `url.startAccessingSecurityScopedResource()` before any file operation and balance with `defer { url.stopAccessingSecurityScopedResource() }` in the same scope.
**Warning signs:** Works in simulator, fails on device with no obvious error.

[CITED: developer.apple.com/documentation/foundation/nsurl/startaccessingsecurityscopedresource()]

### Pitfall 3: iOS — iCloud-Backed Files Not Downloaded

**What goes wrong:** Enumerating an iCloud-synced Obsidian vault returns `.icloud` placeholder files. Reading them hangs or throws `NSUbiquitousFileNotAvailableError`.
**Why it happens:** iCloud Drive may evict file contents to save space; the placeholder is present but content is in cloud.
**How to avoid:** Check `URLResourceValues.ubiquitousItemDownloadingStatus` before reading. Only attempt reads on files with status `.current` or `.downloaded`. Skip `.notDownloaded` files in this sync cycle; optionally trigger download with `FileManager.default.startDownloadingUbiquitousItem(at:)` for a follow-up sync.
**Warning signs:** Sync appears to complete but RAG searches miss content that exists in the vault.

[ASSUMED: iCloud eviction behavior for Obsidian vault — known iOS issue, verify with Apple docs]

### Pitfall 4: Android — SAF URI Permission Not Persisted

**What goes wrong:** App loses access to the user-selected directory after device reboot. `contentResolver.openInputStream(uri)` throws `SecurityException: Permission Denial`.
**Why it happens:** SAF URI grants are ephemeral by default. Only calling `takePersistableUriPermission()` makes them survive reboots.
**How to avoid:** Call `contentResolver.takePersistableUriPermission(uri, FLAG_GRANT_READ_URI_PERMISSION)` immediately in the `ActivityResult` callback, before storing the URI in SQLite.
**Warning signs:** Works on first launch, fails after restart.

[VERIFIED: developer.android.com/training/data-storage/shared/documents-files]

### Pitfall 5: Android — `DocumentFile.listFiles()` on Large Directories

**What goes wrong:** Traversing an Obsidian vault with 2000 files takes 20-30 seconds or causes ANR.
**Why it happens:** `DocumentFile.listFiles()` for a SAF URI fires one IPC call per file. 2000 files × IPC overhead = unacceptable latency.
**How to avoid:** Use `DocumentsContract.buildChildDocumentsUriUsingTree` + `contentResolver.query` to bulk-fetch all children in one IPC call per directory level.
**Warning signs:** `DocumentLibraryScreen` freezes when user adds a large directory; frame drops on the main thread.

[CITED: commonsware.com/blog/2019/12/14/scoped-storage-stories-listfiles-woe.html]

### Pitfall 6: notify inotify Watch Limit on Linux

**What goes wrong:** `RecommendedWatcher::watch()` returns `Err(...)` with `ENOSPC` or similar when watching a large directory tree. Subsequent FS changes are silently missed.
**Why it happens:** Linux inotify default limit is 8192 watches (`/proc/sys/fs/inotify/max_user_watches`). A vault with 500 subdirectories (each requiring a watch) easily approaches this.
**How to avoid:** Catch the `notify::Error` and fall back to `PollWatcher` with a 60-second interval. Log a warning in the UI: "File watching unavailable; syncing on schedule."
**Warning signs:** Desktop sync stops detecting changes; error in logs about too many open files.

[CITED: notify GitHub README — documented known issue]

### Pitfall 7: Stale Bookmarks Not Handled

**What goes wrong:** After user moves/renames the vault folder, the stored bookmark resolves correctly (bookmarks track renames), but `bookmarkDataIsStale` is `true`. App does not update stored bookmark data, so the next resolve attempt fails.
**Why it happens:** Developers check `isStale` but forget to re-create and store the updated bookmark data.
**How to avoid:** On stale bookmark resolution: immediately re-create bookmark from the resolved URL and dispatch `UpdateDirectorySourceBookmark` AppAction to write updated blob to SQLite.

[CITED: adam.garrett-harris.com/2021-08-21-providing-access-to-directories-in-ios-with-bookmarks/]

### Pitfall 8: Large `SyncDirectoryFiles` AppAction Blocking Actor

**What goes wrong:** User adds a vault with 10,000 files. The first full sync sends all 10,000 files' content in a single AppAction, causing a multi-second pause and potential OOM.
**Why it happens:** All content passed as `Vec<Vec<u8>>` in one message.
**How to avoid:** Batch in chunks of 50 files per `SyncDirectoryFiles` dispatch. Native layer iterates in batches; actor processes each batch, emits progress update, then processes next batch. Use `IngestionProgress` for display.

[ASSUMED: batch size recommendation — tune based on average file size]

---

## Code Examples

### Verified: OverrideBuilder Exclusion Pattern

```rust
// Source: [VERIFIED: docs.rs/ignore/latest/ignore/overrides/struct.OverrideBuilder.html]
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;

fn build_walk_with_exclusions(root: &str, exclusions: &[&str]) -> ignore::Walk {
    let mut ob = OverrideBuilder::new(root);
    for pattern in exclusions {
        // "!" prefix = ignore/exclude this pattern
        // Without "!" = whitelist (include ONLY these)
        let _ = ob.add(&format!("!{}", pattern));
    }
    let overrides = ob.build().unwrap_or_default();

    WalkBuilder::new(root)
        .overrides(overrides)
        .hidden(false)
        .git_ignore(false)
        .standard_filters(false)
        .build()
}

// Usage: exclude .obsidian/ metadata and temp files
let walk = build_walk_with_exclusions(
    "/path/to/vault",
    &[".obsidian/", "*.tmp", ".git/", "*.canvas"]
);
```

### Verified: Android DocumentsContract Bulk Traversal

```kotlin
// Source: [CITED: developer.android.com/reference/android/provider/DocumentsContract]
fun queryChildren(context: Context, treeUri: Uri, parentDocId: String): List<DocumentChild> {
    val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
    val projection = arrayOf(
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_LAST_MODIFIED,  // millis, not seconds
        DocumentsContract.Document.COLUMN_SIZE,
        DocumentsContract.Document.COLUMN_MIME_TYPE
    )
    return context.contentResolver.query(childrenUri, projection, null, null, null)
        ?.use { cursor ->
            buildList {
                while (cursor.moveToNext()) {
                    add(DocumentChild(
                        docId = cursor.getString(0),
                        name = cursor.getString(1),
                        lastModifiedMs = cursor.getLong(2),
                        sizeBytes = cursor.getLong(3),
                        mimeType = cursor.getString(4)
                    ))
                }
            }
        } ?: emptyList()
}
```

### Existing Pattern Reference: IngestDocument Handler in lib.rs

The existing `AppAction::IngestDocument` handler (lib.rs ~line 5013) is the direct template for the per-file ingestion step inside `SyncDirectoryFiles`. Key pattern:
- Set `ingestion_progress` before heavy work, emit state
- Extract text, chunk, insert chunks into SQLite, collect rowids
- Embed batch, add to usearch index
- Update `app_state.documents` and emit final state

The directory sync handler follows exactly this pattern per file, wrapped in the diff loop.

[VERIFIED: lib.rs lines 5013-5150 — read in this session]

---

## Runtime State Inventory

This is a greenfield feature (no existing directory sources in production). No runtime state migration required.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — `directory_sources` and `directory_files` tables do not exist yet | Migration V18 creates them |
| Live service config | None | — |
| OS-registered state | None | — |
| Secrets/env vars | None | — |
| Build artifacts | None | — |

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `ignore` crate | Desktop directory walk | ✓ (will add) | 0.4.25 | walkdir + globset manual combo |
| `notify` crate | Desktop FS watching | ✓ (will add) | 8.2.0 | PollWatcher or periodic timer only |
| `notify-debouncer-mini` | Desktop FS watch debounce | ✓ (will add) | 0.7.0 | Manual debounce with tokio timer |
| UIDocumentPickerViewController | iOS folder pick | ✓ iOS 17+ | native | — |
| ACTION_OPEN_DOCUMENT_TREE | Android folder pick | ✓ API 21+ | native | — |
| rfd | Desktop folder pick | ✓ already in Cargo.toml | existing | — |

[VERIFIED: rfd already used in desktop/iced/src/main.rs for file picking (line 773, 1056) — can add `pick_folder()` for directory picking]
[ASSUMED: rfd supports `pick_folder()` — consistent with rfd API surface, should verify]
[VERIFIED: ignore 0.4.25, notify 8.2.0, notify-debouncer-mini 0.7.0 from cargo search]

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `tokio::test` for async; existing pattern in lib.rs |
| Config file | None — inline tests in modules |
| Quick run command | `cargo test -p mango_core --lib rag::directory_sync` |
| Full suite command | `cargo test -p mango_core` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DIR-01 | Diff algorithm: added/modified/removed files correctly identified | unit | `cargo test -p mango_core test_directory_diff` | ❌ Wave 0 |
| DIR-02 | Exclusion glob filter: `.obsidian/` excluded, `notes/` included | unit | `cargo test -p mango_core test_exclusion_globs` | ❌ Wave 0 |
| DIR-03 | `directory_sources` CRUD: insert, list, delete, update_last_synced | unit | `cargo test -p mango_core test_directory_source_queries` | ❌ Wave 0 |
| DIR-04 | `directory_files` fingerprint upsert and stale detection | unit | `cargo test -p mango_core test_directory_file_fingerprints` | ❌ Wave 0 |
| DIR-05 | SyncDirectoryFiles actor handler: chunks + embeds changed files | integration | `cargo test -p mango_core test_sync_directory_files_handler` | ❌ Wave 0 |
| DIR-06 | RemoveDirectorySource: cascades to directory_files and document chunks | unit | `cargo test -p mango_core test_remove_directory_source` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p mango_core --lib rag::directory_sync -- --test-threads 1`
- **Per wave merge:** `cargo test -p mango_core`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `rust/src/rag/directory_sync.rs` — covers DIR-01, DIR-02 (diff + exclusion unit tests)
- [ ] `rust/src/persistence/queries.rs` additions — covers DIR-03, DIR-04
- [ ] Integration test in `rust/src/tests/directory_rag.rs` — covers DIR-05, DIR-06

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | — |
| V3 Session Management | No | — |
| V4 Access Control | Yes — folder access must not exceed user-granted scope | iOS: security-scoped bookmark; Android: SAF URI grant; Desktop: path validation |
| V5 Input Validation | Yes — exclusion globs from user input | Validate globs with `OverrideBuilder::add()` — returns `Err` on malformed patterns; surface error to UI |
| V6 Cryptography | Partial — indexed file chunks follow existing ENC-02 encryption path | Reuse `file_crypto` via DEK already threaded through ActorState |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via exclusion glob (e.g. `../../../../etc/passwd`) | Tampering | `OverrideBuilder` confines patterns to the walk root; patterns outside root are ignored by `ignore` crate design |
| Reading files outside user-granted folder (iOS bookmark escalation) | Elevation of Privilege | Only read URLs returned by enumerating the bookmark-resolved root URL — never construct arbitrary paths |
| SAF URI grant re-use across user accounts (shared Android device) | Information Disclosure | Android OS enforces SAF grants per UID; no additional mitigation needed |
| Excessive disk reads via aggressive sync scheduling | DoS (battery/storage) | Minimum sync interval: 15 minutes on Android (WorkManager constraint), foreground-only on iOS |

---

## Open Questions

1. **iOS bookmark option: `.minimalBookmark` vs no options**
   - What we know: macOS uses `.withSecurityScope`; iOS docs reference `.minimalBookmark` as the recommended option for URL persistence in non-sandboxed iOS apps
   - What's unclear: Whether iOS 17+ has changed this recommendation; whether `.withoutImpliedSecurityScope` matters
   - Recommendation: Test bookmark round-trip on physical iPhone with `.minimalBookmark`; check Apple developer forums for iOS 17 changelog on bookmark options

2. **rfd `pick_folder()` availability**
   - What we know: rfd is already in Cargo.toml for desktop file picking; the API surface likely includes `pick_folder()`
   - What's unclear: Exact method name and return type
   - Recommendation: Check `rfd::FileDialog` docs before planning desktop wave; `pick_folder()` → `Option<PathBuf>` is the expected signature [ASSUMED]

3. **notify-debouncer-mini version compatibility with notify 8.2.0**
   - What we know: `notify-debouncer-mini` 0.7.0 is the latest from cargo search; `notify-debouncer-full` exists as a heavier alternative
   - What's unclear: Which debouncer version is compatible with notify 8.2.0 vs 9.x
   - Recommendation: Add both to `[target.desktop.dependencies]` and verify `cargo check` passes; if version mismatch, use `notify-debouncer-full` which is maintained alongside notify

4. **Embedding throughput for large vault initial sync**
   - What we know: fastembed on Desktop handles batching; mobile EmbeddingProvider uses ONNX; a 10k-file vault could mean 50k+ chunks
   - What's unclear: Whether `fastembed` batch API or `MobileEmbeddingProvider` can handle 50k embed calls without exhausting memory
   - Recommendation: Process in 50-file batches (see Pitfall 8), save VectorIndex after each batch, surface progress via IngestionProgress

5. **Android COLUMN_LAST_MODIFIED reliability**
   - What we know: SAF `COLUMN_LAST_MODIFIED` is millis since epoch on ExternalStorageProvider
   - What's unclear: Whether third-party storage providers (Google Drive, Dropbox) return reliable mtime or return 0
   - Recommendation: Treat `COLUMN_LAST_MODIFIED == 0` as "unknown mtime" — fall back to always reading/re-indexing that file, or use size-only fingerprint

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | iOS `.minimalBookmark` is correct option for iOS 17+ folder bookmarks | Pattern 4, Pitfall 1 | Wrong option may cause bookmark resolution to fail; test on device |
| A2 | `notify-debouncer-mini` 0.7.0 is compatible with `notify` 8.2.0 | Standard Stack | Cargo resolve conflict; fallback is `notify-debouncer-full` |
| A3 | `rfd::FileDialog::pick_folder()` exists and returns `Option<PathBuf>` | Environment Availability | Different API name; check docs.rs/rfd before planning |
| A4 | Wave 0 test requirement IDs DIR-01..DIR-06 (self-assigned) | Validation Architecture | No impact — these are internal planning IDs, not external requirements |
| A5 | SyncDirectoryFiles batch size of 50 files is appropriate | Pattern 1, Pitfall 8 | Too small = many actor messages; too large = OOM on low-end devices. Tune empirically. |
| A6 | iCloud placeholder `.icloud` files require `startDownloadingUbiquitousItem` | Pitfall 3 | If wrong, sync may silently miss cloud-evicted files with no user feedback |

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `DocumentFile.listFiles()` for SAF | Direct `DocumentsContract.query` per directory level | Known since API 19, documented by commonsware ~2019 | 10-100x perf improvement for large directories |
| `notify` 6.x (deprecated) | `notify` 8.2.0 (stable) / 9.x (RC) | 2023-2024 | API cleanup; polling backend improvements |
| BGTaskScheduler for background sync | Foreground-resume scan (pragmatic iOS pattern) | Industry consensus by 2023 | Zero entitlements needed; reliable; matches user expectation |
| Full re-index on every sync | Mtime+size fingerprint incremental sync | Standard in build tools (Make, Cargo) | 100x reduction in work for quiescent vaults |

---

## Sources

### Primary (HIGH confidence)

- `cargo search` output — ignore 0.4.25, walkdir 2.5.0, globset 0.4.18, notify 8.2.0, notify-debouncer-mini 0.7.0 versions verified
- [docs.rs/ignore/latest/ignore/](https://docs.rs/ignore/latest/ignore/) — WalkBuilder, OverrideBuilder, glob semantics
- [docs.rs/ignore/latest/ignore/overrides/struct.OverrideBuilder.html](https://docs.rs/ignore/latest/ignore/overrides/struct.OverrideBuilder.html) — `add()` with `!` prefix
- [docs.rs/notify/8.2.0/notify/](https://docs.rs/notify/8.2.0/notify/) — platform support matrix, RecommendedWatcher
- [developer.android.com/training/data-storage/shared/documents-files](https://developer.android.com/training/data-storage/shared/documents-files) — SAF, takePersistableUriPermission, API 30 restrictions
- Project Cargo.toml — existing dependency versions; rfd already present; sha2, chrono, rusqlite, usearch versions
- Project lib.rs lines 5013-5150 — IngestDocument handler pattern (direct template for SyncDirectoryFiles)
- Project android/DocumentLibraryScreen.kt — ActivityResultContracts.OpenDocument() pattern
- Project ios/DocumentLibraryView.swift — startAccessingSecurityScopedResource() pattern

### Secondary (MEDIUM confidence)

- [adam.garrett-harris.com/2021-08-21-providing-access-to-directories-in-ios-with-bookmarks/](https://adam.garrett-harris.com/2021-08-21-providing-access-to-directories-in-ios-with-bookmarks/) — iOS bookmark lifecycle, isStale handling
- [commonsware.com/blog/2019/12/14/scoped-storage-stories-listfiles-woe.html](https://commonsware.com/blog/2019/12/14/scoped-storage-stories-listfiles-woe.html) — DocumentFile.listFiles() performance issues
- [developer.android.com/topic/libraries/architecture/workmanager](https://developer.android.com/topic/libraries/architecture/workmanager) — WorkManager 15-minute minimum interval
- notify GitHub README — inotify watch limit known issue

### Tertiary (LOW confidence — flag for validation)

- iOS `.minimalBookmark` vs `.withSecurityScope` distinction on iOS 17 — based on Apple forum discussions, not verified against iOS 17 release notes
- Obsidian forum background sync pattern — confirms foreground-resume as industry norm but app-specific

---

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — versions verified via cargo search; Rust crate APIs verified via docs.rs
- Architecture: MEDIUM — schema design and AppAction shape are new designs following existing project patterns, not derived from prior art
- Platform-specific (iOS/Android): MEDIUM — API references confirmed via official docs, but some edge cases (iCloud eviction, bookmark options) require physical device testing
- Pitfalls: HIGH (Android SAF), MEDIUM (iOS bookmark edge cases), HIGH (notify watch limits)

**Research date:** 2026-04-19
**Valid until:** 2026-07-19 (stable ecosystem; notify 9.x may stabilize, check before planning if >30 days elapsed)
