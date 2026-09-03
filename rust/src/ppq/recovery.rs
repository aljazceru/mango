//! Encrypted PPQ recovery backup envelope (Wave 3).
//!
//! File layout (44-byte header + AES-256-GCM ciphertext + 16-byte tag):
//!
//! ```text
//! offset  len  field
//! 0       5    magic b"MPPQ1"
//! 5       1    version = 1u8
//! 6       1    kdf_id = 1u8 (Argon2id)
//! 7       4    m_kib u32 LE
//! 11      4    t_iter u32 LE
//! 15      1    p_lanes u8
//! 16      16   salt
//! 32      12   AES-256-GCM nonce
//! 44      ..   ciphertext || 16-byte GCM tag
//! ```
//!
//! The full 44-byte header is the AAD. Decryption validates version and KDF
//! parameter bounds *before* any Argon2 work so a malicious file cannot force
//! excessive memory or CPU.

use aes_gcm::{
    aead::{Aead, KeyInit, Nonce, Payload},
    Aes256Gcm,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::Rng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("wrong backup password or corrupt file")]
    WrongPasswordOrCorruptFile,
    #[error("unsupported backup version")]
    UnsupportedBackupVersion,
    #[error("backup file rejected")]
    InvalidFile,
}

pub struct RecoveryDocument {
    pub credit_id: Zeroizing<String>,
    pub api_key: Zeroizing<String>,
    pub backend_id: String,
    pub account_created_at: Option<String>,
    pub exported_at: String,
}

/// Debug never prints secret fields.
impl std::fmt::Debug for RecoveryDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecoveryDocument")
            .field("backend_id", &self.backend_id)
            .finish_non_exhaustive()
    }
}

pub const RECOVERY_FILE_MAX_BYTES: usize = 64 * 1024;

const MAGIC: &[u8; 5] = b"MPPQ1";
const VERSION: u8 = 1;
const KDF_ID: u8 = 1;
const HEADER_LEN: usize = 5 + 1 + 1 + 4 + 4 + 1 + 16 + 12; // 44
const TAG_LEN: usize = 16;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const MIN_ENVELOPE_LEN: usize = HEADER_LEN + TAG_LEN;

// Mobile-safe OWASP-ish defaults: 19 MiB, t=2, p=1.
const DEFAULT_M_KIB: u32 = 19 * 1024;
const DEFAULT_T: u32 = 2;
const DEFAULT_P: u8 = 1;

// Decrypt-side resource-abuse guards (checked before any KDF work).
const MAX_M_KIB: u32 = 65536;
const MAX_T: u32 = 8;
const MAX_P: u8 = 4;

const RECOVERY_FORMAT: &str = "mango.ppq.recovery";
const DEFAULT_BACKEND_ID: &str = "ppq-ai";

#[derive(Serialize, Deserialize)]
struct InnerDocument {
    format: String,
    version: u8,
    credit_id: String,
    api_key: String,
    backend_id: String,
    #[serde(default)]
    account_created_at: Option<String>,
    exported_at: String,
}

fn derive_key(
    password: &str,
    salt: &[u8],
    m_kib: u32,
    t: u32,
    p: u8,
) -> Result<Zeroizing<[u8; 32]>, RecoveryError> {
    let password_copy = Zeroizing::new(password.to_string());
    let params =
        Params::new(m_kib, t, p as u32, Some(32)).map_err(|_| RecoveryError::InvalidFile)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password_copy.as_bytes(), salt, &mut key[..])
        .map_err(|_| RecoveryError::InvalidFile)?;
    Ok(key)
}

