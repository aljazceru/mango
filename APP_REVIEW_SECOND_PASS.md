# Second review and remaining verification

Continue on SWE-1.7 with YOLO permissions. Implement directly using the context already inspected; avoid broad rediscovery.

## Two remaining correctness findings from R1–R3

1. `load_post_unlock` now handles existing first-run conversation rows, but only sets `current_conversation_id` and loads message state when `!already_exists`. A crash after the main DB transaction commits but before `clear_pending_first_run` leaves the row present on restart: the router becomes Chat while `current_conversation_id` remains None/stale and messages are not hydrated. Always establish coherent current-conversation/message state from the persisted row before routing to that Chat. Add a regression that seeds active auth + existing conversation + pending continuation and checks both router conversation ID AND current_conversation_id, messages, and a reachable send/save action after restart. Do not rely on merely asserting list length.

2. Startup `NothingToRecover` still clears pending_auth and permits creation of a blank DB when `pending_first_run` is absent. This is valid for a legacy enrollment too: pending_auth is only written by SetupPin AFTER a real DB exists, so the comment claiming a fresh install crashed before DB creation is incorrect. Keep pending auth and block recovery even without a first-run marker. The recovery helper also deletes `enc_tmp` when it is the only remaining file, and does not account for `enc_replaced`. Never delete an only surviving encrypted candidate with matching pending key metadata. Resolve and verify candidates with the entered PIN, or preserve them and visibly block; do not create a blank replacement. Test legacy pending enrollment with missing main/backup, plus only-temp and only-replaced candidates, asserting key metadata/files survive and no blank DB is created.

## Finish R4 and R5 from APP_REVIEW_FOLLOWUP.md

- Add an actual subprocess interruption/restart regression (process exit before cleanup after active promotion is a useful boundary), plus fake keychain/biometric enrollment coverage. Keep tests isolated from production data and existing device app.
- Recovery PinSetup UI must explain that interrupted enrollment needs the previously entered PIN and visibly render failure (especially iOS). Avoid inventing a new-credential-looking form that silently rejects new values. Keep messages free of secrets.
- Fix missing LaunchedEffect import in SystemPromptSheet.kt and run Android Gradle compilation and tests with ANDROID_HOME=/home/lio/Android/Sdk. Inspect available NDK/cargo-ndk; run updated native pipeline before Android integration claims. Add prompt lifecycle behavioral tests and core SetSystemPrompt snapshot regression.
- A connected personal Android device exists; do not clear its app data or replace its installed app. Prefer emulator or isolated test application ID.
- Add iOS back-navigation UI tests and exact runner commands; no Mac runner has been provided yet, so runtime verification remains explicitly pending.
- Enforce the existing four-character minimum PIN/duress policy in core SetupPin too (currently it only rejects whitespace), preserve credential bytes for existing legacy passphrases, and test invalid/colliding credentials without modifying storage.
- Update report with actual results and outstanding prerequisites. Run commands with reliable exit codes; piping to tail needs pipefail. Run full Rust tests after fixes and report exact Android build/test output, not an environment-variable inference.

Deliver implemented changes and tested evidence for independent Codex review. Do not stop at acknowledgments or planning.
