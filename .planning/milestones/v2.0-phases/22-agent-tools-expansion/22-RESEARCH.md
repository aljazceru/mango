# Phase 22: Agent Tools Expansion - Research

**Researched:** 2026-04-04
**Domain:** Rust agent tool dispatch, Brave Search API, HTML scraping, sandboxed file I/O, expression evaluation
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TOOL-01 | Agent can search the web using Brave Search API and return results | Brave API endpoint, reqwest (already in deps), API key via `settings` table |
| TOOL-02 | Agent can fetch and read content from URLs (HTML parsed to text) | `scraper` 0.26.0 crate for HTML-to-text; `reqwest` already in deps |
| TOOL-03 | Agent can create, read, and edit files in the app sandbox | `std::fs` (sync, fits actor thread); sandbox = `{data_dir}/agent_files/`; path traversal guard |
| TOOL-04 | Agent can evaluate mathematical expressions with precision | `evalexpr` 13.1.0 crate; pure sync, no new transitive deps |
| TOOL-05 | Agent tool dispatch integrates with existing ReAct loop and step checkpointing | Extend existing `dispatch_tools` fn + `build_agent_tools` fn in `rust/src/agent/tools.rs` |
</phase_requirements>

---

## Summary

Phase 22 adds four new tools to the existing ReAct agent loop: web search, URL fetching, file I/O, and math evaluation. The existing agent infrastructure (`rust/src/agent/tools.rs`, `handle_agent_step_complete` in `lib.rs`) is already wired for step checkpointing — each `AgentStepRow` with `action_type = "tool_call"` and `action_payload` recording tool name and arguments is written before dispatch. The four new tools plug into `build_agent_tools()` (tool schemas) and `dispatch_tools()` (execution) without architectural changes.

The primary complexity is that `dispatch_tools` is currently synchronous and the two network-based tools (web search, URL fetch) need async HTTP. The established pattern in this codebase is `runtime.block_on()` for calling async code from the actor thread's synchronous path — this is valid because the actor thread owns the `tokio::runtime::Runtime` and is NOT itself running inside an async context. The `runtime` handle must be passed into `dispatch_tools` (adding it to the signature).

Brave Search API key is stored in the `settings` SQLite table under key `brave_api_key` (same pattern used for `global_system_prompt`, `attestation_interval_minutes`). The `data_dir` must be threaded into `dispatch_tools` to resolve the agent sandbox directory for file I/O.

