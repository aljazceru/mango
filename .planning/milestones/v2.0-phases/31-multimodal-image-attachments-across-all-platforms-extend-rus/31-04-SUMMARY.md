---
phase: 31
plan: 04
subsystem: ios-ui
tags: [multimodal, images, ios, swiftui, photospicker, uiimagepickercontroller]
requires:
  - 31-02 (AppAction.attachImage + AttachmentInfo.isImage exposed to Swift)
provides:
  - iOS paperclip now opens a 3-option action sheet: Take Photo / Choose Photo / Attach File
  - Camera path via UIImagePickerController (ImagePickerView UIViewControllerRepresentable)
  - Gallery path via SwiftUI PhotosPicker (iOS 16+)
  - Both image paths convert to JPEG in FileManager.temporaryDirectory and dispatch AppAction.attachImage(filename:filePath:mimeType:)
  - Pending attachment pill renders `photo` SF Symbol when attachment.isImage, `paperclip` otherwise
affects:
  - iOS user-facing attach flow (existing file-import path for .txt/.pdf unchanged)
tech-stack:
  added: []
  patterns:
    - PhotosPicker iOS 16+ used directly in ChatView (no UIViewControllerRepresentable needed for gallery)
    - UIImagePickerController wrapped in UIViewControllerRepresentable for camera (PhotosPicker doesn't cover camera source)
    - JPEG conversion in Swift via UIImage.jpegData(compressionQuality: 0.8) — handles HEIC natively
    - Temp file written to FileManager.temporaryDirectory; path is what Rust actor reads via AttachImage
    - ChatView gains @EnvironmentObject var appManager so it can dispatch .attachImage directly; ContentView adds `.environmentObject(appManager)` when presenting ChatView
key-files:
  created:
    - ios/Mango/Mango/ImagePickerView.swift
    - .planning/phases/31-multimodal-image-attachments-across-all-platforms-extend-rus/31-04-SUMMARY.md
  modified:
    - ios/Mango/Mango/Info.plist
    - ios/Mango/Mango/ChatView.swift
    - ios/Mango/Mango/ComposeBarView.swift
    - ios/Mango/Mango/ContentView.swift
decisions:
  - XcodeGen (`ios/Mango/project.yml`) defines `sources:` via path globs (`path: Mango`), so `ImagePickerView.swift` is auto-picked up on the next `xcodegen` run. No pbxproj edit was necessary (plan Task 1 step 3 was written defensively for legacy-group projects). Skipped the pbxproj mirroring check accordingly.
  - ChatView was using a pure callback pattern and did not hold an `AppManager` reference. The plan's `appManager.dispatch(.attachImage(...))` inside ChatView therefore required adding `@EnvironmentObject var appManager: AppManager` and wiring `.environmentObject(appManager)` at the single ChatView presentation site in ContentView. Preferred over threading yet another `onAttachImage: (String, String, String) -> Void` callback because ChatView already needs to present two pickers (camera sheet + PhotosPicker) with their own dismissal/state ownership; having appManager in-scope avoids a 3-hop callback chain for what is fundamentally a one-shot dispatch from the picker completion handler.
  - Gallery path passes a generic filename `"image.jpg"` (no stable asset name survives PhotosPickerItem.loadTransferable), mimeType `"image/jpeg"`. Camera path uses `"camera.jpg"`. Both are fine because the Rust actor ignores filename for disk layout — the provided filePath (random UUID in temp) is what matters. Humanised names survive only for UI pill display.
metrics:
  duration: ~15min
  tasks: 2 (Task 3 checkpoint auto-approved)
  files: 4 modified + 1 created + 1 summary
  completed: 2026-04-19
---

# Phase 31 Plan 04: iOS Camera + PhotosPicker Wiring Summary

**One-liner:** Paperclip on iOS now opens a `.confirmationDialog` with Take Photo / Choose Photo / Attach File; camera uses `UIImagePickerController` (via new `ImagePickerView` UIViewControllerRepresentable), gallery uses SwiftUI's `PhotosPicker`, both convert to JPEG in `FileManager.temporaryDirectory` and dispatch `AppAction.attachImage(filename:filePath:mimeType:)`, while the existing text/PDF file-importer path is preserved unchanged.

## Outcome

- `Info.plist` declares `NSCameraUsageDescription` and `NSPhotoLibraryUsageDescription` with confidentiality-oriented copy.
- New `ImagePickerView.swift` is a clean `UIViewControllerRepresentable` wrapping `UIImagePickerController(sourceType: .camera)`. Its coordinator writes JPEG data (via `UIImage.jpegData(compressionQuality: 0.8)`) to a UUID-named file in `temporaryDirectory` and invokes a typed `(filename, filePath, mimeType)` callback.
- `ChatView.swift` imports `PhotosUI`, gains `@EnvironmentObject var appManager`, tracks four new `@State` flags (`showAttachOptions`, `showCameraPicker`, `showPhotosPicker`, `photosPickerItem`). Paperclip now sets `showAttachOptions = true` instead of driving `showFilePicker` directly. A `.confirmationDialog` routes the three options, a `.photosPicker` (matching `.images`, `preferredItemEncoding: .compatible`) handles gallery, and a `.sheet(isPresented: $showCameraPicker)` presents `ImagePickerView`.
- Both image paths dispatch `appManager.dispatch(.attachImage(filename:filePath:mimeType:))` on the main actor.
- `ComposeBarView.swift` renders `Image(systemName: attachment.isImage ? "photo" : "paperclip")` for the pending-attachment pill.
- `ContentView.swift` adds `.environmentObject(appManager)` on the ChatView presentation so the new EnvironmentObject resolves.

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Info.plist usage strings + new ImagePickerView.swift | e58736e | ios/Mango/Mango/Info.plist, ios/Mango/Mango/ImagePickerView.swift |
| 2 | ChatView action sheet + PhotosPicker + camera sheet + AttachImage dispatch; ComposeBar image icon | 3db6d7a | ios/Mango/Mango/ChatView.swift, ios/Mango/Mango/ComposeBarView.swift, ios/Mango/Mango/ContentView.swift |
| 3 | Manual verification checkpoint | — | — (auto-approved in auto-mode; physical-device camera validation not run on Linux host) |

## Symbol Inventory

### `ios/Mango/Mango/ImagePickerView.swift` (new)

| Symbol | Kind |
|--------|------|
| `struct ImagePickerView: UIViewControllerRepresentable` | top-level type |
| `let onPicked: (String, String, String) -> Void` | stored property |
| `let onCancel: () -> Void` | stored property |
| `final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate` | nested type |

### `ios/Mango/Mango/ChatView.swift` additions

| Symbol | Kind |
|--------|------|
| `import PhotosUI` | import |
| `@EnvironmentObject var appManager: AppManager` | property |
| `@State private var showAttachOptions = false` | state |
| `@State private var showCameraPicker = false` | state |
| `@State private var showPhotosPicker = false` | state |
| `@State private var photosPickerItem: PhotosPickerItem? = nil` | state |
| `.confirmationDialog("Attach", ...)` | modifier |
| `.photosPicker(isPresented: $showPhotosPicker, selection: $photosPickerItem, matching: .images, preferredItemEncoding: .compatible)` | modifier |
| `.onChange(of: photosPickerItem) { ... }` | modifier (converts to JPEG, dispatches .attachImage) |
| `.sheet(isPresented: $showCameraPicker) { ImagePickerView(...) }` | modifier |

## Verification Results

Static grep acceptance (Task 2 `<acceptance_criteria>`):

- `grep -c 'PhotosPicker' ChatView.swift` → **4** ✓ (≥ 1)
- `grep -c 'ImagePickerView(' ChatView.swift` → **1** ✓ (≥ 1)
- `grep -c '.attachImage(' ChatView.swift` → **2** ✓ (≥ 2 — camera + gallery paths)
- `grep -c 'confirmationDialog' ChatView.swift` → **1** ✓ (≥ 1)
- `grep -c 'isImage' ComposeBarView.swift` → **1** ✓ (≥ 1)
- `grep -q NSCameraUsageDescription Info.plist` → **found** ✓
- `grep -q NSPhotoLibraryUsageDescription Info.plist` → **found** ✓
- `test -f ImagePickerView.swift` → **exists** ✓
- `grep -q UIImagePickerController ImagePickerView.swift` → **found** ✓

xcodebuild and `plutil -lint` were not run: the execution host is Linux (see Deviations). The plist is hand-authored, valid XML, and matches Apple's usage-description format exactly; `xcodegen`-generated targets will regenerate the project referencing `ImagePickerView.swift` automatically via the path-glob `sources:` entry in `project.yml`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 – Blocking] ChatView had no `AppManager` reference**

- **Found during:** Task 2, while implementing the photosPicker `onChange` handler that must call `appManager.dispatch(.attachImage(...))`.
- **Issue:** ChatView was designed as a pure view with callback injection (`onSend`, `onRetry`, etc.). The plan's code blocks inline `appManager.dispatch(.attachImage(...))` assuming an in-scope `appManager`, which did not exist.
- **Fix:** Added `@EnvironmentObject var appManager: AppManager` to ChatView; added `.environmentObject(appManager)` to the single ChatView presentation in `ContentView.swift` (`case .chat(let conversationId):` branch). Preserves existing callback APIs untouched; adds capability-bridge access matching the pattern already used by `OnboardingView`, `SettingsView`, `DocumentLibraryView`, etc.
- **Files modified:** ios/Mango/Mango/ChatView.swift, ios/Mango/Mango/ContentView.swift.
- **Commit:** 3db6d7a.

**2. [Rule 3 – Environment] plist sanity and simulator build skipped on Linux host**

- **Found during:** Task 1 and Task 2 verification steps.
- **Issue:** Task 1 `<verify>` runs `plutil -lint Info.plist`; Task 2 `<verify>` runs `xcodebuild -scheme Mango ... build`. Neither command exists on Linux, which is the execution host. The plan assumes a macOS host.
- **Fix:** Confirmed XML validity by inspection (well-formed `<plist version="1.0">`, two `<key>/<string>` pairs appended before closing `</dict>`). Confirmed Swift syntax by inspection against the bindings (`ios/Bindings/mango_core.swift:3109`) which shows `case attachImage(filename: String, filePath: String, mimeType: String)` matching the call sites verbatim. Swift 5.9 simulator build will be validated in the subsequent human-verify checkpoint when the developer pulls the branch on a Mac.
- **Files modified:** none (verification-only adjustment).
- **Commit:** n/a.

### Plan Steps Intentionally Skipped

- Plan Task 1 step 3 ("pbxproj edit mirroring BiometricProviderImpl.swift"): `ios/Mango/` uses XcodeGen (`project.yml`) instead of a committed `.xcodeproj`. The `sources: [- path: Mango]` entry recursively globs `.swift` files, so adding `ImagePickerView.swift` to the on-disk directory is sufficient — XcodeGen will register it on the next `xcodegen generate` run. This matches the plan's conditional guidance ("If `project.pbxproj` does NOT use filesystem-synced groups...") — here there is no `project.pbxproj` at all.

### Auth Gates

None. No network calls or credentials involved.

## Threat Flags

None. The new code writes JPEGs to the app's `temporaryDirectory` sandbox (already-accessible app-private storage) and hands a local path to the Rust actor via an already-established typed action (`AppAction.attachImage`) whose validation lives in Rust (landed in 31-01: absolute-path check, MIME allowlist, 50 MB cap). No new network endpoints, no new cross-process boundaries, no new schema surface, no new auth paths. Camera and photo-library access are gated by Info.plist usage strings which iOS surfaces at the OS permission prompt.

## Known Stubs

None. Every added state has a real wire-up:
- `showAttachOptions` drives `.confirmationDialog`
- `showPhotosPicker` drives `.photosPicker`
- `showCameraPicker` drives `.sheet`
- `photosPickerItem` drives `.onChange` → JPEG conversion → dispatch
- `ImagePickerView.onPicked` dispatches `.attachImage`
- `ImagePickerView.onCancel` dismisses sheet

The physical-device camera test (Task 3) is deferred to the developer's next Mac/device session — not a stub, just not runnable from this execution host.

## Self-Check: PASSED

- `ios/Mango/Mango/Info.plist` contains `NSCameraUsageDescription` — FOUND.
- `ios/Mango/Mango/Info.plist` contains `NSPhotoLibraryUsageDescription` — FOUND.
- `ios/Mango/Mango/ImagePickerView.swift` exists and contains `UIImagePickerController` + `UIViewControllerRepresentable` — FOUND.
- `ios/Mango/Mango/ChatView.swift` contains `import PhotosUI`, `PhotosPicker`, `ImagePickerView(`, `.attachImage(`, `confirmationDialog`, `@EnvironmentObject var appManager` — FOUND.
- `ios/Mango/Mango/ComposeBarView.swift` contains `attachment.isImage ? "photo" : "paperclip"` — FOUND.
- `ios/Mango/Mango/ContentView.swift` ChatView call site ends with `.environmentObject(appManager)` — FOUND.
- Commit `e58736e` — FOUND on HEAD (`feat(31-04): add iOS camera/photo usage strings + ImagePickerView`).
- Commit `3db6d7a` — FOUND on HEAD (`feat(31-04): wire iOS paperclip action sheet + PhotosPicker + camera`).
