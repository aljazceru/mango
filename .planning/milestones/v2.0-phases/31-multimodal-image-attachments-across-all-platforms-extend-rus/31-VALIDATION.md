---
phase: 31
slug: multimodal-image-attachments-across-all-platforms-extend-rus
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-18
---

# Phase 31 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `#[cfg(test)]` unit tests (inline) + tokio test runtime for async |
| **Config file** | none — inline tests in `rust/src/lib.rs` |
| **Quick run command** | `cargo test -p mango_core prepare_image` |
| **Full suite command** | `cargo test -p mango_core` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mango_core`
- **After every plan wave:** Run `cargo test -p mango_core && cargo build --target aarch64-linux-android`
- **Before `/gsd-verify-work`:** Full suite green + manual IMG-05/06/07 verified on at least one platform
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 31-00-01 | 00 | 0 | IMG-01..IMG-04 | — | Wave 0 test scaffolds present | unit | `cargo test -p mango_core --no-run` | ❌ W0 | ⬜ pending |
| 31-01-01 | 01 | 1 | IMG-01 | — | `prepare_image_for_api` resizes to ≤1024px long-edge and returns a `data:image/jpeg;base64,` URL | unit | `cargo test -p mango_core test_prepare_image_jpeg` | ❌ W0 | ⬜ pending |
| 31-01-02 | 01 | 1 | IMG-02 | — | `do_send_message` emits `ChatCompletionRequestUserMessageContent::Array` with `ImageUrl` part when a `PendingImageAttachment` exists | unit (mock) | `cargo test -p mango_core test_send_message_with_image` | ❌ W0 | ⬜ pending |
| 31-01-03 | 01 | 1 | IMG-03 | — | Text-only messages continue to use `ChatCompletionRequestUserMessageContent::Text(String)` | unit | `cargo test -p mango_core test_send_message_text_only` | ❌ W0 | ⬜ pending |
| 31-01-04 | 01 | 1 | IMG-04 | T-31-01 | `AttachImage` action stores `PendingImageAttachment` in `ActorState` and validates path is inside app sandbox | unit | `cargo test -p mango_core test_attach_image_action` | ❌ W0 | ⬜ pending |
| 31-02-01 | 02 | 2 | IMG-05, IMG-06 (iOS) | T-31-02 | iOS HEIC→JPEG conversion writes to sandbox temp, dispatches `AttachImage` with sandbox path | build | `xcodebuild -scheme Mango -configuration Debug build` | ✅ | ⬜ pending |
| 31-03-01 | 03 | 4 | IMG-05, IMG-06 (Android) | T-31-02 | Manifest declares CAMERA + FileProvider; Gradle manifest merge succeeds | build | `cd android && ./gradlew :app:processDebugManifest` | ✅ | ⬜ pending |
| 31-03-02 | 03 | 4 | IMG-05, IMG-06 (Android) | T-31-02, T-31-03 | Camera/gallery launchers dispatch `AttachImage` with cacheDir path; compose bar renders image pill | build | `cd android && ./gradlew :app:assembleDebug` | ✅ | ⬜ pending |
| 31-04-01 | 04 | 3 | IMG-05, IMG-06 (iOS) | T-31-02 | iOS PhotosPicker + camera sheet routes through Swift HEIC→JPEG to `AttachImage` | build | `xcodebuild -scheme Mango -configuration Debug build` | ✅ | ⬜ pending |
| 31-05-01 | 05 | 3 | IMG-05, IMG-06 (Desktop) | T-31-02 | Desktop rfd dialog branches by extension; images dispatch `AttachImage`, text preserved | build | `cd desktop/iced && cargo build` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `rust/src/lib.rs` inline test `test_prepare_image_jpeg` — covers IMG-01
- [ ] `rust/src/lib.rs` inline test `test_send_message_with_image` — covers IMG-02
- [ ] `rust/src/lib.rs` inline test `test_send_message_text_only` — covers IMG-03
- [ ] `rust/src/lib.rs` inline test `test_attach_image_action` — covers IMG-04

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Platform camera/gallery picker → temp file → `AttachImage` dispatch | IMG-05 | Requires physical camera hardware / photo library permission prompts | Install debug build on device; tap paperclip → Take Photo or Choose Photo; confirm pill appears |
| Image pill appears in compose bar after attachment | IMG-06 | Visual UI verification | Attach image, visually confirm pill with filename and image icon |
| Vision-capable model responds describing image content | IMG-07 | Requires live network call to confidential inference provider with vision model | Send "describe this image" with an image attached against a vision-capable model; confirm coherent description |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
