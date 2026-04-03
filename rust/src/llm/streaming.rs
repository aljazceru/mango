use super::error::LlmError;
use tokio_util::sync::CancellationToken;

/// Simple message role for passing conversation context to the streaming task.
#[derive(Clone, Debug)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// Simple message type for passing conversation context to the streaming task.
/// Not UniFFI-exported -- the full message model comes in Phase 4/5.
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

/// Internal streaming events -- never crosses UniFFI boundary.
/// Boxed inside CoreMsg to avoid bloating the enum (per RESEARCH.md anti-pattern note).
#[derive(Debug)]
pub enum InternalEvent {
    /// A token chunk from SSE stream
    StreamChunk { token: String },
    /// Stream completed naturally (received [DONE] or stream ended)
    StreamDone,
    /// Stream encountered an error mid-response (per D-12: partial message preserved)
    StreamError { error: LlmError },
    /// Stream was cancelled by user via StopGeneration action
    StreamCancelled,
    /// Attestation verification result from a background attestation task (Phase 3).
    /// Carries the full AttestationEvent so the actor loop can update AppState
    /// and persist to the SQLite cache.
    AttestationResult(crate::attestation::AttestationEvent),
    /// Result of a background health check against a backend.
    /// Used by Plan 02 actor loop to call mark_failed / mark_success on the router.
    HealthCheckResult {
        backend_id: String,
        success: bool,
        models: Vec<String>,
    },
    /// Embedding computation completed for a document ingestion batch (Phase 8, D-15).
    ///
    /// Delivered to the actor loop after spawn_blocking returns from the EmbeddingProvider.
    /// The actor adds the embeddings to the VectorIndex and clears ingestion_progress.
    EmbeddingComplete {
        document_id: String,
        chunk_rowids: Vec<i64>,
        embeddings: Vec<f32>,
    },
    /// A single agent step completed (Phase 9, D-03, AGNT-01).
    ///
    /// Delivered from the Tokio runtime.spawn task back to the actor loop.
    /// The actor checkpoints the step to SQLite, dispatches tools if needed,
    /// and either spawns the next step or terminates the session.
    AgentStepComplete {
        session_id: String,
        step_number: i64,
        result: Result<crate::agent::AgentStepResult, super::LlmError>,
    },
    /// Periodic attestation timer tick.
    ///
    /// Sent by the background timer task at each configured interval.
    /// The actor re-runs spawn_attestation_task for the current active backend.
    AttestationTick,
    /// Memory extraction completed for a conversation turn (Phase 20, MEM-01, MEM-07).
    ///
    /// Delivered from the Tokio runtime.spawn extraction task back to the actor loop.
    /// The actor inserts memories into SQLite and adds embeddings to the vector index.
    MemoryExtractionComplete {
        conversation_id: String,
        /// Each string is one extracted memory fact. Empty vec means nothing to store.
        memories: Vec<String>,
    },
}

