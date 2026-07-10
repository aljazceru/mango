---
phase: 28-local-data-encryption-authentication
verified: 2026-04-09T10:00:00Z
status: human_needed
score: 14/14 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 12/14
  re_verified: 2026-04-09T11:00:00Z
  gaps_closed:
    - "ENC-04: SetupPin now stores DEK in platform keychain when enable_biometric=true (commit b15be5b)"
    - "ENC-09: BiometricResult success now loads DEK from keychain, calls open_encrypted + load_post_unlock, transitions to Home without PIN (commit de64310)"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Set up PIN with biometric enabled on an iOS device with Face ID or an Android device with enrolled fingerprint. Lock the app. Return to it and wait for the biometric prompt. After successful biometric authentication, observe whether the app unlocks to the home screen without requiring a PIN."
    expected: "App unlocks to home screen after biometric success alone — no PIN field required."
    why_human: "Requires physical device with enrolled biometrics; cannot exercise LAContext.evaluatePolicy or BiometricPrompt in a headless environment."
  - test: "Build and run on an iOS simulator (fresh install). Verify PIN setup flow appears. Set PIN with biometric toggle ON. Force-quit and relaunch. Verify lock screen → PIN or biometric unlock → Home screen."
    expected: "Lock screen shown on relaunch; PIN entry unlocks to Home; biometric path would require real hardware."
    why_human: "Xcode simulator builds cannot be verified programmatically in this environment."
  - test: "On an Android device with enrolled biometrics (not emulator), verify BiometricPrompt appears on lock screen and that accepting it transitions directly to Home (no PIN required) when biometric was enabled during setup."
    expected: "BiometricPrompt displayed; biometric success opens Home without PIN."
    why_human: "BiometricPrompt requires physical device hardware; Class 3 biometric cannot be simulated."
---

# Phase 28: Local Data Encryption & Authentication Verification Report

**Phase Goal:** All local data (SQLite database, usearch vector indices, cached documents) is encrypted at rest using platform hardware capabilities. Users authenticate via biometrics or PIN/password to unlock. Duress PIN triggers full data wipe. Graceful degradation on devices without biometric hardware. All three platforms: iOS, Android, Desktop.

