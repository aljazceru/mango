/// AES-256-GCM file encryption with MGO1 magic header.
///
/// File format: `[MGO1][12-byte nonce][ciphertext + 16-byte GCM tag]`
///
/// Security properties (per threat model):
/// - T-28-05: GCM authentication tag (16 bytes) detects any ciphertext modification
/// - T-28-07: Fresh random nonce per encrypt operation via OsRng; no nonce reuse possible
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use zeroize::Zeroizing;

/// Magic header identifying Mango encrypted files (version 1).
const MAGIC: &[u8; 4] = b"MGO1";

/// Minimum data length: 4 (magic) + 12 (nonce) + 16 (empty GCM tag) = 32 bytes.
const MIN_DATA_LEN: usize = 4 + 12 + 16;

/// Encrypt `plaintext` with `dek` using AES-256-GCM.
///
/// Returns bytes in format: `[MGO1][12-byte nonce][ciphertext+16-byte tag]`.
/// A fresh random nonce is generated for every call (T-28-07).
pub fn encrypt_file(dek: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new_from_slice(dek).expect("32-byte key is always valid for AES-256");

    // Generate a fresh 12-byte random nonce (T-28-07: no nonce reuse)
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    // encrypt appends the 16-byte auth tag to ciphertext
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .expect("AES-256-GCM encryption should never fail for valid key+nonce");

    // Assemble: [MGO1][nonce][ciphertext+tag]
    let mut out = Vec::with_capacity(MAGIC.len() + 12 + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    out
}

/// Decrypt `data` produced by `encrypt_file` using `dek`.
///
/// Verifies the MGO1 magic header, extracts the nonce, and decrypts.
/// Returns `Err` if the magic is missing, data is too short, or the GCM tag fails (T-28-05).
pub fn decrypt_file(dek: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, anyhow::Error> {
    if data.len() < MIN_DATA_LEN {
        anyhow::bail!(
            "encrypted data too short: {} bytes (minimum {})",
            data.len(),
            MIN_DATA_LEN
        );
    }

    // Verify magic header
    if &data[..4] != MAGIC {
        anyhow::bail!("missing MGO1 magic header");
    }

    let nonce_bytes: [u8; 12] = data[4..16].try_into().expect("slice is exactly 12 bytes");
    let ciphertext_with_tag = &data[16..];

    let key_bytes = Zeroizing::new(*dek);
    let cipher =
        Aes256Gcm::new_from_slice(&*key_bytes).expect("32-byte key is always valid for AES-256");
    let nonce = Nonce::from(nonce_bytes);

    cipher
        .decrypt(&nonce, ciphertext_with_tag)
        .map_err(|_| anyhow::anyhow!("AES-256-GCM decryption failed: wrong key or corrupted data"))
}
