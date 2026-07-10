# Phase 35: contextvm-sdk integration — Research

**Researched:** 2026-05-08

## Summary

Phase 35 is implementable as-CONTEXT'd, with one wrinkle and one happy surprise.

**Happy surprise:** `contextvm-sdk` 0.1.0 (the actual published name on crates.io) is a clean, pure-Rust wrapper around `nostr-sdk 0.43` + `rmcp 0.16`. Its full transitive tree pulls **rustls** (via `tokio-rustls 0.26` + `tokio-tungstenite 0.26`) and **zero `openssl-sys` / `native-tls`** — verified by `cargo tree` on an isolated crate (`/tmp/testdep`). It satisfies CLAUDE.md's "no OpenSSL in the network stack" hard constraint out of the box. No feature gymnastics required.

**The wrinkle:** the SDK's "discovery" surface (`discovery::discover_servers`, `discovery::discover_tools_typed`) is **request/response one-shot** — it queries relays once and returns `Vec<…>`. There is no built-in "subscribe to live tool announcements" stream. For Phase 35's two affordances ("pull on open" Tool Discovery screen + "auto-discover at conversation start"), one-shot semantics are exactly right; we explicitly avoid persistent subscriptions in v1, sidestepping the Android-background and reactor-lifecycle traps.

**The other landmine:** invocation is **NOT** a one-call helper. The SDK exposes a low-level `NostrMCPProxy` that takes a single `server_pubkey`, sends a `JsonRpcMessage::Request`, and returns responses on an `UnboundedReceiver<JsonRpcMessage>`. Each remote tool may live on a *different* provider pubkey, so per-tool invocation requires constructing (or reusing) a proxy keyed by provider pubkey. The dispatch design must account for this: route by tool → look up provider pubkey → ensure a proxy for that pubkey exists → send `tools/call` → await response on the proxy's rx channel with a timeout. This is roughly the same design effort as the existing `dispatch_web_search` (uses `runtime.block_on`), but with the added complexity of a per-provider proxy lifecycle.

**One key thing the planner must know:** the `contextvm-sdk` crate ships **no default relay constant**. The example uses `wss://relay.damus.io` as a placeholder. CTX-07 ("contextvm-sdk defaults + relay.nostr.net") therefore has no "defaults" to union with; the planner should interpret CTX-07 as **a hardcoded relay list inside the Rust core** consisting of a curated set of well-known Nostr relays plus `wss://relay.nostr.net`. Recommended starter set: `wss://relay.damus.io`, `wss://nos.lol`, `wss://relay.nostr.net`. Document this in PLAN.md frontmatter as a CONTEXT amendment.

---

## A. contextvm-sdk crate

### Identity & version
- **Crate name on crates.io:** `contextvm-sdk` (not `contextvm`, not `context-vm`).
- **Version:** `0.1.0` (only published version as of research date).
- **License:** MIT.
- **Repository:** `https://github.com/k0sti/rust-contextvm-sdk`.
- **rust-version:** unknown (cargo info shows `unknown`); the source uses `edition = "2021"` so MSRV is at most 1.56 by edition rules. No explicit MSRV declared — the planner should pin via the CI matrix and let `cargo build` decide.
- **Default features:** `default = ["rmcp"]`. The `rmcp` feature pulls `rmcp 0.16.0` with `["server", "client", "macros", "transport-worker"]`. **Keep `rmcp` enabled** — `discover_tools_typed` and the proxy `serve_client_handler` rely on rmcp types.

### Recommended `Cargo.toml` line
```toml
contextvm-sdk = "0.1.0"
```
No `default-features = false` needed; default features are exactly what the integration uses.

### Discovery API (verified from docs.rs)
The `discovery` module exposes one-shot async fetches. Key functions:

```rust
pub async fn discover_servers(
    client: &Arc<Client>,        // nostr_sdk::Client behind the RelayPool
    relay_urls: &[String],
) -> Result<Vec<ServerAnnouncement>>

pub async fn discover_tools_typed(
    client: &Arc<Client>,
    server_pubkey: &PublicKey,
    relay_urls: &[String],
) -> Result<Vec<Tool>>          // rmcp::model::Tool — typed schema
```

`ServerAnnouncement` exposes `server_info.name`, `server_info.about`, `pubkey: String`, `pubkey_parsed: PublicKey` (verified from `examples/discovery.rs`). The `Tool` type from rmcp carries `name`, `description`, `input_schema` (a `serde_json::Value` matching JSON Schema) — this is what the Phase 35 UI needs for D-04.

Untyped variants (`discover_tools` returning `Vec<serde_json::Value>`) also exist; **prefer `_typed` versions** so the rmcp `Tool` struct does the parsing/validation for us.

Other discovery functions (not used in Phase 35 v1 — listed for reference only):
- `discover_resources_typed`, `discover_resource_templates_typed`, `discover_prompts_typed`.

### Invocation API
No one-call helper. The pattern:

1. Build `NostrClientTransportConfig { relay_urls, server_pubkey: <hex>, encryption_mode: EncryptionMode::Optional, .. Default::default() }`.
2. Wrap in `ProxyConfig { nostr_config: ... }`.
3. `let mut proxy = NostrMCPProxy::new(keys, config).await?;`
4. `let mut rx = proxy.start().await?;` — returns `UnboundedReceiver<JsonRpcMessage>`.
5. Send a `JsonRpcMessage::Request(JsonRpcRequest { method: "tools/call", params: Some(serde_json::json!({"name": tool_name, "arguments": args})), id: <unique>, .. })`.
6. `proxy.send(&request).await?;`
7. `rx.recv().await` for the response. Match `JsonRpcMessage::Response` and read `result.content[0].text` (per protocol spec from Context7) or `result.isError`.
8. `proxy.stop().await?` to clean up. `is_active()` is available for liveness checks.

