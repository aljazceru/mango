/// No-lock enrollment (opt-out of mandatory PIN/biometric login).
///
/// File-backed tests using a shared in-memory keychain so cold starts (a new
/// FfiApp over the same dir) see the cached DEK exactly like a real OS
/// keychain:
/// - SetupNoLock completes first-run enrollment and creates the conversation.
/// - Cold start bypasses the lock screen.
/// - EnablePinLock re-enables a PIN lock; the next cold start locks again and
///   the new PIN unlocks.
/// - SetupNoLock is rejected on an already-enrolled install; EnablePinLock is
///   rejected outside no-lock mode; LockApp is skipped in no-lock mode.
/// - An interrupted no-lock enrollment (pending auth + keychain secret)
///   auto-resumes at startup.
use crate::crypto::bootstrap_db::{AuthParams, BootstrapDb};
use crate::crypto::key_derivation::{
    derive_kek, generate_dek, generate_salt, wrap_dek, DEFAULT_ITERATIONS, DEFAULT_MEMORY_KIB,
    DEFAULT_PARALLELISM,
};
use crate::{
    AppAction, EmbeddingStatus, FfiApp, KeychainProvider, NullBiometricProvider,
    NullEmbeddingProvider, NullLocalLlmProvider, Screen,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct SharedKeychainInner(Arc<Mutex<HashMap<(String, String), String>>>);

struct SharedKeychain(SharedKeychainInner);

impl KeychainProvider for SharedKeychain {
    fn store(&self, service: String, key: String, value: String) -> bool {
        self.0
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert((service, key), value);
        true
    }
    fn load(&self, service: String, key: String) -> Option<String> {
        self.0
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&(service, key))
            .cloned()
    }
    fn delete(&self, service: String, key: String) -> bool {
        self.0
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&(service, key));
        true
    }
}

fn temp_dir(tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "mango_nolock_test_{}_{}",
        tag,
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir.to_str().unwrap().to_string()
}

fn make_app(dir: &str, keychain: &SharedKeychainInner) -> Arc<FfiApp> {
    let app = FfiApp::new(
        dir.into(),
        Box::new(SharedKeychain(keychain.clone())),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    app
}

fn reach_pin_setup(app: &Arc<FfiApp>) {
    for _ in 0..4 {
        app.dispatch(AppAction::NextOnboardingStep);
        app.sync();
    }
    app.dispatch(AppAction::CompleteOnboarding);
    app.sync();
    assert!(matches!(
        app.state().router.current_screen,
        Screen::PinSetup
    ));
}

#[test]
fn setup_no_lock_completes_enrollment_and_creates_first_conversation() {
    let dir = temp_dir("nolock_enroll");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);

    app.dispatch(AppAction::SetupNoLock);
    app.sync();

    let state = app.state();
    assert!(
        !matches!(state.router.current_screen, Screen::PinSetup | Screen::Locked),
        "no-lock enrollment must leave the enrollment/lock screens, got {:?}",
        state.router.current_screen
    );
    assert!(state.auth_initialized, "auth must be committed");
    assert!(state.encryption_enabled, "DB must be encrypted");
    assert!(state.no_lock_mode, "no_lock_mode must be set");
    assert_eq!(
        state.conversations.len(),
        1,
        "first-run continuation must create the conversation exactly once"
    );
    assert_eq!(state.lock_timeout_seconds, -1, "auto-lock must be Never");
    assert!(main_db_is_encrypted(&dir), "main DB must be encrypted");
}

fn main_db_is_encrypted(dir: &str) -> bool {
    crate::persistence::Database::is_encrypted(&format!("{dir}/mango.db"))
}

#[test]
fn setup_no_lock_cold_start_bypasses_lock_screen() {
    let dir = temp_dir("nolock_coldstart");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupNoLock);
    app.sync();
    drop(app);

    let app2 = make_app(&dir, &keychain);
    let state = app2.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Locked),
        "cold start in no-lock mode must bypass the lock screen, got {:?}",
        state.router.current_screen
    );
    assert!(state.no_lock_mode, "no_lock_mode must persist across restart");
    assert_eq!(
        state.conversations.len(),
        1,
        "data must be readable after cold-start bypass"
    );
}

