//! Integration tests for the attestation actor flow.
//!
//! Tests verify that AttestationEvent results flow from task injection
//! through the actor loop into AppState.attestation_statuses, and that
//! raw report blobs are retrievable via get_raw_attestation_report.
//!
//! Uses the same test_send_internal() pattern as streaming.rs -- no real
//! network calls are made in these tests.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::attestation::AttestationEvent;
use crate::llm::streaming::InternalEvent;
use crate::{AttestationStatus, EmbeddingStatus, FfiApp};

/// Create an FfiApp with empty data_dir (uses :memory: SQLite cache) and
/// give the actor thread 50ms to initialize.
fn create_test_app() -> Arc<FfiApp> {
    let app = FfiApp::new(
        String::new(),
        Box::new(crate::NullKeychainProvider),
        Box::new(crate::NullEmbeddingProvider),
        EmbeddingStatus::Active,
    );
    std::thread::sleep(Duration::from_millis(50));
    app
}

/// Wait for the actor to process injected events.
fn wait_for_update() {
    std::thread::sleep(Duration::from_millis(100));
}

/// Current Unix timestamp in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[test]
fn test_initial_attestation_statuses() {
    let app = create_test_app();
    let state = app.state();
    assert!(
        state.attestation_statuses.len() <= 2,
        "Expected at most 2 initial attestation entries, got {}",
        state.attestation_statuses.len()
    );
}

#[test]
fn test_attestation_verified_event_updates_state() {
    let app = create_test_app();
    let now = now_secs();

    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "test_be".to_string(),
            tee_type: "IntelTdx".to_string(),
            report_blob: vec![1, 2, 3],
            expires_at: now + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
        },
    ));
    wait_for_update();

    let state = app.state();
    let entry = state
        .attestation_statuses
        .iter()
        .find(|e| e.backend_id == "test_be");
    assert!(entry.is_some(), "Expected attestation entry for 'test_be'");
    assert_eq!(
        entry.unwrap().status,
        AttestationStatus::Verified,
        "Expected Verified status"
    );
}

#[test]
fn test_attestation_failed_event_updates_state() {
    let app = create_test_app();

    app.test_send_internal(InternalEvent::AttestationResult(AttestationEvent::Failed {
        backend_id: "fail_be".to_string(),
        reason: "bad quote".to_string(),
        is_transient: false,
    }));
    wait_for_update();

    let state = app.state();
    let entry = state
        .attestation_statuses
        .iter()
        .find(|e| e.backend_id == "fail_be");
    assert!(entry.is_some(), "Expected attestation entry for 'fail_be'");
    assert_eq!(
        entry.unwrap().status,
        AttestationStatus::Failed {
            reason: "bad quote".to_string(),
        },
        "Expected Failed status with matching reason"
    );
}

#[test]
fn test_attestation_nvidia_verified_event() {
    let app = create_test_app();
    let now = now_secs();

    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "pv_be".to_string(),
            tee_type: "NvidiaH100Cc".to_string(),
            report_blob: vec![4, 5, 6],
            expires_at: now + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
        },
    ));
    wait_for_update();

    let state = app.state();
    let entry = state
        .attestation_statuses
        .iter()
        .find(|e| e.backend_id == "pv_be");
    assert!(entry.is_some(), "Expected attestation entry for 'pv_be'");
    assert_eq!(
        entry.unwrap().status,
        AttestationStatus::Verified,
        "Expected Verified status for NVIDIA H100 CC backend"
    );
}

#[test]
fn test_raw_report_retrieval() {
    let app = create_test_app();
    let now = now_secs();
    let expected_blob = vec![10u8, 20, 30, 40];

    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "report_be".to_string(),
            tee_type: "IntelTdx".to_string(),
            report_blob: expected_blob.clone(),
            expires_at: now + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
        },
    ));
    wait_for_update();

    let retrieved = app.get_raw_attestation_report("report_be".to_string());
    assert_eq!(
        retrieved,
        Some(expected_blob),
        "get_raw_attestation_report should return the injected blob"
    );
}

