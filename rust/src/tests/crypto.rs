/// Tests for the crypto module (Plan 28-01, Task 2).
///
/// Covers: file_crypto (encrypt/decrypt), key_derivation (generate_dek, derive_kek,
/// wrap/unwrap, hash_pin, verify_pin_hash), bootstrap_db (init, read, write).
use crate::crypto::bootstrap_db::{AuthParams, BootstrapDb};
use crate::crypto::file_crypto::{decrypt_file, encrypt_file};
use crate::crypto::key_derivation::{
    derive_kek, generate_dek, generate_salt, hash_pin, unwrap_dek, verify_pin_hash, wrap_dek,
    DEFAULT_ITERATIONS, DEFAULT_MEMORY_KIB, DEFAULT_PARALLELISM,
};

// ── file_crypto ───────────────────────────────────────────────────────────────

#[test]
fn test_encrypt_decrypt_round_trip() {
    let dek: [u8; 32] = generate_dek();
    let plaintext = b"Hello, confidential world!";
    let ciphertext = encrypt_file(&dek, plaintext);
    let recovered = decrypt_file(&dek, &ciphertext).expect("decrypt should succeed");
    assert_eq!(recovered, plaintext);
}

#[test]
fn test_decrypt_with_wrong_key_fails() {
    let dek: [u8; 32] = generate_dek();
    let wrong_dek: [u8; 32] = generate_dek();
    let plaintext = b"sensitive data";
    let ciphertext = encrypt_file(&dek, plaintext);
    let result = decrypt_file(&wrong_dek, &ciphertext);
    assert!(result.is_err(), "decrypt with wrong key should fail");
}

#[test]
fn test_decrypt_truncated_data_fails() {
    let dek: [u8; 32] = generate_dek();
    let plaintext = b"some data";
    let ciphertext = encrypt_file(&dek, plaintext);
    // Truncate to just the magic header + partial nonce
    let truncated = &ciphertext[..8];
    let result = decrypt_file(&dek, truncated);
    assert!(result.is_err(), "decrypt of truncated data should fail");
}

#[test]
fn test_encrypt_prepends_mgo1_magic() {
    let dek: [u8; 32] = generate_dek();
    let ciphertext = encrypt_file(&dek, b"test");
    assert_eq!(&ciphertext[..4], b"MGO1", "ciphertext must start with MGO1 magic");
}

#[test]
fn test_decrypt_missing_magic_fails() {
    let dek: [u8; 32] = generate_dek();
    // Data without MGO1 header
    let bad_data = b"NOPE\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f";
    let result = decrypt_file(&dek, bad_data);
    assert!(result.is_err(), "decrypt without MGO1 magic should fail");
}

// ── key_derivation ────────────────────────────────────────────────────────────

#[test]
fn test_generate_dek_produces_32_nonzero_bytes() {
    let dek = generate_dek();
    assert_eq!(dek.len(), 32);
    // Statistically a randomly generated key should not be all zeros
    assert_ne!(dek, [0u8; 32], "generate_dek must not return all zeros");
}

