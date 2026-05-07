/// RAG (Retrieval-Augmented Generation) module.
///
/// Phase 8: provides chunking, vector indexing, context injection, and
/// text extraction for the local on-device RAG pipeline.
/// Phase 32-09: text extraction extended beyond `.pdf` + UTF-8 to include
/// `.docx`, `.epub`, `.html`/`.htm`, `.rtf` plus a 20 MiB size cap that
/// short-circuits before any parser allocates.
pub mod chunker;
pub mod context;
pub mod directory_sync;
pub mod index;

pub use chunker::{chunk_text, ChunkOutput, DEFAULT_MAX_TOKENS, DEFAULT_OVERLAP_TOKENS};
pub use context::{build_system_with_context, ChunkResult, DEFAULT_TOP_K};
pub use index::VectorIndex;

/// Maximum input size accepted by [`extract_text_from_file`]. Files larger than
/// this return an error before any parser runs (closes Phase 32 VERIFICATION
/// HI-03 for the extract path). 20 MiB is well above a typical Obsidian note
/// and well below the threshold where any single-file allocation becomes
/// problematic on mobile.
pub const MAX_EXTRACT_INPUT_BYTES: usize = 20 * 1024 * 1024;

/// Extract plain text from a file's bytes based on its filename extension.
///
/// Phase 32-09 dispatch (extends Phase 8 baseline):
/// - `.pdf` → `pdf-extract`
/// - `.docx` → `docx-rs`
/// - `.epub` → `epub` crate (per-chapter XHTML, run through `html2text`)
/// - `.html` / `.htm` → `html2text` (script/style stripped, no JS execution)
/// - `.rtf` → `rtf-parser`
/// - anything else (including `.md`, `.txt`, `.org`, unknown) → UTF-8 lossy
///   passthrough (preserves Phase 8 behaviour).
///
/// Files larger than [`MAX_EXTRACT_INPUT_BYTES`] return `Err` before any
/// parser is invoked. Per-format parser errors are wrapped with
/// `anyhow::anyhow!("<fmt> extract failed for {filename}: {e}")`.
pub fn extract_text_from_file(filename: &str, content_bytes: &[u8]) -> anyhow::Result<String> {
    if content_bytes.len() > MAX_EXTRACT_INPUT_BYTES {
        anyhow::bail!(
            "file too large for text extraction: {} bytes (size cap: {} bytes) — file: {}",
            content_bytes.len(),
            MAX_EXTRACT_INPUT_BYTES,
            filename
        );
    }
    let lower = filename.to_lowercase();
    if lower.ends_with(".pdf") {
        pdf_extract::extract_text_from_mem(content_bytes)
            .map_err(|e| anyhow::anyhow!("pdf extract failed for {}: {}", filename, e))
    } else if lower.ends_with(".docx") {
        extract_docx(content_bytes)
            .map_err(|e| anyhow::anyhow!("docx extract failed for {}: {}", filename, e))
    } else if lower.ends_with(".epub") {
        extract_epub(content_bytes)
            .map_err(|e| anyhow::anyhow!("epub extract failed for {}: {}", filename, e))
    } else if lower.ends_with(".html") || lower.ends_with(".htm") {
        extract_html(content_bytes)
            .map_err(|e| anyhow::anyhow!("html extract failed for {}: {}", filename, e))
    } else if lower.ends_with(".rtf") {
        extract_rtf(content_bytes)
            .map_err(|e| anyhow::anyhow!("rtf extract failed for {}: {}", filename, e))
    } else {
        // .md, .txt, .org, unknown — UTF-8 lossy passthrough (Phase 8 behaviour).
        Ok(String::from_utf8_lossy(content_bytes).into_owned())
    }
}