**Verified:** 2026-04-09T10:00:00Z
**Re-verified:** 2026-04-09T11:00:00Z
**Status:** human_needed
**Re-verification:** Yes — after gap closure (plan 28-08, commits b15be5b + de64310)

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | SQLCipher encrypts the main database transparently when a DEK hex key is provided | VERIFIED | `rust/Cargo.toml` line 26: `bundled-sqlcipher` feature; `persistence/mod.rs`: `open_encrypted`, `is_encrypted`, `migrate_to_encrypted` present and calling `sqlcipher_export` |
| 2 | AES-256-GCM encrypts and decrypts arbitrary binary files with a 32-byte DEK | VERIFIED | `rust/src/crypto/file_crypto.rs`: `encrypt_file` + `decrypt_file` with MGO1 magic header at lines 25, 50 |
| 3 | Argon2id derives a 256-bit KEK from a PIN/password and salt | VERIFIED | `rust/src/crypto/key_derivation.rs`: `derive_kek` at line 42 with Argon2id |
| 4 | Bootstrap DB stores salt, wrapped DEK, duress hash, and KDF params in a singleton row | VERIFIED | `rust/src/crypto/bootstrap_db.rs`: `BootstrapDb` + `AuthParams` structs; `write_auth_params`, `read_auth_params`, `has_auth_params`, `delete_all` all present |
| 5 | DEK can be generated, wrapped with KEK, and unwrapped back to the original bytes | VERIFIED | `key_derivation.rs`: `generate_dek`, `wrap_dek`, `unwrap_dek` at lines 25, 65, 86 |
| 6 | Existing unencrypted DB can be migrated to SQLCipher in-place | VERIFIED | `persistence/mod.rs`: `migrate_to_encrypted` uses `sqlcipher_export` with user_version transfer fix |
| 7 | VectorIndex encrypts the usearch file on save using AES-256-GCM with the DEK | VERIFIED | `rust/src/rag/index.rs`: `save(dek: Option<&[u8; 32]>)` calls `file_crypto::encrypt_file` at line 147 |
| 8 | DEK stored in platform keychain when biometric enabled; biometric unlock retrieves DEK without PIN | VERIFIED | `lib.rs` line 4327-4338: `if enable_biometric { actor_state.keychain.store("mango", "dek", (*dek_hex).clone()) }` — `enable_biometric: _` pattern is gone (0 grep matches). `lib.rs` line 5537-5576: `keychain.load("mango","dek")` → `open_encrypted` → `load_post_unlock` on biometric success. Commits b15be5b + de64310. |
| 9 | Screen::Locked gates all app content; app starts locked on cold launch | VERIFIED | `lib.rs` line 2931: `Screen::Locked` set when `has_auth = true`; `Option<Database>` line 782; every DB-dependent handler guards against `None` |
| 10 | App locks after configurable timeout when returning from background | VERIFIED | iOS: `scenePhase` + `backgroundedAt` in `ContentView.swift` lines 69-84; Android: `onPause`/`onResume` in `MainActivity.kt` lines 28-40 |
| 11 | Biometric unlock (Face ID/Touch ID / BiometricPrompt) hardware wired and now opens DB on success | VERIFIED | iOS `BiometricProviderImpl.swift`: `LAContext.evaluatePolicy` + `DispatchSemaphore`. Android `BiometricProviderImpl.kt`: `BiometricPrompt` + `CountDownLatch`. `BiometricResult` success now calls `keychain.load` → `open_encrypted` → `load_post_unlock` → Home. PIN fallback preserved when keychain returns None or open_encrypted fails. End-to-end hardware behavior requires physical device (see Human Verification). |
| 12 | Duress PIN triggers immediate full data wipe and resets to onboarding | VERIFIED | `lib.rs` lines 4431-4456: duress check before DEK unwrap, `bootstrap.delete_all()`, `fs::remove_file(db_path)`, `fs::remove_dir_all(data_dir)`, screen reset to `Onboarding` |
| 13 | First-time mandatory PIN setup on all three platforms | VERIFIED | iOS: `PinSetupScreen.swift`; Android: `PinSetupScreen.kt`; Desktop: `pin_setup_screen.rs` — all wire `SetupPin` action |
| 14 | Lock timeout configurable in Settings; "Never" shows warning | VERIFIED | iOS: `SettingsView.swift` Security section with `Picker`; Android: `SettingsScreen.kt` `LockTimeoutPicker`; Desktop: `settings.rs` `pick_list`; all show warning for -1 |

