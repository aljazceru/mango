---
phase: 31
plan: 03
subsystem: android-ui
tags: [multimodal, images, android, camera, gallery, fileprovider]
requires:
  - 31-02 (UniFFI Kotlin bindings for AppAction.AttachImage and AttachmentInfo.isImage)
provides:
  - Android paperclip action sheet with Take Photo / Choose Photo / Attach File (D-6)
  - Android camera capture via FileProvider + ActivityResultContracts.TakePicture writing JPEG to cacheDir
  - Android gallery picking via ActivityResultContracts.PickVisualMedia copying image bytes to cacheDir
  - Compose bar image-pill rendering driven by AttachmentInfo.isImage
affects:
  - Plan 31-04 (iOS UI): independent — iOS gets the same action-sheet concept via PhotosPicker / UIImagePickerController
tech-stack:
  added: []
  patterns:
    - ActivityResultContracts.PickVisualMedia for privacy-preserving photo picker (no READ_MEDIA_IMAGES permission required on Android 13+)
    - FileProvider + TakePicture contract for camera capture — URI grants scoped via cache-path
    - Bytes copied into app-sandboxed cacheDir so the Rust actor sees a stable absolute path it can read at send-time
key-files:
  created:
    - android/app/src/main/res/xml/file_paths.xml
    - .planning/phases/31-multimodal-image-attachments-across-all-platforms-extend-rus/31-03-SUMMARY.md
  modified:
    - android/app/src/main/AndroidManifest.xml
    - android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/ComposeBar.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
decisions:
  - Used a Toast for camera-permission-denied surface rather than plumbing a Snackbar host through the Scaffold. No snackbar state exists on ChatScreen today; introducing one would require adding snackbarHost to the Scaffold, a SnackbarHostState, and a scope already exists. Toast is minimal, user-visible, and matches the platform idiom; upgrade to Snackbar is trivial if UX review demands it.
  - cacheDir file naming: `camera_<millis>.jpg` for camera captures, `img_<millis>.<ext>` for gallery imports. Uses System.currentTimeMillis() as a stable unique suffix — no collisions in practice, files are ephemeral (cacheDir is purged by the OS under pressure).
  - Gallery MIME normalization: only "image/png" passes through as PNG; everything else is treated as JPEG. Matches the Rust core's image crate JPEG/PNG allowlist from 31-01.
  - Plan called for ChatScreen to use snackbarHostState.showSnackbar for the permission-denied path. Deviated to Toast (see above). Logged here so 31-04 can mirror or diverge intentionally.
metrics:
  duration: ~12min
  tasks: 2 (auto) + 1 (checkpoint:human-verify auto-approved)
  files: 5
  completed: 2026-04-19
---

# Phase 31 Plan 03: Android Camera + Gallery Wiring Summary

**One-liner:** Wired the Android paperclip through a bottom-sheet action menu (Take Photo / Choose Photo / Attach File) with FileProvider-backed camera capture and PickVisualMedia gallery copy; both paths land JPEG/PNG bytes in cacheDir and dispatch `AppAction.AttachImage`, satisfying IMG-05 and IMG-06.

## Outcome

- `AndroidManifest.xml` declares `android.permission.CAMERA`, an optional `android.hardware.camera` feature, and a `FileProvider` with authority `${applicationId}.fileprovider`.
- `res/xml/file_paths.xml` scopes the FileProvider to `cache-path` (cacheDir) named `camera_captures`.
- `ChatScreen` gained:
  - `onAttachImage: (String, String, String) -> Unit` parameter
  - `galleryLauncher` using `ActivityResultContracts.PickVisualMedia` with `ImageOnly`
  - `cameraLauncher` using `ActivityResultContracts.TakePicture`
  - `cameraPermissionLauncher` using `ActivityResultContracts.RequestPermission`
  - `launchCamera()` helper that checks `ContextCompat.checkSelfPermission` and either launches or requests permission
  - `ModalBottomSheet` with three `TextButton` rows wired to the three launchers
- `ComposeBar` renders `Icons.Default.Image` instead of `Icons.Default.AttachFile` when `pendingAttachment.isImage == true`.
- `MainApp` dispatches `AppAction.AttachImage(filename, filePath, mimeType)` on the new `onAttachImage` callback.
- `./gradlew :app:compileDebugKotlin` — exit 0.
- `./gradlew :app:assembleDebug` — exit 0 (debug APK built).
- `./gradlew :app:processDebugManifest` — exit 0 (manifest merges without warnings about FileProvider/permission).

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Manifest + FileProvider + permissions | 8dd9a5c | android/app/src/main/AndroidManifest.xml, android/app/src/main/res/xml/file_paths.xml |
| 2 | Camera + gallery launchers, action sheet, dispatch AttachImage | 072933e | android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt, android/app/src/main/java/dev/disobey/mango/ui/ComposeBar.kt, android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt |
| 3 | Manual verification on device/emulator | auto-approved (auto-mode) | n/a — requires physical device |

## Verification Results

