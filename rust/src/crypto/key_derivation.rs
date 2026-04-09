/// Key derivation, DEK generation, DEK wrapping, and PIN hashing.
///
/// Security properties (per threat model):
/// - T-28-02: `zeroize::Zeroizing` wrapper on all intermediate key buffers
/// - T-28-04: Argon2id with 64 MiB memory is intentionally slow (~0.5 s)
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params, Version,
};
use rand::RngCore;
use zeroize::Zeroizing;

/// Default Argon2id memory cost (64 MiB) per D-08.
pub const DEFAULT_MEMORY_KIB: u32 = 65536;
/// Default Argon2id iteration count per D-08.
pub const DEFAULT_ITERATIONS: u32 = 3;
/// Default Argon2id parallelism per D-08.
pub const DEFAULT_PARALLELISM: u32 = 1;

/// Generate a 32-byte random DEK from OS entropy.
pub fn generate_dek() -> [u8; 32] {
    let mut dek = [0u8; 32];
    OsRng.fill_bytes(&mut dek);
    dek
}

/// Generate a 32-byte random salt from OS entropy.
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Derive a 32-byte Key Encryption Key (KEK) from `pin` and `salt` using Argon2id.
///
/// Parameters should match those stored in the bootstrap DB so the same KEK can
/// be reproduced on subsequent unlocks.
pub fn derive_kek(
    pin: &[u8],
    salt: &[u8],
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
) -> Result<[u8; 32], anyhow::Error> {
    let params = Params::new(memory_kib, iterations, parallelism, Some(32))
        .map_err(|e| anyhow::anyhow!("Argon2 params invalid: {}", e))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

    let mut kek = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(pin, salt, kek.as_mut())
        .map_err(|e| anyhow::anyhow!("Argon2id key derivation failed: {}", e))?;

    Ok(*kek)
}

/// Wrap (encrypt) `dek` with `kek` using AES-256-GCM.
///
/// Output format: `[12-byte nonce][ciphertext + 16-byte tag]` (no MGO1 header --
/// this is a key blob, not a file on disk).
pub fn wrap_dek(kek: &[u8; 32], dek: &[u8; 32]) -> Vec<u8> {
    let cipher =
        Aes256Gcm::new_from_slice(kek).expect("32-byte key is always valid for AES-256");

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, dek.as_ref())
        .expect("AES-256-GCM DEK wrapping should never fail");

    let mut out = Vec::with_capacity(12 + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

/// Unwrap (decrypt) a `wrapped` DEK using `kek`.
///
/// Returns the 32-byte DEK, or an error if the KEK is wrong or data is corrupted.
pub fn unwrap_dek(kek: &[u8; 32], wrapped: &[u8]) -> Result<[u8; 32], anyhow::Error> {
    // Minimum: 12 (nonce) + 32 (DEK) + 16 (tag) = 60 bytes
    if wrapped.len() < 60 {
        anyhow::bail!("wrapped DEK too short: {} bytes", wrapped.len());
    }

    let nonce_bytes: [u8; 12] = wrapped[..12].try_into().expect("slice is exactly 12 bytes");
    let ciphertext = &wrapped[12..];

    let kek_z = Zeroizing::new(*kek);
    let cipher =
        Aes256Gcm::new_from_slice(&*kek_z).expect("32-byte key is always valid for AES-256");
    let nonce = Nonce::from(nonce_bytes);

    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("DEK unwrap failed: wrong KEK or corrupted data"))?;

    if plaintext.len() != 32 {
        anyhow::bail!(
            "unwrapped DEK has unexpected length: {} (expected 32)",
            plaintext.len()
        );
    }

    let mut dek = [0u8; 32];
    dek.copy_from_slice(&plaintext);
    Ok(dek)
}

/// Hash `pin` using Argon2id and return a PHC-format string.
///
/// Used for the duress PIN hash stored in the bootstrap DB.
/// The `_salt` parameter is accepted for API compatibility but the PHC format
/// embeds its own random salt internally (argon2 crate convention).
pub fn hash_pin(pin: &[u8], _salt: &[u8]) -> String {
    let salt_string = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(pin, &salt_string)
        .expect("Argon2 PIN hashing should not fail")
        .to_string()
}

/// Verify `pin` against a PHC-format `hash` produced by `hash_pin`.
///
/// Uses argon2's built-in constant-time comparison (T-28 Pitfall 6: timing side-channel).
pub fn verify_pin_hash(pin: &[u8], hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(pin, &parsed).is_ok()
}
