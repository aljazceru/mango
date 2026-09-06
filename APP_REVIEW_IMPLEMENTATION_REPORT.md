# App Review Remediation — Implementation Report

**Scope:** All five findings from `APP_REVIEW_REMEDIATION_PLAN.md`.
**Approach:** Rust-core-first, with native UI changes kept as thin presentation layers. No commits, pushes, or production data access. `cargo fmt` applied.

## 1. Summary of Changes

| Finding | Location | What changed |
|---------|----------|--------------|
| 3 — `cold_launch_bypass` preservation | `rust/src/crypto/bootstrap_db.rs` | `write_auth_params` now uses an explicit `INSERT ... ON CONFLICT DO UPDATE` upsert that does **not** overwrite `cold_launch_bypass`. Fresh rows still default to `false`. |
| 4 — iOS chat navigation | `ios/Mango/Mango/ContentView.swift`<br>`ios/Mango/Mango/ChatView.swift` | `.chat` is now rendered inside a local `NavigationStack`; `ChatView` has an explicit `onBack: { appManager.dispatch(.popScreen) }` toolbar button and hides the SwiftUI-synthesized back button. |
| 5 — Android system-prompt prefill | `android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt`<br>`android/app/src/main/java/dev/disobey/mango/ui/SystemPromptSheet.kt` | Sheet is keyed on `currentConversation?.id`, pre-filled from `currentConversation?.systemPrompt`, and resets its draft when the supplied initial prompt changes. `onDismiss` only closes the sheet. |
| 1 — Recoverable PIN enrollment | `rust/src/lib.rs`<br>`rust/src/crypto/bootstrap_db.rs`<br>`rust/src/persistence/mod.rs` | `SetupPin` now stages `pending_auth`, migrates the plaintext DB to SQLCipher, and promotes to active auth **only** after the encrypted DB is verified. Rollback/recovery helpers handle backup/restore at each interruption boundary. WAL/SHM/journal sidecars are cleaned. |
| 2 — Mandatory enrollment after onboarding | `rust/src/lib.rs` | `CompleteOnboarding` and `SkipOnboarding` on real installs persist a `pending_first_run` continuation and route to `Screen::PinSetup` when no auth is configured. After `SetupPin` succeeds, the continuation is applied idempotently. An enrollment gate blocks navigation/chat actions while the gate is active. |

New test modules:
- `rust/src/tests/enrollment_recovery.rs` — 8 tests.
- `rust/src/tests/onboarding_mandatory.rs` — 5 tests.

## 2. Commands Run and Results

All commands were run from `/run/media/lio/data/g/confidential-app`.

### 2.1 Rust type and build checks

```bash
cargo check -p mango_core
```
- Exit code: `0`
- Result: `mango_core` lib checked successfully.

```bash
cargo build -p mango_core
```
- Exit code: `0`
- Result: `Finished dev profile [unoptimized + debuginfo] target(s)` in 20.33s.

```bash
cargo clippy -p mango_core
```
- Exit code: `0`
- Result: no warnings, no errors.

```bash
cargo fmt --check
```
- Exit code: `0`
- Result: already formatted.

```bash
cargo fmt
```
- Exit code: `0`
- Result: formatted 5 modified/new Rust source files.

### 2.2 Rust test suite

```bash
cargo test -p mango_core
```
- Exit code: `0`
- Total tests: `634` (lib unit tests)
- Passed: `613`
- Failed: `0`
- Ignored: `21` (includes `test_desktop_provider_embed` which requires model download and `decrypt_real_backup_file` which requires env vars)
- Measured: `0`
- Filtered: `0`
- Also ran: 5 export-markdown integration tests, all passed; 0 doc-tests.

Selected targeted runs:

```bash
cargo test -p mango_core enrollment_recovery
```
- Exit code: `0`
- 8/8 enrollment-recovery tests passed.

```bash
cargo test -p mango_core onboarding_mandatory
```
- Exit code: `0`
- 5/5 mandatory-onboarding tests passed.

### 2.3 Bindings / native pipeline tools

```bash
cargo run --bin uniffi-bindgen -- --help
```
- Exit code: `0`
- Result: `uniffi-bindgen` available.

