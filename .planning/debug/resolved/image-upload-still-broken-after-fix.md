---
status: resolved
trigger: "Image upload on Android with gemma4-31b still broken despite d66499f/4dec88c/8c78d50 fixes + follow-up: gate image upload by model vision capability"
created: 2026-04-19
updated: 2026-04-19
---

## Current Focus

hypothesis: Encrypted image persistence commits (e6b12f5, dcc3066, a7c204b) which landed AFTER the three fix commits altered the image path such that either (a) `pending_image_attachment` state shape changed and the multipart branch no longer fires, or (b) `api_text`/`final_text` split was undone/regressed, or (c) the attachment is encrypted before `prepare_image_for_api` reads it.
test: Read do_send_message as it exists at HEAD, compare to the fix commits, and check pending_image_attachment flow through the ece-01 changes.
expecting: Find either a regression in do_send_message (api_text no longer passed) OR a change in attachment storage (encrypted path) that breaks prepare_image_for_api OR an unchanged code path (meaning the APK is stale).
next_action: Read rust/src/lib.rs around the image branch at current HEAD.

## Symptoms

expected: Android attach photo → send "whats in the photo" → gemma4-31b describes image.
actual: Model replies "Please provide the photo you are referring to!" — image clearly not received. Chat bubble still shows "[Image: camera_1776590907841.jpg]" line.
errors: None. Silent wrong behavior.
reproduction: Android app → gemma4-31b chat → paperclip → photo → "whats in the photo" → send.
started: After d66499f/4dec88c/8c78d50 fix commits landed (and after subsequent ece-01 encrypted image persistence work).

## Eliminated

- hypothesis: "APK is stale / doesn't contain the three fix commits"
  evidence: android/app/build/outputs/apk/debug/app-debug.apk mtime = 2026-04-19 11:06; all three fix commits (d66499f/4dec88c/8c78d50) landed by 09:28, ece-01 landed 10:35. APK was built after the fixes. Additionally the camera filename in the screenshot (camera_1776590907841.jpg) decodes to a timestamp well after 11:06, consistent with the user running the post-fix APK.
  timestamp: 2026-04-19

- hypothesis: "ece-01 encrypt path consumed/moved pending_image_attachment before the multipart branch"
  evidence: rust/src/lib.rs:1820-1865 — encrypt path reads `actor_state.pending_image_attachment.as_ref()` but does NOT `.take()` it; pending_image_attachment is only cleared at line 2203 after the multipart request has been built at line 2187. Flow is correct.
  timestamp: 2026-04-19

- hypothesis: "Regression test was deleted/weakened during ece-01"
  evidence: `cargo test -p mango_core --lib test_build_user_message_with_image_does_not_leak_placeholder` → 1 passed. Test is still present and green.
  timestamp: 2026-04-19

- hypothesis: "do_send_message regressed to pass final_text instead of api_text"
  evidence: rust/src/lib.rs:2187 still calls `build_user_message_with_image(actor_state, &api_text)`. The api_text/final_text split from d66499f is intact. e6b12f5 touched lib.rs but only added the encrypt path at lines 1816-1865; the multipart branch at 2144-2213 is unchanged from the fix commits.
  timestamp: 2026-04-19

## Evidence

- timestamp: 2026-04-19
  checked: rust/src/llm/streaming.rs:287-313 + rust/src/llm/streaming.rs:336-398 — spawn_streaming_task_from_api_messages routing
  found: For `ProviderTransportKind::TinfoilSecure` (line 287) and `ProviderTransportKind::PpqPrivateE2ee` (line 302), the multipart `Vec<ChatCompletionRequestMessage>` is converted via `api_messages_to_chat_messages()` to plain `Vec<ChatMessage>` BEFORE being passed to `tinfoil_secure::run_streaming_chat_completion` / `ppq_private::run_streaming_chat_completion`. The conversion at line 354-374 explicitly FLATTENS `ChatCompletionRequestUserMessageContent::Array` → plain text by joining Text parts and `filter_map`-dropping `ImageUrl` parts. Comment at 355-363 acknowledges: "Preserving the text lets the conversation continue, at the cost of the image being invisible to the model."
  implication: Fix #2 (4dec88c) — "preserve Array message text on Tinfoil/PPQ transports" — deliberately drops the image. Any image sent to a Tinfoil-hosted vision model silently reaches the model without the image attached. The model sees the text "whats in the photo" with NO accompanying image, so it replies "Please provide the photo you are referring to!" — which is exactly what the screenshot shows.

