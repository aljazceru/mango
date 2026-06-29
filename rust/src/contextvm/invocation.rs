//! Phase 35 — invocation service.
//!
//! Each remote tool call:
//!   1. Resolve (or lazily create) the persistent Nostr secret key for
//!      this device.
//!   2. Build a `NostrMCPProxy` keyed by `provider_pubkey_hex`.
//!   3. Send a `tools/call` JSON-RPC request.
//!   4. Await the response on the proxy's `UnboundedReceiver` with a
//!      15-second timeout.
//!   5. Truncate to 16 KiB, append `... [truncated]` if needed.
//!   6. Stop the proxy (no pooling in v1 — keeps state minimal; one
//!      short-lived proxy per call).
//!
//! Encryption mode = `Optional` (CONTEXT discretion locked, RESEARCH §A
//! footnote 1). Encryption-required providers will surface as a regular
//! invocation error in v1; NIP-59 gift-wrap is deferred.

use std::time::Duration;

use rusqlite::Connection;

use crate::contextvm::error::ContextvmError;
use crate::persistence::queries;

pub const INVOCATION_TIMEOUT_SECS: u64 = 15;
pub const MAX_TOOL_RESULT_BYTES: usize = 16_384;
const SETTINGS_SECRET_KEY: &str = "contextvm_secret_key";

/// Load the persisted Nostr secret key from `settings`, or generate a
/// fresh one and persist it. The key is identity-only; a future plan can
/// promote this to keychain storage if threat model warrants it.
pub fn load_or_create_secret_key(conn: &Connection) -> Result<String, ContextvmError> {
    if let Some(hex) =
        queries::get_setting(conn, SETTINGS_SECRET_KEY).map_err(|e| ContextvmError::Other {
            detail: e.to_string(),
        })?
    {
        if !hex.is_empty() {
            return Ok(hex);
        }
    }
    let keys = contextvm_sdk::signer::generate();
    let secret_hex = keys.secret_key().to_secret_hex();
    queries::set_setting(conn, SETTINGS_SECRET_KEY, &secret_hex).map_err(|e| {
        ContextvmError::Other {
            detail: e.to_string(),
        }
    })?;
    Ok(secret_hex)
}

/// Truncate at byte boundary; append a marker if truncation occurred.
/// Splits on a UTF-8 char boundary by walking back to the nearest one.
pub fn truncate_result(s: String) -> String {
    if s.len() <= MAX_TOOL_RESULT_BYTES {
        return s;
    }
    let mut end = MAX_TOOL_RESULT_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... [truncated]", &s[..end])
}

/// Format a JSON-RPC error envelope as a tool-call result string.
/// Locked copy: matches `ContextvmError::JsonRpc` Display.
pub fn format_jsonrpc_error(code: i64, message: &str) -> String {
    format!("Error: {}: {}", code, message)
}

/// Format a timeout as a tool-call result string. Locked copy.
pub fn format_timeout(tool_name: &str) -> String {
    format!(
        "Error: tool '{}' timed out ({}s)",
        tool_name, INVOCATION_TIMEOUT_SECS
    )
}

/// Invoke a remote tool. Returns the user-visible result string that the
/// dispatch layer feeds back to the LLM as the tool-call result. ALWAYS
/// returns a String — failures are stringified, never propagated as
/// `Result::Err`. This matches the local-tool dispatch contract (see
/// `dispatch_web_search` etc.).
///
/// The `secret_key_hex` is the per-device persistent Nostr identity key
/// loaded via `load_or_create_secret_key` ahead of the call (the caller
/// owns the SQLite connection and the actor sequencing — invocation
/// itself is pure async + network).
pub async fn invoke_tool(
    secret_key_hex: &str,
    provider_pubkey_hex: &str,
    tool_name: &str,
    args_json: &str,
) -> String {
    use contextvm_sdk::proxy::{NostrMCPProxy, ProxyConfig};
    use contextvm_sdk::{
        signer, EncryptionMode, JsonRpcMessage, JsonRpcRequest, NostrClientTransportConfig,
    };
    crate::contextvm::ensure_rustls_crypto_provider();

    // Parse the persisted key. If parsing fails (corrupt setting), fall
    // through to the Other error path.
    let keys = match signer::from_sk(secret_key_hex) {
        Ok(k) => k,
        Err(e) => {
            return ContextvmError::Other {
                detail: format!("invalid persisted secret key: {}", e),
            }
            .to_string();
        }
    };

    let relays = match std::env::var("CONTEXTVM_RELAY_OVERRIDE") {
        Ok(v) if !v.is_empty() => v.split(',').map(|s| s.trim().to_string()).collect(),
        _ => crate::contextvm::default_relays_owned(),
    };
    // `NostrClientTransportConfig` is `#[non_exhaustive]` cross-crate, so
    // we mutate the `Default` instance via its `with_*` builders rather
    // than using a struct expression.
    let nostr_config = NostrClientTransportConfig::default()
        .with_relay_urls(relays)
        .with_server_pubkey(provider_pubkey_hex.to_string())
        .with_encryption_mode(EncryptionMode::Optional);
    let config = ProxyConfig::new(nostr_config);

    let mut proxy = match NostrMCPProxy::new(keys, config).await {
        Ok(p) => p,
        Err(e) => {
            return ContextvmError::RelayUnreachable {
                detail: e.to_string(),
            }
            .to_string();
        }
    };
    let mut rx = match proxy.start().await {
        Ok(rx) => rx,
        Err(e) => {
            let _ = proxy.stop().await;
            return ContextvmError::RelayUnreachable {
                detail: e.to_string(),
            }
            .to_string();
        }
    };

    // Build a tools/call JSON-RPC request.
    let args_value: serde_json::Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(e) => {
            let _ = proxy.stop().await;
            return ContextvmError::Other {
                detail: format!("bad tool args JSON: {}", e),
            }
            .to_string();
        }
    };
    let request = JsonRpcMessage::Request(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: serde_json::json!(1),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({
            "name": tool_name,
            "arguments": args_value,
        })),
    });

    if let Err(e) = proxy.send(&request).await {
        let _ = proxy.stop().await;
        return ContextvmError::Other {
            detail: format!("send failed: {}", e),
        }
        .to_string();
    }

    let timeout = Duration::from_secs(INVOCATION_TIMEOUT_SECS);
    let recv = tokio::time::timeout(timeout, rx.recv()).await;
    let _ = proxy.stop().await;

    let response = match recv {
        Ok(Some(msg)) => msg,
        Ok(None) => return format_timeout(tool_name),
        Err(_) => return format_timeout(tool_name),
    };

    decode_response(&response, tool_name)
}

/// Decode a `JsonRpcMessage` response into the user-visible result string.
/// Pulled into its own function so unit tests can construct synthetic
/// messages without spinning up a real proxy.
pub fn decode_response(response: &contextvm_sdk::JsonRpcMessage, _tool_name: &str) -> String {
    let raw = match serde_json::to_value(response) {
        Ok(v) => v,
        Err(e) => {
            return ContextvmError::Other {
                detail: format!("response serialise failed: {}", e),
            }
            .to_string();
        }
    };

    if let Some(err) = raw.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("(no message)");
        return format_jsonrpc_error(code, msg);
    }

    // Success: extract result.content[0].text per MCP spec.
    let text = raw
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|first| first.get("text"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            // Fall back to whole result blob as JSON string.
            raw.get("result")
                .map(|r| r.to_string())
                .unwrap_or_else(|| "(empty result)".to_string())
        });

    truncate_result(text)
}
