---
phase: 31
plan: 00
subsystem: rust-core
tags: [multimodal, images, tdd-wave0, red, dependencies, image-crate]
requires:
  - rust/Cargo.toml (existing [dependencies] block)
  - rust/src/lib.rs (existing ActorState, AppAction, AttachmentInfo)
provides:
  - image = "0.25" on the classpath (jpeg+png features only)
  - Four RED unit tests pinning the contract for IMG-01..04
affects:
  - Plan 31-01 (must make these tests GREEN by adding the referenced symbols)
tech-stack:
  added:
    - image 0.25 (default-features = false, features = ["jpeg", "png"])
  patterns:
    - Wave-0 TDD: land failing tests first so the implementer cannot drift
key-files:
  created: []
  modified:
    - rust/Cargo.toml
    - rust/src/lib.rs
decisions:
  - image crate uses minimal feature set (jpeg+png) to avoid gif/tiff/webp bloat
  - RED tests live in a dedicated inline `mod image_red_tests` in lib.rs rather than under rust/src/tests/ to keep acceptance check (`grep ... rust/src/lib.rs`) trivial
  - Test-only helper fns (default_actor_state_for_image_tests, handle_attach_image_for_test) are stubbed with unimplemented!() so the test module parses as far as possible; 31-01 replaces them with real helpers
metrics:
  duration: 6min
  tasks: 2
  files: 2
  completed: 2026-04-19
---

# Phase 31 Plan 00: Wave-0 Image Pipeline Dependencies + Failing Tests Summary

**One-liner:** Added `image = "0.25"` (jpeg+png only) and four failing unit tests that pin down the observable behavior of `prepare_image_for_api`, `build_user_message_with_image`, `PendingImageAttachment`, `AppAction::AttachImage`, and `AttachmentInfo::is_image` before Plan 31-01 implements them.

## Outcome

- `cargo tree -p mango_core --depth 1` now shows `image v0.25.10` on the classpath.
- `cargo check -p mango_core` still passes (no production code touched).
- `cargo test -p mango_core --no-run` FAILS with 8 compile errors that name exactly the five symbols Plan 31-01 must introduce. This is the intentional RED state that gates drift.

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Add image crate dep | 79b68f7 | rust/Cargo.toml, Cargo.lock |
| 2 | Wave-0 failing unit tests for IMG-01..04 | 02628cc | rust/src/lib.rs |

## Contract Locked for Plan 31-01

Plan 31-01 must make the following observable:

1. **`prepare_image_for_api(path: &str) -> Result<String, _>`** — takes a path to a PNG/JPEG/etc. image, returns a `data:image/jpeg;base64,<payload>` string whose decoded JPEG has longest side ≤ 1536 px.
2. **`build_user_message_with_image(actor_state: &ActorState, text: &str) -> Result<ChatCompletionRequestMessage, _>`**
   - If `actor_state.pending_image_attachment.is_some()`, produces a multipart user message whose JSON contains both `"type":"image_url"` and a `data:image/jpeg;base64,` data URL.
   - If `None`, produces a plain text user message; the JSON must NOT contain `"type":"image_url"`.
3. **`PendingImageAttachment`** with fields `filename: String`, `file_path: String`, `mime_type: String`.
4. **`AppAction::AttachImage { filename, file_path, mime_type }`** — when dispatched, sets `actor_state.pending_image_attachment` AND publishes an `AttachmentInfo { filename, size_display, is_image: true }` into `actor_state.app_state.pending_attachment`.
5. **`AttachmentInfo::is_image: bool`** field added (existing UniFFI record gains one field — binding regen is 31-02's job).

Plan 31-01 must also replace the two test-only `unimplemented!()` stubs (`default_actor_state_for_image_tests`, `handle_attach_image_for_test`) with working helpers so the four tests actually run.

## Verification Results

### Task 1 — image dep

- `grep -E '^image = \{ version = "0.25"' rust/Cargo.toml` → matches.
- `cargo tree -p mango_core --depth 1 | grep image` → `image v0.25.10`.
- `cargo check -p mango_core` → success (2.75s on top of warm cache).

### Task 2 — RED tests

- `grep -c 'fn test_prepare_image_jpeg\|fn test_send_message_with_image\|fn test_send_message_text_only\|fn test_attach_image_action' rust/src/lib.rs` → `4`.
- `cargo test -p mango_core --no-run` → FAIL with 8 errors (E0609, E0425, E0599, E0422) naming: `pending_image_attachment`, `PendingImageAttachment`, `build_user_message_with_image`, `AppAction::AttachImage`, `AttachmentInfo.is_image`. Intentional RED — exact symbol list 31-01 must add.
- Git diff confirms only an append to lib.rs (lines 6717+); no production code within `#[cfg(test)]` was modified.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Plan references non-existent inline `mod tests { ... }` in lib.rs**

- **Found during:** Task 2 read-first step.
- **Issue:** The plan's `<read_first>` says "append to the last inline test module" in lib.rs. lib.rs actually ends with `#[cfg(test)] mod tests;` pointing to the external directory `rust/src/tests/`, not an inline module.
- **Fix:** Added a new inline module `#[cfg(test)] mod image_red_tests { ... }` at the end of lib.rs. This satisfies the plan's acceptance criterion (`grep ... rust/src/lib.rs` finds all four `fn test_*`), keeps the tests in the file the plan's frontmatter promises to modify (`files_modified: rust/src/lib.rs`), and avoids creating a new file under rust/src/tests/ whose wiring in `tests/mod.rs` 31-01 would need to undo.
- **Files modified:** rust/src/lib.rs (append only).
- **Commit:** 02628cc.

**2. [Rule 3 - Blocking] Test-only ActorState constructor not exposed**

- **Found during:** Task 2 drafting.
- **Issue:** Tests need to build an `ActorState` with a `pending_image_attachment` field that does not yet exist. The existing test files under `rust/src/tests/` likely have local builders, but nothing pub-visible from the main lib scope.
- **Fix:** Added two test-only helpers inside the new `image_red_tests` module — `default_actor_state_for_image_tests()` and `handle_attach_image_for_test()` — that currently `unimplemented!()`. Plan 31-01 is expected to replace them with real helpers (or route the tests through actor-state builders it introduces). This keeps the module syntactically valid enough to surface the real RED signal (unresolved symbols + missing fields) rather than masking it behind "cannot find ActorState constructor" noise.
- **Rationale:** Documented explicitly in the SUMMARY under "Contract Locked for Plan 31-01" so 31-01 cannot miss the follow-up.

### Auth Gates

None — no remote access needed.

## Known Stubs

Two intentional stubs live in `rust/src/lib.rs::image_red_tests`:

- `default_actor_state_for_image_tests()` → `unimplemented!()`
- `handle_attach_image_for_test()` → `unimplemented!()`

Both are test-only (`#[cfg(test)]`) and must be resolved by Plan 31-01 as part of turning the RED tests GREEN. They do not affect production code paths or UniFFI bindings.

## Self-Check: PASSED

- rust/Cargo.toml contains `image = { version = "0.25"` on line 35 — FOUND.
- rust/src/lib.rs contains `fn test_prepare_image_jpeg` — FOUND.
- rust/src/lib.rs contains `fn test_send_message_with_image` — FOUND.
- rust/src/lib.rs contains `fn test_send_message_text_only` — FOUND.
- rust/src/lib.rs contains `fn test_attach_image_action` — FOUND.
- Commit 79b68f7 — FOUND.
- Commit 02628cc — FOUND.
- `cargo check -p mango_core` — passes.
- `cargo test -p mango_core --no-run` — fails with expected RED symbol errors.
