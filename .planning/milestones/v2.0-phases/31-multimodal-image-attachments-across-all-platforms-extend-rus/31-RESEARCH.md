# Phase 31: Multimodal Image Attachments — Research

**Researched:** 2026-04-18
**Domain:** Multimodal chat (image_url content parts), platform camera/gallery APIs, Rust image processing
**Confidence:** HIGH (all key claims verified against source code or registry)

---

## Summary

Phase 31 extends the existing text-file attachment pipeline to carry images. The Rust core needs a
richer `PendingAttachment` / `AttachmentInfo` model, a new `AttachFile` action variant or a second
`AttachImage` action, and the `do_send_message` build-path must emit
`ChatCompletionRequestUserMessageContent::Array` with `ImageUrl` content parts instead of the
current plain-string prefix. All three platforms already have an attach button wired to a handler —
each just needs that handler replaced with a camera/gallery picker that dispatches image bytes or a
file path instead of text content.

**Primary recommendation:** Keep the existing text-attachment flow untouched. Add a parallel
`AttachImage { filename, file_path, mime_type }` action and a matching `PendingImageAttachment`
struct in `ActorState`. At request-build time the actor reads the file, JPEG-compresses/resizes it,
base64-encodes it, and emits a multipart user message. This keeps raw bytes off the UniFFI boundary
and respects the RMP architecture — Rust owns all I/O and protocol logic.

---

## Project Constraints (from CLAUDE.md)

- RMP Architecture: Rust core owns all business logic; native layers are thin UI + capability bridges only.
- async-openai 0.34 is in use (verified: `Cargo.lock` / `cargo metadata`). [VERIFIED: cargo metadata]
- No OpenSSL; rustls only. The `image` crate is pure Rust and safe to add.
- UniFFI 0.29 — bindings regenerated via `just bindings-kotlin` / `just bindings-swift`; `ios/Bindings/` committed to repo.
- Privacy: all data stays on-device; no telemetry, no cloud upload of images.
- Android minSdk 28, compileSdk 36. [VERIFIED: build.gradle.kts]
- iOS 17+ (Info.plist exists; no camera/photo usage strings present yet). [VERIFIED: source]
- Desktop: iced + rfd 0.15. [VERIFIED: desktop/iced/Cargo.toml]

---

## Standard Stack

### Core (Rust)

| Library | Version | Purpose | Source |
|---------|---------|---------|--------|
| `async-openai` | 0.34.0 | `ChatCompletionRequestUserMessageContent::Array` with `ImageUrl` parts | [VERIFIED: cargo metadata, source] |
| `image` | 0.25.x | Decode JPEG/PNG/HEIC-converted input, resize to max dimension, re-encode as JPEG | [ASSUMED — not yet in Cargo.toml] |
| `base64` | 0.22.x | Already present; used for data-URL encoding | [VERIFIED: rust/Cargo.toml] |

**`image` crate note:** Pure Rust; no system deps; compiles cleanly on iOS/Android/Linux. Default
features include `jpeg` and `png`. HEIC is not natively supported — iOS must convert HEIC to JPEG
before handing the path to Rust. [ASSUMED — based on image crate 0.25 feature list, not verified
via Context7 this session]

### Platform

| Layer | Mechanism | Already present? |
|-------|-----------|-----------------|
| Android camera | `ActivityResultContracts.TakePicture` (writes to FileProvider URI) | No — needs FileProvider xml + CAMERA permission |
| Android gallery | `ActivityResultContracts.PickVisualMedia` (API 21+ via Activity 1.7+) | `activity-compose:1.10.1` already dep — covers it |
| iOS camera | `UIImagePickerController` via `UIViewControllerRepresentable` | No — needs NSCameraUsageDescription in Info.plist |
| iOS library | `PhotosPicker` (iOS 16+) | No — needs NSPhotoLibraryUsageDescription |
| Desktop | `rfd::FileDialog::new().add_filter(...)` | `rfd = "0.15"` already dep |

---

## Architecture Patterns

### Recommended Data Flow

