# App review remediation and verification plan

Date: 2026-09-05

## Scope and execution contract

Address all five findings in the latest app review. The broader UX_EVALUATION.md backlog remains a separate scope. Implement with Devin CLI using model `swe-1-7` (SWE-1.7 Max), then submit the code and actual test evidence for independent Codex review. Codex sends actionable findings back to Devin and repeats review after corrections.

Preserve existing user changes in README.md, .planning/STATE.md, android/app/build.gradle.kts, zapstore.yaml and all existing untracked documents. Do not commit, push, deploy, delete user data, use production credentials, or change unrelated dependencies. Read applicable repository instructions. Use temporary databases and test credentials exclusively. Report unavailable tools and failed checks honestly.

Retain the existing D-14 mandatory-encryption policy: first-run completion must enroll a PIN before normal use. This resolves the routing inconsistency without changing the documented security policy. Preserve intentional duress decoy behavior and dormant PPQ credentials.

## 1. Recoverable PIN enrollment (P1)

Files: rust/src/lib.rs, rust/src/crypto/bootstrap_db.rs, rust/src/persistence/mod.rs and focused tests.

Current failure: SetupPin commits auth parameters and optionally a keychain DEK before migration; failure leaves startup selecting encrypted unlock against plaintext storage.

1. Trace enrollment, bootstrap initialization, keychain caching, database migration, startup recovery, and retry behavior. Record state invariants before editing.
2. Introduce a durable enrollment transaction/recovery protocol across bootstrap and main database. A SQLite transaction alone cannot atomically replace another database file. Stage pending auth separately from active auth, verify encrypted output, atomically replace storage, and promote active auth with idempotent startup recovery. Choose and document the concrete ordering and recovery decision for each interruption boundary.
3. Never persist raw DEKs or PINs in recovery metadata. Retain only wrapped key material. Do not delete the sole recoverable copy of user data. Handle WAL/SHM and abandoned encrypted temporary files deliberately.
4. On recoverable failure, reopen original storage and keep PIN enrollment retryable. Reject duplicate enrollment over an already initialized vault. Cache biometric DEKs only when consistent with committed enrollment; clean up only enrollment-owned orphaned entries.
5. Validate credentials in core; reject an empty/invalid primary credential and primary/duress collisions. Keep secrets out of logs and long-lived UI state.

Acceptance/tests: use real file-backed SQLCipher databases containing recognizable sample conversations. Inject errors and simulate restart before and after pending auth persistence, encrypted export, verification, replacement, active auth promotion, and keychain staging. At every boundary original data survives and either enrollment can be retried or the new PIN unlocks successfully. Test successful migration, wrong PIN, repeated recovery, repeated enrollment, stale temporary output, and legacy install handling. Do not rely exclusively on :memory: tests.

## 2. Mandatory enrollment after onboarding (P1)

Files: rust/src/lib.rs and onboarding/auth tests; platform bindings only if public ABI changes.

1. Route CompleteOnboarding and SkipOnboarding through PinSetup when auth is absent. Preserve Complete -> first Chat and Skip -> Home after enrollment succeeds.
2. Persist enough continuation state to survive process restart without creating duplicate first conversations. Avoid exposing Chat/Home while enrollment is incomplete, including alternative navigation/send actions.
3. Preserve already encrypted installs, legacy enrollment, biometric/lock-timeout semantics and the explicit duress decoy exception.
4. Align first-run copy only where needed to match this existing mandatory policy.

Acceptance/tests: Complete and Skip stay behind enrollment in the same session and across restart; setup failure stays retryable; success resumes the intended screen exactly once; chat writes/sends cannot bypass the gate; returning authenticated users and decoy sessions retain intended routing.

## 3. Preserve Never timeout when changing duress PIN (P2)

Files: rust/src/crypto/bootstrap_db.rs, rust/src/lib.rs, bootstrap/auth tests.

