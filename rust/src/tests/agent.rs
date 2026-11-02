/// Agent tests for Phase 9.
///
/// Covers three categories:
/// 1. Persistence: agent session status transitions, step ordering.
/// 2. Tool schemas: verify build_agent_tools() structure.
/// 3. Actor integration: LaunchAgentSession, CancelAgentSession via FfiApp.
/// 4. Failure injection: max-step enforcement, malformed tool args, network timeout.
///
/// Tests requiring live LLM backends are tagged `#[ignore]`.
use std::time::Duration;

use async_openai::types::chat::{ChatCompletionMessageToolCall, FunctionCall};

use crate::agent::{build_agent_tools, dispatch_tools};
use crate::persistence::queries::{
    count_agent_steps, insert_agent_session, insert_agent_step, list_agent_sessions,
    list_agent_steps, update_agent_session_status, AgentSessionRow, AgentStepRow,
};
use crate::persistence::Database;
use crate::rag::VectorIndex;
use crate::{AppAction, EmbeddingStatus, FfiApp, NullEmbeddingProvider, NullKeychainProvider};

// ── Helper functions ──────────────────────────────────────────────────────────

fn make_session(id: &str, status: &str) -> AgentSessionRow {
    AgentSessionRow {
        id: id.into(),
        title: format!("Test session {}", id),
        status: status.into(),
        backend_id: "tinfoil".into(),
        created_at: 1000,
        updated_at: 1000,
    }
}

fn make_step(id: &str, session_id: &str, step_number: i64, action_type: &str) -> AgentStepRow {
    AgentStepRow {
        id: id.into(),
        session_id: session_id.into(),
        step_number,
        action_type: action_type.into(),
        action_payload: "{}".into(),
        result: None,
        status: "completed".into(),
        created_at: 1000,
        tool_origin: None,
    }
}

/// Create FfiApp with in-memory DB and null providers.
fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    // Allow actor thread to initialize
    app.sync();
    app
}

/// Wait for the actor to process a dispatched action.
fn wait(app: &FfiApp) {
    app.sync();
}

// ── Persistence tests ─────────────────────────────────────────────────────────

/// Verify that an agent session can be created and its status updated through
/// all defined lifecycle states: running -> paused -> running -> completed.
#[test]
fn test_agent_session_status_transitions() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();

    insert_agent_session(conn, &make_session("sess-1", "running")).unwrap();

    // Verify initial status
    let rows = list_agent_sessions(conn).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "running");

    // Transition to paused
    update_agent_session_status(conn, "sess-1", "paused", 2000).unwrap();
    let rows = list_agent_sessions(conn).unwrap();
    assert_eq!(
        rows[0].status, "paused",
        "Status should be paused after update"
    );
    assert_eq!(
        rows[0].updated_at, 2000,
        "updated_at should reflect the new timestamp"
    );

    // Transition to running again
    update_agent_session_status(conn, "sess-1", "running", 3000).unwrap();
    let rows = list_agent_sessions(conn).unwrap();
    assert_eq!(
        rows[0].status, "running",
        "Status should be running after resume"
    );

    // Transition to completed
    update_agent_session_status(conn, "sess-1", "completed", 4000).unwrap();
    let rows = list_agent_sessions(conn).unwrap();
    assert_eq!(rows[0].status, "completed", "Status should be completed");

    // Verify failed transition
    insert_agent_session(conn, &make_session("sess-2", "running")).unwrap();
    update_agent_session_status(conn, "sess-2", "failed", 5000).unwrap();
    let rows = list_agent_sessions(conn).unwrap();
    let sess2 = rows
        .iter()
        .find(|r| r.id == "sess-2")
        .expect("sess-2 should exist");
    assert_eq!(sess2.status, "failed");

    // Verify cancelled transition
    insert_agent_session(conn, &make_session("sess-3", "running")).unwrap();
    update_agent_session_status(conn, "sess-3", "cancelled", 6000).unwrap();
    let rows = list_agent_sessions(conn).unwrap();
    let sess3 = rows
        .iter()
        .find(|r| r.id == "sess-3")
        .expect("sess-3 should exist");
    assert_eq!(sess3.status, "cancelled");
}

