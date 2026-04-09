---
phase: 28
plan: "02"
subsystem: rust-core
tags: [authentication, encryption, actor-model, sqlcipher, biometric, pin-auth]
dependency_graph:
  requires: [28-01]
  provides: [28-03, 28-04, 28-05, 28-06, 28-07]
  affects: [rust/src/lib.rs, rust/src/persistence/]
tech_stack:
  added: [BiometricProvider trait, NullBiometricProvider, BootstrapDb integration]
  patterns: [Option<Database> deferred open, auth-gate pattern, load_post_unlock helper]
key_files:
  created: []
  modified:
    - rust/src/lib.rs
    - rust/src/persistence/error.rs
    - rust/src/persistence/mod.rs
    - rust/src/crypto/ (all 4 files — present from 28-01)
decisions:
  - Backward-compat auto-open: when no auth params AND plaintext DB exists, open it directly (no PIN required until user opts into SetupPin)
  - In-memory (empty data_dir) also auto-opens without auth for test compatibility
  - load_post_unlock sets encryption_enabled=true; auto-open path resets it to false
  - Duress PIN detection runs before DEK unwrap to prevent timing side-channels (T-28-11)
  - LockApp clears sensitive state from AppState (T-28-10)
metrics:
  duration: "~40 minutes (continued from prior session)"
  completed: "2026-04-09"
  tasks_completed: 1
  tasks_total: 1
  files_changed: 18
---

# Phase 28 Plan 02: Encrypt-First Actor Restructure Summary

Restructured `rust/src/lib.rs` for encrypted-first operation: `ActorState.db` is now `Option<Database>` (None until unlock), `Screen::Locked`/`Screen::PinSetup` control the pre-auth UI, and six auth action handlers implement the full PIN-to-DEK flow with backward-compatible plaintext DB auto-open.

## What Was Built

### Screen Variants Added
- `Screen::Locked` — shown when returning user needs to unlock
- `Screen::PinSetup` — shown on first launch / no auth params

### BiometricProvider Trait
```rust
pub trait BiometricProvider: Send + Sync + 'static {
    fn biometric_status(&self) -> String;
    fn authenticate(&self, reason: String) -> bool;
}
pub struct NullBiometricProvider;
```
Follows the existing `KeychainProvider` callback_interface pattern. `FfiApp::new` accepts `Box<dyn BiometricProvider>`.

### ActorState Changes
- `db: Option<persistence::Database>` — None until unlock
- `bootstrap: crypto::bootstrap_db::BootstrapDb` — always open pre-unlock
- `biometric_provider: Box<dyn BiometricProvider>`
- `pre_lock_screen: Option<Screen>` — screen to restore after unlock (D-12)

### AppState Phase 28 Fields
- `biometric_available: bool`
- `lock_timeout_seconds: i64` (default 300)
- `auth_initialized: bool`
- `encryption_enabled: bool`

### Auth Action Handlers
| Action | Behavior |
|--------|----------|
| `SetupPin` | Derive KEK via Argon2id, wrap DEK with AES-256-GCM, write BootstrapDb, migrate plaintext DB to SQLCipher, call load_post_unlock |
| `UnlockWithDek` | Open SQLCipher DB with provided DEK hex, call load_post_unlock |
| `UnlockWithPin` | Read auth params, derive KEK, unwrap DEK, detect duress PIN (T-28-11), open DB |
| `LockApp` | Drop db to None, save pre_lock_screen, clear sensitive AppState (T-28-10) |
| `AttemptBiometricUnlock` | Call biometric_provider.authenticate(), dispatch UnlockWithPin on success |
| `SetLockTimeout` | Persist to settings, update AppState |

### load_post_unlock Helper
Called after any successful DB open. Loads backends, conversations, agent sessions, documents, settings, attestation cache, VCEK cache, memory count, brave key status. Determines post-unlock screen (pre_lock_screen > Onboarding > Home).

### DB-Dependent Handler Guards
All handlers that require DB access use `actor_state.db.as_ref().expect("db unlocked")`. Helper functions (`refresh_conversations`, `reload_backends`, etc.) return early if `db` is None.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Functionality] Backward-compat auto-open for pre-encryption databases**
- **Found during:** Task 1 (test run)
- **Issue:** Plan only specified PIN/DEK auth flow. Persistence tests pre-populate a plaintext `mango.db` and expect it to load on startup — without Phase 28 auth enrolled. With `db: None` until PIN setup, 45 tests failed.
- **Fix:** Added dual auto-open logic at actor startup:
  - Empty `data_dir` (in-memory/test mode): always auto-open `:memory:`
  - Non-empty `data_dir` with no auth params AND existing plaintext `mango.db`: auto-open directly (pre-encryption backward compat)
  - `encryption_enabled` and `auth_initialized` set to `false` for these paths so UI can offer SetupPin
- **Files modified:** `rust/src/lib.rs` (actor init block)
- **Commit:** 078d293

**2. [Rule 1 - Bug] `PersistenceError::DecryptionFailed` missing from worktree**
- **Found during:** Task 1 (cargo build)
- **Issue:** `persistence/mod.rs` uses `DecryptionFailed` but worktree's `persistence/error.rs` lacked it
- **Fix:** Added `DecryptionFailed { message: String }` variant + Display impl + From conversion
- **Files modified:** `rust/src/persistence/error.rs`
- **Commit:** 078d293

**3. [Rule 1 - Bug] Type comparison `&String` vs `String`**
- **Found during:** Task 1 (cargo build)
- **Issue:** `&b.id == *active_id` failed with `E0277: can't compare &String with String`
- **Fix:** Removed extraneous `&` prefix: `b.id == *active_id`
- **Files modified:** `rust/src/lib.rs`
- **Commit:** 078d293

**4. [Rule 2 - Missing Functionality] NullBiometricProvider argument in all test FfiApp::new calls**
- **Found during:** Task 1 (cargo test)
- **Issue:** Adding `biometric_provider` param to `FfiApp::new` broke all 45 test call sites
- **Fix:** Added `Box::new(crate::NullBiometricProvider),` to all test `FfiApp::new` calls
- **Files modified:** All `rust/src/tests/*.rs` files
- **Commit:** 078d293

## Known Stubs

None — all implemented functionality is fully wired. The `NullBiometricProvider` is intentional (production implementations are in native platform layers via UniFFI).

## Self-Check

### Files Exist
- rust/src/lib.rs — modified in place
- rust/src/persistence/error.rs — has DecryptionFailed variant
- rust/src/persistence/mod.rs — has open_encrypted, is_encrypted, migrate_to_encrypted

### Commit Exists
- 078d293: feat(28-02): encrypt-first actor
- Test result: 234 passed; 0 failed

## Self-Check: PASSED
