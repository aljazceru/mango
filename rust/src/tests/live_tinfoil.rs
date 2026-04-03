//! Live integration tests for Tinfoil provider.
//!
//! These tests hit the real Tinfoil API and require credentials at
//! `~/.credentials/tinfoil.txt`. They are marked `#[ignore]` so they
//! never run in CI. Run explicitly with:
//!
//! ```
//! cargo test -p mango_core --lib -- live_tinfoil --ignored --nocapture
//! ```

use crate::attestation::AttestationStatus;
use crate::{AppAction, DesktopKeychainProvider, EmbeddingStatus, FfiApp, NullEmbeddingProvider};
use std::time::Duration;

const SERVICE: &str = "mango";
const TINFOIL_KEY_ID: &str = "tinfoil";
const CREDS_PATH: &str = "~/.credentials/tinfoil.txt";
/// Model available on Tinfoil inference enclave.
/// Must match the short IDs used by Tinfoil's API (see MIGRATION_V7 in schema.rs).
const TINFOIL_MODEL: &str = "llama3-3-70b";

fn read_api_key() -> String {
    let path = CREDS_PATH.replace('~', &std::env::var("HOME").unwrap_or_default());
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {}", path, e))
        .trim()
        .to_string()
}

fn seed_keychain(api_key: &str) {
    let kc = DesktopKeychainProvider;
    use crate::KeychainProvider;
    kc.store(
        SERVICE.to_string(),
        TINFOIL_KEY_ID.to_string(),
        api_key.to_string(),
    );
    let loaded = kc.load(SERVICE.to_string(), TINFOIL_KEY_ID.to_string());
    assert_eq!(
        loaded.as_deref(),
        Some(api_key),
        "Keychain round-trip failed — key was not persisted to SecretService"
    );
    eprintln!(
        "Keychain seeded: service='{}', key='{}'",
        SERVICE, TINFOIL_KEY_ID
    );
}

