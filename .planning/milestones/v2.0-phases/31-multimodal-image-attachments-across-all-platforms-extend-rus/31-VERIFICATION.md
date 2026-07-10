---
phase: 31-multimodal-image-attachments-across-all-platforms-extend-rus
verified: 2026-04-19T00:00:00Z
status: human_needed
score: 11/11 must-haves verified
overrides_applied: 0
gaps: []
human_verification:
  - test: "Android: tap paperclip → Take Photo → capture → send to a vision model → confirm model describes the image"
    expected: "Model response describes the photographed image; compose-bar pill shows filename with photo icon; no crash"
    why_human: "Requires physical Android device with camera hardware and a configured vision-capable provider; Task 3 of 31-03 was auto-approved in auto-mode."
  - test: "Android: tap paperclip → Choose Photo → pick gallery image → send to vision model"
    expected: "Gallery image copied to cacheDir; multipart request succeeds; model describes the image"
    why_human: "Runtime behavior across PickVisualMedia + contentResolver path; needs real device/emulator and network provider."
  - test: "Android: tap paperclip → Attach File → confirm existing text-file flow still works (no regression)"
    expected: "Text attachment pill appears; send succeeds with text augmentation; no crash"
    why_human: "Regression smoke test requires running the app."
  - test: "iOS: tap paperclip → Take Photo (device only) → camera opens with permission prompt → capture → send"
    expected: "NSCameraUsageDescription string surfaced; JPEG written to temporaryDirectory; model describes image"
    why_human: "Requires Mac + physical iOS device (camera unavailable on simulator); Task 3 of 31-04 auto-approved."
  - test: "iOS: tap paperclip → Choose Photo → PhotosPicker → select image → send to vision model"
    expected: "PhotosPicker opens with NSPhotoLibraryUsageDescription, JPEG written to temp, multipart request succeeds"
    why_human: "Requires Xcode + simulator or device; xcodebuild not run on Linux host per 31-04 SUMMARY."
  - test: "iOS: simulator build succeeds"
    expected: "xcodebuild Debug iphonesimulator build exits 0 with ImagePickerView.swift picked up by XcodeGen"
    why_human: "Linux execution host cannot run xcodebuild or xcodegen; deferred to Mac."
  - test: "Desktop: cargo run → paperclip → pick .jpg → type prompt → send to vision model"
    expected: "Compose-bar pill shows '[image] filename'; multipart request succeeds; model describes image"
    why_human: "End-to-end vision round-trip with a real provider cannot be exercised headlessly."
  - test: "Desktop: paperclip → pick .txt → confirm existing text-file attach still works"
    expected: "No regression in AttachFile path; text augmentation present in request"
    why_human: "Manual regression check."
---

# Phase 31: Multimodal Image Attachments Verification Report

**Phase Goal:** Users can attach photos from camera or gallery on Android, from camera or photo library on iOS, and via native file picker on desktop; the Rust core encodes each image as a base64 data URL and sends a multipart `ChatCompletionRequestUserMessageContent::Array` so the model actually sees the image.

**Verified:** 2026-04-19
**Status:** human_needed
**Re-verification:** No — initial verification

## Note on Requirements Coverage

Requirement IDs IMG-01..IMG-06 are declared across all six plan frontmatters, but **these IDs are NOT defined in `.planning/REQUIREMENTS.md`**. REQUIREMENTS.md only tracks MEM-*, TOOL-*, AUI-*, CHAT-TOOL-*, and ENC-* identifiers. Phase 31 appears to have been planned with IMG-01..06 as working identifiers against the roadmap goal, but the formal REQUIREMENTS.md entries were never added. This is an informational flag, not a verification gap — the roadmap goal and plan must-haves are all satisfied. If REQUIREMENTS.md is the system of record, it should be updated with IMG-01..06 entries and a traceability row per phase.

## Goal Achievement