#[test]
fn test_raw_report_nonexistent() {
    let app = create_test_app();
    let retrieved = app.get_raw_attestation_report("nonexistent".to_string());
    assert_eq!(
        retrieved, None,
        "get_raw_attestation_report should return None for unknown backend_id"
    );
}

/// Verify that a Verified status is sticky: a subsequent Failed event (e.g. a transient
/// 429 from AMD KDS during periodic re-attestation) must NOT downgrade a Verified backend.
#[test]
fn test_attestation_verified_is_sticky_against_transient_failure() {
    let app = create_test_app();
    let now = now_secs();

    // First: verify the backend.
    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "sticky_be".to_string(),
            tee_type: "AmdSevSnp".to_string(),
            report_blob: vec![1],
            expires_at: now + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
        },
    ));
    wait_for_update();

    // Then: simulate a transient 429 / network failure on re-attestation.
    // is_transient=true because this was a CollateralFetch/NetworkError (never reached the TEE).
    app.test_send_internal(InternalEvent::AttestationResult(AttestationEvent::Failed {
        backend_id: "sticky_be".to_string(),
        reason: "AMD KDS returned HTTP 429 Too Many Requests".to_string(),
        is_transient: true,
    }));
    wait_for_update();

    let state = app.state();
    let entries: Vec<_> = state
        .attestation_statuses
        .iter()
        .filter(|e| e.backend_id == "sticky_be")
        .collect();

    assert_eq!(
        entries.len(),
        1,
        "Should have exactly one entry per backend_id"
    );
    assert_eq!(
        entries[0].status,
        AttestationStatus::Verified,
        "Verified status must NOT be downgraded by a transient Failed event"
    );
}

/// Verify that a genuine cryptographic verification failure (is_transient=false) DOES downgrade
/// a previously-Verified status.  This is the mirror of the stickiness test: only transient
/// errors (network, 429, collateral fetch) preserve Verified; a real bad-signature or
/// measurement-mismatch result must update the status even when prior state was Verified.
#[test]
fn test_attestation_genuine_failure_downgrades_verified() {
    let app = create_test_app();
    let now = now_secs();

    // First: verify the backend.
    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "genuine_fail_be".to_string(),
            tee_type: "IntelTdx".to_string(),
            report_blob: vec![1],
            expires_at: now + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
        },
    ));
    wait_for_update();

    // Then: simulate a genuine cryptographic verification failure (bad signature,
    // measurement mismatch, etc.) — is_transient=false.
    app.test_send_internal(InternalEvent::AttestationResult(AttestationEvent::Failed {
        backend_id: "genuine_fail_be".to_string(),
        reason: "TDX quote verification failed: signature mismatch".to_string(),
        is_transient: false,
    }));
    wait_for_update();

    let state = app.state();
    let entry = state
        .attestation_statuses
        .iter()
        .find(|e| e.backend_id == "genuine_fail_be");

    assert!(
        entry.is_some(),
        "Expected attestation entry for 'genuine_fail_be'"
    );
    assert!(
        matches!(entry.unwrap().status, AttestationStatus::Failed { .. }),
        "Genuine verification failure must downgrade status from Verified to Failed, got: {:?}",
        entry.unwrap().status
    );
}

/// Verify that a Failed status IS replaced when a re-verification produces a new Verified result.
#[test]
fn test_attestation_failed_replaced_by_verified() {
    let app = create_test_app();
    let now = now_secs();

    // First: fail (genuine verification failure, not transient).
    app.test_send_internal(InternalEvent::AttestationResult(AttestationEvent::Failed {
        backend_id: "recover_be".to_string(),
        reason: "initial error".to_string(),
        is_transient: false,
    }));
    wait_for_update();

    // Then: verify successfully.
    app.test_send_internal(InternalEvent::AttestationResult(
        AttestationEvent::Verified {
            backend_id: "recover_be".to_string(),
            tee_type: "IntelTdx".to_string(),
            report_blob: vec![2],
            expires_at: now + 3600,
            tls_public_key_fp: None,
            vcek_url: None,
            vcek_der: None,
        },
    ));
    wait_for_update();

    let state = app.state();
    let entry = state
        .attestation_statuses
        .iter()
        .find(|e| e.backend_id == "recover_be");
    assert!(entry.is_some(), "Entry should exist for recover_be");
    assert_eq!(
        entry.unwrap().status,
        AttestationStatus::Verified,
        "Failed should be replaced when fresh Verified arrives"
    );
}

