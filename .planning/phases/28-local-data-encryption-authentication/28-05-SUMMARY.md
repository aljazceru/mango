---
phase: 28-local-data-encryption-authentication
plan: "05"
subsystem: android-ui
tags: [android, biometric, authentication, lock-screen, compose, uniffi, countdownlatch]
dependency_graph:
  requires:
    - phase: 28-02
      provides: BiometricProvider trait, Screen::Locked/PinSetup, auth AppActions, crypto module
  provides:
    - Android BiometricProviderImpl with CountDownLatch bridge
    - LockScreen Compose composable
    - PinSetupScreen Compose composable
    - Kotlin UniFFI bindings regenerated with Phase 28 types
  affects: [android, ios, desktop]
tech_stack:
  added:
    - androidx.biometric:biometric:1.2.0-alpha05
    - androidx.appcompat:appcompat:1.7.0
  patterns:
    - CountDownLatch bridge: blocks Rust actor thread while Android async BiometricPrompt callback completes
    - WeakReference<FragmentActivity>: avoids Activity leak during orientation changes or destruction
    - AppCompatActivity base class for BiometricPrompt + Compose compatibility
key_files:
  created:
    - android/app/src/main/java/dev/disobey/mango/BiometricProviderImpl.kt
    - android/app/src/main/java/dev/disobey/mango/ui/LockScreen.kt
    - android/app/src/main/java/dev/disobey/mango/ui/PinSetupScreen.kt
  modified:
    - android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt
    - android/app/src/main/java/dev/disobey/mango/AppManager.kt
    - android/app/src/main/java/dev/disobey/mango/MainActivity.kt
    - android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt
    - android/app/build.gradle.kts
    - rust/src/lib.rs
    - desktop/iced/src/main.rs
key_decisions:
  - "MainActivity extends AppCompatActivity (not ComponentActivity) to satisfy FragmentActivity requirement for BiometricPrompt"
  - "AppManager accepts FragmentActivity? parameter; BiometricProviderImpl created at FfiApp init time with the activity reference"
  - "Bindings regenerated in this plan (28-05 Android) since the Rust working tree had Phase 28 types uncommitted"
requirements-completed: [ENC-07, ENC-09, ENC-11, ENC-12, ENC-13, ENC-14]
duration: ~35 min
completed: "2026-04-09"
---

# Phase 28 Plan 05: Android Lock Screen and BiometricProviderImpl Summary

**Android lock gate with BiometricPrompt CountDownLatch bridge, Compose LockScreen/PinSetupScreen, and regenerated UniFFI Kotlin bindings with all Phase 28 auth types**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-09
- **Completed:** 2026-04-09
- **Tasks:** 2 of 2 complete (Task 3 is a human-verify checkpoint)
- **Files modified:** 10

## Accomplishments

- Created `BiometricProviderImpl.kt` implementing the Rust `BiometricProvider` callback interface using Android `BiometricPrompt` (Class 3 / BIOMETRIC_STRONG) with a `CountDownLatch` synchronous bridge so the Rust actor thread blocks until the async Android callback fires
- Regenerated Kotlin UniFFI bindings exposing `Screen.Locked`, `Screen.PinSetup`, `BiometricProvider` interface, `AppAction.SetupPin/UnlockWithPin/UnlockWithDek/LockApp/AttemptBiometricUnlock/SetLockTimeout`, and `AppState` Phase 28 fields
- Created `LockScreen.kt` Compose screen with PIN entry (`PasswordVisualTransformation`), auto-dispatch `AttemptBiometricUnlock` on entry if biometric enrolled, toast error display
- Created `PinSetupScreen.kt` Compose screen with PIN + confirm + optional duress PIN + biometric toggle; full client-side validation (min 4 chars, PINs match, duress differs from real PIN)
- Wired both screens into `MainApp.kt` routing (`Screen.Locked` and `Screen.PinSetup` cases)
- Fixed Rust type comparison bug (`&b.id == *active_id` → `b.id == *active_id`) and added `NullBiometricProvider` to desktop `FfiApp::new` call to restore compilation

## Task Commits

1. **Task 1: Android BiometricProviderImpl** - `fe8e416` (feat)
2. **Task 2: LockScreen and PinSetupScreen composables** - `1fec87b` (feat)

## Files Created/Modified

