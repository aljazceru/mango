---
phase: 31
plan: 01
subsystem: rust-core
tags: [multimodal, images, tdd-wave2, green, async-openai, uniffi, multipart]
requires:
  - image = "0.25" on the classpath (landed by 31-00)
  - 31-00 Wave-0 RED tests referencing prepare_image_for_api, build_user_message_with_image, PendingImageAttachment, AppAction::AttachImage, AttachmentInfo::is_image
provides:
  - AttachmentInfo::is_image: bool (UniFFI Record gains one field — binding regen is 31-02's job)
  - AppAction::AttachImage { filename, file_path, mime_type } with MIME allowlist + absolute-path + 50 MB validation
  - Actor-internal PendingImageAttachment struct + ActorState.pending_image_attachment slot
  - fn prepare_image_for_api(path) -> Result<String> (resize 1536 long-edge, JPEG q80, base64 data URL)
  - fn build_user_message_with_image(actor_state, text) -> Result<ChatCompletionRequestMessage>
  - do_send_message image branch routing through spawn_streaming_task_from_api_messages
affects:
  - Plan 31-02 (UniFFI bindings regen — AttachmentInfo gained is_image field, AppAction gained AttachImage variant)
  - Plan 31-03..05 (platform UI: dispatch AttachImage with absolute app-sandbox path + mime)
tech-stack:
  added: []
  patterns:
    - Wave-2 TDD: turn Wave-0 RED failures green in a single atomic plan
    - Mutually exclusive pending_attachment / pending_image_attachment slots (one UI pill)
    - base64 bytes never persisted (SQLite `content` gets "[Image: {filename}]" placeholder) — T-31-04 mitigation
key-files:
  created: []
  modified:
    - rust/src/lib.rs
decisions:
  - build_user_message_with_image takes &ActorState (not Option<&PendingImageAttachment>) to match the 31-00 test contract; production callers pass actor_state directly, tests verify both image and no-image branches through the same signature
  - Persisted user message content for image turns is "{user_text}\n\n[Image: {filename}]" (or just the placeholder if text is empty) — never base64, never the resized JPEG bytes
  - do_send_message builds API messages inline via the existing ChatCompletionRequest*MessageArgs pattern rather than calling api_messages_to_chat_messages / exposing chat_messages_to_api_messages as public; this mirrors the Phase 27 tool path at lines 1883-1913 and avoids touching streaming.rs visibility
  - JPEG encoding goes through to_rgb8() + JpegEncoder::encode(..., ExtendedColorType::Rgb8) to strip alpha channels that JpegEncoder rejects — works for both PNG and JPEG inputs
  - Test helper default_actor_state_for_image_tests() builds a full in-memory ActorState (runtime + :memory: DB + Null providers) rather than a partial mock; keeps AttachImage match-arm logic production-shaped
metrics:
  duration: 12min
  tasks: 2
  files: 1
  completed: 2026-04-19
---

# Phase 31 Plan 01: Rust Core Multimodal Image Wiring Summary

**One-liner:** Rust core is now vision-capable end-to-end — AttachmentInfo carries `is_image`, `AppAction::AttachImage` validates MIME/path/size, `prepare_image_for_api` resizes to 1536 px and returns a JPEG data URL, and `do_send_message` emits multipart `ChatCompletionRequestUserMessageContent::Array` through `spawn_streaming_task_from_api_messages` when an image is pending.

## Outcome

- All 4 Wave-0 RED tests (IMG-01..04) turn GREEN: `test_prepare_image_jpeg`, `test_send_message_with_image`, `test_send_message_text_only`, `test_attach_image_action`.
- Full `cargo test -p mango_core` suite: **271 passed, 0 failed, 10 ignored, 0 regressions**.
- Text-only send path unchanged — `spawn_streaming_task(...)` still invoked when `pending_image_attachment.is_none()`.
- Image send path: text is persisted as `[Image: {filename}]` placeholder, base64 bytes never touch SQLite (T-31-04).

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Types + action + ActorState plumbing | be6dc7c | rust/src/lib.rs |
| 2 | Image pipeline + multipart builder + do_send_message branch (GREEN) | 3f6834c | rust/src/lib.rs |

## Symbol Inventory (Plan 31-02 UniFFI Regen Input)

| Symbol | Kind | Location |
|--------|------|----------|
| `AttachmentInfo.is_image` | uniffi::Record field (bool) | lib.rs ~line 162 |
| `AppAction::AttachImage { filename, file_path, mime_type }` | uniffi::Enum variant (3 String fields) | lib.rs ~line 480 |
| `PendingImageAttachment` | actor-internal struct (NOT UniFFI) | lib.rs ~line 808 |
| `ActorState.pending_image_attachment` | actor-internal field | lib.rs ~line 821 |
| `prepare_image_for_api` | private fn | lib.rs ~line 1069 |
| `build_user_message_with_image` | private fn | lib.rs ~line 1103 |

Plan 31-02 regenerates Kotlin/Swift bindings for the AttachmentInfo + AppAction changes. The two helpers and PendingImageAttachment stay Rust-internal.

## Verification Results

### Acceptance criteria — Task 1

- `grep -c 'pub is_image: bool' rust/src/lib.rs` → **1** ✓
- `grep -c 'AttachImage {' rust/src/lib.rs` → **4** (enum variant + actor match arm + 2 test helpers) ✓ (≥ 2)
- `grep -c 'struct PendingImageAttachment' rust/src/lib.rs` → **1** ✓
- `grep -c 'pending_image_attachment: None' rust/src/lib.rs` → **2** ✓ (every ActorState construction updated: actor init + test helper)
- `cargo build -p mango_core` → **exit 0** ✓

### Acceptance criteria — Task 2

- `grep -c 'fn prepare_image_for_api' rust/src/lib.rs` → **1** ✓
- `grep -c 'fn build_user_message_with_image' rust/src/lib.rs` → **1** ✓
- `grep -c 'spawn_streaming_task_from_api_messages' rust/src/lib.rs` → **3** ✓ (pre-existing Phase-27 tool-followup use at ~line 5400 + new image branch + import) (≥ 2)
- `grep -c 'has_image_attachment' rust/src/lib.rs` → **3** ✓
- `grep -c 'ChatCompletionRequestUserMessageContent::Array' rust/src/lib.rs` → **1** ✓
- All 4 Wave-0 tests GREEN ✓
- Full `cargo test -p mango_core` exits 0 (271 passed, 10 ignored, 0 failed) ✓

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] build_user_message_with_image signature in plan vs in 31-00 tests**

