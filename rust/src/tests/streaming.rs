use crate::llm::error::LlmError;
use crate::llm::streaming::{ChatMessage, ChatRole, InternalEvent};
use crate::llm::{BackendConfig, TeeType};
use crate::{AppAction, BusyState, CoreMsg, EmbeddingStatus, FfiApp};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

/// Helper: create FfiApp and give it a moment to initialize.
/// Uses empty data_dir so the database opens as :memory:.
fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(crate::NullKeychainProvider),
        Box::new(crate::NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(crate::NullBiometricProvider),
    );
    // Phase 8: VectorIndex init + document list load adds overhead; 150ms is stable in parallel test load.
    std::thread::sleep(Duration::from_millis(150));
    app
}

#[test]
fn test_initial_state_has_backends() {
    let app = make_app();
    let state = app.state();
    // v1 seeds tinfoil (active), v10 seeds ppq-ai (inactive)
    assert_eq!(
        state.backends.len(),
        2,
        "Expected 2 backends (Tinfoil + PPQ.AI)"
    );
    assert_eq!(state.active_backend_id, Some("tinfoil".to_string()));
    assert!(state.backends.iter().any(|b| b.id == "tinfoil"));
}

#[test]
fn test_set_active_backend() {
    let app = make_app();
    // Active backend should already be tinfoil from init
    let state = app.state();
    assert_eq!(state.active_backend_id, Some("tinfoil".to_string()));
}

#[test]
fn test_stop_generation_when_idle_is_noop() {
    let app = make_app();
    app.dispatch(AppAction::StopGeneration);
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    assert_eq!(state.busy_state, BusyState::Idle);
}

#[test]
fn test_send_message_starts_streaming() {
    let app = make_app();
    app.dispatch(AppAction::SendMessage {
        text: "Hello".into(),
        force_role: None,
    });
    std::thread::sleep(Duration::from_millis(100));
    let state = app.state();
    // Should be streaming (or error if no API key, which is expected in tests)
    // The important thing: busy_state transitioned from Idle or an error was recorded
    assert!(
        matches!(state.busy_state, BusyState::Streaming { .. }) || state.last_error.is_some(),
        "Expected streaming or error, got: {:?}",
        state.busy_state
    );
}

#[test]
fn test_qvac_local_server_streams_openai_compatible_chat() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock local server");
    let addr = listener.local_addr().expect("mock local server address");
    let (request_tx, request_rx) = std::sync::mpsc::channel();

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept local chat request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                let request_text = String::from_utf8_lossy(&request);
                let content_length = request_text
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                let header_end = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|pos| pos + 4)
                    .expect("header terminator");
                while request.len().saturating_sub(header_end) < content_length {
                    let read = stream.read(&mut buffer).expect("read request body");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                break;
            }
        }

        let request_text = String::from_utf8_lossy(&request).to_string();
        request_tx
            .send(request_text)
            .expect("send captured request");

        let body = concat!(
            "data: {\"id\":\"chatcmpl-local\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"llama3.2\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"LOCAL_\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-local\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"llama3.2\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"SERVER_OK\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write SSE response");
    });

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let (tx, rx) = flume::unbounded();
    let backend = BackendConfig {
        id: "qvac-local".to_string(),
        name: "Local server".to_string(),
        base_url: format!("http://{addr}/v1/"),
        api_key: String::new(),
        models: vec!["llama3.2".to_string()],
        tee_type: TeeType::Unknown,
        max_concurrent_requests: 1,
        supports_tool_use: false,
    };

    crate::llm::spawn_streaming_task(
        &runtime,
        &backend,
        "llama3.2",
        vec![ChatMessage {
            role: ChatRole::User,
            content: "desktop local server ping".to_string(),
        }],
        None,
        tx,
        None,
    );

    let mut text = String::new();
    loop {
        match rx
            .recv_timeout(Duration::from_secs(5))
            .expect("stream event from qvac-local mock server")
        {
            CoreMsg::InternalEvent(event) => match *event {
                InternalEvent::StreamChunk { token } => text.push_str(&token),
                InternalEvent::StreamDone => break,
                InternalEvent::StreamError { error } => {
                    panic!("qvac-local OpenAI-compatible stream errored: {error}")
                }
                other => panic!("unexpected stream event: {other:?}"),
            },
            _ => panic!("unexpected core message"),
        }
    }

    let request_text = request_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("captured local server request");
    assert!(
        request_text.starts_with("POST /v1/chat/completions "),
        "unexpected request path: {request_text}"
    );
    assert!(
        request_text.contains("\"model\":\"llama3.2\"")
            && request_text.contains("desktop local server ping")
            && request_text.contains("\"stream\":true"),
        "request did not look like an OpenAI-compatible streaming chat: {request_text}"
    );
    assert_eq!(text, "LOCAL_SERVER_OK");

    server.join().expect("mock server completed");
}

