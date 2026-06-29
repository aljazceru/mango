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

/// Nostr profile metadata (kind 0) for a provider.
#[derive(Debug, Clone)]
pub struct ProviderProfile {
    #[allow(dead_code)]
    pubkey_hex: String,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
}

/// Typed, UI-friendly view of one tool from a kind 11317 announcement.
/// `schema_json` is the inputSchema as a serialised JSON string — kept as
/// a string because the dispatch path serialises it back into the OpenAI
/// `tools` array entry verbatim.
#[derive(Debug, Clone)]
pub struct DiscoveredTool {
    pub provider_pubkey_hex: String,
    pub provider_display_name: Option<String>,
    /// Provider profile name from kind 0 metadata (Phase 37).
    pub provider_name: Option<String>,
    /// Provider profile "about" text from kind 0 metadata (Phase 37).
    pub provider_about: Option<String>,
    /// Provider profile picture URL from kind 0 metadata (Phase 37).
    pub provider_picture: Option<String>,
    /// Provider NIP-05 identifier from kind 0 metadata (Phase 37).
    pub provider_nip05: Option<String>,
    pub tool_name: String,
    pub description: String,
    pub schema_json: String,
}

/// Connect to relays, query for kind 11316 announcements, disconnect.
/// Errors map to `ContextvmError::RelayUnreachable` for transport
/// failures, `MalformedAnnouncement` if EVERY announcement failed to
/// parse, otherwise `Other`.
pub async fn discover_servers(relays: &[String]) -> Result<Vec<DiscoveredServer>, ContextvmError> {
    use contextvm_sdk::{signer, RelayPool};
    crate::contextvm::ensure_rustls_crypto_provider();
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
    crate::contextvm::ensure_rustls_crypto_provider();

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

    let raw_tools = contextvm_sdk::discovery::discover_tools_typed(client, &parsed_pk, relays)
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
            provider_name: None,
            provider_about: None,
            provider_picture: None,
            provider_nip05: None,
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

async fn discover_tools_from_servers(
    servers: Vec<DiscoveredServer>,
    relays: &[String],
) -> Result<Vec<DiscoveredTool>, ContextvmError> {
    let mut out: Vec<DiscoveredTool> = Vec::new();
    for s in servers {
        match discover_tools_for_server(&s.pubkey_hex, s.display_name.as_deref(), relays).await {
            Ok(mut tools) => out.append(&mut tools),
            Err(e) => {
                log::warn!("skipping tools from server {}: {}", s.pubkey_hex, e);
            }
        }
    }

    // Fetch profiles for all unique providers (Phase 37)
    let unique_provider_pubkeys: std::collections::HashSet<String> =
        out.iter().map(|t| t.provider_pubkey_hex.clone()).collect();
    let provider_pubkeys: Vec<String> = unique_provider_pubkeys.into_iter().collect();

    if !provider_pubkeys.is_empty() {
        log::info!("Fetching profiles for {} providers", provider_pubkeys.len());
        match fetch_provider_profiles_batch(&provider_pubkeys, relays).await {
            Ok(profiles) => {
                log::info!("Fetched {} provider profiles", profiles.len());
                // Merge profiles into the tools
                for tool in &mut out {
                    if let Some(profile) = profiles.get(&tool.provider_pubkey_hex) {
                        log::info!(
                            "Merging profile for provider {}: name={:?}",
                            tool.provider_pubkey_hex,
                            profile.name
                        );
                        tool.provider_name = profile.name.clone();
                        tool.provider_about = profile.about.clone();
                        tool.provider_picture = profile.picture.clone();
                        tool.provider_nip05 = profile.nip05.clone();
                    }
                }
            }
            Err(e) => {
                log::warn!("failed to fetch provider profiles: {}", e);
            }
        }
    }

    Ok(out)
}

/// Convenience: full discovery sweep — find servers, then for each query
/// its tools. Returns the flattened tool list. Per-server failures are
/// downgraded to a `log::warn!` and skipped so a single broken
/// provider doesn't take down the whole list.
///
/// Phase 37: also fetches provider profiles and merges them into the tools.
pub async fn discover_all(relays: &[String]) -> Result<Vec<DiscoveredTool>, ContextvmError> {
    let servers = discover_servers(relays).await?;
    discover_tools_from_servers(servers, relays).await
}

/// Discover tools only for provider pubkeys that are already trusted.
///
/// This is the mobile auto-discovery path: trust is an allow-list, so filtering
/// server announcements before per-server tool queries avoids blocking the chat
/// actor on unrelated public ContextVM providers.
pub async fn discover_all_for_providers(
    relays: &[String],
    provider_pubkeys: &std::collections::HashSet<String>,
) -> Result<Vec<DiscoveredTool>, ContextvmError> {
    let servers = discover_servers(relays).await?;
    let servers = servers
        .into_iter()
        .filter(|server| provider_pubkeys.contains(&server.pubkey_hex))
        .collect();
    discover_tools_from_servers(servers, relays).await
}

/// Fetch Nostr kind 0 metadata (profile) for a single provider pubkey.
/// Returns None if the profile event is not found or fails to parse.
#[allow(dead_code)]
pub async fn fetch_provider_profile(
    provider_pubkey_hex: &str,
    relays: &[String],
) -> Result<Option<ProviderProfile>, ContextvmError> {
    use contextvm_sdk::signer::{self, PublicKey};
    use contextvm_sdk::RelayPool;
    use nostr::prelude::*;
    crate::contextvm::ensure_rustls_crypto_provider();

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

    // Build a filter for kind 0 (metadata) events from this pubkey
    let filter = Filter::new()
        .author(parsed_pk)
        .kind(Kind::Metadata)
        .limit(1);

    // Query the relays using fetch_events_from with a timeout
    let events = client
        .fetch_events_from(relays, filter, std::time::Duration::from_secs(10))
        .await
        .map_err(|e| ContextvmError::Other {
            detail: e.to_string(),
        })?;

    let _ = relay_pool.disconnect().await;

    if events.is_empty() {
        return Ok(None);
    }

    // Use the most recent profile event
    let latest_event = events.into_iter().next().unwrap();

    // Parse the content JSON (kind 0 content is a JSON object with name, about, picture, etc.)
    let metadata: serde_json::Value = serde_json::from_str(&latest_event.content).map_err(|e| {
        ContextvmError::MalformedAnnouncement {
            detail: format!("failed to parse profile metadata: {}", e),
        }
    })?;

    let name = metadata
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from);
    let about = metadata
        .get("about")
        .and_then(|v| v.as_str())
        .map(String::from);
    let picture = metadata
        .get("picture")
        .and_then(|v| v.as_str())
        .map(String::from);
    let nip05 = metadata
        .get("nip05")
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(Some(ProviderProfile {
        pubkey_hex: provider_pubkey_hex.to_string(),
        name,
        about,
        picture,
        nip05,
    }))
}

