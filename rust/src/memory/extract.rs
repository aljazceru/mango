use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
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

/// Calls the LLM extraction prompt and returns a list of extracted memory strings.
///
/// Builds a transcript from `messages` (each `(role, content)` pair), sends it to
/// the configured backend, and parses the response as a JSON array of strings.
/// Returns an empty vec on parse failure (graceful degradation).
pub async fn call_extraction_llm(
    backend: &crate::llm::BackendConfig,
    messages: &[(String, String)],
    model: &str,
) -> anyhow::Result<Vec<String>> {
    let transcript = messages
        .iter()
        .map(|(role, content)| format!("{role}: {content}"))
        .collect::<Vec<_>>()
        .join("\n\n");

    let config = OpenAIConfig::new()
        .with_api_base(&backend.base_url)
        .with_api_key(&backend.api_key);
    let client = Client::with_config(config);

    let system_msg: ChatCompletionRequestMessage =
        ChatCompletionRequestSystemMessageArgs::default()
            .content(EXTRACTION_SYSTEM)
            .build()?
            .into();

    let user_msg: ChatCompletionRequestMessage = ChatCompletionRequestUserMessageArgs::default()
        .content(format!("Extract memories from:\n\n{transcript}"))
        .build()?
        .into();

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .max_tokens(512u16)
        .messages([system_msg, user_msg])
        .build()?;

    let response = client.chat().create(request).await?;

    let text = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    let memories: Vec<String> = serde_json::from_str(text.trim()).unwrap_or_default();
    Ok(memories)
}