The exact verbatim signature of `NostrMCPProxy::new` per docs:

```rust
pub async fn new<T>(signer: T, config: ProxyConfig) -> Result<Self>
where T: IntoNostrSigner

pub async fn start(&mut self) -> Result<UnboundedReceiver<JsonRpcMessage>>
pub async fn send(&self, message: &JsonRpcMessage) -> Result<()>
pub async fn stop(&mut self) -> Result<()>
pub fn is_active(&self) -> bool
```

There is also a higher-level `serve_client_handler` that accepts an `rmcp::ClientHandler` — useful for long-lived sessions but more abstraction than Phase 35 needs. Stick with `new`/`start`/`send`/`recv`.

### Default relays
**No default relay constant is exported.** The `examples/discovery.rs` hardcodes `vec!["wss://relay.damus.io".to_string()]` as a demonstrative starter. CTX-07 should be amended to read: **"a curated relay list defined in the Rust core: `["wss://relay.damus.io", "wss://nos.lol", "wss://relay.nostr.net"]`"**. The planner should hardcode this in a `const DEFAULT_CONTEXTVM_RELAYS: &[&str]` in (e.g.) `rust/src/contextvm/mod.rs`.

### Async model
- Tokio-based. All public APIs are `async fn` returning `Future`.
- `proxy.start()` returns `tokio::sync::mpsc::UnboundedReceiver`.
- No exposed `Stream` interface for live announcements (one-shot `Vec` returns only).
- No documented blocking calls.
- The crate sets `tokio = { version = "1", features = ["full"] }`. mango_core only enables a subset (`rt-multi-thread`, `sync`, `time`, `macros`); the dependency resolver unifies features so the full set will be enabled in mango_core's tree once contextvm-sdk is added. This is a wider tokio surface than mango_core had before — confirm no compile-time regressions on mobile cross-compile by running `cargo ndk` builds during PLAN execution.

### Dependency tree (transitive, OpenSSL audit)
**Verified via `cargo tree` in `/tmp/testdep` with only `contextvm-sdk = "0.1.0"`:**

- `nostr-sdk 0.43` → `nostr-relay-pool 0.43` → `async-wsocket` → `tokio-rustls 0.26` + `tokio-tungstenite 0.26` (rustls feature) + `tungstenite 0.26` (rustls).
- `rmcp 0.16` (server + client + macros + transport-worker).
- `tokio 1.x`, `serde`, `serde_json`, `thiserror 2.0`, `async-trait 0.1`, `lru 0.12`, `tokio-util 0.7`, `tracing`, `tracing-subscriber`.
- **`openssl-sys`: 0 occurrences.**
- **`native-tls`: 0 occurrences.**
- `rustls 0.23.40` is pulled in transitively — already in mango_core's tree (via `reqwest` with `rustls-tls-webpki-roots`), so this is a duplicate-free addition.

**In the existing mango_core tree, `openssl-sys 0.9.113` IS present, but only via `rusqlite`'s `bundled-sqlcipher-vendored-openssl` feature.** That's a compile-time vendored bundle for SQLCipher — orthogonal to contextvm-sdk. Adding contextvm-sdk does NOT introduce a new openssl-sys edge. The CLAUDE.md prohibition on OpenSSL was specifically about the network stack on iOS/Android, which uses rustls in this codebase; the SQLCipher-vendored openssl is the established exception and Phase 35 does not regress it.

### Cross-compile
- No issues reported in the contextvm-sdk repo issue tracker (the repo is small / new).
- All transitive deps (rustls, tungstenite, nostr-sdk, rmcp) build on `aarch64-linux-android` and `aarch64-apple-ios` — these are mainstream targets for the Rust web ecosystem.
- One open question: rmcp's `transport-worker` feature; its build behavior on iOS is unverified by us. The planner should run `cargo ndk build --target aarch64-linux-android -p mango_core` and a `cargo build --target aarch64-apple-ios -p mango_core` early in PLAN execution as a smoke test before deep integration work begins. If rmcp pulls in any platform-gated dep, swap to `default-features = false` and selectively re-enable.

### Examples
Two key snippets (verbatim from upstream):

**`examples/discovery.rs` — discovery flow:**
```rust
let keys = signer::generate();
let relays = vec!["wss://relay.damus.io".to_string()];
let relay_pool = RelayPool::new(keys).await?;
relay_pool.connect(&relays).await?;
let client = relay_pool.client();

let servers = discovery::discover_servers(client, &relays).await?;
for server in &servers {
    let tools = discovery::discover_tools(client, &server.pubkey_parsed, &relays).await?;
    for tool in &tools {
        let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        // ...
    }
}
relay_pool.disconnect().await?;
```

**`examples/proxy.rs` — invocation flow:**
```rust
let keys = signer::generate();
let config = ProxyConfig {
    nostr_config: NostrClientTransportConfig {
        relay_urls: vec!["wss://relay.damus.io".to_string()],
        server_pubkey: server_pubkey_hex,
        encryption_mode: EncryptionMode::Optional,
        ..Default::default()
    },
};
let mut proxy = NostrMCPProxy::new(keys, config).await?;
let mut rx = proxy.start().await?;

let request = JsonRpcMessage::Request(JsonRpcRequest {
    jsonrpc: "2.0".to_string(),
    id: serde_json::json!(1),
    method: "tools/list".to_string(),
    params: None,
});
proxy.send(&request).await?;

if let Some(response) = rx.recv().await {
    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());
}
proxy.stop().await?;
```