**Note:** UniFFI binding generation was **not** re-run because the exported interface did not change (no UDL edits, no new `#[uniffi::export]` functions, `AppAction` variants unchanged). The new `BootstrapDb`/`Database` helpers are internal Rust APIs, not exposed across the FFI boundary.

### 2.4 Android and iOS environment checks

```bash
which java; java -version; echo "ANDROID_HOME=$ANDROID_HOME"; which xcodebuild; which swift; uname -a
```
- Exit code: `0`
- Observed:
  - `java`: `/usr/bin/java`, OpenJDK 17.0.20.
  - `ANDROID_HOME`: **unset**; no Android SDK/NDK present.
  - `xcodebuild`: not found.
  - `xcode-select -p`: not available.
  - Host: Linux x86_64.

Consequences:
- **Android Gradle checks:** could not run. The environment lacks `ANDROID_HOME`, `sdkmanager`, `adb`, `cargo-ndk`, and the NDK, so `just android-full`, `just build-android`, and `./gradlew :app:assembleDebug` are unavailable.
- **iOS Swift / xcframework checks:** could not run. `xcodebuild`, `xcode-select`, and the iOS SDKs are not present, so `just build-ios`, `just ios-full`, and `just bindings-swift` are unavailable.

The Android and iOS source files were edited anyway because they are thin UI layers and the Rust-side logic remains authoritative. Full native build verification must be performed on a host with the respective SDKs.

## 3. Test Descriptions

### 3.1 New Rust regression tests

`tests::enrollment_recovery` (8 tests):
- `test_setup_pin_migrates_plaintext_preserving_conversations`
- `test_migrate_to_encrypted_leaves_plaintext_backup_until_finalized`
- `test_rollback_encrypted_to_plaintext_restores_original_data`
- `test_recover_plaintext_after_failed_enrollment_restores_backup`
- `test_promote_pending_auth_preserves_cold_launch_bypass`
- `test_resume_encrypted_main_with_pending_auth_succeeds`
- `test_resume_wrong_pin_is_retryable`
- `test_setup_pin_rejects_duplicate_enrollment`

`tests::onboarding_mandatory` (5 tests):
- `test_complete_onboarding_routes_to_pin_setup_file_backed`
- `test_skip_onboarding_routes_to_pin_setup_file_backed`
- `test_restart_resumes_pending_enrollment_no_duplicate_conversation`
- `test_enrollment_gate_blocks_navigation_and_chat`
- `test_in_memory_complete_onboarding_goes_directly_to_chat`

### 3.2 Existing Rust tests that cover earlier findings

`tests::crypto`:
- `test_bootstrap_db_write_preserves_cold_launch_bypass`
- `test_bootstrap_db_fresh_insert_defaults_cold_launch_bypass_to_false`

`tests::persistence_encrypted`:
- `test_migrate_to_encrypted_converts_plaintext_db`
- `test_open_encrypted_with_correct_key_runs_migrations`
- `test_open_encrypted_with_wrong_key_returns_error`

All passed in the full `cargo test -p mango_core` run.

## 4. Known Limitations and Risks

1. **Native builds.** *(Superseded by section 9)* Android was verified: `compileDebugKotlin`, `testDebugUnitTest`, `assembleDebug`, and on-device instrumented tests all pass with `ANDROID_HOME=/home/lio/Android/Sdk`. iOS remains unverified — `xcodebuild` requires a macOS runner (see §9.7). The debug `just build-android` recipe fails on the x86_64 ABI due to a pre-existing `usearch`/`numkong` NDK header conflict; the shipped arm64 path (`build-android-release`) works.
2. **Binding regeneration.** *(Superseded by section 9)* `just bindings-swift` and `just bindings-kotlin` run on the host (no Xcode needed) and were re-run — a new `AppState.enrollmentResumePending` field is now exported.
3. **FfiApp re-initialization after duress/reset.** The duress wipe already clears `pending_auth` and `pending_first_run`, which keeps the clean-state semantics.
4. **WAL/SHM cleanup edge case.** Migration now removes SQLite sidecars, but if a future caller leaves a connection open during `migrate_to_encrypted`, the rename can still fail. The code drops the plaintext `Database` handle before migration and returns an error with rollback.
5. **Enrollment gate covers explicit actions only.** It blocks `PushScreen`, `PopScreen`, `NewConversation`, `LoadConversation`, `SendMessage`, `RetryLastMessage`, `EditMessage`, `ForkConversation`, `NextOnboardingStep`, `PreviousOnboardingStep`, `DeleteConversation`, and `DeleteAllConversations`. Any new action that should also be gated must be added to the `blocked` match in the `enrollment_gate_active` guard.
6. **In-memory test compatibility.** `CompleteOnboarding` in `:memory:` mode still goes directly to `Chat` (preserving existing unit-test expectations). File-backed installs are gated through `PinSetup`.

