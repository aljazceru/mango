/// File-backed enrollment crash-recovery tests (Finding 1).
///
/// These tests exercise the durable enrollment transaction spanning the bootstrap
/// auth DB and the SQLCipher main DB. They use real on-disk databases and
/// simulate interruption boundaries by manipulating files and bootstrap state
/// directly.
use std::path::Path;

#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::crypto::bootstrap_db::BootstrapDb;
use crate::crypto::key_derivation::{
    derive_kek, generate_dek, generate_salt, wrap_dek, DEFAULT_ITERATIONS, DEFAULT_MEMORY_KIB,
    DEFAULT_PARALLELISM,
};
use crate::persistence::queries::{insert_conversation, ConversationRow};
use crate::persistence::Database;
use crate::{
    AppAction, AppState, BiometricProvider, EmbeddingStatus, FfiApp, KeychainProvider,
    NullBiometricProvider, NullEmbeddingProvider, NullKeychainProvider, NullLocalLlmProvider,
    Screen,
};

fn temp_dir(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "mango_enroll_test_{}_{}",
        tag,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir.to_str().unwrap().to_string()
}

fn db_path(dir: &str) -> String {
    format!("{}/mango.db", dir)
}

fn bootstrap_path(dir: &str) -> String {
    format!("{}/mango_auth.db", dir)
}

fn test_pin() -> String {
    "1234".into()
}

fn setup_auth_params(pin: &str) -> (crate::crypto::bootstrap_db::AuthParams, String) {
    let dek = generate_dek();
    let salt = generate_salt();
    let kek = derive_kek(
        pin.as_bytes(),
        &salt,
        DEFAULT_MEMORY_KIB,
        DEFAULT_ITERATIONS,
        DEFAULT_PARALLELISM,
    )
    .expect("derive kek");
    let wrapped = wrap_dek(&kek, &dek);
    let dek_hex: String = dek.iter().map(|b| format!("{:02x}", b)).collect();
    let params = crate::crypto::bootstrap_db::AuthParams {
        salt: salt.to_vec(),
        wrapped_dek: wrapped,
        duress_hash: None,
        kdf_memory_kib: DEFAULT_MEMORY_KIB,
        kdf_iterations: DEFAULT_ITERATIONS,
        kdf_parallelism: DEFAULT_PARALLELISM,
    };
    (params, dek_hex)
}

// ── Successful full enrollment preserving file-backed data ───────────────────

#[test]
fn test_setup_pin_migrates_plaintext_preserving_conversations() {
    let dir = temp_dir("migrate_keep_data");
    let path = db_path(&dir);

    // Pre-seed a plaintext main DB with a recognizable conversation.
    {
        let db = Database::open(&path).expect("open plaintext");
        let row = ConversationRow {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Recovery Test Conversation".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: 1700000000,
            updated_at: 1700000000,
            tools_enabled: false,
        };
        insert_conversation(db.conn(), &row).expect("insert conversation");
    }

    // Mark onboarding complete so the app routes to PinSetup, not Onboarding.
    {
        let db = Database::open(&path).expect("open plaintext");
        crate::persistence::queries::set_setting(db.conn(), "has_completed_onboarding", "true")
            .expect("set completed");
    }

    // Build the app over the temp directory.
    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "legacy/completed install with no auth should show PinSetup, got {:?}",
        state.router.current_screen
    );
    assert!(
        state
            .conversations
            .iter()
            .any(|c| c.title == "Recovery Test Conversation"),
        "plaintext conversation should be loaded before enrollment"
    );

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        state.auth_initialized,
        "auth should be initialized after SetupPin"
    );
    assert!(state.encryption_enabled, "main DB should be encrypted");
    assert!(
        state
            .conversations
            .iter()
            .any(|c| c.title == "Recovery Test Conversation"),
        "conversation must survive the encrypted migration"
    );
    assert!(
        Database::is_encrypted(&path),
        "main DB file must be encrypted"
    );

    // Verify the encrypted DB opens with the derived DEK.
    app.dispatch(AppAction::LockApp);
    app.sync();
    app.dispatch(AppAction::UnlockWithPin { pin: test_pin() });
    app.sync();

    let state = app.state();
    assert!(
        state
            .conversations
            .iter()
            .any(|c| c.title == "Recovery Test Conversation"),
        "conversation must still be readable after lock/unlock"
    );

    // Cleanup.
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Persistence-level interruption boundary tests ────────────────────────────