These are the two canonical patterns Phase 35 will mirror.

---

## B. Nostr protocol footprint

### Event kinds (verified via Context7 against the ContextVM draft spec)
- **`11316`** — Server Announcement (`KIND_SERVER_ANNOUNCEMENT`). Replaceable. Payload: `{ protocolVersion, capabilities, serverInfo: { name, version }, instructions }`. Tags include `name`, `about`, `picture`, `website`, optional `support_encryption`.
- **`11317`** — Tools List event. Payload: `{ tools: [{ name, description, inputSchema }] }`.
- **`11318`** — Resources List event (not used in Phase 35).
- **`11319`** — Resource Templates List event (not used).
- **`11320`** — Prompts List event (not used).
- **`25910`** — Ephemeral request/response event for tool invocation. Tags: `["p", "<provider-pubkey>"]` on requests; `["e", "<request-event-id>"]` on responses. Content: full JSON-RPC 2.0 envelope (`tools/call` request → `{ result: { content: [...], isError } }` response).

### Transport model
- **Discovery** uses **public events** (kind 11316/11317) — no encryption; anyone can subscribe and read.
- **Invocation** (kind 25910) **may** use NIP-59 gift-wrap encryption depending on `EncryptionMode`. The SDK uses `EncryptionMode::Optional` in examples — content is plaintext unless the provider requires encryption. Our v1 should use `EncryptionMode::Optional` to maximize compatibility. Encryption requires the `nip59` feature on `nostr-sdk` — already enabled by contextvm-sdk's Cargo.toml.

### Identity / key management
- The app **must** manage a Nostr keypair (sec/pub key) — the proxy `new(signer, ...)` requires a signer. `signer::generate()` creates fresh `Keys`; `signer::from_sk(...)` imports.
- For Phase 35 v1: **generate one identity at first use, persist the secret key** so the same pubkey is presented to providers across sessions (helpful for any future rate-limiting/reputation work — out of scope here but harmless to set up). Storage: a single row in the existing `settings` table under key `contextvm_secret_key` (hex-encoded). NOT in the keychain (the existing keychain is for backend API keys); the Nostr key is non-sensitive in the v1 threat model — it identifies the *client* to public relays, not a high-value secret.
- Discovery flows can also use an ephemeral key (`signer::generate()` per-query) since discovery is read-only on public events. Keep it simple: one persisted key for both.

### Battery / connection lifecycle
- `RelayPool::connect` opens persistent WebSockets (one per relay).
- For Phase 35, **avoid persistent subscriptions** — the v1 model is pull-on-open (Tool Discovery) + pull-at-conversation-start (auto-discover). Connect, query, disconnect each time. Battery cost: one TLS handshake + ~100ms of relay round-trip per query. Acceptable for a user-initiated screen open and a once-per-conversation auto-discover query.
- For invocation: open a `NostrMCPProxy`, send the call, await response (with a timeout — 30s recommended), stop the proxy. Do not pool proxies across calls in v1 — keep it stateless. Re-opening costs a couple of hundred ms but avoids reconnect bookkeeping.
- On Android background: per RMP architecture, the actor thread runs inside the app process; no special WorkManager involvement needed because contextvm calls are synchronous within the actor's lifecycle (we never expect a tool call to outlive the foreground app).

---

## C. Integration points in this codebase

All file paths are absolute.

### 1. Tool dispatch fan-out
**File:** `/home/lio/g/confidential-app/rust/src/agent/tools.rs` line **249** — `pub fn dispatch_tools(...)`.

```rust
// rust/src/agent/tools.rs:249
pub fn dispatch_tools(
    calls: &[ChatCompletionMessageToolCall],
    db_conn: &rusqlite::Connection,
    vector_index: &VectorIndex,
    embedding_provider: &dyn EmbeddingProvider,
    runtime: &tokio::runtime::Runtime,
    data_dir: &str,
    brave_api_key: &str,
) -> Vec<(String, String)> {
    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let function_name = call.function.name.as_str();
        let result = match function_name {
            "search_documents" => dispatch_search_documents(...),
            "read_document" => dispatch_read_document(...),
            "finish" => dispatch_finish(args_str),
            "web_search" => dispatch_web_search(args_str, runtime, brave_api_key),
            "fetch_url" => dispatch_fetch_url(args_str, runtime),
            "file" => dispatch_file(args_str, data_dir),
            "calculate" => dispatch_calculate(args_str),
            unknown => format!("Error: unknown tool '{}'", unknown),
        };
        results.push((tool_call_id, result));
    }
    results
}
```

This is the **single chokepoint** for routing. Phase 35 must extend the `match` arm so that `unknown` falls through to a new contextvm dispatcher (instead of returning an error). Alternatively, a check on the function name (sentinel prefix or in-memory lookup) decides remote vs local.

### 2. OpenAI-compatible `tools` array assembly
**File:** `/home/lio/g/confidential-app/rust/src/agent/tools.rs` line **206** — `pub fn build_chat_tools(include_doc_search, brave_api_key_set)`.

```rust
// rust/src/agent/tools.rs:206
pub fn build_chat_tools(
    include_doc_search: bool,
    brave_api_key_set: bool,
) -> Vec<ChatCompletionTools> {
    let all = build_agent_tools();
    all.into_iter()
        .filter(|tool| {
            let name = match tool {
                ChatCompletionTools::Function(ref t) => t.function.name.as_str(),
                ChatCompletionTools::Custom(_) => return true,
            };
            match name {
                "finish" => false,
                "search_documents" | "read_document" => include_doc_search,
                "web_search" => brave_api_key_set,
                _ => true,
            }
        })
        .collect()
}
```

