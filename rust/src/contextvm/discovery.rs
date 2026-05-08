//! Phase 35 — discovery service.
//!
//! One-shot pull-on-open Nostr queries: connect → query → disconnect.
//! No persistent subscriptions in v1 (per RESEARCH §B "Battery /
//! connection lifecycle").
//!
//! Discovery uses public events (kind 11316 server announcements + kind
//! 11317 tools list) — no encryption, no signer needed for read.
//! Nevertheless contextvm-sdk's API requires `Keys` to construct the
//! RelayPool; an ephemeral key is fine for read-only discovery.

use crate::contextvm::error::ContextvmError;

/// Typed, UI-friendly view of a server announcement (kind 11316).
#[derive(Debug, Clone)]
pub struct DiscoveredServer {
    pub pubkey_hex: String,
    /// `server_info.name` from the announcement, or None if absent.
    pub display_name: Option<String>,
}

/// Typed, UI-friendly view of one tool from a kind 11317 announcement.
/// `schema_json` is the inputSchema as a serialised JSON string — kept as
/// a string because the dispatch path serialises it back into the OpenAI
/// `tools` array entry verbatim.
#[derive(Debug, Clone)]
pub struct DiscoveredTool {
    pub provider_pubkey_hex: String,
    pub provider_display_name: Option<String>,
    pub tool_name: String,
    pub description: String,
    pub schema_json: String,
}

/// Connect to relays, query for kind 11316 announcements, disconnect.
/// Errors map to `ContextvmError::RelayUnreachable` for transport
/// failures, `MalformedAnnouncement` if EVERY announcement failed to
/// parse, otherwise `Other`.
pub async fn discover_servers(
    relays: &[String],
) -> Result<Vec<DiscoveredServer>, ContextvmError> {
    use contextvm_sdk::{signer, RelayPool};
    let keys = signer::generate();
    let relay_pool = RelayPool::new(keys)
        .await
        .map_err(|e| ContextvmError::RelayUnreachable {
            detail: e.to_string(),
        })?;
    relay_pool
        .connect(relays)
        .await
        .map_err(|e| ContextvmError::RelayUnreachable {
            detail: e.to_string(),
        })?;
    let client = relay_pool.client();
    let announcements = contextvm_sdk::discovery::discover_servers(client, relays)
        .await
        .map_err(|e| ContextvmError::Other {
            detail: e.to_string(),
        })?;
    let _ = relay_pool.disconnect().await;
    Ok(announcements
        .into_iter()
        .map(|a| DiscoveredServer {
            pubkey_hex: a.pubkey.clone(),
            display_name: a.server_info.name.clone(),
        })
        .collect())
}

/// Connect, query kind 11317 tools list for the given provider, disconnect.
/// Returns Vec<DiscoveredTool>; per-tool parse failures are logged via
/// `log::warn!` and silently skipped (matches RESEARCH §G row 3).
pub async fn discover_tools_for_server(
    provider_pubkey_hex: &str,
    provider_display_name: Option<&str>,
    relays: &[String],
) -> Result<Vec<DiscoveredTool>, ContextvmError> {
    use contextvm_sdk::signer::{self, PublicKey};
    use contextvm_sdk::RelayPool;

    let keys = signer::generate();
    let relay_pool = RelayPool::new(keys)
        .await
        .map_err(|e| ContextvmError::RelayUnreachable {
            detail: e.to_string(),
        })?;
    relay_pool
        .connect(relays)
        .await
        .map_err(|e| ContextvmError::RelayUnreachable {
            detail: e.to_string(),
        })?;
    let client = relay_pool.client();

    let parsed_pk = PublicKey::from_hex(provider_pubkey_hex).map_err(|e| {
        ContextvmError::MalformedAnnouncement {
            detail: format!("bad pubkey: {}", e),
        }
    })?;

    let raw_tools =
        contextvm_sdk::discovery::discover_tools_typed(client, &parsed_pk, relays)
            .await
            .map_err(|e| ContextvmError::Other {
                detail: e.to_string(),
            })?;
    let _ = relay_pool.disconnect().await;

    let mut out = Vec::with_capacity(raw_tools.len());
    for t in raw_tools {
        // Each `t` is `rmcp::model::Tool { name, description, input_schema, .. }`.
        let schema_json = match serde_json::to_string(&t.input_schema) {
            Ok(s) => s,
            Err(e) => {
                log::warn!(
                    "skipping tool {} from {}: schema serialisation failed: {}",
                    t.name,
                    provider_pubkey_hex,
                    e
                );
                continue;
            }
        };
        out.push(DiscoveredTool {
            provider_pubkey_hex: provider_pubkey_hex.to_string(),
            provider_display_name: provider_display_name.map(str::to_string),
            tool_name: t.name.to_string(),
            description: t
                .description
                .as_ref()
                .map(|c| c.to_string())
                .unwrap_or_default(),
            schema_json,
        });
    }
    Ok(out)
}

/// Convenience: full discovery sweep — find servers, then for each query
/// its tools. Returns the flattened tool list. Per-server failures are
/// downgraded to a `log::warn!` and skipped so a single broken
/// provider doesn't take down the whole list.
pub async fn discover_all(
    relays: &[String],
) -> Result<Vec<DiscoveredTool>, ContextvmError> {
    let servers = discover_servers(relays).await?;
    let mut out: Vec<DiscoveredTool> = Vec::new();
    for s in servers {
        match discover_tools_for_server(&s.pubkey_hex, s.display_name.as_deref(), relays)
            .await
        {
            Ok(mut tools) => out.append(&mut tools),
            Err(e) => {
                log::warn!("skipping tools from server {}: {}", s.pubkey_hex, e);
            }
        }
    }
    Ok(out)
}
