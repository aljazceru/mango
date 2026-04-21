---
quick_id: 260421-bys
type: execute
status: ready
created: 2026-04-21
updated: 2026-04-21
interpretation: "C — honor lock_timeout=Never on cold launch by reusing the keychain DEK cache"
---

# Quick Task 260421-bys — Never auto-lock should skip cold-launch PIN prompt

**Request:** "user needs to be able to disable pin/biometric login if they want"
**Chosen interpretation (C):** When the user has explicitly set `lock_timeout_seconds == -1` ("Never"), cold launch must not force PIN entry. The PIN is still set during onboarding (Phase 28 D-14) and is still required when the user chooses a finite timeout. Biometric login toggle is unchanged.

## Pre-plan discovery — device-keystore DEK path EXISTS

Good news: the keystore-wrapped DEK path is **already implemented and in production**. It's the biometric-login cache:

- Store: `rust/src/lib.rs:6220-6226` — on `SetBiometricLoginEnabled { enabled: true }`, the raw DEK is hex-encoded and written to `actor_state.keychain.store("mango", "dek", ...)`. Platform keychain/keystore holds it (iOS Keychain, Android Keystore-backed EncryptedSharedPrefs, desktop `keyring`).
- Load: `rust/src/lib.rs:7770-7820` — on successful biometric auth, DEK is loaded back from keychain and used to open SQLCipher + VectorIndex.
- Delete: `rust/src/lib.rs:6227-6230` — on `SetBiometricLoginEnabled { enabled: false }`, keychain entry deleted.

Threat-model delta between biometric-only and "Never":
- Biometric today: keychain entry exists, but access requires a successful biometric prompt before `BiometricResult` handler runs.
- Never (this plan): keychain entry exists, and cold launch reads it with no prompt at all. Protection reduces to OS account/device-unlock gating the keychain item itself. This is the Phase 28 "biometric failure" threat surface (attacker with unlocked OS) — not a new one. **We are not inventing a new crypto path.**

No new crypto code. No schema changes. No Phase 28 D-14/D-24 reversal — PIN is still set in onboarding, still required when the user picks a finite timeout.

## Security constraints preserved

- Onboarding still mandates PIN setup (D-14). No "skip PIN" UX added.
- `auth_params` row still written, `wrapped_dek` still PIN-wrapped. Bootstrap DB unchanged.
- Duress PIN flow unchanged (still verified on any PIN entry that does occur).
- Finite `lock_timeout_seconds` (default 300) keeps the current cold-launch PIN prompt.
- Desktop: same story as biometric login today. Desktop has no biometric toggle (D-23), so this plan intentionally makes "Never" a no-op on desktop cold launch too — but **only** when the keychain actually holds a DEK. See Task 1 guard.

## Threat-model delta (user-visible)

When a user picks "Never":
- ✅ App content stays inaccessible without OS account unlock (keychain/keystore item is OS-gated).
- ⚠️ Lost-device attacker with an unlocked OS session can open the app and read all data. Same guarantee level as biometric login with a PIN bypass.
- ⚠️ On desktop, `keyring` backends vary (macOS Keychain = strong; Linux Secret Service = depends on session lock state). We surface this as a warning in the lock-timeout picker when "Never" is selected (Task 3).

## Scope

Three atomic tasks. No UniFFI changes (action already exists: `SetLockTimeout`). No new `AppAction` variants.

---

## Task 1 — Rust: cache DEK in keychain when Never is selected; skip `Screen::Locked` on cold launch

<task type="auto">
  <name>Task 1: Rust core — persist DEK to keychain on SetLockTimeout(Never) and branch cold-launch screen</name>
  <files>rust/src/lib.rs</files>
  <action>
Two changes in `rust/src/lib.rs`, both in the existing actor loop / init path.

**Change 1: `SetLockTimeout` handler (around L6241-6250)** — when the new value is `-1` AND `actor_state.dek` is `Some(..)`, cache the DEK in the keychain using the exact same format biometric login uses (hex-encoded under namespace `"mango"`, key `"dek"`). When the new value is >= 0 AND biometric login is NOT enabled, delete the keychain entry. When the new value is >= 0 AND biometric login IS enabled, leave the entry alone (biometric needs it).