**Primary recommendation:** Extend `dispatch_tools` to accept `&tokio::runtime::Runtime`, `&str` (data_dir), and `&str` (brave_api_key). Add four new tool schemas and four new dispatch arms. No new actor event types needed — checkpointing is handled by the existing `action_type = "tool_call"` path in `handle_agent_step_complete`.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `reqwest` | 0.12.x (already in deps) | HTTP for Brave Search API calls and URL fetching | Already in Cargo.toml with `rustls-tls-webpki-roots` + `json` features; cross-platform, no OpenSSL |
| `scraper` | 0.26.0 | HTML parsing and text extraction for TOOL-02 | Industry standard Rust HTML parser (wraps servo's html5ever); 15.5M downloads; ISC license; published 2026-03-18 |
| `evalexpr` | 13.1.0 | Mathematical expression evaluation for TOOL-04 | 6.9M downloads; actively maintained; pure Rust, zero transitive deps; supports arithmetic, trig, float/int auto-conversion |
| `std::fs` | stdlib | File I/O for TOOL-03 | Synchronous — fits actor thread model; no new dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde_json` | 1.x (already in deps) | Parse Brave API JSON response | Already used throughout |
| `tokio` | 1.x (already in deps) | `runtime.block_on()` for async HTTP inside sync dispatch | Actor thread owns the runtime; block_on is valid from non-async context |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `scraper` | `html2text` crate | `html2text` produces cleaner "reading view" output but is less maintained; `scraper` is better maintained and more widely used |
| `scraper` | `nipper` or `select` | Both have fewer downloads and less maintenance; `scraper` is the clear standard |
| `evalexpr` | `meval` 0.2.0 | `meval` is unmaintained (last release 2018); `evalexpr` is actively developed |
| `runtime.block_on()` | Refactor `dispatch_tools` to async | Async refactor would require changing `handle_agent_step_complete` in `lib.rs` significantly; `block_on` is correct and simpler |

**Installation:**
```toml
# Add to rust/Cargo.toml [dependencies]
scraper = "0.26"
evalexpr = "13.1"
```

**Version verification (confirmed via crates.io API 2026-04-04):**
- `scraper` 0.26.0 — published 2026-03-18
- `evalexpr` 13.1.0 — published 2025-11-26, not yanked

---

## Architecture Patterns

### Recommended Project Structure

No new files or modules needed. All changes are within:
```
rust/src/agent/
├── tools.rs      # Add 4 tool schemas to build_agent_tools(), 4 dispatch arms
└── mod.rs        # No changes needed
rust/src/
└── lib.rs        # Pass runtime + data_dir + brave_api_key to dispatch_tools
```

One new setting key in SQLite (no migration needed — `settings` table exists):
- `brave_api_key` stored via `set_setting(conn, "brave_api_key", key)` (existing mechanism)

### Pattern 1: Extending `build_agent_tools()`

Add four new `ChatCompletionTools::Function` entries to the `vec![]` returned by `build_agent_tools()`. The existing three tools remain unchanged.

```rust
// Source: existing tools.rs pattern
ChatCompletionTools::Function(ChatCompletionTool {
    function: FunctionObject {
        name: "web_search".to_string(),
        description: Some("Search the web using Brave Search. Returns titles, URLs, and descriptions for the top results.".to_string()),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" },
                "count": { "type": "integer", "description": "Number of results (default 5, max 10)", "minimum": 1, "maximum": 10 }
            },
            "required": ["query"]
        })),
        strict: None,
    },
}),
```

### Pattern 2: `dispatch_tools` Signature Extension

Add three new parameters — the runtime handle, data_dir, and brave_api_key — to `dispatch_tools`:

```rust
// Updated signature in rust/src/agent/tools.rs
pub fn dispatch_tools(
    calls: &[ChatCompletionMessageToolCall],
    db_conn: &rusqlite::Connection,
    vector_index: &VectorIndex,
    embedding_provider: &dyn EmbeddingProvider,
    runtime: &tokio::runtime::Runtime,   // new
    data_dir: &str,                       // new: for sandbox path
    brave_api_key: &str,                  // new: from settings table
) -> Vec<(String, String)>
```

The two call sites in `lib.rs` (`handle_agent_step_complete` line ~1634 and `handle_resume_agent_session` line ~1956) must be updated accordingly. The `actor_state` already has access to:
- `actor_state.runtime` — the Tokio runtime
- `data_dir` — must be added to `ActorState` (currently not stored; only used at init time)

### Pattern 3: Storing `data_dir` in `ActorState`

`data_dir` is currently computed before the actor thread spawns and then discarded (only used for `db_path` and `vector_data_dir`). It must be stored in `ActorState` for the file tool:

```rust
// In struct ActorState (lib.rs)
data_dir: String,   // new field — empty string means in-memory/test mode

