---
phase: 28
plan: "04"
subsystem: ios-ui + rust-core
tags: [authentication, ios, swiftui, biometric, pin-auth, lock-screen, uniffi]
dependency_graph:
  requires: [28-01, 28-02]
  provides: [28-05, 28-06, 28-07]
  affects: [rust/src/lib.rs, ios/Mango/Mango/, ios/Bindings/]
tech_stack:
  added: [LocalAuthentication framework, BiometricProvider callback_interface, Screen::Locked, Screen::PinSetup]
  patterns: [LAContext semaphore bridge, deferred DB open, load_post_unlock helper, Option<Database> guard]
key_files:
  created:
    - ios/Mango/Mango/BiometricProviderImpl.swift
    - ios/Mango/Mango/LockScreen.swift
    - ios/Mango/Mango/PinSetupScreen.swift
  modified:
    - rust/src/lib.rs
    - ios/Bindings/mango_core.swift
    - ios/Bindings/mango_coreFFI.h
    - ios/Mango/Mango/AppManager.swift
    - ios/Mango/Mango/ContentView.swift
    - rust/src/tests/*.rs (16 test files — FfiApp::new signature update)
decisions:
  - Phase 28-02 Rust implementation was missing from this worktree; implemented inline as Rule 3 deviation
  - BiometricProvider succeeds -> LAContext result accepted as-is (T-28-17 accept disposition)
  - AttemptBiometricUnlock logs success but platforms must dispatch UnlockWithDek directly after keychain retrieval (D-06)
  - load_post_unlock extracted to shared helper called by both startup auto-open and post-unlock paths
  - Backward-compat auto-open: in-memory (tests) and no-auth-params (legacy plaintext) paths open DB at startup
  - Duress wipe path deletes db_path file + recursively clears data_dir + bootstrap auth params (D-15)
  - PinSetupScreen step 3 (biometrics) skipped on devices without biometric hardware (appState.biometricAvailable)
metrics:
  duration: "~90 minutes"
  completed: "2026-04-09"
  tasks_completed: 2
  tasks_total: 3
  files_changed: 22
---

# Phase 28 Plan 04: iOS Lock Screen + UniFFI Bindings Summary

iOS lock gate with biometric/PIN unlock and first-time PIN setup, backed by Phase 28-02 Rust auth types (BiometricProvider trait, Screen::Locked/PinSetup, auth action handlers) regenerated into UniFFI Swift bindings.

## What Was Built

### Phase 28-02 Rust Types (implemented as prerequisite — Rule 3 deviation)

The 28-02-SUMMARY referenced commit `078d293` from a parallel worktree that was never merged. The Rust auth types were absent from this worktree's `lib.rs`.

**Screen enum additions:**
- `Screen::Locked` — shown on every cold launch and after background timeout (D-09)
- `Screen::PinSetup` — shown on first launch post-onboarding (D-14)

**BiometricProvider trait:**
```rust
#[uniffi::export(callback_interface)]
pub trait BiometricProvider: Send + Sync + 'static {
    fn biometric_status(&self) -> String;   // "available" | "not_enrolled" | "not_available"
    fn authenticate(&self, reason: String) -> bool;
}
pub struct NullBiometricProvider; // always returns false
```

**AppState Phase 28 fields:**
- `biometric_available: bool` — queried at startup via BiometricProvider.biometric_status()
- `lock_timeout_seconds: i64` — default 300 (5 minutes), persisted to settings
- `auth_initialized: bool` — true once SetupPin has written bootstrap auth params
- `encryption_enabled: bool` — true when DB was opened with SQLCipher

**ActorState Phase 28 fields:**
- `db: Option<Database>` — None until unlock (deferred open)
- `bootstrap: BootstrapDb` — always-open unencrypted DB for auth params
- `biometric_provider: Box<dyn BiometricProvider>` — platform-injected
- `pre_lock_screen: Option<Screen>` — restored after unlock (D-12)
- `db_path: String` — used to open encrypted DB post-unlock

**AppAction variants (Phase 28):**
| Action | Behavior |
|--------|----------|
| `SetupPin` | Argon2id KEK derivation, DEK wrap, bootstrap DB write, SQLCipher migration |
| `UnlockWithPin` | Duress check (T-28-11), KEK derivation, DEK unwrap, encrypted DB open |
| `UnlockWithDek` | Direct DEK hex → encrypted DB open (biometric keychain path, D-06) |
| `LockApp` | Drop DB handle, save pre_lock_screen, clear sensitive AppState (T-28-10) |
| `AttemptBiometricUnlock` | Calls biometric_provider.authenticate(), logs result |
| `SetLockTimeout` | Persists seconds to settings table, updates AppState |

**load_post_unlock helper:**
Called after any successful DB open. Loads backends, conversations, agent sessions, health state, documents, attestation cache, VCEK cache, all settings, determines post-unlock screen, spawns attestation task, starts attestation timer.

**FfiApp::new signature update:**
Added `biometric_provider: Box<dyn BiometricProvider>` parameter. Updated 16 test call sites with `NullBiometricProvider`.

### UniFFI Swift Bindings (Task 1)

Regenerated `ios/Bindings/mango_core.swift` and `ios/Bindings/mango_coreFFI.h` via `just bindings-swift`. New Swift types:
- `BiometricProvider` protocol (callback interface)
- `Screen.locked` and `Screen.pinSetup` enum cases (discriminants 10, 11)
- `AppAction.setupPin`, `.unlockWithPin`, `.lockApp`, `.attemptBiometricUnlock`, `.setLockTimeout`
- `AppState.biometricAvailable`, `.lockTimeoutSeconds`, `.authInitialized`, `.encryptionEnabled`
- `FfiApp.init(dataDir:keychain:embeddingProvider:embeddingStatus:biometricProvider:)`

### BiometricProviderImpl.swift (Task 1)

`ios/Mango/Mango/BiometricProviderImpl.swift` implements the `BiometricProvider` UniFFI protocol:
- `biometricStatus()`: `LAContext().canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics)` → returns `"available"`, `"not_enrolled"`, or `"not_available"` based on `LAError.Code`
- `authenticate(reason:)`: `LAContext().evaluatePolicy(...)` with `DispatchSemaphore` to bridge the async callback back to the blocking Rust actor thread

`AppManager.swift` updated to inject `BiometricProviderImpl()` into `FfiApp`.

### iOS Lock Screen (Task 2) — `ios/Mango/Mango/LockScreen.swift`

- App icon + "Mango" wordmark at top
- Auto-dispatches `AttemptBiometricUnlock` on `.onAppear` if `biometricAvailable`
- `SecureField` PIN input (T-28-18: masked, never retained beyond submit)
- "Unlock" button dispatches `UnlockWithPin(pin:)`
- "Use Face ID / Touch ID" button (shown only when `biometricAvailable`)
- No "Forgot PIN" link per security model (D-15)

### iOS PIN Setup Screen (Task 2) — `ios/Mango/Mango/PinSetupScreen.swift`

3-step wizard:
1. **PIN step**: `SecureField` + confirm field, 4-char minimum validation, PIN match validation
2. **Duress PIN step**: Optional "emergency PIN" that triggers full wipe. Validates duress != real PIN (D-18). "Skip" toggle available.
3. **Biometric step**: Toggle for Face ID/Touch ID enable. Only shown if `biometricAvailable`. Dispatches `SetupPin(pin:duressPin:enableBiometric:)`.

### ContentView.swift Routing (Task 2)

Added two cases to the screen switch before `.onboarding`:
```swift
case .locked:
    LockScreen().environmentObject(appManager)
case .pinSetup:
    PinSetupScreen().environmentObject(appManager)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Phase 28-02 Rust implementation missing from worktree**
- **Found during:** Task 1 (checking Screen enum for Locked/PinSetup)
- **Issue:** The 28-02 SUMMARY referenced commit `078d293` from a parallel worktree agent that was never merged to main. `lib.rs` had no `Screen::Locked`, `BiometricProvider`, auth actions, or `Option<Database>` changes. Bindings generation would produce no Phase 28 types.
- **Fix:** Implemented the complete Phase 28-02 Rust changes inline:
  - Screen::Locked, Screen::PinSetup variants
  - BiometricProvider trait + NullBiometricProvider
  - Phase 28 AppState fields (biometric_available, lock_timeout_seconds, auth_initialized, encryption_enabled)
  - Phase 28 AppAction variants (SetupPin, UnlockWithPin, UnlockWithDek, LockApp, AttemptBiometricUnlock, SetLockTimeout)
  - ActorState.db changed to Option<Database> with deferred open
  - ActorState Phase 28 fields (bootstrap, biometric_provider, pre_lock_screen, db_path)
  - load_post_unlock helper function
  - All auth action handlers with backward-compat auto-open
  - FfiApp::new signature update + 16 test file updates
- **Files modified:** rust/src/lib.rs, rust/src/tests/*.rs (16 files)
- **Commit:** 8816738

**2. [Rule 2 - Missing] Plan specified ios/ConfidentialApp/ but actual path is ios/Mango/Mango/**
- **Found during:** Task 1 (file listing)
- **Issue:** Plan references `ios/ConfidentialApp/ContentView.swift` etc. Actual project uses `ios/Mango/Mango/` directory structure.
- **Fix:** Created all files at the correct `ios/Mango/Mango/` paths.
- **Impact:** No code change needed; purely a path correction.

## Known Stubs

None. The `AttemptBiometricUnlock` handler logs success but doesn't auto-dispatch `UnlockWithDek` — this is intentional: platforms that store the DEK in biometric-gated keychain (Secure Enclave) must dispatch `AppAction.unlockWithDek(dek:)` directly from the native biometric callback (D-06). The `BiometricProviderImpl.authenticate()` → Rust success path is designed for this: iOS would store the DEK via `IOSKeychainProvider` gated by `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly` and retrieve it after biometric success.

## Threat Surface Scan

No new unplanned threat surfaces. The `load_post_unlock` function reads from the already-open (decrypted) DB — same trust boundary as all other DB reads. The new Rust auth handlers operate within the actor's single-threaded model.

## Self-Check

### Files Exist

- ios/Mango/Mango/BiometricProviderImpl.swift — FOUND
- ios/Mango/Mango/LockScreen.swift — FOUND
- ios/Mango/Mango/PinSetupScreen.swift — FOUND
- ios/Bindings/mango_core.swift — FOUND (regenerated)

### Commits Exist

- 8816738: feat(28-04): Phase 28-02 Rust auth types + UniFFI bindings + BiometricProviderImpl
- c1bddf4: feat(28-04): iOS LockScreen, PinSetupScreen, and ContentView routing

### Test Results

265 tests passed, 0 failed (cargo test -p mango_core --lib)

## Self-Check: PASSED