- `android/app/src/main/java/dev/disobey/mango/BiometricProviderImpl.kt` - BiometricPrompt + CountDownLatch bridge implementing Rust BiometricProvider trait
- `android/app/src/main/java/dev/disobey/mango/ui/LockScreen.kt` - Lock gate composable with PIN input and biometric auto-prompt
- `android/app/src/main/java/dev/disobey/mango/ui/PinSetupScreen.kt` - First-time PIN setup with optional duress PIN and biometric enrollment toggle
- `android/app/src/main/java/dev/disobey/mango/ui/MainApp.kt` - Added Screen.Locked and Screen.PinSetup routing cases
- `android/app/src/main/java/dev/disobey/mango/rust/mango_core.kt` - Regenerated UniFFI Kotlin bindings (Phase 28 types)
- `android/app/src/main/java/dev/disobey/mango/AppManager.kt` - Accept FragmentActivity param, pass BiometricProviderImpl to FfiApp, add Phase 28 AppState defaults
- `android/app/src/main/java/dev/disobey/mango/MainActivity.kt` - Extend AppCompatActivity, pass `this` to AppManager.getInstance
- `android/app/build.gradle.kts` - Add biometric:1.2.0-alpha05 and appcompat:1.7.0 dependencies
- `rust/src/lib.rs` - Fix type comparison bug in load_post_unlock
- `desktop/iced/src/main.rs` - Add NullBiometricProvider to FfiApp::new

## Decisions Made

- **AppCompatActivity over ComponentActivity:** BiometricPrompt 1.2.0-alpha05 requires `FragmentActivity`. `AppCompatActivity` extends `FragmentActivity` and fully supports Compose via `setContent`. Changed `MainActivity` base class.
- **Activity injection via AppManager constructor:** AppManager singleton is created in `MainActivity.onCreate()` which has a `FragmentActivity` reference. Added optional `activity: FragmentActivity?` parameter with null-safe fallback to inline NullBiometricProvider.
- **Bindings regenerated in this plan:** The Rust working tree had Phase 28 additions (BiometricProvider trait, Screen::Locked/PinSetup, auth AppActions) in uncommitted state — `just bindings-kotlin` regenerated from the compiled `.so` after fixing the type comparison bug.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Rust compilation errors prevented bindings regeneration**
- **Found during:** Task 1 (bindings regeneration prerequisite)
- **Issue:** Working tree `rust/src/lib.rs` had type comparison `&b.id == *active_id` (E0277) and the desktop binary missing `NullBiometricProvider` in `FfiApp::new`. Both prevented `cargo build --release` which is required before `just bindings-kotlin`.
- **Fix:** Fixed type comparison to `b.id == *active_id`; added `NullBiometricProvider` import and argument to desktop `FfiApp::new`.
- **Files modified:** `rust/src/lib.rs`, `desktop/iced/src/main.rs`
- **Verification:** `cargo test -p mango_core` — 265 passed, 0 failed
- **Committed in:** fe8e416 (Task 1 commit)

**2. [Rule 3 - Blocking] MainActivity extends ComponentActivity, incompatible with FragmentActivity**
- **Found during:** Task 1 (first assembleDebug attempt)
- **Issue:** `BiometricProviderImpl` requires `FragmentActivity`; `ComponentActivity` is not in that hierarchy. Build error: "Argument type mismatch: actual type is MainActivity, but FragmentActivity? was expected."
- **Fix:** Changed `MainActivity` to extend `AppCompatActivity` (which extends `FragmentActivity`). Added `androidx.appcompat:appcompat:1.7.0` dependency.
- **Files modified:** `android/app/src/main/java/dev/disobey/mango/MainActivity.kt`, `android/app/build.gradle.kts`
- **Verification:** `./gradlew assembleDebug` — BUILD SUCCESSFUL
- **Committed in:** fe8e416 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary to unblock bindings regeneration and Android build. No scope creep.

## Known Stubs

None — all functionality fully wired. `LockScreen` and `PinSetupScreen` dispatch real AppActions that invoke the Rust auth handlers implemented in Phase 28-02.

## Threat Flags

None — no new network endpoints, auth paths, or schema changes introduced beyond the threat model already specified in the plan.

## Self-Check

### Files Exist
- android/app/src/main/java/dev/disobey/mango/BiometricProviderImpl.kt — FOUND
- android/app/src/main/java/dev/disobey/mango/ui/LockScreen.kt — FOUND
- android/app/src/main/java/dev/disobey/mango/ui/PinSetupScreen.kt — FOUND

### Commits Exist
- fe8e416: feat(28-05): Android BiometricProviderImpl with CountDownLatch bridge
- 1fec87b: feat(28-05): Android LockScreen and PinSetupScreen composables with routing

## Self-Check: PASSED
