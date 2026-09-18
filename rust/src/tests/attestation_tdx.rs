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
