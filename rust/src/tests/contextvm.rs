//! Phase 35 — contextvm-sdk integration test stubs.
//!
//! Each `ctx_NN_*` stub maps to one CTX-NN requirement in ROADMAP Phase 35.
//! Stubs are `#[ignore]`-gated until their owning plan implements the
//! production code they assert against. As each plan lands, it un-ignores
//! the matching stub and replaces the body with the real assertion.

#[test]
#[ignore = "owned by Plan 35-00 (this plan) — covers via cargo-tree audit"]
fn ctx_01_pure_rust_no_openssl() {
    // Acceptance: cargo tree -p mango_core does not show a new openssl-sys
    // edge through contextvm-sdk. Verified in 35-00 acceptance, not at
    // runtime.
    unimplemented!("verified by `cargo tree` audit in 35-00 acceptance");
}

#[test]
#[ignore = "owned by Plan 35-06 (Android) and 35-07 (Desktop) — UI smoke"]
fn ctx_02_settings_discover_tools_row_and_screen() {
    unimplemented!("UI test in 35-06 / 35-07");
}

#[test]
fn ctx_03_per_tool_enable_persists_across_launches() {
    // Real coverage:
    // tests::persistence::test_update_contextvm_tool_enabled_persists_after_reopen.
    // This integration checkpoint runs a minimal smoke version against the
    // real Database / queries layer landed in Plan 35-01.
    let db = crate::persistence::Database::open(":memory:").unwrap();
    let row = crate::persistence::queries::ContextvmToolRow {
        id: "pkA:smoke".into(),
        tool_name: "smoke".into(),
        display_name: None,
        description: String::new(),
        provider_pubkey: "pkA".into(),
        provider_display_name: None,
        schema_json: "{}".into(),
        enabled: false,
        last_seen_at: 1,
    };
    crate::persistence::queries::upsert_contextvm_tool(db.conn(), &row).unwrap();
    crate::persistence::queries::update_contextvm_tool_enabled(db.conn(), "pkA:smoke", true)
        .unwrap();
    let r = crate::persistence::queries::get_contextvm_tool_by_name(db.conn(), "smoke")
        .unwrap()
        .unwrap();
    assert!(r.enabled);
}

#[test]
fn ctx_04_auto_discover_tools_toggle_persists() {
    let db = crate::persistence::Database::open(":memory:").unwrap();
    crate::persistence::queries::set_setting(db.conn(), "auto_discover_tools", "1").unwrap();
    assert_eq!(
        crate::persistence::queries::get_setting(db.conn(), "auto_discover_tools")
            .unwrap()
            .as_deref(),
        Some("1")
    );
}

#[test]
#[ignore = "owned by Plan 35-04 (build_chat_tools extension)"]
fn ctx_05_enabled_tools_appear_in_openai_tools_array() {
    unimplemented!("build_chat_tools_with_contextvm appends enabled remote tools");
}

#[test]
#[ignore = "owned by Plan 35-04 (dispatch wiring) — invocation primitive itself \
            is now covered by the helper-function tests below; un-ignored \
            when dispatch_tools routes a remote name to invoke_tool"]
fn ctx_06_invocation_routes_through_nostr_returns_tool_result() {
    unimplemented!("dispatch_tools routes remote name to NostrMCPProxy and returns string");
}

// ── Plan 35-03 helper unit tests ──────────────────────────────────────

#[test]
fn test_truncate_result_under_limit_unchanged() {
    let s = "hello".repeat(100); // ~500 bytes
    let out = crate::contextvm::invocation::truncate_result(s.clone());
    assert_eq!(out, s);
}

#[test]
fn test_truncate_result_over_limit_appends_marker() {
    let s = "x".repeat(20_000);
    let out = crate::contextvm::invocation::truncate_result(s);
    assert!(
        out.ends_with("... [truncated]"),
        "got tail: {:?}",
        &out[out.len().saturating_sub(40)..]
    );
    // Body cap (16 KiB) + marker (15 bytes) is the exact upper bound;
    // the slack guards against future char-boundary walk-back.
    assert!(out.len() <= crate::contextvm::MAX_TOOL_RESULT_BYTES + "... [truncated]".len() + 4);
}

#[test]
fn test_truncate_result_respects_utf8_boundary() {
    // Build a string whose exact 16_384th byte sits inside a multi-byte
    // codepoint. "é" = 2 bytes; pad to push the boundary inside it.
    let mut s = String::with_capacity(20_000);
    for _ in 0..(crate::contextvm::MAX_TOOL_RESULT_BYTES / 2) {
        s.push('é');
    }
    s.push_str("tail-payload-tail-payload");
    // Pre-truncation must not be a valid char_boundary at the cap.
    let out = crate::contextvm::invocation::truncate_result(s);
    assert!(out.ends_with("... [truncated]"));
}

#[test]
fn test_format_timeout_locked_copy() {
    let s = crate::contextvm::invocation::format_timeout("get_weather");
    assert_eq!(s, "Error: tool 'get_weather' timed out (15s)");
}

#[test]
fn test_format_jsonrpc_error_locked_copy() {
    let s = crate::contextvm::invocation::format_jsonrpc_error(-32000, "boom");
    assert_eq!(s, "Error: -32000: boom");
}