```
Native UI picks image
  → converts HEIC→JPEG (iOS only, in Swift)
  → writes to temp file in app sandbox
  → dispatches AppAction::AttachImage { filename, file_path, mime_type }
       (file_path is an absolute path to the temp file)
Actor receives AttachImage
  → stores PendingImageAttachment { filename, file_path, mime_type } in ActorState
  → sets AppState.pending_attachment = Some(AttachmentInfo { filename, size_display, is_image: true })
do_send_message is called
  → reads file bytes from file_path
  → decodes with `image` crate, resizes to ≤ 1024px long-edge, encodes JPEG quality=80
  → base64-encodes → builds data URL "data:image/jpeg;base64,<b64>"
  → builds ChatCompletionRequestUserMessageContent::Array([
        ContentPart::Text(user_text),
        ContentPart::ImageUrl(ChatCompletionRequestMessageContentPartImage {
            image_url: ImageUrl { url: data_url, detail: Some(ImageDetail::Auto) }
        })
    ])
  → persists message to SQLite as plain text (no base64 stored — path reference only)
```

### async-openai 0.34 Exact Types [VERIFIED: source inspection]

```rust
use async_openai::types::chat::{
    ChatCompletionRequestUserMessageArgs,
    ChatCompletionRequestUserMessageContent,
    ChatCompletionRequestUserMessageContentPart,
    ChatCompletionRequestMessageContentPartText,
    ChatCompletionRequestMessageContentPartImage,
};
use async_openai::types::shared::{ImageUrl, ImageDetail};

// Build multipart user message:
let text_part = ChatCompletionRequestUserMessageContentPart::Text(
    ChatCompletionRequestMessageContentPartText { text: user_text.to_string() },
);
let image_part = ChatCompletionRequestUserMessageContentPart::ImageUrl(
    ChatCompletionRequestMessageContentPartImage {
        image_url: ImageUrl {
            url: format!("data:image/jpeg;base64,{}", base64_bytes),
            detail: Some(ImageDetail::Auto),
        },
    },
);
let content = ChatCompletionRequestUserMessageContent::Array(vec![text_part, image_part]);
let msg = ChatCompletionRequestUserMessageArgs::default()
    .content(content)
    .build()?;
```

**Note:** `ChatCompletionRequestUserMessageContent::Array` is the enum variant for multipart
content. The `Text(String)` variant remains valid for text-only messages — no need to change
existing messages. [VERIFIED: source inspection of async-openai-0.34.0/src/types/chat/chat_.rs]

### AttachmentInfo Evolution (backward compatible)

Current `AttachmentInfo` (UniFFI Record) carries `filename: String`, `size_display: String`.

Add one optional field:

```rust
#[derive(uniffi::Record, Clone, Debug)]
pub struct AttachmentInfo {
    pub filename: String,
    pub size_display: String,
    pub is_image: bool,   // new — false for text attachments (backward compat default)
}
```

Add a separate actor-internal struct (not UniFFI):

```rust
struct PendingImageAttachment {
    filename: String,
    file_path: String,   // absolute path in app sandbox
    mime_type: String,   // "image/jpeg" | "image/png"
}
```

Keep `PendingAttachment` (text) unchanged. Store both options as an enum in `ActorState` or use
separate `Option` fields — separate options is simpler given UniFFI doesn't see this struct.

Add AppAction variant:

```rust
AttachImage {
    filename: String,
    file_path: String,   // absolute path — actor reads file bytes at request time
    mime_type: String,
}
```

**Why file_path and not raw bytes?** Crossing base64-encoded image bytes over UniFFI would allocate
a String copy for every image. Passing a file path is O(1) allocation; the actor reads bytes on the
actor thread (which already owns file I/O for RAG). This respects the RMP architecture principle.

### ChatMessage struct in llm::streaming

`ChatMessage` has `content: String` only. For the initial implementation:
- Keep `ChatMessage` as `String` — it is used for history replay of old messages.
- Image-bearing messages are only ever the *last user message* in a turn. Build the multipart
  `ChatCompletionRequestMessage` directly in `do_send_message` rather than storing it in the
  `chat_messages: Vec<ChatMessage>` history. The history-replay loop already runs before the new
  message is appended, so image content only needs to appear in the final API call — not in
  historical messages.

This avoids a more invasive `ChatMessage` refactor and keeps the change scope tight.