#[test]
fn test_derive_kek_deterministic_with_same_pin_and_salt() {
    let pin = b"my_secure_pin_1234";
    let salt = generate_salt();
    let kek1 = derive_kek(pin, &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek should succeed");
    let kek2 = derive_kek(pin, &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek should succeed");
    assert_eq!(kek1, kek2, "same PIN + salt must produce same KEK");
}

#[test]
fn test_derive_kek_different_pin_produces_different_kek() {
    let salt = generate_salt();
    let kek1 = derive_kek(b"pin_a", &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek pin_a");
    let kek2 = derive_kek(b"pin_b", &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek pin_b");
    assert_ne!(kek1, kek2, "different PINs must produce different KEKs");
}

#[test]
fn test_wrap_then_unwrap_dek_round_trips() {
    let pin = b"test_pin";
    let salt = generate_salt();
    let kek = derive_kek(pin, &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek");
    let dek = generate_dek();
    let wrapped = wrap_dek(&kek, &dek);
    let unwrapped = unwrap_dek(&kek, &wrapped).expect("unwrap_dek should succeed");
    assert_eq!(dek, unwrapped, "unwrap must return the original DEK");
}

#[test]
fn test_unwrap_dek_with_wrong_kek_fails() {
    let salt = generate_salt();
    let kek = derive_kek(b"correct_pin", &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek correct");
    let wrong_kek = derive_kek(b"wrong_pin", &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek wrong");
    let dek = generate_dek();
    let wrapped = wrap_dek(&kek, &dek);
    let result = unwrap_dek(&wrong_kek, &wrapped);
    assert!(result.is_err(), "unwrap with wrong KEK must fail");
}

#[test]
fn test_hash_pin_then_verify_succeeds_for_correct_pin() {
    let pin = b"my_duress_pin";
    let salt = generate_salt();
    let hash = hash_pin(pin, &salt);
    assert!(
        verify_pin_hash(pin, &hash),
        "verify_pin_hash should succeed for correct PIN"
    );
}

#[test]
fn test_verify_pin_hash_fails_for_wrong_pin() {
    let pin = b"correct_pin";
    let salt = generate_salt();
    let hash = hash_pin(pin, &salt);
    assert!(
        !verify_pin_hash(b"wrong_pin", &hash),
        "verify_pin_hash should fail for wrong PIN"
    );
}

// ── bootstrap_db ─────────────────────────────────────────────────────────────

fn temp_bootstrap_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!("mango_bootstrap_test_{}_{}.db", tag, uuid::Uuid::new_v4()))
        .to_str()
        .unwrap()
        .to_string()
}

#[test]
fn test_bootstrap_db_init_creates_table() {
    let path = temp_bootstrap_path("init");
    let db = BootstrapDb::open(&path).expect("BootstrapDb::open should succeed");
    // After init, has_auth_params should be false (empty table)
    assert!(!db.has_auth_params(), "new bootstrap DB should have no auth params");
}

#[test]
fn test_bootstrap_db_write_read_round_trips() {
    let path = temp_bootstrap_path("round_trip");
    let db = BootstrapDb::open(&path).expect("BootstrapDb::open");

    let dek = generate_dek();
    let salt = generate_salt();
    let kek = derive_kek(b"test_pin", &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek");
    let wrapped = wrap_dek(&kek, &dek);
    // hash_pin embeds its own random salt in the PHC string; no separate duress_salt needed.
    let dummy_salt = generate_salt();
    let duress_hash = hash_pin(b"duress_pin", &dummy_salt);

    let params = AuthParams {
        salt: salt.to_vec(),
        wrapped_dek: wrapped,
        duress_hash: Some(duress_hash.clone()),
        kdf_memory_kib: DEFAULT_MEMORY_KIB,
        kdf_iterations: DEFAULT_ITERATIONS,
        kdf_parallelism: DEFAULT_PARALLELISM,
    };

    db.write_auth_params(&params).expect("write_auth_params");

    let read_back = db.read_auth_params().expect("read_auth_params").expect("should be Some");
    assert_eq!(read_back.salt, params.salt);
    assert_eq!(read_back.wrapped_dek, params.wrapped_dek);
    assert_eq!(read_back.duress_hash, params.duress_hash);
    assert_eq!(read_back.kdf_memory_kib, DEFAULT_MEMORY_KIB);
    assert_eq!(read_back.kdf_iterations, DEFAULT_ITERATIONS);
    assert_eq!(read_back.kdf_parallelism, DEFAULT_PARALLELISM);
    assert!(db.has_auth_params(), "has_auth_params should be true after write");
}

#[test]
fn test_bootstrap_db_delete_all_clears_params() {
    let path = temp_bootstrap_path("delete");
    let db = BootstrapDb::open(&path).expect("BootstrapDb::open");

    let salt = generate_salt();
    let dek = generate_dek();
    let kek = derive_kek(b"pin", &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .expect("derive_kek");
    let params = AuthParams {
        salt: salt.to_vec(),
        wrapped_dek: wrap_dek(&kek, &dek),
        duress_hash: None,
        kdf_memory_kib: DEFAULT_MEMORY_KIB,
        kdf_iterations: DEFAULT_ITERATIONS,
        kdf_parallelism: DEFAULT_PARALLELISM,
    };

    db.write_auth_params(&params).expect("write_auth_params");
    assert!(db.has_auth_params());

    db.delete_all().expect("delete_all");
    assert!(!db.has_auth_params(), "has_auth_params should be false after delete_all");
    let read_back = db.read_auth_params().expect("read_auth_params");
    assert!(read_back.is_none(), "read_auth_params should return None after delete_all");
}
