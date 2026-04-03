/// Context injection into LLM system prompts for RAG.
///
/// Phase 8 (D-05, D-06, LRAG-05): builds the augmented system prompt by
/// prepending retrieved chunk text in a structured XML block before the
/// base system prompt.

/// Default number of top-k chunks to retrieve for context injection.
pub const DEFAULT_TOP_K: usize = 4;

/// A retrieved chunk with its cosine similarity score.
#[derive(Debug, Clone)]
pub struct ChunkResult {
    /// The text content of the chunk.
    pub text: String,
    /// Cosine similarity score (0.0–1.0, higher = more relevant).
    pub score: f32,
}

/// Build an augmented system prompt by prepending retrieved chunk context.
///
/// If `chunks` is empty, returns `base_system` unchanged.
/// Otherwise prepends a `<context>` XML block with numbered chunks before
/// the base system prompt:
///
/// ```text
/// <context>
/// [1] first chunk text
///
/// [2] second chunk text
///
/// </context>
///
/// {base_system}
/// ```
pub fn build_system_with_context(base_system: &str, chunks: &[ChunkResult]) -> String {
    if chunks.is_empty() {
        return base_system.to_owned();
    }

    let mut out = String::from("<context>\n");
    for (i, chunk) in chunks.iter().enumerate() {
        out.push_str(&format!("[{}] {}\n\n", i + 1, chunk.text));
    }
    out.push_str("</context>\n\n");
    out.push_str(base_system);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_chunks_returns_base() {
        let result = build_system_with_context("You are helpful", &[]);
        assert_eq!(result, "You are helpful");
    }

    #[test]
    fn test_with_chunks_starts_with_context_tag() {
        let chunks = vec![ChunkResult {
            text: "doc text".into(),
            score: 0.9,
        }];
        let result = build_system_with_context("You are helpful", &chunks);
        assert!(
            result.starts_with("<context>"),
            "Result should start with <context>"
        );
    }

    #[test]
    fn test_with_chunks_ends_with_base_system() {
        let chunks = vec![ChunkResult {
            text: "doc text".into(),
            score: 0.9,
        }];
        let result = build_system_with_context("You are helpful", &chunks);
        assert!(
            result.ends_with("You are helpful"),
            "Result should end with base system"
        );
    }

    #[test]
    fn test_chunk_numbering() {
        let chunks = vec![
            ChunkResult {
                text: "first".into(),
                score: 0.9,
            },
            ChunkResult {
                text: "second".into(),
                score: 0.8,
            },
        ];
        let result = build_system_with_context("base", &chunks);
        assert!(result.contains("[1] first"));
        assert!(result.contains("[2] second"));
    }

    #[test]
    fn test_context_block_closed_before_base() {
        let chunks = vec![ChunkResult {
            text: "data".into(),
            score: 0.8,
        }];
        let result = build_system_with_context("base prompt", &chunks);
        let context_end = result
            .find("</context>")
            .expect("Should contain </context>");
        let base_start = result
            .find("base prompt")
            .expect("Should contain base prompt");
        assert!(
            context_end < base_start,
            "</context> should appear before base prompt"
        );
    }
}
