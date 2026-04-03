/// RAG (Retrieval-Augmented Generation) module.
///
/// Phase 8: provides chunking, vector indexing, context injection, and
/// text extraction for the local on-device RAG pipeline.
pub mod chunker;
pub mod context;
pub mod index;

pub use chunker::{chunk_text, ChunkOutput, DEFAULT_MAX_TOKENS, DEFAULT_OVERLAP_TOKENS};
pub use context::{build_system_with_context, ChunkResult, DEFAULT_TOP_K};
pub use index::VectorIndex;

/// Extract plain text from a file's bytes based on its filename extension.
///
/// Phase 8 (LRAG-07):
/// - `.pdf` files: use pdf_extract to extract embedded text
/// - `.txt`, `.md`, and any other format: interpret as UTF-8 text
///
/// Returns an error if PDF extraction fails (corrupted or encrypted PDF).
/// For non-PDF files, invalid UTF-8 bytes are replaced with the replacement
/// character (U+FFFD) via `String::from_utf8_lossy`.
pub fn extract_text_from_file(filename: &str, content_bytes: &[u8]) -> anyhow::Result<String> {
    let lower = filename.to_lowercase();
    if lower.ends_with(".pdf") {
        let text = pdf_extract::extract_text_from_mem(content_bytes)?;
        Ok(text)
    } else {
        // .txt, .md, or any other extension: treat as UTF-8 text.
        Ok(String::from_utf8_lossy(content_bytes).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_plain_text() {
        let result = extract_text_from_file("test.txt", b"hello world").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_extract_markdown() {
        let result = extract_text_from_file("test.md", b"# Title").unwrap();
        assert_eq!(result, "# Title");
    }

    #[test]
    fn test_extract_unknown_extension_as_text() {
        let result = extract_text_from_file("document.docx", b"some content").unwrap();
        assert_eq!(result, "some content");
    }

    #[test]
    fn test_extract_pdf_invalid_returns_error() {
        // An invalid PDF should return an error, not panic
        let result = extract_text_from_file("doc.pdf", b"not a pdf");
        assert!(result.is_err(), "Invalid PDF bytes should return an error");
    }
}