// In ActorState initialization:
let mut actor_state = ActorState {
    // ... existing fields ...
    data_dir: vector_data_dir.clone(),  // reuse the already-computed copy
};
```

### Pattern 4: Brave Search HTTP Call (TOOL-01)

```rust
// Source: Brave Search API quickstart (HIGH confidence)
// Endpoint: GET https://api.search.brave.com/res/v1/web/search
// Auth header: X-Subscription-Token
fn dispatch_web_search(
    args_str: &str,
    runtime: &tokio::runtime::Runtime,
    brave_api_key: &str,
) -> String {
    if brave_api_key.is_empty() {
        return "Error: Brave Search API key not configured. Set brave_api_key in settings.".to_string();
    }
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error parsing args: {}", e),
    };
    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q.to_string(),
        None => return "Error: web_search requires 'query' parameter".to_string(),
    };
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(5).min(10);

    // reqwest .query() handles URL encoding automatically
    let result = runtime.block_on(async {
        reqwest::Client::new()
            .get("https://api.search.brave.com/res/v1/web/search")
            .query(&[("q", query.as_str()), ("count", &count.to_string())])
            .header("X-Subscription-Token", brave_api_key)
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await
    });
    match result {
        Ok(json) => format_brave_results(&json),
        Err(e) => format!("Error: web search failed: {}", e),
    }
}
```

**Response structure (MEDIUM confidence — from official Brave docs):**
```json
{
  "web": {
    "results": [
      { "title": "...", "url": "...", "description": "..." }
    ]
  }
}
```
Extract `json["web"]["results"]` as a JSON array for the tool result.

### Pattern 5: URL Fetch + HTML Strip (TOOL-02)

```rust
// Source: scraper 0.26.0 docs.rs (HIGH confidence)
fn dispatch_fetch_url(args_str: &str, runtime: &tokio::runtime::Runtime) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error parsing args: {}", e),
    };
    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.to_string(),
        None => return "Error: fetch_url requires 'url' parameter".to_string(),
    };

    let fetch_result = runtime.block_on(async {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap()
            .get(&url)
            .send()
            .await?
            .text()
            .await
    });

    match fetch_result {
        Ok(html) => {
            use scraper::{Html, Selector};
            let document = Html::parse_document(&html);
            let body_selector = Selector::parse("body").unwrap();
            let text = if let Some(body) = document.select(&body_selector).next() {
                body.text().collect::<Vec<_>>().join(" ")
            } else {
                document.root_element().text().collect::<Vec<_>>().join(" ")
            };
            // Truncate to avoid context overflow
            if text.len() > 8000 {
                format!("{}... [truncated at 8000 chars]", &text[..8000])
            } else {
                text
            }
        }
        Err(e) => format!("Error: failed to fetch '{}': {}", url, e),
    }
}
```

### Pattern 6: Sandboxed File I/O (TOOL-03)

```rust
// All file operations canonicalize paths and reject traversal attempts
fn resolve_sandbox_path(data_dir: &str, relative_path: &str) -> Result<std::path::PathBuf, String> {
    if data_dir.is_empty() {
        return Err("Error: file tool unavailable in test/in-memory mode".to_string());
    }
    // Reject obvious traversal attempts early
    if relative_path.contains("..") {
        return Err(format!("Error: path '{}' contains '..' which is not allowed", relative_path));
    }
    let sandbox = std::path::Path::new(data_dir).join("agent_files");
    std::fs::create_dir_all(&sandbox)
        .map_err(|e| format!("Error: sandbox init failed: {}", e))?;
    let candidate = sandbox.join(relative_path);
    // Additional check via starts_with after join
    if !candidate.starts_with(&sandbox) {
        return Err(format!("Error: path escapes sandbox"));
    }
    Ok(candidate)
}
```

Three sub-operations in one tool via `operation` parameter:
- `read`: `std::fs::read_to_string(path)`
- `write`: `std::fs::write(path, content)` (creates or overwrites)
- `append`: `std::fs::OpenOptions::new().append(true).create(true).open(path)`

### Pattern 7: Math Expression Evaluation (TOOL-04)

The `evalexpr` crate's top-level evaluate function parses and runs an arithmetic/boolean expression string.

```rust
// Source: evalexpr 13.1.0 docs.rs (HIGH confidence)
fn dispatch_calculate(args_str: &str) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error parsing args: {}", e),
    };
    let expression = match args.get("expression").and_then(|v| v.as_str()) {
        Some(e) => e.to_string(),
        None => return "Error: calculate requires 'expression' parameter".to_string(),
    };
    // Guard against overly long expressions (DoS protection)
    if expression.len() > 200 {
        return "Error: expression too long (max 200 chars)".to_string();
    }
    match evalexpr::evaluate(&expression) {
        Ok(value) => value.to_string(),
        Err(e) => format!("Error: math evaluation failed: {}", e),
    }
}
```

Note: The tool name in the LLM-facing schema should be `calculate` or `math` (not `eval_math`) to avoid confusion with code execution. The actual evalexpr function called is `evalexpr::evaluate()` (the crate also exposes an `eval` alias, but prefer the explicit function name).

### Anti-Patterns to Avoid

- **Async dispatch_tools:** Do not refactor `dispatch_tools` to be `async`. The function is called synchronously on the actor thread. Use `runtime.block_on()` for the network calls instead.
- **Unguarded file paths:** Never pass `relative_path` directly to `std::fs`. Always resolve through the sandbox and verify `starts_with(sandbox_dir)`.
- **No timeout on HTTP:** Always set a `reqwest::Client` timeout (15s) for URL fetch; Brave search response is fast but URL fetch can be slow.
- **Returning large HTML responses verbatim:** Always truncate URL fetch results to a reasonable limit (8000 chars).
- **Ignoring Brave API key absence:** Return a clear error string (not a panic) when `brave_api_key` is empty.
- **Using HTML root text directly:** `document.root_element().text()` includes script/style text nodes. Use a `body` selector instead.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTML parsing and tag removal | Custom regex HTML stripper | `scraper` crate | HTML has edge cases (self-closing tags, CDATA, embedded SVG) that regex cannot handle reliably |
| Math expression parsing | Custom expression parser | `evalexpr` | Operator precedence, parentheses, built-in functions are non-trivial; evalexpr is battle-tested |
| Path traversal prevention | Custom string manipulation | `Path::starts_with()` after `join()` | Platform path normalization (e.g., `..` on Windows vs Unix) requires the OS path APIs |

**Key insight:** The HTML and math tools have enough edge cases that hand-rolling would introduce bugs; use the established crates.

---

## Common Pitfalls

### Pitfall 1: `block_on` Inside Async Context
**What goes wrong:** Calling `runtime.block_on()` from within an `async` context panics with "Cannot start a runtime from within a Tokio runtime."
**Why it happens:** `dispatch_tools` is called from `handle_agent_step_complete` which runs on the actor thread's synchronous loop — NOT inside an async block. This is safe. But if someone moves the dispatch call into a `runtime.spawn(async { ... })` future, it will panic.
**How to avoid:** Keep `dispatch_tools` synchronous. Verify the call site is in the synchronous actor loop, not inside a spawned async task.
**Warning signs:** Runtime panic with "Cannot start a runtime from within a Tokio runtime".

### Pitfall 2: Path Traversal via `../` in File Tool
**What goes wrong:** Agent passes `../../etc/passwd` as a file path. Without canonicalization, `sandbox.join("../../etc/passwd")` resolves outside the sandbox.
**Why it happens:** `Path::join()` does not check for `..` components.
**How to avoid:** Reject any `relative_path` containing `..` immediately, before `join()`. Additionally verify `candidate.starts_with(sandbox)`.
**Warning signs:** Any path containing `..` should be rejected with a clear error message.

### Pitfall 3: Brave API Key Not Available in `dispatch_tools`
**What goes wrong:** The Brave API key is in SQLite settings but `dispatch_tools` only receives `db_conn`. Current callers don't query settings before calling dispatch.
**Why it happens:** The caller (`handle_agent_step_complete`) would need to look up `brave_api_key` from the settings table before invoking dispatch.
**How to avoid:** Query `get_setting(actor_state.db.conn(), "brave_api_key")` in `handle_agent_step_complete` before calling `dispatch_tools`, and pass the result as a `&str` parameter (empty string if not set).
**Warning signs:** Web search always returns "Error: API key not configured" even when key is set.

### Pitfall 4: Scraper Text Includes Script/Style Content
**What goes wrong:** `document.root_element().text()` returns ALL text nodes including the contents of `<script>` and `<style>` tags.
**Why it happens:** `scraper`'s `.text()` iterator is a flat walk of all text nodes regardless of element type.
**How to avoid:** Use `Selector::parse("body")` to select only the body element and call `.text()` on that. This skips `<head>` which contains most `<script>` and `<style>` tags.
**Warning signs:** HTML fetch results containing large blobs of JavaScript or CSS in the extracted text.

### Pitfall 5: Expression DoS in Math Tool
**What goes wrong:** An agent passes `2^2^2^2^2` which evaluates to `2^65536` — a computation that may not terminate quickly.
**Why it happens:** `evalexpr` does not have built-in computation budget limits.
**How to avoid:** Limit expression length (reject expressions > 200 chars) before calling `evalexpr::evaluate()`.
**Warning signs:** The math tool dispatch call hanging indefinitely.

### Pitfall 6: `data_dir` Not Stored in `ActorState`
**What goes wrong:** The file tool cannot resolve the sandbox directory because `data_dir` is consumed at actor initialization and not stored.
**Why it happens:** In `FfiApp::new`, `data_dir` is used to compute `db_path` and `vector_data_dir`, then the thread takes ownership. The `ActorState` struct does not currently store `data_dir`.
**How to avoid:** Add `data_dir: String` to `ActorState`. Populate it in the actor initialization block using the existing `vector_data_dir` clone. Pass `&actor_state.data_dir` to `dispatch_tools`.

---

## Code Examples

### Updated `dispatch_tools` Match Arms

```rust
// In rust/src/agent/tools.rs — dispatch match block
let result = match function_name {
    "search_documents" => dispatch_search_documents(args_str, db_conn, vector_index, embedding_provider),
    "read_document"    => dispatch_read_document(args_str, db_conn),
    "finish"           => dispatch_finish(args_str),
    // Phase 22 additions:
    "web_search"       => dispatch_web_search(args_str, runtime, brave_api_key),
    "fetch_url"        => dispatch_fetch_url(args_str, runtime),
    "file"             => dispatch_file(args_str, data_dir),
    "calculate"        => dispatch_calculate(args_str),
    unknown            => format!("Error: unknown tool '{}'", unknown),
};
```

### Brave API Response Formatting

```rust
fn format_brave_results(json: &serde_json::Value) -> String {
    let results = json
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);

    if results.is_empty() {
        return "No results found".to_string();
    }

    let items: Vec<serde_json::Value> = results.iter().map(|r| {
        serde_json::json!({
            "title": r.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            "url": r.get("url").and_then(|v| v.as_str()).unwrap_or(""),
            "description": r.get("description").and_then(|v| v.as_str()).unwrap_or("")
        })
    }).collect();

    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "Error serializing results".to_string())
}
```

### Caller Update in `handle_agent_step_complete`

```rust
// In lib.rs handle_agent_step_complete, before dispatch_tools call:
let brave_api_key = persistence::queries::get_setting(actor_state.db.conn(), "brave_api_key")
    .unwrap_or(None)
    .unwrap_or_default();

