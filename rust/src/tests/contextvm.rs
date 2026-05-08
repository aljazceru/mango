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
#[ignore = "owned by Plan 35-01 (persistence) and 35-05 (actor)"]
fn ctx_03_per_tool_enable_persists_across_launches() {
    unimplemented!("set enabled=true → reopen DB → query → assert enabled");
}

#[test]
#[ignore = "owned by Plan 35-01 + 35-05"]
fn ctx_04_auto_discover_tools_toggle_persists() {
    unimplemented!("settings key auto_discover_tools round-trips");
}

#[test]
#[ignore = "owned by Plan 35-04 (build_chat_tools extension)"]
fn ctx_05_enabled_tools_appear_in_openai_tools_array() {
    unimplemented!("build_chat_tools_with_contextvm appends enabled remote tools");
}

#[test]
#[ignore = "owned by Plan 35-03 (invocation) + Plan 35-04 (dispatch)"]
fn ctx_06_invocation_routes_through_nostr_returns_tool_result() {
    unimplemented!("dispatch_tools routes remote name to NostrMCPProxy and returns string");
}

#[test]
#[ignore = "owned by Plan 35-02 (DEFAULT_RELAYS const)"]
fn ctx_07_default_relay_set_includes_relay_nostr_net() {
    unimplemented!("DEFAULT_CONTEXTVM_RELAYS contains \"wss://relay.nostr.net\"");
}

#[test]
#[ignore = "owned by Plan 35-02 + 35-03 (error mapping)"]
fn ctx_08_graceful_degradation_on_relay_failure() {
    unimplemented!("unreachable relay → ContextvmDiscoveryState::Error, no panic");
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
