---
phase: 28-local-data-encryption-authentication
plan: 01
subsystem: crypto
tags: [sqlcipher, aes-gcm, argon2id, key-derivation, encryption, sqlite, rust]

requires:
  - phase: 04-persistence-layer
    provides: Database struct with rusqlite, migration runner, PersistenceError

provides:
  - SQLCipher as the bundled SQLite engine (bundled-sqlcipher feature)
  - Database::open_encrypted, is_encrypted, migrate_to_encrypted methods
  - rust/src/crypto/ module with file_crypto, key_derivation, bootstrap_db submodules
  - AES-256-GCM file encryption with MGO1 magic header
  - Argon2id KEK derivation from PIN + salt
  - DEK generation, wrapping (AES-256-GCM), and unwrapping
  - PHC-format PIN hashing and constant-time verification
  - BootstrapDb singleton for persisting auth params (salt, wrapped DEK, KDF params)

affects:
  - 28-02-unlock-flow
  - 28-03-platform-keystore
  - 28-04-vector-index-encryption
  - 28-05-document-encryption
  - 28-06-biometric-unlock
  - 28-07-duress-pin

tech-stack:
  added:
    - rusqlite bundled-sqlcipher (replaces bundled; SQLCipher 4.x as bundled SQLite engine)
    - argon2 0.5 (Argon2id KDF with PHC password hashing)
    - subtle 2 (constant-time comparison utility)
  patterns:
    - SQLCipher key pragma issued as first operation after Connection::open (D-01)
    - sqlcipher_export + user_version transfer for plaintext-to-encrypted migration
    - AES-256-GCM with new_from_slice + Nonce::from (avoids deprecated from_slice)
    - Zeroizing<[u8; 32]> for all intermediate key material (T-28-02)
    - PHC-format Argon2id for PIN hashing (argon2 crate embeds salt in hash string)
    - DEK wrap format: [12-byte nonce][ciphertext+16-byte tag] (no MGO1 — key blob only)
    - File encrypt format: [MGO1][12-byte nonce][ciphertext+16-byte tag]

key-files:
  created:
    - rust/src/crypto/mod.rs
    - rust/src/crypto/file_crypto.rs
    - rust/src/crypto/key_derivation.rs
    - rust/src/crypto/bootstrap_db.rs
    - rust/src/tests/crypto.rs
    - rust/src/tests/persistence_encrypted.rs
  modified:
    - rust/Cargo.toml (bundled-sqlcipher, argon2, subtle)
    - rust/src/persistence/mod.rs (open_encrypted, is_encrypted, migrate_to_encrypted)
    - rust/src/persistence/error.rs (DecryptionFailed, IoError variants)
    - rust/src/lib.rs (pub mod crypto)
    - rust/src/tests/mod.rs (mod crypto, mod persistence_encrypted)

key-decisions:
  - "sqlcipher_export does not transfer PRAGMA user_version — must be explicitly set on encrypted copy before calling open_encrypted to avoid re-running all migrations"
  - "Use new_from_slice + Nonce::from instead of from_slice to avoid deprecated hpke::generic_array re-export warnings"
  - "hash_pin generates its own random salt internally (PHC format embeds salt); the _salt param is kept for API compatibility but unused"
  - "DEK wrap uses raw [nonce][ciphertext+tag] format without MGO1 header — MGO1 is file-on-disk only"
  - "Bootstrap DB is unencrypted SQLite; its security derives from OS file permissions + the wrapped DEK requiring PIN to decrypt"

patterns-established:
  - "Crypto primitives live in rust/src/crypto/ — no crypto logic in persistence, lib, or other modules"
  - "All 32-byte key material uses Zeroizing<[u8; 32]> for T-28-02 compliance"
  - "SQLCipher key pragma is ALWAYS the first operation after Connection::open — enforced in open_encrypted"

requirements-completed:
  - ENC-01
  - ENC-02
  - ENC-03
  - ENC-04
  - ENC-05
  - ENC-06

duration: 11min
completed: 2026-04-09
---

# Phase 28 Plan 01: Rust Crypto Foundation Summary

**SQLCipher-backed Database, AES-256-GCM file encryption with MGO1 header, Argon2id DEK/KEK derivation, and BootstrapDb singleton — full crypto foundation for local data encryption**

## Performance

- **Duration:** 11 min
- **Started:** 2026-04-09T05:36:42Z
- **Completed:** 2026-04-09T05:47:22Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments

- SQLCipher replaces vanilla SQLite as the bundled engine; Database has `open_encrypted`, `is_encrypted`, and `migrate_to_encrypted` methods with correct key pragma ordering and `user_version` transfer on migration
- Complete `rust/src/crypto/` module: AES-256-GCM file encryption (MGO1 header), Argon2id key derivation (default 64 MiB / 3 iterations), DEK wrap/unwrap, PHC-format PIN hashing with constant-time verification
- `BootstrapDb` stores singleton auth params row (salt, wrapped DEK, duress hash, KDF params) in a separate unencrypted SQLite file
- 20 new unit tests (5 persistence encrypted + 15 crypto) covering round-trips, wrong-key errors, magic header checks, determinism, and delete-all

## Task Commits

