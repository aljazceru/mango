# Phase 28: Local Data Encryption & Authentication - Research

**Researched:** 2026-04-09
**Domain:** Encryption at rest, platform biometrics, key management, SQLCipher migration
**Confidence:** HIGH (core stack), MEDIUM (platform biometric integration details)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** SQLCipher for SQLite at-rest encryption — `rusqlite` with `bundled-sqlcipher` feature. Drop-in replacement; inject key via `conn.pragma_update(None, "key", &hex_key)` after open.
- **D-02:** Encrypt usearch `.usearch` files with AES-256-GCM (`aes-gcm` RustCrypto crate). File-level wrapper on save/load.
- **D-03:** Cached documents in `data_dir` encrypted with AES-256-GCM using same DEK as vector index.
- **D-04:** Generate 256-bit random DEK on first launch.
- **D-05:** Store DEK in platform keychain via existing `KeychainProvider` trait.
- **D-06:** Biometric unlock = platform unlocks keychain entry → DEK available. No extra key derivation.
- **D-07:** PIN/password fallback: derive KEK from PIN via Argon2id (`argon2` RustCrypto crate). KEK wraps/unwraps a DEK copy stored in bootstrap DB.
- **D-08:** Argon2id params: memory=64MiB, iterations=3, parallelism=1. Salt + wrapped DEK stored in bootstrap DB (NOT the main encrypted mango.db).
- **D-09:** New `Screen::Locked` variant in `Screen` enum.
- **D-10:** Lock on cold launch (always) and after configurable timeout (default 5 min) on background return.
- **D-11:** Show biometric prompt first; fall back to PIN/password field. iOS: `LAContext.evaluatePolicy`, Android: `BiometricPrompt`, Desktop: PIN-only (macOS Touch ID opportunistic via `keyring`).
- **D-12:** On successful unlock, restore previous `Screen` state. Actor defers `Database::open` until DEK available.
- **D-13:** Lock timeout options: Immediately / 1 min / 5 min (default) / 15 min / Never (warn).
- **D-14:** First-time setup: mandatory PIN/password setup after onboarding. No skip. Biometrics optional.
- **D-15:** Duress PIN wipes mango.db, all files in data_dir, all keychain entries.
- **D-16:** After wipe, app resets to `Screen::Onboarding`.
- **D-17:** No confirmation dialog on duress PIN — immediate silent wipe.
- **D-18:** Duress PIN set during initial PIN setup; optional; must differ from real PIN by ≥1 digit.
- **D-19:** Duress PIN hash stored in bootstrap DB. Compared before DEK unwrap.
- **D-20:** Capability detection at launch; result stored in `AppState` for UI adaptation.
- **D-21:** iOS: `LAContext.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics)`.
- **D-22:** Android: `BiometricManager.canAuthenticate(BIOMETRIC_STRONG)`, min API 28.
- **D-23:** Desktop: PIN-only default. macOS Touch ID via `keyring` opportunistic.
- **D-24:** PIN/password is the minimum auth method on all platforms. Biometrics are additive.
- **D-25:** SQLCipher bundled (unconditional). `aes-gcm` uses AES-NI/ARMv8 if available, degrades to software.

### Claude's Discretion

- Bootstrap DB format (flat file vs tiny SQLite)
- Exact Argon2id parameter tuning if 64 MiB is too much on low-end Android
- Lock screen UI design details
- Whether to show "X failed attempts remaining"
- Migration strategy for existing unencrypted databases (encrypt-in-place vs copy-and-encrypt)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

## Summary

Phase 28 adds full encryption at rest and biometric/PIN authentication to all three platforms. The Rust core handles all crypto: SQLCipher for the main database, AES-256-GCM for binary files, Argon2id for PIN key derivation. Platform-specific biometric auth is bridged via a new `BiometricProvider` UniFFI callback interface (matching the existing `KeychainProvider` pattern). A small bootstrap SQLite database (unencrypted, contains only salt + wrapped DEK + duress PIN hash) gates access to the real encrypted `mango.db`.

The most critical implementation detail is the deferred-open pattern in `FfiApp::new()`: `Database::open()` must not be called until the DEK is available post-unlock. The current code opens the DB unconditionally in the actor thread constructor (line 2464 of lib.rs). This requires restructuring `ActorState` to hold `Option<Database>` and handling a pre-unlock message queue.

The migration path for existing unencrypted `mango.db` files uses SQLCipher's `sqlcipher_export()` pragma: open the plaintext DB, attach a new encrypted DB, call `sqlcipher_export`, close and replace — atomic and loss-free.