let tool_results = agent::dispatch_tools(
    &calls,
    actor_state.db.conn(),
    &actor_state.vector_index,
    actor_state.embedding_provider.as_ref(),
    &actor_state.runtime,         // new
    &actor_state.data_dir,        // new
    &brave_api_key,               // new
);
```

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Brave Search API key | TOOL-01 | Runtime config | N/A | Return error message "API key not configured" |
| `reqwest` (network) | TOOL-01, TOOL-02 | Already in deps | 0.12.x | N/A — already available |
| Rust stable toolchain | Build | Available | See flake.nix | N/A |
| `scraper` crate | TOOL-02 | Not in deps yet | 0.26.0 | N/A — must add to Cargo.toml |
| `evalexpr` crate | TOOL-04 | Not in deps yet | 13.1.0 | N/A — must add to Cargo.toml |

**Missing dependencies with no fallback:**
- `scraper = "0.26"` — must be added to `rust/Cargo.toml`
- `evalexpr = "13.1"` — must be added to `rust/Cargo.toml`

**Missing dependencies with fallback:**
- Brave API key — soft failure with clear error string; agent can still use other tools

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`cargo test`) |
| Config file | None (workspace uses `cargo test`) |
| Quick run command | `cargo test -p mango_core agent -- --nocapture` |
| Full suite command | `cargo test -p mango_core` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TOOL-01 | `web_search` tool schema exists with correct parameters | unit | `cargo test -p mango_core test_agent_tools_include_web_search` | Wave 0 |
| TOOL-01 | `dispatch_web_search` returns error when API key empty | unit | `cargo test -p mango_core test_web_search_no_api_key_returns_error` | Wave 0 |
| TOOL-01 | Live Brave search returns results (ignored) | live | `cargo test -p mango_core test_live_web_search -- --ignored` | Wave 0 |
| TOOL-02 | `fetch_url` tool schema exists with correct parameters | unit | `cargo test -p mango_core test_agent_tools_include_fetch_url` | Wave 0 |
| TOOL-02 | `dispatch_fetch_url` strips HTML to plain text | unit | `cargo test -p mango_core test_fetch_url_html_stripped` | Wave 0 |
| TOOL-02 | `dispatch_fetch_url` returns error for unreachable URL | unit | `cargo test -p mango_core test_fetch_url_unreachable_returns_error` | Wave 0 |
| TOOL-03 | `file` tool schema exists with read/write/append operations | unit | `cargo test -p mango_core test_agent_tools_include_file` | Wave 0 |
| TOOL-03 | `dispatch_file` write then read roundtrip works | unit | `cargo test -p mango_core test_file_write_read_roundtrip` | Wave 0 |
| TOOL-03 | `dispatch_file` rejects path traversal | unit | `cargo test -p mango_core test_file_path_traversal_rejected` | Wave 0 |
| TOOL-04 | `calculate` tool schema exists | unit | `cargo test -p mango_core test_agent_tools_include_calculate` | Wave 0 |
| TOOL-04 | `dispatch_calculate` evaluates basic expressions correctly | unit | `cargo test -p mango_core test_calculate_basic` | Wave 0 |
| TOOL-04 | `dispatch_calculate` handles invalid expressions without panic | unit | `cargo test -p mango_core test_calculate_invalid_no_panic` | Wave 0 |
| TOOL-05 | `build_agent_tools()` returns 7 tools (3 existing + 4 new) | unit | `cargo test -p mango_core test_agent_tools_count_seven` | Wave 0 |
| TOOL-05 | Tool dispatch arms handle all 7 known tool names | unit | `cargo test -p mango_core test_dispatch_all_known_tools` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p mango_core agent`
- **Per wave merge:** `cargo test -p mango_core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
All tests for TOOL-01 through TOOL-05 are new and must be created. Add them to `rust/src/tests/agent.rs` (the existing agent test file). File tool tests require `tempfile` (already in dev-dependencies).

- [ ] `rust/src/tests/agent.rs` — new test functions for web_search, fetch_url, file, calculate tool schemas and dispatch
- [ ] Live test tagged `#[ignore]` for Brave API (requires `BRAVE_API_KEY` env var at runtime)
- [ ] File I/O tests use `tempfile::tempdir()` for sandbox isolation (already available)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `meval` for Rust math evaluation | `evalexpr` | meval unmaintained since 2018 | evalexpr has active development, broader function support |
| `select` crate for HTML | `scraper` | 2020+ | scraper uses servo's html5ever — browser-grade parsing |

