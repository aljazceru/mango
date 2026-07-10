# Phase 28: Local Data Encryption & Authentication - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

Encrypt all locally stored data (SQLite database, usearch vector index files, any cached documents) at rest using platform hardware capabilities. Provide user authentication via biometrics (where available) or PIN/password to unlock the encrypted data. Include a duress PIN that performs full data wipe. Graceful degradation when hardware capabilities (Secure Enclave, StrongBox, biometrics) are unavailable. All three platforms: iOS, Android, Desktop.

</domain>

<decisions>
## Implementation Decisions

### Encryption strategy
- **D-01:** Use SQLCipher for SQLite at-rest encryption — `rusqlite` with `bundled-sqlcipher` feature is a drop-in replacement, same API, transparent encryption/decryption. Replace `rusqlite::Connection::open(path)` with `conn.pragma_update(None, "key", &hex_key)` after open.
- **D-02:** Encrypt usearch vector index files (`.usearch`) with AES-256-GCM file-level wrapper — usearch has no native encryption, so encrypt/decrypt the serialized index file on save/load. Use the `aes-gcm` RustCrypto crate (pure Rust, no OpenSSL dependency — consistent with project constraints).
- **D-03:** Any cached document files in `data_dir` also encrypted with AES-256-GCM using the same DEK. File-level encryption, not filesystem-level.

### Key management & derivation
- **D-04:** Generate a 256-bit random Data Encryption Key (DEK) on first app launch. This DEK encrypts the SQLite DB and all index/document files.
- **D-05:** Store the DEK in the platform keychain via the existing `KeychainProvider` trait (Keychain Services on iOS, Android Keystore on Android, `keyring` crate on Desktop). The DEK is protected at rest by the platform's hardware-backed key storage (Secure Enclave, StrongBox/TEE, OS credential store).
- **D-06:** Biometric unlock = platform unlocks keychain entry → DEK available → open encrypted DB. No additional key derivation needed — the platform handles biometric-to-keychain gating.
- **D-07:** PIN/password fallback: derive a Key Encryption Key (KEK) from the user's PIN/password via Argon2id (using `argon2` RustCrypto crate). The KEK wraps/unwraps a copy of the DEK stored separately from the biometric-gated keychain entry. This is the fallback when biometrics are unavailable or not enrolled.
- **D-08:** Argon2id parameters: memory = 64 MiB, iterations = 3, parallelism = 1 (safe for mobile devices, ~0.5s on modern phones). Store salt alongside the wrapped DEK in a `local_auth` table in a small unencrypted bootstrap SQLite DB (or flat file) that only holds the salt and wrapped DEK — never plaintext secrets.

