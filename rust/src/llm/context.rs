//! Model context window handling.
//!
//! Prompts are bounded by the model's REAL limits, never by artificial caps:
//! - Local models report the prompt budget their engine actually enforces.
//! - Remote models report their own limit via 400 context-overflow errors,
//!   which state the exact maximum context length (`model_context_limits`
//!   remembers it per backend/model).
//! - When the limit is unknown there is no client-side pre-check at all —
//!   the provider's own limit is the only bound.

use super::streaming::ChatMessage;

/// Rough token estimate: ~4 chars/token, the standard English heuristic.
/// Deliberately a mild UNDERestimate (CJK/code tokenize denser): overestimating
/// would waste real model context, and the provider's own limit remains the
/// enforcement backstop — overflow errors teach us the exact numbers.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64 + 3) / 4
}

/// Wire-formatting overhead per message (role markers, separators).
const PER_MESSAGE_OVERHEAD_TOKENS: u64 = 8;

/// Estimated wire size of a full prompt.
pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> u64 {
    messages
        .iter()
        .map(|message| estimate_tokens(&message.content) + PER_MESSAGE_OVERHEAD_TOKENS)
        .sum()
}

/// Provider-reported context overflow, in tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextOverflow {
    /// Input tokens we sent, when the provider states them.
    pub input_tokens: Option<u64>,
    /// The model's maximum context length.
    pub limit_tokens: u64,
}

/// Parse a provider's context-overflow error body.
///
/// Matches the formats providers actually emit, e.g.
/// `Input length (139080) exceeds model's maximum context length (131072).`
/// and `This model's maximum context length is 8192 tokens. However, you
/// requested 9000 tokens in the messages...`.
pub fn parse_context_overflow(reason: &str) -> Option<ContextOverflow> {
    let limit_tokens = first_u64_after(reason, "maximum context length")?;
    if limit_tokens == 0 {
        return None;
    }
    let input_tokens = first_u64_after(reason, "input length");
    Some(ContextOverflow {
        input_tokens,
        limit_tokens,
    })
}

/// First integer (commas allowed) appearing after a case-insensitive needle.
fn first_u64_after(text: &str, needle: &str) -> Option<u64> {
    let haystack = text.to_lowercase();
    let position = haystack.find(needle)? + needle.len();
    let digits: String = text[position..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .filter(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Outcome of fitting a prompt into a model's input window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FitOutcome {
    /// The prompt fits as-is.
    Fits,
    /// A single oversized message (e.g. a pasted document) was truncated on
    /// the wire to fit. Device-side content is untouched.
    TruncatedMessage,
    /// The prompt is too long even with the newest message shrunk — older
    /// turns must be compacted (summarized) to fit.
    OverBudget {
        needed_tokens: u64,
        limit_tokens: u64,
    },
}

/// Fit a prompt to a model's input window.
///
/// Never silently drops turns — that is compaction's job and it is
/// user-offered. The only automatic shrink is truncating the final message
/// when the whole prompt does not fit without it: the model physically
/// cannot accept more, and history on the device stays intact.
pub fn fit_messages_to_limit(messages: &mut Vec<ChatMessage>, limit_tokens: u64) -> FitOutcome {
    let needed_tokens = estimate_messages_tokens(messages);
    if needed_tokens <= limit_tokens {
        return FitOutcome::Fits;
    }

    // Budget for the final message once every earlier message is accounted for.
    let prefix_tokens: u64 = messages[..messages.len().saturating_sub(1)]
        .iter()
        .map(|message| estimate_tokens(&message.content) + PER_MESSAGE_OVERHEAD_TOKENS)
        .sum();
    let Some(budget_tokens) =
        limit_tokens.checked_sub(prefix_tokens + PER_MESSAGE_OVERHEAD_TOKENS)
    else {
        return FitOutcome::OverBudget {
            needed_tokens,
            limit_tokens,
        };
    };
    if budget_tokens == 0 {
        return FitOutcome::OverBudget {
            needed_tokens,
            limit_tokens,
        };
    }

    let Some(last) = messages.last_mut() else {
        return FitOutcome::OverBudget {
            needed_tokens,
            limit_tokens,
        };
    };

    let marker = "\n\n[content truncated to fit the model's context window]";
    let budget_chars = (budget_tokens * 4) as usize;
    let truncated_chars = budget_chars.saturating_sub(marker.chars().count());
    last.content = last.content.chars().take(truncated_chars).collect();
    last.content.push_str(marker);

    if estimate_messages_tokens(messages) <= limit_tokens {
        FitOutcome::TruncatedMessage
    } else {
        FitOutcome::OverBudget {
            needed_tokens,
            limit_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::streaming::ChatRole;

    fn user(content: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::User,
            content: content.to_string(),
        }
    }

    #[test]
    fn parses_provider_stated_overflow() {
        // The exact shape that prompted this feature.
        let overflow = parse_context_overflow(
            "Input length (139080) exceeds model's maximum context length (131072).",
        )
        .expect("must parse");
        assert_eq!(overflow.input_tokens, Some(139080));
        assert_eq!(overflow.limit_tokens, 131072);
    }

    #[test]
    fn parses_openai_style_overflow() {
        let overflow = parse_context_overflow(
            "This model's maximum context length is 8192 tokens. However, you requested 9000 tokens in the messages.",
        )
        .expect("must parse");
        assert_eq!(overflow.limit_tokens, 8192);
        assert_eq!(overflow.input_tokens, None);
    }

    #[test]
    fn unrelated_error_is_not_an_overflow() {
        assert_eq!(parse_context_overflow("invalid api key"), None);
        assert_eq!(parse_context_overflow("model overloaded, retry later"), None);
    }

    #[test]
    fn estimate_is_roughly_four_chars_per_token() {
        assert_eq!(estimate_tokens(&"a".repeat(400)), 100);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn fits_untouched_when_within_limit() {
        let mut messages = vec![user(&"a".repeat(400))];
        assert_eq!(
            fit_messages_to_limit(&mut messages, 1_000),
            FitOutcome::Fits
        );
    }

    #[test]
    fn truncates_single_oversized_message_on_wire_only() {
        let mut messages = vec![user(&"a".repeat(40_000))];
        let original = messages[0].content.clone();
        match fit_messages_to_limit(&mut messages, 1_000) {
            FitOutcome::TruncatedMessage => {}
            other => panic!("expected TruncatedMessage, got {other:?}"),
        }
        assert!(messages[0].content.len() < original.len());
        assert!(messages[0].content.ends_with("context window]"));
    }

    #[test]
    fn over_budget_is_reported_not_silently_trimmed() {
        let mut messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: "sys".to_string(),
            },
            user(&"a".repeat(20_000)),
            user(&"b".repeat(20_000)),
        ];
        match fit_messages_to_limit(&mut messages, 1_000) {
            FitOutcome::OverBudget {
                needed_tokens,
                limit_tokens,
            } => {
                assert!(needed_tokens > limit_tokens);
            }
            other => panic!("expected OverBudget, got {other:?}"),
        }
        // The oversized FIRST message must not be silently trimmed away.
        assert_eq!(messages[1].content.len(), 20_000);
    }
}