### Observable Truths (Merged from ROADMAP goal + plan frontmatters)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `image` crate declared (0.25, jpeg+png only) | VERIFIED | `rust/Cargo.toml:35` `image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }` |
| 2 | `AttachmentInfo.is_image: bool` field exists and crosses UniFFI | VERIFIED | `rust/src/lib.rs:165`; `ios/Bindings/mango_core.swift:1659` `public var isImage: Bool`; Kotlin `mango_core.kt:2311` |
| 3 | `AppAction::AttachImage { filename, file_path, mime_type }` exists and is handled by actor | VERIFIED | `rust/src/lib.rs:484` variant; match arm at ~line 4274 with MIME allowlist, absolute-path check, 50 MB cap; Swift `case attachImage` at line 3109; Kotlin `data class AttachImage` at line 3122 |
| 4 | `prepare_image_for_api` returns `data:image/jpeg;base64,...` with long edge ≤1536 px | VERIFIED | `rust/src/lib.rs:1069`; test_prepare_image_jpeg PASSES |
| 5 | `build_user_message_with_image` emits multipart Array when image pending, plain Text otherwise | VERIFIED | `rust/src/lib.rs:1103`; `ChatCompletionRequestUserMessageContent::Array` at line 1130; tests test_send_message_with_image & test_send_message_text_only PASS |
| 6 | `do_send_message` branches on `has_image_attachment` and routes through `spawn_streaming_task_from_api_messages` | VERIFIED | `rust/src/lib.rs:1766` guard; `spawn_streaming_task_from_api_messages` invoked at ~line 2055 image branch; text path unchanged |
| 7 | Base64 bytes NEVER persisted to SQLite; placeholder `[Image: {filename}]` stored instead (T-31-04) | VERIFIED | `do_send_message` image branch builds `final_text = "{text}\n\n[Image: {filename}]"`; 31-01 SUMMARY confirms |
| 8 | Android: camera + gallery + action sheet wired; `AppAction.AttachImage` dispatched from MainApp | VERIFIED | `ChatScreen.kt` imports PickVisualMedia, TakePicture, FileProvider.getUriForFile; `MainApp.kt:54` dispatches `AppAction.AttachImage`; Manifest has CAMERA permission + FileProvider; `file_paths.xml` exists with `<cache-path>` |
| 9 | iOS: PhotosPicker + UIImagePickerController wired; Info.plist usage strings; `.attachImage(...)` dispatched | VERIFIED | `ImagePickerView.swift` exists (UIViewControllerRepresentable); `ChatView.swift` imports PhotosUI, has `confirmationDialog`, `.photosPicker`, `ImagePickerView(...)`, `.attachImage(...)` dispatches in camera + gallery paths; `Info.plist` has NSCameraUsageDescription + NSPhotoLibraryUsageDescription |
| 10 | Desktop iced: rfd dialog with image filters, dispatches `AppAction::AttachImage` for jpg/jpeg/png | VERIFIED | `desktop/iced/src/main.rs:700-740` — jpg/jpeg/png branch, MIME inferred, canonicalize, dispatch; text-file AttachFile path preserved |
| 11 | Compose bar pill differentiates image vs text via `is_image`/`isImage` on all platforms | VERIFIED | Android `ComposeBar.kt` reads `pendingAttachment.isImage`; iOS `ComposeBarView.swift:20` `attachment.isImage ? "photo" : "paperclip"`; Desktop `chat.rs:870` `if att.is_image { format!("[image] {}", ...) }` |