Pseudocode to add after the existing `set_setting` write in that handler:

```rust
// Quick 260421-bys: Never mode reuses the biometric DEK-cache path.
// - seconds == -1 and we have a live DEK -> cache it so cold launch can skip PIN.
// - seconds >= 0 and biometric is disabled -> evict cache so cold launch prompts PIN again.
// - seconds >= 0 and biometric is enabled -> leave cache alone (biometric owns it).
if seconds == -1 {
    if let Some(dek) = actor_state.dek.as_ref() {
        let dek_hex: String = dek.iter().map(|b| format!("{:02x}", b)).collect();
        actor_state.keychain.store("mango".to_string(), "dek".to_string(), dek_hex);
        log::info!("[auth] SetLockTimeout(Never): DEK cached in keychain for cold-launch bypass");
    } else {
        log::warn!("[auth] SetLockTimeout(Never) with no live DEK — cache not written; next unlock will re-cache");
    }
} else if !actor_state.app_state.biometric_login_enabled {
    actor_state
        .keychain
        .delete("mango".to_string(), "dek".to_string());
    log::info!("[auth] SetLockTimeout(finite) + biometric off: DEK evicted from keychain");
}
```

Also set a toast when selecting Never so the user sees the threat-model note:
```rust
if seconds == -1 {
    actor_state.app_state.toast = Some(
        "Auto-lock disabled. App will open without PIN — protected only by your device unlock.".to_string()
    );
    actor_state.app_state.rev += 1;
}
```

**Change 2: cold-launch screen selection (around L4101-4149)** — extend Case D. Before forcing `Screen::Locked`, check whether the stored `lock_timeout_seconds` is `-1` AND the keychain has a DEK entry. If yes, open the encrypted DB from the keychain DEK (same code path as `BiometricResult` at L7770-7820) and do NOT set `current_screen = Screen::Locked`.

Pseudocode sketch:

```rust
// Case D branch — replace the unconditional Screen::Locked with:
if has_auth {
    // Read lock_timeout_seconds from bootstrap-adjacent storage.
    // NOTE: lock_timeout_seconds lives in the encrypted mango.db today, which we haven't
    // opened yet. We need it BEFORE we decide to open. Two options:
    //   (a) Mirror it into the bootstrap DB as a small non-sensitive hint.
    //   (b) Probe the keychain first: if keychain has "dek" AND there is a saved
    //       "cold_launch_bypass" flag in the bootstrap DB, skip lock.
    // Pick (a): add a `cold_launch_bypass INTEGER DEFAULT 0` column to bootstrap
    // auth_params (set to 1 when user picks Never, 0 otherwise).
    //
    // See Task 2 for the bootstrap schema change.

    let bypass = bootstrap.read_cold_launch_bypass().unwrap_or(false);
    let dek_hex_opt = if bypass {
        actor_state_keychain.load("mango".to_string(), "dek".to_string())
    } else { None };

    if let Some(dek_hex) = dek_hex_opt {
        // Open encrypted DB immediately, mirror the BiometricResult path.
        match persistence::Database::open_encrypted(&db_path, &dek_hex) {
            Ok(db) => {
                initial_state.router.current_screen = Screen::Home; // or Onboarding check
                // populate actor_state.db, actor_state.dek, actor_state.vector_index
                // (factor the BiometricResult post-unlock body into a helper and reuse)
            }
            Err(e) => {
                log::warn!("[auth] Cold-launch bypass failed ({e}); falling back to lock screen");
                initial_state.router.current_screen = Screen::Locked;
            }
        }
    } else {
        initial_state.router.current_screen = Screen::Locked;
    }
}
```