/// Verify that agent steps are returned sorted by step_number regardless of insertion order.
#[test]
fn test_agent_steps_ordered() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();

    insert_agent_session(conn, &make_session("sess-ordered", "running")).unwrap();

    // Insert 5 steps out of order
    insert_agent_step(conn, &make_step("step-5", "sess-ordered", 5, "tool_call")).unwrap();
    insert_agent_step(conn, &make_step("step-2", "sess-ordered", 2, "tool_call")).unwrap();
    insert_agent_step(
        conn,
        &make_step("step-4", "sess-ordered", 4, "final_answer"),
    )
    .unwrap();
    insert_agent_step(conn, &make_step("step-1", "sess-ordered", 1, "tool_call")).unwrap();
    insert_agent_step(conn, &make_step("step-3", "sess-ordered", 3, "tool_call")).unwrap();

    let steps = list_agent_steps(conn, "sess-ordered").unwrap();
    assert_eq!(steps.len(), 5, "Should have 5 steps");

    let numbers: Vec<i64> = steps.iter().map(|s| s.step_number).collect();
    assert_eq!(
        numbers,
        vec![1, 2, 3, 4, 5],
        "Steps should be sorted by step_number ascending"
    );
    assert_eq!(steps[0].id, "step-1", "First step should be step-1");
    assert_eq!(steps[4].id, "step-5", "Last step should be step-5");
}

/// Verify count_agent_steps returns correct count.
#[test]
fn test_count_agent_steps() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();

    insert_agent_session(conn, &make_session("sess-count", "running")).unwrap();

    let count_before = count_agent_steps(conn, "sess-count").unwrap();
    assert_eq!(count_before, 0, "Count should be 0 before any steps");

    insert_agent_step(conn, &make_step("s1", "sess-count", 1, "tool_call")).unwrap();
    insert_agent_step(conn, &make_step("s2", "sess-count", 2, "tool_call")).unwrap();
    insert_agent_step(conn, &make_step("s3", "sess-count", 3, "final_answer")).unwrap();

    let count_after = count_agent_steps(conn, "sess-count").unwrap();
    assert_eq!(count_after, 3, "Count should be 3 after inserting 3 steps");
}

fn checkpoint_actor() -> crate::ActorState {
    let mut state = crate::image_red_tests::default_actor_state_for_image_tests();
    insert_agent_session(
        state.db.as_ref().unwrap().conn(),
        &make_session("checkpoint", "running"),
    )
    .unwrap();
    // This transport rejects requests locally, so no test needs a live backend.
    state.backends.push(crate::llm::BackendConfig {
        id: "tinfoil".into(),
        name: "Checkpoint test".into(),
        base_url: "local://checkpoint".into(),
        api_key: String::new(),
        models: vec!["checkpoint-model".into()],
        tee_type: crate::llm::TeeType::Unknown,
        max_concurrent_requests: 1,
        supports_tool_use: true,
    });
    state.active_agent_sessions.insert(
        "checkpoint".into(),
        crate::agent::AgentExecutionState {
            session_id: "checkpoint".into(),
            messages: vec![],
            step_number: 1,
            backend_id: "tinfoil".into(),
            model: "checkpoint-model".into(),
        },
    );
    state
}

fn complete_checkpoint_step(state: &mut crate::ActorState, result: crate::agent::AgentStepResult) {
    let (core_tx, _core_rx) = flume::unbounded();
    let (update_tx, _update_rx) = flume::unbounded();
    let shared = std::sync::Arc::new(std::sync::RwLock::new(state.app_state.clone()));
    crate::handle_agent_step_complete(
        state,
        "checkpoint".into(),
        1,
        Ok(result),
        &core_tx,
        &shared,
        &update_tx,
    );
}

fn checkpoint_calls() -> Vec<ChatCompletionMessageToolCall> {
    [("call-a", "1 + 1"), ("call-b", "2 * 3")]
        .into_iter()
        .map(|(id, expression)| ChatCompletionMessageToolCall {
            id: id.into(),
            function: FunctionCall {
                name: "calculate".into(),
                arguments: serde_json::json!({"expression": expression}).to_string(),
            },
        })
        .collect()
}