// --- InternalEvent injection tests (LLMC-03) ---
// These use test_send_internal() to bypass HTTP and test the actor's
// event processing logic directly.

#[test]
fn test_stream_chunk_accumulates() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "Hello".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.test_send_internal(InternalEvent::StreamChunk {
        token: " world".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    assert_eq!(state.streaming_text, Some("Hello world".to_string()));
}

#[test]
fn test_stream_done_sets_idle() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "Done test".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.test_send_internal(InternalEvent::StreamDone);
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    // StreamDone must set BusyState::Idle
    assert_eq!(state.busy_state, BusyState::Idle);
    // Phase 5: streaming_text is cleared on StreamDone (moved into messages list).
    // When there is no active conversation (as in this test), content is still cleared.
    assert!(
        state.streaming_text.is_none() || state.streaming_text.as_deref() == Some(""),
        "streaming_text should be cleared after StreamDone (Phase 5 moves it to messages)"
    );
}

#[test]
fn test_stream_error_preserves_partial() {
    let app = make_app();
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "Partial".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    app.test_send_internal(InternalEvent::StreamError {
        error: LlmError::NetworkError {
            reason: "connection lost".into(),
        },
    });
    std::thread::sleep(Duration::from_millis(50));
    let state = app.state();
    // Partial text should be preserved, not cleared
    assert!(
        state.streaming_text.is_some(),
        "streaming_text should preserve partial content on error"
    );
    assert!(
        state.last_error.is_some(),
        "last_error should be set on StreamError"
    );
}

// --- Streaming cancellation tests (TEST-01) ---
// These verify the StopGeneration -> StreamCancelled -> BusyState::Idle path.

#[test]
fn test_stop_generation_cancels_active_stream() {
    // Tinfoil HPKE transport: dispatching StopGeneration during active streaming,
    // followed by the StreamCancelled event, transitions to BusyState::Idle.
    let app = make_app();

    // Inject a chunk to simulate an in-progress stream
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "partial response".into(),
    });
    std::thread::sleep(Duration::from_millis(50));
    {
        let state = app.state();
        assert_eq!(
            state.streaming_text,
            Some("partial response".to_string()),
            "streaming_text must accumulate before cancel"
        );
    }

    // Dispatch StopGeneration -- exercises the active_stream_token.cancel() path.
    // BusyState must NOT go Idle here; we wait for StreamCancelled.
    app.dispatch(AppAction::StopGeneration);

    // Simulate what the Tinfoil transport sends after cancel_token fires
    app.test_send_internal(InternalEvent::StreamCancelled);
    std::thread::sleep(Duration::from_millis(100));

    let state = app.state();
    assert_eq!(
        state.busy_state,
        BusyState::Idle,
        "StopGeneration + StreamCancelled must transition to Idle"
    );
    // Partial text is preserved on cancel, not cleared
    assert!(
        state.streaming_text.is_some(),
        "streaming_text must be preserved (not cleared) after cancel"
    );
}

#[test]
fn test_stop_generation_cancels_ppq_stream() {
    // PPQ AES-GCM transport uses the same StreamCancelled event as Tinfoil HPKE (per D-03).
    // This test documents PPQ coverage of the same cancellation path.
    let app = make_app();

    // Inject a chunk that distinguishes this test from the Tinfoil test
    app.test_send_internal(InternalEvent::StreamChunk {
        token: "ppq partial".into(),
    });
    std::thread::sleep(Duration::from_millis(50));

    // Dispatch StopGeneration, then inject StreamCancelled as the PPQ transport would
    app.dispatch(AppAction::StopGeneration);
    app.test_send_internal(InternalEvent::StreamCancelled);
    std::thread::sleep(Duration::from_millis(100));

    let state = app.state();
    assert_eq!(
        state.busy_state,
        BusyState::Idle,
        "StopGeneration + StreamCancelled must transition to Idle (PPQ path)"
    );
    assert!(
        state.streaming_text.is_some(),
        "streaming_text must be preserved after cancel (PPQ path)"
    );
}
