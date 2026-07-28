//! Local contextvm echo server for live integration testing.
//!
//! Generates (or loads from env) a persistent Nostr identity, connects to
//! the same default relays the app uses, and serves an `echo` MCP tool.
//!
//! Usage:
//!   # fresh ephemeral key (printed to stdout):
//!   cargo run -p contextvm-echo-server
//!
//!   # persistent key across restarts (paste the secret hex from a prior run):
//!   CONTEXTVM_SECRET_KEY=<64-char hex> cargo run -p contextvm-echo-server
//!
//! The server prints the pubkey hex on startup.  Copy it into
//! `ECHO_SERVER_PUBKEY` in `rust/src/tests/contextvm.rs`, un-ignore
//! `live_invoke_echo_local`, and run:
//!   cargo test -p mango_core live_invoke_echo_local -- --nocapture --ignored

use anyhow::Result;
use contextvm_sdk::transport::server::{NostrServerTransport, NostrServerTransportConfig};
use contextvm_sdk::{signer, EncryptionMode, GiftWrapMode, RelayPool, ServerInfo};
use nostr_sdk::prelude::{EventBuilder, Kind};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt,
};

/// Default relay set (same as the app). Override with CONTEXTVM_RELAY_OVERRIDE.
const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.nostr.net",
];

fn resolve_relays() -> Vec<String> {
    match std::env::var("CONTEXTVM_RELAY_OVERRIDE") {
        Ok(v) if !v.is_empty() => v.split(',').map(|s| s.trim().to_string()).collect(),
        _ => DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect(),
    }
}

async fn publish_discovery_events(
    keys: &signer::Keys,
    relay_urls: &[String],
    server_info: &ServerInfo,
) -> Result<()> {
    let relay_pool = RelayPool::new(keys.clone()).await?;
    relay_pool.connect(relay_urls).await?;
    let content = serde_json::to_string(server_info)?;
    let event_id = relay_pool
        .publish(EventBuilder::new(Kind::Custom(11316), content))
        .await?;
    let tools = serde_json::json!({
        "tools": [
            {
                "name": "echo",
                "description": "Echo the input message back unchanged",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "The message to echo"
                        }
                    },
                    "required": ["message"]
                }
            }
        ]
    });
    let tools_event_id = relay_pool
        .publish(EventBuilder::new(
            Kind::Custom(11317),
            serde_json::to_string(&tools)?,
        ))
        .await?;
    relay_pool.disconnect().await?;
    println!("Server announcement: {event_id}");
    println!("Tools announcement: {tools_event_id}");
    Ok(())
}

// ── Echo MCP server ───────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EchoParams {
    message: String,
}

#[derive(Clone)]
struct EchoServer {
    tool_router: ToolRouter<Self>,
}

impl EchoServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl EchoServer {
    #[tool(description = "Echo the input message back unchanged")]
    async fn echo(
        &self,
        Parameters(EchoParams { message }): Parameters<EchoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        println!("[echo] called with: {message}");
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Echo: {message}"
        ))]))
    }
}

#[tool_handler]
impl ServerHandler for EchoServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        let server_info = rmcp::model::Implementation::new(
            "mango-echo-server",
            env!("CARGO_PKG_VERSION"),
        )
        .with_title("Mango Local Echo Server")
        .with_description("Local test echo server for mango_core integration tests");
        rmcp::model::ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(server_info)
        .with_instructions("Call echo with {\"message\": \"...\"}")
    }
}

// ── Entry point ───────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let keys = match std::env::var("CONTEXTVM_SECRET_KEY") {
        Ok(hex) if !hex.is_empty() => {
            let k = signer::from_sk(&hex)?;
            println!("Loaded identity from CONTEXTVM_SECRET_KEY");
            k
        }
        _ => {
            let k = signer::generate();
            println!(
                "Generated new identity (set CONTEXTVM_SECRET_KEY={} to reuse)",
                k.secret_key().to_secret_hex()
            );
            k
        }
    };

    let relay_urls = resolve_relays();
    let pubkey_hex = keys.public_key().to_hex();
    let server_info = ServerInfo::default()
        .with_name("mango-echo-server")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_about("Local test echo server for mango_core integration tests");
    println!("Server pubkey: {pubkey_hex}");
    println!("Relays: {}", relay_urls.join(", "));
    println!();
    println!("Paste the pubkey into ECHO_SERVER_PUBKEY in rust/src/tests/contextvm.rs");
    println!("then run:");
    if relay_urls.iter().any(|r| r.starts_with("ws://localhost")) {
        println!("  CONTEXTVM_RELAY_OVERRIDE={} cargo test -p mango_core live_invoke_echo_local -- --nocapture --ignored", relay_urls.join(","));
    } else {
        println!("  cargo test -p mango_core live_invoke_echo_local -- --nocapture --ignored");
    }
    publish_discovery_events(&keys, &relay_urls, &server_info).await?;
    println!("Server ready");
    println!();
    println!("Waiting for tool calls (Ctrl+C to stop)...");

    let transport = NostrServerTransport::new(
        keys,
        NostrServerTransportConfig::default()
            .with_relay_urls(relay_urls)
            .with_encryption_mode(EncryptionMode::Optional)
            .with_gift_wrap_mode(GiftWrapMode::Optional)
            .with_server_info(server_info)
            .with_announced_server(true),
    )
    .await?;

    let service = EchoServer::new().serve(transport).await?;

    tokio::select! {
        res = service.waiting() => {
            res?;
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down.");
        }
    }

    Ok(())
}