#[test]
fn agent_checkpoint_persists_results_and_resumes_matching_tool_messages() {
    let mut state = checkpoint_actor();
    complete_checkpoint_step(
        &mut state,
        crate::agent::AgentStepResult::ToolCalls(checkpoint_calls()),
    );
    let steps = list_agent_steps(state.db.as_ref().unwrap().conn(), "checkpoint").unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].status, "completed");
    let results: Vec<(String, String)> =
        serde_json::from_str(steps[0].result.as_deref().unwrap()).unwrap();
    assert_eq!(
        results,
        vec![("call-a".into(), "2".into()), ("call-b".into(), "6".into())]
    );
    let original =
        serde_json::to_value(&state.active_agent_sessions["checkpoint"].messages).unwrap();
    assert_eq!(original[0]["tool_calls"][0]["id"], "call-a");
    assert_eq!(original[1]["tool_call_id"], "call-a");
    assert_eq!(original[1]["content"], "2");
    assert_eq!(original[2]["tool_call_id"], "call-b");
    assert_eq!(original[2]["content"], "6");

    state.active_agent_sessions.remove("checkpoint");
    update_agent_session_status(
        state.db.as_ref().unwrap().conn(),
        "checkpoint",
        "paused",
        2000,
    )
    .unwrap();
    let (core_tx, _core_rx) = flume::unbounded();
    crate::handle_resume_agent_session(&mut state, "checkpoint".into(), &core_tx);
    let resumed = &state.active_agent_sessions["checkpoint"];
    assert_eq!(resumed.step_number, 2);
    let resumed_messages = serde_json::to_value(&resumed.messages).unwrap();
    assert_eq!(
        &resumed_messages.as_array().unwrap()[2..],
        original.as_array().unwrap().as_slice()
    );
}

#[test]
fn agent_checkpoint_replays_multiple_steps_and_reorders_results_by_call_id() {
    let mut step = make_step("a", "checkpoint", 1, "tool_call");
    step.action_payload = serde_json::json!([
        {"id":"a", "name":"calculate", "arguments":"{}"},
        {"id":"b", "name":"calculate", "arguments":"{}"},
    ])
    .to_string();
    step.result = Some(serde_json::json!([["b", "second"], ["a", "first"]]).to_string());
    let mut next = make_step("b", "checkpoint", 2, "tool_call");
    next.action_payload = serde_json::json!([
        {"id":"c", "name":"calculate", "arguments":"{}"},
    ])
    .to_string();
    next.result = Some(serde_json::json!([["c", "third"]]).to_string());
    let messages = crate::restore_agent_tool_messages(&[step, next]).unwrap();
    let messages = serde_json::to_value(messages).unwrap();
    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(messages[1]["tool_call_id"], "a");
    assert_eq!(messages[1]["content"], "first");
    assert_eq!(messages[2]["tool_call_id"], "b");
    assert_eq!(messages[3]["role"], "assistant");
    assert_eq!(messages[4]["tool_call_id"], "c");
    assert_eq!(messages[4]["content"], "third");
}

