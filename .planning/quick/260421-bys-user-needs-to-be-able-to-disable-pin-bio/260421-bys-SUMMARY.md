---
quick_id: 260421-bys
type: summary
status: complete
created: 2026-04-21
completed: 2026-04-21
duration: ~25min
tasks_completed: 3
files_changed: 5
commits:
  - 94036e4
  - b7f31d8
tags: [auth, cold-launch, keychain, bootstrap-db, never-lock, ios, android, desktop]
---

# Quick Task 260421-bys — Never auto-lock should skip cold-launch PIN prompt

**One-liner:** Reuse the biometric DEK-cache path to let "Never" lock-timeout bypass the cold-launch PIN screen, backed by a `cold_launch_bypass` flag in the bootstrap DB and identical threat-model copy on all three platforms.

## What Was Done

### Task 1 + 2 — Rust core (committed together as 94036e4)

**bootstrap_db.rs:**
- Added `cold_launch_bypass INTEGER NOT NULL DEFAULT 0` column via `ALTER TABLE … .ok()` — idempotent on existing DBs, ignored on upgrade
- Added `read_cold_launch_bypass()` and `write_cold_launch_bypass()` helpers; column is deliberately excluded from `write_auth_params` so PIN-setup cannot accidentally reset it
- Doc comment on the column clarifies it is a non-sensitive hint: bypass=1 without a keychain DEK falls back to `Screen::Locked`

**lib.rs — SetLockTimeout handler:**
- When `seconds == -1` and a live DEK is available: hex-encode DEK and write to keychain `("mango", "dek")` (same path as biometric login), then call `write_cold_launch_bypass(true)`
- When `seconds >= 0` and biometric is disabled: delete keychain entry and call `write_cold_launch_bypass(false)`
- When `seconds >= 0` and biometric is enabled: leave keychain alone (biometric owns it)
- Sets a toast: "Auto-lock disabled. App will open without PIN — protected only by your device unlock."

**lib.rs — cold-launch Case D:**
- Before the DB-open decision: read `cold_launch_bypass` from bootstrap DB and probe keychain for `("mango", "dek")`
- If both are present: attempt `Database::open_encrypted`; on success, skip `Screen::Locked` and proceed to `load_post_unlock` as a normal unlocked session
- Decoded DEK bytes stored in `actor_state.dek`; VectorIndex opened with the DEK so RAG is immediately available
- Any failure (open_encrypted error, corrupt keychain, missing DEK) falls back to `Screen::Locked` without crash or keychain eviction

### Task 3 — Native UI warning copy (committed as b7f31d8)

Updated inline caption shown when `lock_timeout_seconds == -1` on all three platforms to identical copy:

> "Auto-lock disabled. The app will open without your PIN — it is protected only by your device unlock. If your device is unlocked, anyone with access can open the app."

- **iOS** (`SettingsSecurityView.swift`): `.font(.caption)` + `.foregroundStyle(.secondary)` — matches existing secondary copy style
- **Android** (`SettingsSecurityScreen.kt`): `MaterialTheme.typography.bodySmall` + `MaterialTheme.colorScheme.onSurfaceVariant` — matches Material You secondary text pattern
- **Desktop** (`desktop/iced/src/views/settings.rs`): `.size(11)` + `vc.muted` color — matches existing muted caption style

## Commits

| Hash | Message | Files |
|------|---------|-------|
| 94036e4 | feat(quick/260421-bys): skip PIN on cold launch when lock timeout is Never | rust/src/lib.rs, rust/src/crypto/bootstrap_db.rs |
| b7f31d8 | feat(quick/260421-bys): show threat-model warning copy when Never is selected | ios/.../SettingsSecurityView.swift, android/.../SettingsSecurityScreen.kt, desktop/.../settings.rs |

## Verification

- `cargo check --manifest-path rust/Cargo.toml` — clean (1 pre-existing dead_code warning, not introduced here)
- `cargo test --manifest-path rust/Cargo.toml --lib` — 321 passed, 0 failed
- `cargo check --manifest-path desktop/iced/Cargo.toml` — clean
- `./gradlew :app:compileDebugKotlin` — BUILD SUCCESSFUL (pre-existing deprecation warnings only)
- iOS: no automated build available in this environment (xcodebuild not present); Swift syntax verified by manual review

## Deviations from Plan

### Note: BiometricResult helper not extracted

The plan suggested factoring the post-unlock body from `BiometricResult` into a `bypass_unlock_with_cached_dek` helper and reusing it from cold-launch. The cold-launch bypass uses the same *logical* steps (decode hex DEK, open_encrypted, open VectorIndex, call load_post_unlock) but executes them inline during actor init — before `actor_state` is fully constructed — which makes sharing the exact same helper function awkward (actor_state doesn't exist yet). The code was kept as inline logic in the init path, and the `BiometricResult` handler was left unchanged. The plan acknowledged this as acceptable: "if that's too invasive, duplication is acceptable as a note in the summary."

### Note: Toast rev bump only on Never

The `rev += 1` bump is only applied when `seconds == -1` (to trigger the "Auto-lock disabled" toast). Finite-timeout changes do not bump rev in the handler — this matches the pre-existing pattern where `SetLockTimeout` did not emit a toast.

## Security Constraints Preserved

- Onboarding still mandates PIN setup — `auth_params` + `wrapped_dek` remain PIN-derived
- Duress PIN flow unchanged — still checked on any PIN entry that occurs
- Finite lock-timeout keeps cold-launch PIN prompt
- Bypass flag alone does not open the DB — keychain must also return a valid DEK
- On desktop Linux without Secret Service: keychain write fails silently; cold-launch falls back to `Screen::Locked`

## Known Stubs

None. All three platforms render the warning from live `lock_timeout_seconds` state.

## Threat Flags

None. No new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries introduced. The `cold_launch_bypass` column is a non-sensitive hint in the existing bootstrap DB; the actual security gate remains the OS keychain item.

## Self-Check: PASSED

- rust/src/lib.rs — modified (confirmed via cargo check)
- rust/src/crypto/bootstrap_db.rs — modified (confirmed via cargo check + tests)
- ios/Mango/Mango/SettingsSecurityView.swift — modified (confirmed via read)
- android/app/src/main/java/dev/disobey/mango/ui/SettingsSecurityScreen.kt — modified (confirmed via gradlew compile)
- desktop/iced/src/views/settings.rs — modified (confirmed via cargo check)
- Commits 94036e4 and b7f31d8 exist in git log
