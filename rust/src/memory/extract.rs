use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
        CreateChatCompletionResponse, FinishReason,
    },
    Client,
};

/// System prompt for memory extraction.
///
/// Instructs the LLM to extract facts, preferences, and entities from the
/// conversation and return them as a JSON array of strings.
pub const EXTRACTION_SYSTEM: &str = "\
You are a memory extraction assistant.
Extract facts, preferences, and entities from the conversation.
Respond with a JSON array of strings. Each string is one memory fact.
Be concise. Only extract information the user stated or clearly implied.
If nothing is worth remembering, respond with an empty array: []
Example: [\"User prefers dark mode\", \"User's name is Alex\", \"User works at Acme Corp\"]";

/// Minimum total character count across all messages before extraction is attempted.
pub const MIN_EXTRACTION_CHARS: usize = 100;

/// Output-token budget for the extraction completion. Bounded everywhere —
/// PPQ is prepaid per token, and an unbounded sealed completion would quietly
/// drain balance on every turn. 1024 tokens ≈ 40+ concise memory facts.
pub const EXTRACTION_MAX_TOKENS: u32 = 1024;

/// Ceiling for the extraction transcript (chars of conversation text sent as
/// input). Long conversations re-prompt the FULL history after every turn —
/// cumulative input cost grows quadratically. Extract from a bounded recent
/// window instead; older turns have usually already been extracted.
// ponytail: fixed tail window; a per-conversation extraction watermark +
// fact dedup would skip re-extraction entirely — add when memory cost matters.
pub const EXTRACTION_TRANSCRIPT_CHAR_CAP: usize = 60_000;

/// Returns true only if the conversation is large enough to be worth extracting.
///
/// Requires at least 2 messages AND total content character count >= `MIN_EXTRACTION_CHARS`.
pub fn should_extract(messages: &[(String, String)]) -> bool {
    if messages.len() < 2 {
        return false;
    }
    let total_chars: usize = messages.iter().map(|(_, content)| content.len()).sum();
    total_chars >= MIN_EXTRACTION_CHARS
}

/// Trim the conversation to the most recent turns within the transcript cap.
/// The most recent turn is always included — but truncated to the cap if it
/// alone exceeds it (review finding D-5), so a single giant message can never
/// ship unbounded input to the extraction backend.
pub fn bounded_transcript(messages: &[(String, String)]) -> String {
    let mut window: Vec<(&str, &str)> = Vec::new();
    let mut total = 0usize;
    for (role, content) in messages.iter().rev() {
        let len = role.len() + 2 + content.len();
        if !window.is_empty() && total + len > EXTRACTION_TRANSCRIPT_CHAR_CAP {
            break;
        }
        total += len;
        window.push((role.as_str(), content.as_str()));
    }
    window
        .into_iter()
        .rev()
        .map(|(role, content)| {
            // Single oversized turn: keep the newest chars, drop the tail.
            let budget = EXTRACTION_TRANSCRIPT_CHAR_CAP.saturating_sub(role.len() + 2);
            if content.chars().count() > budget {
                let head: String = content.chars().take(budget).collect();
                format!("{role}: {head}\n[older content truncated]")
            } else {
                format!("{role}: {content}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Calls the LLM extraction prompt and returns a list of extracted memory strings.
///
/// Builds a transcript from `messages` (each `(role, content)` pair), sends it to
/// the configured backend over that backend's own transport — sealed providers
/// (PPQ/Tinfoil/Venice/Redpill) need their `create_chat_completion` paths, a
/// plain async-openai client can never speak to a sealed endpoint — and parses
/// the response as a JSON array of strings. Returns an empty vec on parse
/// failure (graceful degradation; truncation is logged, not surfaced).
pub async fn call_extraction_llm(
    backend: &crate::llm::BackendConfig,
    messages: &[(String, String)],
    model: &str,
) -> anyhow::Result<Vec<String>> {
    let transcript = bounded_transcript(messages);

    let system_msg: ChatCompletionRequestMessage =
        ChatCompletionRequestSystemMessageArgs::default()
            .content(EXTRACTION_SYSTEM)
            .build()?
            .into();

    let user_msg: ChatCompletionRequestMessage = ChatCompletionRequestUserMessageArgs::default()
        .content(format!("Extract memories from:\n\n{transcript}"))
        .build()?
        .into();

    let response = complete(backend, model, vec![system_msg, user_msg]).await?;
    let choice = response.choices.first();
    // Distinguish "nothing worth remembering" (finish_reason=stop, "[]") from a
    // truncated JSON body (finish_reason=length) — the latter silently parsed
    // to "no memories" before and hid provider-side output limits.
    if choice.is_none_or(|c| c.finish_reason != Some(FinishReason::Stop)) {
        log::warn!(
            target: "memory",
            "[memory] extraction finished with {:?} (expected stop); memories may be truncated or lost",
            choice.map(|c| c.finish_reason)
        );
    }

    let text = choice
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    let memories: Vec<String> = serde_json::from_str(text.trim()).unwrap_or_default();
    Ok(memories)
}

/// One-shot non-streaming completion routed through the backend's own
/// transport, preserving the extraction output budget on every provider.
/// Mirrors the dispatch in `agent::loop::run_agent_step_for_backend`.
async fn complete(
    backend: &crate::llm::BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
) -> Result<CreateChatCompletionResponse, crate::llm::LlmError> {
    use crate::llm::ProviderTransportKind;
    match backend.transport_kind() {
        ProviderTransportKind::PpqPrivateE2ee => {
            crate::llm::ppq_private::create_chat_completion(
                backend,
                model,
                messages,
                vec![],
                Some(EXTRACTION_MAX_TOKENS),
            )
            .await
        }
        ProviderTransportKind::TinfoilSecure => {
            crate::llm::tinfoil_secure::create_chat_completion(
                backend,
                model,
                messages,
                vec![],
                Some(EXTRACTION_MAX_TOKENS),
            )
            .await
        }
        ProviderTransportKind::VeniceE2ee => {
            crate::llm::venice::create_chat_completion(
                backend.clone(),
                model.to_string(),
                messages,
                None,
                Some(EXTRACTION_MAX_TOKENS),
            )
            .await
        }
        ProviderTransportKind::Redpill => {
            crate::llm::redpill::create_chat_completion(
                backend.clone(),
                model.to_string(),
                messages,
                None,
                Some(EXTRACTION_MAX_TOKENS),
            )
            .await
        }
        ProviderTransportKind::OpenAiCompatible | ProviderTransportKind::LocalOnDevice => {
            let config = OpenAIConfig::new()
                .with_api_base(&backend.base_url)
                .with_api_key(&backend.api_key);
            let client = Client::with_config(config);
            let request = CreateChatCompletionRequestArgs::default()
                .model(model)
                .max_tokens(EXTRACTION_MAX_TOKENS)
                .messages(messages)
                .build()
                .map_err(|e: async_openai::error::OpenAIError| {
                    crate::llm::LlmError::NetworkError {
                        reason: e.to_string(),
                    }
                })?;
            client
                .chat()
                .create(request)
                .await
                .map_err(crate::llm::error::map_openai_error)
        }
    }
}