## 5. Baseline / Regression Results

The full `cargo test -p mango_core` suite produced the same baseline quality as the pre-remediation run: **0 failures**, with the only ignored tests being long-running model-download tests and env-var-dependent PPQ tests. No new warnings were introduced by `cargo clippy` or `cargo fmt --check`.

## 6. Pre-existing Working-Tree Changes

The following files were already modified in the working tree before this final implementation pass and were **preserved unchanged**:
- `.planning/STATE.md`
- `README.md`
- `android/app/build.gradle.kts`
- `zapstore.yaml`

## 7. Final State

- Implementation: complete for all five findings.
- Rust verification: `cargo check`, `cargo clippy`, `cargo build`, `cargo test` all pass.
- Formatting: clean per `cargo fmt --check`.
- Native verification: blocked by missing Android SDK / Xcode toolchains.
- No commits, no pushes, no production data access.
- Ready for independent Codex review.

## 8. R1/R2/R3 Follow-up Implementation

This section records the follow-up work for the three Rust/persistence defects described in `APP_REVIEW_FOLLOWUP.md`.

### 8.1 R1 — Checked, retryable cleanup after verified active-auth open

| File | Change |
|------|--------|
| `rust/src/persistence/mod.rs` | `Database::finalize_encrypted_storage` now returns `Result<(), PersistenceError>`. It removes `.enc_tmp`, `.plain_bak`, and `.enc_replaced` sidecars. All file removals are checked and surfaced to the caller. |
| `rust/src/lib.rs` | `load_post_unlock` calls `finalize_encrypted_storage` only after active auth is verified and the encrypted DB is open. A failure is observable (`last_error`) and will be retried on the next verified open. `SetupPin` (fresh and resume) also checks the result and logs it. |
| Tests | `test_finalize_encrypted_storage_is_strict_and_retryable`, `test_rollback_preserves_encrypted_main_when_backup_missing`, `test_migrate_to_encrypted_does_not_create_blank_database`. |

### 8.2 R2 — Preserve pending auth on failed restore and avoid blank DB creation

| File | Change |
|------|--------|
| `rust/src/persistence/mod.rs` | `Database::recover_plaintext_after_failed_enrollment` now returns `EnrollmentRecovery`, never creates a fresh main DB, and distinguishes `PlaintextReady`, `EncryptedReady`, and `NothingToRecover`. `Database::migrate_to_encrypted` rejects missing sources. `Database::rollback_encrypted_to_plaintext` refuses to destroy the encrypted main when no backup exists. |
| `rust/src/lib.rs` | `FfiApp::new` startup, `SetupPin` stale-pending handling, `SetupPin` fresh migration failure, and `SetupPin` resume/fresh promotion failure all preserve pending auth until rollback/recovery is verified. They do not clear `pending_auth` on error and do not fall back to `Database::open` on a missing path, which would create a blank database. |
| Tests | `test_startup_recovery_failure_blocks_no_blank_database`, `test_migrate_to_encrypted_does_not_create_blank_database`, `test_rollback_preserves_encrypted_main_when_backup_missing`. |

### 8.3 R3 — Transactionally persist first-run continuation before clearing marker

| File | Change |
|------|--------|
| `rust/src/lib.rs` | `load_post_unlock` now applies the first-run continuation in a single `rusqlite::Transaction`: it first sets `has_completed_onboarding`, then inserts the first conversation (only if the requested conversation ID does not already exist), then commits. The bootstrap `pending_first_run` marker is cleared only after the main DB transaction commits successfully. If any step fails, the marker is retained and the user is routed to `Home` so a later unlock can retry. |
| Tests | `test_first_run_continuation_is_idempotent`, `test_first_run_insert_failure_retains_continuation`, `test_first_run_set_setting_failure_retains_continuation`. |