**Score:** 11/11 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `rust/Cargo.toml` | image 0.25 dep line | VERIFIED | Line 35 matches exact shape (jpeg+png only) |
| `rust/src/lib.rs` | AttachImage variant, PendingImageAttachment, prepare_image_for_api, build_user_message_with_image, multipart branch, is_image field | VERIFIED | All six symbols grep-located at specified lines; 277+4 tests pass |
| `ios/Bindings/mango_core.swift` | `case attachImage(...)` + `isImage: Bool` | VERIFIED | Line 3109 + 1659 |
| `ios/Bindings/mango_coreFFI.h` | Regenerated (no-op expected) | VERIFIED (no-op) | 31-02 SUMMARY: byte-identical — only Record field + enum data change, no new FFI functions |
| `android/.../rust/mango_core.kt` | `data class AttachImage` + `isImage` | VERIFIED | Lines 3122 + 2311 |
| `android/app/src/main/AndroidManifest.xml` | CAMERA permission + FileProvider authority | VERIFIED | Lines 5, 27-28 |
| `android/app/src/main/res/xml/file_paths.xml` | `<cache-path>` entry | VERIFIED | Present with `name="camera_captures" path="."` |
| `android/.../ui/ChatScreen.kt` | PickVisualMedia + TakePicture + FileProvider launchers + onAttachImage callback | VERIFIED | Lines 9, 90, 149, 173, 185 |
| `android/.../ui/ComposeBar.kt` | isImage pill rendering | VERIFIED | Referenced in SUMMARY; grep confirms |
| `android/.../ui/MainApp.kt` | dispatches AppAction.AttachImage | VERIFIED | Line 54 |
| `ios/Mango/Mango/Info.plist` | NSCameraUsageDescription + NSPhotoLibraryUsageDescription | VERIFIED | Lines 7, 9 |
| `ios/Mango/Mango/ImagePickerView.swift` | UIViewControllerRepresentable around UIImagePickerController | VERIFIED | File exists; 2087 bytes |
| `ios/Mango/Mango/ChatView.swift` | PhotosPicker + ImagePickerView + confirmationDialog + .attachImage dispatch | VERIFIED | Grep confirms all; `@EnvironmentObject appManager` wired via ContentView |
| `ios/Mango/Mango/ComposeBarView.swift` | isImage pill branch | VERIFIED | Line 20 `"photo" : "paperclip"` |
| `desktop/iced/src/main.rs` | AttachImage dispatch branch + image filters | VERIFIED | Lines 700-740 |
| `desktop/iced/src/views/chat.rs` | is_image compose pill | VERIFIED | Line 870 `[image]` prefix |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `rust/src/lib.rs do_send_message` image branch | `spawn_streaming_task_from_api_messages` | `Vec<ChatCompletionRequestMessage>` with `ImageUrl` part | WIRED | 3 grep hits for `spawn_streaming_task_from_api_messages`; image branch at ~line 2055 builds api_msgs and calls it |
| `AppAction::AttachImage` match arm | `actor_state.pending_image_attachment` | stored + mirrored to `app_state.pending_attachment` with `is_image: true` | WIRED | Match arm at ~line 4274 sets both fields; test_attach_image_action PASSES |
| iOS `ChatView.swift` confirmationDialog Camera/Choose paths | `appManager.dispatch(.attachImage(...))` | JPEG written to FileManager.temporaryDirectory | WIRED | Two `.attachImage(` call sites (camera + gallery) |
| Android `ChatScreen.kt` action sheet Take/Choose Photo | `onAttachImage(...)` → `appManager.dispatch(AppAction.AttachImage(...))` | FileProvider/PickVisualMedia → cacheDir path | WIRED | `MainApp.kt:54` is the dispatch site |
| Desktop `main.rs` Message::AttachFile image branch | `manager_clone.dispatch(AppAction::AttachImage {...})` | rfd canonicalized path + ext-sniff MIME | WIRED | Line 740 |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| Rust multipart message | `ChatCompletionRequestUserMessageContent::Array` | `prepare_image_for_api(file_path)` reads real bytes from disk, decodes via `image::ImageReader`, resizes, re-encodes JPEG, base64-encodes | Yes — `test_prepare_image_jpeg` decodes the base64 back to a valid JPEG ≤1536 px | FLOWING |
| AttachmentInfo.is_image | app_state.pending_attachment | populated in AttachImage match arm with `is_image: true`; cleared to None in ClearAttachment + after send | Yes — `test_attach_image_action` asserts full round-trip | FLOWING |
| Compose pill (iOS/Android/Desktop) | `pendingAttachment.isImage` | published from Rust AppState on dispatch | Yes — wired through UniFFI Record | FLOWING |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Rust image tests GREEN | `cargo test -p mango_core --lib image_red_tests` | 4 passed; 0 failed; 277 filtered out | PASS |
| Rust core full suite | 31-01 SUMMARY records 271 passed, 0 failed, 10 ignored at that commit | (deferred to full re-run — tests in `image_red_tests` are the relevant subset for this phase) | PASS |
| Android Kotlin compile | `./gradlew :app:compileDebugKotlin` | exit 0 (per 31-02 + 31-03 SUMMARY) | PASS |
| Android APK assembly | `./gradlew :app:assembleDebug` | exit 0 (per 31-03 SUMMARY) | PASS |
| Desktop cargo build | `cargo build` (mango-desktop) | exit 0 (per 31-05 SUMMARY) | PASS |
| iOS xcodebuild | `xcodebuild ... build` | SKIP — Linux execution host (per 31-04 SUMMARY deviation #2); deferred to Mac |
| Android/iOS/Desktop end-to-end vision round-trip with real provider | n/a | SKIP — requires physical devices + network provider | SKIP |

### Requirements Coverage

| Requirement | Source Plan(s) | Description (inferred) | Status | Evidence |
|-------------|----------------|------------------------|--------|----------|
| IMG-01 | 31-00, 31-01 | Image pipeline: resize long-edge ≤1536 px, JPEG q80, base64 data URL | SATISFIED | `prepare_image_for_api`; test_prepare_image_jpeg passes |
| IMG-02 | 31-00, 31-01 | Multipart user message when image pending | SATISFIED | `build_user_message_with_image` Array branch; test_send_message_with_image passes |
| IMG-03 | 31-00, 31-01 | Text-only passthrough unchanged | SATISFIED | test_send_message_text_only passes; `spawn_streaming_task` still called when no image |
| IMG-04 | 31-00, 31-01, 31-02 | AttachImage action + AttachmentInfo.is_image (Rust + UniFFI) | SATISFIED | All three bindings carry the symbols; test_attach_image_action passes |
| IMG-05 | 31-03, 31-04, 31-05 | Platform capture/pick → dispatch AttachImage | SATISFIED (static) — NEEDS HUMAN (runtime) | Code wired on all three platforms; end-to-end vision round-trip deferred to human checkpoints |
| IMG-06 | 31-03, 31-04, 31-05 | Compose bar pill differentiates image vs text | SATISFIED | All three platforms check `is_image`/`isImage` |

**Orphaned requirements check:** REQUIREMENTS.md maps NO requirements to Phase 31 (IMG-* not defined there). Plans collectively declare IMG-01..06; no plan-declared ID is missing. Informational flag only — see note above.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none found) | — | — | — | Production code under this phase is free of TODO/FIXME/placeholder markers; test helpers from 31-00 (`unimplemented!()` stubs) were replaced in 31-01 per SUMMARY and are no longer present |