/// Verify that a non-Verified status is correctly upserted by another non-Verified status.
#[test]
fn test_attestation_status_upsert_non_verified() {
    let app = create_test_app();

    // Start unverified (no entry).
    // Send a first Failed (genuine verification failure).
    app.test_send_internal(InternalEvent::AttestationResult(AttestationEvent::Failed {
        backend_id: "upsert_be".to_string(),
        reason: "first error".to_string(),
        is_transient: false,
    }));
    wait_for_update();

    // Send a second Failed with a different reason — should replace.
    app.test_send_internal(InternalEvent::AttestationResult(AttestationEvent::Failed {
        backend_id: "upsert_be".to_string(),
        reason: "second error".to_string(),
        is_transient: false,
    }));
    wait_for_update();

    let state = app.state();
    let entries: Vec<_> = state
        .attestation_statuses
        .iter()
        .filter(|e| e.backend_id == "upsert_be")
        .collect();

    assert_eq!(
        entries.len(),
        1,
        "Should have exactly one entry per backend_id (upsert)"
    );
    assert_eq!(
        entries[0].status,
        AttestationStatus::Failed {
            reason: "second error".to_string(),
        },
        "Non-Verified status should be updated by the latest Failed"
    );
}

/// Verify that spawning an attestation task for an unreachable backend
/// results in an AttestationEvent::Failed being sent back.
///
/// Uses 127.0.0.1:1 which is guaranteed unreachable. Covers ATST-07:
/// when self-verification fails due to network error, the flow produces
/// a Failed event rather than panicking or hanging.
///
/// Note: runs in a dedicated thread to avoid nested-runtime issues when
/// running under the test harness.
#[test]
fn test_provider_fallback() {
    use crate::llm::backend::BackendConfig;
    use crate::llm::TeeType;

    // Run the test body in a dedicated OS thread with its own Tokio runtime
    // to avoid the "cannot drop runtime in async context" panic.
    let result = std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_time()
            .enable_io()
            .build()
            .expect("tokio runtime for test");

        let (tx, rx) = flume::unbounded::<crate::CoreMsg>();

        let backend = BackendConfig {
            id: "unreachable_backend".to_string(),
            name: "Unreachable".to_string(),
            base_url: "http://127.0.0.1:1/v1/".to_string(),
            api_key: String::new(),
            models: vec![],
            tee_type: TeeType::IntelTdx,
            max_concurrent_requests: 5,
            supports_tool_use: true,
        };

        let vcek_cache = std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::<
            String,
            Vec<u8>,
        >::new()));
        crate::attestation::spawn_attestation_task(&runtime, &backend, tx, vcek_cache, crate::attestation::TeePolicy::default());

        // Poll for up to 10 seconds for the task result
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            match rx.try_recv() {
                Ok(msg) => return Ok(msg),
                Err(_) => {
                    if std::time::Instant::now() >= deadline {
                        return Err("Timed out waiting for attestation result".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    })
    .join()
    .expect("Test thread panicked");

    let msg = result.expect("Attestation task did not produce a result within 10 seconds");

    match msg {
        crate::CoreMsg::InternalEvent(event) => match *event {
            InternalEvent::AttestationResult(AttestationEvent::Failed {
                backend_id,
                reason,
                ..
            }) => {
                assert_eq!(backend_id, "unreachable_backend");
                let reason_lower = reason.to_lowercase();
                assert!(
                    reason_lower.contains("network")
                        || reason_lower.contains("error")
                        || reason_lower.contains("connect")
                        || reason_lower.contains("failed")
                        || reason_lower.contains("refused"),
                    "Expected network/error reason, got: {}",
                    reason
                );
            }
            other => panic!("Expected AttestationResult(Failed), got: {:?}", other),
        },
        crate::CoreMsg::Action(_) => panic!("Expected InternalEvent, got Action"),
    }
}
