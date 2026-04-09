/// Cryptographic primitives for local data encryption and authentication.
///
/// - `file_crypto`: AES-256-GCM file encryption with MGO1 magic header
/// - `key_derivation`: DEK generation, Argon2id KEK derivation, DEK wrap/unwrap, PIN hashing
/// - `bootstrap_db`: Unencrypted SQLite DB storing auth params (salt, wrapped DEK, KDF params)
pub mod bootstrap_db;
pub mod file_crypto;
pub mod key_derivation;
