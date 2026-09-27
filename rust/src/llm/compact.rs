//! Chat compaction: summarize older turns into a compact prefix for the model.
//!
//! User-offered, never destructive — the full message history stays on the
//! device untouched. `compaction_summary` + `compaction_covered_count` on the
//! conversation row are derived state used only when assembling the prompt.
//! Older turns are replaced by the summary in the WIRE prompt only.

use std::sync::Arc;

use async_openai::types::chat::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
};

use super::error::LlmError;
use super::local::LocalLlmProvider;
use super::streaming::{ChatMessage, ChatRole};
use super::BackendConfig;

/// Recent messages kept verbatim after the compaction point (the live tail).
pub const COMPACT_KEEP_TAIL_MESSAGES: usize = 2;

/// Fallback summarizer input budget (chars) when the model's window is unknown.
const DEFAULT_CHUNK_CHAR_BUDGET: usize = 24_000;

/// Output budget for summarization calls. Compaction is user-triggered and
/// rare, so the summary can be richer than the per-turn extraction budget.
const SUMMARY_MAX_TOKENS: u32 = 2048;

/// Prompt overhead reserve for summarizer calls (system prompt + framing).
const SUMMARY_PROMPT_OVERHEAD_TOKENS: u64 = 64;

const PARTIAL_SYSTEM: &str = "\
You compress chat transcripts so an AI assistant can keep working within its context window.
Summarize the transcript below. Preserve: facts and decisions, user preferences and personal \
details, names and entities, unresolved questions and planned next steps, and any code or data \
the assistant will need later. Be concise. Output only the summary, no preamble.";

const MERGE_SYSTEM: &str = "\
You compress chat transcripts so an AI assistant can keep working within its context window.
Combine the prior summary and the newer transcript summary below into one coherent summary. \
Preserve the same kind of details. If they conflict, the newer content wins. Be concise. \
Output only the merged summary, no preamble.";

/// What one compaction pass will cover: messages
/// `[already_covered .. total - COMPACT_KEEP_TAIL_MESSAGES)`.
#[derive(Debug, Clone)]
pub struct CompactionPlan {
    /// `(role, content)` of the messages the summary will cover, in order.
    pub covered: Vec<(String, String)>,
    /// Covered ordinal to persist on success (message count the summary covers).
    pub covered_count: usize,
}

/// Plan a compaction over the full conversation history.
///
/// Returns `None` when there is nothing new to compact (conversation too
/// short, or everything beyond the kept tail is already covered).
pub fn plan_compaction(
    messages: &[(String, String)],
    already_covered: usize,
) -> Option<CompactionPlan> {
    let covered_count = messages.len().checked_sub(COMPACT_KEEP_TAIL_MESSAGES)?;
    if covered_count <= already_covered {
        return None;
    }
    Some(CompactionPlan {
        covered: messages[already_covered..covered_count].to_vec(),
        covered_count,
    })
}

/// Format `(role, content)` messages into standalone transcript chunks that
/// each fit `budget_chars`, so the summarizer itself fits small models.
pub fn chunk_transcripts(messages: &[(String, String)], budget_chars: usize) -> Vec<String> {
    let budget = budget_chars.max(1);
    let marker = "\n[older content truncated]";
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for (role, content) in messages {
        let line = format!("{role}: {content}\n\n");
        if line.chars().count() > budget {
            // Single oversized turn: keep the newest chars, drop the tail —
            // matching `memory::extract::bounded_transcript` semantics.
            let keep = budget.saturating_sub(marker.chars().count());
            let clipped: String = content.chars().take(keep).collect();
            let line = format!("{role}: {clipped}{marker}\n\n");
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            chunks.push(line.trim_end().to_string());
            continue;
        }
        if current.chars().count() + line.chars().count() > budget && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        current.push_str(&line);
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim_end().to_string());
    }
    chunks
}

/// Where summarization calls run: the conversation's own model when it is a
/// remote backend, otherwise the on-device engine.
pub enum CompletionTarget<'a> {
    Remote {
        backend: &'a BackendConfig,
        model: &'a str,
    },
    Local {
        provider: Arc<dyn LocalLlmProvider>,
        data_dir: String,
        backend_id: &'a str,
        model_id: &'a str,
    },
}

impl CompletionTarget<'_> {
    /// Summarizer input budget in chars, sized from the model's REAL window
    /// (or the fallback when the window is unknown).
    fn chunk_budget_chars(&self, known_context_tokens: Option<u64>) -> usize {
        let prompt_tokens = match self {
            Self::Remote { .. } => known_context_tokens,
            Self::Local { provider, .. } => {
                let budget = provider.max_prompt_tokens();
                (budget > 0).then_some(budget as u64)
            }
        };
        match prompt_tokens {
            Some(tokens) => {
                let usable = tokens.saturating_sub(SUMMARY_MAX_TOKENS as u64 + SUMMARY_PROMPT_OVERHEAD_TOKENS);
                (usable * 4).max(1_024) as usize
            }
            None => DEFAULT_CHUNK_CHAR_BUDGET,
        }
    }
}

