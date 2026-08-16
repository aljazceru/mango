//! Regression test: desktop-new-chat-db-panic.
//!
//! Reproduces the panic reported at rust/src/lib.rs:4503:61 ("db unlocked")
//! when the UI dispatches AppAction::NewConversation while the actor is in
//! Case D (returning user with auth configured, DB not yet unlocked).
//!
//! Pre-fix: the NewConversation handler unconditionally calls
//! `actor_state.db.as_ref().expect("db unlocked").conn()`, which panics.
//!
//! Post-fix: the handler must no-op (ideally with a toast or log) when
//! `actor_state.db` is None instead of panicking, so a stray UI dispatch
//! while the lock screen is shown cannot take the whole process down.

use crate::{
    crypto, AppAction, EmbeddingStatus, FfiApp, NullBiometricProvider, NullEmbeddingProvider,
    NullKeychainProvider, Screen,
};

/// Create a filesystem-backed data dir with bootstrap auth_params already
/// populated so FfiApp::new goes down the Case D (locked) path.
fn make_locked_data_dir(tag: &str) -> String {
    let base = std::env::temp_dir().join(format!("mango_locked_{}_{}", tag, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).expect("create test data dir");

    // Write auth_params into mango_auth.db so has_auth_params() returns true.
    // The wrapped_dek won't be used unless we try to unlock; we just need the
    // row to exist so FfiApp::new takes the "Case D deferred" branch and
    // leaves actor_state.db as None.
    let bootstrap_path = base.join("mango_auth.db");
    let bootstrap = crypto::bootstrap_db::BootstrapDb::open(bootstrap_path.to_str().unwrap())
        .expect("open bootstrap db");
    // Derive KEK from a dummy PIN to generate a real wrapped_dek (so the
    // cryptographic shape of auth_params is realistic; content doesn't matter
    // for this test because we never attempt to unlock).
    let salt = [0u8; 32];
    let kek = crypto::key_derivation::derive_kek(
        b"000000",
        &salt,
        crypto::key_derivation::DEFAULT_MEMORY_KIB,
        crypto::key_derivation::DEFAULT_ITERATIONS,
        crypto::key_derivation::DEFAULT_PARALLELISM,
    )
    .expect("derive_kek");
    let dek = [7u8; 32];
    let wrapped_dek = crypto::key_derivation::wrap_dek(&kek, &dek);
    bootstrap
        .write_auth_params(&crypto::bootstrap_db::AuthParams {
            salt: salt.to_vec(),
            wrapped_dek,
            duress_hash: None,
            kdf_memory_kib: crypto::key_derivation::DEFAULT_MEMORY_KIB,
            kdf_iterations: crypto::key_derivation::DEFAULT_ITERATIONS,
            kdf_parallelism: crypto::key_derivation::DEFAULT_PARALLELISM,
        })
        .expect("write auth params");

    base.to_string_lossy().to_string()
}

/// Returning-user app with no unlocked DB. FfiApp::new should settle on
/// Screen::Locked with `actor_state.db = None`.
fn make_locked_app(data_dir: String) -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        data_dir,
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    app
}

#[test]
fn returning_user_starts_on_lock_screen_with_no_db() {
    // Sanity check the preconditions of the regression test below: a
    // returning user with auth configured must land on Screen::Locked.
    let data_dir = make_locked_data_dir("sanity");
    let app = make_locked_app(data_dir);
    let state = app.state();
    assert!(
        matches!(state.router.current_screen, Screen::Locked),
        "returning user should land on Screen::Locked, got: {:?}",
        state.router.current_screen
    );
    assert!(
        state.auth_initialized,
        "auth_initialized should be true for a returning user"
    );
    assert!(
        state.encryption_enabled,
        "encryption_enabled should be true for a returning user"
    );
}

#[test]
fn new_conversation_while_locked_does_not_panic() {
    // Regression: pre-fix this dispatched NewConversation would panic in the
    // actor thread with 'db unlocked' at lib.rs:4503. Post-fix the actor must
    // ignore NewConversation (or surface a soft error) and keep running.
    let data_dir = make_locked_data_dir("new_conv_locked");
    let app = make_locked_app(data_dir);

    // Precondition: screen is Locked (no unlock has happened).
    assert!(
        matches!(app.state().router.current_screen, Screen::Locked),
        "precondition: lock screen before NewConversation"
    );

    // Dispatching NewConversation while locked must NOT take down the actor.
    // Pre-fix this hit `actor_state.db.as_ref().expect("db unlocked")` and
    // killed the actor thread. We detect actor death by sending a Noop probe
    // afterwards: every action handler unconditionally bumps `rev` at the end
    // of the match arm, so if the actor is alive, rev will increase.
    let rev_before_new_conv = app.state().rev;
    app.dispatch(AppAction::NewConversation);
    app.sync();

    let state_after_new_conv = app.state();
    assert!(
        matches!(state_after_new_conv.router.current_screen, Screen::Locked),
        "screen should remain Locked after no-op NewConversation, got: {:?}",
        state_after_new_conv.router.current_screen
    );
    assert_eq!(
        state_after_new_conv.conversations.len(),
        0,
        "no conversation should be created while DB is locked"
    );

    // Liveness probe: Noop bumps rev at the end of the action match. If the
    // actor thread panicked on NewConversation, rev stays frozen and this
    // assertion fails — which is exactly the pre-fix behaviour.
    app.dispatch(AppAction::Noop);
    app.sync();
    let state_probe = app.state();
    assert!(
        state_probe.rev > rev_before_new_conv,
        "actor thread appears dead after NewConversation: rev stuck at {} (pre_rev was {})",
        state_probe.rev,
        rev_before_new_conv,
    );
}