Call site: `rust/src/lib.rs:2360` (`let tools = agent::build_chat_tools(has_docs, brave_key_set);`). Phase 35 extends the signature to accept a `Vec<ContextvmToolDescriptor>` (or similar) and appends them after filtering.

### 3. AppAction enum
**File:** `/home/lio/g/confidential-app/rust/src/lib.rs` line **446** — `pub enum AppAction { ... }`.

The handler dispatch loop is at `lib.rs:5888` (representative — `AppAction::SetMemoriesEnabled`):

```rust
// rust/src/lib.rs:5888
AppAction::SetMemoriesEnabled { enabled } => {
    let _ = persistence::queries::set_setting(
        actor_state.db.as_ref().expect("db unlocked").conn(),
        "memories_enabled",
        if enabled { "1" } else { "0" },
    );
    actor_state.app_state.memories_enabled = enabled;
}
```

Phase 35 will add new variants: `DiscoverContextvmTools`, `SetContextvmToolEnabled { tool_id, enabled }`, `SetAutoDiscoverTools { enabled }`, `RetryContextvmDiscovery`. Each handler mirrors the `SetMemoriesEnabled` pattern.

### 4. Settings persistence
**File:** `/home/lio/g/confidential-app/rust/src/persistence/queries.rs` lines **930–945**.

```rust
// queries.rs:930
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, PersistenceError> {
    let mut stmt = conn.prepare_cached("SELECT value FROM settings WHERE key = ?1")?;
    // ...
}
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), PersistenceError> {
    conn.prepare_cached("INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)")?
        // ...
}
```

The `settings` table is a key/value store. Existing keys: `memories_enabled`, `brave_api_key`, `default_backend_id`, `has_completed_onboarding`, `duress_decoy_mode`, `tee_policy_tdx`, `tee_policy_snp`. Hydrated on unlock at `lib.rs:3937` (`memories_enabled`) and pushed into `AppState.memories_enabled` at `lib.rs:4021`.

Phase 35 adds:
- New settings key `auto_discover_tools` ("1"/"0", default "0").
- Dedicated table `contextvm_tools` (see Section D — JSON blob in settings is too crude for per-tool toggle + provider pubkey + schema lookup).

### 5. AgentStepSummary / provenance record
**File:** `/home/lio/g/confidential-app/rust/src/lib.rs` line **97** — `pub struct AgentStepSummary`.

```rust
// rust/src/lib.rs:97
#[derive(uniffi::Record, Clone, Debug)]
pub struct AgentStepSummary {
    pub id: String,
    pub step_number: u32,
    pub action_type: String,        // "tool_call" | "tool_result" | "final_answer"
    pub tool_name: Option<String>,
    pub tool_input: Option<String>, // first ~200 chars of payload
    pub result_snippet: Option<String>,
    pub status: String,
}
```

Constructor at `rust/src/lib.rs:3214`. Phase 35 adds a new field:

```rust
pub tool_origin: Option<String>,  // "local" | "contextvm". None for non-tool_call steps.
```

This satisfies UI-SPEC §I and CONTEXT D-13. The Android `AgentStepItem` (`AgentScreen.kt:339–373`) and Desktop `build_step_row` (`desktop/iced/src/views/agents.rs:443–540`) gain a conditional `Remote` badge when `tool_origin == Some("contextvm")`.

The persistence side (`AgentStepRow` at `queries.rs:174`) needs a parallel `tool_origin` column — added via the new migration.

### 6. UniFFI surface
**Convention (verified):** mango_core uses **proc-macro UniFFI** — no UDL file. Records are declared with `#[derive(uniffi::Record, Clone, Debug)]` (see `lib.rs:47, 58, 79, 97, 117`). Enums use `#[derive(uniffi::Enum, ...)]` (`lib.rs:397, 433, 445`). Callback interfaces use `#[uniffi::export(callback_interface)]` (`lib.rs:703, 717, 731, 766`).

Phase 35 declares (in `lib.rs`):

```rust
#[derive(uniffi::Record, Clone, Debug)]
pub struct DiscoverableTool {
    pub id: String,                  // stable id: provider_pubkey + ":" + tool_name
    pub name: String,                // tool function name (e.g. "get_weather")
    pub description: String,
    pub provider_pubkey: String,     // hex
    pub provider_display_name: Option<String>,
    pub enabled: bool,
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum ContextvmDiscoveryState {
    Idle,
    Loading,
    Loaded,
    Error { message: String },
}
```

`AppState` gains: `contextvm_tools: Vec<DiscoverableTool>`, `contextvm_discovery_state: ContextvmDiscoveryState`, `auto_discover_tools_enabled: bool`. New `Screen::ToolDiscovery` variant added to `Screen` enum (`lib.rs:398`).

### 7. Settings sub-screen pattern (mirror targets)
**Android — `SettingsScreen.kt:127–135` and `SettingsLinkCard` at `:168–201`:**

```kotlin
// SettingsScreen.kt:127
item {
    Spacer(Modifier.height(16.dp))
    SettingsSectionLabel("Tools")
    SettingsLinkCard(
        title = "Tools",
        subtitle = if (appState.braveApiKeySet) "Web search configured" else "Web search not configured",
        onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsTools)) },
    )
}
```

**Existing canonical sub-screens to mirror layout/state:** `SettingsProvidersScreen.kt` (provider list with per-row toggles), `SettingsMemoryScreen.kt` (toggle row with subtitle), `SettingsToolsScreen.kt` (Brave key entry — has loading/error states). Mirror **`SettingsProvidersScreen.kt`** for the per-tool toggle list and **`SettingsMemoryScreen.kt`** for the "Automatically discover and use tools" toggle.