### Recommended Project Structure Changes

```
rust/src/lib.rs
  AttachmentInfo      — add is_image: bool field
  AppAction           — add AttachImage variant
  PendingImageAttachment — new actor-internal struct
  ActorState          — add pending_image_attachment: Option<PendingImageAttachment>
  do_send_message     — branch: if pending_image_attachment → multipart request
  handler for AttachImage → store PendingImageAttachment

rust/Cargo.toml
  image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }

android/app/src/main/
  AndroidManifest.xml — CAMERA permission, FileProvider declaration
  res/xml/file_paths.xml — FileProvider paths config
  java/.../ui/ChatScreen.kt — replace fileLauncher with camera/gallery pickers
  java/.../ui/ComposeBar.kt — update attach button (optional: add camera icon)

ios/Mango/Mango/
  Info.plist — NSCameraUsageDescription, NSPhotoLibraryUsageDescription
  ChatView.swift — replace fileImporter with PhotosPicker + ImagePickerController sheet
  ImagePickerView.swift — new UIViewControllerRepresentable for camera

desktop/iced/src/main.rs
  Message::AttachFile handler — add image filter to FileDialog
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Image decode / resize / JPEG encode | Custom pixel loop | `image` crate | Handles EXIF orientation, ICC profiles, correct JPEG encode |
| HEIC decode on iOS | Rust HEIC decoder | Convert in Swift before handing path to Rust | Apple's `UIImage` → `jpegData()` handles HEIC natively |
| Android content URI → file path | URI path parsing | `contentResolver.openInputStream()` → write temp file | Content URIs may not map to real file paths on all OEMs |
| Base64 encoding | Custom encoder | Already-present `base64 = "0.22"` | Consistent with attestation encoding already in codebase |

---

## Common Pitfalls

### Pitfall 1: EXIF Orientation
**What goes wrong:** JPEG photos taken in portrait mode carry orientation metadata. If decoded and
re-encoded without applying the rotation, images appear sideways or upside-down in the model's
vision output.
**How to avoid:** Call `image::DynamicImage::apply_orientation` (available in image 0.25) after
decoding, before resizing. Or apply EXIF rotation in Swift/Kotlin before handing to Rust.
**Warning signs:** Model describes image as rotated; thumbnail looks wrong in UI.

### Pitfall 2: HEIC on iOS
**What goes wrong:** iPhone default camera format is HEIC; the `image` Rust crate cannot decode it.
**How to avoid:** In Swift, load with `UIImage(contentsOfFile:)`, call `.jpegData(compressionQuality: 0.8)`,
write to a temp `.jpg` file in the app sandbox, then pass that path to Rust via `AttachImage`.
**Warning signs:** Rust `image::open()` returns an error for `.heic` extension.

### Pitfall 3: Android Content URI vs File Path
**What goes wrong:** `PickVisualMedia` returns a `content://` URI. Passing its `lastPathSegment` as
a file path to Rust will fail — the path is not a real filesystem path.
**How to avoid:** In Kotlin, use `contentResolver.openInputStream(uri)`, copy bytes to a temp file
in `context.cacheDir`, then pass the temp file path via `AttachImage`.

### Pitfall 4: FileProvider for Camera (Android)
**What goes wrong:** `TakePicture` requires a `Uri` created via `FileProvider.getUriForFile()`.
Without the `<provider>` declaration in `AndroidManifest.xml` and a `file_paths.xml`, the app
crashes on Android 7+.
**How to avoid:** Add `FileProvider` to manifest with `authority = "${applicationId}.fileprovider"`,
add `res/xml/file_paths.xml` pointing to `context.cacheDir` (or `filesDir`).

### Pitfall 5: base64 Bloat and Provider Size Limits
**What goes wrong:** A 12MP iPhone photo is ~8MB JPEG → ~11MB base64 string. Most confidential
inference providers have request body limits of 4–20MB. Base64 adds ~33% overhead.
**How to avoid:** Resize to ≤ 1024px long-edge, JPEG quality 80 → typically < 300KB → ~400KB
base64. This is well within any provider's limit and is the OpenAI-recommended approach for
`detail: low` (≤ 512px) or `detail: auto` (≤ 2048px).
**Warning signs:** `413 Request Entity Too Large` or provider timeout.