/// Extract plain text from a `.docx` byte buffer using `docx-rs`.
///
/// Strategy: parse the document, serialize to JSON, then walk the JSON tree
/// collecting any `{"type":"text","data":{"text":"..."}}` runs. This handles
/// nested paragraphs, tables, hyperlinks, structured-data tags, and inserts
/// uniformly without recursing through every typed enum variant.
fn extract_docx(bytes: &[u8]) -> anyhow::Result<String> {
    let docx = docx_rs::read_docx(bytes)
        .map_err(|e| anyhow::anyhow!("read_docx: {}", e))?;
    let json: serde_json::Value = serde_json::from_str(&docx.json())
        .map_err(|e| anyhow::anyhow!("docx json parse: {}", e))?;
    let mut out = String::new();
    walk_docx_text(&json, &mut out);
    Ok(out)
}

/// Recursively walk a `docx-rs` JSON value collecting text runs.
///
/// `docx-rs` serializes a `RunChild::Text(Text)` as
/// `{"type":"text","data":{"text":"..."}}` — match that shape and append the
/// text. Walks all Object/Array children otherwise. Adds a single space
/// between collected runs so adjacent paragraphs don't visually merge into
/// `helloworld`.
fn walk_docx_text(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::Object(map) => {
            let is_text =
                map.get("type").and_then(|t| t.as_str()) == Some("text");
            if is_text {
                if let Some(t) = map
                    .get("data")
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(t);
                    return;
                }
            }
            for (_, val) in map.iter() {
                walk_docx_text(val, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                walk_docx_text(v, out);
            }
        }
        _ => {}
    }
}

/// Extract plain text from an `.epub` byte buffer using the `epub` crate.
///
/// Iterates the spine, fetches each chapter's XHTML resource, and pipes it
/// through `html2text` to strip markup. Concatenates with blank-line
/// separators between chapters.
fn extract_epub(bytes: &[u8]) -> anyhow::Result<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut doc = epub::doc::EpubDoc::from_reader(cursor)
        .map_err(|e| anyhow::anyhow!("epub open: {}", e))?;
    let mut out = String::new();
    loop {
        if let Some((xhtml, _mime)) = doc.get_current_str() {
            let text = html2text::from_read(xhtml.as_bytes(), 80)
                .map_err(|e| anyhow::anyhow!("epub chapter html→text: {}", e))?;
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&text);
        }
        if !doc.go_next() {
            break;
        }
    }
    Ok(out)
}

/// Extract plain text from `.html` / `.htm` bytes using `html2text`.
///
/// `html2text` does NOT execute JavaScript — `<script>` content is dropped
/// at the DOM-render stage, closing the T-32-I5 information-disclosure
/// threat for HTML clipping inputs.
fn extract_html(bytes: &[u8]) -> anyhow::Result<String> {
    html2text::from_read(bytes, 80)
        .map_err(|e| anyhow::anyhow!("html→text: {}", e))
}

/// Extract plain text from `.rtf` bytes using `rtf-parser`.
///
/// Pipeline: `Lexer::scan` → `Parser::new(tokens).parse()` → `get_text()`.
/// We deliberately use the `Result`-returning chain rather than the
/// panic-prone `parse_rtf(String)` convenience wrapper.
fn extract_rtf(bytes: &[u8]) -> anyhow::Result<String> {
    let s = std::str::from_utf8(bytes)
        .map_err(|e| anyhow::anyhow!("rtf utf-8: {}", e))?;
    let tokens = rtf_parser::lexer::Lexer::scan(s)
        .map_err(|e| anyhow::anyhow!("rtf lex: {:?}", e))?;
    let doc = rtf_parser::parser::Parser::new(tokens)
        .parse()
        .map_err(|e| anyhow::anyhow!("rtf parse: {:?}", e))?;
    Ok(doc.get_text())
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
        // Phase 32-09: `.docx` now goes through the docx extractor, so use a
        // truly unknown extension here to exercise the UTF-8 lossy fallback.
        let result = extract_text_from_file("document.xyz", b"some content").unwrap();
        assert_eq!(result, "some content");
    }

    #[test]
    fn test_extract_pdf_invalid_returns_error() {
        // An invalid PDF should return an error, not panic
        let result = extract_text_from_file("doc.pdf", b"not a pdf");
        assert!(result.is_err(), "Invalid PDF bytes should return an error");
    }
}
