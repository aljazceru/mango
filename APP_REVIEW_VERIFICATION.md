# Independent verification and disposition

Reviewer: Codex. Date: 2026-09-05.

## Scope

The five findings from the latest app review, implemented by Devin CLI using SWE-1.7 (`swe-1-7`) with YOLO (`dangerous`/bypass) permissions. This does not mark the separate 33-ticket UX_EVALUATION.md backlog implemented.

## Review outcome

The five original findings have implementations and regression coverage. Independent review identified additional cleanup, rollback, continuation hydration, native-compilation and recovery-UI defects. Those findings were returned to Devin and addressed through successive passes documented in APP_REVIEW_FOLLOWUP.md, APP_REVIEW_SECOND_PASS.md and APP_REVIEW_FINAL_UI_PASS.md.

No further actionable defect was identified in the final source review of these changes. This is a scoped review conclusion, not a claim that the entire application or every platform has been exhaustively validated.

- Enrollment uses pending auth and recoverable file transitions; recovery preserves surviving encrypted candidates and key metadata, blocks missing-data states, and retries cleanup after verified unlock.
- Complete/Skip require enrollment under the existing mandatory-encryption policy. Persisted first-run continuation is transactional in the main DB and rehydrates the active conversation on restart.
- Auth updates preserve cold-launch bypass when duress credentials change.
- iOS chat has its navigation container and explicit Back action. iOS, Android and desktop have recovery setup guidance; iOS displays setup errors across wizard steps.
- Android Instructions reads the saved prompt, refreshes core snapshots after changes, and has device-tested save/cancel/reopen/switch behavior.

## Evidence inspected or independently rerun

After the final source changes, Codex independently ran:

- `cargo test -p mango_core`: exit 0; 632 unit tests passed, 21 ignored, zero failures; five integration tests passed.
- `cargo test -p mango-desktop -- pin_setup_screen::tests`: exit 0; three tests passed.

The focused 22-test enrollment-recovery suite also passed in an independent run before the last resume-validation adjustment; the final full-suite run above includes those tests after that adjustment.

Devin rebuilt the final arm64 native library and debug APK, then reran device tests. Codex inspected the resulting XML:

- `android/app/build/outputs/androidTest-results/connected/debug/TEST-Pixel 9a - 17-_app-.xml`: five tests, zero failures, zero errors, zero skipped; timestamp 2026-09-05T20:47:00.
- Test suite: `dev.disobey.mango.ui.SystemPromptSheetInstrumentedTest`.
- Final APK: `android/app/build/outputs/apk/debug/app-debug.apk`, 150565197 bytes.
- Devin reports successful final `just build-android-release`, `assembleDebug`, and device tests using the debug application ID. Exact commands are in APP_REVIEW_IMPLEMENTATION_REPORT.md sections 9–11.
- Two JVM tests remain for the actual production save-value helper; the duplicate test-only draft implementation was removed.

## Remaining verification limits

1. iOS build and UI execution require a Mac/Xcode runner. The source and generated bindings were reviewed, and a Back-navigation UI test was added, but iOS runtime success is not established. Runner details were requested and have not been provided.
2. ~~The combined Android debug native pipeline's x86_64 target fails in the existing usearch/numkong dependency against NDK headers. The shipped arm64 pipeline and final APK succeeded. No x86_64 success is claimed.~~ **Superseded by `NDK_COMPATIBILITY_IMPLEMENTATION_REPORT.md` (2026-09-06):** the `numkong`/`Bionic` `syscall` exception-spec conflict is resolved by a vendored in-repo `numkong 7.8.1` patch with an `__ANDROID__` guard. `just build-android` now produces both `arm64-v8a` and `x86_64` `libmango_core.so`; the debug APK still ships only `arm64-v8a` because the existing `build.gradle.kts` `abiFilters` are unchanged. **Caveat:** the host `cargo test -p mango_core` suite is not unconditionally green — the WIP `enrollment_recovery` setup-pin migration test repeatedly panicked (`db unlocked`) under the default parallel harness in that pass; it passed under `--test-threads=1`, in module isolation, and in one exact-version unpatched control run. Root cause is not conclusively diagnosed; see the NDK report §6.3.
3. Twenty-one Rust tests are ignored by the suite. They are not counted as passing.
4. Generated Kotlin/Swift binding output contains generator-produced trailing whitespace; a global `git diff --check` is not clean for that reason. No handwritten source whitespace defect was reported by that check.

No commits or pushes were made. Existing unrelated working-tree changes were preserved. Implementation and review documents remain local workspace files.
