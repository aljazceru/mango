---
phase: quick
plan: 260419-ece
subsystem: image-persistence
tags: [encryption, images, uniffi, android, ios, desktop, sqlite]
dependency_graph:
  requires: [file_crypto, persistence, uniffi-bindings]
  provides: [encrypted-image-persistence, thumbnail-rendering]
  affects: [chat-ui-ios, chat-ui-android, chat-ui-desktop]
tech_stack:
  added: [iced image feature]
  patterns: [MGO1 encrypt-on-write, decrypt-on-read, actor oneshot channel, LaunchedEffect thumbnail, iced Task::perform thumbnail]
key_files:
  created:
    - rust/src/tests/encrypted_image_persistence.rs
  modified:
    - rust/src/persistence/schema.rs
    - rust/src/persistence/queries.rs
    - rust/src/lib.rs
    - rust/src/tests/mod.rs
    - rust/src/tests/persistence.rs
    - rust/src/tests/rag.rs
    - rust/src/tests/attestation_integration.rs
    - ios/Bindings/mango_coreFFI.h
    - ios/Bindings/mango_core.swift
    - ios/Mango/Mango/MessageBubbleView.swift
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
    - android/app/src/main/java/dev/disobey/mango/AppManager.kt
    - android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MessageBubble.kt
    - desktop/iced/Cargo.toml
    - desktop/iced/src/main.rs
    - desktop/iced/src/views/chat.rs
decisions:
  - "Reused file_crypto::encrypt_file/decrypt_file exclusively — no parallel AES path"
  - "UniFFI bindgen silent in worktree; manually patched all three binding files (FFI header, Swift, Kotlin)"
  - "Checksum for read_encrypted_image obtained via ctypes Python call to shared lib: 58567"
  - "Desktop uses Task::perform + image_cache HashMap rather than async in view (iced pattern)"
  - "Dek deref: pass dek directly to encrypt_file/decrypt_file (Zeroizing<[u8;32]> derefs to [u8;32])"
metrics:
  duration: ~3 hours
  completed: 2026-04-19
  tasks_completed: 3
  files_modified: 19
---

# Quick Task 260419-ece: Encrypted Image Persistence Summary

Encrypted image persist-on-send and decrypt-on-read with inline thumbnail rendering across all three platforms, using the existing MGO1 file format exclusively.

## What Was Built

**Task 1 — Rust core:**
- MIGRATION_V17: `ALTER TABLE messages ADD COLUMN image_path TEXT`
- `MessageRow.image_path: Option<String>` + `UiMessage.image_path: Option<String>` across UniFFI
- Encryption in `do_send_message`: reads source JPEG, calls `file_crypto::encrypt_file(dek, &bytes)`, writes to `{data_dir}/images/{msg_id}.jpg.mgo1`, stores path in DB
- `CoreMsg::ReadEncryptedImage { message_id, reply }` actor variant with oneshot flume channel
- `FfiApp::read_encrypted_image(message_id) -> Result<Vec<u8>, String>` FFI function
- 5 unit tests covering roundtrip, SQLite persistence, null path for text messages, encryption on send, and missing-DEK graceful fallback

**Task 2 — Bindings + mobile thumbnail rendering:**
- Manually patched `mango_coreFFI.h`, `mango_core.swift`, `mango_core.kt` (UniFFI bindgen produced no output from worktree)
- `MessageBubbleView.swift`: `@State thumbnail: UIImage?`, `.task(id: message.id)` async decrypt-on-read via `appManager.ffiApp.readEncryptedImage`, `Image(uiImage:)` at 240pt max width
- `MessageBubble.kt`: `LaunchedEffect(message.id)` + `BitmapFactory.decodeByteArray` on `Dispatchers.IO`, Compose `Image(bitmap.asImageBitmap())` at 240dp max width
- `AppManager.kt`: `readEncryptedImage()` delegate added
- `ChatScreen.kt` + `MainApp.kt`: `onReadEncryptedImage` callback threaded through

