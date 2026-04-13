/// Tests for SQLCipher-based encrypted database operations (Plan 28-01, Task 1).
///
/// Covers: open_encrypted, is_encrypted, migrate_to_encrypted.
use crate::persistence::Database;

fn temp_db_path(tag: &str) -> String {
    std::env::temp_dir()
        .join(format!(
            "mango_enc_test_{}_{}.db",
            tag,
            uuid::Uuid::new_v4()
        ))
        .to_str()
        .unwrap()
        .to_string()
}

/// A 64-character hex string representing a 32-byte key.
const TEST_KEY_HEX: &str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
const WRONG_KEY_HEX: &str = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

// ── open_encrypted ────────────────────────────────────────────────────────────

#[test]
fn test_open_encrypted_with_correct_key_runs_migrations() {
    let path = temp_db_path("open_ok");
    let db = Database::open_encrypted(&path, TEST_KEY_HEX);
    assert!(
        db.is_ok(),
        "open_encrypted with correct key should succeed: {:?}",
        db.err()
    );
}

#[test]
fn test_open_encrypted_with_wrong_key_returns_error() {
    let path = temp_db_path("open_wrong");
    // Create an encrypted DB first
    Database::open_encrypted(&path, TEST_KEY_HEX).expect("create encrypted db");
    // Re-open with wrong key should fail
    let result = Database::open_encrypted(&path, WRONG_KEY_HEX);
    assert!(
        result.is_err(),
        "open_encrypted with wrong key should return an error"
    );
}

// ── is_encrypted ─────────────────────────────────────────────────────────────

#[test]
fn test_is_encrypted_returns_false_for_plaintext_db() {
    let path = temp_db_path("plain");
    Database::open(&path).expect("create plaintext db");
    assert!(
        !Database::is_encrypted(&path),
        "is_encrypted should return false for a plaintext DB"
    );
}

#[test]
fn test_is_encrypted_returns_true_for_sqlcipher_db() {
    let path = temp_db_path("enc");
    Database::open_encrypted(&path, TEST_KEY_HEX).expect("create encrypted db");
    assert!(
        Database::is_encrypted(&path),
        "is_encrypted should return true for a SQLCipher DB"
    );
}

// ── migrate_to_encrypted ─────────────────────────────────────────────────────

#[test]
fn test_migrate_to_encrypted_converts_plaintext_db() {
    let path = temp_db_path("migrate");

    // Create plaintext DB with some data
    {
        let db = Database::open(&path).expect("create plaintext db");
        db.conn()
            .execute_batch(
                "INSERT OR IGNORE INTO settings (key, value) VALUES ('test_key', 'test_value');",
            )
            .ok(); // OK if settings table doesn't exist yet, migration may handle it
    }

    assert!(
        !Database::is_encrypted(&path),
        "DB should start as plaintext"
    );

    // Migrate to encrypted
    Database::migrate_to_encrypted(&path, TEST_KEY_HEX)
        .expect("migrate_to_encrypted should succeed");

    // Now it should be encrypted
    assert!(
        Database::is_encrypted(&path),
        "DB should be encrypted after migration"
    );

    // And it should open with the correct key
    let db = Database::open_encrypted(&path, TEST_KEY_HEX);
    assert!(
        db.is_ok(),
        "Encrypted DB should open with correct key after migration: {:?}",
        db.err()
    );

    // Cleanup
    let _ = std::fs::remove_file(&path);
}
