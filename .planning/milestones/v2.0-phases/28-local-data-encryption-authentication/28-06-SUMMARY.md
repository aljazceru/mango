---
phase: 28
plan: "06"
subsystem: desktop-iced
tags: [authentication, lock-screen, pin-setup, desktop, iced, biometric-provider]
dependency_graph:
  requires: [28-01, 28-02]
  provides: []
  affects: [desktop/iced/src/main.rs, desktop/iced/src/lock_screen.rs, desktop/iced/src/pin_setup_screen.rs, rust/src/lib.rs]
tech_stack:
  added: [BiometricProvider trait, NullBiometricProvider, setup_pin_auth, verify_pin_auth helpers]
  patterns: [iced secure text_input, Screen::Locked pre-emptive routing, PIN-only desktop auth]
key_files:
  created:
    - desktop/iced/src/lock_screen.rs
    - desktop/iced/src/pin_setup_screen.rs
  modified:
    - desktop/iced/src/main.rs
    - rust/src/lib.rs
    - rust/src/crypto/key_derivation.rs
    - rust/src/tests/ (11 test files — FfiApp::new biometric_provider param)
decisions:
  - Desktop uses NullBiometricProvider (D-23) — PIN-only, no biometric API on Linux/macOS via this path
  - Screen::Locked and Screen::PinSetup routing in view() precede all other screens so no content leaks before auth
  - PIN input cleared immediately after submit in update() handler (T-28-23 mitigation)
  - setup_pin_auth and verify_pin_auth helpers added to key_derivation.rs (high-level wrappers over existing primitives)
  - Duress PIN detection runs before DEK unwrap in verify_pin_auth (T-28-11 timing side-channel prevention)
  - Duress wipe replaces actor DB with in-memory DB before file deletion (avoids move-out-of-struct-field error)
metrics:
  duration: "~45 minutes"
  completed: "2026-04-09"
  tasks_completed: 1
  tasks_total: 2
  files_changed: 16
---

# Phase 28 Plan 06: Desktop Lock Screen and PIN Setup Summary

Desktop (iced) lock screen and PIN setup screen implemented with PIN-only authentication. BiometricProvider trait and NullBiometricProvider added to Rust core as prerequisites. Screen::Locked and Screen::PinSetup are now wired in desktop main.rs routing before any other screen.

## What Was Built

### Rust Core Additions (Prerequisites from 28-02 Not Yet Merged)

**Screen enum:**
- `Screen::Locked` — authentication gate, shown on cold launch
- `Screen::PinSetup` — first-time PIN setup, mandatory (no skip)

**BiometricProvider trait:**
```rust
pub trait BiometricProvider: Send + Sync + 'static {
    fn biometric_status(&self) -> String;
    fn authenticate(&self, reason: String) -> bool;
}
pub struct NullBiometricProvider; // always "unavailable" / false
```

**AppState Phase 28 fields:**
- `biometric_available: bool` — always false on desktop
- `lock_timeout_seconds: i64` — default 300
- `auth_initialized: bool`
- `encryption_enabled: bool`

**ActorState Phase 28 fields:**
- `biometric_provider: Box<dyn BiometricProvider>`
- `pre_lock_screen: Option<Screen>` — screen restored after unlock

**FfiApp::new signature updated:**
```rust
pub fn new(data_dir, keychain, embedding_provider, embedding_status, biometric_provider) -> Arc<Self>
```

**Auth action handlers:**
| Action | Behavior |
|--------|----------|
| `SetupPin` | Derive KEK, wrap DEK, write bootstrap DB, navigate to Home |
| `UnlockWithPin` | Derive KEK, check duress PIN first (T-28-11), unwrap DEK, navigate to pre-lock screen |
| `UnlockWithDek` | Direct DEK unlock (mobile biometric path), navigate to pre-lock screen |
| `LockApp` | Save pre-lock screen, clear sensitive state (T-28-10), route to Locked |
| `AttemptBiometricUnlock` | Call BiometricProvider.authenticate() — no-op on desktop |
| `SetLockTimeout` | Persist to settings table, update AppState |

**key_derivation.rs helpers:**
- `setup_pin_auth(pin, duress_pin, bootstrap)` — full first-time setup flow
- `verify_pin_auth(pin, bootstrap)` — duress check first, then KEK derive + DEK unwrap

### Desktop Files Created

**`desktop/iced/src/lock_screen.rs`:**
- Centered card layout: app name, "Enter your PIN to unlock", secure text_input (masked, T-28-23), Unlock button
- Error message row shown when `state.toast` is Some (wrong PIN feedback)
- No biometric button (D-23)
- PIN cleared after submit in update handler