### Authentication flow & lock behavior
- **D-09:** New `Screen::Locked` variant added to the existing `Screen` enum. This is the gate screen before any app content is accessible.
- **D-10:** App locks on cold launch (always) and on return from background after configurable timeout (default: 5 minutes). Timeout stored in settings table.
- **D-11:** Unlock screen shows biometric prompt first (if available and enrolled), then falls back to PIN/password input field. On iOS: `LAContext.evaluatePolicy`, on Android: `BiometricPrompt`, on Desktop: PIN/password only (with optional macOS Touch ID via keyring's biometric support).
- **D-12:** On successful unlock, restore the previous `Screen` state (preserve navigation stack). The actor thread does not start DB operations until unlock completes — `Database::open` is deferred until DEK is available.
- **D-13:** Lock timeout configurable in settings: "Immediately", "1 minute", "5 minutes" (default), "15 minutes", "Never" (disable lock — not recommended, show warning).
- **D-14:** First-time setup flow: after onboarding wizard completes, prompt user to set a PIN/password and optionally enable biometrics. This is mandatory — no "skip" option. Encryption is always on.

### Duress PIN behavior
- **D-15:** Duress PIN triggers full data wipe: delete `mango.db`, delete all files in `data_dir` (usearch indices, cached documents), delete all keychain entries via `KeychainProvider::delete` for all known service/key combinations.
- **D-16:** After wipe, app resets to the onboarding wizard (`Screen::Onboarding` first step). From an observer's perspective, it looks like a fresh install.
- **D-17:** No confirmation dialog on duress PIN entry — immediate wipe. The purpose is plausible deniability under coercion.
- **D-18:** Duress PIN is set during initial PIN setup. Prompt: "Optionally set an emergency PIN that will erase all data." User can skip (no duress PIN) or set one. Duress PIN must differ from the real PIN by at least 1 digit.
- **D-19:** Duress PIN hash stored in the bootstrap DB alongside the salt and wrapped DEK. Comparison happens before attempting DEK unwrap — if duress PIN matches, wipe immediately without attempting decryption.

### Platform capability degradation
- **D-20:** Capability detection at app launch: query platform biometric availability and store result in `AppState` for UI adaptation.
- **D-21:** iOS: `LAContext.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics)` — supports Face ID and Touch ID. Fallback to PIN/password if `.biometryNotAvailable` or `.biometryNotEnrolled`.
- **D-22:** Android: `BiometricManager.canAuthenticate(BIOMETRIC_STRONG)` — supports Class 3 biometrics (fingerprint, face). Fallback to PIN/password if `BIOMETRIC_ERROR_NO_HARDWARE` or `BIOMETRIC_ERROR_NONE_ENROLLED`. Minimum API 28 (existing constraint).
- **D-23:** Desktop: No standard biometric API. Default to PIN/password only. On macOS, the `keyring` crate can leverage Touch ID for keychain access if the Mac has a Touch Bar/Touch ID sensor — this is opportunistic, not guaranteed.
- **D-24:** All platforms MUST support PIN/password as the minimum authentication method — biometrics are additive. The app must be fully functional with PIN/password alone.
- **D-25:** SQLCipher availability is unconditional (bundled, no hardware dependency). AES-256-GCM via `aes-gcm` crate is pure Rust with optional hardware acceleration (AES-NI on x86, ARMv8 crypto extensions) — degrades to software implementation transparently.

### Claude's Discretion
- Bootstrap DB format (flat file vs tiny SQLite) for storing salt and wrapped DEK
- Exact Argon2id parameter tuning if benchmarks show 64 MiB is too much for low-end Android devices
- Lock screen UI design details (layout, animations, error messages)
- Whether to show "X failed attempts remaining" or silently accept retries
- Migration strategy for existing unencrypted databases (encrypt-in-place vs copy-and-encrypt)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

No external specs — requirements fully captured in decisions above. Key codebase files to read:

### Persistence layer
- `rust/src/persistence/mod.rs` — Database struct, open() with WAL mode, migration runner
- `rust/src/persistence/schema.rs` — All migration SQL, settings table schema
- `rust/src/persistence/queries.rs` — All query builders (must work with SQLCipher)

### Keychain & capability bridges
- `rust/src/lib.rs` §566-662 — KeychainProvider trait, NullKeychainProvider, DesktopKeychainProvider
- `rust/src/lib.rs` §2397-2470 — FfiApp::new() constructor, data_dir handling, DB open, actor thread setup

### Vector index
- `rust/src/rag/index.rs` — VectorIndex wrapping usearch, serialize/deserialize to disk

### Platform native layers
- `ios/ConfidentialApp/` — SwiftUI layer, MobileEmbeddingProvider, KeychainProvider impl
- `android/app/src/main/java/` — Compose layer, MobileEmbeddingProvider, KeychainProvider impl
- `desktop/iced/src/` — iced UI, DesktopKeychainProvider

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `KeychainProvider` trait (UniFFI callback interface): Already abstracts platform keychain — can store/load the DEK using existing `store`/`load`/`delete` methods. Needs no changes for DEK storage.
- `Database::open()` in `persistence/mod.rs`: Entry point for SQLite connection — this is where SQLCipher key pragma will be injected.
- `Screen` enum in `lib.rs`: Navigation FSM — add `Locked` variant following the established pattern.
- `AppState` fields: Add `biometric_available: bool`, `lock_timeout_seconds: i64`, etc. following the existing record pattern.
- `settings` table in persistence: Already stores app preferences as key-value pairs — lock timeout config fits here.

### Established Patterns
- **UniFFI callback interfaces** for platform capabilities: `KeychainProvider`, `EmbeddingProvider`, `FilePickerProvider` — biometric authentication should follow the same pattern (new `BiometricProvider` callback interface).
- **Actor-thread-only DB access**: All persistence goes through the actor on a dedicated thread. The encrypted DB open must happen on this thread.
- **`AppAction` → actor → `AppState` cycle**: Authentication actions (unlock, set PIN, duress wipe) follow the same dispatch pattern.
- **Migration runner**: Existing `run_migrations()` with `user_version` pragma. SQLCipher migration from unencrypted DB needs special handling (sqlcipher_export).

### Integration Points
- `FfiApp::new()`: Must defer `Database::open()` until DEK is available (post-unlock). Current code opens DB immediately in the actor thread constructor.
- `Screen` navigation: `Locked` screen must intercept before any other screen renders.
- Settings UI: Lock timeout and PIN management go in existing settings screens (Phase 24/26 established the pattern).
- Onboarding wizard: PIN setup step added after existing wizard completes (Phase 7 wizard flow).

</code_context>

<specifics>
## Specific Ideas

- User explicitly wants biometrics as primary unlock with PIN/password as fallback — biometric should be prompted automatically, PIN/password requires manual interaction
- Duress PIN is a key feature — must be non-obvious (looks like a normal PIN entry, triggers silently)
- Graceful degradation is important — older phones with no biometric hardware must still work fully via PIN/password
- "All platforms need to be covered" — desktop is explicitly included, not just mobile
- Hardware capabilities should be utilized "if possible" — use Secure Enclave, StrongBox, AES-NI where available but don't require them

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 28-local-data-encryption-authentication*
*Context gathered: 2026-04-09*
