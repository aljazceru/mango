---
quick_id: 260421-tg6
status: executed
duration: ~20min
tasks_completed: 2
checkpoint: human-verify (automated portion complete, manual E2E pending)
commits:
  - f0856e4 test(quick/260421-tg6): add failing tests for markdown conversation exporter
  - ec88a86 feat(quick/260421-tg6): add core markdown conversation exporter
  - 22d9252 feat(quick/260421-tg6): wire Desktop chat menu to export-as-markdown
key-files:
  created:
    - rust/tests/export_markdown_test.rs
  modified:
    - rust/src/lib.rs
    - desktop/iced/src/views/chat.rs
    - desktop/iced/src/main.rs
requirements: [TG6-01]
---

# Quick Task 260421-tg6: Export Chat to Markdown File — Summary

One-liner: Rust-core pure markdown renderer for conversations exposed via a
`export_conversation_markdown` FFI method, wired into the Desktop chat `···`
menu with `rfd::FileDialog` save flow — strictly on-device, no network.

## What Shipped

### Rust core (`rust/src/lib.rs`)

- **`ExportMessage`** — public struct (role/content/image_path) that decouples
  the formatter from `persistence::queries::MessageRow` (which is crate-private).
- **`format_conversation_as_markdown_with_now(title, messages, now_rfc3339)`** —
  pure, deterministic renderer. Outputs `# <title>` (fallback
  `# Untitled conversation`), `_Exported <RFC3339>_`, `## User`/`## Assistant`/
  `## System`/title-cased-other headings, verbatim content, `_[image attachment]_`
  marker when `image_path` is `Some`, single trailing newline.
- **`format_conversation_as_markdown(title, messages)`** — thin wrapper that
  fills the timestamp from `chrono::Utc::now().to_rfc3339()`.
- **`CoreMsg::ExportConversationMarkdown { conversation_id, reply }`** — new
  actor message. Handler looks up the title from
  `actor_state.app_state.conversations`, calls
  `persistence::queries::list_messages`, maps rows to `ExportMessage`, and
  replies with the rendered string. Read-only — no AppState mutation, no
  emit (follows the `ReadEncryptedImage` pattern).
- **`FfiApp::export_conversation_markdown(conversation_id) -> Result<String, FfiError>`** —
  synchronous query-style FFI method that routes through the actor thread via
  a bounded flume reply channel.

### Tests (`rust/tests/export_markdown_test.rs`)

5 integration tests — all pass under `cargo test -p mango_core --test
export_markdown_test`:

1. `test_format_empty_conversation` — title + metadata line, no `##` headings
2. `test_format_user_assistant_exchange` — both role blocks, correct ordering
3. `test_format_empty_title_falls_back` — `# Untitled conversation`
4. `test_format_system_and_image_marker` — System heading + image marker placed
   AFTER user content
5. `test_format_unknown_role_title_cased` — `tool` → `## Tool`

### Desktop (iced) wire-up

- **`desktop/iced/src/views/chat.rs`** — new `export_btn` rendered alongside
  `docs_btn` / `tools_btn` in the conversation-menu row. Disabled (grey `muted`
  color, no `on_press`) when `current_conversation_id` is `None` or
  `state.messages` is empty.
- **`desktop/iced/src/main.rs`** —
  - `Message::ExportConversationMarkdown` and
    `Message::ExportMarkdownReady { result: Result<Option<PathBuf>, String> }`
    variants.
  - Handler renders markdown via `ffi.export_conversation_markdown`, runs
    `rfd::FileDialog::save_file()` + `std::fs::write` inside
    `tokio::task::spawn_blocking` on a `Task::perform`.
  - `sanitize_filename` helper (ASCII alnum / `_` / `-` / whitespace→`_`,
    collapse runs, trim, 60-char cap, fallback `"conversation"`).
  - Success toast `"Exported to {path}"`; failure toast `"Export failed: {reason}"`;
    silent on user-cancelled dialog.

## Verification

- `cargo test -p mango_core --test export_markdown_test` — **5/5 passed**
- `cargo check -p mango_core` — **clean**
- `cargo check --workspace` — **clean** (only pre-existing `tee_type_to_str`
  dead-code warning, unrelated to this change)
