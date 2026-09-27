//! One-shot non-streaming chat completion over a backend's own transport.
//!
//! Shared by memory extraction and chat compaction. Sealed providers
//! (PPQ/Tinfoil/Venice/Redpill) need their own `create_chat_completion`
//! paths — a plain async-openai client can never speak to a sealed endpoint
//! (mirrors the dispatch in `agent::loop::run_agent_step_for_backend`).
//!
//! Local on-device backends are NOT handled here: they have no HTTP surface.
//! Use `llm::local::complete_local_blocking` (via `llm::compact::CompletionTarget`).

use async_openai::types::chat::{
    ChatCompletionRequestMessage, CreateChatCompletionRequestArgs, CreateChatCompletionResponse,
};
use async_openai::{config::OpenAIConfig, Client};

use super::error::LlmError;
use super::BackendConfig;
use crate::llm::ProviderTransportKind;

/// Non-streaming completion routed through the backend's own transport.
///
/// `max_tokens` bounds the completion output (callers keep it small: these are
/// prepaid/summarization outputs, not chat turns).
pub async fn complete(
    backend: &BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    max_tokens: u32,
) -> Result<CreateChatCompletionResponse, LlmError> {
    match backend.transport_kind() {
        ProviderTransportKind::PpqPrivateE2ee => {
            crate::llm::ppq_private::create_chat_completion(
                backend,
                model,
                messages,
                vec![],
                Some(max_tokens),
            )
            .await
        }
        ProviderTransportKind::TinfoilSecure => {
            crate::llm::tinfoil_secure::create_chat_completion(
                backend,
                model,
                messages,
                vec![],
                Some(max_tokens),
            )
            .await
        }
        ProviderTransportKind::VeniceE2ee => {
            crate::llm::venice::create_chat_completion(
                backend.clone(),
                model.to_string(),
                messages,
                None,
                Some(max_tokens),
            )
            .await
        }
        ProviderTransportKind::Redpill => {
            crate::llm::redpill::create_chat_completion(
                backend.clone(),
                model.to_string(),
                messages,
                None,
                Some(max_tokens),
            )
            .await
        }
        ProviderTransportKind::LocalOnDevice => Err(LlmError::NetworkError {
            reason: "local on-device backends have no HTTP completion path; use the on-device completer"
                .to_string(),
        }),
        ProviderTransportKind::OpenAiCompatible => {
            let config = OpenAIConfig::new()
                .with_api_base(&backend.base_url)
                .with_api_key(&backend.api_key);
            let client = Client::with_config(config);
            let request = CreateChatCompletionRequestArgs::default()
                .model(model)
                .max_tokens(max_tokens)
                .messages(messages)
                .build()
                .map_err(|e: async_openai::error::OpenAIError| LlmError::NetworkError {
                    reason: e.to_string(),
                })?;
            client
                .chat()
                .create(request)
                .await
                .map_err(crate::llm::error::map_openai_error)
        }
    }
}
