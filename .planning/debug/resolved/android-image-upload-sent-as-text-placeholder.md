---
status: resolved
trigger: "On Android, attaching an image via paperclip and sending to vision model results in model receiving [Image: filename] as plain text, not multipart image_url content part"
created: 2026-04-19
updated: 2026-04-19
---

## Current Focus

hypothesis: (Candidate 2 — strongest): The multipart user message's TEXT content part contains the SQLite-placeholder-augmented text, not the raw user input. Specifically `build_user_message_with_image(actor_state, &final_text)` at rust/src/lib.rs:2093 passes `final_text` which is `"{user_text}\n\n[Image: {filename}]"` — the placeholder version meant only for SQLite persistence. The text part of the OpenAI-multipart request therefore carries the literal string "[Image: camera_1776582758498.jpg]" instead of the user's actual question, which is exactly what is visible in the chat bubble. Also the outgoing request text ends with a bracketed filename placeholder that the model can interpret as a truncated/missing attachment reference — plausible cause of "appears cut off" response.
test: Grep-traced final_text construction and the build_user_message_with_image call-site; verified unit test test_send_message_with_image passes a raw "describe" string, so the integration mis-pass of final_text is NOT covered by any existing test.
expecting: Fix is to pass the RAW user text to build_user_message_with_image (either derive it from the original `text` parameter before placeholder augmentation, or strip the "\n\n[Image: ...]" suffix before the multipart build). The placeholder belongs only on the SQLite row and the UiMessage, not on the wire.
next_action: Return diagnosis to caller (goal=find_root_cause_only).

## Symptoms

expected: Android paperclip → pick/capture image → send "whats in the photo" → model receives multipart ChatCompletionRequestUserMessageContent::Array with image_url data URL + text part, and describes the image.
actual: Chat bubble shows attachment pill "camera_1776582758498.jpg" plus user text plus inline fallback line "[Image: camera_1776582758498.jpg]". Model (gemma4-31b) replies that message appears cut off.
errors: No crash. No visible error. Silent wrong behavior.
reproduction: Android app → chat with gemma4-31b → paperclip → Take Photo / Choose Photo → type "whats in the photo" → send.
started: Just implemented in Phase 31. Never verified on device.

## Eliminated

- hypothesis: "Android launcher wired to AttachFile instead of AttachImage"
  evidence: android/app/.../ChatScreen.kt:148-181 — both galleryLauncher (PickVisualMedia) and cameraLauncher (TakePicture) call onAttachImage(name, absolutePath, mime). MainApp.kt:54 wires onAttachImage → AppAction.AttachImage. Dispatch is correct.
  timestamp: 2026-04-19

- hypothesis: "AttachImage handler drops the attachment or validation fails silently"
  evidence: rust/src/lib.rs:4274-4313 — handler validates mime (jpeg/png), absolute path, 50 MB cap, and on success sets pending_image_attachment AND app_state.pending_attachment (with is_image: true). The user sees the attachment pill in the compose bar — proof the handler succeeded.
  timestamp: 2026-04-19

- hypothesis: "prepare_image_for_api fails and falls back to text"
  evidence: rust/src/lib.rs:2097-2105 — on prepare_image failure the error is written to last_error, pending_image_attachment is cleared, busy_state returns to Idle, and the function early-returns WITHOUT sending anything to the LLM. Since the user got a model response, this path was not hit.
  timestamp: 2026-04-19

## Evidence

- timestamp: 2026-04-19
  checked: rust/src/lib.rs:1765-1787 — has_image_attachment branch in do_send_message
  found: `final_text` is set to `"{text}\n\n[Image: {filename}]"` (or the bare placeholder if text is empty) for the image case. This variable is then used both for SQLite persistence (line 1798) AND for the UiMessage (line 1811) AND for the multipart text part (line 2093).
  implication: The SAME placeholder-augmented string that goes to SQLite is also sent as the multipart text content part. That is the smoking gun — the wire payload contains the literal text "[Image: camera_1776582758498.jpg]" immediately after the user question, exactly mirroring what is rendered in the chat bubble.

- timestamp: 2026-04-19
  checked: rust/src/lib.rs:1103-1138 — build_user_message_with_image
  found: Function uses `user_text.to_string()` as the Text content part verbatim (line 1119). It does not strip, trim, or otherwise clean the argument.
  implication: Whatever the caller passes is what the model sees.

- timestamp: 2026-04-19
  checked: rust/src/lib.rs:2093 — the call site
  found: `build_user_message_with_image(actor_state, &final_text)` — passes the placeholder-augmented string, not the raw user `text` parameter from do_send_message.
  implication: The text part of the multipart request is "whats in the photo\n\n[Image: camera_1776582758498.jpg]" rather than the intended "whats in the photo".

- timestamp: 2026-04-19
  checked: rust/src/lib.rs:7145-7207 — IMG-02/IMG-03 unit tests
  found: test_send_message_with_image passes the clean string "describe" and asserts only that `"type":"image_url"` and `data:image/jpeg;base64,` appear in the JSON. It does NOT exercise do_send_message and therefore does NOT see the `final_text` mis-pass. The Phase 31 verification was unit-level only; the integration path is untested.
  implication: Classic "green unit tests, broken integration" — this bug is invisible to the existing cargo test suite and was locked in by auto-mode approving the human-verify checkpoint without a device run.