/// Summarize covered messages into a compaction summary, merging with any
/// prior summary so repeated compactions stay coherent.
///
/// `known_context_tokens` is the model's real input window when learned
/// (sizes the summarizer chunks); `None` uses the fallback budget.
pub async fn summarize_for_compaction(
    target: &CompletionTarget<'_>,
    prior_summary: Option<&str>,
    covered: &[(String, String)],
    known_context_tokens: Option<u64>,
) -> Result<String, LlmError> {
    let chunks = chunk_transcripts(covered, target.chunk_budget_chars(known_context_tokens));
    if chunks.is_empty() {
        return Err(LlmError::NetworkError {
            reason: "nothing to compact".to_string(),
        });
    }

    let mut partials = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let messages = vec![
            system(PARTIAL_SYSTEM),
            user(format!("Extract memories from:\n\n{chunk}")),
        ];
        let text = run_completion(target, messages).await?;
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            partials.push(trimmed.to_string());
        }
    }

    match (prior_summary, partials.len()) {
        (None, 1) => Ok(partials.remove(0)),
        (prior, _) => {
            let mut merge_input = String::new();
            if let Some(prior) = prior {
                merge_input.push_str("Prior summary:\n");
                merge_input.push_str(prior);
                merge_input.push_str("\n\n");
            }
            merge_input.push_str("Newer transcript summary:\n");
            merge_input.push_str(&partials.join("\n\n"));
            let messages = vec![system(MERGE_SYSTEM), user(merge_input)];
            let merged = run_completion(target, messages).await?;
            let trimmed = merged.trim();
            if trimmed.is_empty() {
                return Err(LlmError::NetworkError {
                    reason: "compaction produced an empty summary".to_string(),
                });
            }
            Ok(trimmed.to_string())
        }
    }
}

fn system(content: &str) -> ChatMessage {
    ChatMessage {
        role: ChatRole::System,
        content: content.to_string(),
    }
}

fn user(content: String) -> ChatMessage {
    ChatMessage {
        role: ChatRole::User,
        content,
    }
}

async fn run_completion(
    target: &CompletionTarget<'_>,
    messages: Vec<ChatMessage>,
) -> Result<String, LlmError> {
    match target {
        CompletionTarget::Remote { backend, model } => {
            let api_messages = to_api_messages(&messages)?;
            let response =
                super::complete::complete(backend, model, api_messages, SUMMARY_MAX_TOKENS).await?;
            Ok(response
                .choices
                .first()
                .and_then(|choice| choice.message.content.clone())
                .unwrap_or_default())
        }
        CompletionTarget::Local {
            provider,
            data_dir,
            backend_id,
            model_id,
        } => {
            let provider = provider.clone();
            let data_dir = data_dir.clone();
            let backend_id = (*backend_id).to_string();
            let model_id = (*model_id).to_string();
            tokio::task::spawn_blocking(move || {
                super::local::complete_local_blocking(
                    provider,
                    data_dir,
                    &backend_id,
                    &model_id,
                    &messages,
                )
            })
            .await
            .map_err(|error| LlmError::NetworkError {
                reason: format!("on-device compaction task failed: {error}"),
            })?
        }
    }
}

fn to_api_messages(
    messages: &[ChatMessage],
) -> Result<Vec<ChatCompletionRequestMessage>, LlmError> {
    let mut api_messages = Vec::with_capacity(messages.len());
    for message in messages {
        let built = match message.role {
            ChatRole::System => ChatCompletionRequestSystemMessageArgs::default()
                .content(message.content.as_str())
                .build()
                .map(ChatCompletionRequestMessage::from),
            ChatRole::User => ChatCompletionRequestUserMessageArgs::default()
                .content(message.content.as_str())
                .build()
                .map(ChatCompletionRequestMessage::from),
            ChatRole::Assistant => ChatCompletionRequestAssistantMessageArgs::default()
                .content(message.content.as_str())
                .build()
                .map(ChatCompletionRequestMessage::from),
        };
        api_messages.push(built.map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?);
    }
    Ok(api_messages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convo(n: usize) -> Vec<(String, String)> {
        (0..n)
            .map(|i| ("user".to_string(), format!("message {i}")))
            .collect()
    }

    #[test]
    fn plan_keeps_live_tail_and_skips_covered() {
        let messages = convo(6);
        let plan = plan_compaction(&messages, 0).expect("must plan");
        assert_eq!(plan.covered_count, 4);
        assert_eq!(plan.covered.len(), 4);
        assert_eq!(plan.covered[0].1, "message 0");
        // Already-covered messages are not re-summarized.
        assert!(plan_compaction(&messages, 4).is_none());
    }

    #[test]
    fn plan_needs_more_than_the_tail() {
        assert!(plan_compaction(&convo(2), 0).is_none());
        assert!(plan_compaction(&convo(0), 0).is_none());
    }

    #[test]
    fn chunks_respect_budget() {
        let messages: Vec<(String, String)> = (0..10)
            .map(|i| ("user".to_string(), format!("{}{}", "x".repeat(200), i)))
            .collect();
        let chunks = chunk_transcripts(&messages, 500);
        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
        for chunk in &chunks {
            assert!(chunk.chars().count() <= 500 + 32, "chunk too big");
        }
        // Every message's content is preserved across the chunks.
        let joined = chunks.join("");
        for i in 0..10 {
            assert!(
                joined.contains(&format!("x{i}")),
                "message {i} content lost"
            );
        }
    }

    #[test]
    fn oversized_single_turn_is_clipped_not_shipped_whole() {
        let messages = vec![("user".to_string(), "y".repeat(10_000))];
        let chunks = chunk_transcripts(&messages, 500);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].chars().count() <= 500 + 32);
        assert!(chunks[0].contains("older content truncated"));
    }
}
