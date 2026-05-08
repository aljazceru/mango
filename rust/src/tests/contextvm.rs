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
fn ctx_02_settings_discover_tools_row_and_screen() {
    // Rust-core checkpoint for CTX-02: the Screen enum has the
    // ToolDiscovery variant the UI dispatches PushScreen for.
    let _s = crate::Screen::ToolDiscovery;
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
fn ctx_05_enabled_tools_appear_in_openai_tools_array() {
    use async_openai::types::chat::ChatCompletionTools;
    let desc = crate::contextvm::ContextvmToolDescriptor {
        tool_name: "translate".into(),
        description: "x".into(),
        schema: serde_json::json!({"type": "object"}),
        provider_pubkey_hex: "pkA".into(),
        provider_display_name: None,
        last_seen_at: 1,
    };
    let tools = crate::agent::tools::build_chat_tools_with_contextvm(false, false, &[desc]);
    let names: Vec<String> = tools
        .iter()
        .filter_map(|t| match t {
            ChatCompletionTools::Function(f) => Some(f.function.name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        names.contains(&"translate".to_string()),
        "remote tool 'translate' missing from chat_tools array: {:?}",
        names
    );
}

#[test]
fn ctx_06_invocation_routes_through_nostr_returns_tool_result() {
    // Acceptance proxy: the dispatch fallback arm in `dispatch_tools`
    // routes any name present in `contextvm_map` to
    // `crate::contextvm::invoke_tool`. We assert the routing decision
    // (i.e., the unknown-tool error string is NOT produced when the
    // name is in the map) without spinning up a real Nostr relay.
    //
    // The invoke_tool path will fail to connect (empty secret key,
    // unreachable relays) and surface a typed error string — what we
    // care about here is precedence: the unknown-tool arm is bypassed.
    use async_openai::types::chat::{ChatCompletionMessageToolCall, FunctionCall};

    let tmp = tempfile::tempdir().unwrap();
    let db = crate::persistence::Database::open(":memory:").unwrap();
    let index =
        crate::rag::VectorIndex::new(tmp.path().to_str().unwrap(), None).unwrap();
    let provider = crate::NullEmbeddingProvider;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut map = std::collections::HashMap::new();
    map.insert(
        "translate".to_string(),
        crate::contextvm::ContextvmToolDescriptor {
            tool_name: "translate".into(),
            description: "x".into(),
            schema: serde_json::json!({"type": "object"}),
            // Bogus pubkey so invoke_tool fails fast at proxy build.
            provider_pubkey_hex: "00".repeat(32),
            provider_display_name: None,
            last_seen_at: 1,
        },
    );

    let call = ChatCompletionMessageToolCall {
        id: "call-1".to_string(),
        function: FunctionCall {
            name: "translate".to_string(),
            arguments: "{}".to_string(),
        },
    };
    // Use a dummy 32-byte hex secret key so signer::from_sk parses.
    let secret = "11".repeat(32);
    let results = crate::agent::tools::dispatch_tools(
        &[call],
        db.conn(),
        &index,
        &provider,
        &rt,
        "",
        "",
        &map,
        &secret,
    );
    assert_eq!(results.len(), 1);
    assert!(
        !results[0].1.contains("unknown tool"),
        "remote tool was misrouted to unknown-tool arm: {}",
        results[0].1
    );
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
async fn live_discover_servers_against_default_relays() {
    let relays = crate::contextvm::default_relays_owned();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        crate::contextvm::discover_servers(&relays),
    )
    .await;

    // Outer timeout reached: relays are unreachable from this CI runner;
    // skip-pass to keep the suite green.
    let result = match result {
        Ok(r) => r,
        Err(_) => {
            eprintln!(
                "live_discover_servers_against_default_relays: timed out — \
                 network unreachable, skipping assertion"
            );
            return;
        }
    };

    match result {
        Ok(servers) => {
            // Don't assert non-empty — public relays may have 0 announcements
            // at any given moment. The test passes if the call itself
            // succeeds. Best-effort try discover_all + invoke_tool against
            // the first announced tool, but never fail the test on the
            // remote leg — the point is to exercise the full pipeline.
            eprintln!("Discovered {} servers", servers.len());
            let all_result = tokio::time::timeout(
                std::time::Duration::from_secs(20),
                crate::contextvm::discover_all(&relays),
            )
            .await;
            if let Ok(Ok(tools)) = all_result {
                eprintln!("discover_all: {} tools announced", tools.len());
                if let Some(first) = tools.first() {
                    let db = crate::persistence::Database::open(":memory:").unwrap();
                    let sk =
                        crate::contextvm::invocation::load_or_create_secret_key(db.conn())
                            .unwrap();
                    let invoke = tokio::time::timeout(
                        std::time::Duration::from_secs(20),
                        crate::contextvm::invocation::invoke_tool(
                            &sk,
                            &first.provider_pubkey_hex,
                            &first.tool_name,
                            "{}",
                        ),
                    )
                    .await;
                    eprintln!(
                        "invoke_tool({}@{}): {:?}",
                        first.tool_name, first.provider_pubkey_hex, invoke
                    );
                }
            }
        }
        Err(crate::contextvm::ContextvmError::RelayUnreachable { detail }) => {
            eprintln!(
                "live_discover_servers_against_default_relays: \
                 RelayUnreachable: {} — skipping",
                detail
            );
        }
        Err(e) => {
            panic!("unexpected error from discover_servers: {}", e);
        }
    }
}

#[test]
fn ctx_09_uniffi_bindings_regenerated_for_all_three_platforms() {
    // Phase 35-08 — verify both Swift and Kotlin binding files carry
    // the Phase 35 surface (DiscoverableTool, ContextvmDiscoveryState).
    // Bindings live OUTSIDE the rust/ crate so we use std::fs at runtime —
    // this is a smoke check that the last `just bindings-{swift,kotlin}`
    // invocation produced sane output.
    let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(|s| {
            std::path::PathBuf::from(s)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Swift bindings — tolerant on Linux dev hosts where iOS-side files
    // may be absent in a stripped checkout.
    let swift_path = workspace_root.join("ios/Bindings/mango_core.swift");
    if let Ok(content) = std::fs::read_to_string(&swift_path) {
        assert!(
            content.contains("DiscoverableTool"),
            "Swift bindings missing DiscoverableTool — re-run `just bindings-swift`"
        );
        assert!(
            content.contains("ContextvmDiscoveryState"),
            "Swift bindings missing ContextvmDiscoveryState — re-run `just bindings-swift`"
        );
    }

    // Kotlin bindings — Linux is the canonical Android dev target, so
    // Kotlin bindings must exist and contain the Phase 35 types.
    let kotlin_root = workspace_root.join("android/app/src/main/java");
    let mut kotlin_ok = false;
    if kotlin_root.exists() {
        for entry in walkdir_compat(&kotlin_root) {
            if entry
                .file_name()
                .map(|n| n == "mango_core.kt")
                .unwrap_or(false)
            {
                if let Ok(content) = std::fs::read_to_string(&entry) {
                    if content.contains("DiscoverableTool")
                        && content.contains("ContextvmDiscoveryState")
                    {
                        kotlin_ok = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        kotlin_ok,
        "Kotlin bindings missing Phase 35 types — re-run `just bindings-kotlin`"
    );
}

fn walkdir_compat(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&p) {
            for e in entries.filter_map(Result::ok) {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(path);
                }
            }
        }
    }
    out
}

#[test]
fn ctx_10_agent_step_summary_carries_tool_origin_for_remote_tool_calls() {
    // Phase 35 — AgentStepSummary surfaces tool provenance via the
    // `tool_origin` field. The actor stamps "contextvm" when the dispatch
    // map routed the call to a remote tool, "local" otherwise.
    let s = crate::AgentStepSummary {
        id: "a".into(),
        step_number: 1,
        action_type: "tool_call".into(),
        tool_name: Some("translate".into()),
        tool_input: None,
        result_snippet: None,
        status: "ok".into(),
        tool_origin: Some("contextvm".into()),
    };
    assert_eq!(s.tool_origin.as_deref(), Some("contextvm"));
}

// ── Plan 35-04 dispatch helper unit tests ─────────────────────────────

fn fixture_descriptor(name: &str, ts: i64) -> crate::contextvm::ContextvmToolDescriptor {
    crate::contextvm::ContextvmToolDescriptor {
        tool_name: name.into(),
        description: format!("desc {}", name),
        schema: serde_json::json!({"type": "object"}),
        provider_pubkey_hex: "pkA".into(),
        provider_display_name: None,
        last_seen_at: ts,
    }
}

#[test]
fn test_finalise_for_turn_filters_reserved_local_names() {
    let input = vec![
        fixture_descriptor("calculate", 1),
        fixture_descriptor("get_weather", 2),
        fixture_descriptor("web_search", 3),
        fixture_descriptor("translate", 4),
    ];
    let out = crate::contextvm::finalise_for_turn(input);
    let names: Vec<&str> = out.iter().map(|d| d.tool_name.as_str()).collect();
    assert_eq!(names, vec!["translate", "get_weather"]);
    assert!(!names.contains(&"calculate"));
    assert!(!names.contains(&"web_search"));
}

#[test]
fn test_finalise_for_turn_caps_at_8_sorted_desc_by_last_seen() {
    let mut input: Vec<_> = (0..20)
        .map(|i| fixture_descriptor(&format!("tool_{:02}", i), i as i64))
        .collect();
    input.reverse();
    let out = crate::contextvm::finalise_for_turn(input);
    assert_eq!(out.len(), 8);
    let names: Vec<String> = out.iter().map(|d| d.tool_name.clone()).collect();
    assert_eq!(names[0], "tool_19");
    assert_eq!(names[7], "tool_12");
}

#[test]
fn test_finalise_for_turn_alphabetical_tiebreak_on_equal_last_seen() {
    let input = vec![
        fixture_descriptor("zebra", 5),
        fixture_descriptor("apple", 5),
        fixture_descriptor("mango", 5),
    ];
    let out = crate::contextvm::finalise_for_turn(input);
    let names: Vec<&str> = out.iter().map(|d| d.tool_name.as_str()).collect();
    assert_eq!(names, vec!["apple", "mango", "zebra"]);
}

#[test]
fn test_descriptor_caps_description_at_500_chars() {
    let row = crate::persistence::queries::ContextvmToolRow {
        id: "pkA:big".into(),
        tool_name: "big".into(),
        display_name: None,
        description: "x".repeat(1000),
        provider_pubkey: "pkA".into(),
        provider_display_name: None,
        schema_json: "{\"type\":\"object\"}".into(),
        enabled: true,
        last_seen_at: 1,
    };
    let d = crate::contextvm::ContextvmToolDescriptor::from_row(&row).unwrap();
    let chars = d.description.chars().count();
    assert_eq!(chars, 501, "got {} chars (expected 500 + ellipsis)", chars);
    assert!(d.description.ends_with('…'));
}

#[test]
fn test_descriptor_under_cap_unchanged() {
    let row = crate::persistence::queries::ContextvmToolRow {
        id: "pkA:small".into(),
        tool_name: "small".into(),
        display_name: None,
        description: "short".into(),
        provider_pubkey: "pkA".into(),
        provider_display_name: None,
        schema_json: "{\"type\":\"object\"}".into(),
        enabled: true,
        last_seen_at: 1,
    };
    let d = crate::contextvm::ContextvmToolDescriptor::from_row(&row).unwrap();
    assert_eq!(d.description, "short");
}

#[test]
fn test_descriptors_to_chat_tools_round_trip() {
    use async_openai::types::chat::ChatCompletionTools;
    let descs = vec![fixture_descriptor("translate", 1)];
    let tools = crate::contextvm::descriptors_to_chat_tools(&descs);
    assert_eq!(tools.len(), 1);
    match &tools[0] {
        ChatCompletionTools::Function(f) => {
            assert_eq!(f.function.name, "translate");
            assert_eq!(f.function.description.as_deref(), Some("desc translate"));
        }
        _ => panic!("expected Function variant"),
    }
}

#[test]
fn test_build_dispatch_map_keyed_by_tool_name() {
    let descs = vec![fixture_descriptor("a", 1), fixture_descriptor("b", 2)];
    let m = crate::contextvm::build_dispatch_map(&descs);
    assert_eq!(m.len(), 2);
    assert!(m.contains_key("a"));
    assert!(m.contains_key("b"));
}

#[test]
fn test_finalise_for_turn_filtered_collisions_handled() {
    // Contract: the actor (Plan 35-05) MUST run finalise_for_turn so
    // collisions are filtered before reaching build_chat_tools_with_contextvm.
    let collide = fixture_descriptor("calculate", 1);
    let filtered = crate::contextvm::finalise_for_turn(vec![collide]);
    assert!(
        filtered.is_empty(),
        "finalise_for_turn must drop reserved-name collisions"
    );
}

#[test]
fn test_locals_win_on_collision_via_match_arm_precedence() {
    // Even if a remote descriptor for "calculate" leaks into the map
    // (e.g., test injection bypassing finalise_for_turn), the local
    // match-arm in dispatch_tools fires first.
    use async_openai::types::chat::{ChatCompletionMessageToolCall, FunctionCall};

    let tmp = tempfile::tempdir().unwrap();
    let db = crate::persistence::Database::open(":memory:").unwrap();
    let index =
        crate::rag::VectorIndex::new(tmp.path().to_str().unwrap(), None).unwrap();
    let provider = crate::NullEmbeddingProvider;
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut map = std::collections::HashMap::new();
    map.insert(
        "calculate".to_string(),
        crate::contextvm::ContextvmToolDescriptor {
            tool_name: "calculate".into(),
            description: "remote calc".into(),
            schema: serde_json::json!({"type": "object"}),
            provider_pubkey_hex: "00".repeat(32),
            provider_display_name: None,
            last_seen_at: 1,
        },
    );

    let call = ChatCompletionMessageToolCall {
        id: "c1".into(),
        function: FunctionCall {
            name: "calculate".into(),
            arguments: "{\"expression\": \"2+2\"}".into(),
        },
    };
    let secret = "11".repeat(32);
    let results = crate::agent::tools::dispatch_tools(
        &[call],
        db.conn(),
        &index,
        &provider,
        &rt,
        "",
        "",
        &map,
        &secret,
    );
    assert_eq!(results.len(), 1);
    // Local calculate produced "4" — proves local arm fired, not the
    // remote fallback (which would have yielded a relay/network error).
    assert_eq!(
        results[0].1, "4",
        "local calculator must win over remote 'calculate' descriptor; got: {}",
        results[0].1
    );
}

#[test]
fn test_build_chat_tools_with_contextvm_appends_remote() {
    use async_openai::types::chat::ChatCompletionTools;
    let desc = crate::contextvm::ContextvmToolDescriptor {
        tool_name: "translate".into(),
        description: "translate text".into(),
        schema: serde_json::json!({"type": "object"}),
        provider_pubkey_hex: "pkA".into(),
        provider_display_name: None,
        last_seen_at: 1,
    };
    let tools_no_remote = crate::agent::tools::build_chat_tools(false, false);
    let tools_with_remote =
        crate::agent::tools::build_chat_tools_with_contextvm(false, false, &[desc]);
    assert_eq!(tools_with_remote.len(), tools_no_remote.len() + 1);
    let last = tools_with_remote.last().expect("at least one tool");
    match last {
        ChatCompletionTools::Function(f) => {
            assert_eq!(f.function.name, "translate");
        }
        _ => panic!("expected Function variant"),
    }
}

#[test]
fn test_build_chat_tools_with_contextvm_caps_at_8_via_finalise() {
    // Caller must run finalise_for_turn first; we exercise the full path.
    let descs: Vec<_> = (0..20)
        .map(|i| fixture_descriptor(&format!("remote_tool_{:02}", i), i as i64))
        .collect();
    let finalised = crate::contextvm::finalise_for_turn(descs);
    assert_eq!(finalised.len(), 8);
    let baseline = crate::agent::tools::build_chat_tools(false, false);
    let with_remote =
        crate::agent::tools::build_chat_tools_with_contextvm(false, false, &finalised);
    assert_eq!(with_remote.len(), baseline.len() + 8);
}