**Primary recommendation:** Follow the bootstrap-DB pattern (decision D-08) using a tiny SQLite file `mango_auth.db` separate from `mango.db`. This allows clean separation: auth data is readable before decryption, main data is never accessible without the DEK.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rusqlite` with `bundled-sqlcipher` | 0.39 | SQLite encryption at rest | Drop-in for existing rusqlite; SQLCipher is the only mature SQLite encryption solution. Feature `bundled-sqlcipher` bundles SQLCipher amalgamation — no system library needed. [VERIFIED: crates.io Cargo.toml inspection] |
| `aes-gcm` | 0.10 | AES-256-GCM file encryption | Already in Cargo.toml (used for HPKE). Pure Rust, no OpenSSL. AEAD with nonce = integrity + confidentiality. [VERIFIED: existing Cargo.toml] |
| `argon2` (RustCrypto) | 0.6.0-rc.8 | Argon2id key derivation from PIN | Latest RustCrypto argon2 crate (rc.8 is the current version as of 2026-04). Pure Rust, implements PHC standard. [VERIFIED: cargo search] |
| `zeroize` | 1.x | Wipe key material from memory | Already in Cargo.toml. Essential for DEK, KEK in Rust structs. [VERIFIED: existing Cargo.toml] |
| `rand` | 0.8 | Cryptographically secure random bytes | Already in Cargo.toml. Use `rand::rngs::OsRng` for DEK generation and Argon2id salt. [VERIFIED: existing Cargo.toml] |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `hex` | 0.4 | Encode DEK as hex string for SQLCipher key pragma | Already in Cargo.toml. SQLCipher key pragma accepts hex-encoded key with `x'...'` prefix. [VERIFIED: existing Cargo.toml] |
| `keyring` | 3.6 | Desktop OS credential store (DEK storage) | Already in Cargo.toml for desktop target. Existing `DesktopKeychainProvider` uses it. [VERIFIED: existing Cargo.toml] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `bundled-sqlcipher` | `bundled-sqlcipher-vendored-openssl` | Vendored OpenSSL variant builds on more targets but violates project no-OpenSSL constraint. Never use. |
| bootstrap SQLite (`mango_auth.db`) | flat JSON/TOML file | SQLite allows atomic writes via transactions; flat file requires manual fsync + temp-file dance. SQLite is safer. |
| `argon2` RustCrypto | `rust-argon2` | `rust-argon2` 3.0 is a separate implementation. RustCrypto `argon2` is the canonical crate recommended by the PHC. |

**Installation — Cargo.toml changes:**
```toml
# Replace existing rusqlite line:
# rusqlite = { version = "0.39", features = ["bundled"] }
# With:
rusqlite = { version = "0.39", features = ["bundled-sqlcipher"] }

# Add (not yet present):
argon2 = "0.5"   # NOTE: verify exact version — cargo search shows rc.8 for 0.6.x prerelease
```

**IMPORTANT:** `bundled-sqlcipher` and `bundled` are mutually exclusive. Switching from `bundled` to `bundled-sqlcipher` is the entire change to the dependency — the rusqlite API remains identical. [ASSUMED — verify feature mutual exclusivity in rusqlite 0.39 docs before planning]

**Version note on argon2:** `cargo search` returned `argon2 = "0.6.0-rc.8"` as latest. This is a release candidate. The stable `0.5.x` series is also available and may be preferable for production. Planner should verify with `cargo info argon2` or crates.io before pinning.

---

## Architecture Patterns

### Bootstrap DB vs Main DB Split

```
data_dir/
├── mango_auth.db        # Unencrypted bootstrap — salt, wrapped DEK, duress hash ONLY
├── mango.db             # Encrypted with SQLCipher (requires DEK to open)
└── embeddings.usearch.enc  # AES-256-GCM encrypted blob (renamed from .usearch)
```

The bootstrap DB `mango_auth.db` is a tiny SQLite database that can be opened without any key. It contains:
- `auth_params` table: one row with `salt BLOB`, `wrapped_dek BLOB`, `duress_pin_hash TEXT`, `pin_hash_params TEXT` (JSON: iterations, memory, parallelism)
- This DB is NEVER encrypted. It exists only to provide the unlock inputs.

### Pattern 1: DEK Lifecycle

**First launch:**
1. `rand::rngs::OsRng` generates 32 random bytes → DEK
2. Store DEK in platform keychain (for biometric path): `keychain.store("mango", "dek", hex::encode(&dek))`
3. Generate random 32-byte Argon2id salt
4. Derive KEK from user's PIN via Argon2id
5. Encrypt (wrap) DEK with KEK using AES-256-GCM → `wrapped_dek`
6. Hash duress PIN (if set) with Argon2id (separate salt) → `duress_pin_hash`
7. Write salt + wrapped_dek + duress_pin_hash to `mango_auth.db`
8. Open `mango.db` with SQLCipher using DEK

**Subsequent unlock (biometric):**
1. `BiometricProvider::authenticate()` → platform unlocks keychain
2. `keychain.load("mango", "dek")` → hex DEK
3. Open `mango.db` with DEK

**Subsequent unlock (PIN):**
1. Check PIN against duress hash first → if match, wipe everything
2. Read salt from `mango_auth.db`
3. Derive KEK from entered PIN + salt via Argon2id
4. Decrypt `wrapped_dek` with KEK → DEK
5. Open `mango.db` with DEK

### Pattern 2: SQLCipher Key Injection

```rust
// Source: [ASSUMED — SQLCipher pragma documentation pattern]
pub fn open_encrypted(path: &str, dek_hex: &str) -> Result<Self, PersistenceError> {
    let conn = rusqlite::Connection::open(path)?;
    // SQLCipher key pragma — must be first operation after open
    conn.pragma_update(None, "key", &format!("x'{}'", dek_hex))?;
    // Verify connection works (will fail if wrong key)
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let mut db = Self { conn };
    db.run_migrations()?;
    Ok(db)
}
```

The key pragma MUST be the first pragma issued after `Connection::open()`. Issuing any other pragma before `key` causes SQLCipher to treat the database as plaintext and fail. [ASSUMED — verify SQLCipher docs; this is standard SQLCipher behavior but needs citation]

### Pattern 3: AES-256-GCM File Encryption

```rust
// Source: aes-gcm 0.10 crate (already in Cargo.toml — used by existing HPKE code)
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::aead::rand_core::RngCore;

