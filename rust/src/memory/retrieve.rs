/// Memory retrieval and system prompt injection for Phase 21.
///
/// Mirrors the pattern from `rag/context.rs` (`build_system_with_context`)
/// but operates on the `memories` table rather than document chunks.

/// Default number of top-k memories to retrieve for injection.
pub const DEFAULT_MEMORY_TOP_K: usize = 5;

/// A retrieved memory with its cosine similarity score.
#[derive(Debug, Clone)]
pub struct MemoryResult {
    /// The memory content (extracted fact or preference).
    pub content: String,
    /// Cosine similarity score (0.0–1.0, higher = more relevant).
    pub score: f32,
}

/// Build an augmented system prompt by prepending retrieved memories.
///
/// If `memories` is empty, returns `current_system` unchanged (no injection artifacts).
/// Otherwise prepends a `<memories>` XML block with numbered entries before the
/// existing system prompt:
///
/// ```text
/// <memories>
/// [1] first memory content
///
/// [2] second memory content
///
/// </memories>
///
/// {current_system}
/// ```
pub fn build_system_with_memories(current_system: &str, memories: &[MemoryResult]) -> String {
    if memories.is_empty() {
        return current_system.to_owned();
    }

    let mut out = String::from("<memories>\n");
    for (i, mem) in memories.iter().enumerate() {
        out.push_str(&format!("[{}] {}\n\n", i + 1, mem.content));
    }
    out.push_str("</memories>\n\n");
    out.push_str(current_system);
    out
}