Factor the post-unlock DB + DEK + VectorIndex + post_unlock_load wiring that today lives inside the `BiometricResult` handler (L7770-7850ish) into a helper `fn bypass_unlock_with_cached_dek(actor_state, dek_hex) -> Result<()>` so Change 2 can call it without duplicating ~80 lines. The helper should perform the same steps biometric does: `open_encrypted`, decode hex → `actor_state.dek`, `VectorIndex::new(data_dir, Some(dek_ref))`, then call the existing post-unlock loader (`load_post_unlock` or equivalent — follow whatever BiometricResult does).

Error handling: if `open_encrypted` fails or keychain returns None despite `bypass == true`, log at WARN level and fall through to `Screen::Locked`. Do NOT crash. Do NOT wipe the keychain entry (user may have just rebooted into a weird state).
  </action>
  <verify>
    <automated>cargo check --manifest-path rust/Cargo.toml && cargo test --manifest-path rust/Cargo.toml --lib</automated>
  </verify>
  <done>
- `cargo check` clean.
- Setting `SetLockTimeout { seconds: -1 }` writes keychain entry and sets bootstrap `cold_launch_bypass = 1`.
- Setting `SetLockTimeout { seconds: 300 }` with biometric off clears keychain entry and sets `cold_launch_bypass = 0`.
- Cold launch with `cold_launch_bypass = 1` and valid keychain DEK opens encrypted DB and lands on `Screen::Home` (or `Screen::Onboarding` if onboarding not complete), not `Screen::Locked`.
- Cold launch with `cold_launch_bypass = 1` but missing/corrupt keychain DEK falls back to `Screen::Locked` without crashing.
  </done>
</task>

---

## Task 2 — Rust: bootstrap-DB schema adds `cold_launch_bypass` flag

<task type="auto">
  <name>Task 2: Bootstrap DB — add cold_launch_bypass column with read/write helpers</name>
  <files>rust/src/crypto/bootstrap_db.rs</files>
  <action>
Add a nullable-to-0 column and two helpers.

1. In `BootstrapDb::open`, add a migration:
```rust
conn.execute_batch(
    "ALTER TABLE auth_params ADD COLUMN cold_launch_bypass INTEGER NOT NULL DEFAULT 0;"
)
.ok(); // ignore "duplicate column" on existing DBs
```
(Or: use `PRAGMA table_info` to check for the column before running ALTER — whichever matches the project's existing migration style. Check if there's already a pattern in the repo for idempotent ALTERs; if not, the `.ok()` swallow is acceptable for this single non-critical column.)

2. Add methods:
```rust
pub fn read_cold_launch_bypass(&self) -> Result<bool, anyhow::Error> {
    let val: i32 = self
        .conn
        .query_row(
            "SELECT cold_launch_bypass FROM auth_params WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(val != 0)
}

pub fn write_cold_launch_bypass(&self, on: bool) -> Result<(), anyhow::Error> {
    self.conn.execute(
        "UPDATE auth_params SET cold_launch_bypass = ?1 WHERE id = 1",
        rusqlite::params![if on { 1 } else { 0 }],
    )?;
    Ok(())
}
```

3. Do NOT include `cold_launch_bypass` in the `AuthParams` struct — it's an orthogonal flag, kept out of `write_auth_params` / `read_auth_params` so the PIN-setup path (which calls `write_auth_params`) doesn't accidentally reset it. Expose it only through the two new methods.

4. Task 1 calls `bootstrap.write_cold_launch_bypass(seconds == -1)` from the `SetLockTimeout` handler (in addition to the keychain cache write).

**Security note to preserve in a doc comment on the new column:**
```rust
// `cold_launch_bypass`: non-sensitive hint. Flipping this to 1 without the
// corresponding keychain DEK entry is benign — Task 1's cold-launch code
// falls back to Screen::Locked when the keychain load returns None.
```
  </action>
  <verify>
    <automated>cargo test --manifest-path rust/Cargo.toml -p mango_core crypto::bootstrap_db</automated>
  </verify>
  <done>
