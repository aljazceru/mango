---
status: resolved
trigger: "Android SAF folder picker shows 'Can't use this folder / To protect your privacy, choose another folder' when user tries to add a directory source on Pixel 9a running GrapheneOS."
created: 2026-04-20T00:00:00Z
updated: 2026-04-20T12:00:00Z
---

## Current Focus

hypothesis: CONFIRMED — Two distinct bugs found. (1) The picker launches with `launcher.launch(null)`, passing null as the initial URI to OpenDocumentTree. This causes DocumentsUI to open at the internal-storage root ("Pixel 9a"). On Android 11+ (API 30+), ANY folder selected via navigation FROM that root is rejected with "Can't use this folder". (2) The flag passed to `takePersistableUriPermission` is `FLAG_GRANT_READ_URI_PERMISSION` only, but OpenDocumentTree's default contract grants both read AND write. Omitting `FLAG_GRANT_WRITE_URI_PERMISSION` causes the permission take to silently capture only read, which may cause later query failures on some providers/versions (secondary issue).
test: Confirmed by reading DirectorySourcePicker.kt line 73: `return { launcher.launch(null) }` — null initial URI causes DocumentsUI to start at the restricted internal storage root. Standard fix is to pass `MediaStore.Images.Media.EXTERNAL_CONTENT_URI` or any non-restricted URI as the initial hint, or use `Environment.getExternalStoragePublicDirectory` as a starting point.
expecting: Fix: pass a non-restricted EXTRA_INITIAL_URI so DocumentsUI starts in a safe location (e.g., the Downloads public dir or the app's external files dir), not the internal storage root.
next_action: DONE — user confirmed fixed on Pixel 9a GrapheneOS device

## Symptoms

expected: User picks a folder in the SAF picker → app adds it as a directory source for RAG ingestion → sync begins
actual: SAF picker shows "Can't use this folder / To protect your privacy, choose another folder" banner with "Create new folder" option but "Use this folder" button is greyed out/disabled. User navigated to Pixel 9a > test > test (a user-created folder on internal storage).
errors: "Can't use this folder / To protect your privacy, choose another folder" — Android DocumentsUI built-in error, NOT an app-level error
reproduction: Open app → go to Directory Sources → tap Add folder → navigate to any folder on device → see error
started: Reported during Phase 32 RAG directory sync feature implementation
platform: Android (GrapheneOS), Pixel 9a

## Eliminated

(none yet)

## Evidence

- timestamp: 2026-04-20T00:01:00Z
  checked: DirectorySourcePicker.kt line 73
  found: `return { launcher.launch(null) }` — null is passed as the initial URI to the OpenDocumentTree contract launcher
  implication: DocumentsUI receives no EXTRA_INITIAL_URI hint and defaults to opening at the internal storage root ("Pixel 9a" in GrapheneOS). Android 11+ (API 30+) DocumentsUI treats any folder selected via navigation FROM the internal storage root as restricted and shows "Can't use this folder / To protect your privacy, choose another folder". This is the direct cause of the reported error.

- timestamp: 2026-04-20T00:01:00Z
  checked: DirectorySourcePicker.kt lines 56-61 (takePersistableUriPermission call)
  found: `val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION` — only read flag is captured; OpenDocumentTree by default grants both read+write, but the permission take only persists read
  implication: Secondary issue — not the cause of the "Can't use this folder" banner, but means write permission is abandoned. For a read-only RAG indexer this is acceptable short-term, but `ContentResolver.query()` on some ROMs (including GrapheneOS) may still require the write flag be taken to maintain the grant. Low priority vs. the primary bug.

- timestamp: 2026-04-20T00:01:00Z
  checked: AndroidManifest.xml
  found: No READ_EXTERNAL_STORAGE, MANAGE_EXTERNAL_STORAGE, or any storage permissions declared. Only INTERNET and CAMERA.
  implication: Correct for API 33+ SAF usage — SAF does not require manifest storage permissions. Not a contributing factor.

- timestamp: 2026-04-20T00:01:00Z
  checked: Android 11+ DocumentsUI source / known behavior
  found: DocumentsUI blocks folder selection when the user navigates to a folder via the internal storage root ("primary" volume root). The restriction triggers even on subdirectories that are not themselves restricted. The fix is to set EXTRA_INITIAL_URI to a non-root location (e.g., Downloads public dir) so the user starts inside a safe subtree, OR to use MediaStore.Downloads URI as the seed.
  implication: Passing a non-restricted initial URI via `launcher.launch(uri)` instead of `launcher.launch(null)` bypasses the root-navigation restriction. The user can still navigate to any accessible folder from that starting point without triggering the banner.

## Resolution

root_cause: DirectorySourcePicker.kt calls `launcher.launch(null)`, passing null as the initial URI to the OpenDocumentTree contract. This causes Android DocumentsUI to open at the internal storage root ("Pixel 9a" on GrapheneOS). Android 11+ (API 30+) blocks selecting ANY folder reached by navigating from the internal storage root, showing "Can't use this folder / To protect your privacy, choose another folder". The fix is to pass a non-restricted initial URI (e.g., MediaStore.Downloads or the app's external files dir) so DocumentsUI opens inside a permitted subtree.
fix: In rememberDirectoryPicker, build an initial URI pointing to the public Downloads directory using MediaStore.Downloads.EXTERNAL_CONTENT_URI (API 29+) and pass it to launcher.launch(). Add a null-safe fallback for older APIs. Also add FLAG_GRANT_WRITE_URI_PERMISSION to the takePersistableUriPermission call to correctly persist both flags matching what OpenDocumentTree grants.
verification: CONFIRMED by user on Pixel 9a running GrapheneOS. Folder picker now works correctly — "Can't use this folder" banner no longer appears. Fix verified on device 2026-04-20.
files_changed: [android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt]