Replace destructive singleton INSERT OR REPLACE updates with an explicit UPDATE or INSERT ON CONFLICT DO UPDATE that retains cold_launch_bypass and other independent fields. Correct the misleading comment. Keep fresh inserts defaulting to false. Apply this consistently to SetDuressPin and enrollment updates where appropriate.

Acceptance/tests: create auth with cold_launch_bypass=true; set, replace and remove a duress PIN; verify the flag remains true, the wrapped DEK and primary KDF fields are unchanged, and cold-start bypass still works with a valid cached key. Repeat with false and a fresh bootstrap. Failed updates leave the original row usable.

## 4. Restore iOS chat navigation (P1)

Files: ios/Mango/Mango/ContentView.swift, ChatView.swift and existing iOS UI tests.

Provide a NavigationStack for the chat destination compatible with the existing core-owned router, and an explicit back button invoking onBack. Avoid nesting conflicting stacks or relying on a SwiftUI back stack that is empty because routing replaces the root. Ensure a first chat opened directly from onboarding can return to Home even when the core navigation stack is empty. Keep toolbar/model picker usable and preserve existing home Library/Settings routes.

Acceptance/tests: open an existing chat and a newly created chat, return to the list, reopen a chat, exercise model toolbar, and verify onboarding-first-chat back behavior. Test repeated navigation and lock/unlock restoration. Run xcodebuild and UI tests only on an available configured macOS/Xcode runner; otherwise explicitly leave runtime sign-off unverified and provide exact remaining commands. Source inspection alone is not a passing UI test.

## 5. Prefill Android Instructions correctly (P2)

Files: android/app/src/main/java/dev/disobey/mango/ui/ChatScreen.kt and relevant Compose tests.

Resolve the active conversation's saved system prompt from current core state and pass it into SystemPromptSheet instead of the empty constant. Check sheet state initialization/reset when changing conversations or reopening. Preserve intentional clearing, cancel behavior, and persisted updates.

Acceptance/tests: reopen an existing prompt unchanged; edit/save/reopen; cancel an edit; intentionally clear/save; switch between conversations with different prompts and a conversation with no prompt. No prompt from a previously viewed conversation may leak into the next.

## Execution and test gates

1. Baseline: inspect git status/diff, available Rust/Android/iOS tools, relevant existing tests, and any already-running builds. Capture baseline failures separately.
2. Implement bootstrap preservation and recoverable enrollment, then routing, then platform UI fixes. Add focused behavioral regression tests with the relevant implementation.
3. Run focused Rust tests first, then `cargo test -p mango_core` and `cargo check -p mango_core`; check the desktop consumer if shared state/actions change. Run formatting checks without reformatting unrelated files.
4. Run applicable Android unit/Compose checks and debug compilation using existing Gradle tooling. If Rust changes affect the shipped native implementation, run the established native/bindings pipeline before claiming Android integration success. Inspect justfile recipes before invoking potentially destructive cleanup commands.
5. Run iOS build/UI checks where the toolchain is available. Never substitute static string checks for runtime success.
6. Write APP_REVIEW_IMPLEMENTATION_REPORT.md: changed files, per-finding status, recovery state machine, exact commands with exit codes/test counts, baseline failures, unavailable checks, and remaining risks. Attach or reference durable test logs without secrets.
7. Codex independently reviews final diffs and tests, especially crash recovery, credential consistency, route bypasses and platform state synchronization. Every actionable finding goes back to Devin CLI with SWE-1.7 for repair and targeted retesting. Iterate until no known actionable findings remain or an external prerequisite blocks verification; never label blocked checks as passing.

## Devin implementation instruction

Implement this plan now in the current workspace. You own the five fixes and their tests/report. Other edits already belong to the user: preserve them. Do not stop at a plan or request routine implementation approval. Run available tests and resolve introduced failures. Do not claim everything works without real evidence. Report any unavailable platform verification clearly so Codex can review both code and limitations.
