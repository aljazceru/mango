---
phase: 28
plan: "08"
subsystem: rust-core
tags: [biometric-unlock, keychain, encryption, gap-closure, ENC-04, ENC-09]
dependency_graph:
  requires: [28-02, 28-04, 28-05]
  provides: [biometric-keychain-unlock]
  affects:
    - rust/src/lib.rs
tech_stack:
  added: []
  patterns:
    - SetupPin stores DEK hex via keychain.store("mango","dek",...) when enable_biometric=true
    - BiometricResult success loads DEK via keychain.load("mango","dek") then calls open_encrypted + load_post_unlock
    - Graceful fallback to PIN when keychain has no DEK (biometric not enrolled at setup)
    - Graceful fallback to PIN via error toast when open_encrypted fails after keychain load
key_files:
  created: []
  modified:
    - rust/src/lib.rs
decisions:
  - "SetupPin: change enable_biometric: _ to enable_biometric; after dek_hex computed, call keychain.store(mango,dek,dek_hex) when enable_biometric=true (ENC-04)"
  - "BiometricResult: on success, keychain.load(mango,dek); if Some -> open_encrypted + load_post_unlock + continue; if None -> biometric_authenticated=true for PIN fallback (ENC-09)"
  - "Loaded dek_hex is plain String (not Zeroizing) — brief stack lifetime acceptable, same pattern as UnlockWithPin derived DEK hex (T-28-GC-04: accepted)"
metrics:
  duration: "~10 minutes"
  completed: "2026-04-09"
  tasks_completed: 2
  tasks_total: 2
  files_changed: 1
---

# Phase 28 Plan 08: Biometric Unlock Gap Closure Summary

DEK keychain store wired in SetupPin (ENC-04) and full keychain-backed biometric unlock path implemented in BiometricResult (ENC-09) — biometric success alone now opens the encrypted DB and transitions to Home without PIN entry.

## What Was Built

### Task 1: Store DEK in platform keychain when biometric is enabled during PIN setup

**File: `rust/src/lib.rs` (SetupPin handler)**

- Changed `enable_biometric: _` to `enable_biometric` in the `AppAction::SetupPin` match arm — parameter was previously silently ignored
- After `dek_hex: Zeroizing<String>` is computed (raw DEK as hex string), added conditional keychain store:

```rust
if enable_biometric {
    actor_state.keychain.store(
        "mango".to_string(),
        "dek".to_string(),
        (*dek_hex).clone(),
    );
    log::info!("[auth] SetupPin: DEK stored in platform keychain for biometric unlock");
}
```

- `(*dek_hex).clone()` dereferences the `Zeroizing<String>` wrapper to get `&String`, then clones to an owned `String` for `store()` which takes `String` by value
- Closes **ENC-04**: DEK is now cached in the platform keychain (Secure Enclave on iOS, StrongBox on Android, keyring on Desktop) when the user opts into biometric unlock during PIN setup

### Task 2: Wire BiometricResult success to load DEK from keychain and open encrypted DB

**File: `rust/src/lib.rs` (BiometricResult internal event handler)**

Replaced the placeholder handler (which only set `biometric_authenticated=true`) with the full keychain-backed unlock flow:

1. On `success=true`: call `actor_state.keychain.load("mango", "dek")`
2. **If DEK present** (`Some(dek_hex)`):
   - Log `"[auth] Biometric unlock: DEK loaded from keychain — opening DB"`
   - Call `persistence::Database::open_encrypted(&actor_state.db_path, &dek_hex)`
   - On success: set `encryption_enabled`, call `load_post_unlock(true)`, `continue` (load_post_unlock already emits state)
   - On `open_encrypted` failure: show error toast, set `biometric_authenticated=true` (PIN fallback), emit, `continue`
3. **If DEK absent** (`None` — biometric not enabled at setup or keychain cleared):
   - Log `"[auth] Biometric succeeded but no DEK in keychain — falling back to PIN"`
   - Set `biometric_authenticated=true` (existing PIN-prompt behavior preserved)
4. On `success=false`: show "Biometric authentication failed." toast (unchanged behavior)

The `:memory:` branch opens a plaintext DB (consistent with test patterns in UnlockWithDek and UnlockWithPin handlers).

Closes **ENC-09**: biometric success alone unlocks the app when DEK is in the keychain. PIN fallback preserved for all error/absent-DEK cases.

## Task Commits

1. **Task 1: DEK keychain store in SetupPin** — `b15be5b` (feat)
2. **Task 2: BiometricResult keychain-backed unlock** — `de64310` (feat)

## Deviations from Plan

None — plan executed exactly as written. Both tasks followed the implementation spec verbatim including the exact `(*dek_hex).clone()` pattern for the Zeroizing dereference.

## Known Stubs

None. Both keychain paths use the real `KeychainProvider` trait that is implemented by platform-native code (Secure Enclave on iOS, StrongBox on Android, keyring on Desktop). The `NullKeychainProvider` in tests returns `None` for `load()` which exercises the PIN fallback path.

## Threat Flags

No new threat surface. Both changes are within the existing trust boundary documented in the plan's threat model:
- `T-28-GC-01`: BiometricResult is dispatched only from the internal `spawn_blocking` task — no external forgery possible
- `T-28-GC-02`: DEK in keychain is hardware-backed storage — design intent per D-05/D-06
- `T-28-GC-03`: PIN fallback preserved when keychain returns None — no auth bypass
- `T-28-GC-04`: Plain String lifetime of loaded dek_hex is brief (stack-only, block exit drops it) — accepted

## Self-Check

### Files Exist

- rust/src/lib.rs — FOUND (SetupPin + BiometricResult handlers modified)

### Commits Exist

- b15be5b: feat(28-08): store DEK in platform keychain when biometric enabled during SetupPin — FOUND
- de64310: feat(28-08): wire BiometricResult success to load DEK from keychain and open encrypted DB — FOUND

### Build and Test Verification

- `cargo build --manifest-path rust/Cargo.toml`: Finished (no errors)
- `cargo test --manifest-path rust/Cargo.toml`: 265 passed; 0 failed

### Acceptance Criteria

| Check | Result |
|-------|--------|
| `enable_biometric: _` in lib.rs — zero matches | PASS (0 matches) |
| `keychain.store` in SetupPin region (lines 4300-4400) | PASS (line 4328) |
| `keychain.load` in BiometricResult region (lines 5500-5600) | PASS (line 5533) |
| `"Biometric unlock: DEK loaded from keychain"` log line | PASS (line 5538) |
| `open_encrypted` in BiometricResult handler | PASS (confirmed) |
| `load_post_unlock` in BiometricResult handler | PASS (confirmed) |
| `cargo build` succeeds | PASS |
| `cargo test` 265 passed, 0 failed | PASS |

## Self-Check: PASSED