### 8.4 Commands and results

```bash
cargo check -p mango_core
```
- Exit code: `0`

```bash
cargo clippy -p mango_core
```
- Exit code: `0`

```bash
cargo fmt --check
```
- Exit code: `0`

```bash
cargo test -p mango_core
```
- Exit code: `0`
- Total lib unit tests: `641`
- Passed: `620`
- Failed: `0`
- Ignored: `21`
- Also: 5 export-markdown tests passed; 0 doc-tests.

```bash
cargo test -p mango_core enrollment_recovery
```
- Exit code: `0`
- 12/12 enrollment recovery tests passed (4 new from R1/R2).

```bash
cargo test -p mango_core onboarding_mandatory
```
- Exit code: `0`
- 8/8 onboarding/mandatory tests passed (3 new from R3).

### 8.5 Remaining work

- R4 (subprocess restart test, keychain/biometric fakes, resume UX hint on PinSetup) and R5 (Android verification, Gradle compile/unit tests) are explicitly deferred to the next turn, per user instruction.
- No commit, push, PR, or merge was performed.

## 9. Second Follow-up: R1–R5 Completion

This section covers the remaining findings from `APP_REVIEW_FOLLOWUP.md` and
`APP_REVIEW_SECOND_PASS.md`, including the R2/R3 follow-up defects and the R4/R5
verification work deferred in section 8.5.

### 9.1 R3 follow-up — hydrate chat state when the continuation row already exists

| File | Change |
|------|--------|
| `rust/src/lib.rs` | `load_post_unlock` no longer conditions `current_conversation_id`/`messages` on whether the first-run row was newly inserted. After the continuation transaction commits it always sets `current_conversation_id = Some(conv_id)`, calls `refresh_messages`, and sets `show_first_chat_placeholder = messages.is_empty()` before routing to `Screen::Chat`. This covers the crash window where the main-DB transaction committed but `clear_pending_first_run` never ran. |
| Test | `tests::onboarding_mandatory::test_first_run_restart_hydrates_existing_conversation_state` — seeds active auth + an existing conversation + a persisted message + a pending continuation, then asserts the router points at `Chat{conversation_id}`, `current_conversation_id` matches, the persisted message is hydrated, and `SetSystemPrompt` on that conversation persists and refreshes the snapshot. |

### 9.2 R2 follow-up — never lose an only-surviving encrypted candidate

| File | Change |
|------|--------|
| `rust/src/persistence/mod.rs` | `EnrollmentRecovery::EncryptedCandidate` added. `recover_plaintext_after_failed_enrollment` now reports (never deletes) a sole surviving `enc_tmp` or `enc_replaced`. `promote_encrypted_candidate` verifies a candidate against a DEK before moving it into place — wrong key returns `false` with the file preserved, verified key promotes and removes the temp sidecar. A surviving `enc_replaced` combined with a plaintext backup restores the backup. |
| `rust/src/lib.rs` | Startup `NothingToRecover` now **always** preserves `pending_auth` and blocks at PinSetup when no recoverable file exists — including the legacy case with no `pending_first_run` marker — and never creates a blank DB. The `SetupPin` stale-pending path promotes a candidate using the entered PIN, keeps `pending_auth` when the PIN doesn't match the candidate, and blocks when nothing is recoverable. `enrollment_resume_pending` is populated at startup, on resume completion, and on `load_post_unlock`. |
| Tests | `test_startup_pending_auth_no_first_run_blocks_and_preserves_key_material`, `test_recovery_preserves_and_promotes_only_enc_tmp_candidate`, `test_recovery_preserves_and_promotes_only_enc_replaced_candidate`, `test_setup_pin_resumes_from_only_encrypted_candidate`. |

### 9.3 PIN policy enforced in core `SetupPin`