#[test]
fn test_load_or_create_secret_key_creates_then_returns_same() {
    let db = crate::persistence::Database::open(":memory:").unwrap();
    let k1 =
        crate::contextvm::invocation::load_or_create_secret_key(db.conn()).unwrap();
    assert!(!k1.is_empty());
    // 32-byte secret hex = 64 chars.
    assert_eq!(k1.len(), 64, "expected 64-char hex secret, got {}", k1.len());
    // Persisted under the documented key.
    let raw = crate::persistence::queries::get_setting(db.conn(), "contextvm_secret_key")
        .unwrap();
    assert_eq!(raw.as_deref(), Some(k1.as_str()));
    // Second call must return the SAME hex.
    let k2 =
        crate::contextvm::invocation::load_or_create_secret_key(db.conn()).unwrap();
    assert_eq!(k1, k2);
}

#[test]
fn test_decode_response_jsonrpc_error_envelope() {
    use contextvm_sdk::{JsonRpcError, JsonRpcErrorResponse, JsonRpcMessage};
    let msg = JsonRpcMessage::ErrorResponse(JsonRpcErrorResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::json!(1),
        error: JsonRpcError {
            code: -32601,
            message: "method not found".to_string(),
            data: None,
        },
    });
    let s = crate::contextvm::invocation::decode_response(&msg, "whatever");
    assert_eq!(s, "Error: -32601: method not found");
}

#[test]
fn test_decode_response_success_text_content() {
    use contextvm_sdk::{JsonRpcMessage, JsonRpcResponse};
    let msg = JsonRpcMessage::Response(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::json!(1),
        result: serde_json::json!({
            "content": [{"type": "text", "text": "the answer is 42"}],
            "isError": false,
        }),
    });
    let s = crate::contextvm::invocation::decode_response(&msg, "x");
    assert_eq!(s, "the answer is 42");
}

#[test]
fn test_decode_response_oversize_text_truncated() {
    use contextvm_sdk::{JsonRpcMessage, JsonRpcResponse};
    let big = "y".repeat(20_000);
    let msg = JsonRpcMessage::Response(JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: serde_json::json!(1),
        result: serde_json::json!({
            "content": [{"type": "text", "text": big}],
        }),
    });
    let s = crate::contextvm::invocation::decode_response(&msg, "x");
    assert!(s.ends_with("... [truncated]"));
    assert!(s.len() <= crate::contextvm::MAX_TOOL_RESULT_BYTES + 32);
}

#[tokio::test]
#[ignore = "live network — no public always-on contextvm test tool exists; \
            skipped by default. Un-ignore manually with a known-good \
            provider_pubkey + tool_name to smoke-test against real relays."]
async fn live_invoke_tool_against_known_provider() {
    let db = crate::persistence::Database::open(":memory:").unwrap();
    let sk = crate::contextvm::invocation::load_or_create_secret_key(db.conn()).unwrap();
    // Replace these with a real test fixture before un-ignoring:
    let provider = "0000000000000000000000000000000000000000000000000000000000000000";
    let result =
        crate::contextvm::invocation::invoke_tool(&sk, provider, "echo", "{}").await;
    // We don't assert content (depends on remote); just non-panic.
    let _ = result;
}

#[test]
fn ctx_07_default_relay_set_includes_relay_nostr_net() {
    let relays = crate::contextvm::DEFAULT_CONTEXTVM_RELAYS;
    assert!(
        relays.contains(&"wss://relay.nostr.net"),
        "DEFAULT_CONTEXTVM_RELAYS missing relay.nostr.net: {:?}",
        relays
    );
    assert!(
        !relays.is_empty(),
        "DEFAULT_CONTEXTVM_RELAYS must not be empty"
    );
    assert!(
        relays.contains(&"wss://relay.damus.io"),
        "DEFAULT_CONTEXTVM_RELAYS missing relay.damus.io"
    );
    assert!(
        relays.contains(&"wss://nos.lol"),
        "DEFAULT_CONTEXTVM_RELAYS missing nos.lol"
    );
}

#[test]
fn ctx_08_graceful_degradation_on_relay_failure() {
    // Verify the error vocabulary is in place: discovery surfaces typed
    // ContextvmError variants that the actor will stash into AppState
    // (full actor-state assertion lives in Plan 35-05). The Display impl
    // is the public contract for the user-facing error_message string.
    let e = crate::contextvm::ContextvmError::RelayUnreachable {
        detail: "connection refused".into(),
    };
    let msg = e.to_string();
    assert!(
        msg.starts_with("Couldn't reach relays"),
        "RelayUnreachable display mismatch: {}",
        msg
    );

    let e2 = crate::contextvm::ContextvmError::Timeout {
        tool_name: "search".into(),
        secs: 30,
    };
    assert!(
        e2.to_string().contains("'search'"),
        "Timeout display missing tool name"
    );
}

#[tokio::test]
#[ignore = "live network — un-ignored by Plan 35-09"]
async fn live_discover_servers_against_default_relays() {
    let relays = crate::contextvm::default_relays_owned();
    let result = crate::contextvm::discover_servers(&relays).await;
    // Whatever the result, it must not panic and must return Result.
    let _ = result;
}

#[test]
#[ignore = "owned by Plan 35-08 (UniFFI binding regen)"]
fn ctx_09_uniffi_bindings_regenerated_for_all_three_platforms() {
    unimplemented!("Bindings regenerated; smoke test: kotlin/swift files contain DiscoverableTool");
}

#[test]
#[ignore = "owned by Plan 35-05 (AgentStepSummary.tool_origin)"]
fn ctx_10_agent_step_summary_carries_tool_origin_for_remote_tool_calls() {
    unimplemented!("AgentStepSummary {{ tool_origin: Some(\"contextvm\".into()), .. }}");
}