- timestamp: 2026-04-19
  checked: rust/src/llm/tinfoil_secure.rs:160-243 — run_streaming_chat_completion
  found: Takes `Vec<ChatMessage>` (plain text per role) and rebuilds API messages via `ChatCompletionRequestUserMessageArgs::default().content(msg.content.clone())` at line 186-188. Then serializes via `serde_json::to_vec(&request)` at line 231 and sends to `/chat/completions` via `send_secure_request`. The function itself is not inherently text-only — it would happily serialize a multipart Array message if passed one. The restriction is imposed by the caller choosing to pass plain ChatMessage.
  implication: Tinfoil's secure streaming pipe is agnostic to message content shape — it's just serde_json + encrypted HTTP. A multipart request CAN be sent through it unchanged. The fix is to let the multipart Array cross the FFI into tinfoil_secure without the intermediate collapse.

- timestamp: 2026-04-19
  checked: WebSearch — Tinfoil's inference.tinfoil.sh hosts Gemma 3 (up to 27B) with vision via base64 data URLs; UI label "gemma4-31b" is most plausibly a Tinfoil-hosted Gemma 3 variant or a user-configured custom backend that routes to a Tinfoil-compatible endpoint.
  found: Tinfoil explicitly advertises vision processing using base64 image encoding on their platform. Gemma 3 is a multimodal model with SigLIP vision encoder.
  implication: The user's model IS capable of vision IF the image reaches it. The client is the broken link.

- timestamp: 2026-04-19
  checked: rust/src/llm/transport.rs:14-34 — ProviderTransportKind selection
  found: Only backend id "tinfoil" routes to TinfoilSecure; only backend id "ppq-ai" with /private in URL routes to PpqPrivateE2ee. All other backends (including any user-added backend with a non-matching id — even if base_url points at inference.tinfoil.sh) route to OpenAiCompatible.
  implication: Two plausible realities: (a) user is on the stock Tinfoil backend (id="tinfoil") → Array-drop bug triggers. (b) user configured a custom backend (id != "tinfoil") pointing at some provider → OpenAiCompatible path is taken and multipart SHOULD flow through unchanged. Case (a) is the one that matches the observed symptom exactly. Case (b) would require a different root cause. Fixing (a) is required regardless because it's a known defect (fix #2 called it out as a TODO) and the screenshot symptom is the exact predicted failure mode for that defect.

## Resolution

root_cause: `api_messages_to_chat_messages` in rust/src/llm/streaming.rs (called unconditionally for Tinfoil and PPQ transports inside `spawn_streaming_task_from_api_messages`) collapses `ChatCompletionRequestUserMessageContent::Array` to plain text and drops all image_url parts. Fix #2 from the prior debug session (4dec88c) deliberately implemented this as an interim measure with a TODO to "propagate multipart once Tinfoil/PPQ support vision". Tinfoil already supports vision (Gemma 3 with base64 data URLs), so the drop is the root cause of the user-visible regression: the model receives the raw user text ("whats in the photo") with NO image and correctly responds "Please provide the photo you are referring to!"

