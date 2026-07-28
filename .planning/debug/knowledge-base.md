# GSD Debug Knowledge Base

Resolved debug sessions. Used by `gsd-debugger` to surface known-pattern hypotheses at the start of new investigations.

---

## attestation-vcek-429-and-cert-caching — 429 from AMD KDS downgrades Verified status; VCEK cert fetched on every run with no cache
- **Date:** 2026-03-26
- **Error patterns:** 429 Too Many Requests, VCEK, AMD KDS, kdsintf.amd.com, attestation status failed, Verified downgraded, CollateralFetch, NetworkError, periodic attestation, rate limit
- **Root cause:** Two bugs: (1) AttestationEvent::Failed unconditionally overwrote Verified status in lib.rs even for transient fetch errors (429, network), so any rate-limit hit from AMD KDS would clobber a previously Verified backend. (2) verify_snp_report fetched the VCEK DER certificate from AMD KDS on every invocation with no cache, causing repeated KDS requests during periodic re-attestation and triggering the 429s in the first place.
- **Fix:** Added AttestationError::is_transient() method (true for NetworkError + CollateralFetch, false for genuine verification failures). Added is_transient field to AttestationEvent::Failed. Revised stickiness guard in lib.rs: transient errors preserve Verified; genuine failures (QuoteVerification, NonceMismatch, JwtVerification) always downgrade. Added VcekCache (Arc<RwLock<HashMap<String,Vec<u8>>>>) in-memory cache passed into verify_snp_report, plus a vcek_cert_cache SQLite table (schema v9) for cross-process warmup.
- **Files changed:** rust/src/lib.rs, rust/src/attestation/task.rs, rust/src/attestation/mod.rs, rust/src/attestation/tdx.rs, rust/src/attestation/nvidia.rs, rust/src/attestation/error.rs, rust/src/persistence/schema.rs, rust/src/tests/attestation_types.rs, rust/src/tests/attestation_integration.rs
---

## android-saf-cant-use-folder-grapheneos — SAF folder picker shows "Can't use this folder" on Android 11+ / GrapheneOS due to null initial URI
- **Date:** 2026-04-20
- **Error patterns:** Can't use this folder, To protect your privacy, choose another folder, SAF, OpenDocumentTree, launcher.launch(null), DocumentsUI, GrapheneOS, Android 11, internal storage root, EXTRA_INITIAL_URI, FLAG_GRANT_WRITE_URI_PERMISSION, folder picker, directory source
- **Root cause:** DirectorySourcePicker.kt calls `launcher.launch(null)`, passing null as the initial URI to the OpenDocumentTree ActivityResultContract. DocumentsUI then opens at the internal storage root ("Pixel 9a"). Android 11+ (API 30+) blocks selecting ANY folder reached by navigating from the internal storage root, showing "Can't use this folder / To protect your privacy, choose another folder". Secondary bug: `takePersistableUriPermission` only captured `FLAG_GRANT_READ_URI_PERMISSION`, abandoning the write grant that OpenDocumentTree supplies.
- **Fix:** Pass `MediaStore.Downloads.EXTERNAL_CONTENT_URI` (API 29+) as the initial URI to `launcher.launch()` so DocumentsUI opens inside a permitted subtree. Added null fallback for API < 29. Also added `FLAG_GRANT_WRITE_URI_PERMISSION` to `takePersistableUriPermission` to persist both flags.
- **Files changed:** android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcePicker.kt
---

## directory-sources-ui-polish — DirectorySource row wraps buttons, hides full path, and lacks open-folder action
- **Date:** 2026-04-20
- **Error patterns:** button wraps, OutlinedButton overflow, Remove wraps two lines, displayName only, full path hidden, no open folder, DirectorySourceRow, tree URI, resolveTreeUri, DocumentsContract, shareddocuments, open::that
- **Root cause:** (1) Three OutlinedButtons with default 16dp horizontal content padding overflowed a narrow card on Android, causing the "Remove" label to wrap. (2) DirectorySourceSummary.path was absent from the UniFFI record (intentionally omitted as opaque handle, but path is a safe display string), so only displayName was shown on all platforms. (3) No open-folder action existed on any platform.
- **Fix:** Added `path: Option<String>` to DirectorySourceSummary in Rust lib.rs (UniFFI record + load_directory_sources_summary). Regenerated bindings for Kotlin and Swift. Android: reduced button content padding to PaddingValues(horizontal=10dp, vertical=4dp) with labelSmall text + 14dp icons; derives human-readable path from DocumentsContract.getTreeDocumentId().substringAfterLast(':'); added "Open" button with Intent.ACTION_VIEW. iOS: shows source.path under displayName; added "Open" button calling UIApplication.shared.open with shareddocuments URL. Desktop: shows src.path in build_source_row; added OpenFolder message + open::that(path) handler in main.rs.
- **Files changed:** rust/src/lib.rs, android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt, ios/Bindings/mango_core.swift, android/app/src/main/java/dev/disobey/mango/ui/DirectorySourcesScreen.kt, ios/Mango/Mango/DirectorySourcesView.swift, desktop/iced/src/views/directory_sources.rs, desktop/iced/src/main.rs
---