**Score:** 14/14 truths verified

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `rust/Cargo.toml` | VERIFIED | `bundled-sqlcipher`, `argon2 = "0.5"`, `subtle = "2"` present |
| `rust/src/crypto/mod.rs` | VERIFIED | Module root with `pub mod file_crypto; pub mod key_derivation; pub mod bootstrap_db;` |
| `rust/src/crypto/file_crypto.rs` | VERIFIED | `encrypt_file`, `decrypt_file`, MGO1 header |
| `rust/src/crypto/key_derivation.rs` | VERIFIED | `generate_dek`, `generate_salt`, `derive_kek`, `wrap_dek`, `unwrap_dek`, `hash_pin`, `verify_pin_hash` |
| `rust/src/crypto/bootstrap_db.rs` | VERIFIED | `BootstrapDb`, `AuthParams`, all 5 methods |
| `rust/src/persistence/mod.rs` | VERIFIED | `open_encrypted`, `is_encrypted`, `migrate_to_encrypted` |
| `rust/src/rag/index.rs` | VERIFIED | `new(dek: Option<&[u8;32]>)`, `save(dek: Option<&[u8;32]>)`, `encrypt_file`/`decrypt_file` calls, MGO1 detection |
| `rust/src/lib.rs` | VERIFIED | SetupPin stores DEK via `keychain.store("mango","dek",...)` at line 4332 when `enable_biometric=true`; `enable_biometric: _` pattern removed (0 matches). BiometricResult success calls `keychain.load("mango","dek")` at line 5537, then `open_encrypted` + `load_post_unlock` at lines 5549-5574. |
| `ios/Mango/Mango/BiometricProviderImpl.swift` | VERIFIED | `LAContext`, `canEvaluatePolicy`, `evaluatePolicy`, `DispatchSemaphore` present |
| `ios/Mango/Mango/LockScreen.swift` | VERIFIED | `attemptBiometricUnlock()` dispatches `AttemptBiometricUnlock`; PIN field dispatches `UnlockWithPin` |
| `ios/Mango/Mango/PinSetupScreen.swift` | VERIFIED | Dispatches `SetupPin(pin:duressPin:enableBiometric:)` |
| `ios/Mango/Mango/ContentView.swift` | VERIFIED | `case .locked:` and `case .pinSetup:` routing; `scenePhase` background timeout |
| `android/.../BiometricProviderImpl.kt` | VERIFIED | `BiometricPrompt`, `CountDownLatch`, `WeakReference`, `runOnUiThread` present |
| `android/.../ui/LockScreen.kt` | VERIFIED | Dispatches `AttemptBiometricUnlock` and `UnlockWithPin` |
| `android/.../ui/PinSetupScreen.kt` | VERIFIED | Dispatches `SetupPin` |
| `android/.../MainActivity.kt` | VERIFIED | `onPause`/`onResume` with `backgroundedAt`, dispatches `LockApp` |
| `desktop/iced/src/lock_screen.rs` | VERIFIED | Secure PIN input, dispatches `UnlockWithPin` |
| `desktop/iced/src/pin_setup_screen.rs` | VERIFIED | Dispatches `SetupPin` |
| `desktop/iced/src/main.rs` | VERIFIED | `Screen::Locked` and `Screen::PinSetup` routing; `NullBiometricProvider` |
| `ios/Mango/Mango/SettingsView.swift` | VERIFIED | Security section with lock timeout `Picker` |
| `android/.../ui/SettingsScreen.kt` | VERIFIED | SECURITY section with `LockTimeoutPicker` |
| `desktop/iced/src/views/settings.rs` | VERIFIED | SECURITY section with `pick_list` |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `key_derivation.rs` | `file_crypto.rs` | DEK bytes used as AES-256-GCM key | VERIFIED | `wrap_dek` uses same AES-256-GCM primitives; `index.rs` calls `file_crypto::encrypt_file` with DEK |
| `bootstrap_db.rs` | `key_derivation.rs` | salt and wrapped_dek stored/loaded for KEK derivation | VERIFIED | `SetupPin` writes `AuthParams`; `UnlockWithPin` reads params and calls `derive_kek(pin, &params.salt, ...)` |
| `AppAction::SetupPin` | `crypto::key_derivation` | generate_dek → derive_kek → wrap_dek | VERIFIED | Lines 4301-4325 in lib.rs |
| `AppAction::UnlockWithDek` | `Database::open_encrypted` | Actor opens DB with DEK hex | VERIFIED | Lines 4381-4398 in lib.rs |
| `AppAction::SetupPin.enable_biometric` | `KeychainProvider::store(DEK)` | DEK stored in keychain when biometrics enabled | VERIFIED | `lib.rs` line 4332: `actor_state.keychain.store("mango", "dek", (*dek_hex).clone())` inside `if enable_biometric` block. `enable_biometric: _` binding removed. |
| `BiometricResult::success` | `UnlockWithDek` logic | Biometric success loads DEK from keychain, opens DB | VERIFIED | `lib.rs` line 5537: `keychain.load("mango","dek")`; line 5549: `open_encrypted`; line 5570: `load_post_unlock(true)` with `continue`. Fallback to PIN when keychain returns None. |
| `ContentView.scenePhase` | `AppAction::lockApp` | Background timeout triggers lock | VERIFIED | iOS ContentView.swift lines 69-84 |
| `MainActivity.onResume` | `AppAction::LockApp` | Android background timeout triggers lock | VERIFIED | MainActivity.kt lines 31-40 |
| `Screen::Locked` (iOS) | `LockScreen` composable | ContentView routing | VERIFIED | `case .locked:` at line 15 |
| `Screen::Locked` (Android) | `LockScreen` composable | MainApp.kt routing | VERIFIED | `Screen.Locked` case in routing |
| `Screen::Locked` (Desktop) | `lock_screen::view()` | main.rs routing | VERIFIED | Line 1170 matches `Screen::Locked` |