pub fn encrypt_recovery_document(
    credit_id: &str,
    api_key: &str,
    account_created_at: Option<&str>,
    exported_at: &str,
    password: &str,
) -> Result<Vec<u8>, RecoveryError> {
    if credit_id.is_empty() || api_key.is_empty() {
        return Err(RecoveryError::InvalidFile);
    }

    let inner = InnerDocument {
        format: RECOVERY_FORMAT.to_string(),
        version: VERSION,
        credit_id: credit_id.to_string(),
        api_key: api_key.to_string(),
        backend_id: DEFAULT_BACKEND_ID.to_string(),
        account_created_at: account_created_at.map(Into::into),
        exported_at: exported_at.to_string(),
    };
    let plaintext =
        Zeroizing::new(serde_json::to_vec(&inner).map_err(|_| RecoveryError::InvalidFile)?);

    let mut salt = [0u8; SALT_LEN];
    rand::rng().fill_bytes(&mut salt);

    let key = derive_key(password, &salt, DEFAULT_M_KIB, DEFAULT_T, DEFAULT_P)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::<Aes256Gcm>::from(nonce_bytes);

    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.push(VERSION);
    header.push(KDF_ID);
    header.extend_from_slice(&DEFAULT_M_KIB.to_le_bytes());
    header.extend_from_slice(&DEFAULT_T.to_le_bytes());
    header.push(DEFAULT_P);
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce_bytes);
    debug_assert_eq!(header.len(), HEADER_LEN);

    let cipher = Aes256Gcm::new_from_slice(&key[..]).map_err(|_| RecoveryError::InvalidFile)?;
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_slice(),
                aad: header.as_slice(),
            },
        )
        .map_err(|_| RecoveryError::InvalidFile)?;

    let mut out = header;
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_recovery_document(
    bytes: &[u8],
    password: &str,
) -> Result<RecoveryDocument, RecoveryError> {
    if bytes.len() > RECOVERY_FILE_MAX_BYTES || bytes.len() < MIN_ENVELOPE_LEN {
        return Err(RecoveryError::InvalidFile);
    }
    if bytes[5] != VERSION {
        return Err(RecoveryError::UnsupportedBackupVersion);
    }
    // Threat review (low): KDF id must be Argon2id — reject silently-unknown
    // KDFs before any derivation work.
    if bytes[6] != KDF_ID {
        return Err(RecoveryError::UnsupportedBackupVersion);
    }

    let m_kib = u32::from_le_bytes(
        bytes[7..11]
            .try_into()
            .map_err(|_| RecoveryError::InvalidFile)?,
    );
    let t = u32::from_le_bytes(
        bytes[11..15]
            .try_into()
            .map_err(|_| RecoveryError::InvalidFile)?,
    );
    let p = bytes[15];
    if m_kib > MAX_M_KIB || t > MAX_T || p > MAX_P {
        return Err(RecoveryError::InvalidFile);
    }

    let salt = &bytes[16..32];
    let nonce_bytes: [u8; NONCE_LEN] = bytes[32..44]
        .try_into()
        .map_err(|_| RecoveryError::InvalidFile)?;
    let header = &bytes[..HEADER_LEN];
    let ciphertext = &bytes[HEADER_LEN..];

    let key = derive_key(password, salt, m_kib, t, p)?;
    let cipher = Aes256Gcm::new_from_slice(&key[..]).map_err(|_| RecoveryError::InvalidFile)?;
    let nonce = Nonce::<Aes256Gcm>::from(nonce_bytes);
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: header,
                },
            )
            .map_err(|_| RecoveryError::WrongPasswordOrCorruptFile)?,
    );

    let inner: InnerDocument =
        serde_json::from_slice(plaintext.as_slice()).map_err(|_| RecoveryError::InvalidFile)?;
    if inner.format != RECOVERY_FORMAT
        || inner.version != VERSION
        || inner.backend_id.is_empty()
        || inner.credit_id.is_empty()
        || inner.api_key.is_empty()
    {
        return Err(RecoveryError::InvalidFile);
    }

    Ok(RecoveryDocument {
        credit_id: Zeroizing::new(inner.credit_id),
        api_key: Zeroizing::new(inner.api_key),
        backend_id: inner.backend_id,
        account_created_at: inner.account_created_at,
        exported_at: inner.exported_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_bytes() -> Vec<u8> {
        encrypt_recovery_document(
            "credit_12345678901234567890",
            "api_key_1234567890",
            Some("2026-09-03T00:00:00Z"),
            "2026-09-03T01:00:00Z",
            "correct-horse-battery-staple",
        )
        .unwrap()
    }

    #[test]
    fn round_trip() {
        let bytes = roundtrip_bytes();
        let doc = decrypt_recovery_document(&bytes, "correct-horse-battery-staple").unwrap();
        assert_eq!(doc.credit_id.as_str(), "credit_12345678901234567890");
        assert_eq!(doc.api_key.as_str(), "api_key_1234567890");
        assert_eq!(doc.backend_id, "ppq-ai");
        assert_eq!(
            doc.account_created_at.as_deref(),
            Some("2026-09-03T00:00:00Z")
        );
        assert_eq!(doc.exported_at, "2026-09-03T01:00:00Z");
    }

    #[test]
    fn wrong_password() {
        let bytes = roundtrip_bytes();
        let err = decrypt_recovery_document(&bytes, "wrong-password").unwrap_err();
        assert!(matches!(err, RecoveryError::WrongPasswordOrCorruptFile));
    }

    #[test]
    fn tampered_header() {
        let bytes = roundtrip_bytes();

        // Magic tamper: AAD mismatch must fail AEAD.
        let mut t = bytes.clone();
        t[0] = b'X';
        let err = decrypt_recovery_document(&t, "correct-horse-battery-staple").unwrap_err();
        assert!(matches!(err, RecoveryError::WrongPasswordOrCorruptFile));

        // Version tamper: version check runs before AEAD.
        let mut t = bytes.clone();
        t[5] = 2;
        let err = decrypt_recovery_document(&t, "correct-horse-battery-staple").unwrap_err();
        assert!(matches!(err, RecoveryError::UnsupportedBackupVersion));

        // m_kib tamper (small +1, still within bounds): AAD mismatch.
        let mut t = bytes.clone();
        t[7] = t[7].wrapping_add(1);
        let err = decrypt_recovery_document(&t, "correct-horse-battery-staple").unwrap_err();
        assert!(matches!(err, RecoveryError::WrongPasswordOrCorruptFile));
    }

    #[test]
    fn truncated_file() {
        let bytes = roundtrip_bytes();
        let err = decrypt_recovery_document(&bytes[..10], "x").unwrap_err();
        assert!(matches!(err, RecoveryError::InvalidFile));
    }

    #[test]
    fn empty_file() {
        let err = decrypt_recovery_document(&[], "x").unwrap_err();
        assert!(matches!(err, RecoveryError::InvalidFile));
    }

    #[test]
    fn version_two() {
        let mut bytes = roundtrip_bytes();
        bytes[5] = 2;
        let err = decrypt_recovery_document(&bytes, "correct-horse-battery-staple").unwrap_err();
        assert!(matches!(err, RecoveryError::UnsupportedBackupVersion));
    }

    #[test]
    fn kdf_abuse() {
        let mut abuse = vec![0u8; HEADER_LEN + TAG_LEN];
        abuse[0..5].copy_from_slice(MAGIC);
        abuse[5] = VERSION;
        abuse[6] = KDF_ID;
        abuse[7..11].copy_from_slice(&999_999u32.to_le_bytes());
        abuse[11..15].copy_from_slice(&DEFAULT_T.to_le_bytes());
        abuse[15] = DEFAULT_P;
        // salt and nonce are all zeros for this guard test.
        let err = decrypt_recovery_document(&abuse, "x").unwrap_err();
        assert!(matches!(err, RecoveryError::InvalidFile));
    }

    #[test]
    fn zeroization_smoke() {
        // The call uses Zeroizing on all sensitive intermediates; this test
        // just verifies the public signatures compile and do not panic.
        let _ = encrypt_recovery_document(
            "credit_123",
            "api_key_123",
            None,
            "2026-09-03T00:00:00Z",
            "password",
        )
        .unwrap();
    }
}