**Desktop — `desktop/iced/src/views/settings.rs:135–164` (summary row pattern) and `:344–366` (toggle row pattern):**

```rust
// desktop/iced/src/views/settings.rs:344
let memory_toggle = container(
    row![
        text("Auto-extract Memories").size(14).color(vc.text),
        iced::widget::Space::new().width(Length::Fill),
        toggler(state.memories_enabled)
            .on_toggle(Message::SettingsMemoriesEnabledToggled)
            .size(20),
    ].align_y(Alignment::Center).spacing(8),
)
.padding(Padding::from([10u16, 16]))
.width(Length::Fill)
.style(/* card style with vc.border, radius 8 */);
```

The new `desktop/iced/src/views/tool_discovery.rs` mirrors the structure of `desktop/iced/src/views/memories.rs` (loading / empty / error / list states).

### 8. Migration runner
**File:** `/home/lio/g/confidential-app/rust/src/persistence/schema.rs`. Last applied migration: **MIGRATION_V19** (Phase 34.1, attestation_cache columns). Migrations are concatenated into a `&[&str]` constant `MIGRATIONS` at `schema.rs:354–374` and applied by the migration runner in `persistence/mod.rs`.

Phase 35 adds **`MIGRATION_V20`** with the new `contextvm_tools` table (and ALTER TABLE for `agent_steps.tool_origin`) — exact schema in Section D.

---

## D. Storage shape (proposal)

Recommend **a new table for the tool list, settings-table key for the toggle, ALTER on `agent_steps` for provenance.**

### MIGRATION_V20

```sql
-- Phase 35: contextvm tool discovery state.
-- One row per discovered (provider_pubkey, tool_name) pair the user has seen.
-- Rows persist when the user toggles enabled ON; rows for disabled tools may be
-- pruned at re-discovery time (only enabled rows must round-trip across launches
-- per CTX-03).
CREATE TABLE IF NOT EXISTS contextvm_tools (
    id                    TEXT PRIMARY KEY NOT NULL,   -- "<provider_pubkey_hex>:<tool_name>"
    tool_name             TEXT NOT NULL,
    display_name          TEXT,                        -- usually NULL; tool_name is the user-facing label
    description           TEXT NOT NULL DEFAULT '',
    provider_pubkey       TEXT NOT NULL,               -- hex
    provider_display_name TEXT,                        -- NULL → fallback to "{first-8-chars}…"
    schema_json           TEXT NOT NULL,               -- inputSchema JSON blob, used by build_chat_tools
    enabled               INTEGER NOT NULL DEFAULT 0,  -- 0/1
    last_seen_at          INTEGER NOT NULL             -- unix seconds; refreshed on re-discovery
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_contextvm_tools_name
    ON contextvm_tools(tool_name);    -- used by dispatch lookup; collision policy in §E

CREATE INDEX IF NOT EXISTS idx_contextvm_tools_enabled
    ON contextvm_tools(enabled);

-- Provenance for tool-call steps: "local" or "contextvm". Pre-V20 rows store NULL
-- and the UI treats NULL as "local" (no badge). Mirrors V19's nullable-column pattern.
ALTER TABLE agent_steps ADD COLUMN tool_origin TEXT;
```

