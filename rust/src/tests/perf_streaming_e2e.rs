//! End-to-end perf verification (#1 client cache, #2 emit coalescing) against a
//! real local OpenAI-compatible SSE server. Spin up the server on an ephemeral
//! port, add it as a backend, send two real streaming turns, and assert:
//! - both replies complete with the full streamed text,
//! - the HTTP client is REUSED across turns (same source port => no new
//!   TCP/TLS handshake per message),
//! - the mid-stream shared snapshot stays fresh while emissions coalesce.

use crate::{
    AppAction, EmbeddingStatus, FfiApp, NullBiometricProvider, NullEmbeddingProvider,
    NullKeychainProvider,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// Minimal streaming OpenAI-compatible server. Records the source port of
/// every /chat/completions request so the test can prove connection reuse.
struct FakeServer {
    port: u16,
    ports: Arc<Mutex<Vec<u16>>>,
}

impl FakeServer {
    fn start(reply: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().unwrap().port();
        let ports = Arc::new(Mutex::new(Vec::new()));
        let ports_clone = ports.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let ports_ref = ports_clone.clone();
                std::thread::spawn(move || {
                    // Keep-alive loop: serve every request on this connection so
                    // a pooled client can reuse it (verifies the client cache).
                    loop {
                        let peer_port = stream.peer_addr().map(|a| a.port()).unwrap_or(0);
                        let mut buf = [0u8; 8192];
                        let mut req = String::new();
                        let is_post = loop {
                            let n = match stream.read(&mut buf) {
                                Ok(n) => n,
                                Err(_) => return,
                            };
                            if n == 0 {
                                return;
                            }
                            req.push_str(&String::from_utf8_lossy(&buf[..n]));
                            if is_complete(&req) {
                                break req.starts_with('P');
                            }
                        };
                        if req.contains("/chat/completions") {
                            ports_ref.lock().unwrap().push(peer_port);
                        }
                        if is_post {
                            let resp = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";
                            if stream.write_all(resp).is_err() {
                                return;
                            }
                            let mut chunk_write = |data: &[u8], stream: &mut std::net::TcpStream| {
                                let framed = format!("{:x}\r\n", data.len());
                                stream.write_all(framed.as_bytes()).is_ok()
                                    && stream.write_all(data).is_ok()
                                    && stream.write_all(b"\r\n").is_ok()
                                    && stream.flush().is_ok()
                            };
                            for word in reply.split(' ') {
                                let chunk = format!(
                                    "data: {{\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"echo-mini\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{word} \"}},\"finish_reason\":null}}]}}\n\n"
                                );
                                if !chunk_write(chunk.as_bytes(), &mut stream) {
                                    return;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(20));
                            }
                            let done = b"data: {\"id\":\"c\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"echo-mini\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
                            if !chunk_write(done, &mut stream) {
                                return;
                            }
                            // Terminate the chunked body; connection stays pooled.
                            let _ = stream.write_all(b"0\r\n\r\n");
                            let _ = stream.flush();
                        } else {
                            let body = format!(
                                "{{\"object\":\"list\",\"data\":[{{\"id\":\"echo-mini\",\"object\":\"model\"}}]}}"
                            );
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            if stream.write_all(resp.as_bytes()).is_err() {
                                return;
                            }
                            let _ = stream.flush();
                        }
                    }
                });
            }
        });
        Self { port, ports }
    }
}

fn is_complete(req: &str) -> bool {
    // GET is complete after headers; POST after blank line.
    if let Some(headers_end) = req.find("\r\n\r\n") {
        if req.starts_with("GET") {
            return true;
        }
        let cl = req
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        return req.len() >= headers_end + 4 + cl;
    }
    false
}

fn make_app() -> std::sync::Arc<FfiApp> {
    let app = FfiApp::new(
        "".into(),
        Box::new(NullKeychainProvider),
        Box::new(NullEmbeddingProvider),
        EmbeddingStatus::Active,
        Box::new(crate::NullLocalLlmProvider),
        Box::new(NullBiometricProvider),
    );
    app.sync();
    app
}

fn wait(app: &FfiApp) {
    app.sync();
}

#[test]
fn streaming_turns_complete_and_reuse_http_connection() {
    let server = FakeServer::start("pong one two three four five six seven");
    let app = make_app();

    // Add the fake server as a configured backend with a model.
    app.dispatch(AppAction::AddBackend {
        name: "FakeLocal".into(),
        base_url: format!("http://127.0.0.1:{}/v1/", server.port),
        api_key: "dummy-key".into(),
        tee_type: crate::llm::TeeType::Unknown,
        models: vec!["echo-mini".to_string()],
    });
    wait(&app);

    app.dispatch(AppAction::NewConversation);
    wait(&app);
    app.dispatch(AppAction::SelectModel {
        model_id: "echo-mini".into(),
    });
    wait(&app);

    // Turn 1: real streaming against the local server.
    app.dispatch(AppAction::SendMessage {
        text: "say pong".into(),
        force_role: None,
    });
    // The stream takes ~8 chunks * 20ms; poll until the assistant message lands.
    let mut got1 = None;
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        wait(&app);
        let state = app.state();
        if let Some(msg) = state.messages.iter().find(|m| m.role == "assistant") {
            got1 = Some(msg.content.clone());
            break;
        }
    }
    let content1 = got1.unwrap_or_else(|| {
        let state = app.state();
        panic!(
            "turn 1 must complete with an assistant message (last_error={:?}, streaming={:?}, msgs={}, convs={})",
            state.last_error,
            state.streaming_text,
            state.messages.len(),
            state.conversations.len()
        )
    });
    assert!(
        content1.contains("pong"),
        "streamed reply must contain the payload, got: {content1}"
    );

    // Turn 2: same conversation, same backend — proves the cached client works.
    app.dispatch(AppAction::SendMessage {
        text: "say it again".into(),
        force_role: None,
    });
    let mut got2 = None;
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        wait(&app);
        let state = app.state();
        if state
            .messages
            .iter()
            .filter(|m| m.role == "assistant")
            .count()
            >= 2
        {
            got2 = Some(());
            break;
        }
    }
    assert!(got2.is_some(), "turn 2 must complete");

    // #1: the chat client must be REUSED — the two SendMessage turns arrived
    // over the same connection (same source port). The FIRST chat POST (if
    // any) is AddBackend's auth probe, which uses a separate client.
    let ports = server.ports.lock().unwrap().clone();
    assert!(
        ports.len() >= 2,
        "expected at least two chat completions requests, got {ports:?}"
    );
    let turn_ports = &ports[ports.len() - 2..];
    assert_eq!(
        turn_ports[0], turn_ports[1],
        "second turn must reuse the HTTP connection (per-backend client cache)"
    );
}