**Task 3 — Desktop iced thumbnail rendering:**
- Added `image` feature to iced in `desktop/iced/Cargo.toml`
- `App::Loaded.image_cache: HashMap<String, image::Handle>` for cached decoded thumbnails
- `Message::ThumbnailLoaded { message_id, handle }` variant
- `CoreUpdated` handler spawns `Task::perform(tokio::task::spawn_blocking(...))` for each new message with `image_path` not yet cached
- `chat_view` → `render_message` → `render_user_message` thread `image_cache` through; renders `iced_image::Image` at 240px fixed width above text content

## Deviations from Plan

**1. [Rule 1 - Bug] DEK deref fix**
- Found during: Task 1
- Issue: `dek.as_ref()` on `Zeroizing<[u8;32]>` yields `&[u8]` not `&[u8;32]`; `encrypt_file`/`decrypt_file` expect `&[u8;32]`
- Fix: Pass `dek` directly — `Zeroizing<[u8;32]>` implements `Deref<Target=[u8;32]>`, so `&*dek` is `&[u8;32]`
- Files: `rust/src/lib.rs`

**2. [Rule 3 - Blocking] UniFFI bindgen silent in worktree**
- Found during: Task 2
- Issue: `just bindings-swift` / `just bindings-kotlin` ran successfully (exit 0) but produced empty output (`modules: {}` in pipeline); worktree environment issue
- Fix: Manually applied all binding changes to the three binding files following exact existing patterns; obtained checksum 58567 via Python ctypes call to compiled shared lib
- Files: `ios/Bindings/mango_coreFFI.h`, `ios/Bindings/mango_core.swift`, `android/.../mango_core.kt`

**3. [Rule 2 - Missing] Non-exhaustive CoreMsg match**
- Found during: Task 1
- Issue: `attestation_integration.rs` has a non-exhaustive match on `CoreMsg`; adding a new variant broke compilation
- Fix: Added `CoreMsg::ReadEncryptedImage { .. } => panic!(...)` arm
- Files: `rust/src/tests/attestation_integration.rs`

**4. [Rule 2 - Missing] Hardcoded version numbers in tests**
- Found during: Task 1
- Issue: `persistence.rs` (4 sites) and `rag.rs` (1 site) hardcoded migration version 16; V17 incremented the version
- Fix: Updated all 5 sites to 17; added `image_path: None` to 2 MessageRow constructions
- Files: `rust/src/tests/persistence.rs`, `rust/src/tests/rag.rs`

## Commits

| Task | Hash    | Description |
|------|---------|-------------|
| 1    | e6b12f5 | feat(ece-01): Rust core — schema V17, encrypted image persistence, UniFFI image_path, decrypt FFI |
| 2    | dcc3066 | feat(260419-ece): wire UniFFI bindings + iOS/Android thumbnail rendering |
| 3    | a7c204b | feat(260419-ece): desktop iced thumbnail rendering for encrypted images |

## Verification

- `cargo test -p mango_core`: 279 passed, 0 failed
- `./gradlew :app:compileDebugKotlin`: BUILD SUCCESSFUL
- `cargo check --workspace`: Finished with no errors

## Known Stubs

None. All image_path data flows through to rendering on all three platforms.

## Threat Flags

None. Images are encrypted before any disk write using the existing DEK; no plaintext bytes are written to disk. The `images/` subdirectory lives inside the app sandbox alongside the existing DB.

## Self-Check: PASSED

- e6b12f5: found in git log
- dcc3066: found in git log
- a7c204b: found in git log
- `rust/src/tests/encrypted_image_persistence.rs`: exists
- `ios/Mango/Mango/MessageBubbleView.swift`: modified (thumbnail branch present)
- `android/.../MessageBubble.kt`: modified (LaunchedEffect thumbnail present)
- `desktop/iced/src/main.rs`: modified (image_cache, ThumbnailLoaded present)