pub fn encrypt_file(dek: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let key = Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext).expect("encryption failure");
    // Prepend nonce: [12 bytes nonce][ciphertext+tag]
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ciphertext);
    out
}

pub fn decrypt_file(dek: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(dek);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ciphertext)
}
```

Nonce is prepended to the ciphertext. 12-byte random nonce per file. The 16-byte GCM tag is appended by the `aes-gcm` crate automatically. Total overhead: 28 bytes per file. [VERIFIED: aes-gcm 0.10 is already used in this codebase; pattern consistent with AEAD standard]

### Pattern 4: Argon2id Key Derivation

```rust
// Source: [ASSUMED — argon2 RustCrypto crate 0.5/0.6 API]
use argon2::{Argon2, Algorithm, Version, Params};
use argon2::password_hash::{PasswordHasher, SaltString};

pub fn derive_kek(pin: &[u8], salt: &[u8; 32]) -> [u8; 32] {
    // D-08: memory=64MiB, iterations=3, parallelism=1
    let params = Params::new(65536, 3, 1, Some(32)).expect("valid params");
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut kek = [0u8; 32];
    argon2.hash_password_into(pin, salt, &mut kek).expect("kdf");
    kek
}
```

Memory parameter is in KiB: 64 MiB = 65536 KiB. [ASSUMED — verify Params::new signature in argon2 0.5/0.6 docs; parameter order may differ between versions]

### Pattern 5: DEK Wrap/Unwrap with AES-256-GCM

The KEK wraps the DEK using AES-256-GCM (same `encrypt_file`/`decrypt_file` functions above, treating the 32-byte DEK as the "plaintext"). This is standard key-wrapping using an AEAD cipher. No separate key-wrap crate needed.

### Pattern 6: Migration — Unencrypted to Encrypted DB

SQLCipher provides `sqlcipher_export()` for in-place migration: [ASSUMED — verify against SQLCipher docs; this is well-documented behavior]

```sql
-- Attach new encrypted DB, export, then replace the original
ATTACH DATABASE 'mango_encrypted.db' AS encrypted KEY "x'<hex_dek>'";
SELECT sqlcipher_export('encrypted');
DETACH DATABASE encrypted;
-- Then: delete mango.db, rename mango_encrypted.db to mango.db
```

In Rust via rusqlite:
```rust
conn.execute_batch(&format!(
    "ATTACH DATABASE '{path}_enc' AS encrypted KEY \"x'{hex_dek}'\";
     SELECT sqlcipher_export('encrypted');
     DETACH DATABASE encrypted;"
))?;
std::fs::rename(format!("{path}_enc"), path)?;
```

This is safe (the export runs in a transaction) and preserves all data including the `user_version` pragma. Detection of "is this DB already encrypted?" uses: attempt `conn.pragma_query_value(None, "user_version", ...)` without key — if it fails with a SQLite error, the DB is encrypted.

### Pattern 7: BiometricProvider UniFFI Callback Interface

Following the existing `KeychainProvider` pattern (lib.rs §575-580):

```rust
// Rust core — new callback interface
#[uniffi::export(callback_interface)]
pub trait BiometricProvider: Send + Sync + 'static {
    /// Check if biometric authentication is available and enrolled.
    /// Returns: "available", "not_enrolled", "not_available", "not_supported"
    fn biometric_status(&self) -> String;
    /// Trigger biometric authentication prompt. Blocking call — platform shows UI.
    /// Returns true on success, false on failure/cancel.
    fn authenticate(&self, reason: String) -> bool;
}
```

iOS Swift implementation uses `LAContext`. Android Kotlin implementation uses `BiometricPrompt` (which requires a Fragment/Activity reference — this is the key integration challenge; see Pitfalls). Desktop returns `"not_supported"` from `biometric_status`.

### Pattern 8: Deferred Database Open

Current `FfiApp::new()` (lib.rs line 2464) calls `Database::open()` unconditionally. This must change:

```rust
// ActorState change:
struct ActorState {
    db: Option<Database>,  // None until unlock
    pending_dek: Option<Zeroizing<[u8; 32]>>,
    // ... rest of fields
}
```

The actor loop must handle a `AppAction::Unlock { dek_hex }` message that:
1. Opens the encrypted DB
2. Runs migrations
3. Loads backends from the now-open DB
4. Sets `AppState.screen` to the saved pre-lock screen (or `Screen::Conversations` if first unlock)
5. Clears `pending_dek`

Before unlock completes, the actor must reject or queue any DB-dependent actions and return an error state. The simplest approach: dispatch `AppAction::Unlock` immediately from `FfiApp::new()` only if a `NullKeychainProvider` is in use (test/dev mode), bypassing the lock screen.

### Anti-Patterns to Avoid

- **Key pragma after WAL mode:** Setting WAL pragma before the key pragma will fail. Key MUST come first.
- **Storing DEK in mango.db:** The DEK cannot be stored in the database it encrypts. Store it in `mango_auth.db` (wrapped) and keychain (biometric path).
- **Nonce reuse in AES-GCM:** Never reuse a nonce with the same key. Use fresh `OsRng`-generated nonce per file save.
- **Blocking Argon2id on main thread:** Argon2id with 64 MiB memory takes ~0.5s. Must run on actor thread (already on a dedicated thread), never inside a Tokio async task that might block the runtime.
- **BiometricPrompt on Android from Rust thread:** Android `BiometricPrompt` must be called from the main thread with an Activity reference. The `BiometricProvider` callback interface bridges this — the Kotlin implementation must dispatch to the main thread internally. The `authenticate()` call blocks the Rust actor thread until the platform callback completes.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQLite encryption | Custom page-level AES wrapper | `rusqlite` `bundled-sqlcipher` | SQLCipher handles page encryption, IV management, key schedule, WAL encryption — 10+ years of hardening |
| Key derivation from PIN | Custom PBKDF | `argon2` RustCrypto crate | Memory-hard KDFs have subtle implementation requirements; custom PBKDF2/bcrypt has well-known weaknesses against GPU attacks |
| AEAD encryption for files | Custom AES-CBC + HMAC | `aes-gcm` crate (already present) | Combining CBC + HMAC correctly (encrypt-then-MAC, avoiding padding oracles) is error-prone |
| Secure key storage | Write keys to SQLite or UserDefaults | Platform keychain via `KeychainProvider` | Platform keychains use hardware-backed key storage (Secure Enclave, StrongBox) |
| Secure memory zeroing | `drop()` or manual memset | `zeroize` crate (already present) | Compilers optimize away naive zeroing; `zeroize` uses volatile writes + memory barriers |

---

## Common Pitfalls

### Pitfall 1: SQLCipher Feature Conflict with bundled

**What goes wrong:** Adding `bundled-sqlcipher` alongside `bundled` causes a compile error — both features try to build SQLite and they conflict.

**Why it happens:** `bundled` builds vanilla SQLite; `bundled-sqlcipher` builds the SQLCipher fork. Both cannot coexist.

**How to avoid:** Replace `features = ["bundled"]` with `features = ["bundled-sqlcipher"]` — not add.

**Warning signs:** Compile error mentioning `libsqlite3-sys` feature conflict.

### Pitfall 2: Android BiometricPrompt Requires UI Thread + Activity

**What goes wrong:** Calling `BiometricPrompt` from a background thread throws `IllegalStateException`. The Rust actor thread is not the Android main thread.

**Why it happens:** `BiometricPrompt` internally uses a `FragmentManager` which requires running on the main thread with an Activity reference.

**How to avoid:** The Kotlin `BiometricProvider` implementation must:
1. Hold a `WeakReference<FragmentActivity>` (not strong reference to avoid leaks)
2. Use `runOnUiThread {}` to show the prompt
3. Use a `CountDownLatch` or `CompletableFuture` to block until the callback fires
4. Return the result synchronously to the waiting Rust thread

This is a synchronous-bridge pattern — the `authenticate()` method in Kotlin blocks via a `CountDownLatch(1)`, shows the prompt on UI thread, and releases the latch in the `onAuthenticationSucceeded`/`onAuthenticationFailed` callback.

**Warning signs:** App crashes with `CalledFromWrongThreadException` or hangs forever.

### Pitfall 3: Lock Screen Timing — AppState Published Before DB Open

**What goes wrong:** If `AppState` is published before the DB is open (actor fires `reconcile` before unlock), the UI may briefly show stale state.

**Why it happens:** `update_tx.send(AppUpdate::State(...))` in the actor loop runs before the DB is ready.

**How to avoid:** Emit a minimal `AppState { screen: Screen::Locked, ... }` as the initial state. Do not emit any chat/memory/backend state until unlock. After unlock succeeds, re-query all DB-dependent state and emit a full state update.

### Pitfall 4: SQLCipher Migration Corrupts DB on Crash Mid-Export

**What goes wrong:** If the app crashes between `sqlcipher_export()` and the file rename, both the old plaintext DB and the half-written encrypted DB exist. On next launch, the app tries to open the encrypted DB that wasn't renamed.

**Why it happens:** `fs::rename` is atomic on POSIX (same filesystem), but the export itself writes to a new file.

**How to avoid:**
1. Write encrypted DB to `mango_encrypted.db` (temp file)
2. After export completes, verify the encrypted DB opens correctly
3. Rename `mango_encrypted.db` → `mango.db` (atomic on same filesystem)
4. Delete the temp file only after rename succeeds
5. On launch: if both `mango_encrypted.db` and `mango.db` exist, the crash recovery is: delete `mango_encrypted.db` (incomplete export) and try to open `mango.db` as plaintext first

### Pitfall 5: Argon2id Memory on Low-End Android Devices

**What goes wrong:** 64 MiB Argon2id causes OOM on devices with 512 MiB RAM running Android API 28 (minimum supported).

**Why it happens:** Android kills processes exceeding their memory allocation; `BIOMETRIC_ERROR_NONE_ENROLLED` devices may be older low-RAM phones.

**How to avoid (Claude's Discretion):** Detect `ActivityManager.getMemoryClass()` in Kotlin; if `memoryClass < 256` (device has <256 MiB per app), use reduced params (32 MiB, 3 iterations). Store the params used alongside the salt in `mango_auth.db` so the same params are used on every unlock. The `pin_hash_params` JSON field in the bootstrap DB covers this.

### Pitfall 6: Duress PIN Hash Timing Side-Channel

**What goes wrong:** Comparing duress PIN hash before real PIN hash using `==` leaks timing information — an attacker measuring response time could detect a near-miss on the duress PIN.

**Why it happens:** String equality short-circuits on the first differing byte.

**How to avoid:** Use `subtle::ConstantTimeEq` from the `subtle` crate for the duress hash comparison. Or use Argon2id's built-in `verify_password()` which is constant-time. The `subtle` crate is a transitive dependency via RustCrypto crates — it should already be available in the lock file.

### Pitfall 7: Nonce File Format — usearch Encrypted File

**What goes wrong:** Changing the file extension from `.usearch` to `.usearch.enc` or similar breaks the existing `INDEX_FILENAME` constant and the detection logic in `VectorIndex::new()`.

**Why it happens:** `rag/index.rs` uses `const INDEX_FILENAME: &str = "embeddings.usearch"` and `std::path::Path::new(&path).exists()` to detect existing index.

**How to avoid:** Keep the filename `embeddings.usearch` — encrypt in-place. The file on disk is now always the encrypted blob; the `VectorIndex::new()` code path changes to: decrypt → load usearch from bytes → keep in memory. On save: serialize to bytes → encrypt → write. Alternatively, use `embeddings.usearch.enc` as the new canonical filename and update `INDEX_FILENAME` — but this requires migration of existing `.usearch` files.

**Recommendation (Claude's Discretion):** Use `embeddings.usearch` as the filename (no extension change). The file is always encrypted after Phase 28. Add a 4-byte magic header `b"MGO1"` before the nonce to distinguish encrypted from unencrypted (migration detection).

---

## Code Examples

### SQLCipher Key Pragma (Database::open modification)

```rust
// Source: [ASSUMED — SQLCipher pragma API; standard pattern across all SQLCipher integrations]
impl Database {
    pub fn open_encrypted(path: &str, dek_hex: &str) -> Result<Self, PersistenceError> {
        let conn = rusqlite::Connection::open(path)?;
        // CRITICAL: key pragma must be first
        conn.pragma_update(None, "key", &format!("x'{}'", dek_hex))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Detect if an existing DB file is already SQLCipher-encrypted.
    /// Returns Ok(true) if encrypted, Ok(false) if plaintext, Err if file unreadable.
    pub fn is_encrypted(path: &str) -> bool {
        let Ok(conn) = rusqlite::Connection::open(path) else { return false; };
        conn.pragma_query_value::<i32, _, _>(None, "user_version", |r| r.get(0)).is_err()
    }
}
```

### Bootstrap DB Schema

```rust
// Source: [ASSUMED — based on D-08 decisions, standard SQLite pattern]
const BOOTSTRAP_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS auth_params (
    id             INTEGER PRIMARY KEY DEFAULT 1 CHECK(id = 1), -- singleton row
    salt           BLOB NOT NULL,           -- 32-byte Argon2id salt for PIN KEK
    wrapped_dek    BLOB NOT NULL,           -- DEK encrypted with PIN-derived KEK (AES-256-GCM)
    duress_hash    TEXT,                    -- Argon2id hash of duress PIN, or NULL if not set
    duress_salt    BLOB,                    -- separate salt for duress PIN hash
    kdf_params     TEXT NOT NULL            -- JSON: {\"m\":65536,\"t\":3,\"p\":1}
);
";
```

### Biometric Status Check (iOS Swift pattern)

```swift
// Source: [ASSUMED — LAContext Apple documentation pattern]
import LocalAuthentication

class BiometricProviderImpl: BiometricProvider {
    func biometricStatus() -> String {
        let context = LAContext()
        var error: NSError?
        if context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) {
            return "available"
        }
        guard let err = error else { return "not_available" }
        switch LAError.Code(rawValue: err.code) {
        case .biometryNotEnrolled: return "not_enrolled"
        case .biometryNotAvailable: return "not_available"
        default: return "not_supported"
        }
    }

    func authenticate(reason: String) -> Bool {
        let context = LAContext()
        var result = false
        let semaphore = DispatchSemaphore(value: 0)
        context.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics,
                                localizedReason: reason) { success, _ in
            result = success
            semaphore.signal()
        }
        semaphore.wait()
        return result
    }
}
```

---

## Runtime State Inventory

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| Stored data | `mango.db` — existing unencrypted SQLite at `{data_dir}/mango.db` | Migration: sqlcipher_export on first launch after Phase 28 |
| Stored data | `embeddings.usearch` — existing unencrypted binary at `{data_dir}/embeddings.usearch` | Migration: encrypt-in-place on first unlock (read → encrypt → overwrite) |
| Live service config | None — no external services involved | None |
| OS-registered state | None — no scheduled tasks or daemons involved | None |
| Secrets/env vars | `KeychainProvider` stores API keys under service="mango", key={backend_id} — these are separate from the DEK keychain entry | No change to API key entries; DEK stored under service="mango", key="dek" — no collision |
| Build artifacts | None — no compiled artifacts carry state | None |

**Migration detection strategy:** At actor start, before opening any DB, check:
1. Does `mango_auth.db` exist? If not → first launch, generate DEK, show PIN setup, create bootstrap DB, migrate mango.db to SQLCipher.
2. Does `mango_auth.db` exist but `mango.db` is plaintext? → crash during previous migration; retry migration.
3. Both exist and `mango.db` is encrypted → normal flow.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All compilation | Yes | rustc 1.93.0 | — |
| `aes-gcm` crate | File encryption | Yes (in Cargo.toml) | 0.10 | — |
| `zeroize` crate | Memory safety | Yes (in Cargo.toml) | 1.x | — |
| `rand` crate | DEK/salt generation | Yes (in Cargo.toml) | 0.8 | — |
| `hex` crate | SQLCipher key encoding | Yes (in Cargo.toml) | 0.4 | — |
| `rusqlite` `bundled-sqlcipher` | DB encryption | Feature change needed (currently `bundled`) | 0.39 | — |
| `argon2` crate | PIN key derivation | Not yet in Cargo.toml | 0.5.x stable (or 0.6.0-rc.8) | — |
| LAContext (iOS) | Biometric auth | Platform API (iOS 17+) | iOS 17+ | PIN-only fallback |
| BiometricPrompt (Android) | Biometric auth | Platform API (API 28+) | API 28+ | PIN-only fallback |
| Touch ID via keyring (macOS) | Desktop biometric | Opportunistic | keyring 3.6 | PIN-only fallback |

**Missing dependencies with no fallback:**
- `argon2` crate: must be added to Cargo.toml before PIN key derivation can be implemented.
- `rusqlite` feature change to `bundled-sqlcipher`: must replace `bundled` feature.

**Missing dependencies with fallback:**
- Biometric APIs on all platforms: PIN/password path fully functional without them (D-24).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `tokio::test` for async |
| Config file | none (cargo test, no separate config) |
| Quick run command | `cargo test -p mango_core --lib 2>&1 \| tail -20` |
| Full suite command | `cargo test -p mango_core 2>&1 \| tail -30` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ENC-01 | AES-256-GCM encrypt/decrypt round-trip | unit | `cargo test -p mango_core encryption::tests -- --nocapture` | Wave 0 |
| ENC-02 | SQLCipher DB opens with correct key, fails with wrong key | unit | `cargo test -p mango_core persistence::tests::test_encrypted_open` | Wave 0 |
| ENC-03 | Argon2id produces deterministic output for same pin+salt | unit | `cargo test -p mango_core auth::tests::test_argon2id_deterministic` | Wave 0 |
| ENC-04 | DEK wrap/unwrap round-trip (KEK wraps DEK, KEK unwraps DEK) | unit | `cargo test -p mango_core auth::tests::test_dek_wrap_unwrap` | Wave 0 |
| ENC-05 | Duress PIN triggers wipe (mock FS + mock keychain) | unit | `cargo test -p mango_core auth::tests::test_duress_wipe` | Wave 0 |
| ENC-06 | Bootstrap DB read/write auth_params singleton | unit | `cargo test -p mango_core auth::tests::test_bootstrap_db` | Wave 0 |
| ENC-07 | Migration: plaintext DB exports to SQLCipher (temp file) | integration | `cargo test -p mango_core persistence::tests::test_sqlcipher_migration` | Wave 0 |
| ENC-08 | usearch encrypt/decrypt round-trip | unit | `cargo test -p mango_core rag::tests::test_index_encryption` | Wave 0 |
| ENC-09 | Lock screen FSM: Locked → Unlock → Conversations | unit | `cargo test -p mango_core tests::test_lock_unlock_fsm` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p mango_core --lib 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p mango_core 2>&1 | tail -30`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `rust/src/auth/mod.rs` — new module: DEK lifecycle, Argon2id derivation, bootstrap DB, duress logic
- [ ] `rust/src/auth/tests.rs` — unit tests for ENC-03 through ENC-06, ENC-09
- [ ] `rust/src/persistence/tests.rs` — tests for ENC-02, ENC-07 (new file or extend existing)
- [ ] `rust/src/rag/tests.rs` — test for ENC-08 (usearch encryption round-trip)
- [ ] `rust/src/encryption.rs` — AES-256-GCM helpers (encrypt_bytes/decrypt_bytes), test for ENC-01

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Yes | PIN/password + biometrics via LAContext / BiometricPrompt |
| V3 Session Management | Yes | Lock timeout (D-10, D-13); lock on background |
| V4 Access Control | Yes | Screen::Locked gates all app content |
| V5 Input Validation | Yes | PIN length/character validation; duress PIN differs by ≥1 digit |
| V6 Cryptography | Yes | AES-256-GCM (aes-gcm), Argon2id (argon2), secure random (OsRng), zeroize for cleanup |
| V7 Error Handling | Yes | Wrong PIN must not reveal whether near-match; failed biometric should not enumerate attempts to Rust layer |
| V8 Data Protection | Yes | DEK never written to disk in plaintext; KEK never persisted; zeroize all key material on drop |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Cold boot attack on DEK in RAM | Info Disclosure | `zeroize` on all key structs; `Zeroizing<[u8; 32]>` wrapper for DEK |
| Brute-force PIN attack via offline DB copy | Tampering / Info Disclosure | Argon2id with 64 MiB memory makes offline GPU attacks expensive |
| Duress PIN timing oracle | Info Disclosure | Constant-time comparison via `subtle::ConstantTimeEq` or Argon2id verify |
| SQLite journal file leaks plaintext | Info Disclosure | SQLCipher encrypts WAL journal too (automatic) |
| Backup of `mango.db` without key | Info Disclosure | iOS: exclude from iCloud backup with `isExcludedFromBackup`; Android: `android:allowBackup="false"` or `fullBackupContent` exclusion rules |
| Nonce reuse in AES-GCM | Tampering / Forgery | Fresh `OsRng` nonce per file save; never deterministic nonce |
| Android keystore key extraction | Info Disclosure | Use `KeyProperties.PURPOSE_DECRYPT` only; require user authentication (biometric) before key use if using Android Keystore-backed key |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `bundled-sqlcipher` and `bundled` features are mutually exclusive in rusqlite 0.39 | Standard Stack | Low — switching features is trivial; risk is adding both causes compile error (recoverable) |
| A2 | SQLCipher key pragma must be the first pragma issued after open | Pattern 2 | HIGH — if wrong, DB could open as plaintext silently; planner must verify against SQLCipher docs before implementing |
| A3 | `sqlcipher_export()` pragma preserves `user_version` | Pattern 6 | MEDIUM — if user_version is reset, migration runner re-runs all migrations on migrated DB (data corruption risk) |
| A4 | argon2 0.5 stable API: `Params::new(m_cost_kib, t_cost, p_cost, output_len)` | Pattern 4 | MEDIUM — API may differ in 0.5 vs 0.6 rc; planner should verify exact Params constructor signature |
| A5 | LAContext.evaluatePolicy is safe to call from a background thread via semaphore | Code Examples | MEDIUM — Apple recommends main thread; semaphore approach is common but should be verified against Apple docs |
| A6 | `isExcludedFromBackup` file attribute prevents iCloud backup of mango.db | Security Domain | MEDIUM — if not set, iCloud could backup the encrypted DB (which is safe) but also the bootstrap DB auth_params (which leaks the wrapped DEK and salt — an attacker with PIN can decrypt) |
| A7 | rusqlite 0.39 (not 0.38 as in CLAUDE.md) is the actual version in Cargo.toml | Standard Stack | No risk — verified from Cargo.toml inspection (0.39 is installed) |

