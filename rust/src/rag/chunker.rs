/// Fixed-size text chunking with overlap for the RAG pipeline.
///
/// Phase 8 (D-01, D-02): produces overlapping chunks from a document so nearby
/// context is always present at retrieval time. Chunk boundaries are snapped to
/// whitespace to avoid splitting mid-word.

/// Default chunk size in approximate tokens (chars / 4).
pub const DEFAULT_MAX_TOKENS: usize = 512;

/// Default overlap in approximate tokens between consecutive chunks.
pub const DEFAULT_OVERLAP_TOKENS: usize = 51;

/// A single text chunk with its byte offset in the original string.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkOutput {
    /// Byte offset of the first character of this chunk in the original text.
    pub char_offset: usize,
    /// The chunk text.
    pub text: String,
}

/// Split `text` into overlapping fixed-size chunks.
///
/// `max_tokens` and `overlap_tokens` are approximate token counts, using
/// chars / 4 as the token approximation (matches typical English token density).
///
/// Chunk boundaries are snapped to the nearest whitespace before `max_chars` to
/// avoid splitting words. If the entire text fits in one chunk, returns a single
/// ChunkOutput with `char_offset = 0`.
///
/// # Panics
/// Panics if `overlap_tokens >= max_tokens`.
pub fn chunk_text(text: &str, max_tokens: usize, overlap_tokens: usize) -> Vec<ChunkOutput> {
    assert!(
        overlap_tokens < max_tokens,
        "overlap_tokens must be less than max_tokens"
    );

    let max_chars = max_tokens * 4;
    let overlap_chars = overlap_tokens * 4;
    let step_chars = max_chars - overlap_chars;

    let text_len = text.len();

    if text_len == 0 {
        return vec![];
    }

    if text_len <= max_chars {
        return vec![ChunkOutput {
            char_offset: 0,
            text: text.to_owned(),
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text_len {
        let raw_end = (start + max_chars).min(text_len);

        // Snap to nearest whitespace boundary before raw_end (if not at end).
        let end = if raw_end >= text_len {
            text_len
        } else {
            // Walk back from raw_end to find a whitespace boundary.
            // We look in the byte string; since we're working with char boundaries,
            // find the last whitespace at or before raw_end.
            snap_to_whitespace(text, raw_end)
        };

        let chunk_text = &text[start..end];
        chunks.push(ChunkOutput {
            char_offset: start,
            text: chunk_text.to_owned(),
        });

        if end >= text_len {
            break;
        }

        // Next chunk starts at (start + step_chars), snapped to a char boundary.
        let next_start_raw = start + step_chars;
        start = snap_char_boundary(text, next_start_raw.min(text_len - 1));

        // Safety: if we did not advance, force at least one char forward.
        if start <= chunks.last().map(|c| c.char_offset).unwrap_or(0) && start < text_len {
            start = advance_one_char(text, start);
        }
    }

    chunks
}

/// Return the byte index of the last whitespace at or before `pos` in `text`.
/// If no whitespace found, returns `pos` (no snapping possible).
fn snap_to_whitespace(text: &str, pos: usize) -> usize {
    let bytes = text.as_bytes();
    let clamped = pos.min(bytes.len());

    // Walk backwards from clamped to find ASCII whitespace.
    let mut i = clamped;
    while i > 0 {
        i -= 1;
        if bytes[i].is_ascii_whitespace() {
            // Return the position after the whitespace (i+1) to exclude it.
            return i + 1;
        }
    }
    // No whitespace found; return original pos (can't snap).
    clamped
}

/// Ensure `pos` falls on a valid UTF-8 character boundary.
fn snap_char_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos.min(text.len());
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Advance past the current character at `pos`.
fn advance_one_char(text: &str, pos: usize) -> usize {
    let bytes = text.as_bytes();
    let mut p = pos + 1;
    while p < bytes.len() && !text.is_char_boundary(p) {
        p += 1;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_text_single_chunk() {
        let chunks = chunk_text("short", DEFAULT_MAX_TOKENS, DEFAULT_OVERLAP_TOKENS);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].char_offset, 0);
        assert_eq!(chunks[0].text, "short");
    }

    #[test]
    fn test_empty_text() {
        let chunks = chunk_text("", DEFAULT_MAX_TOKENS, DEFAULT_OVERLAP_TOKENS);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_long_text_multiple_chunks() {
        let text = "x ".repeat(1500); // 3000 chars
        let chunks = chunk_text(&text, 512, 51);
        assert!(
            chunks.len() > 1,
            "3000-char text should produce multiple chunks"
        );

        let max_chars = 512 * 4;
        for chunk in &chunks {
            assert!(
                chunk.text.len() <= max_chars,
                "Chunk text length {} should be <= {} chars",
                chunk.text.len(),
                max_chars
            );
        }
    }

    #[test]
    fn test_first_chunk_offset_is_zero() {
        let text = "x ".repeat(1500);
        let chunks = chunk_text(&text, 512, 51);
        assert_eq!(chunks[0].char_offset, 0);
    }

    #[test]
    fn test_second_chunk_offset_less_than_max_chars() {
        let text = "x ".repeat(1500);
        let chunks = chunk_text(&text, 512, 51);
        assert!(chunks.len() >= 2);
        let max_chars = 512 * 4;
        assert!(
            chunks[1].char_offset < max_chars,
            "Second chunk offset {} should be less than max_chars {} (overlap exists)",
            chunks[1].char_offset,
            max_chars
        );
    }

    #[test]
    fn test_overlap_region_exists() {
        let text = "x ".repeat(1500);
        let chunks = chunk_text(&text, 512, 51);
        assert!(chunks.len() >= 2);

        let first_end = chunks[0].char_offset + chunks[0].text.len();
        let second_start = chunks[1].char_offset;
        assert!(
            second_start < first_end,
            "Second chunk (start={}) should begin before first chunk ends (end={})",
            second_start,
            first_end
        );
    }

    #[test]
    fn test_chunks_cover_full_text() {
        let text = "hello world this is a test ".repeat(100);
        let chunks = chunk_text(&text, 512, 51);
        // First chunk starts at 0
        assert_eq!(chunks[0].char_offset, 0);
        // Last chunk ends at or near the end of text
        let last = chunks.last().unwrap();
        assert_eq!(
            last.char_offset + last.text.len(),
            text.len(),
            "Chunks should cover the entire text"
        );
    }
}