**Deprecated/outdated:**
- `meval` 0.2.0: Last release 2018, treat as abandoned. Use `evalexpr` 13.1.0.
- `nipper`: Lower maintenance, fewer downloads than `scraper`.

---

## Open Questions

1. **Brave API Key UX flow**
   - What we know: Key stored via `set_setting(conn, "brave_api_key", key)`. No UI for this exists yet.
   - What's unclear: Is Phase 22 responsible for any UI to enter the key, or is it settings-only?
   - Recommendation: Phase 22 wires the tool to read from settings; Phase 23 (agent UI re-enable) can add a settings screen entry. The REQUIREMENTS.md explicitly defers Brave key management UI ("use settings/environment for now").

2. **scraper + html5ever compile time on mobile targets**
   - What we know: `scraper` 0.26.0 depends on `html5ever` and `selectors` which are large crates from the Servo project.
   - What's unclear: Whether these compile cleanly on `aarch64-linux-android` and `aarch64-apple-ios` targets.
   - Recommendation: Verify with `cargo check --target aarch64-linux-android` in the first plan. If there are issues, fall back to simple regex-based tag stripping (acceptable given the constraint).

3. **reqwest Client reuse vs per-call creation**
   - What we know: Creating a `reqwest::Client` per tool call is correct but sacrifices connection pooling across calls.
   - What's unclear: Whether the agent loop calls tools frequently enough to benefit from a shared client.
   - Recommendation: Per-call client for Phase 22 (simpler); revisit if profiling shows overhead.