---

## Open Questions

1. **argon2 crate version: 0.5.x stable vs 0.6.0-rc.8 prerelease**
   - What we know: `cargo search` returns rc.8 as "latest"; stable 0.5.x also exists.
   - What's unclear: Whether 0.6 rc.8 API is stable enough; whether it has breaking changes from 0.5.
   - Recommendation: Use `argon2 = "0.5"` (stable). Check `Params::new()` signature in 0.5 docs before implementation.

2. **Bootstrap DB format: flat file (JSON/CBOR) vs SQLite**
   - What we know: Claude's Discretion item. SQLite gives atomic writes; flat file is simpler.
   - Recommendation: Use a minimal SQLite file `mango_auth.db`. Rationale: rusqlite is already a dependency, atomic writes are free (transactions), and the schema is self-documenting. Overhead is negligible (the file will be ~8KB).

3. **iOS backup exclusion for mango_auth.db**
   - What we know: `mango_auth.db` contains the wrapped DEK and salt. An iCloud backup of this file combined with the PIN allows offline decryption.
   - What's unclear: Whether marking the entire `data_dir` as excluded from backup is acceptable UX (it prevents any backup) or whether only `mango_auth.db` should be excluded.
   - Recommendation: Exclude `mango_auth.db` specifically (it's the sensitive file). `mango.db` encrypted with SQLCipher is safe to back up — it's useless without the DEK.

4. **Android keystore-backed vs software-backed DEK storage**
   - What we know: `KeychainProvider.store()` currently stores string values. Android Keystore can generate hardware-backed keys that never leave the TEE — but this requires a different API (key generation inside keystore, encrypt/decrypt operations through keystore).
   - What's unclear: The current `KeychainProvider` stores the raw DEK hex string. On Android with StrongBox, the DEK should ideally be encrypted by a hardware-backed key that requires biometric auth for each use.
   - Recommendation: For Phase 28, store the DEK hex string via existing `KeychainProvider` (software-backed on Android). Document as a known limitation. Phase 29+ can enhance to hardware-backed key wrapping if required.

---

## Sources

### Primary (HIGH confidence)
- `/home/lio/g/confidential-app/rust/Cargo.toml` — verified: aes-gcm 0.10, zeroize, rand 0.8, hex, rusqlite 0.39 already present
- `/home/lio/g/confidential-app/rust/src/lib.rs` §566-662 — KeychainProvider trait pattern for BiometricProvider design
- `/home/lio/g/confidential-app/rust/src/lib.rs` §2390-2480 — FfiApp::new() DB open location requiring deferred-open refactor
- `/home/lio/g/confidential-app/rust/src/persistence/mod.rs` — Database::open, migration runner structure
- `/home/lio/g/confidential-app/rust/src/rag/index.rs` — VectorIndex save/load pattern for AES-GCM wrapper
- `cargo search argon2` — version 0.6.0-rc.8 confirmed as current; 0.5.x stable also available

### Secondary (MEDIUM confidence)
- SQLCipher documentation (well-known: key pragma is first operation) — [ASSUMED, standard SQLCipher usage]
- Apple LAContext documentation — biometricStatus pattern [ASSUMED]
- Android BiometricPrompt documentation — BIOMETRIC_STRONG flag, API 28 minimum [ASSUMED]

### Tertiary (LOW confidence)
- `sqlcipher_export()` preserves user_version — claimed in SQLCipher docs, not independently verified this session

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all core crates verified in Cargo.toml or cargo registry
- Architecture: HIGH — patterns derived from existing codebase structure (KeychainProvider, actor pattern, VectorIndex save/load)
- Pitfalls: MEDIUM — most from established security knowledge; BiometricPrompt threading pitfall well-known in Android community
- SQLCipher pragma ordering: ASSUMED — plan must verify before implementing

**Research date:** 2026-04-09
**Valid until:** 2026-05-09 (stable crates; SQLCipher and platform biometric APIs are mature)