#[test]
fn test_migrate_to_encrypted_leaves_plaintext_backup_until_finalized() {
    let dir = temp_dir("migration_backup");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("create plaintext db");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["sample_key", "sample_value"],
        )
        .expect("insert sample");
    drop(db);

    let (_params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    assert!(
        Database::is_encrypted(&path),
        "main file should be encrypted"
    );
    let bak = Database::plaintext_backup_path(&path);
    assert!(
        Path::new(&bak).exists(),
        "plaintext backup should exist until finalize"
    );

    Database::finalize_encrypted_storage(&path).expect("finalize removes backup");
    assert!(
        !Path::new(&bak).exists(),
        "plaintext backup should be removed after finalize"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_rollback_encrypted_to_plaintext_restores_original_data() {
    let dir = temp_dir("migration_rollback");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("create plaintext db");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["sample_key", "sample_value"],
        )
        .expect("insert sample");
    drop(db);

    let (_params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    Database::rollback_encrypted_to_plaintext(&path).expect("rollback");
    assert!(
        !Database::is_encrypted(&path),
        "main file should be plaintext again"
    );

    let db = Database::open(&path).expect("reopen plaintext");
    let val = crate::persistence::queries::get_setting(db.conn(), "sample_key").expect("read");
    assert_eq!(
        val.as_deref(),
        Some("sample_value"),
        "original data must survive rollback"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_recover_plaintext_after_failed_enrollment_restores_backup() {
    let dir = temp_dir("recover_plain");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("create plaintext db");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["sample_key", "sample_value"],
        )
        .expect("insert sample");
    drop(db);

    // Simulate a migration that got as far as rename-aside but crashed before
    // the encrypted copy moved into place.
    let bak = Database::plaintext_backup_path(&path);
    std::fs::rename(&path, &bak).expect("park plaintext backup");
    assert!(!Path::new(&path).exists());

    let _ = Database::recover_plaintext_after_failed_enrollment(&path).expect("recover");
    assert!(Path::new(&path).exists(), "main file should be restored");

    let db = Database::open(&path).expect("reopen plaintext");
    let val = crate::persistence::queries::get_setting(db.conn(), "sample_key").expect("read");
    assert_eq!(val.as_deref(), Some("sample_value"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_promote_pending_auth_preserves_cold_launch_bypass() {
    let dir = temp_dir("promote_bypass");
    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");

    // Seed active auth with cold_launch_bypass = 1.
    let (params, _dek) = setup_auth_params(&test_pin());
    bs.write_auth_params(&params).expect("write active");
    bs.write_cold_launch_bypass(true).expect("set bypass");

    // Staging different pending auth.
    let (pending_params, _dek2) = setup_auth_params("5678");
    bs.write_pending_auth(&pending_params)
        .expect("write pending");

    bs.promote_pending_auth().expect("promote");
    assert!(
        bs.read_cold_launch_bypass().unwrap_or(false),
        "promote_pending_auth must preserve the existing cold_launch_bypass flag"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Resume from the encrypted-replacement boundary ───────────────────────────

#[test]
fn test_resume_encrypted_main_with_pending_auth_succeeds() {
    let dir = temp_dir("resume_encrypted");
    let path = db_path(&dir);

    // Build a plaintext DB, migrate it, then stage pending auth without
    // promoting active auth.
    let db = Database::open(&path).expect("plaintext");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["has_completed_onboarding", "true"],
        )
        .expect("complete");
    drop(db);

    let (params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_pending_auth(&params).expect("write pending");

    // Create the app: it should detect the encrypted main + pending auth and
    // resume at PinSetup, then complete enrollment when the same PIN is entered.
    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "should resume to PinSetup, got {:?}",
        state.router.current_screen
    );

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        state.auth_initialized,
        "auth should be initialized after resume"
    );
    assert!(
        Database::is_encrypted(&path),
        "main DB should remain encrypted"
    );
    assert!(!bs.has_pending_auth(), "pending auth should be cleared");
    assert!(bs.has_auth_params(), "active auth should be committed");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_resume_wrong_pin_is_retryable() {
    let dir = temp_dir("resume_wrong");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("plaintext");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["has_completed_onboarding", "true"],
        )
        .expect("complete");
    drop(db);

    let (params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_pending_auth(&params).expect("write pending");

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    app.dispatch(AppAction::SetupPin {
        pin: "wrong".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "wrong PIN should stay on PinSetup"
    );
    assert!(!state.auth_initialized, "auth should not be initialized");
    assert!(bs.has_pending_auth(), "pending auth should remain");

    // Retry with the correct PIN.
    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(state.auth_initialized, "correct retry should succeed");

    let _ = std::fs::remove_dir_all(&dir);
}

// ── Duplicate enrollment rejection ───────────────────────────────────────────

#[test]
fn test_setup_pin_rejects_duplicate_enrollment() {
    let dir = temp_dir("dup_enroll");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("plaintext");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["has_completed_onboarding", "true"],
        )
        .expect("complete");
    drop(db);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    assert!(
        app.state().auth_initialized,
        "first enrollment should succeed"
    );

    // Try to enroll again.
    app.dispatch(AppAction::SetupPin {
        pin: "5678".into(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        state.toast.as_deref() == Some("A PIN is already configured."),
        "second enrollment should be rejected"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── R1: checked, retryable cleanup after verified active-auth open ─────────────

#[test]
fn test_finalize_encrypted_storage_is_strict_and_retryable() {
    let dir = temp_dir("finalize_retry");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("create plaintext");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["has_completed_onboarding", "true"],
        )
        .expect("complete");
    drop(db);

    let (_params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    let bak = Database::plaintext_backup_path(&path);
    let enc_tmp = Database::encrypted_temp_path(&path);
    let enc_replaced = Database::replaced_encrypted_path(&path);
    assert!(Path::new(&bak).exists());

    // Simulate a permission failure: make the parent directory read-only.
    let parent = Path::new(&path).parent().unwrap();
    let original_perms = std::fs::metadata(parent).unwrap().permissions();
    #[cfg(target_family = "unix")]
    {
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o555)).unwrap();
    }

    let result = Database::finalize_encrypted_storage(&path);

    #[cfg(target_family = "unix")]
    {
        std::fs::set_permissions(parent, original_perms).unwrap();
    }

    assert!(
        result.is_err(),
        "finalize must be fallible and report cleanup failure"
    );
    assert!(
        Path::new(&bak).exists(),
        "backup must survive a failed finalize"
    );

    // Retry after permission is restored.
    Database::finalize_encrypted_storage(&path).expect("finalize succeeds after retry");
    assert!(
        !Path::new(&bak).exists(),
        "backup removed after successful finalize"
    );
    assert!(
        !Path::new(&enc_tmp).exists(),
        "enc tmp removed after finalize"
    );
    assert!(
        !Path::new(&enc_replaced).exists(),
        "enc replaced removed after finalize"
    );

    // The encrypted main is intact and openable.
    let _ = Database::open_encrypted(&path, &dek_hex).expect("encrypted main still open");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_rollback_preserves_encrypted_main_when_backup_missing() {
    let dir = temp_dir("rollback_no_backup");
    let path = db_path(&dir);

    let db = Database::open(&path).expect("create plaintext");
    db.conn()
        .execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            ["has_completed_onboarding", "true"],
        )
        .expect("complete");
    drop(db);

    let (_params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");
    assert!(Database::is_encrypted(&path));

    // Delete the plaintext backup.
    let bak = Database::plaintext_backup_path(&path);
    assert!(Path::new(&bak).exists());
    std::fs::remove_file(&bak).unwrap();

    // Rollback with no backup must fail and must not destroy the encrypted main.
    let result = Database::rollback_encrypted_to_plaintext(&path);
    assert!(result.is_err(), "rollback must fail without a backup");
    assert!(
        Database::is_encrypted(&path),
        "encrypted main must remain intact when no backup"
    );
    let _ = Database::open_encrypted(&path, &dek_hex).expect("encrypted main still open");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_migrate_to_encrypted_does_not_create_blank_database() {
    let dir = temp_dir("migrate_no_source");
    let path = db_path(&dir);

    // Ensure the source file does not exist.
    assert!(!Path::new(&path).exists());

    let (_params, dek_hex) = setup_auth_params(&test_pin());
    let result = Database::migrate_to_encrypted(&path, &dek_hex);
    assert!(
        result.is_err(),
        "migrate must fail when source does not exist"
    );
    assert!(
        !Path::new(&path).exists(),
        "migrate must not create a blank database"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── R2: preserve pending auth on failed recovery and avoid blank DB ────────────

#[test]
fn test_startup_recovery_failure_blocks_no_blank_database() {
    let dir = temp_dir("startup_recovery_block");
    let path = db_path(&dir);
    let bs_path = bootstrap_path(&dir);

    // Pending auth + pending first-run, but no main/backup/enc files.
    let bs = BootstrapDb::open(&bs_path).expect("open bootstrap");
    let (params, _dek) = setup_auth_params(&test_pin());
    bs.write_pending_auth(&params).expect("write pending");
    bs.set_pending_first_run("complete", Some(&uuid::Uuid::new_v4().to_string()))
        .expect("write pending first run");
    drop(bs);

    assert!(!Path::new(&path).exists());

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "startup with pending auth and no DB must show PinSetup, got {:?}",
        state.router.current_screen
    );
    assert!(
        !Path::new(&path).exists(),
        "must not create a blank database when recovery has not succeeded"
    );
    assert!(state.conversations.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

// ── R2 follow-up: pending auth survives with no DB files; encrypted candidates
//    are preserved and resolvable with the entered PIN ─────────────────────────

#[test]
fn test_startup_pending_auth_no_first_run_blocks_and_preserves_key_material() {
    let dir = temp_dir("startup_pending_no_files");
    let path = db_path(&dir);
    let bs_path = bootstrap_path(&dir);

    // Legacy interrupted enrollment: pending auth exists but there is NO
    // pending_first_run marker and NO database files at all. Pending auth is only
    // written by SetupPin after a real DB exists, so this means data loss — the
    // app must block and must not create a blank replacement.
    let bs = BootstrapDb::open(&bs_path).expect("open bootstrap");
    let (params, _dek) = setup_auth_params(&test_pin());
    bs.write_pending_auth(&params).expect("write pending");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "startup with pending auth but no DB files must block at PinSetup, got {:?}",
        state.router.current_screen
    );
    assert!(
        state.enrollment_resume_pending,
        "enrollment_resume_pending must be set so the UI can explain the resume flow"
    );
    assert!(
        !Path::new(&path).exists(),
        "no blank database may be created"
    );

    // Key metadata must survive for a later recovery attempt.
    let bs = BootstrapDb::open(&bs_path).expect("reopen bootstrap");
    assert!(
        bs.has_pending_auth(),
        "pending auth must be preserved when nothing can be recovered"
    );
    drop(bs);

    // A SetupPin retry with the same PIN must still block — there is nothing to
    // resolve — and pending auth must still survive.
    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "SetupPin retry with no recoverable files must stay on PinSetup, got {:?}",
        state.router.current_screen
    );
    assert!(!state.auth_initialized);
    assert!(!Path::new(&path).exists(), "still no blank database");
    let bs = BootstrapDb::open(&bs_path).expect("reopen bootstrap");
    assert!(
        bs.has_pending_auth(),
        "pending auth must survive a blocked SetupPin retry"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_recovery_preserves_and_promotes_only_enc_tmp_candidate() {
    let dir = temp_dir("only_enc_tmp");
    let path = db_path(&dir);

    // Seed a plaintext DB with recognizable data, migrate it, then simulate a
    // crash after the encrypted export was written but before the rename: only
    // the .enc_tmp candidate survives.
    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["sample_key", "sample_value"],
            )
            .expect("insert sample");
    }
    let (params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    let enc_tmp = Database::encrypted_temp_path(&path);
    let bak = Database::plaintext_backup_path(&path);
    std::fs::rename(&path, &enc_tmp).expect("move encrypted main to enc_tmp");
    std::fs::remove_file(&bak).expect("remove plaintext backup");
    assert!(!Path::new(&path).exists());
    assert!(Path::new(&enc_tmp).exists());

    // Recovery must report the candidate and must not delete it.
    let outcome = Database::recover_plaintext_after_failed_enrollment(&path).expect("recover");
    assert_eq!(
        outcome,
        crate::persistence::EnrollmentRecovery::EncryptedCandidate,
        "a sole surviving encrypted candidate must be reported, not deleted"
    );
    assert!(Path::new(&enc_tmp).exists(), "enc_tmp must be preserved");
    assert!(!Path::new(&path).exists(), "no blank main may be created");

    // The wrong DEK must not promote or destroy the candidate.
    let (_wrong_params, wrong_dek_hex) = setup_auth_params("9999");
    let promoted =
        Database::promote_encrypted_candidate(&path, &wrong_dek_hex).expect("promote attempt");
    assert!(!promoted, "wrong DEK must not promote the candidate");
    assert!(
        Path::new(&enc_tmp).exists(),
        "candidate preserved on failure"
    );
    assert!(!Path::new(&path).exists(), "no blank main may be created");

    // The correct DEK verifies and promotes the candidate into place.
    assert!(Database::promote_encrypted_candidate(&path, &dek_hex).expect("promote"));
    assert!(Database::is_encrypted(&path), "promoted main is encrypted");
    assert!(
        !Path::new(&enc_tmp).exists(),
        "enc_tmp consumed by promotion"
    );
    let db = Database::open_encrypted(&path, &dek_hex).expect("open promoted");
    let val = crate::persistence::queries::get_setting(db.conn(), "sample_key").expect("read");
    assert_eq!(
        val.as_deref(),
        Some("sample_value"),
        "data survives promotion"
    );

    let _ = params; // pending auth material remains the caller's responsibility
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_recovery_preserves_and_promotes_only_enc_replaced_candidate() {
    let dir = temp_dir("only_enc_replaced");
    let path = db_path(&dir);

    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["sample_key", "sample_value"],
            )
            .expect("insert sample");
    }
    let (_params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    // Simulate a rollback that crashed after parking the encrypted main: only
    // .enc_replaced survives.
    let enc_replaced = Database::replaced_encrypted_path(&path);
    let bak = Database::plaintext_backup_path(&path);
    std::fs::rename(&path, &enc_replaced).expect("park encrypted main");
    std::fs::remove_file(&bak).expect("remove plaintext backup");

    let outcome = Database::recover_plaintext_after_failed_enrollment(&path).expect("recover");
    assert_eq!(
        outcome,
        crate::persistence::EnrollmentRecovery::EncryptedCandidate
    );
    assert!(
        Path::new(&enc_replaced).exists(),
        "enc_replaced must be preserved"
    );

    assert!(Database::promote_encrypted_candidate(&path, &dek_hex).expect("promote"));
    assert!(Database::is_encrypted(&path));
    let db = Database::open_encrypted(&path, &dek_hex).expect("open promoted");
    let val = crate::persistence::queries::get_setting(db.conn(), "sample_key").expect("read");
    assert_eq!(val.as_deref(), Some("sample_value"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_setup_pin_resumes_from_only_encrypted_candidate() {
    let dir = temp_dir("resume_from_candidate");
    let path = db_path(&dir);
    let bs_path = bootstrap_path(&dir);

    // Stage: pending auth + only a surviving .enc_tmp holding the migrated data.
    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["has_completed_onboarding", "true"],
            )
            .expect("complete");
        let row = ConversationRow {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Candidate Resume".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: 1700000000,
            updated_at: 1700000000,
            tools_enabled: false,
        };
        insert_conversation(db.conn(), &row).expect("insert conversation");
    }
    let (params, dek_hex) = setup_auth_params(&test_pin());
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");
    let enc_tmp = Database::encrypted_temp_path(&path);
    let bak = Database::plaintext_backup_path(&path);
    std::fs::rename(&path, &enc_tmp).expect("move encrypted main to enc_tmp");
    std::fs::remove_file(&bak).expect("remove plaintext backup");

    let bs = BootstrapDb::open(&bs_path).expect("open bootstrap");
    bs.write_pending_auth(&params).expect("write pending");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::PinSetup),
        "surviving candidate must route to PinSetup resume, got {:?}",
        state.router.current_screen
    );
    assert!(state.enrollment_resume_pending);
    assert!(
        Path::new(&enc_tmp).exists(),
        "candidate must not be deleted"
    );

    // Re-entering the previously chosen PIN resolves the candidate and finishes
    // enrollment without losing data.
    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(state.auth_initialized, "resume via candidate must succeed");
    assert!(!state.enrollment_resume_pending);
    assert!(Database::is_encrypted(&path));
    assert!(
        state
            .conversations
            .iter()
            .any(|c| c.title == "Candidate Resume"),
        "migrated data must be readable after candidate promotion"
    );
    let bs = BootstrapDb::open(&bs_path).expect("reopen bootstrap");
    assert!(bs.has_auth_params());
    assert!(!bs.has_pending_auth());

    let _ = std::fs::remove_dir_all(&dir);
}

// ── R4: actual subprocess interruption / restart ─────────────────────────────

/// Child entry point: stages the post-promotion / pre-cleanup state and then
/// exits the process hard (no destructors, no finalize), simulating a crash at
/// the most dangerous boundary — active auth committed, plaintext backup still
/// on disk.
///
/// When run in the normal test suite the env var is absent and this returns
/// immediately; the parent test drives it via `std::env::current_exe()`.
#[test]
fn child_stage_post_promotion_state_then_exit() {
    let dir = match std::env::var("MANGO_ENROLL_CHILD_DIR") {
        Ok(d) if !d.is_empty() => d,
        _ => return,
    };
    let path = db_path(&dir);
    let bs_path = bootstrap_path(&dir);

    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["has_completed_onboarding", "true"],
            )
            .expect("complete");
        let row = ConversationRow {
            id: "child-conv".into(),
            title: "Subprocess Conversation".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: 1700000000,
            updated_at: 1700000000,
            tools_enabled: false,
        };
        insert_conversation(db.conn(), &row).expect("insert conversation");
    }

    let (params, dek_hex) = setup_auth_params("1234");
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");

    let bs = BootstrapDb::open(&bs_path).expect("open bootstrap");
    bs.write_pending_auth(&params).expect("write pending");
    bs.promote_pending_auth().expect("promote active auth");

    // Hard exit BEFORE finalize_encrypted_storage — the plaintext backup and any
    // temp files are left exactly as a crash would leave them.
    std::process::exit(0);
}

#[test]
fn test_subprocess_restart_after_promotion_recovers_and_finalizes() {
    let dir = temp_dir("subprocess_restart");
    let path = db_path(&dir);
    let bak = Database::plaintext_backup_path(&path);
    let bs_path = bootstrap_path(&dir);

    // Run the staging helper in a real child process so this exercises an actual
    // process boundary, not an in-process drop.
    let exe = std::env::current_exe().expect("current test binary");
    let status = std::process::Command::new(exe)
        .args([
            "--exact",
            "tests::enrollment_recovery::child_stage_post_promotion_state_then_exit",
            "--nocapture",
        ])
        .env("MANGO_ENROLL_CHILD_DIR", &dir)
        .status()
        .expect("spawn child");
    assert!(status.success(), "child process must exit cleanly");

    // The child left: encrypted main + plaintext backup + committed active auth.
    assert!(Database::is_encrypted(&path), "encrypted main staged");
    assert!(
        Path::new(&bak).exists(),
        "plaintext backup must survive the simulated crash"
    );
    {
        let bs = BootstrapDb::open(&bs_path).expect("open bootstrap");
        assert!(bs.has_auth_params(), "active auth committed");
        assert!(!bs.has_pending_auth(), "pending auth consumed by promote");
    }

    // A fresh process over the same directory must open normally (committed
    // auth), restore the conversation, and finalize away the plaintext backup.
    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    assert!(matches!(app.state().router.current_screen, Screen::Locked));

    app.dispatch(AppAction::UnlockWithPin { pin: "1234".into() });
    app.sync();

    let state = app.state();
    assert!(
        state
            .conversations
            .iter()
            .any(|c| c.title == "Subprocess Conversation"),
        "original data must be readable after restart unlock"
    );
    assert!(
        !Path::new(&bak).exists(),
        "post-unlock finalize must remove the plaintext backup"
    );
    assert!(Database::is_encrypted(&path));

    let _ = std::fs::remove_dir_all(&dir);
}

// ── R4: fake keychain / biometric enrollment coverage ────────────────────────

/// In-memory keychain fake whose contents are shared across FfiApp instances,
/// so secrets staged during enrollment survive a simulated process restart.
#[derive(Clone, Default)]
struct MemoryKeychainProvider {
    inner: Arc<Mutex<HashMap<(String, String), String>>>,
}

impl KeychainProvider for MemoryKeychainProvider {
    fn store(&self, service: String, key: String, value: String) -> bool {
        self.inner.lock().unwrap().insert((service, key), value);
        true
    }
    fn load(&self, service: String, key: String) -> Option<String> {
        self.inner.lock().unwrap().get(&(service, key)).cloned()
    }
    fn delete(&self, service: String, key: String) -> bool {
        self.inner.lock().unwrap().remove(&(service, key));
        true
    }
}

/// Keychain fake whose writes always fail, simulating a Keystore/Keychain
/// error while storing the biometric DEK.
struct FailingKeychainProvider;

impl KeychainProvider for FailingKeychainProvider {
    fn store(&self, _: String, _: String, _: String) -> bool {
        false
    }
    fn load(&self, _: String, _: String) -> Option<String> {
        None
    }
    fn delete(&self, _: String, _: String) -> bool {
        false
    }
}

/// Biometric fake: reports enrolled hardware and returns a configurable result.
struct FakeBiometricProvider {
    succeed: AtomicBool,
}

impl FakeBiometricProvider {
    fn succeeding() -> Self {
        Self {
            succeed: AtomicBool::new(true),
        }
    }
    fn failing() -> Self {
        Self {
            succeed: AtomicBool::new(false),
        }
    }
}

impl BiometricProvider for FakeBiometricProvider {
    fn biometric_status(&self) -> String {
        "available".into()
    }
    fn authenticate(&self, _reason: String) -> bool {
        self.succeed.load(Ordering::SeqCst)
    }
}

fn wait_until(app: &FfiApp, mut cond: impl FnMut(&AppState) -> bool, timeout_ms: u64) -> bool {
    let start = std::time::Instant::now();
    loop {
        app.sync();
        if cond(&app.state()) {
            return true;
        }
        if start.elapsed().as_millis() > timeout_ms as u128 {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn test_setup_pin_biometric_stages_dek_and_fake_biometric_unlocks() {
    let dir = temp_dir("bio_enroll");
    let path = db_path(&dir);

    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["has_completed_onboarding", "true"],
            )
            .expect("complete");
        let row = ConversationRow {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Biometric Conversation".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: 1700000000,
            updated_at: 1700000000,
            tools_enabled: false,
        };
        insert_conversation(db.conn(), &row).expect("insert conversation");
    }

    let keychain = MemoryKeychainProvider::default();
    let app = FfiApp::new(
        dir.clone(),
        Box::new(keychain.clone()),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(FakeBiometricProvider::succeeding()),
    );
    app.sync();
    assert!(
        app.state().biometric_available,
        "fake biometric must report available"
    );

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: true,
    });
    app.sync();

    let state = app.state();
    assert!(state.auth_initialized);
    assert!(
        state.biometric_login_enabled,
        "biometric login should be enabled after successful DEK staging"
    );
    assert!(
        keychain
            .load("mango".to_string(), "dek".to_string())
            .is_some(),
        "DEK must be staged in the platform keychain"
    );

    // Simulate a restart over the same data dir with the same keychain contents.
    drop(app);
    let app2 = FfiApp::new(
        dir.clone(),
        Box::new(keychain.clone()),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(FakeBiometricProvider::succeeding()),
    );
    app2.sync();

    let state = app2.state();
    assert!(
        state.biometric_login_enabled,
        "biometric login must be detected on restart from the keychain"
    );
    assert!(matches!(state.router.current_screen, Screen::Locked));

    app2.dispatch(AppAction::AttemptBiometricUnlock);
    let unlocked = wait_until(
        &app2,
        |s| !matches!(s.router.current_screen, Screen::Locked),
        10_000,
    );
    assert!(unlocked, "fake biometric unlock should leave Locked");

    let state = app2.state();
    assert!(
        state
            .conversations
            .iter()
            .any(|c| c.title == "Biometric Conversation"),
        "conversation must be readable after biometric unlock"
    );

    // A failed biometric attempt must stay locked without touching storage.
    drop(app2);
    let app3 = FfiApp::new(
        dir.clone(),
        Box::new(keychain.clone()),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(FakeBiometricProvider::failing()),
    );
    app3.sync();
    app3.dispatch(AppAction::AttemptBiometricUnlock);
    let still_locked = !wait_until(
        &app3,
        |s| !matches!(s.router.current_screen, Screen::Locked),
        2_000,
    );
    assert!(still_locked, "failed biometric must keep the app locked");
    assert!(Database::is_encrypted(&path));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_setup_pin_keychain_store_failure_does_not_enable_biometric() {
    let dir = temp_dir("keychain_fail");
    let path = db_path(&dir);

    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["has_completed_onboarding", "true"],
            )
            .expect("complete");
    }

    let app = FfiApp::new(
        dir.clone(),
        Box::new(FailingKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(FakeBiometricProvider::succeeding()),
    );
    app.sync();

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: None,
        enable_biometric: true,
    });
    app.sync();

    let state = app.state();
    assert!(
        state.auth_initialized,
        "enrollment itself must still succeed when the keychain write fails"
    );
    assert!(state.encryption_enabled);
    assert!(
        !state.biometric_login_enabled,
        "a failed DEK store must not enable a dead biometric unlock path"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ── PIN policy enforcement in core SetupPin ──────────────────────────────────

#[test]
fn test_setup_pin_rejects_short_pin_without_touching_storage() {
    let dir = temp_dir("short_pin");
    let path = db_path(&dir);
    let bs_path = bootstrap_path(&dir);

    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["has_completed_onboarding", "true"],
            )
            .expect("complete");
        let row = ConversationRow {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Policy Test".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: 1700000000,
            updated_at: 1700000000,
            tools_enabled: false,
        };
        insert_conversation(db.conn(), &row).expect("insert conversation");
    }

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();

    for (pin, expected_fragment) in [
        ("", "valid PIN"),
        ("   ", "valid PIN"),
        ("abc", "at least 4 characters"),
        ("xy", "at least 4 characters"),
    ] {
        app.dispatch(AppAction::SetupPin {
            pin: pin.into(),
            duress_pin: None,
            enable_biometric: false,
        });
        app.sync();
        let state = app.state();
        assert!(
            state
                .toast
                .as_deref()
                .map(|t| t.contains(expected_fragment))
                .unwrap_or(false),
            "pin {pin:?} must be rejected (toast {:?})",
            state.toast
        );
        assert!(!state.auth_initialized);
    }

    // Storage must be completely untouched: no pending/auth rows, DB still
    // plaintext, no migration artifacts.
    let bs = BootstrapDb::open(&bs_path).expect("open bootstrap");
    assert!(!bs.has_auth_params(), "no active auth may be written");
    assert!(!bs.has_pending_auth(), "no pending auth may be written");
    drop(bs);
    assert!(!Database::is_encrypted(&path), "DB must remain plaintext");
    assert!(!Path::new(&Database::plaintext_backup_path(&path)).exists());
    assert!(!Path::new(&Database::encrypted_temp_path(&path)).exists());

    // Colliding and too-short duress values are also rejected before any write.
    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: Some(test_pin()),
        enable_biometric: false,
    });
    app.sync();
    assert_eq!(
        app.state().toast.as_deref(),
        Some("Duress PIN must be different from your main PIN.")
    );

    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: Some("ab".into()),
        enable_biometric: false,
    });
    app.sync();
    assert_eq!(
        app.state().toast.as_deref(),
        Some("Emergency PIN must be at least 4 characters.")
    );

    let bs = BootstrapDb::open(&bs_path).expect("reopen bootstrap");
    assert!(!bs.has_auth_params());
    assert!(!bs.has_pending_auth());
    drop(bs);
    assert!(!Database::is_encrypted(&path));

    // A valid pair still enrolls cleanly.
    app.dispatch(AppAction::SetupPin {
        pin: test_pin(),
        duress_pin: Some("9876".into()),
        enable_biometric: false,
    });
    app.sync();
    assert!(app.state().auth_initialized);
    assert!(app.state().duress_pin_configured);
    assert!(Database::is_encrypted(&path));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_unlock_preserves_legacy_short_passphrase() {
    // Credential bytes are fed to the KDF unchanged: an existing install whose
    // passphrase predates the 4-character policy must still unlock.
    let dir = temp_dir("legacy_short_pin");
    let path = db_path(&dir);

    {
        let db = Database::open(&path).expect("plaintext");
        db.conn()
            .execute(
                "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
                ["has_completed_onboarding", "true"],
            )
            .expect("complete");
        let row = ConversationRow {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Legacy Data".into(),
            model_id: "test-model".into(),
            backend_id: "test-backend".into(),
            system_prompt: None,
            created_at: 1700000000,
            updated_at: 1700000000,
            tools_enabled: false,
        };
        insert_conversation(db.conn(), &row).expect("insert conversation");
    }

    // Stage active auth with a 3-char legacy passphrase, bypassing SetupPin.
    let legacy_pin = "abc";
    let (params, dek_hex) = setup_auth_params(legacy_pin);
    Database::migrate_to_encrypted(&path, &dek_hex).expect("migrate");
    let bs = BootstrapDb::open(&bootstrap_path(&dir)).expect("open bootstrap");
    bs.write_auth_params(&params).expect("write active auth");
    drop(bs);

    let app = FfiApp::new(
        dir.clone(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    assert!(matches!(app.state().router.current_screen, Screen::Locked));

    app.dispatch(AppAction::UnlockWithPin {
        pin: legacy_pin.into(),
    });
    app.sync();

    let state = app.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Locked),
        "legacy passphrase must still unlock, got {:?}",
        state.router.current_screen
    );
    assert!(
        state.conversations.iter().any(|c| c.title == "Legacy Data"),
        "data must be readable after legacy unlock"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