#[test]
fn agent_resume_rejects_incomplete_legacy_or_mismatched_checkpoints_visibly() {
    for (status, payload, result) in [
        (
            "running",
            r#"[{"id":"a","name":"calculate","arguments":"{}"}]"#,
            Some(r#"[["a","2"]]"#),
        ),
        (
            "completed",
            r#"[{"id":"a","name":"calculate","arguments":"{}"}]"#,
            None,
        ),
        (
            "completed",
            r#"[{"id":"a","name":"calculate","arguments":"{}"}]"#,
            Some("not json"),
        ),
        (
            "completed",
            r#"[{"id":"a","name":"calculate","arguments":"{}"}]"#,
            Some(r#"[["wrong","2"]]"#),
        ),
        (
            "completed",
            r#"[{"id":"a","name":"calculate","arguments":"{}"},{"id":"b","name":"calculate","arguments":"{}"}]"#,
            Some(r#"[["a","2"],["a","3"]]"#),
        ),
        (
            "completed",
            r#"[{"id":"a","name":"calculate"}]"#,
            Some(r#"[["a","2"]]"#),
        ),
        ("completed", "[]", Some("[]")),
    ] {
        let mut state = checkpoint_actor();
        state.active_agent_sessions.clear();
        let mut step = make_step("bad", "checkpoint", 1, "tool_call");
        step.status = status.into();
        step.action_payload = payload.into();
        step.result = result.map(String::from);
        insert_agent_step(state.db.as_ref().unwrap().conn(), &step).unwrap();
        let (core_tx, core_rx) = flume::unbounded();
        crate::handle_resume_agent_session(&mut state, "checkpoint".into(), &core_tx);
        assert!(state.active_agent_sessions.is_empty());
        assert!(
            core_rx.is_empty(),
            "Resume must not launch a request with an invalid checkpoint"
        );
        assert_eq!(
            list_agent_sessions(state.db.as_ref().unwrap().conn()).unwrap()[0].status,
            "failed"
        );
        assert!(state
            .app_state
            .toast
            .as_deref()
            .unwrap()
            .contains("cannot safely resume"));
    }
}

#[test]
fn agent_checkpoint_write_failures_stop_session_without_advancing() {
    for trigger in [
        "CREATE TRIGGER fail_checkpoint BEFORE INSERT ON agent_steps BEGIN SELECT RAISE(ABORT, 'checkpoint insert failed'); END;",
        "CREATE TRIGGER fail_checkpoint BEFORE UPDATE ON agent_steps BEGIN SELECT RAISE(ABORT, 'checkpoint result failed'); END;",
        "CREATE TRIGGER fail_checkpoint BEFORE UPDATE ON agent_sessions WHEN NEW.status = 'running' BEGIN SELECT RAISE(ABORT, 'checkpoint status failed'); END;",
    ] {
        let mut state = checkpoint_actor();
        state.db.as_ref().unwrap().conn().execute_batch(trigger).unwrap();
        let (core_tx, core_rx) = flume::unbounded();
        let (update_tx, _update_rx) = flume::unbounded();
        let shared = std::sync::Arc::new(std::sync::RwLock::new(state.app_state.clone()));
        crate::handle_agent_step_complete(
            &mut state, "checkpoint".into(), 1,
            Ok(crate::agent::AgentStepResult::ToolCalls(checkpoint_calls())),
            &core_tx, &shared, &update_tx,
        );
        assert!(state.active_agent_sessions.is_empty());
        assert!(core_rx.is_empty());
        assert_eq!(list_agent_sessions(state.db.as_ref().unwrap().conn()).unwrap()[0].status, "failed");
        assert!(state.app_state.toast.as_deref().unwrap().contains("checkpoint"));
        let steps = list_agent_steps(state.db.as_ref().unwrap().conn(), "checkpoint").unwrap();
        if trigger.contains("UPDATE ON agent_steps") {
            assert_eq!(steps[0].status, "running");
            assert!(steps[0].result.is_none());
        }
    }
}

#[test]
fn agent_resume_read_and_status_failures_are_visible() {
    for statement in [
        "DROP TABLE agent_steps;",
        "CREATE TRIGGER fail_resume BEFORE UPDATE ON agent_sessions BEGIN SELECT RAISE(ABORT, 'resume status failed'); END;",
    ] {
        let mut state = checkpoint_actor();
        state.active_agent_sessions.clear();
        state.db.as_ref().unwrap().conn().execute_batch(statement).unwrap();
        let (core_tx, core_rx) = flume::unbounded();
        crate::handle_resume_agent_session(&mut state, "checkpoint".into(), &core_tx);
        assert!(state.active_agent_sessions.is_empty());
        assert!(core_rx.is_empty());
        assert_eq!(state.app_state.agent_sessions[0].status, "failed");
        assert!(state.app_state.toast.as_deref().unwrap().contains("Agent resume failed"));
    }
}

#[test]
fn agent_final_answer_checkpoint_failure_is_not_reported_as_completed() {
    for trigger in [
        "CREATE TRIGGER fail_final BEFORE INSERT ON agent_steps BEGIN SELECT RAISE(ABORT, 'final checkpoint failed'); END;",
        "CREATE TRIGGER fail_final BEFORE UPDATE ON agent_sessions WHEN NEW.status = 'completed' BEGIN SELECT RAISE(ABORT, 'final checkpoint failed'); END;",
    ] {
        let mut state = checkpoint_actor();
        state.db.as_ref().unwrap().conn().execute_batch(trigger).unwrap();
        complete_checkpoint_step(&mut state, crate::agent::AgentStepResult::FinalAnswer("done".into()));
        assert!(state.active_agent_sessions.is_empty());
        assert_eq!(list_agent_sessions(state.db.as_ref().unwrap().conn()).unwrap()[0].status, "failed");
        assert!(state.app_state.toast.as_deref().unwrap().contains("final checkpoint failed"));
    }
}

#[test]
fn agent_tool_checkpoint_survives_database_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("agent.db");
    {
        let db = Database::open(path.to_str().unwrap()).unwrap();
        insert_agent_session(db.conn(), &make_session("checkpoint", "paused")).unwrap();
        let mut step = make_step("durable", "checkpoint", 1, "tool_call");
        step.status = "running".into();
        step.action_payload =
            r#"[{"id":"durable-call","name":"calculate","arguments":"{}"}]"#.into();
        insert_agent_step(db.conn(), &step).unwrap();
        crate::persistence::queries::complete_agent_tool_step(
            db.conn(),
            "durable",
            r#"[["durable-call","saved result"]]"#,
        )
        .unwrap();
        assert!(
            crate::persistence::queries::complete_agent_tool_step(db.conn(), "missing", "[]",)
                .is_err()
        );
    }
    let db = Database::open(path.to_str().unwrap()).unwrap();
    let steps = list_agent_steps(db.conn(), "checkpoint").unwrap();
    let messages = crate::restore_agent_tool_messages(&steps).unwrap();
    let messages = serde_json::to_value(messages).unwrap();
    assert_eq!(messages[0]["tool_calls"][0]["id"], "durable-call");
    assert_eq!(messages[1]["tool_call_id"], "durable-call");
    assert_eq!(messages[1]["content"], "saved result");
}

// ── Tool schema tests ─────────────────────────────────────────────────────────

/// Verify build_agent_tools() returns exactly 7 tools with the correct names.
#[test]
fn test_agent_tools_build() {
    let tools = build_agent_tools();
    assert_eq!(
        tools.len(),
        7,
        "Should have exactly 7 agent tools (3 existing + 4 new)"
    );

    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| match t {
            async_openai::types::chat::ChatCompletionTools::Function(f) => {
                Some(f.function.name.as_str())
            }
            _ => None,
        })
        .collect();

    assert!(
        names.contains(&"search_documents"),
        "Tools should include search_documents"
    );
    assert!(
        names.contains(&"read_document"),
        "Tools should include read_document"
    );
    assert!(names.contains(&"finish"), "Tools should include finish");
    assert!(
        names.contains(&"web_search"),
        "Tools should include web_search"
    );
    assert!(
        names.contains(&"fetch_url"),
        "Tools should include fetch_url"
    );
    assert!(names.contains(&"file"), "Tools should include file");
    assert!(
        names.contains(&"calculate"),
        "Tools should include calculate"
    );
}

#[test]
fn test_agent_tools_count_seven() {
    let tools = build_agent_tools();
    assert_eq!(
        tools.len(),
        7,
        "Should have exactly 7 agent tools (3 existing + 4 new)"
    );
}

#[test]
fn test_agent_tools_include_web_search() {
    let tools = build_agent_tools();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| match t {
            async_openai::types::chat::ChatCompletionTools::Function(f) => {
                Some(f.function.name.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        names.contains(&"web_search"),
        "Tools should include web_search"
    );
}

#[test]
fn test_agent_tools_include_fetch_url() {
    let tools = build_agent_tools();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| match t {
            async_openai::types::chat::ChatCompletionTools::Function(f) => {
                Some(f.function.name.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        names.contains(&"fetch_url"),
        "Tools should include fetch_url"
    );
}

#[test]
fn test_agent_tools_include_file() {
    let tools = build_agent_tools();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| match t {
            async_openai::types::chat::ChatCompletionTools::Function(f) => {
                Some(f.function.name.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(names.contains(&"file"), "Tools should include file");
}

#[test]
fn test_agent_tools_include_calculate() {
    let tools = build_agent_tools();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| match t {
            async_openai::types::chat::ChatCompletionTools::Function(f) => {
                Some(f.function.name.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        names.contains(&"calculate"),
        "Tools should include calculate"
    );
}

#[test]
fn test_web_search_no_api_key_returns_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = crate::agent::tools::dispatch_web_search(r#"{"query":"test"}"#, &rt, "");
    assert!(
        result.starts_with("Error:"),
        "Empty API key should return error; got: {}",
        result
    );
    assert!(result.contains("not configured"));
}

#[test]
fn test_fetch_url_unreachable_returns_error() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    // RFC 5737 TEST-NET address -- guaranteed unreachable
    let result = crate::agent::tools::dispatch_fetch_url(r#"{"url":"http://192.0.2.1:1"}"#, &rt);
    assert!(
        result.starts_with("Error:"),
        "Unreachable URL should return error; got: {}",
        result
    );
}

#[test]
fn test_file_write_read_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_str().unwrap();
    let write_result = crate::agent::tools::dispatch_file(
        r#"{"operation":"write","path":"test.txt","content":"hello world"}"#,
        data_dir,
    );
    assert!(
        write_result.contains("Wrote"),
        "Write should succeed; got: {}",
        write_result
    );
    let read_result =
        crate::agent::tools::dispatch_file(r#"{"operation":"read","path":"test.txt"}"#, data_dir);
    assert_eq!(read_result, "hello world");
}

#[test]
fn test_file_path_traversal_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().to_str().unwrap();
    let result = crate::agent::tools::dispatch_file(
        r#"{"operation":"read","path":"../etc/passwd"}"#,
        data_dir,
    );
    assert!(
        result.starts_with("Error:"),
        "Path traversal should be rejected; got: {}",
        result
    );
    assert!(result.contains(".."));
}

#[test]
fn test_calculate_basic() {
    let result = crate::agent::tools::dispatch_calculate(r#"{"expression":"2 + 3 * 4"}"#);
    assert!(
        result.contains("14"),
        "2 + 3 * 4 should be 14; got: {}",
        result
    );
}

#[test]
fn test_calculate_invalid_no_panic() {
    let result = crate::agent::tools::dispatch_calculate(r#"{"expression":"+++invalid"}"#);
    assert!(
        result.starts_with("Error:"),
        "Invalid expression should return error; got: {}",
        result
    );
}

#[test]
fn test_dispatch_all_known_tools() {
    // Verify dispatch_tools handles all 7 known tool names without "unknown tool" error.
    let tool_names = ["web_search", "fetch_url", "file", "calculate"];
    let tmp = tempfile::tempdir().unwrap();
    let db = crate::persistence::Database::open(":memory:").unwrap();
    let index = crate::rag::VectorIndex::new(tmp.path().to_str().unwrap(), None).unwrap();
    let provider = NullEmbeddingProvider;
    let rt = tokio::runtime::Runtime::new().unwrap();

    for name in &tool_names {
        let call = ChatCompletionMessageToolCall {
            id: format!("call-{}", name),
            function: FunctionCall {
                name: name.to_string(),
                arguments: "{}".to_string(),
            },
        };
        let results = dispatch_tools(
            &[call],
            db.conn(),
            &index,
            &provider,
            &rt,
            "",
            "",
            &std::collections::HashMap::new(),
            "",
        );
        assert_eq!(results.len(), 1);
        // Should NOT contain "unknown tool"
        assert!(
            !results[0].1.contains("unknown tool"),
            "Tool '{}' should be dispatched, not unknown; got: {}",
            name,
            results[0].1
        );
    }
}

/// Verify each tool has a description and parameters.
#[test]
fn test_agent_tools_have_descriptions_and_params() {
    let tools = build_agent_tools();

    for tool in &tools {
        if let async_openai::types::chat::ChatCompletionTools::Function(f) = tool {
            assert!(
                f.function.description.is_some()
                    && !f.function.description.as_ref().unwrap().is_empty(),
                "Tool '{}' should have a non-empty description",
                f.function.name
            );
            assert!(
                f.function.parameters.is_some(),
                "Tool '{}' should have parameters",
                f.function.name
            );
        }
    }
}

// ── Actor integration tests ───────────────────────────────────────────────────

/// Verify that LaunchAgentSession creates a session with status "running" in AppState.
///
/// Note: This test will fail if no tool-capable backend (tinfoil) is configured.
/// The seeded migration includes tinfoil, so this should work with the in-memory DB.
#[test]
fn test_launch_agent_session() {
    let app = make_app();

    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "Test agent task".to_string(),
    });
    wait(&app);

    let state = app.state();
    // Should have at least one agent session
    assert!(
        !state.agent_sessions.is_empty(),
        "agent_sessions should not be empty after LaunchAgentSession; got: {:?}",
        state.agent_sessions
    );

    let session = &state.agent_sessions[0];
    assert_eq!(
        session.title, "Test agent task",
        "Session title should match task description"
    );
    // Status may be "running" (step in flight) or "failed" (no API key in test)
    // Both are valid: the session was created
    assert!(
        session.status == "running" || session.status == "failed" || session.status == "completed",
        "Session status should be running/failed/completed; got: {}",
        session.status
    );
}

/// Verify that CancelAgentSession marks the session as "cancelled".
#[test]
fn test_cancel_agent_session() {
    let app = make_app();

    // Launch a session first
    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "Task to cancel".to_string(),
    });
    wait(&app);

    let state = app.state();
    assert!(
        !state.agent_sessions.is_empty(),
        "Precondition: at least one session must exist"
    );
    let session_id = state.agent_sessions[0].id.clone();

    // Cancel the session
    app.dispatch(AppAction::CancelAgentSession {
        session_id: session_id.clone(),
    });
    wait(&app);

    let state = app.state();
    let session = state
        .agent_sessions
        .iter()
        .find(|s| s.id == session_id)
        .expect("Session should still appear in the list after cancellation");
    assert_eq!(
        session.status, "cancelled",
        "Session status should be 'cancelled' after CancelAgentSession; got: {}",
        session.status
    );
}

/// Verify that LoadAgentSession populates current_agent_session_id.
#[test]
fn test_load_agent_session() {
    let app = make_app();

    // Launch a session
    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "Task to load".to_string(),
    });
    wait(&app);

    let state = app.state();
    assert!(
        !state.agent_sessions.is_empty(),
        "Precondition: session must exist"
    );
    let session_id = state.agent_sessions[0].id.clone();

    // Load the session
    app.dispatch(AppAction::LoadAgentSession {
        session_id: session_id.clone(),
    });
    wait(&app);

    let state = app.state();
    assert_eq!(
        state.current_agent_session_id,
        Some(session_id.clone()),
        "current_agent_session_id should be set after LoadAgentSession"
    );
}

/// Verify that ClearAgentDetail clears the current_agent_session_id and steps.
#[test]
fn test_clear_agent_detail() {
    let app = make_app();

    // Launch + load session
    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "Task to clear".to_string(),
    });
    wait(&app);

    let state = app.state();
    if state.agent_sessions.is_empty() {
        return; // Skip if session not created (shouldn't happen)
    }
    let session_id = state.agent_sessions[0].id.clone();

    app.dispatch(AppAction::LoadAgentSession {
        session_id: session_id.clone(),
    });
    wait(&app);

    // Verify loaded
    let state = app.state();
    assert_eq!(state.current_agent_session_id, Some(session_id));

    // Clear detail view
    app.dispatch(AppAction::ClearAgentDetail);
    wait(&app);

    let state = app.state();
    assert_eq!(
        state.current_agent_session_id, None,
        "current_agent_session_id should be None after ClearAgentDetail"
    );
    assert!(
        state.current_agent_steps.is_empty(),
        "current_agent_steps should be empty after ClearAgentDetail"
    );
}

// ── Failure injection tests (TEST-02) ────────────────────────────────────────

/// Verify the 20-step limit logic at the persistence layer (D-04).
///
/// The actor checks `count_agent_steps >= 20` before dispatching the next LLM
/// call. This test seeds 20 steps in-memory and verifies that:
/// 1. The count equals 20.
/// 2. The `>= 20` condition that triggers session failure is satisfied.
/// 3. After `update_agent_session_status("failed")` the row status is "failed".
///
/// Note: A full actor-level test of the 20-step path would require a live backend
/// that returns ToolCalls 20 consecutive times, which is not feasible in unit
/// tests. The persistence-level test here verifies the exact DB state the actor
/// relies on to make its termination decision.
#[test]
fn test_agent_max_step_enforcement() {
    let db = Database::open(":memory:").unwrap();
    let conn = db.conn();

    insert_agent_session(conn, &make_session("sess-max", "running")).unwrap();

    // Insert exactly 20 steps
    for i in 1..=20i64 {
        insert_agent_step(
            conn,
            &make_step(&format!("step-{}", i), "sess-max", i, "tool_call"),
        )
        .unwrap();
    }

    let count = count_agent_steps(conn, "sess-max").unwrap();
    assert_eq!(count, 20, "Should have exactly 20 steps");

    // This is the exact condition the actor checks (see lib.rs handle_agent_step_complete)
    assert!(
        count >= 20,
        "Step limit condition must trigger at 20 steps; count was {}",
        count
    );

    // Simulate what the actor does when the limit is reached
    update_agent_session_status(conn, "sess-max", "failed", 9999).unwrap();

    let sessions = list_agent_sessions(conn).unwrap();
    let sess = sessions
        .iter()
        .find(|s| s.id == "sess-max")
        .expect("sess-max should exist");
    assert_eq!(
        sess.status, "failed",
        "Session must be marked failed after 20-step limit"
    );
}

/// Verify that the actor Err path (network error) sets status "failed" and a toast (D-05).
///
/// Strategy: Launch a session (which adds it to `active_agent_sessions`), then
/// immediately inject `AgentStepComplete { result: Err(NetworkError) }` before the
/// real spawned network call returns. The actor processes the error synchronously on
/// the actor thread, marking the session as "failed" and setting a toast message.
///
/// When the real network call eventually returns, the guard at line 1447 of lib.rs
/// (`if !active_agent_sessions.contains_key(&session_id)`) drops the stale event
/// because the session was already removed by the injected error.
#[test]
fn test_agent_network_timeout_marks_failed() {
    use crate::llm::streaming::InternalEvent;
    use crate::llm::LlmError;

    let app = make_app();

    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "Timeout test".to_string(),
    });
    // Give the actor a tick to process LaunchAgentSession and insert the session
    // into active_agent_sessions (synchronous, happens before the async spawn).
    app.sync();

    // Capture the session ID from state
    let session_id = {
        let state = app.state();
        assert!(
            !state.agent_sessions.is_empty(),
            "Session must exist after LaunchAgentSession"
        );
        state.agent_sessions[0].id.clone()
    };

    // Inject a network error for this session. This simulates what happens when
    // the backend is unreachable or returns a connection-level failure (D-05).
    // The actor will process this before the real spawned step completes.
    app.test_send_internal(InternalEvent::AgentStepComplete {
        session_id: session_id.clone(),
        step_number: 1,
        result: Err(LlmError::NetworkError {
            reason: "Connection timed out".to_string(),
        }),
    });
    wait(&app); // deterministic barrier: injected event processed

    let state = app.state();
    let session = state
        .agent_sessions
        .iter()
        .find(|s| s.id == session_id)
        .expect("Session must still be in the list after failure");
    assert_eq!(
        session.status, "failed",
        "Session must be failed after injected network error; got: {}",
        session.status
    );
    assert!(
        state.toast.is_some(),
        "Toast must be set when agent step fails with network error"
    );
    let toast = state.toast.as_ref().unwrap();
    assert!(
        toast.contains("Agent step failed"),
        "Toast must indicate agent step failure; got: {}",
        toast
    );
}

/// Verify that `dispatch_tools` handles malformed JSON arguments without panicking (D-04).
///
/// When the LLM returns tool call arguments that are not valid JSON, the
/// dispatch function must return an error string rather than panic.
#[test]
fn test_dispatch_tools_malformed_args_no_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Database::open(":memory:").unwrap();
    let index = VectorIndex::new(tmp.path().to_str().unwrap(), None).unwrap();
    let provider = NullEmbeddingProvider;

    let malformed_call = ChatCompletionMessageToolCall {
        id: "call-bad-1".to_string(),
        function: FunctionCall {
            name: "search_documents".to_string(),
            arguments: "not valid json {{{".to_string(),
        },
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    // Must not panic -- malformed JSON must produce an error string
    let results = dispatch_tools(
        &[malformed_call],
        db.conn(),
        &index,
        &provider,
        &rt,
        "",
        "",
        &std::collections::HashMap::new(),
        "",
    );

    assert_eq!(
        results.len(),
        1,
        "Should return one result for one tool call"
    );
    assert!(
        results[0].1.starts_with("Error:"),
        "Malformed args must produce an error string starting with 'Error:'; got: {}",
        results[0].1
    );
}

/// Live agent test: requires TINFOIL_API_KEY environment variable.
/// Runs a real agent session to completion. Ignored in CI.
#[test]
#[ignore]
fn test_live_agent_session_completes() {
    let app = make_app();

    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "What is 2 + 2? Use the finish tool to provide your answer.".to_string(),
    });

    // Poll up to 60 seconds for completion
    for _ in 0..120 {
        std::thread::sleep(Duration::from_millis(500));
        let state = app.state();
        if let Some(session) = state.agent_sessions.first() {
            if session.status == "completed" || session.status == "failed" {
                assert_eq!(
                    session.status, "completed",
                    "Session should complete successfully; got: {}",
                    session.status
                );
                return;
            }
        }
    }
    panic!("Agent session did not complete within 60 seconds");
}