### Requirements Coverage

| Requirement | Description | Status | Evidence |
|-------------|-------------|--------|---------|
| ENC-01 | SQLCipher as bundled encryption engine | SATISFIED | `bundled-sqlcipher` in Cargo.toml; `open_encrypted` with key pragma |
| ENC-02 | Usearch vector index files encrypted with AES-256-GCM | SATISFIED | `rag/index.rs` with `encrypt_file`/`decrypt_file`; MGO1 header |
| ENC-03 | AES-256-GCM with MGO1 header, random nonce, authenticated tag | SATISFIED | `file_crypto.rs`: MGO1 prefix, 12-byte OsRng nonce, 16-byte GCM tag |
| ENC-04 | DEK stored in platform keychain on first launch when biometric enabled | SATISFIED | `lib.rs` line 4327-4338: `if enable_biometric { actor_state.keychain.store("mango","dek",...) }` after `dek_hex` computed. Commit b15be5b. Previously BLOCKED. |
| ENC-05 | Argon2id (64MiB, 3 iter, parallelism 1) to wrap/unwrap DEK | SATISFIED | `key_derivation.rs`: `DEFAULT_MEMORY_KIB=65536`, `DEFAULT_ITERATIONS=3`, `DEFAULT_PARALLELISM=1` |
| ENC-06 | Bootstrap DB stores salt, wrapped DEK, duress hash, KDF params | SATISFIED | `bootstrap_db.rs` `AuthParams` struct with all fields; `write_auth_params` wired in `SetupPin` |
| ENC-07 | Screen::Locked gates all content; app starts locked | SATISFIED | `Option<Database>` pattern; `Screen::Locked` initial state for returning users; all handlers guarded |
| ENC-08 | App locks after configurable timeout from background | SATISFIED | iOS `scenePhase` + `backgroundedAt`; Android `onPause`/`onResume`; Desktop cold-launch-only (documented limitation) |
| ENC-09 | Biometric unlock (Face ID/Touch ID / BiometricPrompt) with PIN fallback | SATISFIED | Biometric hardware auth wired on iOS and Android. `BiometricResult` success now calls `keychain.load` → `open_encrypted` → `load_post_unlock` → Home. PIN fallback when keychain returns None or DB open fails. End-to-end behavior requires physical device (see Human Verification). Commit de64310. Previously PARTIAL. |
| ENC-10 | Duress PIN: full data wipe and reset to onboarding | SATISFIED | `UnlockWithPin` handler: `bootstrap.delete_all()`, `remove_file(db_path)`, `remove_dir_all(data_dir)`, `Screen::Onboarding` |
| ENC-11 | First-time mandatory PIN setup with optional duress PIN and biometric enrollment | SATISFIED | `PinSetupScreen` on all 3 platforms; multi-step UI with duress field and biometric toggle |
| ENC-12 | Existing unencrypted DBs migrated via sqlcipher_export | SATISFIED | `migrate_to_encrypted` in `persistence/mod.rs` with `sqlcipher_export`; called from `SetupPin` when plaintext DB exists |
| ENC-13 | Lock timeout configurable: Immediately, 1min, 5min (default), 15min, Never | SATISFIED | All 5 options on all 3 platforms; warning text for Never; `SetLockTimeout` action persisted |
| ENC-14 | All 3 platforms support PIN/password as minimum; biometrics additive | SATISFIED | iOS, Android, Desktop all have lock + PIN setup screens; Desktop uses NullBiometricProvider; biometrics are additive |