---

## Sources

### Primary (HIGH confidence)
- `rust/src/agent/tools.rs` — existing tool schema and dispatch pattern (direct codebase read)
- `rust/src/agent/loop.rs` — existing ReAct step execution (direct codebase read)
- `rust/src/lib.rs` — `ActorState`, `handle_agent_step_complete`, `dispatch_tools` call sites (direct codebase read)
- `rust/src/persistence/schema.rs` — migration history, `settings` table (direct codebase read)
- scraper 0.26.0 — https://docs.rs/scraper/0.26.0/scraper/ — text extraction API confirmed
- evalexpr 13.1.0 — https://docs.rs/evalexpr/latest/evalexpr/ — evaluate function, supported operators confirmed
- Brave Search API — https://api-dashboard.search.brave.com/documentation/quickstart — endpoint, headers, response JSON structure confirmed
- crates.io API — version/publish-date verification for scraper 0.26.0 and evalexpr 13.1.0

### Secondary (MEDIUM confidence)
- Brave Search API response JSON structure — confirmed endpoint `https://api.search.brave.com/res/v1/web/search`, header `X-Subscription-Token`, fields `web.results[].{title,url,description}`
- `meval` abandonment — crates.io shows last release 2018; no recent commits visible

### Tertiary (LOW confidence)
- Mobile compile behavior of `scraper` / `html5ever` on iOS/Android targets — not verified against cross-compilation; flagged for first build

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — scraper and evalexpr versions verified via crates.io API; reqwest already proven in codebase
- Architecture: HIGH — based on direct codebase reading; `dispatch_tools` extension pattern is unambiguous
- Pitfalls: HIGH for path traversal and block_on; MEDIUM for Brave rate limits (not researched in depth)

**Research date:** 2026-04-04
**Valid until:** 2026-07-04 (stable crates; Brave API endpoint may update)
