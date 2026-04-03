//! Unit tests for attestation types and errors.
//! Covers ATST-05 (status enum).

use crate::attestation::{AttestationError, AttestationEvent, AttestationRecord, AttestationStatus};

#[test]
fn test_attestation_status_variants() {
    // Construct all 4 variants
    let verified = AttestationStatus::Verified;
    let unverified = AttestationStatus::Unverified;
    let failed = AttestationStatus::Failed {
        reason: "cert expired".to_string(),
    };
    let expired = AttestationStatus::Expired;

    // Clone should not panic
    let _ = verified.clone();
    let _ = unverified.clone();
    let _ = failed.clone();
    let _ = expired.clone();

    // PartialEq
    assert_eq!(AttestationStatus::Verified, AttestationStatus::Verified);
    assert_eq!(AttestationStatus::Unverified, AttestationStatus::Unverified);
    assert_eq!(
        AttestationStatus::Failed {
            reason: "cert expired".to_string()
        },
        AttestationStatus::Failed {
            reason: "cert expired".to_string()
        }
    );
    assert_eq!(AttestationStatus::Expired, AttestationStatus::Expired);
    assert_ne!(AttestationStatus::Verified, AttestationStatus::Expired);
}

#[test]
fn test_attestation_error_display() {
    let variants: Vec<AttestationError> = vec![
        AttestationError::CollateralFetch {
            reason: "HTTP 403".to_string(),
        },
        AttestationError::QuoteVerification {
            reason: "bad header magic".to_string(),
        },
        AttestationError::NonceMismatch {
            expected: "aabbcc".to_string(),
            actual: "ddeeff".to_string(),
        },
        AttestationError::JwtVerification {
            reason: "signature invalid".to_string(),
        },
        AttestationError::CacheFailed {
            reason: "disk full".to_string(),
        },
        AttestationError::Unsupported,
        AttestationError::NetworkError {
            reason: "connection refused".to_string(),
        },
    ];

    for err in &variants {
        let msg = err.display_message();
        assert!(
            !msg.is_empty(),
            "display_message() should be non-empty for {:?}",
            err
        );
        // Also verify the standard Display impl (from thiserror) is non-empty
        assert!(!format!("{}", err).is_empty());
    }
}

/// Verify that AttestationError::is_transient() correctly classifies network/fetch errors
/// (transient) vs genuine verification errors (non-transient).
#[test]
fn test_attestation_error_is_transient() {
    // Transient errors: network problems prevent us from even attempting verification.
    assert!(
        AttestationError::NetworkError {
            reason: "connection refused".to_string()
        }
        .is_transient(),
        "NetworkError must be transient"
    );
    assert!(
        AttestationError::CollateralFetch {
            reason: "HTTP 429".to_string()
        }
        .is_transient(),
        "CollateralFetch must be transient"
    );

    // Non-transient errors: the TEE report was reachable and inspected but found invalid.
    assert!(
        !AttestationError::QuoteVerification {
            reason: "signature mismatch".to_string()
        }
        .is_transient(),
        "QuoteVerification must NOT be transient"
    );
    assert!(
        !AttestationError::NonceMismatch {
            expected: "aabb".to_string(),
            actual: "ccdd".to_string()
        }
        .is_transient(),
        "NonceMismatch must NOT be transient"
    );
    assert!(
        !AttestationError::JwtVerification {
            reason: "invalid signature".to_string()
        }
        .is_transient(),
        "JwtVerification must NOT be transient"
    );
    assert!(
        !AttestationError::CacheFailed {
            reason: "disk full".to_string()
        }
        .is_transient(),
        "CacheFailed must NOT be transient"
    );
    assert!(
        !AttestationError::Unsupported.is_transient(),
        "Unsupported must NOT be transient"
    );
}

#[test]
fn test_attestation_record_construction() {
    let record = AttestationRecord {
        backend_id: "tinfoil".to_string(),
        tee_type: "IntelTdx".to_string(),
        status: AttestationStatus::Verified,
        report_blob: vec![0x01, 0x02, 0x03],
        verified_at: 1_700_000_000,
        expires_at: 1_700_014_400, // +4 hours
    };

    assert_eq!(record.backend_id, "tinfoil");
    assert_eq!(record.tee_type, "IntelTdx");
    assert_eq!(record.status, AttestationStatus::Verified);
    assert_eq!(record.report_blob, vec![0x01, 0x02, 0x03]);
    assert_eq!(record.verified_at, 1_700_000_000);
    assert_eq!(record.expires_at, 1_700_014_400);
}

#[test]
fn test_attestation_event_variants() {
    let verified = AttestationEvent::Verified {
        backend_id: "tinfoil".to_string(),
        tee_type: "IntelTdx".to_string(),
        report_blob: vec![0xDE, 0xAD],
        expires_at: 1_700_014_400,
        tls_public_key_fp: None,
        vcek_url: None,
        vcek_der: None,
    };

    let nvidia_verified = AttestationEvent::Verified {
        backend_id: "test-backend".to_string(),
        tee_type: "NvidiaH100Cc".to_string(),
        report_blob: vec![0xBE, 0xEF],
        expires_at: 1_700_003_600,
        tls_public_key_fp: None,
        vcek_url: None,
        vcek_der: None,
    };

    let failed = AttestationEvent::Failed {
        backend_id: "tinfoil".to_string(),
        reason: "PCS unreachable".to_string(),
        is_transient: true,
    };

    // All variants must be constructable without panic
    // Use Debug to exercise the derived impl
    assert!(format!("{:?}", verified).contains("Verified"));
    assert!(format!("{:?}", nvidia_verified).contains("Verified"));
    assert!(format!("{:?}", failed).contains("Failed"));
}