- Fresh bootstrap DB has `cold_launch_bypass = 0` after `open`.
- `write_cold_launch_bypass(true)` → `read_cold_launch_bypass()` returns `true`.
- Existing auth_params rows on upgrade gain the column with value 0 (migration idempotent — running `open` twice doesn't error).
- `setup_pin_auth` (which calls `write_auth_params`) does not clobber the flag.
  </done>
</task>

---

## Task 3 — Native UI: warning copy when Never is selected

<task type="auto">
  <name>Task 3: iOS/Android/Desktop — warning row + threat-model copy on "Never"</name>
  <files>ios/Mango/Mango/SettingsSecurityView.swift, android/app/src/main/java/dev/disobey/mango/ui/SettingsSecurityScreen.kt, desktop/iced/src/views/settings.rs</files>
  <action>
When `lockTimeoutSeconds == -1`, show a small caption under the picker with this exact copy (keep it identical across platforms so the threat-model description is consistent):

> "Auto-lock disabled. The app will open without your PIN — it is protected only by your device unlock. If your device is unlocked, anyone with access can open the app."

**iOS (`SettingsSecurityView.swift`)** — below the existing lock-timeout picker block (around L17-18 where `("Never", -1)` is listed; find the `Picker` / `Form` row and append a conditional `Text` or `Label` shown only when the selected value is `-1`). Use `.font(.caption)` and `.foregroundStyle(.secondary)` to match existing secondary copy.

**Android (`SettingsSecurityScreen.kt`)** — below the existing lock-timeout setting (around L215-220). Add an `AnimatedVisibility` block gated on `lockTimeoutSeconds == -1L` containing a `Text` with `MaterialTheme.typography.bodySmall` and `MaterialTheme.colorScheme.onSurfaceVariant`.

**Desktop (`desktop/iced/src/views/settings.rs`)** — below the existing lock-timeout row (around L68-77). Add a `text(...)` widget shown only when the selected value is `-1`, styled with the existing "caption" size if one is defined, else `.size(12)` and `.style(iced::theme::Text::Color(...))` for a muted tone that matches the theme.

Do NOT add a modal confirmation. The lock-timeout picker already debounces on selection change; the inline caption is sufficient friction. The toast from Task 1 (Rust-side) confirms the change separately.

No new UniFFI surface. No state changes beyond reading the existing `lockTimeoutSeconds` value.
  </action>
  <verify>
    <automated>MISSING — no UI test harness exists for these screens. Manual verification: build each platform, navigate to Settings → Security → Lock Timeout, select "Never", confirm the caption appears and the toast fires. Build check: `just build-ios-check || cargo check -p desktop-iced` and Android `./gradlew :app:compileDebugKotlin` (run whichever subset the dev environment supports).</automated>
  </verify>
  <done>
- iOS, Android, Desktop each render the warning caption only when `lockTimeoutSeconds == -1`.
- Caption copy is identical across platforms (single source of truth for the threat-model statement).
- No crashes / layout jumps when switching between finite and Never.
  </done>
</task>

---

## Success criteria

1. User picks "Never" in Settings → Security → Lock Timeout. App shows inline warning + toast. DEK is now in keychain.
2. User force-quits the app and relaunches. App opens straight to `Screen::Home` (or wherever `pre_lock_screen` would have been) — **no PIN prompt**.
3. User changes timeout back to "5 minutes" (or any finite value). Keychain DEK is evicted (unless biometric is also enabled). Next cold launch shows `Screen::Locked` and requires PIN.
4. PIN is never removed from the bootstrap DB. Phase 28 invariant holds: `auth_params` + `wrapped_dek` remain PIN-derived. If the user later toggles back to a finite timeout without remembering their PIN, they are locked out — same as today's biometric-only story.
5. On devices without a secure keychain backend (desktop Linux without Secret Service), the keychain write will fail silently at the `Keychain` trait layer; cold launch then falls back to `Screen::Locked`. This degradation is intentional.

## Out of scope (explicitly)

- Removing PIN entirely (Interpretation B — reverses D-14/D-24, needs a phase).
- Desktop biometric login (D-23 — separate decision).
- Changing Argon2id parameters or KEK derivation.
- Exposing a raw "no encryption" mode.
- Auto-relocking based on OS screen-lock events (different feature).