| File | Change |
|------|--------|
| `rust/src/lib.rs` | `SetupPin` rejects: empty/whitespace main PIN, main PIN under 4 characters (`chars().count()`, matching the native UIs' 4-char minimum), an enabled duress PIN under 4 characters, and duress == main. All rejections happen **before** any bootstrap write or migration, so invalid attempts leave storage untouched. Legacy passphrases are unaffected — unlock and candidate-promotion feed credential bytes to the KDF unchanged. |
| Tests | `test_setup_pin_rejects_short_pin_without_touching_storage`, `test_unlock_preserves_legacy_short_passphrase`. |

### 9.4 R4 — interruption/restart evidence, fakes, and resume UX

- `test_subprocess_restart_after_promotion_recovers_and_finalizes` spawns the test binary as a real child process (`std::env::current_exe()`); the child stages an encrypted main + plaintext backup + committed active auth and calls `std::process::exit(0)` before finalize — the hardest crash boundary. The parent verifies the staged state, restarts `FfiApp`, unlocks, and confirms data is readable and the plaintext backup is finalized away.
- Fakes: `MemoryKeychainProvider` (shared across app instances), `FailingKeychainProvider` (store always fails), `FakeBiometricProvider` (configurable success). `test_setup_pin_biometric_stages_dek_and_fake_biometric_unlocks` covers DEK staging, biometric unlock across a simulated restart, and failed-attempt lockout. `test_setup_pin_keychain_store_failure_does_not_enable_biometric` covers a failed keychain write: enrollment completes but `biometric_login_enabled` stays false (both `SetupPin` paths now gate the flag on the `keychain.store` result instead of assuming success).
- Resume UX: `AppState.enrollment_resume_pending` (new UniFFI field) tells the UI a staged enrollment exists. iOS `PinSetupScreen` shows a dedicated resume step ("Finish Encryption Setup", single field, "Resume Setup") and renders `appState.toast`/`pinError` inline since iOS has no global toast rendering. Android `PinSetupScreen` shows the same guidance, hides the duress/biometric sections on resume (they were captured in the staged enrollment), and also surfaces `appState.toast` inline.
- iOS back-navigation UI test: `testChatBackButtonReturnsToHome` added to `MangoIOSSimulatorE2ETests.swift`.

### 9.5 R5 — Android fixes and verification

- `SystemPromptSheet.kt`: added the missing `import androidx.compose.runtime.LaunchedEffect`.
- New `SystemPromptSheetLogic.kt` (`SystemPromptDraft`, `systemPromptSaveValue`) extracts the draft lifecycle; `ChatScreen` now routes the save path through `systemPromptSaveValue`.
- `AppManager.kt` initial `AppState` gains `enrollmentResumePending = false` (required by the new UniFFI record field).
- UniFFI bindings regenerated on the host: `just bindings-swift` and `just bindings-kotlin` both succeeded and emit `enrollmentResumePending` in `ios/Bindings/mango_core.swift` and `android/.../rust/mango_core.kt`. **Correction to section 2.3/4:** binding generation does not require Xcode — it runs from the host `libmango_core.so`; the earlier "unavailable" claim was wrong.

### 9.6 Commands run and actual results

Environment: `ANDROID_HOME=/home/lio/Android/Sdk`, NDK `28.2.13676358`, `cargo-ndk` at `/home/lio/.cargo/bin/cargo-ndk`, llama.cpp checkout at pinned commit `0eb874d3` (verified by `verifyLlamaCppInputs` / `scripts/check_llama_versions.sh`).

```bash
cargo test -p mango_core enrollment_recovery
```
- Exit code: `0` — **22/22** tests passed (10 new this round, including the real subprocess restart test).

```bash
cargo test -p mango_core onboarding_mandatory
```
- Exit code: `0` — **9/9** tests passed.

```bash
cargo test -p mango_core
```
- Exit code: `0` — **632 passed, 0 failed, 21 ignored** (lib unit tests) + 5/5 export-markdown integration tests.

```bash
cargo fmt --check && cargo clippy -p mango_core && cargo build -p mango_core --release
```
- All exit code `0`.

```bash
cd android && ./gradlew :app:compileDebugKotlin :app:testDebugUnitTest
```
- Exit code: `0` — `BUILD SUCCESSFUL`; `SystemPromptSheetLogicTest` 7/7 passed (prefill, edit-save, reopen, cancel, blank-clears, conversation-switch reset).

```bash
just bindings-kotlin && just build-android-release
```
- Exit code: `0` — arm64 `libmango_core.so` rebuilt and copied to `jniLibs`, `libc++_shared.so` and pinned llama.cpp libs refreshed.

```bash
just build-android
```
- Exit code: `101` — **fails on the x86_64 ABI**: `usearch v2.26.1` → `numkong/capabilities.h:138` redeclares `syscall` as `noexcept`, conflicting with the NDK `unistd.h`. This is a pre-existing upstream crate/toolchain incompatibility unrelated to these changes; the shipped configuration is arm64-only and builds cleanly via `build-android-release`.
- **Superseded by §12 (2026-09-06):** the conflict is resolved by an in-repo vendored `numkong 7.8.1` patch and `just build-android` now exits `0` for both `arm64-v8a` and `x86_64`.

```bash
./gradlew :app:assembleDebug
```
- Exit code: `0` — `BUILD SUCCESSFUL` (APK packages the freshly built arm64 native libs).

```bash
./gradlew :app:connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=dev.disobey.mango.ui.SystemPromptSheetInstrumentedTest
```
- Exit code: `0` — **5/5 instrumented Compose tests passed** on the connected device (`Pixel 9a`), covering prefill, edit-save, reopen-after-save, cancel, and conversation-switch draft reset. The run used the isolated debug applicationId `dev.disobey.mango.dev`; no app data was cleared and the installed personal app was not touched.

### 9.7 iOS runtime verification — still blocked

No macOS/Xcode runner exists on this host (`xcodebuild` not present). Swift source changes were made but **not compiled or run**. The new UI test and the full E2E can be executed on a Mac with:

```bash
cd ios/Mango && xcodegen generate        # if the .xcodeproj isn't checked in
cd ../.. && just ios-full                # bindings + xcframework (needs Xcode)
cd ios/Mango && xcodebuild test \
  -project Mango.xcodeproj -scheme Mango \
  -destination 'platform=iOS Simulator,name=iPhone 16' \
  -only-testing:MangoUITests/MangoIOSSimulatorE2ETests/testChatBackButtonReturnsToHome
# full suite:
xcodebuild test -project Mango.xcodeproj -scheme Mango \
  -destination 'platform=iOS Simulator,name=iPhone 16'
```

### 9.8 Files changed this round

- `rust/src/lib.rs` — R3 hydration fix, `NothingToRecover` blocking, `EncryptedCandidate` handling in startup and `SetupPin`, 4-char PIN policy, `enrollment_resume_pending` plumbing, keychain-store-gated `biometric_login_enabled`.
- `rust/src/persistence/mod.rs` — `EncryptedCandidate` variant, `promote_encrypted_candidate`, candidate-preserving recovery.
- `rust/src/crypto/bootstrap_db.rs` — `pending_auth` retrieval helper.
- `rust/src/tests/enrollment_recovery.rs` — 10 new tests (candidates, subprocess restart, keychain/biometric fakes, PIN policy, legacy passphrase).
- `rust/src/tests/onboarding_mandatory.rs` — hydration regression.
- `rust/src/tests/chat.rs` — `SetSystemPrompt` snapshot regression.
- `android/.../SystemPromptSheet.kt` — `LaunchedEffect` import.
- `android/.../SystemPromptSheetLogic.kt` — new draft-lifecycle logic.
- `android/.../ChatScreen.kt` — save path via `systemPromptSaveValue`.
- `android/.../PinSetupScreen.kt` — resume mode UI + inline toast rendering.
- `android/.../AppManager.kt` — new `AppState` field in initial state.
- `android/app/src/test/.../SystemPromptSheetLogicTest.kt` — 2 JVM tests of `systemPromptSaveValue` (removed the mirror-only `SystemPromptDraft` state-machine tests).
- `android/app/src/androidTest/.../SystemPromptSheetInstrumentedTest.kt` — 5 instrumented tests.
- `android/app/build.gradle.kts` — Compose UI-test deps.
- `ios/Mango/Mango/PinSetupScreen.swift` — resume step + inline error rendering; common `currentError` area visible on every step; stale core errors cleared on each new submission.
- `ios/Mango/MangoUITests/MangoIOSSimulatorE2ETests.swift` — back-navigation test.
- `ios/Bindings/mango_core.swift`, `android/.../rust/mango_core.kt` — regenerated UniFFI bindings (`enrollmentResumePending`).
- `desktop/iced/src/pin_setup_screen.rs` — resume mode UI and view-model validation; threads `enrollment_resume_pending` from the caller.
- `desktop/iced/src/main.rs` — passes `enrollment_resume_pending` to the PIN setup view and clears stale toasts when the user retypes.

No commit, push, PR, or merge was performed.

## 10. Final bounded UI corrections

This section records the three targeted corrections requested after the second follow-up.

### 10.1 Desktop `PinSetup` resume mode

| File | Change |
|------|--------|
| `desktop/iced/src/pin_setup_screen.rs` | `view` now accepts `is_resume`. In resume mode it shows "Finish Encryption Setup" with a single PIN field, hides confirmation and duress inputs, and labels the action "Resume Setup". Validation accepts any non-empty PIN (preserving legacy passphrase lengths). The inline error prefers local validation, then displays the core toast. `build_setup_pin_action` also accepts `is_resume` and dispatches `SetupPin` with `duress_pin: None`. Unit tests `fresh_validation_requires_four_chars_and_match`, `resume_validation_only_requires_non_empty_pin`, and `build_setup_pin_action_fresh_and_resume` were added. |
| `desktop/iced/src/main.rs` | `PinSetupPinChanged`/`PinSetupConfirmChanged`/`PinSetupDuressChanged` now dispatch `ClearToast` when a toast is present so stale core errors are cleared as the user starts retyping. `PinSetupSubmit` passes `state.enrollment_resume_pending` to the builder. The view call receives `state.enrollment_resume_pending` from `AppState`. |

### 10.2 iOS `PinSetup` errors visible on every step

| File | Change |
|------|--------|
| `ios/Mango/Mango/PinSetupScreen.swift` | Added `currentError` (local `pinError`/`duressError`, then `appState.toast`) and a `clearSubmissionErrors()` helper. `pinStep`, `duressStep`, `biometricStep`, and `resumeStep` now all render a single shared error area from `currentError`. `validateAndAdvanceFromPin`, `validateAndAdvanceFromDuress`, `submitSetup`, and the resume button all clear stale core toasts before a new submission or validation, preventing an old failure from being mixed with the current attempt. |

### 10.3 Android `SystemPromptSheetLogic` reduced to the production function

| File | Change |
|------|--------|
| `android/.../SystemPromptSheetLogic.kt` | Removed the unused `SystemPromptDraft` state-machine class; only `systemPromptSaveValue` remains, and it is what `ChatScreen` actually calls. |
| `android/app/src/test/.../SystemPromptSheetLogicTest.kt` | Replaced the 7 mirror-only `SystemPromptDraft` tests with 2 focused tests of the real production function (`systemPromptSaveValue`). The 5 real Compose instrumented tests in `SystemPromptSheetInstrumentedTest` continue to cover prefill, edit-save, reopen, cancel, and conversation-switch behavior. |

### 10.4 Verification after bounded corrections

```bash
cargo check -p mango-desktop
```
- Exit code: `0`.

```bash
cargo test -p mango-desktop pin_setup_screen
```
- Exit code: `0` — **3/3** desktop view-model tests passed.

```bash
cd android && ./gradlew :app:compileDebugKotlin :app:testDebugUnitTest
```
- Exit code: `0` — `BUILD SUCCESSFUL`; `SystemPromptSheetLogicTest` now reports **2/2** passed (down from the earlier 7 mirror-only tests); all other unit tests continue to pass.

```bash
cargo test -p mango_core
```
- Exit code: `0` — **632 passed, 0 failed, 21 ignored** (lib) + 5/5 export integration tests.

```bash
cargo fmt --check
```
- Exit code: `0`.

```bash
cargo check -p mango_core && cargo clippy -p mango_core
```
- Exit codes: `0`.

```bash
cargo clippy -p mango-desktop
```
- Exit code: `0`.

### 10.5 Outstanding limitations

- **iOS runtime verification remains unavailable.** `PinSetupScreen.swift` source was updated but `xcodebuild` cannot run on this Linux host. A Mac/Xcode runner is still required.
- **`just build-android` x86_64 ABI still fails.** The pre-existing `usearch`/`numkong` NDK header conflict for `x86_64-linux-android` was not in scope for these UI corrections; the shipped arm64 path (`build-android-release`) and `./gradlew :app:assembleDebug` continue to pass.

## 11. Final artifact synchronization

The `SetupPin` validation refinement (fresh-only 4-char/duress rules so resume accepts the earlier PIN verbatim) landed after the previous arm64 build, so the native artifact was rebuilt from the final source before assembling the APK. No source edits were made in this pass.

### 11.1 arm64 native rebuild

```bash
ANDROID_HOME=/home/lio/Android/Sdk just build-android-release
```
- Exit code: `0` — `mango_core` release build for `arm64-v8a` finished in ~28s; llama.cpp version pin verified (`b9771` / `0eb874d`).
- `jniLibs/arm64-v8a/` now contains exactly the intended companion libraries, all rebuilt/copied fresh: `libmango_core.so` (33,972,024 bytes, built from final source), `libc++_shared.so`, `libggml-base.so`, `libggml-cpu.so`, `libggml.so`, `libllama-common.so`, `libllama.so`.
- `cargo ndk` again emitted two dependency cdylib artifacts (`libdcap_qvl-*.so`, `librtf_parser-*.so`) alongside the main library. These are code-generated copies, not app libraries (the crate rlibs are already linked into `libmango_core.so`), and were removed before assembling — consistent with the earlier pass.

### 11.2 Debug APK assembly from final source

```bash
cd android && ./gradlew :app:assembleDebug
```
- Exit code: `0` — `BUILD SUCCESSFUL`.
- Artifact: `android/app/build/outputs/apk/debug/app-debug.apk` (150,565,197 bytes, timestamp 22:46 — fresh, contains the rebuilt `libmango_core.so`).

### 11.3 Instrumented tests on final artifacts

```bash
./gradlew :app:connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=dev.disobey.mango.ui.SystemPromptSheetInstrumentedTest
```
- Exit code: `0` — **5/5 passed, 0 failures, 0 skipped** on Pixel 9a (Android 17), isolated application id `dev.disobey.mango.dev` (personal install untouched).
- Passing tests: `sheetPrefillsSavedPrompt`, `editThenSaveDispatchesEditedText`, `reopenShowsSavedPrompt`, `cancelDiscardsDraftWithoutSaving`, `newInitialPromptResetsDraftOnConversationSwitch`.

### 11.4 Blockers (unchanged)

- **iOS**: still cannot compile or run — `xcodebuild`/`xcodegen` require macOS; all iOS changes are source-only pending a Mac runner.
- **~~Android x86_64~~**: Superseded by `NDK_COMPATIBILITY_IMPLEMENTATION_REPORT.md` (2026-09-06). The `usearch`/`numkong` `syscall` `noexcept` vs. Bionic `unistd.h` conflict is now resolved by an in-repo vendored `numkong 7.8.1` patch with an `__ANDROID__` guard. `just build-android` builds both `arm64-v8a` and `x86_64` `mango_core`; `./gradlew :app:assembleDebug` remains green. The debug APK still packages only `arm64-v8a` because the existing `build.gradle.kts` `abiFilters` were not changed.

No commit, push, PR, or merge was performed.

## 12. Addendum — NDK x86_64 compatibility remediation (2026-09-06)

The x86_64 Android NDK blocker described in sections 9.7, 10.5, and 11.4 above has been resolved in the follow-up `NDK_COMPATIBILITY_REMEDIATION_PLAN.md` / `NDK_COMPATIBILITY_IMPLEMENTATION_REPORT.md`. The fix is a single-hunk vendored `numkong 7.8.1` patch and a `[patch.crates-io]` override in the workspace root. It does not modify unrelated app/actor code, does not change the shipped ABI filters, and preserves all prior arm64 and APK results. iOS limitations remain unchanged.