### Pitfall 6: SQLite Storage of Image Content
**What goes wrong:** If the base64 image string is stored in the `messages` table `content` column,
the DB grows rapidly (~400KB per message). Encrypted with SQLCipher (Phase 28), this compounds.
**How to avoid:** Store only the original file path (or a placeholder text like `[Image attached]`)
in the `content` column. For message history replay, images are sent once per conversation turn;
historical image messages can be replayed as text-only placeholders. The model sees the image in
real time; re-sending old images on every turn is not needed for stateless API calls.

### Pitfall 7: ChatMessage History for Tool Round
**What goes wrong:** Phase 27 wires a tool-use first round that converts `Vec<ChatMessage>` to API
messages. If a user sends an image with tools enabled, the multipart construction must also apply
to that path.
**How to avoid:** Factor the "build user message content" logic into a shared function used by both
the streaming path and the tool-round path.

### Pitfall 8: UniFFI `bool` field default
**What goes wrong:** Adding `is_image: bool` to `AttachmentInfo` (a UniFFI Record) without a
default may break Kotlin/Swift deserialization of old AppState snapshots.
**How to avoid:** UniFFI Records are positional — existing callers must update. Regenerate bindings
immediately after adding the field. No runtime migration needed (AppState is not persisted).

---

## Provider Compatibility

| Provider | Vision support | Notes |
|----------|---------------|-------|
| Chutes | [ASSUMED] likely yes for vision models | Passes through OpenAI-compatible image_url |
| Maple | [ASSUMED] model-dependent | Must select a vision-capable model |
| PPQ.AI | [ASSUMED] model-dependent | Some models in their catalog support vision |
| Privatemode | [ASSUMED] unknown | |
| Tinfoil | [ASSUMED] unknown | |
| Redpill | [ASSUMED] unknown | |
| NEAR AI | [ASSUMED] unknown | |
| NanoGPT | [ASSUMED] unknown | |

**Recommended approach:** Do not gate image sending by model capability detection. Send the
multipart message unconditionally when an image is attached. If the model does not support vision,
the provider will return a 400/422 error which is surfaced via the existing `last_error` path. This
is simpler than per-backend capability flags and consistent with how tool use works (Phase 27).