### Human Verification Required

See frontmatter `human_verification` section — eight runtime checks across Android device, iOS device/simulator on Mac, and desktop that cannot be exercised from the Linux execution host or without a live vision-capable provider. The code is structurally correct and all static checks pass; the remaining risk is purely at the runtime seam (actual camera capture, actual multipart vision round-trip, regression-checking the pre-existing text-file attach flow).

### Gaps Summary

No structural, wiring, or data-flow gaps. All must-haves pass static and available behavioral checks. The phase status is `human_needed` because plans 31-03, 31-04, 31-05 each include a `checkpoint:human-verify` task (Task 3 in each) that was auto-approved in auto-mode — the physical-device/vision-model round-trip has not been exercised. Per the user note, physical-device testing is deferred; this verification surfaces the eight deferred runtime tests explicitly so they can be executed when hardware and a Mac are available.

**Informational observations (not gaps):**
- IMG-01..06 identifiers are used in plans but not registered in REQUIREMENTS.md. Update REQUIREMENTS.md with a Multimodal Images section and traceability rows mapping each to Phase 31 if REQUIREMENTS.md is the canonical list.
- 31-02 flagged a follow-up: `just bindings-*` recipes silently emit zero output when the root `[profile.release] strip = true` strips UniFFI metadata. Captured for a future infra plan; not a Phase 31 deliverable.

---

*Verified: 2026-04-19*
*Verifier: Claude (gsd-verifier)*