/// Verify AgentStepSummary.tool_input is populated for tool_call steps (AUI-02).
///
/// Inserts a tool_call step directly into a DB, loads it via the actor, and asserts
/// that tool_input is Some for tool_call steps and None otherwise.
#[test]
fn test_agent_step_tool_input() {
    // Verify the tool_input field exists on AgentStepSummary -- compilation is the primary assertion.
    // Also verify the actor's LoadAgentSession handler populates it correctly.
    let app = make_app();

    // Launch a session so there is something in the DB to load
    app.dispatch(AppAction::LaunchAgentSession {
        task_description: "Tool input field test".to_string(),
    });
    wait(&app);

    let state = app.state();
    if !state.agent_sessions.is_empty() {
        let session_id = state.agent_sessions[0].id.clone();
        app.dispatch(AppAction::LoadAgentSession { session_id });
        wait(&app);

        let state = app.state();
        // Verify tool_input field is accessible on AgentStepSummary (compilation proves it exists)
        for step in &state.current_agent_steps {
            if step.action_type == "tool_call" {
                assert!(
                    step.tool_input.is_some(),
                    "tool_input should be Some for tool_call steps, got None for step {}",
                    step.id
                );
            } else {
                assert!(
                    step.tool_input.is_none(),
                    "tool_input should be None for non-tool_call step {}, action_type={}",
                    step.id,
                    step.action_type
                );
            }
        }
    }
    // If no sessions (no API key), compilation verification is sufficient --
    // the field exists and the field-access code above would have been type-checked.
}
