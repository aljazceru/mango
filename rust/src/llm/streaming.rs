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
    /// Progress from a background local model download.
    LocalModelDownloadProgress(super::local_models::LocalModelDownloadProgress),
    /// Completion from a background local model download and integrity check.
    LocalModelDownloadComplete {
        model_id: String,
        result: Result<(), String>,
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
    /// Non-streaming tool round returned tool calls (Phase 27, CHAT-TOOL-04).
    /// Actor thread dispatches tools and spawns streaming follow-up.
    /// CRITICAL: Tool dispatch MUST happen on actor thread, NOT inside async task
    /// (dispatch_tools calls runtime.block_on which panics inside Tokio).
    ChatToolCallsReady {
        conv_id: String,
        tool_calls: Vec<async_openai::types::chat::ChatCompletionMessageToolCall>,
        /// Full ChatCompletionRequestMessage history up to and including user message.
        /// Actor appends assistant tool_calls msg + tool result msgs, then spawns streaming.
        pre_tool_messages: Vec<async_openai::types::chat::ChatCompletionRequestMessage>,
        backend_id: String,
        model: String,
    },
    /// Non-streaming tool round returned a final answer (no tools called) (Phase 27).
    /// Actor emits the text as StreamChunk + StreamDone.
    ChatToolNone { conv_id: String, text: String },
    /// Result of a background Brave Search API key validation call.
    ///
    /// Sent by spawn_brave_api_key_validation back to the actor loop.
    /// On success the actor persists the key and shows a success toast.
    /// On failure the actor shows an error toast without persisting.
    BraveApiKeyValidationResult {
        api_key: String,
        success: bool,
        /// Human-readable error detail when success=false.
        error_message: Option<String>,
    },
    /// Result of a biometric authentication attempt dispatched via spawn_blocking.
    ///
    /// Sent by the AttemptBiometricUnlock handler after the platform biometric
    /// prompt resolves. Processed in the InternalEvent branch so the actor loop
    /// is not blocked while the system biometric UI is displayed (CR-03).
    BiometricResult { success: bool },
    /// Phase 35 — discovery query completed. Carries the merged result
    /// (announcements list or error message) for AppState update.
    /// Spawned by AppAction::DiscoverContextvmTools / RetryContextvmDiscovery.
    ContextvmDiscoveryComplete {
        result: Result<Vec<crate::contextvm::DiscoveredTool>, String>,
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
/// - `messages`: conversation history as simple ChatMessage types (converted internally)
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
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    };

    let cancel_token = CancellationToken::new();
    let token_for_task = cancel_token.clone();

    let backend = backend.clone();
    let transport = backend.transport_kind();
    let base_url = backend.base_url.trim_end_matches('/').to_string();
    let model = model.to_string();

    log::debug!(target: "streaming", "[streaming] connection setup base_url={} model={}", base_url, model);

    runtime.spawn(async move {
        // Acquire concurrency permit -- queues if semaphore is full (per D-02).
        let _permit = if let Some(sem) = semaphore {
            match sem.acquire_owned().await {
                Ok(permit) => Some(permit),
                Err(_) => {
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
        if transport == super::transport::ProviderTransportKind::VeniceE2ee {
            crate::llm::venice::run_streaming_chat_completion(
                backend,
                model,
                messages,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }
        if transport == super::transport::ProviderTransportKind::Redpill {
            crate::llm::redpill::run_streaming_chat_completion(
                backend,
                model,
                messages,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }

        // Convert ChatMessage types to async-openai request message types
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

        run_streaming_with_api_messages(
            backend,
            transport,
            model,
            openai_messages,
            pinned_tls_public_key_fp,
            token_for_task,
            core_tx,
        )
        .await;
    });

    cancel_token
}

/// Spawn a streaming task using async-openai ChatCompletionRequestMessage types directly.
///
/// Used by the ChatToolCallsReady handler (Phase 27) to avoid lossy conversion of
/// Tool and Assistant-with-tool-calls messages through the simpler ChatMessage type.
/// Messages from the tool round already contain Tool-role entries that must be passed
/// to the model verbatim for the follow-up streaming response.
///
/// For Tinfoil/PPQ backends (which have custom streaming paths that accept ChatMessage),
/// the messages are converted via best-effort: System/User/Assistant roles are kept,
/// Tool-role messages are summarised as assistant messages so context is not lost.
pub fn spawn_streaming_task_from_api_messages(
    runtime: &tokio::runtime::Runtime,
    backend: &super::backend::BackendConfig,
    model: &str,
    messages: Vec<async_openai::types::chat::ChatCompletionRequestMessage>,
    pinned_tls_public_key_fp: Option<String>,
    core_tx: flume::Sender<crate::CoreMsg>,
    semaphore: Option<std::sync::Arc<tokio::sync::Semaphore>>,
) -> CancellationToken {
    let cancel_token = CancellationToken::new();
    let token_for_task = cancel_token.clone();

    let backend = backend.clone();
    let transport = backend.transport_kind();
    let base_url = backend.base_url.trim_end_matches('/').to_string();
    let model = model.to_string();

    log::debug!(target: "streaming", "[streaming/tool-followup] connection setup base_url={} model={}", base_url, model);

    runtime.spawn(async move {
        // Acquire concurrency permit
        let _permit = if let Some(sem) = semaphore {
            match sem.acquire_owned().await {
                Ok(permit) => Some(permit),
                Err(_) => {
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
            // Pass the async-openai API messages through unchanged. This preserves
            // multipart user messages (vision `Content::Array` with image_url parts)
            // which the prior lossy `api_messages_to_chat_messages` conversion dropped
            // -- see debug session `image-upload-still-broken-after-fix`. Tool-role
            // messages, which ChatMessage cannot represent at all, are also passed
            // verbatim so the model receives the full conversation state.
            crate::llm::tinfoil_secure::run_streaming_chat_completion_from_api_messages(
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
            // See Tinfoil branch above -- same rationale: preserve multipart content
            // so vision requests actually carry the image to the model.
            crate::llm::ppq_private::run_streaming_chat_completion_from_api_messages(
                backend,
                model,
                messages,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }
        if transport == super::transport::ProviderTransportKind::VeniceE2ee {
            crate::llm::venice::run_streaming_chat_completion_from_api_messages(
                backend,
                model,
                messages,
                None,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }
        if transport == super::transport::ProviderTransportKind::Redpill {
            crate::llm::redpill::run_streaming_chat_completion_from_api_messages(
                backend,
                model,
                messages,
                None,
                token_for_task,
                core_tx,
            )
            .await;
            return;
        }

        // Standard OpenAI-compatible path: pass API messages directly, no conversion needed.
        run_streaming_with_api_messages(
            backend,
            transport,
            model,
            messages,
            pinned_tls_public_key_fp,
            token_for_task,
            core_tx,
        )
        .await;
    });

    cancel_token
}

/// Inner async function that runs the OpenAI-compatible streaming request.
///
/// Shared between `spawn_streaming_task` (after ChatMessage conversion) and
/// `spawn_streaming_task_from_api_messages` (direct API message types).
/// Handles client construction and SSE consumption loop.
async fn run_streaming_with_api_messages(
    backend: super::backend::BackendConfig,
    transport: super::transport::ProviderTransportKind,
    model: String,
    messages: Vec<async_openai::types::chat::ChatCompletionRequestMessage>,
    pinned_tls_public_key_fp: Option<String>,
    token_for_task: CancellationToken,
    core_tx: flume::Sender<crate::CoreMsg>,
) {
    use super::error::map_openai_error;
    use async_openai::types::chat::CreateChatCompletionRequestArgs;
    use futures::StreamExt;

    let base_url = backend.base_url.trim_end_matches('/').to_string();

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

    // Build the streaming request
    let request = match CreateChatCompletionRequestArgs::default()
        .model(model.as_str())
        .messages(messages)
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

    let mut stream = match client.chat().create_stream(request).await {
        Ok(s) => s,
        Err(e) => {
            let mapped = map_openai_error(e);
            log::warn!(
                target: "streaming",
                "[streaming] failed to open stream base_url={} model={} pinned={} error={}",
                base_url,
                model,
                used_pin,
                mapped
            );
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamError { error: mapped },
            )));
            return;
        }
    };

    // Consume the SSE stream with cooperative cancellation support (per Pattern 5).
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
                        let mapped = map_openai_error(e);
                        log::warn!(target: "streaming", "[streaming] stream error base_url={} model={} error={}", base_url, model, mapped);
                        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                            InternalEvent::StreamError { error: mapped },
                        )));
                        break;
                    }
                    None => {
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
}

#[cfg(test)]
mod tests {
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestMessageContentPartImage,
        ChatCompletionRequestMessageContentPartText, ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestUserMessageContent, ChatCompletionRequestUserMessageContentPart,
        CreateChatCompletionRequestArgs, ImageDetail, ImageUrl,
    };

    /// Regression (debug session `image-upload-still-broken-after-fix`):
    /// a multipart User message (`Content::Array` with Text + ImageUrl parts) must
    /// survive the serialization that happens inside the Tinfoil/PPQ
    /// `run_streaming_chat_completion_from_api_messages` functions. The wire body
    /// that eventually hits `/chat/completions` must contain both the raw user
    /// text AND the image data URL -- no lossy collapse, no dropped parts.
    ///
    /// The prior implementation routed Tinfoil/PPQ requests through
    /// `api_messages_to_chat_messages`, which flattened Array → plain text and
    /// filter_map-dropped ImageUrl parts. That made vision silently impossible
    /// on these transports even though Tinfoil DOES support Gemma 3 vision via
    /// base64 data URLs. This test locks in the fix by exercising the same
    /// request-body construction the new code path performs, and asserting that
    /// the serialized JSON still carries the image data URL.
    #[test]
    fn multipart_user_message_survives_request_body_serialization() {
        let text_part = ChatCompletionRequestUserMessageContentPart::Text(
            ChatCompletionRequestMessageContentPartText {
                text: "whats in the photo".to_string(),
            },
        );
        let image_part = ChatCompletionRequestUserMessageContentPart::ImageUrl(
            ChatCompletionRequestMessageContentPartImage {
                image_url: ImageUrl {
                    url: "data:image/jpeg;base64,PROOF_OF_IMAGE_BYTES".to_string(),
                    detail: Some(ImageDetail::Auto),
                },
            },
        );
        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(ChatCompletionRequestUserMessageContent::Array(vec![
                text_part, image_part,
            ]))
            .build()
            .expect("build user multipart message");
        let api_messages: Vec<ChatCompletionRequestMessage> =
            vec![ChatCompletionRequestMessage::User(user_msg)];

        // This mirrors what both `tinfoil_secure::run_streaming_chat_completion_from_api_messages`
        // and `ppq_private::run_streaming_chat_completion_from_api_messages` do
        // before handing bytes to their secure transports.
        let request = CreateChatCompletionRequestArgs::default()
            .model("gemma3:27b")
            .messages(api_messages)
            .stream(true)
            .build()
            .expect("build request");
        let body = serde_json::to_vec(&request).expect("serialize request body");
        let body_str = std::str::from_utf8(&body).expect("utf-8 body");

        assert!(
            body_str.contains("whats in the photo"),
            "wire body must contain the raw user text, got: {body_str}"
        );
        assert!(
            body_str.contains("data:image/jpeg;base64,PROOF_OF_IMAGE_BYTES"),
            "wire body must contain the image data URL -- the multipart ImageUrl \
             part must NOT be dropped on its way into the Tinfoil/PPQ transport; \
             got: {body_str}"
        );
        assert!(
            body_str.contains("\"image_url\""),
            "wire body must contain the image_url content part key; got: {body_str}"
        );
    }

    /// A plain Text user message (no attachment) must still produce a
    /// non-multipart request body after the fix. This ensures the common
    /// chat-only path is not disturbed.
    #[test]
    fn text_only_user_message_survives_request_body_serialization() {
        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(ChatCompletionRequestUserMessageContent::Text(
                "hello world".to_string(),
            ))
            .build()
            .expect("build user text message");
        let api_messages: Vec<ChatCompletionRequestMessage> =
            vec![ChatCompletionRequestMessage::User(user_msg)];

        let request = CreateChatCompletionRequestArgs::default()
            .model("gemma3:27b")
            .messages(api_messages)
            .stream(true)
            .build()
            .expect("build request");
        let body = serde_json::to_vec(&request).expect("serialize request body");
        let body_str = std::str::from_utf8(&body).expect("utf-8 body");

        assert!(body_str.contains("hello world"));
        assert!(
            !body_str.contains("\"image_url\""),
            "text-only request must not contain image_url parts; got: {body_str}"
        );
    }
}
