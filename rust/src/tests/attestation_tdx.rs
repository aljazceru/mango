//! Unit tests for Intel TDX/DCAP quote verification.

use crate::attestation::tdx::{verify_tdx_quote, ReportDataLayout};

#[tokio::test]
async fn test_verify_tdx_quote_short_input() {
    // 10-byte input is below minimum -- should return error, not panic
    let short_bytes = vec![0u8; 10];
    let nonce = [0u8; 32];
    let result = verify_tdx_quote(
        &short_bytes,
        &nonce,
        "test-backend",
        ReportDataLayout::VeniceAddrPadNonce,
    )
    .await;
    assert!(
        result.is_err(),
        "short input should return an error, not succeed"
    );
}

#[test]
fn stale_tcb_statuses_are_rejected() {
    use crate::attestation::tdx::verify_acceptable_tcb_status;
    assert!(verify_acceptable_tcb_status("UpToDate").is_ok());
    assert!(verify_acceptable_tcb_status("SWHardeningNeeded").is_ok());
    for status in ["OutOfDate", "ConfigurationNeeded", "Revoked", "unknown"] {
        assert!(verify_acceptable_tcb_status(status).is_err(), "{status}");
    }
}