- `cargo build -p mango-desktop` — **clean** (same pre-existing warning)
- Privacy grep — `grep "reqwest|http|fetch|url" rust/src/lib.rs | grep -i export`
  returns **nothing**. Export path is purely local.

### Deferred — human-verify checkpoint

Task 3 in the plan is a `checkpoint:human-verify`. Per executor orchestration
rules, automated verification is complete. Manual E2E steps remaining (per
the plan's `<how-to-verify>` block):

1. Run `just run-desktop` (or `cargo run -p mango-desktop`).
2. Open an existing conversation with ≥2 messages → expand `···` menu →
   confirm **Export as Markdown** button renders alongside Docs / Tools /
   Instructions.
3. Click it → confirm native Save dialog opens with `<sanitized-title>.md`
   pre-filled.
4. Save → confirm success toast with saved path.
5. Open the file → verify `# <title>`, `_Exported ..._` line, `## User` /
   `## Assistant` (/ `## System` if any) blocks in order, content intact,
   `_[image attachment]_` after any image message.
6. On an empty conversation, confirm button is disabled / no-ops.
7. Click Export then cancel the dialog → no toast, no crash, no partial file.

## Platforms Not Yet Wired

Per the plan's explicit scope decision, iOS and Android are deliberately
out-of-scope for this quick task. Both reuse the same
`FfiApp.export_conversation_markdown` method — the follow-ups are ~10 lines
each:

### iOS (Swift, SwiftUI)

```swift
// In ChatView's conversation menu:
if let cid = appState.currentConversationId {
    ShareLink(
        item: (try? ffiApp.exportConversationMarkdown(conversationId: cid)) ?? "",
        preview: SharePreview("\(conversationTitle).md")
    ) { Label("Export as Markdown", systemImage: "square.and.arrow.up") }
}
```
For a real `.md` file (not a string), write to `FileManager.default
.temporaryDirectory.appendingPathComponent("\(sanitized).md")` first, then
`ShareLink(item: url)`.

### Android (Kotlin, Jetpack Compose)

Use `ActivityResultContracts.CreateDocument("text/markdown")`:

```kotlin
val launcher = rememberLauncherForActivityResult(
    ActivityResultContracts.CreateDocument("text/markdown")
) { uri ->
    uri?.let {
        val md = ffiApp.exportConversationMarkdown(currentConversationId)
        context.contentResolver.openOutputStream(it)?.use { os ->
            os.write(md.toByteArray())
        }
    }
}
// In the conversation menu:
DropdownMenuItem(
    text = { Text("Export as Markdown") },
    onClick = { launcher.launch("${sanitizedTitle}.md") }
)
```

## Deviations from Plan

None. The plan was followed as written:
- TDD flow: RED tests → RED commit → GREEN implementation → GREEN commit.
- Introduced `ExportMessage` as a local struct (plan anticipated this — the
  `persistence` module is `mod persistence;` (private) at the lib.rs root,
  so integration tests cannot see `MessageRow` directly).
- The actor handler maps `MessageRow → ExportMessage` before invoking the
  formatter, keeping the formatter decoupled from persistence.
- `rfd::FileDialog` invoked via `tokio::task::spawn_blocking` inside
  `Task::perform`, matching the `PickDocumentFile` pattern (main.rs line ~1143).
- `AppAction::ShowToast` used for success/failure surfacing; no new toast
  mechanism.
- No `AppAction` variant added for export (it is a read, not a mutation).

## Self-Check: PASSED

- `rust/tests/export_markdown_test.rs` exists (5 tests, all passing).
- `rust/src/lib.rs` contains `pub fn export_conversation_markdown` — verified.
- `rust/src/lib.rs` contains `CoreMsg::ExportConversationMarkdown` — verified.
- `desktop/iced/src/views/chat.rs` contains `Export` button — verified.
- `desktop/iced/src/main.rs` contains `export_conversation_markdown` call
  and `sanitize_filename` helper — verified.
- Commits f0856e4, ec88a86, 22d9252 all present in `git log` — verified.