#[test]
fn enable_pin_lock_restores_lock_and_unlock_works() {
    let dir = temp_dir("nolock_enable_pin");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupNoLock);
    app.sync();

    app.dispatch(AppAction::EnablePinLock {
        pin: "1234".to_string(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(!state.no_lock_mode, "no_lock_mode must clear");
    assert!(state.auth_initialized);

    drop(app);
    let app2 = make_app(&dir, &keychain);
    assert!(
        matches!(app2.state().router.current_screen, Screen::Locked),
        "cold start after EnablePinLock must show the lock screen"
    );

    app2.dispatch(AppAction::UnlockWithPin {
        pin: "1234".to_string(),
    });
    app2.sync();
    let state = app2.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Locked),
        "the new PIN must unlock the vault"
    );
    assert_eq!(state.conversations.len(), 1, "data survives re-locking");
}

#[test]
fn setup_no_lock_rejected_when_auth_exists() {
    let dir = temp_dir("nolock_reject_existing");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupPin {
        pin: "1234".to_string(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    app.dispatch(AppAction::SetupNoLock);
    app.sync();

    let state = app.state();
    assert!(!state.no_lock_mode, "PIN install must not flip to no-lock");
    assert!(
        state.toast.as_deref().is_some_and(|t| t.contains("already")),
        "rejection must explain, got {:?}",
        state.toast
    );
}

#[test]
fn enable_pin_lock_rejected_outside_no_lock_mode() {
    let dir = temp_dir("nolock_enable_reject");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupPin {
        pin: "1234".to_string(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();
    app.dispatch(AppAction::UnlockWithPin {
        pin: "1234".to_string(),
    });
    app.sync();

    app.dispatch(AppAction::EnablePinLock {
        pin: "5678".to_string(),
        duress_pin: None,
        enable_biometric: false,
    });
    app.sync();

    let state = app.state();
    assert!(
        state.toast.as_deref().is_some_and(|t| t.contains("already")),
        "EnablePinLock on a PIN install must be rejected, got {:?}",
        state.toast
    );
    // The original PIN still unlocks after the rejected attempt.
    drop(app);
    let app2 = make_app(&dir, &keychain);
    app2.dispatch(AppAction::UnlockWithPin {
        pin: "1234".to_string(),
    });
    app2.sync();
    assert!(!matches!(
        app2.state().router.current_screen,
        Screen::Locked
    ));
}

#[test]
fn lock_app_is_skipped_in_no_lock_mode() {
    let dir = temp_dir("nolock_lockapp");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupNoLock);
    app.sync();

    app.dispatch(AppAction::LockApp);
    app.sync();

    let state = app.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Locked),
        "LockApp must not strand a no-lock install on the lock screen"
    );
}

#[test]
fn interrupted_no_lock_enrollment_auto_resumes_at_startup() {
    let dir = temp_dir("nolock_crash_resume");
    let keychain = SharedKeychainInner::default();
    // Start once so a plaintext mango.db exists, then simulate a crash after
    // pending auth was written but before promotion.
    let app = make_app(&dir, &keychain);
    drop(app);

    let dek = generate_dek();
    let salt = generate_salt();
    let secret = generate_dek();
    let kek = derive_kek(&secret, &salt, DEFAULT_MEMORY_KIB, DEFAULT_ITERATIONS, DEFAULT_PARALLELISM)
        .unwrap();
    let secret_hex: String = secret.iter().map(|b| format!("{:02x}", b)).collect();
    let params = AuthParams {
        salt: salt.to_vec(),
        wrapped_dek: wrap_dek(&kek, &dek),
        duress_hash: None,
        no_lock: true,
        kdf_memory_kib: DEFAULT_MEMORY_KIB,
        kdf_iterations: DEFAULT_ITERATIONS,
        kdf_parallelism: DEFAULT_PARALLELISM,
    };
    let bootstrap = BootstrapDb::open(&format!("{dir}/mango_auth.db")).unwrap();
    bootstrap.write_pending_auth(&params).unwrap();
    let kc = SharedKeychain(keychain.clone());
    kc.store("mango".into(), crate::KEYCHAIN_NO_LOCK_SECRET_KEY.into(), secret_hex);

    let app2 = make_app(&dir, &keychain);
    let state = app2.state();
    assert!(
        !matches!(state.router.current_screen, Screen::PinSetup | Screen::Locked),
        "no-lock resume needs no user input, got {:?}",
        state.router.current_screen
    );
    assert!(state.no_lock_mode, "resumed install is in no-lock mode");
    assert!(state.encryption_enabled, "resume must finish encryption");
    assert!(
        !bootstrap_has_pending(&dir),
        "pending auth must be promoted by the resume"
    );
}

#[test]
fn interrupted_no_lock_enrollment_survives_candidate_only_crash_window() {
    let dir = temp_dir("nolock_crash_candidate");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupNoLock);
    app.sync();
    drop(app);

    // Simulate a crash in the migration rename window: only an encrypted
    // candidate survives (no mango.db, no .plain_bak) and active auth was
    // never promoted.
    std::fs::rename(
        format!("{dir}/mango.db"),
        format!("{dir}/mango.db.enc_replaced"),
    )
    .unwrap();
    let bootstrap = BootstrapDb::open(&format!("{dir}/mango_auth.db")).unwrap();
    let params = bootstrap.read_auth_params().unwrap().expect("params");
    bootstrap.delete_all().unwrap();
    bootstrap.write_pending_auth(&params).unwrap();

    let app2 = make_app(&dir, &keychain);
    let state = app2.state();
    assert!(
        !matches!(state.router.current_screen, Screen::PinSetup | Screen::Locked),
        "candidate-window resume needs no user input, got {:?}",
        state.router.current_screen
    );
    assert!(state.no_lock_mode, "resumed install is in no-lock mode");
    assert!(
        std::path::Path::new(&format!("{dir}/mango.db")).exists(),
        "the encrypted candidate must be promoted back into place"
    );
}