**`desktop/iced/src/pin_setup_screen.rs`:**
- Centered card: PIN, Confirm PIN, optional duress PIN fields (all masked, T-28-23)
- Client-side validation: min 4 chars, PINs must match, duress must differ from real PIN (D-18)
- Set PIN button disabled (gray) when validation fails; enabled (accent) when valid
- `build_setup_pin_action()` helper constructs `AppAction::SetupPin` with `enable_biometric: false` (D-23)
- Inline error text shows relevant validation message

**`desktop/iced/src/main.rs` wiring:**
- `mod lock_screen; mod pin_setup_screen;` added
- `NullBiometricProvider` added to imports and passed to `FfiApp::new`
- Message variants: `UnlockPinChanged`, `UnlockSubmit`, `PinSetupPinChanged`, `PinSetupConfirmChanged`, `PinSetupDuressChanged`, `PinSetupSubmit`
- State fields: `lock_pin_input`, `setup_pin_input`, `setup_confirm_input`, `setup_duress_input`
- `view()` routes `Screen::Locked` → `lock_screen::view()` and `Screen::PinSetup` → `pin_setup_screen::view()` BEFORE any other screen check

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Functionality] Rust core Phase 28-02 prerequisites not fully merged**
- **Found during:** Task 1 (reading lib.rs showed Screen::Locked/PinSetup absent despite 28-02 SUMMARY claiming they were added)
- **Issue:** The merged commit `c717956` for Plan 28-02 only contained minor VectorIndex changes. Screen::Locked, Screen::PinSetup, BiometricProvider trait, and auth AppAction variants were not present in the codebase.
- **Fix:** Added all required Rust core components as part of Plan 28-06 execution — Screen variants, BiometricProvider trait, NullBiometricProvider, AppState auth fields, ActorState biometric fields, FfiApp::new biometric_provider param, and all auth action handlers.
- **Files modified:** `rust/src/lib.rs`, `rust/src/crypto/key_derivation.rs`
- **Commit:** aebd632

**2. [Rule 2 - Missing Functionality] setup_pin_auth / verify_pin_auth helpers absent**
- **Found during:** Task 1 (auth action handlers referenced functions that didn't exist in key_derivation.rs)
- **Fix:** Added `setup_pin_auth()`, `verify_pin_auth()`, and `PinVerifyResult` to `key_derivation.rs` as high-level wrappers over existing `generate_dek/salt`, `derive_kek`, `wrap/unwrap_dek`, `hash_pin`, `verify_pin_hash` primitives.
- **Files modified:** `rust/src/crypto/key_derivation.rs`
- **Commit:** aebd632

**3. [Rule 1 - Bug] FfiApp::new call sites in 11 test files missing biometric_provider argument**
- **Found during:** Task 1 (cargo test showed 16 compilation errors for wrong argument count)
- **Fix:** Added `Box::new(NullBiometricProvider),` and `NullBiometricProvider` import to all affected test files.
- **Files modified:** `rust/src/tests/` (11 files)
- **Commit:** aebd632

**4. [Rule 1 - Bug] iced `.password()` method doesn't exist in iced 0.14**
- **Found during:** Task 1 (cargo build --bin mango-desktop error)
- **Fix:** Replaced `.password()` with `.secure(true)` matching the pattern used in existing settings.rs and onboarding.rs files.
- **Files modified:** `desktop/iced/src/lock_screen.rs`, `desktop/iced/src/pin_setup_screen.rs`
- **Commit:** aebd632

**5. [Rule 1 - Bug] Lifetime error — can't return borrowed local in pin_setup_screen view()**
- **Found during:** Task 1 (E0515 compiler error)
- **Fix:** Changed `if let Some(err) = &inline_err` to `if let Some(err) = inline_err` to take ownership, so the text widget holds owned String.
- **Files modified:** `desktop/iced/src/pin_setup_screen.rs`
- **Commit:** aebd632

## Known Stubs

None — all implemented functionality is fully wired. `NullBiometricProvider` is intentional (production implementations are in native platform layers via UniFFI for iOS/Android; desktop is PIN-only per D-23).

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: information_disclosure | desktop/iced/src/lock_screen.rs | PIN field uses `.secure(true)` (masked) per T-28-23 — mitigated |
| threat_flag: information_disclosure | desktop/iced/src/main.rs | `lock_pin_input` cleared after submit in UnlockSubmit handler — mitigated |

## Self-Check

### Files Exist
- desktop/iced/src/lock_screen.rs — FOUND
- desktop/iced/src/pin_setup_screen.rs — FOUND
- desktop/iced/src/main.rs has Screen::Locked routing — FOUND
- desktop/iced/src/main.rs has Screen::PinSetup routing — FOUND
- desktop/iced/src/main.rs has NullBiometricProvider — FOUND

### Commits Exist
- aebd632: feat(28-06): desktop lock screen and PIN setup (iced) — FOUND

### Build Verification
- `cargo test -p mango_core`: 265 passed; 0 failed
- `cargo build --bin mango-desktop`: Finished (no errors)

## Self-Check: PASSED