- **Found during:** Task 2 draft review against rust/src/lib.rs ~line 6951.
- **Issue:** The plan's action block defines `build_user_message_with_image(pending_image: Option<&PendingImageAttachment>, user_text: &str)`, but the 31-00 RED test invokes `build_user_message_with_image(&actor_state, "describe")`. The two are incompatible; since the test signature is locked by the Wave-0 commit (02628cc), the implementation must conform.
- **Fix:** Implemented `build_user_message_with_image(actor_state: &ActorState, user_text: &str) -> anyhow::Result<ChatCompletionRequestMessage>` and read `actor_state.pending_image_attachment.as_ref()` inside the function. Behaviorally identical to the plan's described logic.
- **Files modified:** rust/src/lib.rs (lib.rs image-helpers block).
- **Commit:** 3f6834c.

**2. [Rule 1 - Bug] JpegEncoder::encode_image rejects images with alpha channel**

- **Found during:** Task 2 first test run of `test_prepare_image_jpeg` against a PNG input.
- **Issue:** A PNG decoded as RGBA8 cannot be encoded directly by `JpegEncoder::encode_image`, because JPEG does not support an alpha channel; the encoder errors out with `UnsupportedImageFormat`.
- **Fix:** Call `out.to_rgb8()` to strip alpha before encoding, then use `enc.encode(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)` instead of `encode_image(&out)`. Works for both JPEG and PNG inputs.
- **Files modified:** rust/src/lib.rs (`prepare_image_for_api`).
- **Commit:** 3f6834c.

**3. [Rule 2 - Critical] Error path in do_send_message image branch left busy state stuck**

- **Found during:** Task 2 code review.
- **Issue:** The plan's snippet sets `last_error` and returns early when `build_user_message_with_image` fails, but by that point busy_state has already been set to `BusyState::Streaming` and `streaming_text` to `Some(String::new())`. A returning-without-reset would leave the UI spinner hanging forever.
- **Fix:** On image-prepare failure also reset `busy_state = BusyState::Idle`, `streaming_text = None`, call `refresh_backend_summaries`, then return. Mirrors the idle-reset pattern used elsewhere on early-exit failure paths.
- **Files modified:** rust/src/lib.rs (`do_send_message` image branch error handling).
- **Commit:** 3f6834c.

**4. [Rule 2 - Critical] Persisted content for image turns lacked the mandated placeholder**

- **Found during:** Task 2 implementation against the plan's `<behavior>` spec (IMG-04 / T-31-04).
- **Issue:** The plan specifies that SQLite `content` for an image turn should be "plain user text plus a `[Image: {filename}]` placeholder" — base64 NEVER persisted. The naive implementation would have just carried `text` through as `final_text`, losing the placeholder and the provenance record.
- **Fix:** When `has_image_attachment`, build `final_text` as `"{text}\n\n[Image: {filename}]"` (or just the bracketed placeholder if `text` is empty). This is what lands in the SQLite row and the UiMessage; the base64 data URL only flows through the API request as an `image_url` multipart part.
- **Files modified:** rust/src/lib.rs (`do_send_message` attachment text block).
- **Commit:** 3f6834c.

### Auth Gates

None — plan is pure Rust-core wiring.

## Threat Flags

None — the plan and its implementation stay within the trust boundaries and mitigations documented in the 31-01 `<threat_model>` (T-31-01 absolute-path check, T-31-02 50 MB cap, T-31-03 MIME allowlist, T-31-04 no-base64-persistence, T-31-05 image-crate errors wrapped in anyhow).

## Known Stubs

None. The 31-00 `unimplemented!()` test stubs (`default_actor_state_for_image_tests`, `handle_attach_image_for_test`) are fully replaced with working helpers that build a real in-memory `ActorState` and apply the real AttachImage validation logic.

## Self-Check: PASSED

- rust/src/lib.rs contains `pub is_image: bool` — FOUND.
- rust/src/lib.rs contains `AppAction::AttachImage` variant definition (enum) — FOUND.
- rust/src/lib.rs contains `struct PendingImageAttachment` — FOUND.
- rust/src/lib.rs contains `fn prepare_image_for_api` — FOUND.
- rust/src/lib.rs contains `fn build_user_message_with_image` — FOUND.
- rust/src/lib.rs contains `ChatCompletionRequestUserMessageContent::Array` usage — FOUND.
- rust/src/lib.rs contains `has_image_attachment` guard in do_send_message — FOUND.
- Commit be6dc7c — FOUND on main.
- Commit 3f6834c — FOUND on main.
- `cargo test -p mango_core image_red_tests` — 4 passed, 0 failed.
- `cargo test -p mango_core` — 271 passed, 0 failed, 10 ignored.
