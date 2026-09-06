# Independent review: first implementation pass

Reviewer: Codex, 2026-09-05. All items below must be addressed by Devin SWE-1.7 in YOLO mode before completion. Preserve existing user edits.

## R1 — P1: plaintext backup survives committed enrollment after crash

`promote_pending_auth` atomically deletes pending_auth, then SetupPin separately calls finalize_encrypted_storage. A crash between these operations leaves mango.db.plain_bak containing plaintext conversations. Startup only recovers when pending_auth exists, so it never finalizes this committed state. A cleanup unlink failure is also only logged and is never retried. This violates encryption-at-rest after successful unlock.

Add idempotent cleanup of enrollment-owned plaintext artifacts after a verified encrypted open with committed auth, including ordinary PIN unlock and Never/biometric paths. Preserve backup on failed verification. Make cleanup failure observable/retryable without lying about completion. Check duress/reset removes these new artifacts. Add a real file-backed test that stages main encrypted + active auth + no pending + backup, restarts/unlocks, asserts data intact and plaintext backup gone; also exercise cleanup failure/retry.

## R2 — P1: rollback/recovery errors discard the only pending auth

Startup and SetupPin use `let _ = recover.../rollback...` and then clear_pending_auth unconditionally. rollback_encrypted_to_plaintext returns Ok when backup is absent, even if the main database is encrypted, and removes the main file before attempting the backup rename. A restore permission/I/O failure can therefore leave a missing/encrypted main with neither usable active nor pending auth, followed by creation of an empty plaintext database on startup.

Preserve pending auth until restoration of the original data is confirmed; propagate recovery failures into a retryable blocked enrollment state. Do not create a fresh database when recovery has not succeeded. Avoid deleting the encrypted main before a fallible restore; preserve at least one valid file with its matching auth at every step. Treat missing backup explicitly. Add injected restore failure, promotion failure, missing backup, and recovery retry tests that verify original sample data and recoverable key material survive.

## R3 — P2: first-run continuation clears itself even on failed storage writes

load_post_unlock ignores set_setting and insert_conversation errors, fabricates a ConversationSummary, selects Chat, and clears pending_first_run anyway. Disk-full/read-only errors can thus lose the continuation and expose a chat absent from storage. Idempotence also needs to distinguish an existing row from another insert failure.

Persist the completion flag and first conversation transactionally in the main database; select/load actual persisted rows. Clear the continuation only after successful persistence. Retain retry state and surface errors on failed writes. Add an injected insertion/setting failure and repeated continuation retry test.

## R4 — P1: interruption coverage is incomplete

The eight recovery tests manually stage a few states and mostly use helper calls. They do not cover post-promotion/pre-cleanup, restore failure, cleanup failure, or keychain failure. The restart test says dropping FfiApp simulates process death, but there is no verified actor shutdown and one `let _ = app` does not drop the value. Do not label this complete crash-safety evidence.

Cover the missing boundaries with deterministic injection and at least one actual subprocess stop/restart test for the migration protocol. Verify wrong PIN, original sample data, idempotence, and matching key material after each retry. Add realistic fake keychain/biometric tests. Also make resumed enrollment intelligible in the UI: currently PinSetup looks like new credential creation but silently requires the earlier PIN; iOS does not render global toasts. Provide visible recovery/error guidance without exposing secrets.

## R5 — Android verification was skipped based on an incorrect environment assumption

Concrete compile defect in the submitted patch: SystemPromptSheet.kt invokes LaunchedEffect without importing androidx.compose.runtime.LaunchedEffect. Fix and verify through compilation.

SDK exists at /home/lio/Android/Sdk. `/home/lio/Android/Sdk/platform-tools/adb devices` lists a connected device. Inspect installed NDK/build tools and cargo-ndk before asserting absence; set ANDROID_HOME/ANDROID_SDK_ROOT in scoped command environment. Run Gradle compilation and relevant unit/Compose checks. Add behavioral Instructions tests (prefill, edit-save-reopen, cancel, clear, conversation switch), including the core SetSystemPrompt snapshot refresh. Build updated Rust native artifacts before claiming end-to-end Android integration. Do not clear data or replace the existing installed app on the personal device: prefer an emulator or isolated test application ID for runtime tests.

The report falsely states native source changes "compile in isolation" despite no native compilation and says bindings-swift unavailable solely because Xcode is absent even though host binding generation is separate. Correct these claims; include actual checks and blockers. iOS runtime sign-off remains unavailable absent a configured Mac/Xcode runner; provide real UI tests and exact commands, do not claim success from source review.

## Execution

Implement fixes and tests now, run targeted suites and full Rust checks after relevant changes, run Android checks with the existing SDK, update APP_REVIEW_IMPLEMENTATION_REPORT.md. Do not simply acknowledge findings. Report exact results to Codex for the next independent review.
