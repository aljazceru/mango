---
quick_id: 260419-jz5
description: Feature request: allow changing chat title manually
date: 2026-04-19
status: executed
---

# Quick Task 260419-jz5 — Summary

## Goal

Expose manual chat title editing from the chat top bar. The backend (`AppAction::RenameConversation`, `persistence::queries::rename_conversation`) and UniFFI bindings already existed; rename UI existed only on the conversation list. This task adds a tap-on-title affordance inside the chat view on all three platforms.

## Tasks Executed

### Task 1 — Rust core defensive guard (commits 3be7429 + 5620c3f)

TDD'd the trim + empty/whitespace guard into the `RenameConversation` handler. Empty or whitespace-only input is a no-op; leading/trailing whitespace is trimmed before persisting.

- `rust/src/lib.rs` — handler update
- `rust/src/tests/chat.rs` — 4 unit tests (rename persists, trim leading/trailing, empty no-op, whitespace-only no-op)

Verification: `cargo test --lib rename_conversation` → 4 passed.

### Task 2 — Tap-on-title rename UI (commit 52fc9f8)

Three platforms, consistent UX: tap the title → edit affordance dispatches `RenameConversation { id, trimmed }`. Empty/whitespace falls through to the core guard.

- **iOS** (`ChatView.swift`, `ContentView.swift`) — `.alert` with text field + Save/Cancel, wired via callback threaded from `ContentView`.
- **Android** (`ChatScreen.kt`) — Compose `AlertDialog` with `OutlinedTextField`, dispatches `AppAction.RenameConversation`.
- **Desktop** (`desktop/iced/src/views/chat.rs`, `main.rs`) — inline `text_input` reusing existing sidebar rename plumbing.

Verification:
- `cargo build -p mango_core` → clean
- `cargo check` (iced) → clean
- `./gradlew :app:compileDebugKotlin` → BUILD SUCCESSFUL
- No UniFFI regeneration needed (no new actions/types).

### Task 3 — Human-verify checkpoint (deferred)

Skipped during automated execution. User verifies on device per the plan's checkpoint criteria.

## Commits

- `3be7429` test(quick/260419-jz5): add failing tests for empty/whitespace/trim rename guard
- `5620c3f` feat(quick/260419-jz5): defensive empty/whitespace/trim guard on RenameConversation
- `52fc9f8` feat(quick/260419-jz5): tap-on-title rename UI in chat header (iOS/Android/Desktop)

## Files Changed

- `rust/src/lib.rs`
- `rust/src/tests/chat.rs`
- `android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt`
- `ios/Mango/Mango/ChatView.swift`
- `ios/Mango/Mango/ContentView.swift`
- `desktop/iced/src/views/chat.rs`
- `desktop/iced/src/main.rs`

## Device Verification Checklist

1. **iOS** — open a conversation, tap the title in the top bar, type a new name, Save → title updates in view and conversation list; restart app → title persists. Cancel → no change. Empty/whitespace → no change.
2. **Android** — same flow via Compose dialog.
3. **Desktop** — click title, inline input, Enter to save / Esc to cancel.
