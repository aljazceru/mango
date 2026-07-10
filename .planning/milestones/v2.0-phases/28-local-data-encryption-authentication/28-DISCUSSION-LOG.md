# Phase 28: Local Data Encryption & Authentication - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 28-local-data-encryption-authentication
**Areas discussed:** Encryption strategy, Key management & derivation, Authentication flow & lock behavior, Duress PIN behavior, Platform capability degradation
**Mode:** --auto (all areas auto-selected, recommended defaults chosen)

---

## Encryption strategy

| Option | Description | Selected |
|--------|-------------|----------|
| SQLCipher + AES-256-GCM | SQLCipher for SQLite, AES-256-GCM for vector index/doc files | ✓ |
| Application-level field encryption | Encrypt individual fields in plaintext SQLite | |
| Full-disk encryption only | Rely on OS-level FDE, no app-level encryption | |

**User's choice:** [auto] SQLCipher + AES-256-GCM (recommended default)
**Notes:** SQLCipher is drop-in for rusqlite via bundled-sqlcipher feature. AES-256-GCM from aes-gcm crate is pure Rust, consistent with project's no-OpenSSL constraint. Field-level encryption rejected as too complex and doesn't protect metadata. OS-level FDE rejected as insufficient — user explicitly wants app-level encryption.

---

## Key management & derivation

| Option | Description | Selected |
|--------|-------------|----------|
| Platform keychain DEK + Argon2id KEK | Random DEK in keychain, PIN-derived KEK as fallback | ✓ |
| PIN-only derivation | Always derive key from PIN, no keychain storage | |
| Hardware-bound key only | DEK locked to Secure Enclave/StrongBox, no PIN fallback | |

**User's choice:** [auto] Platform keychain DEK + Argon2id KEK (recommended default)
**Notes:** Balances security (hardware-backed storage) with usability (biometric unlock) and degradation (PIN fallback). PIN-only derivation rejected because it would require re-deriving on every unlock (slow). Hardware-only rejected because it breaks desktop and older devices.

---

## Authentication flow & lock behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Lock on launch + background timeout | Always lock on cold start, configurable timeout on background | ✓ |
| Lock on launch only | Only lock when app starts fresh | |
| Always lock on background | Lock every time app goes to background | |

**User's choice:** [auto] Lock on launch + background timeout (recommended default)
**Notes:** Default 5-minute timeout balances security and convenience. "Always lock on background" would be too aggressive for normal use. "Launch only" insufficient for security-conscious users.

---

## Duress PIN behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Full wipe + reset to onboarding | Delete DB, index, keychain, show fresh install state | ✓ |
| Partial wipe (DB only) | Delete database but preserve app config | |
| Show fake empty state | Don't delete, just hide data behind fake empty screen | |

**User's choice:** [auto] Full wipe + reset to onboarding (recommended default)
**Notes:** User explicitly wants to "delete all the local data to prevent any leakage." Partial wipe leaves forensic traces. Fake empty state is fragile and data is still recoverable. Full wipe with no confirmation is the correct approach for plausible deniability.

---

## Platform capability degradation

| Option | Description | Selected |
|--------|-------------|----------|
| Runtime detection + fallback chain | Detect biometrics at launch, fall back to PIN/password | ✓ |
| Require biometrics or fail | Only allow biometric-capable devices | |
| PIN/password only everywhere | Skip biometrics entirely for simplicity | |

**User's choice:** [auto] Runtime detection + fallback chain (recommended default)
**Notes:** User explicitly wants "biometrics if available" with graceful degradation. Requiring biometrics breaks older phones and desktop. Skipping biometrics wastes hardware capability the user explicitly wants to use.

---

## Claude's Discretion

- Bootstrap DB format
- Argon2id parameter tuning
- Lock screen UI design
- Failed attempt handling
- Migration strategy for existing unencrypted DBs

## Deferred Ideas

None — discussion stayed within phase scope
