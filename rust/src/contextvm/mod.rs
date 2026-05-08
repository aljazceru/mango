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

pub use discovery::{
    discover_all, discover_servers, discover_tools_for_server, DiscoveredServer, DiscoveredTool,
};
pub use dispatch::{
    build_dispatch_map, descriptors_to_chat_tools, finalise_for_turn, hydrate_from_db,
    ContextvmToolDescriptor, DESCRIPTION_CAP_CHARS, MAX_REMOTE_TOOLS_PER_TURN,
    RESERVED_LOCAL_NAMES,
};
pub use error::ContextvmError;
pub use invocation::{invoke_tool, INVOCATION_TIMEOUT_SECS, MAX_TOOL_RESULT_BYTES};

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