- `grep -c 'android.permission.CAMERA' AndroidManifest.xml` → **1** ≥ 1 ✓
- `grep -c 'androidx.core.content.FileProvider' AndroidManifest.xml` → **1** ≥ 1 ✓
- `grep -c '${applicationId}.fileprovider' AndroidManifest.xml` → **1** ≥ 1 ✓
- `file_paths.xml` exists with `<cache-path>` element ✓
- `grep -c 'PickVisualMedia' ChatScreen.kt` → **3** ≥ 1 ✓
- `grep -c 'TakePicture' ChatScreen.kt` → **1** ≥ 1 ✓
- `grep -c 'FileProvider.getUriForFile' ChatScreen.kt` → **1** ≥ 1 ✓
- `grep -c 'onAttachImage(' ChatScreen.kt` → **2** ≥ 1 ✓
- `grep -c 'isImage' ComposeBar.kt` → **1** ≥ 1 ✓
- `grep -c 'AttachImage' MainApp.kt` → **1** ≥ 1 ✓ (AppAction.AttachImage dispatched on the new callback)
- `./gradlew :app:processDebugManifest` → exit 0 ✓
- `./gradlew :app:compileDebugKotlin` → exit 0 ✓
- `./gradlew :app:assembleDebug` → exit 0 ✓

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Worktree was rooted on f40049a instead of the expected 2ad0681 base.**

- **Found during:** Preflight — `git merge-base HEAD 2ad0681` returned `f40049a`, which is the HEAD of the worktree branch and does NOT contain commits 31-01 (be6dc7c/3f6834c/5959d9a), 31-02 (a62fc32), or 31-05 (681fc9c/979c571). Soft-resetting to 2ad0681 would have staged reversals of those commits.
- **Fix:** `git reset --hard 2ad0681d9792c2fc159ec6c56eb12b7a5685211a` to align the worktree with the orchestrator's expected base, which is the merge commit on `main` that consolidates all prior 31-0x work. This gave a clean working tree with UniFFI bindings containing `AppAction.AttachImage` and `AttachmentInfo.isImage`, as required.
- **Files modified:** none (hard reset affected the index/worktree only).
- **Commit:** n/a — pre-task setup.

### Intentional Plan Diversions

**2. Toast instead of Snackbar for camera-permission-denied**

- **Plan text:** "dispatch a no-op or use local Snackbar" with `snackbarHostState.showSnackbar("Camera permission denied")`.
- **Rationale:** ChatScreen's Scaffold has no snackbarHost today and no SnackbarHostState is held. Introducing one touches the Scaffold structure and adds state not needed elsewhere. `Toast.makeText(context, "Camera permission denied", Toast.LENGTH_SHORT).show()` is a single-line, platform-idiomatic alternative.
- **Impact:** Denial still surfaces a user-visible message as required by the must-haves; no crash path; easy upgrade to Snackbar later if UX demands consistency.

### Auth Gates

None. No network or provider authentication is exercised in this plan.

## Threat Flags

None. The change adds:
- A CAMERA runtime permission behind an explicit user-triggered flow (first-tap-camera request).
- A FileProvider scoped to cacheDir with `exported="false"` and `grantUriPermissions="true"` — the URI grant is per-intent, consumed by the camera app for the TakePicture call.
- Two file writes to cacheDir (app sandbox) from bytes already controlled by the user (camera output or gallery pick).

No new network endpoints, no new auth paths, no schema changes, no cross-trust-boundary surface. The image validation (absolute path, MIME allowlist, 50 MB cap) remains in the Rust actor from 31-01 and is unchanged.

## Known Stubs

None. Both flows are fully wired end-to-end; the checkpoint would exercise the complete dispatch → Rust actor → multipart request path established by 31-01.

## Checkpoint Auto-Approval Note

Task 3 (`checkpoint:human-verify`) was auto-approved per the orchestrator's auto-mode setting. A physical Android device/emulator with camera hardware is required to exercise:
- Taking a photo → multipart vision request → model response describing the image
- Permission-denial Toast rendering
- Regression-check that text `Attach File` flow still works

The static verification above (grep counts + clean gradle builds including manifest merge, Kotlin compile, and full APK assembly) proves the code structurally satisfies the "how-to-verify" steps that do not need hardware. Runtime verification is deferred to the user or a later manual session.

## Self-Check: PASSED

- `android/app/src/main/AndroidManifest.xml` — FOUND and contains `android.permission.CAMERA` + `FileProvider`.
- `android/app/src/main/res/xml/file_paths.xml` — FOUND and contains `<cache-path>`.
- `android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt` — FOUND; contains `PickVisualMedia`, `TakePicture`, `FileProvider.getUriForFile`, `onAttachImage(`.
- `android/app/src/main/java/dev/disobey/mango/ui/ComposeBar.kt` — FOUND; contains `pendingAttachment.isImage`.
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` — FOUND; contains `AppAction.AttachImage` dispatch.
- Commit 8dd9a5c — FOUND on worktree-agent-a24ecd77.
- Commit 072933e — FOUND on worktree-agent-a24ecd77.
- `./gradlew :app:assembleDebug` — exit 0.
