//! Phase 36 — npub bech32 encoding for displaying contextvm tool provider pubkeys.
//!
//! Pre-computed once per row at projection time (see `row_to_discoverable_tool`)
//! and surfaced as `DiscoverableTool.npub` to native UI. Never panics — invalid
//! input falls back to `"invalid:<first-8-chars>"` per Phase 36 RESEARCH Pitfall 6.

use contextvm_sdk::signer::PublicKey;
use nostr::nips::nip19::ToBech32;

/// Encode a hex-encoded provider pubkey as bech32 `npub1…`. On invalid hex
/// or bech32 encode failure, returns an `invalid:<prefix>` fallback string
/// and emits a `log::warn!`. Never panics.
pub fn encode_npub(provider_pubkey_hex: &str) -> String {
    match PublicKey::from_hex(provider_pubkey_hex) {
        Ok(pk) => pk.to_bech32().unwrap_or_else(|e| {
            log::warn!(
                "npub bech32 encoding failed for pubkey '{}': {}",
                provider_pubkey_hex,
                e
            );
            fallback(provider_pubkey_hex)
        }),
        Err(e) => {
            log::warn!("invalid hex pubkey '{}': {}", provider_pubkey_hex, e);
            fallback(provider_pubkey_hex)
        }
    }
}

fn fallback(input: &str) -> String {
    let prefix: String = input.chars().take(8).collect();
    format!("invalid:{}", prefix)
}