fn make_app_with_real_keychain() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".to_string(),
        Box::new(DesktopKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    // Allow actor init + attestation task to start.
    std::thread::sleep(Duration::from_millis(300));
    app
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Seeds the Tinfoil API key into the OS keychain so the app (and subsequent
/// tests in this module) can load it at runtime.
#[test]
#[ignore]
fn live_seed_keychain() {
    let key = read_api_key();
    seed_keychain(&key);
    eprintln!(
        "OK: Tinfoil API key stored in OS keychain ({} chars)",
        key.len()
    );
}

/// Verifies the Tinfoil API key is already in the keychain (run after live_seed_keychain).
#[test]
#[ignore]
fn live_verify_keychain_readable() {
    use crate::KeychainProvider;
    let kc = DesktopKeychainProvider;
    let loaded = kc.load(SERVICE.to_string(), TINFOIL_KEY_ID.to_string());
    assert!(
        loaded.is_some() && !loaded.as_deref().unwrap_or("").is_empty(),
        "Keychain returned empty for (service='{}', key='{}'). \
         Run live_seed_keychain first.",
        SERVICE,
        TINFOIL_KEY_ID
    );
    eprintln!("OK: keychain load returned {} chars", loaded.unwrap().len());
}

/// Sends a real streaming request to Tinfoil over the secure EHBP transport and
/// waits for a complete response.
///
/// Auth: standard Bearer token via api_key in BackendConfig (loaded from OS keychain).
/// Request path: https://inference.tinfoil.sh/v1/chat/completions.
/// Attestation bundle: https://atc.tinfoil.sh/attestation for the enclave origin.
#[test]
#[ignore]
fn live_streaming_tinfoil() {
    let key = read_api_key();
    seed_keychain(&key);

    let app = make_app_with_real_keychain();

    // Activate Tinfoil.
    app.dispatch(AppAction::SetActiveBackend {
        backend_id: TINFOIL_KEY_ID.to_string(),
    });
    std::thread::sleep(Duration::from_millis(100));

    let state = app.state();
    assert_eq!(
        state.active_backend_id.as_deref(),
        Some(TINFOIL_KEY_ID),
        "Active backend did not switch to tinfoil"
    );

    app.dispatch(AppAction::NewConversation);
    std::thread::sleep(Duration::from_millis(100));
    app.dispatch(AppAction::SelectModel {
        model_id: TINFOIL_MODEL.to_string(),
    });
    std::thread::sleep(Duration::from_millis(50));

    app.dispatch(AppAction::SendMessage {
        text: "Reply with exactly three words: yes I work".to_string(),
    });

    std::thread::sleep(Duration::from_millis(600));

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let state = app.state();
        let has_assistant = state
            .messages
            .iter()
            .any(|m| m.role == "assistant" && !m.content.is_empty());
        if state.busy_state == crate::BusyState::Idle && has_assistant {
            let last = state
                .messages
                .iter()
                .filter(|m| m.role == "assistant")
                .last()
                .unwrap();
            eprintln!("OK: Tinfoil streaming response: {:?}", last.content);
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Timed out waiting for Tinfoil streaming response (idle={:?}, has_assistant={})",
            state.busy_state,
            has_assistant
        );
    }
}

/// Fetches Tinfoil's attestation bundle from ATC and checks it reaches a terminal status.
///
/// Tinfoil uses AMD SEV-SNP (sev-snp-guest/v2 format as of 2026-03-24).
/// The attestation task decodes the gzipped body, parses the SNP report,
/// verifies the certificate binding and HPKE key endpoint, and validates the
/// AMD certificate chain and report signature.
/// On success the status is Verified.
#[test]
#[ignore]
fn live_attestation_tinfoil() {
    let key = read_api_key();
    seed_keychain(&key);

    let app = make_app_with_real_keychain();
    app.dispatch(AppAction::SetActiveBackend {
        backend_id: TINFOIL_KEY_ID.to_string(),
    });
    std::thread::sleep(Duration::from_millis(200));

    // Wait up to 15s for the attestation task to complete.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let state = app.state();
        if let Some(entry) = state
            .attestation_statuses
            .iter()
            .find(|e| e.backend_id == TINFOIL_KEY_ID)
        {
            match &entry.status {
                AttestationStatus::Unverified => {
                    // Still pending — keep waiting.
                }
                AttestationStatus::Verified => {
                    eprintln!("OK: Tinfoil attestation VERIFIED (AMD SEV-SNP cryptographic verification passed)");
                    return;
                }
                AttestationStatus::Failed { reason } => {
                    panic!("Tinfoil attestation FAILED: {:?}", reason);
                }
                AttestationStatus::Expired => {
                    eprintln!("NOTE: Tinfoil attestation Expired (TTL elapsed)");
                    return;
                }
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Timed out waiting for Tinfoil attestation status after 15s — \
             entry not found or stuck at Unknown"
        );
    }
}

/// Comprehensive end-to-end test: seed keychain, send message via Tinfoil, verify attestation.
#[test]
#[ignore]
fn live_e2e_tinfoil() {
    // 1. Seed keychain.
    let key = read_api_key();
    seed_keychain(&key);
    eprintln!("Step 1/3: Keychain seeded");

    // 2. Streaming.
    let app = make_app_with_real_keychain();
    app.dispatch(AppAction::SetActiveBackend {
        backend_id: TINFOIL_KEY_ID.to_string(),
    });
    std::thread::sleep(Duration::from_millis(200));

    app.dispatch(AppAction::NewConversation);
    std::thread::sleep(Duration::from_millis(100));
    app.dispatch(AppAction::SelectModel {
        model_id: TINFOIL_MODEL.to_string(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.dispatch(AppAction::SendMessage {
        text: "Reply with exactly two words: it works".to_string(),
    });
    std::thread::sleep(Duration::from_millis(600));

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let state = app.state();
        let has_assistant = state
            .messages
            .iter()
            .any(|m| m.role == "assistant" && !m.content.is_empty());
        if state.busy_state == crate::BusyState::Idle && has_assistant {
            let last = state
                .messages
                .iter()
                .filter(|m| m.role == "assistant")
                .last()
                .unwrap();
            eprintln!("Step 2/3: Streaming OK — {:?}", last.content);
            break;
        }
        assert!(std::time::Instant::now() < deadline, "Streaming timed out");
    }

    // 3. Attestation.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let state = app.state();
        if let Some(entry) = state
            .attestation_statuses
            .iter()
            .find(|e| e.backend_id == TINFOIL_KEY_ID)
        {
            if !matches!(entry.status, AttestationStatus::Unverified) {
                eprintln!("Step 3/3: Attestation terminal status = {:?}", entry.status);
                assert!(
                    !matches!(entry.status, AttestationStatus::Failed { .. }),
                    "Tinfoil attestation must not fail in e2e test: {:?}",
                    entry.status
                );
                eprintln!("\nOK: live_e2e_tinfoil PASSED");
                return;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Attestation timed out"
        );
    }
}