## tinfoil-streaming-timeout — Streaming times out because trailing slash in base_url produces double-slash URL
- **Date:** 2026-03-25
- **Error patterns:** Streaming timed out, chat completions request body is empty, failed to deserialize api response, expected value at line 1 column 1, StreamError, NetworkError, busy_state Idle
- **Root cause:** base_url stored in DB with trailing slash (e.g. 'https://inference.tinfoil.sh/v1/'). async-openai 0.33 constructs URLs as `format!("{}{}", api_base, "/chat/completions")`, producing a double slash (`/v1//chat/completions`). The provider returns a plain-text error instead of JSON; async-openai fails to deserialize it, leaving busy_state=Idle with no assistant message, causing polling loops to hit the timeout deadline.
- **Fix:** In `spawn_streaming_task`, apply `backend.base_url.trim_end_matches('/').to_string()` before passing to `OpenAIConfig::with_api_base`. (The same pattern was already used in `spawn_health_check`.)
- **Files changed:** rust/src/llm/streaming.rs
---

## desktop-new-chat-db-panic — NewConversation panics actor thread with "db unlocked" when dispatched while locked
- **Date:** 2026-04-21
- **Error patterns:** db unlocked, panicked at rust/src/lib.rs:4503, AppAction::NewConversation, actor thread panic, expect("db unlocked"), Case D, Screen::Locked, actor_state.db None, new chat crash, locked dispatch
- **Root cause:** AppAction::NewConversation handler in rust/src/lib.rs:4498 unwrapped actor_state.db with .expect("db unlocked") on every invocation. Dispatch is an async channel send with no screen gate, so any stray enqueue while db=None (lock transitions, future UI paths without lock checks) crashed the actor thread and took the whole process down. Latent hazard introduced in Phase 28 (lock screen), not a regression.
- **Fix:** Added an `if actor_state.db.is_none() { log::warn!(...); continue; }` guard at the top of the handler. Added headless regression test rust/src/tests/desktop_locked_new_conv.rs that reproduces the panic pre-fix and verifies actor liveness post-fix via rev-bump probe.
- **Files changed:** rust/src/lib.rs, rust/src/tests/desktop_locked_new_conv.rs, rust/src/tests/mod.rs, rust/src/tests/attestation_integration.rs
---

## desktop-bare-ui-no-onboarding — Desktop UI frozen at Home/empty-conversations; no lock screen, onboarding, or settings render
- **Date:** 2026-04-21
- **Error patterns:** bare UI, New Conversation button does nothing, no onboarding, no lock screen, no settings, empty chat list, desktop iced frozen state, state mirror not updating, CoreUpdated handler, Screen::Home stuck, AppState::default frozen, rev stuck at 0, IMG-07 regression, a7c204b
- **Root cause:** Commit a7c204b (Apr 19, IMG-07 thumbnail rendering) accidentally removed `*state = latest;` at the end of the Message::CoreUpdated handler in desktop/iced/src/main.rs. From that commit on, App::Loaded.state was set exactly once at startup (from manager.state() which returns AppState::default() before the actor's initial emit) and never updated. view() branches on state.router.current_screen, so the UI was permanently frozen at Screen::Home with empty conversations. Secondary issue: the pre-regression code had an `if latest.rev > state.rev` guard that was also wrong — initial emit has rev=0 on both sides, so the guard never fires on first launch.
- **Fix:** Restored `*state = latest;` at the end of the handler. Dropped the `if latest.rev > state.rev` guard so the initial emit runs (downstream logic is already idempotent — parsed_messages/streaming/thumbnails all use their own membership checks). Changed thumbnail loop to iterate `latest.messages` instead of stale `state.messages`.
- **Files changed:** desktop/iced/src/main.rs
---