#[test]
fn no_lock_resume_survives_second_restart_after_lost_dek_slot() {
    let dir = temp_dir("nolock_resume_second");
    let keychain = SharedKeychainInner::default();
    let app = make_app(&dir, &keychain);
    reach_pin_setup(&app);
    app.dispatch(AppAction::SetupNoLock);
    app.sync();
    drop(app);

    // Crash before promotion with the DEK slot wiped but the secret intact.
    let bootstrap = BootstrapDb::open(&format!("{dir}/mango_auth.db")).unwrap();
    let params = bootstrap.read_auth_params().unwrap().expect("params");
    bootstrap.delete_all().unwrap();
    bootstrap.write_pending_auth(&params).unwrap();
    let kc = SharedKeychain(keychain.clone());
    kc.delete("mango".into(), "dek".into());

    let app2 = make_app(&dir, &keychain);
    let state = app2.state();
    assert!(state.no_lock_mode, "resume must succeed from the secret alone");
    assert!(state.encryption_enabled, "resume must finish encryption");
    drop(app2);

    // Pending auth is gone now — the next cold start must bypass from the
    // (re-stored) DEK slot rather than dead-end on the lock screen.
    let app3 = make_app(&dir, &keychain);
    let state = app3.state();
    assert!(
        !matches!(state.router.current_screen, Screen::Locked),
        "second restart after resume must still bypass the lock screen"
    );
    assert!(state.no_lock_mode);
}

fn bootstrap_has_pending(dir: &str) -> bool {
    BootstrapDb::open(&format!("{dir}/mango_auth.db"))
        .unwrap()
        .has_pending_auth()
}
