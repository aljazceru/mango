//! REPORTDATA decoder + Venice attestation unit tests against golden capture.
//! Stubs: each test calls a not-yet-implemented function; will be filled in by Plan 02.
//! All tests are `#[ignore]`-gated so default `cargo test` stays GREEN during Wave 0.

#![allow(unused_imports)]

use crate::tests::common::venice_fixtures::*;

#[test]
#[ignore = "RED — Plan 02 implements verify_venice_report_data (VEN-04a)"]
fn reportdata_layout_ok() {
    // Will call: crate::attestation::venice::verify_venice_report_data(&rd, pk, &nonce)
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (VEN-04b)"]
fn reportdata_address_mismatch() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (VEN-04c)"]
fn reportdata_nonce_mismatch() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 02 (VEN-04d)"]
fn reportdata_padding_nonzero() {
    panic!("not yet implemented");
}

#[tokio::test]
#[ignore = "RED — Plan 02 (VEN-06)"]
async fn tdx_debug_bit_rejected() {
    panic!("not yet implemented");
}

#[tokio::test]
#[ignore = "RED — Plan 02 (VEN-03)"]
async fn tdx_verify_golden_capture_signature() {
    panic!("not yet implemented");
}