**Settings table addition (no migration needed — it's just a key insert at runtime):**
- `auto_discover_tools` — `"0"` (default) or `"1"`.
- `contextvm_secret_key` — hex-encoded Nostr secret key, lazy-created on first contextvm action.

### Why this shape
- **Per-tool persistence (CTX-03):** the table holds tool_name, schema_json, provider_pubkey, enabled. The schema_json is needed at conversation start to construct `ChatCompletionTools` entries without re-querying relays.
- **Auto-discover toggle (CTX-04):** single setting key, identical pattern to `memories_enabled`.
- **Dispatch lookup:** the unique index on `tool_name` is the routing key (see §E).
- **Provenance:** a single nullable column on `agent_steps` is enough; AgentStepSummary maps NULL → `"local"`.
- **Why not a JSON blob in settings?** Because `build_chat_tools` reads enabled tools per-conversation, and the dispatch path looks up a tool by name on every call. Both need indexed access; a JSON blob would force JSON parse-and-scan on every tool round.

### Schema collisions
The unique index on `tool_name` enforces "one provider wins per name" at storage time. The collision rule (Section E) decides which wins.

---

## E. Dispatch routing decision

**Locked recommendation: Option 2 — In-memory map of remote tool names, hydrated at conversation start.**

### Mechanism
At conversation start (just before `build_chat_tools` runs), the actor:

1. Loads the enabled `contextvm_tools` rows: `SELECT id, tool_name, schema_json, provider_pubkey FROM contextvm_tools WHERE enabled = 1` plus, if `auto_discover_tools = 1`, the union of newly-discovered tools (capped — see §F).
2. Builds an in-memory `HashMap<String, ContextvmToolDescriptor>` keyed by `tool_name`.
3. Stores it on `actor_state` as `current_conv_contextvm_tools: HashMap<String, ContextvmToolDescriptor>`.
4. `build_chat_tools(...)` is extended to accept `&[ContextvmToolDescriptor]` and append them as `ChatCompletionTools::Function` entries.

`dispatch_tools` (extended signature) consults the map:

```rust
let result = match function_name {
    "search_documents" => dispatch_search_documents(...),
    "read_document" => ...,
    // ... existing local arms ...
    name if contextvm_map.contains_key(name) => {
        let desc = &contextvm_map[name];
        dispatch_contextvm_tool(desc, args_str, runtime, signer, contextvm_map)
    }
    unknown => format!("Error: unknown tool '{}'", unknown),
};
```

### Tie-break rule
**Local tools always win over remote tools on name collision.** If a remote announcement uses `web_search`, `calculate`, `file`, `fetch_url`, `read_document`, `search_documents`, or `finish`, that remote tool is **silently filtered out** of the `tools` array and the map.

Filter implementation: `build_chat_tools` keeps a `RESERVED_LOCAL_NAMES: &[&str]` array and skips contextvm entries whose `tool_name` is in it. The Tool Discovery UI also greys out (or hides) any tool with a colliding name and shows a small note — but the simpler v1 path is to silently skip and keep only the curated local set; document the policy in PLAN.md.

### Why Option 2 over the alternatives
- **Option 1 (sentinel prefix `cvm_<id>`):** would break the user-facing tool name shown to the LLM (the LLM may not call a tool named `cvm_a3f2b1_get_weather`). Discarded.
- **Option 3 (descriptor enum):** elegant but requires touching every tool definition and the OpenAI-compatible serialization layer. The `ChatCompletionTools::Function` schema does not support custom origin metadata anyway — the LLM only sees `name`/`description`/`parameters`. Discarded.
- Option 2 keeps existing local tool code 100% untouched, and the lookup is O(1) per call.

### Where the map lives
Add to `ActorState` (`lib.rs:1149-1200` area):

```rust
/// Phase 35: in-memory map of remote tool names → descriptors, hydrated at
/// LoadConversation / NewConversation. None when no contextvm tools are active.
current_conv_contextvm_tools: HashMap<String, agent::contextvm::ContextvmToolDescriptor>,
```

Reset on `LoadConversation`, `NewConversation`, `SetActiveBackend`, and after `SetContextvmToolEnabled` (re-hydrate).

---

## F. Auto-discover heuristic

**Locked v1 behavior:**

- **When the toggle is on, run a discovery query at `conversation start`** (specifically: in `LoadConversation` and `NewConversation` handlers, and at the top of `do_send_message` if the map is empty for the active conversation). One query per conversation lifecycle, not per turn — keeps token budget bounded and battery cost low.
- **The result is the union of all announced tool schemas, capped to `MAX_AUTO_DISCOVER_TOOLS = 8`.** Sort order: by `last_seen_at` DESC (most recently announced first). Tie-break alphabetical on `tool_name`.
- **Send to the LLM via the existing `tools` array assembly.** `build_chat_tools` gains a new boolean parameter `auto_discover: bool`; when true, the auto-discovered set is appended to the user-enabled set, deduplicated by `tool_name`.
- **Cap rationale:** 8 schemas at ~150 tokens each = ~1200 tokens of system prompt overhead. Acceptable. Anything more risks crowding out user context.
- **Filtering:** apply the `RESERVED_LOCAL_NAMES` filter (Section E) to auto-discovered tools too. No keyword-match filtering against message text in v1 — the planner deliberately picks the simplest version. Document in PLAN.md as discretion choice.

### Cache and refresh
- Auto-discovered results from a query are written to `contextvm_tools` with `enabled = 0` and the tool ID in the form `<pubkey>:<name>` (same as manually-enabled tools). They are NOT auto-toggled to `enabled = 1` — the auto-discover path uses a separate "in-conversation only" overlay.
- Implementation hint: keep the auto-discover result in `actor_state.auto_discovery_buffer: HashMap<String, ContextvmToolDescriptor>` separate from the persisted enabled set; merge both into `current_conv_contextvm_tools` at conversation start. Persistence of auto-discovered rows is a nice-to-have ("last_seen_at refresh") but not required for CTX-04.

### Failure mode
If auto-discover is on and the discovery query fails (relays unreachable), the conversation proceeds with **only** the user's manually-enabled tools. Surface a one-time toast (`Couldn't reach relays`) but never block message send. Aligns with CONTEXT D-11.

---

## G. Error matrix

| Failure mode | Rust core behavior | UI (UI-SPEC reference) |
|---|---|---|
| **Relay WebSocket can't connect** (network down / DNS fail) | `RelayPool::connect` returns `Err`; emit `ContextvmDiscoveryState::Error { message }`. No retry inside the actor. | UI-SPEC §E (error state). Headline: `Couldn't reach relays`. Body: `Check your connection and try again.`. Button: `Try again`. (Locked copy from UI-SPEC §E.) |
| **Relay connects but returns no tools** | `discover_servers` / `discover_tools_typed` returns `Ok(vec![])`. Emit `ContextvmDiscoveryState::Loaded` with empty `contextvm_tools`. | UI-SPEC §D (empty state). Headline: `No tools found`. Body: `Tools advertised on Nostr will appear here.`. Button: `Try again`. |
| **Tool announcement parses but is malformed** (missing required field) | `discover_tools_typed` already filters / errors per-event; if the typed parse fails for one event, log via `tracing` and skip. Don't fail the whole query. Other valid tools still surface. | Whatever loaded successfully renders normally (UI-SPEC §F). No special UI for skipped tools in v1. |
| **Invocation request times out** (no response within 30s) | `tokio::time::timeout(Duration::from_secs(30), rx.recv()).await` returns `Err(_)` or `Ok(None)`. Tool result string: `"Error: tool '<name>' timed out (30s)"`. The LLM reads this on its next round, same as any tool error. | The Agent step / chat tool summary shows the timeout result text. The "Remote" badge (UI-SPEC §I) still renders because `tool_origin == "contextvm"`. No additional UI surface in v1 (consistent with how local tool errors are surfaced today). |
| **Invocation returns a JSON-RPC error** (provider returns `{ error: ... }`) | Format the error as `"Error: <code>: <message>"` in the result string. Same path as timeout. | Same as timeout. |
| **Invocation response payload too large** | Truncate response text to `MAX_TOOL_RESULT_BYTES = 16_384` bytes (matches existing local tool truncation in `dispatch_fetch_url`). Append `... [truncated]`. | None. The truncated string is what the LLM sees and what `result_snippet` shows in the agent step UI. |
| **Provider pubkey for a tool is no longer reachable** (server stopped announcing) | Discovery returns the tool with `last_seen_at` from a previous query; user toggles enabled; invocation hits relay → relay subscribes → no event from that pubkey → 30s timeout → falls through to "timeout" path. | Same as timeout. v1 does not auto-prune unreachable tools. |

**Single shared error-result helper:** add `fn contextvm_error_result(tool_name: &str, msg: &str) -> String` in `agent/contextvm.rs` to keep the format consistent across all paths.

---

## H. Cross-platform notes

### Android
- **WebSocket lifecycle across Activity rotation:** the actor thread (Rust) lives in the Application/process scope, not the Activity. Per RMP architecture, rotation does not affect the actor. New `RelayPool` connections are created per discovery query and explicitly disconnected — no long-lived sockets to manage.
- **WorkManager:** **not needed.** All contextvm operations are foreground, user-driven, and complete in seconds. The auto-discover query runs at conversation-start (a foreground action). No background pull.
- **App backgrounding mid-invocation:** the actor's tokio runtime continues for a brief window; if the OS kills the process before the response arrives, the conversation is in a "tool call pending" state and resumes at next foreground via the existing chat-tool round restart logic (already handled by the existing chat-tool round at `lib.rs:7790+` for the local Brave call — same pattern applies). Confirm during PLAN execution.
- **Doze mode:** affects long-lived TCP sockets, but Phase 35 doesn't keep any. Each query opens fresh.

### Desktop (iced)
- **Reactor lifecycle:** the existing tokio runtime in `actor_state.runtime` is used for `runtime.block_on` calls in `dispatch_*` functions. Contextvm dispatch follows the same pattern: `runtime.block_on(invoke_remote(desc, args, signer))`. No new reactor needed.
- **Adding/removing relays at runtime:** v1 uses a hardcoded list, so this isn't a concern. (Deferred per CONTEXT.)
- **Cold-start to first response time:** discovery query opens N WebSockets in parallel (rustls handshakes), waits for END-OF-STORED-EVENTS or a timeout, then closes. Expect 1–3 seconds typical, 5+ seconds worst case. UI must show the loading state (UI-SPEC §C).

### Mobile builds — OpenSSL audit
**Verified by running `cargo tree` in an isolated `/tmp/testdep` Cargo project containing only `contextvm-sdk = "0.1.0"`:**

```
$ cd /tmp/testdep && cargo tree | grep -iE "openssl-sys|native-tls"
(no output)
```

**No `openssl-sys`, no `native-tls` is introduced by contextvm-sdk's transitive tree.** All TLS goes through `rustls 0.23.40` via `tokio-rustls 0.26` and `tokio-tungstenite 0.26` (rustls feature).

The pre-existing `openssl-sys 0.9.113` in mango_core's tree is from `rusqlite 0.39` with `bundled-sqlcipher-vendored-openssl` (compile-time vendored bundle for SQLCipher; Phase 28 baseline). Phase 35 does not regress this.

**Required smoke test in PLAN execution:**
```
cargo tree -p mango_core 2>&1 | grep -iE "openssl-sys|native-tls"
```
Expected: only the existing `openssl-sys 0.9.113` line under `libsqlite3-sys`. If a new `openssl-sys` edge appears under nostr-sdk / contextvm-sdk / rmcp / async-wsocket, **stop** and re-audit.

Also required (per CLAUDE.md cross-compile guarantee):
- `cargo build --target aarch64-apple-ios -p mango_core`
- `cargo ndk -t arm64-v8a build -p mango_core`
- `cargo build -p mango_core` (host Linux)

If iOS or Android target shows a build break, the most likely culprit is `rmcp`'s `transport-worker` feature — try `default-features = false, features = ["rmcp"]` minus transport-worker, or open an upstream issue.

---

## Validation Architecture (Nyquist)

Phase 35 needs validation across four layers. PLAN.md must enumerate each as an acceptance criterion.

### Unit tests (Rust core)

**`agent::contextvm::dispatch` module:**
- `test_dispatch_routes_local_first_on_collision` — register a remote tool named `web_search`; dispatch a `web_search` call; assert local handler runs (RESERVED_LOCAL_NAMES rule from §E).
- `test_dispatch_unknown_tool_falls_back_to_remote_when_present` — register remote `get_weather`; dispatch a `get_weather` call; assert remote handler is invoked.
- `test_dispatch_unknown_tool_returns_error_when_no_remote` — assert "unknown tool" error string for a name in neither set.
- `test_invocation_timeout_returns_error_string` — mock proxy that never responds; assert `Error: tool '...' timed out (30s)` after timeout.
- `test_invocation_jsonrpc_error_returns_error_string` — mock proxy returning `{ error: ... }`; assert formatted error.
- `test_oversized_response_truncated` — 32 KiB response; assert truncated to 16 KiB plus `... [truncated]`.

**`persistence::queries::contextvm_tools`:**
- `test_migration_v20_creates_contextvm_tools_table` — pattern from `schema.rs:380` (`test_migration_v18_creates_directory_tables`).
- `test_migration_v20_adds_tool_origin_to_agent_steps` — pattern from `schema.rs:439` (V19 columns test).
- `test_unique_index_on_tool_name` — insert duplicate name → expect UNIQUE constraint error.
- `test_round_trip_enabled_tool` — insert, query enabled, round-trip schema_json.

**`build_chat_tools_with_contextvm`:**
- `test_remote_tools_appended_to_chat_tools` — local set + 3 remote enabled → 10 entries.
- `test_collision_filtered_out` — remote tool named `calculate` is filtered.
- `test_auto_discover_capped_at_8` — feed 20 announced tools, expect 8 in result.

### Integration tests (actor + UniFFI)

- `test_set_contextvm_tool_enabled_persists` — fire `AppAction::SetContextvmToolEnabled`, restart actor, verify enabled in fresh `AppState.contextvm_tools`.
- `test_set_auto_discover_tools_persists` — fire `AppAction::SetAutoDiscoverTools { enabled: true }`, restart, verify `AppState.auto_discover_tools_enabled = true`.
- `test_discovery_loading_then_loaded_state_transition` — push `Screen::ToolDiscovery`, fire `AppAction::DiscoverContextvmTools`, mock relay, assert `Loading` → `Loaded` state transition with rev increment.
- `test_discovery_error_state_on_unreachable_relay` — point at `wss://localhost:1` (unroutable); assert `Error { message }` state. (UI-SPEC §E coverage.)
- `test_load_conversation_hydrates_contextvm_map` — enable two tools, call `LoadConversation`, assert `current_conv_contextvm_tools` contains them.

### UI tests (Compose / iced)

**Android (Compose UI tests under `android/app/src/androidTest`):**
- `SettingsScreenTest::tools_section_shows_discover_row` — assert "Discover tools" row with `No tools enabled` subtitle (UI-SPEC §A).
- `SettingsScreenTest::tools_section_shows_auto_discover_toggle` — assert "Automatically discover and use tools" row with subtitle copy (UI-SPEC §B).
- `ToolDiscoveryScreenTest::loading_shows_spinner_and_subtitle` — UI-SPEC §C.
- `ToolDiscoveryScreenTest::empty_shows_no_tools_found` — UI-SPEC §D.
- `ToolDiscoveryScreenTest::error_shows_couldnt_reach_relays` — UI-SPEC §E with destructive-color heading.
- `ToolDiscoveryScreenTest::success_renders_list_with_toggles` — UI-SPEC §F.
- `AgentScreenTest::remote_badge_renders_for_contextvm_origin` — UI-SPEC §I.

**Desktop (iced unit tests in `desktop/iced/src/views/`):**
- `tool_discovery_test::renders_loading_state` — call `view()` with `Loading` state, assert it returns the centred subtitle text.
- `tool_discovery_test::renders_error_state` — assert `Couldn't reach relays` text appears.
- `tool_discovery_test::renders_empty_state` — assert `No tools found` text.
- `agents_test::remote_badge_visible_only_when_origin_contextvm` — direct assertion on `build_step_row` output.

### End-to-end (live relay)

A single happy-path e2e test in `rust/src/tests/`:

- `e2e_discover_then_invoke` — connect to a public relay set, query for a known echo tool (or stand up a tiny local MCP server during test setup), enable it, invoke `tools/call`, assert response. **Marked `#[ignore]` by default** (network-dependent, flaky in CI). Runnable manually as `cargo test --test e2e -- --ignored e2e_discover_then_invoke`.

### UI-SPEC state coverage map
- §A subtitle states (0/1/N enabled) → unit tests on a pure helper `tool_discovery_subtitle(n: usize) -> String`.
- §B toggle subtitle → static string assertion.
- §C/D/E/F/G/H/I → all covered by the UI tests above.

---

## Open Questions

1. **Encryption mode policy.** `EncryptionMode::Optional` is what the SDK example uses. If a provider advertises `support_encryption` and *requires* encryption, our request must be gift-wrapped. Phase 35 v1 should always send Optional and document that encryption-required providers may fail until a v2 phase wires NIP-59 gift-wrap. **Recommendation:** planner picks Optional for v1, surfaces an "encryption-required" failure as a regular invocation error in the matrix above, and files a follow-up backlog item.

2. **Nostr key persistence keychain vs settings.** The Nostr secret key is low-value (it identifies the *client* to public relays, not a user secret), and the SQLCipher-encrypted settings table is the simplest place. Planner could push it to the platform keychain for parity with backend API keys. **Recommendation:** v1 stores in the encrypted settings table under key `contextvm_secret_key`; defer keychain promotion.

3. **rmcp `transport-worker` feature on iOS.** Untested. The planner should run `cargo build --target aarch64-apple-ios -p mango_core` early in PLAN execution. If it fails, try `contextvm-sdk = { version = "0.1.0", default-features = false, features = ["rmcp"] }` and surgically reduce rmcp features. If it still fails, surface to user via `/gsd-discuss-phase` for decision (downgrade rmcp version vs. iOS-deferred).

4. **Tool name collision UX.** Section E filters silently; the alternative is to disable the row in the Tool Discovery list and show "name conflicts with built-in tool" as a per-row state. UI-SPEC doesn't cover this. **Recommendation:** v1 silently filters; planner can add a UX-SPEC amendment if user testing surfaces confusion.

5. **Provider display name source.** The protocol allows providers to set a `name` tag on the kind 11316 announcement (Server Announcement). The `ServerAnnouncement.server_info.name` field appears to capture this. **Recommendation:** use `server_info.name` if present, else fall back to `pubkey[..8] + "…"` per UI-SPEC §F. Planner verifies the field is reliably populated by sample providers during smoke testing.

6. **Handling N>1 providers per tool name.** Two providers may announce the same tool name. The unique index in §D forces last-write-wins on re-discovery. Acceptable for v1. Planner may surface a small "(duplicate)" badge in the discovery list if needed; otherwise document as a known v1 limitation.
