//! Attestation challenge nonce generation.
//!
//! Per D-13: every attestation challenge includes a fresh nonce.
//! Per ATST-06: nonces must be 32 bytes and unique per call.

use rand::Rng;

/// Generate a cryptographically secure 32-byte nonce for attestation challenges.
///
/// Per D-13: every attestation challenge includes a fresh nonce. The verifier
/// rejects reports where the embedded nonce does not match the challenge nonce.
///
/// Uses `rand::rng()` which internally uses the OS CSPRNG (getrandom).
#[allow(dead_code)]
pub fn generate_nonce() -> [u8; 32] {
    let mut nonce = [0u8; 32];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}
