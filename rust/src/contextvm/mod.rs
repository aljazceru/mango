//! Phase 35 — contextvm-sdk integration.
//!
//! Pure-Rust Nostr-based tool discovery + invocation. All public functions
//! are async and intended to be called from the actor's tokio runtime via
//! `runtime.block_on(...)` in dispatch_tools, mirroring the existing
//! Brave/fetch_url pattern in agent::tools.
//!
//! Per RESEARCH §A, contextvm-sdk does NOT export a default relay list, so
//! we hardcode one here. CTX-07 is interpreted as: "contextvm-sdk-style
//! defaults (i.e., a curated list of well-known Nostr relays) plus
//! `wss://relay.nostr.net`".

pub mod discovery;
pub mod dispatch;
pub mod error;
pub mod invocation;
pub mod npub;

pub use discovery::DiscoveredTool;
pub use dispatch::{
    build_dispatch_map, descriptors_to_chat_tools, finalise_for_turn,
    ContextvmToolDescriptor, DESCRIPTION_CAP_CHARS,
};
pub use invocation::invoke_tool;

/// Curated default relay list. CTX-07 amendment: no upstream "defaults"
/// constant exists in contextvm-sdk 0.1.0, so we ship our own.
pub const DEFAULT_CONTEXTVM_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.net",
];

/// Materialise the const slice into the `Vec<String>` shape the
/// contextvm-sdk APIs accept.
pub fn default_relays_owned() -> Vec<String> {
    DEFAULT_CONTEXTVM_RELAYS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// Install the rustls default `CryptoProvider` exactly once for the
/// process. contextvm-sdk → nostr-relay-pool → tokio-tungstenite pulls in
/// rustls 0.23 transitively, but tokio-tungstenite does not enable any
/// crypto-provider feature itself. With both `aws-lc-rs` and `ring`
/// disabled (or both enabled) at the leaf, rustls aborts the process the
/// first time a TLS client config is built, with the message:
///
///   "Could not automatically determine the process-level CryptoProvider
///    from Rustls crate features."
///
/// Our own `rust/Cargo.toml` pins `rustls` with `features = ["ring"]`, so
/// `ring::default_provider()` is the right provider to install. The
/// existing `crate::net::tls` module installs the same provider lazily in
/// its TLS-pinning helper, but that path is only exercised by the
/// attestation flow — discovery and invocation hit websocket TLS first.
///
/// Idempotent: `install_default()` returns `Err` if a provider is already
/// installed, which we deliberately ignore. The wrapping `Once` keeps the
/// hot path branch-free after first call.
pub(crate) fn ensure_rustls_crypto_provider() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Best-effort: if some other code path already installed a provider,
        // `install_default()` will fail and we leave that one in place.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