/// Spawn an async-openai streaming task on the given Tokio runtime.
///
/// The task sends InternalEvent messages back to the actor loop via `core_tx`.
/// Returns a CancellationToken that the caller stores to support StopGeneration.
///
/// # Arguments
/// - `runtime`: the Tokio runtime owned by the actor thread
/// - `backend`: which provider to use (base_url + api_key)
/// - `model`: model ID string
/// - `messages`: conversation history
/// - `core_tx`: flume sender for InternalEvent delivery back to the actor
/// - `semaphore`: optional per-backend concurrency limiter; permit acquired at task start
pub fn spawn_streaming_task(
    runtime: &tokio::runtime::Runtime,
    backend: &super::backend::BackendConfig,
    model: &str,
    messages: Vec<ChatMessage>,
    pinned_tls_public_key_fp: Option<String>,
    core_tx: flume::Sender<crate::CoreMsg>,
    semaphore: Option<std::sync::Arc<tokio::sync::Semaphore>>,
) -> CancellationToken {
    use super::error::map_openai_error;
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    };
    use futures::StreamExt;

    let cancel_token = CancellationToken::new();
    let token_for_task = cancel_token.clone();

    let backend = backend.clone();
    let transport = backend.transport_kind();
    let base_url = backend.base_url.trim_end_matches('/').to_string();
    let model = model.to_string();
    let pinned_tls_public_key_fp = pinned_tls_public_key_fp.clone();

    log::debug!(target: "streaming", "[streaming] connection setup base_url={} model={}", base_url, model);

    runtime.spawn(async move {
        // Acquire concurrency permit -- queues if semaphore is full (per D-02).
        // The permit is held for the entire streaming task duration and released on drop.
        let _permit = if let Some(sem) = semaphore {
            match sem.acquire_owned().await {
                Ok(permit) => Some(permit),
                Err(_) => {
                    // Semaphore closed -- should not happen in normal operation
                    let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                        InternalEvent::StreamError {
                            error: LlmError::NetworkError {
                                reason: "Concurrency limiter closed".into(),
                            },
                        },
                    )));
                    return;
                }
            }
        } else {
            None
        };

        if transport == super::transport::ProviderTransportKind::TinfoilSecure {
            crate::llm::tinfoil_secure::run_streaming_chat_completion(
                backend,
                model,
                messages,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }
        if transport == super::transport::ProviderTransportKind::PpqPrivateE2ee {
            crate::llm::ppq_private::run_streaming_chat_completion(
                backend,
                model,
                messages,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }

        let make_client = |pin: Option<&str>| {
            transport.build_openai_client(&backend, pin, std::time::Duration::from_secs(60))
        };
        let (client, used_pin) = match make_client(pinned_tls_public_key_fp.as_deref()) {
            Ok(client) => client,
            Err(error) => {
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    InternalEvent::StreamError { error },
                )));
                return;
            }
        };

        // Convert our ChatMessage types to async-openai request message types
        let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
        for msg in &messages {
            let result: Result<ChatCompletionRequestMessage, String> = match msg.role {
                ChatRole::System => ChatCompletionRequestSystemMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                    .map_err(|e| e.to_string()),
                ChatRole::User => ChatCompletionRequestUserMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                    .map_err(|e| e.to_string()),
                ChatRole::Assistant => ChatCompletionRequestAssistantMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                    .map_err(|e| e.to_string()),
            };
            match result {
                Ok(m) => openai_messages.push(m),
                Err(e) => {
                    let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                        InternalEvent::StreamError {
                            error: LlmError::NetworkError { reason: e },
                        },
                    )));
                    return;
                }
            }
        }

        // Build the streaming request
        let request = match CreateChatCompletionRequestArgs::default()
            .model(model.as_str())
            .messages(openai_messages)
            .stream(true)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                let err_msg = e.to_string();
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    InternalEvent::StreamError {
                        error: LlmError::NetworkError { reason: err_msg },
                    },
                )));
                return;
            }
        };

        let mut stream = match client.chat().create_stream(request.clone()).await {
            Ok(s) => s,
            Err(e) if used_pin => {
                let mapped = map_openai_error(e);
                log::warn!(
                    target: "streaming",
                    "[streaming] pinned stream open failed base_url={} model={} error={} retrying unpinned",
                    base_url,
                    model,
                    mapped
                );
                let (retry_client, _) = match make_client(None) {
                    Ok(client) => client,
                    Err(error) => {
                        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                            InternalEvent::StreamError { error },
                        )));
                        return;
                    }
                };
                match retry_client.chat().create_stream(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let mapped = map_openai_error(e);
                        log::warn!(target: "streaming", "[streaming] failed to open stream base_url={} model={} error={}", base_url, model, mapped);
                        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                            InternalEvent::StreamError {
                                error: mapped,
                            },
                        )));
                        return;
                    }
                }
            }
            Err(e) => {
                let mapped = map_openai_error(e);
                log::warn!(target: "streaming", "[streaming] failed to open stream base_url={} model={} error={}", base_url, model, mapped);
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    InternalEvent::StreamError {
                        error: mapped,
                    },
                )));
                return;
            }
        };

        // Consume the SSE stream with cooperative cancellation support (per Pattern 5).
        // tokio::select! races the cancellation signal against the next chunk.
        loop {
            tokio::select! {
                biased;  // check cancellation first to avoid processing extra chunks
                _ = token_for_task.cancelled() => {
                    log::debug!(target: "streaming", "[streaming] stream cancelled base_url={} model={}", base_url, model);
                    let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                        InternalEvent::StreamCancelled,
                    )));
                    break;
                }
                chunk = stream.next() => {
                    match chunk {
                        Some(Ok(response)) => {
                            // Extract delta content from first choice (SSE chunk)
                            if let Some(content) = response
                                .choices
                                .first()
                                .and_then(|c| c.delta.content.as_deref())
                            {
                                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                                    InternalEvent::StreamChunk {
                                        token: content.to_string(),
                                    },
                                )));
                            }
                        }
                        Some(Err(e)) => {
                            // Mid-stream error -- per D-12 the partial message is preserved
                            let mapped = map_openai_error(e);
                            log::warn!(target: "streaming", "[streaming] stream error base_url={} model={} error={}", base_url, model, mapped);
                            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                                InternalEvent::StreamError {
                                    error: mapped,
                                },
                            )));
                            break;
                        }
                        None => {
                            // Stream ended naturally ([DONE] sentinel received by async-openai)
                            log::debug!(target: "streaming", "[streaming] stream completed base_url={} model={}", base_url, model);
                            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                                InternalEvent::StreamDone,
                            )));
                            break;
                        }
                    }
                }
            }
        }
    });

    cancel_token
}
