//! Phase 35 — typed errors for the contextvm subsystem.
//!
//! Variants map 1-to-1 to the rows in 35-RESEARCH.md §G (error matrix).
//! Display implementations produce strings the actor can stash directly
//! into `AppState.contextvm_discovery_state = Error { message: ... }` and
//! `dispatch_tools` can return as a tool-call error result.

use std::fmt;

#[derive(Debug, Clone)]
pub enum ContextvmError {
    /// Transport-level failure: relay WebSocket can't connect, DNS fails,
    /// TLS handshake fails. Covers the entire "no relay reachable" class.
    RelayUnreachable { detail: String },
    /// At least one tool announcement parsed but failed validation. The
    /// actor still surfaces whichever tools DID parse — this variant is
    /// reserved for "no announcements parsed at all".
    MalformedAnnouncement { detail: String },
    /// `tokio::time::timeout` elapsed before the proxy delivered a
    /// response on its `UnboundedReceiver<JsonRpcMessage>`.
    Timeout { tool_name: String, secs: u64 },
    /// Provider returned a JSON-RPC `error` envelope.
    JsonRpc { code: i64, message: String },
    /// Anything else — preserves the underlying error message verbatim
    /// for debugging.
    Other { detail: String },
}

impl fmt::Display for ContextvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextvmError::RelayUnreachable { detail } => {
                write!(f, "Couldn't reach relays: {}", detail)
            }
            ContextvmError::MalformedAnnouncement { detail } => {
                write!(f, "Malformed tool announcement: {}", detail)
            }
            ContextvmError::Timeout { tool_name, secs } => {
                write!(f, "Error: tool '{}' timed out ({}s)", tool_name, secs)
            }
            ContextvmError::JsonRpc { code, message } => {
                write!(f, "Error: {}: {}", code, message)
            }
            ContextvmError::Other { detail } => write!(f, "Error: {}", detail),
        }
    }
}

impl std::error::Error for ContextvmError {}

impl From<anyhow::Error> for ContextvmError {
    fn from(e: anyhow::Error) -> Self {
        ContextvmError::Other {
            detail: e.to_string(),
        }
    }
}