- timestamp: 2026-04-19
  checked: rust/src/llm/streaming.rs:336-378 — api_messages_to_chat_messages (alternative hypothesis)
  found: For Tinfoil (ProviderTransportKind::TinfoilSecure) and PPQ /private (PpqPrivateE2ee) transports, this conversion silently DROPS any User message whose content is `ChatCompletionRequestUserMessageContent::Array` (returns None in filter_map at lines 351-357). A multipart image message disappears entirely from the request.
  implication: Secondary bug — any future attempt to send an image through Tinfoil or PPQ's native private transport would leave the final user turn missing from the request. Not the root cause today (gemma4-31b is a custom-backend model, not Tinfoil/PPQ), but a latent defect that will surface when image attachments are enabled for those providers.

- timestamp: 2026-04-19
  checked: rust/src/lib.rs:1974-2046 — tools branch in do_send_message (alternative hypothesis)
  found: The Phase 27 chat tools branch runs BEFORE the has_image_attachment branch at line 2055, and it early-returns (line 2043) without consuming `pending_image_attachment` and without sending any multipart content. The `api_messages` it builds use `msg.content.as_str()` which for the image turn is `final_text` (text + "[Image: ...]" placeholder).
  implication: Tertiary bug — if a user enables tools for a conversation and also attaches an image, the image is silently dropped, ONLY the placeholder text is sent, AND `pending_image_attachment` remains in actor state for the NEXT send. Not the root cause today (new conversations default tools_enabled=false per line 4035) but a latent defect for tool-enabled conversations.

## Resolution

root_cause: do_send_message passes `final_text` (the SQLite-persistence string with an appended "[Image: {filename}]" placeholder) to build_user_message_with_image at rust/src/lib.rs:2093. The placeholder is intentional for the SQLite `content` column and the UiMessage rendering (plan 31-01 decision, see 31-01-SUMMARY.md "Auto-fixed Issues" #4) — but it must NOT cross the wire. The multipart user message's Text content part therefore carries the literal "[Image: camera_1776582758498.jpg]" instead of the user's raw question. This is exactly what the chat bubble shows, because the bubble and the API request share the same string.

fix: Applied in three atomic commits:

1. **Primary (d66499f)** — `fix(31): don't leak [Image:] placeholder into multipart text content`
   Split persistence text from wire text in `do_send_message`. The image branch now returns a 4-tuple `(final_text, api_text, has_attachment, attachment_name)`. `final_text` keeps the `"{text}\n\n[Image: {filename}]"` placeholder for SQLite + UiMessage; `api_text` carries the raw user input. The call site at rust/src/lib.rs passes `&api_text` to `build_user_message_with_image`. Text-attachment branch keeps `api_text == final_text` (no change in behavior). Text-only branch keeps `api_text == text`.
   Regression test added: `test_build_user_message_with_image_does_not_leak_placeholder` asserts the wire JSON contains the raw user text, does NOT contain `"[Image:"`, and does NOT leak the client-side filename.

2. **Latent Array drop (4dec88c)** — `fix(31): preserve Array message text on Tinfoil/PPQ transports`
   `api_messages_to_chat_messages` in rust/src/llm/streaming.rs now handles `ChatCompletionRequestUserMessageContent::Array` by joining the Text content parts into a single string and dropping image_url parts. TODO comment flags the image drop as interim; propagate multipart once Tinfoil/PPQ support vision.
   Tests added: `array_user_message_preserves_text_and_drops_image`, `text_user_message_preserved_verbatim`.

3. **Latent tools-branch image drop (8c78d50)** — `fix(31): route image-bearing turns around the tools branch`
   Guard condition on the Phase 27 chat tools branch now includes `&& !has_image_attachment`. Image-bearing turns bypass the tools branch and fall through to the multipart image branch, which consumes `pending_image_attachment` properly. Comment explains the precedence: vision takes precedence over tools for that turn.

Test suite: `cargo test -p mango_core` → 274 passed, 0 failed, 10 ignored (up from 271 baseline; 3 new regression tests).

verification:
- Unit-level: all 5 image_red_tests pass (including new no-leak regression test); 2 new streaming tests pass; full suite green with no regressions.
- **Device re-test REQUIRED (outstanding):** The original bug was observed on a physical Android device with gemma4-31b via the paperclip → attach → send flow. Self-verification here is unit-only. Once these commits are built into a fresh Android APK, re-run the repro: attach photo → type "whats in the photo" → send → confirm (a) chat bubble still shows the "[Image: ...]" line (placeholder persistence unchanged), (b) the model actually describes the image (no more "appears cut off" reply). If the wire request can be inspected (e.g., via a proxy), confirm the last user message's `content[0].text` equals "whats in the photo" with no bracketed placeholder.

files_changed:
  - rust/src/lib.rs (fixes 1 and 3, plus regression test)
  - rust/src/llm/streaming.rs (fix 2, plus 2 tests)

commits:
  - d66499f fix(31): don't leak [Image:] placeholder into multipart text content
  - 4dec88c fix(31): preserve Array message text on Tinfoil/PPQ transports
  - 8c78d50 fix(31): route image-bearing turns around the tools branch

## Bulk Re-Verification (2026-07-28)

**Verdict:** ALREADY-RESOLVED
**Action:** Confirmed status during bulk archive sweep; moved to resolved/.