### Anti-Patterns Found

No blockers or warnings in modified files. Previous blocker anti-patterns resolved:

| File | Line | Previous Pattern | Resolution |
|------|------|-----------------|------------|
| `rust/src/lib.rs` | 4297 (was) | `enable_biometric: _` — parameter ignored | Removed. Parameter now used in `if enable_biometric` guard at line 4327. |
| `rust/src/lib.rs` | 5524-5530 (was) | `biometric_authenticated = true` without DEK retrieval | Replaced with full keychain-load + open_encrypted + load_post_unlock flow. |

### Human Verification Required

#### 1. Biometric Auto-Unlock Flow (iOS Face ID / Android Fingerprint)

**Test:** Set up PIN with biometric enabled on an iOS device with Face ID or on an Android device with enrolled fingerprint. Lock the app. Return to it and wait for the biometric prompt. After successful biometric authentication, observe whether the app unlocks to the home screen without requiring a PIN.
**Expected:** App unlocks to home screen after biometric success alone — no PIN field required.
**Why human:** Requires physical device with enrolled biometrics. `LAContext.evaluatePolicy` and `BiometricPrompt` cannot be exercised in a headless environment.

#### 2. Platform-Specific Build Verification (iOS Simulator)

**Test:** Build and run the iOS app on a simulator (fresh install). Verify PIN setup flow appears. Set PIN with biometric toggle ON. Force-quit and relaunch — verify lock screen shows and PIN unlock works. (Biometric will not trigger on simulator but keychain store should still execute.)
**Expected:** Lock screen shown on relaunch; PIN entry unlocks to Home screen.
**Why human:** Xcode simulator builds cannot be verified programmatically in this environment.

#### 3. Android Biometric Prompt on Physical Device

**Test:** Install on Android device with enrolled biometrics (not emulator). Enable biometric during PIN setup. Lock app, return to it, verify `BiometricPrompt` appears and that accepting it transitions directly to Home without PIN.
**Expected:** Class 3 biometric prompt displayed; success → Home screen without PIN entry.
**Why human:** `BiometricPrompt` requires physical device hardware.

## Re-Verification Summary

Both gaps from the initial verification are now closed.

**ENC-04 (was FAILED):** The `SetupPin` handler at `lib.rs` line 4327-4338 now contains `if enable_biometric { actor_state.keychain.store("mango".to_string(), "dek".to_string(), (*dek_hex).clone()) }`. The `enable_biometric: _` wildcard binding is gone — confirmed by 0-match grep. Commit b15be5b.

**ENC-09 (was PARTIAL):** The `BiometricResult` handler at `lib.rs` lines 5532-5588 is fully replaced. On `success=true`, it calls `keychain.load("mango","dek")`. If a DEK is present, it calls `open_encrypted` and `load_post_unlock(true)` (the exact same pattern as `UnlockWithPin`), then `continue` to skip the trailing emit. If `open_encrypted` fails, it shows an error toast and falls back to PIN. If the keychain returns `None` (biometric not enrolled at setup), it falls back to PIN via `biometric_authenticated=true`. `load_post_unlock` is now called from 5 sites (confirmed by grep). Commit de64310.

**Regressions:** None. All 12 previously-verified truths remain intact — key symbols present (27 matches), `bundled-sqlcipher` at Cargo.toml line 26 unchanged, build passed per SUMMARY self-check (265 tests, 0 failures).

The three human verification items are unchanged from the initial verification — they require physical devices with enrolled biometrics and are not actionable programmatically.

---

_Initial verification: 2026-04-09T10:00:00Z_
_Re-verification: 2026-04-09T11:00:00Z_
_Verifier: Claude (gsd-verifier)_