1. **Task 1: SQLCipher + Database encrypted methods** - `66e6fdc` (feat)
2. **Task 2: Crypto module — file crypto, key derivation, bootstrap DB** - `4531f47` (feat)

## Files Created/Modified

- `rust/Cargo.toml` — Switch to `bundled-sqlcipher`, add `argon2 = "0.5"`, `subtle = "2"`
- `rust/src/persistence/mod.rs` — `open_encrypted`, `is_encrypted`, `migrate_to_encrypted`
- `rust/src/persistence/error.rs` — `DecryptionFailed`, `IoError` variants + Display + From<io::Error>
- `rust/src/crypto/mod.rs` — Module root re-exporting submodules
- `rust/src/crypto/file_crypto.rs` — `encrypt_file`, `decrypt_file` with MGO1 header
- `rust/src/crypto/key_derivation.rs` — `generate_dek`, `generate_salt`, `derive_kek`, `wrap_dek`, `unwrap_dek`, `hash_pin`, `verify_pin_hash`, DEFAULT_* constants
- `rust/src/crypto/bootstrap_db.rs` — `BootstrapDb`, `AuthParams` structs
- `rust/src/lib.rs` — `pub mod crypto;`
- `rust/src/tests/crypto.rs` — 15 crypto unit tests
- `rust/src/tests/persistence_encrypted.rs` — 5 SQLCipher persistence tests

## Decisions Made

- `sqlcipher_export` does not copy `PRAGMA user_version` — must be explicitly read from source and written to encrypted copy before verifying with `open_encrypted` to avoid re-running all migrations on the exported DB.
- Used `Aes256Gcm::new_from_slice` + `Nonce::from` instead of the deprecated `Key::from_slice` / `Nonce::from_slice` (the `hpke` crate re-exports an old `generic_array` that marks these deprecated; consistent with existing codebase patterns in `llm/ppq_private.rs`).
- `hash_pin` uses argon2's PHC `SaltString::generate` internally; the `_salt` parameter exists for API symmetry with `derive_kek` but is unused (PHC embeds its own salt).
- DEK wrap/unwrap uses raw `[12-byte nonce][ciphertext+tag]` format without MGO1 — MGO1 is file-on-disk identification only, not used for in-memory key blobs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed pragma_query_value type parameter count**
- **Found during:** Task 1 (SQLCipher integration)
- **Issue:** Plan code used `pragma_query_value::<i32, _, _>` (3 type params) but rusqlite 0.39 only takes 2 generic parameters on this method
- **Fix:** Changed to `pragma_query_value::<i32, _>` at two call sites
- **Files modified:** `rust/src/persistence/mod.rs`
- **Verification:** Compiled and all 5 persistence encrypted tests passed
- **Committed in:** `66e6fdc`

**2. [Rule 1 - Bug] Fixed sqlcipher_export not preserving user_version**
- **Found during:** Task 1 (`test_migrate_to_encrypted_converts_plaintext_db`)
- **Issue:** `sqlcipher_export` copies schema + data but NOT `PRAGMA user_version`. The exported DB had `user_version=0`, causing `open_encrypted` → `run_migrations` to re-apply all migrations, which failed with "duplicate column name" on migration V6
- **Fix:** Added explicit read of source `user_version` before export, then write it to the encrypted copy via a keyed connection before verification
- **Files modified:** `rust/src/persistence/mod.rs`
- **Verification:** `test_migrate_to_encrypted_converts_plaintext_db` passed
- **Committed in:** `66e6fdc`

**3. [Rule 1 - Bug] Fixed deprecated from_slice warnings in crypto files**
- **Found during:** Task 2 (crypto module)
- **Issue:** Initial implementation used `Key::<Aes256Gcm>::from_slice` and `Nonce::from_slice` which are deprecated in `hpke`'s re-exported `generic_array` version
- **Fix:** Replaced with `Aes256Gcm::new_from_slice` and `Nonce::from(nonce_bytes)` matching the existing codebase pattern in `llm/ppq_private.rs`
- **Files modified:** `rust/src/crypto/file_crypto.rs`, `rust/src/crypto/key_derivation.rs`
- **Verification:** No warnings from crypto files after fix; all 15 crypto tests pass
- **Committed in:** `4531f47`

---

**Total deviations:** 3 auto-fixed (all Rule 1 - Bug)
**Impact on plan:** All fixes were correctness/compatibility issues. No scope creep.

## Issues Encountered

None beyond the auto-fixed bugs documented above.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- All crypto primitives are in place for Plan 28-02 (unlock flow): `derive_kek`, `unwrap_dek`, `open_encrypted`, `BootstrapDb::read_auth_params` are ready to wire into the actor's `UnlockApp` action
- `BootstrapDb::has_auth_params` enables first-launch detection for setup flow
- `migrate_to_encrypted` is ready for Plan 28-03 (platform keystore integration) to trigger post-initial-setup
- No blockers

---
## Self-Check: PASSED

- All 6 crypto/test files exist in worktree
- Task commits 66e6fdc and 4531f47 both verified in git log
- 254 tests pass (cargo test -p mango_core), 0 failures

---
*Phase: 28-local-data-encryption-authentication*
*Completed: 2026-04-09*
