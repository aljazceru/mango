---
status: resolved
trigger: "Investigate and fix UI issue: directory-sources-ui-polish"
created: 2026-04-20T00:00:00Z
updated: 2026-04-20T12:00:00Z
---

## Current Focus

hypothesis: Three independent layout/UX defects in DirectorySources UI on all three platforms
test: Code inspection of DirectorySourcesScreen.kt, DirectorySourcesView.swift, directory_sources.rs
expecting: Fix all three issues in the UI layer without touching Rust core
next_action: Apply fixes to Android, iOS, and Desktop

## Symptoms

expected: Directory Sources row renders cleanly with action buttons on a single line, shows the full folder path, and provides a way to open the folder in the native file browser.
actual: On Android the "Remove" button wraps to two lines due to button sizing; only folder display name ("test") is visible (not full path); no way to open the folder externally.
errors: None — visual/UX issue only.
reproduction: Open the app on Android, go to Settings → Directory Sources, add a folder via the "+" FAB. Observe the row layout in screenshot-folder2.png.
timeline: Introduced in Phase 32 (plans 04/05/06). Not yet fixed.

## Eliminated

- hypothesis: Rust core fields need updating
  evidence: DirectorySourceSummary intentionally omits path/tree_uri (T-32-I2). Path must be resolved in native layer from resolveTreeUri (Android), bookmark (iOS), or existing path field (Desktop). No core changes needed.
  timestamp: 2026-04-20T00:00:00Z

## Evidence

- timestamp: 2026-04-20T00:00:00Z
  checked: DirectorySourcesScreen.kt DirectorySourceRow composable (lines 261-283)
  found: Three OutlinedButton components in a Row with no weight modifiers. Default OutlinedButton has 16dp horizontal content padding + min width ~88dp. Three buttons in a 360dp-wide card = overflow → "Remove" wraps.
  implication: Switch to compact buttons (contentPadding reduced) or IconButton+Text layout.

- timestamp: 2026-04-20T00:00:00Z
  checked: DirectorySourceRow on all three platforms
  found: Only source.displayName is shown. The tree URI path (Android), bookmark-resolved path (iOS), and file-system path (Desktop) are not displayed. resolveTreeUri() in DirectorySyncWorker.kt returns the full content:// URI string. A human-readable path can be derived from DocumentsContract.getTreeDocumentId(uri).substringAfterLast(':') on Android.
  implication: For "Show full path" — on Android derive path segment from tree URI via resolveTreeUri+DocumentsContract; on iOS use source bookmark resolved URL last path components; on Desktop use AppState path field which is already stored (schema.rs line 319 shows path TEXT column). Must pass the path string down to the row.

- timestamp: 2026-04-20T00:00:00Z
  checked: resolveTreeUri in DirectorySyncWorker.kt and DirectorySourcePicker.kt
  found: resolveTreeUri returns the full content:// URI string. The document path portion is DocumentsContract.getTreeDocumentId(uri).substringAfterLast(':') — this gives e.g. "Download/test" which is the human-readable relative path. For "Open in Files", Android can fire Intent(Intent.ACTION_VIEW, uri) targeting DocumentsUI (com.google.android.documentsui or android.provider.Downloads); iOS can open URL("shareddocuments://...") or use the picker URL directly with UIApplication.openURL.
  implication: All three platforms can implement "Open" using a callback added to DirectorySourceRow.

- timestamp: 2026-04-20T00:00:00Z
  checked: Desktop directory_sources.rs build_source_row — src.path field access
  found: DirectorySourceSummary in Rust does NOT expose path in the UniFFI record (lib.rs lines 832-843). The Desktop path is stored in SQLite but not surfaced via the UniFFI boundary. However, desktop DirectorySourceSummary does have display_name. For desktop we can expose path in a new field OR just show display_name as the path (desktop display_name IS the directory name picked via rfd). Actually checking the AppAction::AddDirectorySource — path is passed as Option<String> for desktop. Need to check if it's stored and whether we need a new field.
  implication: Need to add path field to DirectorySourceSummary (Rust core change required for Desktop path display) OR find another way. Let me check if display_name is the full path on Desktop.

- timestamp: 2026-04-20T00:00:00Z
  checked: desktop/iced/src/views/directory_sources.rs Message::FolderPicked and how path is set
  found: FolderPicked(Option<PathBuf>) — the PathBuf IS the full path. In main.rs this will dispatch AddDirectorySource with path=Some(path_buf.to_string_lossy()). So path IS stored on Desktop. But DirectorySourceSummary doesn't expose it. Need to add path: Option<String> to the UniFFI record.
  implication: Add path: Option<String> to DirectorySourceSummary in Rust, populate it from the DB, expose it across UniFFI. Then use it on Desktop and iOS (iOS path is nil per T-32-I2 — use displayName as fallback). On Android path is also nil — use tree URI document path instead.

## Resolution

root_cause: Three separate issues: (1) OutlinedButton default min-touch-target padding (16dp horizontal + 40dp min height) caused three buttons to overflow a 360dp card, wrapping "Remove" to two lines; (2) DirectorySourceSummary.path was intentionally omitted from UniFFI record (T-32-I2 comment grouped path with opaque handles, but path is just a display string — safe to expose); (3) No "Open in files" action existed on any platform.
fix: |
  Rust core (lib.rs): Added `path: Option<String>` to DirectorySourceSummary UniFFI record; populated from `row.path` in load_directory_sources_summary.
  Generated bindings: Updated mango_core.kt (data class + FfiConverter) and mango_core.swift (struct + Equatable/Hashable + FfiConverter) to include the new field with FfiConverterOptionalString/FfiConverterOptionString.
  Android (DirectorySourcesScreen.kt): (1) Replaced default OutlinedButton padding with PaddingValues(horizontal=10dp, vertical=4dp) and labelSmall text + 14dp icons — fits 4 buttons on 360dp; (2) Derives displayPath from DocumentsContract.getTreeDocumentId().substringAfterLast(':') and shows it under display name; (3) Added "Open" OutlinedButton firing Intent.ACTION_VIEW with the tree URI.
  iOS (DirectorySourcesView.swift): (1) Added `path` display under displayName when non-nil; (2) Added "Open" button calling UIApplication.shared.open(URL("shareddocuments://")).
  Desktop (directory_sources.rs + main.rs): (1) Shows src.path under display name in build_source_row; (2) Added OpenFolder message + handler in main.rs using open::that(path).
verification: All 321 Rust tests pass. cargo check clean for both rust/ and desktop/iced/.
files_changed:
  - rust/src/lib.rs
  - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
  - ios/Bindings/mango_core.swift
  - android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt
  - ios/Mango/Mango/DirectorySourcesView.swift
  - desktop/iced/src/views/directory_sources.rs
  - desktop/iced/src/main.rs
