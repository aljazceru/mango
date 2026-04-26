//! Venice provider transport unit tests.
//! All tests `#[ignore]`-gated until Plan 03/04 implement `crate::llm::venice::*`.

#![allow(unused_imports)]

use crate::tests::common::venice_fixtures::*;

#[test]
#[ignore = "RED — Plan 04 (VEN-01) backend preset"]
fn venice_preset_present() {
    // Will assert: crate::llm::backend::known_provider_presets() contains id == "venice-ai"
    panic!("not yet implemented");
}

#[test]
fn attestation_url_format() {
    use crate::llm::venice::format_attestation_url;
    let url = format_attestation_url(
        "e2ee-venice-uncensored-24b-p",
        "abc123",
        "https://api.venice.ai",
    );
    assert!(
        url.ends_with(
            "/api/v1/tee/attestation?model=e2ee-venice-uncensored-24b-p&nonce=abc123"
        ),
        "unexpected URL: {url}"
    );
    assert!(url.starts_with("https://api.venice.ai"));

    // Also accepts a base_url that already includes /api/v1.
    let url2 = format_attestation_url(
        "e2ee-venice-uncensored-24b-p",
        "abc123",
        "https://api.venice.ai/api/v1/",
    );
    assert!(url2.ends_with(
        "/api/v1/tee/attestation?model=e2ee-venice-uncensored-24b-p&nonce=abc123"
    ));
}

#[test]
#[ignore = "RED — Plan 02 (VEN-05) NRAS payload double-parse"]
fn nvidia_payload_double_parse() {
    panic!("not yet implemented");
}

#[test]
fn ecdh_aes_round_trip() {
    use crate::llm::venice::{derive_session_key, open_envelope, seal_message};
    use k256::ecdh::{diffie_hellman, EphemeralSecret};
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    use k256::{PublicKey, SecretKey};
    use rand::thread_rng;

    // Server-side: simulated signing keypair
    let server_secret = SecretKey::random(&mut thread_rng());
    let server_pub_point = server_secret.public_key().to_encoded_point(false);
    let mut server_pub_65 = [0u8; 65];
    server_pub_65.copy_from_slice(server_pub_point.as_bytes());

    // Client-side: ephemeral + ECDH
    let eph_secret = EphemeralSecret::random(&mut thread_rng());
    let eph_pub_point = eph_secret.public_key().to_encoded_point(false);
    let mut eph_pub_65 = [0u8; 65];
    eph_pub_65.copy_from_slice(eph_pub_point.as_bytes());

    let client_aes = derive_session_key(&eph_secret, &server_pub_65).unwrap();

    // Server side derives the same AES key via ECDH against the eph pub.
    let eph_pub_for_server = PublicKey::from_sec1_bytes(&eph_pub_65).unwrap();
    let server_shared = diffie_hellman(
        server_secret.to_nonzero_scalar(),
        eph_pub_for_server.as_affine(),
    );
    let mut server_aes = [0u8; 32];
    hkdf::Hkdf::<sha2::Sha256>::new(None, server_shared.raw_secret_bytes())
        .expand(b"ecdsa_encryption", &mut server_aes)
        .unwrap();

    assert_eq!(client_aes, server_aes, "ECDH+HKDF must agree on both sides");

    // Round-trip an arbitrary plaintext.
    let plaintext = b"hello venice e2ee";
    let env = seal_message(plaintext, &client_aes, &eph_pub_65).unwrap();
    let recovered = open_envelope(&env, &server_aes).unwrap();
    assert_eq!(recovered, plaintext);

    // Two ephemerals against the same server pub produce DIFFERENT AES keys
    // (per-request randomness — VEN-07).
    let other_eph = EphemeralSecret::random(&mut thread_rng());
    let other_aes = derive_session_key(&other_eph, &server_pub_65).unwrap();
    assert_ne!(client_aes, other_aes, "two ephemerals must derive different AES keys");
}

#[test]
fn envelope_round_trip() {
    use crate::llm::venice::{open_envelope, seal_message};

    let key = [0x42u8; 32];
    let eph_pub = [0u8; 65];
    let pt = b"payload";

    // Pitfall 7: fresh nonce per call — same key + same plaintext must produce
    // distinct envelopes.
    let e1 = seal_message(pt, &key, &eph_pub).unwrap();
    let e2 = seal_message(pt, &key, &eph_pub).unwrap();
    assert_ne!(e1, e2, "fresh nonce per call (Pitfall 7) — envelopes must differ");

    // Both envelopes round-trip with the same key.
    assert_eq!(open_envelope(&e1, &key).unwrap(), pt);
    assert_eq!(open_envelope(&e2, &key).unwrap(), pt);

    // Wrong key fails closed (T-33-13).
    let wrong = [0x99u8; 32];
    assert!(open_envelope(&e1, &wrong).is_err());

    // Truncated envelope rejected.
    let short = hex::encode([0u8; 65 + 12 + 1]);
    assert!(open_envelope(&short, &key).is_err());
}

#[test]
#[ignore = "RED — Plan 03 (VEN-08) request body shape"]
fn request_body_shape() {
    panic!("not yet implemented");
}

#[test]
#[ignore = "RED — Plan 04 (VEN-09) backend summary"]
fn backend_summary_after_add() {
    panic!("not yet implemented");
}
