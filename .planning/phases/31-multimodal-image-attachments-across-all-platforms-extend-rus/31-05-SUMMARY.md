---
phase: 31
plan: 05
subsystem: desktop-iced
tags: [multimodal, images, desktop, iced, rfd, attach]
requires:
  - 31-01 landed AppAction::AttachImage + AttachmentInfo::is_image in mango_core
provides:
  - Desktop paperclip dispatches AppAction::AttachImage for jpg/jpeg/png files
  - Desktop paperclip preserves AppAction::AttachFile (text) path for txt/md/json/csv/log
  - Compose bar pill prefixes "[image]" label when pending attachment is an image
affects:
  - End-user desktop attach UX (single dialog, two code paths)
tech-stack:
  added: []
  patterns:
    - rfd::FileDialog add_filter groups (Attachable | Images | Text) for one-dialog UX
    - Extension-based MIME sniff (jpg/jpeg -> image/jpeg, png -> image/png)
    - Absolute-path canonicalize before dispatch (T-31-01 mitigation on the caller side)
key-files:
  created:
    - .planning/phases/31-multimodal-image-attachments-across-all-platforms-extend-rus/31-05-SUMMARY.md
  modified:
    - desktop/iced/src/main.rs
    - desktop/iced/src/views/chat.rs
decisions:
  - Kept the existing synchronous spawn_blocking + Task::perform shape; no new Message variant
  - Single rfd filter set combining both attachment categories instead of two separate paperclip buttons
  - MIME inferred from extension locally (no file-type sniff) — cheap and correct for the v1 allowlist
metrics:
  duration: 4min
  tasks: 2
  files: 2
  completed: 2026-04-19
---

# Phase 31 Plan 05: Desktop Image Attach Summary

**One-liner:** Desktop paperclip now accepts jpg/jpeg/png via a single `rfd::FileDialog` with Attachable/Images/Text filter groups, dispatching `AppAction::AttachImage` for images and preserving the existing text-file `AppAction::AttachFile` path; the compose-bar pill prefixes `[image]` when `AttachmentInfo.is_image` is true.

## Outcome

- IMG-05 (desktop image attach) and IMG-06 (pill image vs text distinction) satisfied.
- `cargo build` for `mango-desktop` succeeds (`Finished dev profile ... in 1m 30s`).
- Existing text-attachment flow is byte-for-byte unchanged — only reached when `is_image == false`.
- `AppAction::AttachImage` receives an absolute file path (canonicalized) and a literal `image/jpeg` or `image/png` MIME, matching the 31-01 allowlist (T-31-03) and absolute-path check (T-31-01).

## Tasks Completed

| # | Name | Commit | Files |
|---|------|--------|-------|
| 1 | Extend Message::AttachFile handler with image branch + update pill renderer | 681fc9c | desktop/iced/src/main.rs, desktop/iced/src/views/chat.rs |
| 2 | Manual verification on desktop (auto-approved per auto-mode) | — | — |

## Symbol Inventory

No new Rust types introduced. Desktop crate consumes existing `mango_core` symbols:

| Symbol | Kind | Location |
|--------|------|----------|
| `AppAction::AttachImage` | enum variant | rust/src/lib.rs ~line 484 |
| `AttachmentInfo::is_image` | bool field | rust/src/lib.rs ~line 165 |

## Verification Results

### Acceptance criteria — Task 1

- `grep -c 'AppAction::AttachImage' desktop/iced/src/main.rs` → **2** ✓ (≥ 1)
- `grep -cE '"jpg"|"jpeg"|"png"' desktop/iced/src/main.rs` → **4** ✓ (image filter present)
- `grep -c 'is_image' desktop/iced/src/views/chat.rs` → **1** ✓ (≥ 1)
- `cargo build` → **exit 0** ✓ (desktop/iced build finished in 1m 30s)

### Acceptance criteria — Task 2

- checkpoint:human-verify — auto-approved per auto-mode policy for this plan run.

## Deviations from Plan

### Auto-fixed Issues

None — plan executed as written. The plan's sketch showed an async `rfd::AsyncFileDialog`
variant with a separate `Message::AttachFileChosen(Option<PathBuf>)` message; the plan
explicitly said "Match the existing style in the file. If the current handler is
synchronous using blocking `rfd::FileDialog::new().pick_file()`, keep it synchronous —
just add the ext-branch logic inline." The existing handler already used
`tokio::task::spawn_blocking` with `rfd::FileDialog::new().pick_file()`, so the
existing style was preserved and the ext-branch was added inline. No new Message
variant was needed.

### Auth Gates

None — local file picker only.

## Known Stubs

None. Desktop AttachImage dispatch is fully wired to the mango_core action handler
landed in 31-01; that handler validates MIME (T-31-03), absolute path (T-31-01), and
50 MB size cap (T-31-02), and publishes `AttachmentInfo { is_image: true }` into
`AppState.pending_attachment` — which the compose-bar pill now renders with an
`[image]` prefix.

## Self-Check: PASSED

- desktop/iced/src/main.rs contains `AppAction::AttachImage` dispatch — FOUND.
- desktop/iced/src/main.rs contains `"jpg"`/`"jpeg"`/`"png"` filter entries — FOUND.
- desktop/iced/src/views/chat.rs contains `is_image` check — FOUND.
- `cargo build` (mango-desktop) exits 0 — CONFIRMED.
- Commit 681fc9c — FOUND on branch worktree-agent-a6e7f50a.