/// Fetch profiles for multiple provider pubkeys in a single connection.
/// Returns a map from pubkey_hex to ProviderProfile (missing entries mean
/// no profile was found or failed to parse).
pub async fn fetch_provider_profiles_batch(
    pubkeys: &[String],
    relays: &[String],
) -> Result<std::collections::HashMap<String, ProviderProfile>, ContextvmError> {
    use contextvm_sdk::signer::{self, PublicKey};
    use contextvm_sdk::RelayPool;
    use nostr::prelude::*;
    crate::contextvm::ensure_rustls_crypto_provider();

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

    let mut parsed_pks = Vec::new();
    for pubkey in pubkeys {
        match PublicKey::from_hex(pubkey) {
            Ok(pk) => parsed_pks.push(pk),
            Err(e) => {
                log::warn!("skipping invalid pubkey {}: {}", pubkey, e);
            }
        }
    }

    // Build a filter for kind 0 (metadata) events from all pubkeys
    let filter = Filter::new().authors(parsed_pks).kind(Kind::Metadata);

    // Query the relays using fetch_events_from with a timeout
    let events = client
        .fetch_events_from(relays, filter, std::time::Duration::from_secs(10))
        .await
        .map_err(|e| ContextvmError::Other {
            detail: e.to_string(),
        })?;

    let _ = relay_pool.disconnect().await;

    let mut result: std::collections::HashMap<String, ProviderProfile> =
        std::collections::HashMap::new();

    for event in events {
        let pubkey_hex = event.pubkey.to_hex();
        let metadata: serde_json::Value = match serde_json::from_str(&event.content) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("failed to parse profile metadata for {}: {}", pubkey_hex, e);
                continue;
            }
        };

        let name = metadata
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from);
        let about = metadata
            .get("about")
            .and_then(|v| v.as_str())
            .map(String::from);
        let picture = metadata
            .get("picture")
            .and_then(|v| v.as_str())
            .map(String::from);
        let nip05 = metadata
            .get("nip05")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Kind-0 events are returned newest-first; keep first-seen per pubkey
        // so older duplicates do not overwrite fresher metadata.
        if !result.contains_key(&pubkey_hex) {
            result.insert(
                pubkey_hex.clone(),
                ProviderProfile {
                    pubkey_hex,
                    name,
                    about,
                    picture,
                    nip05,
                },
            );
        }
    }

    Ok(result)
}