Add a comment in the code noting that `detail: ImageDetail::Auto` lets the provider decide token
cost vs. quality, matching the OpenAI guidance. [CITED: https://platform.openai.com/docs/guides/vision]

---

## Code Examples

### Resize + JPEG encode in Rust [ASSUMED pattern — image crate 0.25]

```rust
use image::{DynamicImage, ImageFormat, GenericImageView};
use base64::{engine::general_purpose::STANDARD, Engine};

fn prepare_image_for_api(file_path: &str) -> anyhow::Result<String> {
    let img = image::open(file_path)?;
    // Apply EXIF orientation if present
    // image 0.25: auto-applies EXIF on open by default for JPEG
    let (w, h) = img.dimensions();
    let max_dim: u32 = 1024;
    let img = if w > max_dim || h > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img
    };
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Jpeg)?;
    let b64 = STANDARD.encode(&buf);
    Ok(format!("data:image/jpeg;base64,{}", b64))
}
```

### Android: PickVisualMedia launcher [ASSUMED — pattern from androidx.activity 1.7+]

```kotlin
val mediaLauncher = rememberLauncherForActivityResult(
    contract = ActivityResultContracts.PickVisualMedia()
) { uri: Uri? ->
    uri?.let {
        scope.launch(Dispatchers.IO) {
            val bytes = context.contentResolver.openInputStream(it)?.readBytes() ?: return@launch
            val tmpFile = File(context.cacheDir, "img_${System.currentTimeMillis()}.jpg")
            tmpFile.writeBytes(bytes)
            val filename = it.lastPathSegment ?: "image.jpg"
            withContext(Dispatchers.Main) {
                onAttachImage(filename, tmpFile.absolutePath, "image/jpeg")
            }
        }
    }
}
// Launch:
mediaLauncher.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
```

### Android: TakePicture launcher with FileProvider [ASSUMED]

```kotlin
var cameraImageUri by remember { mutableStateOf<Uri?>(null) }
val cameraLauncher = rememberLauncherForActivityResult(
    contract = ActivityResultContracts.TakePicture()
) { success ->
    if (success) {
        cameraImageUri?.let { uri ->
            // uri is a file:// URI pointing into cacheDir via FileProvider
            onAttachImage("camera.jpg", uri.path ?: return@let, "image/jpeg")
        }
    }
}
// Usage: create temp file first, then launch
val tmpFile = File(context.cacheDir, "camera_capture.jpg")
cameraImageUri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", tmpFile)
cameraLauncher.launch(cameraImageUri!!)
```

### iOS: HEIC → JPEG in Swift [ASSUMED]

```swift
func convertToJpeg(url: URL) -> URL? {
    guard let uiImage = UIImage(contentsOfFile: url.path) else { return nil }
    guard let jpegData = uiImage.jpegData(compressionQuality: 0.8) else { return nil }
    let tmpURL = FileManager.default.temporaryDirectory
        .appendingPathComponent(UUID().uuidString + ".jpg")
    try? jpegData.write(to: tmpURL)
    return tmpURL
}
```

---

## Environment Availability

Step 2.6: SKIPPED — Phase is code changes only; no new external services or CLI tools are required
beyond the `image` Rust crate (pure Rust, no system deps) and existing platform toolchains.

---

## Runtime State Inventory

Step 2.5: Phase is greenfield feature addition, not a rename/refactor. SKIPPED.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[cfg(test)]` unit tests (inline) + tokio test runtime for async |
| Config file | None (inline tests) |
| Quick run command | `cargo test -p mango_core prepare_image` |
| Full suite command | `cargo test -p mango_core` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| IMG-01 | `prepare_image_for_api` resizes and returns valid data-URL | unit | `cargo test -p mango_core test_prepare_image_jpeg` | No — Wave 0 |
| IMG-02 | `do_send_message` builds `Array` content when image pending | unit (mock) | `cargo test -p mango_core test_send_message_with_image` | No — Wave 0 |
| IMG-03 | Text-only messages still use `Text(String)` content | unit | `cargo test -p mango_core test_send_message_text_only` | No — Wave 0 |
| IMG-04 | `AttachImage` action stores `PendingImageAttachment` in ActorState | unit | `cargo test -p mango_core test_attach_image_action` | No — Wave 0 |
| IMG-05 | Platform camera/gallery picker → temp file → AttachImage dispatch | manual | — | manual only |
| IMG-06 | Image pill appears in compose bar after attachment | manual | — | manual only |
| IMG-07 | Model responds describing image content | manual (vision model) | — | manual only |

### Sampling Rate

- Per task commit: `cargo test -p mango_core`
- Per wave merge: `cargo test -p mango_core && cargo build --target aarch64-linux-android`
- Phase gate: full suite green + manual IMG-05/06/07 verified on at least one platform before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `rust/src/lib.rs` inline test `test_prepare_image_jpeg` — covers IMG-01
- [ ] `rust/src/lib.rs` inline test `test_send_message_with_image` — covers IMG-02
- [ ] `rust/src/lib.rs` inline test `test_send_message_text_only` — covers IMG-03
- [ ] `rust/src/lib.rs` inline test `test_attach_image_action` — covers IMG-04

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | validate MIME type; reject non-image files at action dispatch |
| V6 Cryptography | no (image not stored encrypted; base64 is encoding not encryption) | — |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via `file_path` field in `AttachImage` | Tampering | Rust actor validates path is within app sandbox (`starts_with(data_dir)`) before opening |
| Oversized image exhausting memory | DoS | Enforce max file size (e.g. 20MB) before decoding; `image::open()` streams; resize bounds limit RAM |
| Malformed JPEG/PNG triggering image crate panic | Tampering | Wrap `image::open()` in `anyhow::Result` — `image` crate returns `Err` for malformed files, no panic |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `image = "0.25"` compiles cleanly on iOS/Android targets | Standard Stack | Build failure; may need feature pruning or alternative |
| A2 | Provider backends accept `image_url` with base64 data-URL syntax when model supports vision | Provider Compatibility | API 400 errors for all image sends; would require provider-specific workarounds |
| A3 | `image` 0.25 auto-applies EXIF orientation on JPEG open | Code Examples | Images appear rotated; need explicit EXIF read + rotate |
| A4 | `PickVisualMedia` is available without extra dep via `activity-compose:1.10.1` | Standard Stack (Android) | May need explicit `registerForActivityResult` with older API compat |
| A5 | Confidential inference backends support `detail: auto` in the ImageUrl object | Provider Compatibility | Providers may reject unknown fields; use `detail: None` as fallback |

---

## Open Questions

1. **Should the attach button remain a single button (paperclip) that opens a choice dialog, or split into separate camera + gallery buttons?**
   - What we know: Current ComposeBar has a single `onAttach` callback with a paperclip icon on all platforms.
   - What's unclear: UX preference for camera vs. gallery as separate entry points.
   - Recommendation: Add a bottom sheet or action sheet with "Camera" / "Photo Library" / "File" options behind the existing paperclip, keeping the single-button composebar signature.

2. **Should old text-file attachment and image attachment share the same button?**
   - What we know: Current flow accepts `*/*` MIME (any file) for text context.
   - Recommendation: Keep file and image as separate actions — the "Photo" entry in the action sheet dispatches `AttachImage`, the "File" entry retains the existing `AttachFile` text path.

3. **Is vision support per-model or per-backend?**
   - What we know: No per-backend capability config exists for vision (only `TeeType`, `models` list).
   - Recommendation: Defer capability gating to v2 (add a `vision_capable` flag to `BackendCapabilities`). For Phase 31, send unconditionally and surface errors.

---

## Sources

### Primary (HIGH confidence)
- `async-openai-0.34.0` source at `~/.cargo/registry/src/.../async-openai-0.34.0/src/types/chat/chat_.rs` — `ChatCompletionRequestUserMessageContent`, `ChatCompletionRequestUserMessageContentPart::ImageUrl`, `ChatCompletionRequestMessageContentPartImage`, `ImageUrl`, `ImageDetail` all verified [VERIFIED: source inspection]
- `rust/Cargo.toml` — confirmed `async-openai = "0.34"`, `base64 = "0.22"`, `image` crate absent [VERIFIED: source]
- `rust/src/lib.rs` — confirmed `AttachmentInfo`, `PendingAttachment`, `AttachFile` action, `do_send_message` text-prefix logic, actor state layout [VERIFIED: source]
- `android/app/build.gradle.kts` — `activity-compose:1.10.1`, minSdk 28 [VERIFIED: source]
- `desktop/iced/Cargo.toml` — `rfd = "0.15"` confirmed [VERIFIED: source]
- `ios/Mango/Mango/Info.plist` — no camera/photo usage strings present [VERIFIED: source]
- `ios/Mango/Mango/ChatView.swift` — `.fileImporter` with `[.plainText, .pdf, .data]` (no image types) [VERIFIED: source]
- `android/app/.../ChatScreen.kt` — `ActivityResultContracts.GetContent()` with `*/*` filter [VERIFIED: source]

### Secondary (MEDIUM confidence)
- OpenAI Vision guide: base64 data-URL format `data:image/jpeg;base64,...`, `detail` parameter, 20MB limit — [CITED: https://platform.openai.com/docs/guides/vision]
- Android FileProvider documentation — `<provider>` manifest element, `file_paths.xml` format — [ASSUMED: well-known Android API, not re-fetched]
- `image` crate 0.25 EXIF orientation handling — [ASSUMED: based on training knowledge; verify with `image` crate changelog]

---

## Metadata

**Confidence breakdown:**
- Standard stack (async-openai types): HIGH — verified by source inspection of downloaded crate
- Architecture pattern: HIGH — grounded in existing code structure
- Platform APIs (Android/iOS): MEDIUM — standard patterns, not re-verified against current SDK docs
- Provider vision compatibility: LOW — no authoritative source; flagged in Assumptions Log

**Research date:** 2026-04-18
**Valid until:** 2026-05-18 (stable APIs; image crate and async-openai unlikely to change in 30 days)