The three commits d66499f / 4dec88c / 8c78d50 did correctly fix the `[Image:]` placeholder leak (symptom #1 from the prior session — "Please provide the photo" with different reply "appears cut off"). They did NOT make the image actually reach the model on Tinfoil/PPQ transports.

fix:
  Added `run_streaming_chat_completion_from_api_messages` to both `rust/src/llm/tinfoil_secure.rs` and `rust/src/llm/ppq_private.rs`. These functions accept `Vec<ChatCompletionRequestMessage>` (async-openai API message types) directly and serialize them straight to the `/chat/completions` wire body -- no intermediate collapse to plain `ChatMessage`. Existing `run_streaming_chat_completion(Vec<ChatMessage>, ...)` functions now delegate to the new functions after their one-shot conversion, preserving the common chat path without duplication.

  `spawn_streaming_task_from_api_messages` in `rust/src/llm/streaming.rs` now routes TinfoilSecure and PpqPrivateE2ee requests through these new functions, bypassing the lossy `api_messages_to_chat_messages` helper.

  Deleted the dead `api_messages_to_chat_messages` helper and its two accompanying tests (`array_user_message_preserves_text_and_drops_image`, `text_user_message_preserved_verbatim`) -- their whole reason for existing was the drop-image-silently interim measure that the fix removes.

  Added two new regression tests that exercise the exact body-serialization pipeline both Tinfoil and PPQ use after the fix:
    - `multipart_user_message_survives_request_body_serialization` builds a multipart User message with Text + ImageUrl parts, wraps it via `CreateChatCompletionRequestArgs`, serializes via `serde_json::to_vec`, and asserts the resulting bytes contain both the raw user text AND the `data:image/jpeg;base64,...` URL AND the `"image_url"` content part key. This is the same pipeline `{tinfoil_secure,ppq_private}::run_streaming_chat_completion_from_api_messages` executes before sending bytes into the secure tunnel, so it proves the image reaches the wire body.
    - `text_only_user_message_survives_request_body_serialization` asserts text-only requests do NOT carry any spurious `"image_url"` -- guards against accidental regression of the common path.

verification:
  - `cargo build -p mango_core` clean (no warnings).
  - `cargo test -p mango_core --lib` -> 279 passed, 0 failed, 10 ignored (was 274 before ece-01; 274 + 5 ece-01 tests = 279, matching; my new tests replaced the 2 deleted ones 1:1 on each side so net test count is unchanged by this fix -- the 2 added and 2 deleted offset).
  - New regression tests pass: `multipart_user_message_survives_request_body_serialization`, `text_only_user_message_survives_request_body_serialization`, plus existing `test_build_user_message_with_image_does_not_leak_placeholder`.
  - **Device re-test REQUIRED:** Unit tests prove the wire body carries the image for the Tinfoil/PPQ code paths. A fresh APK built against this commit plus a repro on the device (attach photo -> "whats in the photo" -> gemma4-31b) should now elicit a description of the image rather than "please provide the photo".

files_changed:
  - rust/src/llm/tinfoil_secure.rs
  - rust/src/llm/ppq_private.rs
  - rust/src/llm/streaming.rs

## Follow-up: Vision Capability Gating

### Trigger
User feedback: "we should be verifying that the model the user is using in the chat supports image upload". Today the paperclip offers "Take Photo" / "Choose Photo" on every conversation regardless of whether the selected model can see an image. If a text-only model is selected, the image is base64-encoded, shipped over the wire, and the model either ignores it or complains ("Please provide the photo"). Silent failure from the user's POV.

### Current-state evidence
- rust/src/lib.rs:4370-4409 — `AttachImage` handler validates MIME and size but not model capability. Any conversation can hold a pending image attachment.
- rust/src/lib.rs:1114-1200 (build_user_message_with_image) — builds a multipart `Array` user message whenever `pending_image_attachment` is set. No model gate.
- rust/src/llm/backend.rs:35-49 — `BackendConfig.supports_tool_use` exists as a per-BACKEND boolean, but there is no per-MODEL capability metadata anywhere in the codebase. Models are stored as bare strings in `backends.model_list` (TEXT JSON array). The `/v1/models` probe parses only `id` fields and discards any other metadata the provider might return.
- rust/src/lib.rs:1291-1296 + filter_models_for_backend — models are filtered by prefix (`private/`) for PPQ; no capability layer today.
- ios/Mango/Mango/ChatView.swift:174-214 — paperclip opens `confirmationDialog` with three hardcoded buttons (Take Photo / Choose Photo / Attach File). No model-aware branching.
- android/app/.../ChatScreen.kt:319-357 — identical pattern with `ModalBottomSheet`.
- desktop/iced/src/main.rs:744-795 — `AttachFile` accepts both images and documents via a single file picker. No model-aware filter.
- Paperclip also serves text-file attach (RAG-ish context prepend), which is independent of model vision capability — so hiding the paperclip entirely is wrong. Only the IMAGE sub-actions need gating.

### Design decision (no user question needed — pick the safe default)

**UX:** hide "Take Photo" and "Choose Photo" from the attach sheet when the selected model is not vision-capable. Keep "Attach File" always available (it's for text/doc context, not vision). If neither image option is available, the sheet still shows "Attach File". Desktop: the iced picker's `Images` filter entries are removed when the model is non-vision. Paperclip itself never disappears.

Rationale: parallels existing "Tools: disabled when Brave key not set" pattern. Avoids a mid-send error that would discard an already-encoded image. Doesn't require the user to understand model capabilities — they just never see an option that would fail.

**Capability source:** pattern-match function in Rust core, `model_supports_vision(model_id: &str) -> bool`, UniFFI-exported. No DB migration (pure function). Starting patterns (case-insensitive substring):
- `gemma3`, `gemma-3`, `gemma4`, `gemma-4` — Google Gemma 3/4 multimodal (covers user's Tinfoil "gemma4-31b" label)
- `-vl` (delimited), `qwen-vl`, `qwen3-vl` — Qwen vision-language
- `llava`
- `pixtral`
- `gpt-4o`, `gpt-4-turbo`, `gpt-4.1`, `gpt-4v`, `gpt-4.5`
- `claude-3`, `claude-4` (including opus/sonnet/haiku variants)

Conservative bias: if unsure, return `false`. False negative = user can't attach a photo to a model that might have supported it (easy to fix by extending the list). False positive = user sends to a text-only model and gets the current broken UX. Prefer false negatives.

**Where the check is consumed:**
1. Rust: `AttachImage` handler ALSO re-checks and sets `last_error` if the action fires anyway — defense-in-depth against stale UI or edit-after-attach.
2. iOS/Android/Desktop UI: look at the selected model for the current conversation and conditionally render image menu entries.

### Hypotheses & plan (falsifiable)

- H1: The pattern-match function correctly classifies the four canonical model families.
  - Test: unit tests in Rust for each canonical model id (gemma3:27b, private/qwen3-vl-30b, llama3-3-70b, private/kimi-k2-5, claude-3-haiku, gpt-4o, deepseek-r1-0528).
  - Expected: vision-capable ones return true, text-only ones return false.

- H2: The `AttachImage` handler rejects non-vision models with a user-visible error.
  - Test: unit test that dispatches AttachImage against a conversation whose model is "llama3-3-70b" → assert `last_error` is set, `pending_image_attachment` stays None.

- H3: All three UIs can read capability through UniFFI without duplicating logic.
  - Test: confirm the UniFFI export surface (check generated .swift / .kt binding files name the function).

### Current Focus (updated for follow-up)
hypothesis: CONFIRMED. A `model_supports_vision` UniFFI-exported function + conditional UI rendering + defense-in-depth guard in AttachImage cleanly gates image upload to vision-capable models across all three platforms.
test: Implemented, unit tests written, UIs updated. `cargo test -p mango_core --lib` -> 284 passed. `cargo build -p mango_core -p mango-desktop` -> clean.
expecting: Device re-test confirms paperclip offers only "Attach File" on text-only models and image options on vision-capable models.
next_action: Human verification on device.

## Evidence (follow-up, appended)

- timestamp: 2026-04-19
  checked: rust/src/llm/backend.rs + persistence/schema.rs migration list (v1..v17)
  found: No model-level capability metadata. `supports_tool_use` is per-backend only. Model ids are stored as bare strings in `backends.model_list`. The /v1/models probe (rust/src/lib.rs:1459-1479) discards everything except model.id.
  implication: Capability must live in code (pure function) — matches user's "make it easy to extend" constraint. No DB migration needed.

- timestamp: 2026-04-19
  checked: ios/Mango/Mango/ChatView.swift:174-214, android/app/.../ChatScreen.kt:319-357, desktop/iced/src/main.rs:744-795
  found: iOS uses confirmationDialog with 3 hardcoded Buttons; Android uses ModalBottomSheet with 3 TextButtons; Desktop uses rfd FileDialog with both Images and Documents filters. All three platforms can easily elide the image options behind a single bool.
  implication: UI change is surgical — add `model_supports_vision(model_id)` read on the selected conversation's model, wrap the image-related menu entries in `if vision_supported`. No layout changes required.

- timestamp: 2026-04-19
  checked: Implementation + `cargo test -p mango_core --lib` + `cargo build -p mango_core -p mango-desktop`
  found: 284 tests pass (was 279; +3 capability tests + 2 AttachImage guard tests). Desktop + core build clean. UniFFI bindings manually patched following the dcc3066 pattern (checksum 37098 captured via ctypes introspection of libmango_core.so).
  implication: Rust-side gating is complete and tested. All three UIs wired to the new function. Ready for human on-device verification.

## Follow-up Resolution

root_cause: No per-model vision capability metadata existed anywhere in the codebase. The AttachImage handler and all three UIs unconditionally offered photo attachment regardless of whether the selected model could process images. Result: users could silently send images to text-only models (llama3, kimi, gpt-oss) and get confused replies like "Please provide the photo".

fix:
  - Added `rust/src/llm/capabilities.rs` with a pure `is_vision_model(&str) -> bool` pattern-match function covering Gemma 3/4, Qwen-VL, LLaVA, Pixtral, Llama-Vision, GPT-4 vision variants, Claude 3/4, Gemini 1.5/2.
  - Exposed via UniFFI as `model_supports_vision(model_id: String) -> bool` and re-exported at crate root as `mango_core::is_vision_model`.
  - Defense-in-depth: `AttachImage` handler in `rust/src/lib.rs` now resolves the current conversation's `model_id` and rejects non-vision models with `last_error`, keeping `pending_image_attachment = None`.
  - iOS `ChatView.swift`: wrapped Take Photo / Choose Photo buttons in `if currentModelSupportsVision`, using a computed property that calls `modelSupportsVision(modelId:)`.
  - Android `ChatScreen.kt`: derived `showImageOptions` from the current conversation's `modelId` via `modelSupportsVision(...)`, wraps both TextButtons.
  - Desktop `main.rs`: resolves current model, sets `allow_images`, toggles the rfd FileDialog `Images` filter accordingly.
  - Manually patched `ios/Bindings/mango_core.swift`, `ios/Bindings/mango_coreFFI.h`, `android/.../mango_core.kt` following the dcc3066 pattern (uniffi 0.29 bindgen is broken for this codebase due to preexisting `Result<_, String>` usage). Checksum 37098 captured via ctypes introspection.

verification:
  - `cargo test -p mango_core --lib` → 284 passed (was 279; +3 `capabilities.rs` tests + 2 AttachImage guard tests).
  - `cargo build -p mango_core -p mango-desktop` → clean, no warnings.
  - New tests: `vision_models_return_true`, `text_only_models_return_false`, `matching_is_case_insensitive`, `attach_image_rejected_for_non_vision_model`, `attach_image_allowed_for_vision_model`.
  - **Device verification REQUIRED** across iOS/Android/Desktop to confirm the UI actually hides photo options for text-only models.

files_changed:
  - rust/src/llm/capabilities.rs (new)
  - rust/src/llm/mod.rs
  - rust/src/lib.rs
  - ios/Bindings/mango_core.swift
  - ios/Bindings/mango_coreFFI.h
  - ios/Mango/Mango/ChatView.swift
  - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
  - android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
  - desktop/iced/src/main.rs


## Bulk Re-Verification (2026-07-28)

**Verdict:** FIXED-IN-CODE
**Evidence:** run_streaming_chat_completion_from_api_messages tinfoil_secure.rs:304, ppq_private.rs:228
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
