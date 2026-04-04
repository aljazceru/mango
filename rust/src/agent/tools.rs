/// Agent tool definitions and dispatch for the ReAct loop.
///
/// Phase 9 (D-01, D-02, AGNT-02): Provides the three v1 tools available to the
/// agent: search_documents, read_document, and finish. Tool dispatch is synchronous
/// and runs on the actor thread -- per RESEARCH.md guidance, tool calls are I/O bound
/// (SQLite + usearch) and complete in microseconds.
///
/// Phase 22 (TOOL-01 through TOOL-05): Adds web_search, fetch_url, file, and calculate
/// tools. The dispatch_tools signature is extended with runtime, data_dir, and brave_api_key
/// parameters for the new network/file/math tools.
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;

use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionTool, ChatCompletionTools, FunctionObject,
};
use scraper::{Html, Selector};
use serde_json::json;

use crate::embedding::EmbeddingProvider;
use crate::persistence::queries;
use crate::rag::VectorIndex;

/// Build the seven agent tool schemas as a Vec<ChatCompletionTools>.
///
/// Tools:
/// - `search_documents(query: String, top_k: Option<i64>)` -- semantic search
/// - `read_document(doc_id: String)` -- retrieve full document text
/// - `finish(result: String)` -- signal task completion with final answer
/// - `web_search(query: String, count: Option<u64>)` -- Brave web search
/// - `fetch_url(url: String)` -- fetch and extract text from a URL
/// - `file(operation: String, path: String, content: Option<String>)` -- file I/O
/// - `calculate(expression: String)` -- evaluate math expressions
///
/// Each `ChatCompletionTools::Function` wraps a `ChatCompletionTool { function: ... }`.
pub fn build_agent_tools() -> Vec<ChatCompletionTools> {
    vec![
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "search_documents".to_string(),
                description: Some(
                    "Search the user's document library using semantic similarity. Returns the most relevant document chunks for the given query.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query to find relevant document chunks"
                        },
                        "top_k": {
                            "type": "integer",
                            "description": "Number of top results to return (default: 4)",
                            "minimum": 1,
                            "maximum": 20
                        }
                    },
                    "required": ["query"]
                })),
                strict: None,
            },
        }),
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "read_document".to_string(),
                description: Some(
                    "Read the full text content of a document by its ID. Use this to get complete document content after identifying it via search_documents.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "doc_id": {
                            "type": "string",
                            "description": "The document ID to read (UUID)"
                        }
                    },
                    "required": ["doc_id"]
                })),
                strict: None,
            },
        }),
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "finish".to_string(),
                description: Some(
                    "Signal that you have completed the task and provide your final answer to the user.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "result": {
                            "type": "string",
                            "description": "The final answer or result to return to the user"
                        }
                    },
                    "required": ["result"]
                })),
                strict: None,
            },
        }),
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "web_search".to_string(),
                description: Some(
                    "Search the web using Brave Search. Returns titles, URLs, and descriptions for the top results.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        },
                        "count": {
                            "type": "integer",
                            "description": "Number of results (default 5, max 10)",
                            "minimum": 1,
                            "maximum": 10
                        }
                    },
                    "required": ["query"]
                })),
                strict: None,
            },
        }),
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "fetch_url".to_string(),
                description: Some(
                    "Fetch a URL and return its text content with HTML tags stripped.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "url": {
                            "type": "string",
                            "description": "The URL to fetch"
                        }
                    },
                    "required": ["url"]
                })),
                strict: None,
            },
        }),
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "file".to_string(),
                description: Some(
                    "Read, write, or append to files in the agent sandbox directory.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "description": "File operation to perform",
                            "enum": ["read", "write", "append"]
                        },
                        "path": {
                            "type": "string",
                            "description": "Relative path to the file within the agent sandbox"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write or append (required for write/append operations)"
                        }
                    },
                    "required": ["operation", "path"]
                })),
                strict: None,
            },
        }),
        ChatCompletionTools::Function(ChatCompletionTool {
            function: FunctionObject {
                name: "calculate".to_string(),
                description: Some(
                    "Evaluate a mathematical expression and return the result. Supports arithmetic (+, -, *, /, ^), parentheses, and functions (sqrt, floor, ceil, min, max).".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "The mathematical expression to evaluate"
                        }
                    },
                    "required": ["expression"]
                })),
                strict: None,
            },
        }),
    ]
}

/// Dispatch tool calls synchronously on the actor thread.
///
/// Returns a Vec of `(tool_call_id, result_text)` pairs. One pair per call in `calls`.
/// Tool execution is synchronous -- SQLite and usearch queries are sub-millisecond.
///
/// # Parameters
/// - `calls`: The tool calls from the LLM response
/// - `db_conn`: SQLite connection for document queries
/// - `vector_index`: HNSW vector index for semantic search
/// - `embedding_provider`: Provider for generating query embeddings
/// - `runtime`: Tokio runtime for async network calls (web_search, fetch_url)
/// - `data_dir`: Base directory for file operations sandbox (empty string disables file tool)
/// - `brave_api_key`: Brave Search API key (empty string disables web_search)
///
/// # Tool behaviour
/// - `search_documents`: Embeds query, searches vector index, fetches chunk text from SQLite.
/// - `read_document`: Loads all chunks for the document from SQLite, concatenates text.
/// - `finish`: Extracts result text (signals termination to the caller via FinishTool result).
/// - `web_search`: Queries Brave Search API and returns formatted results.
/// - `fetch_url`: Fetches a URL and returns plain text from body element.
/// - `file`: Reads/writes/appends files in the agent sandbox directory.
/// - `calculate`: Evaluates mathematical expressions via evalexpr.
pub fn dispatch_tools(
    calls: &[ChatCompletionMessageToolCall],
    db_conn: &rusqlite::Connection,
    vector_index: &VectorIndex,
    embedding_provider: &dyn EmbeddingProvider,
    runtime: &tokio::runtime::Runtime,
    data_dir: &str,
    brave_api_key: &str,
) -> Vec<(String, String)> {
    let mut results = Vec::with_capacity(calls.len());

    for call in calls {
        let tool_call_id = call.id.clone();
        let function_name = call.function.name.as_str();
        let args_str = &call.function.arguments;

        let result = match function_name {
            "search_documents" => {
                dispatch_search_documents(args_str, db_conn, vector_index, embedding_provider)
            }
            "read_document" => dispatch_read_document(args_str, db_conn),
            "finish" => dispatch_finish(args_str),
            "web_search" => dispatch_web_search(args_str, runtime, brave_api_key),
            "fetch_url" => dispatch_fetch_url(args_str, runtime),
            "file" => dispatch_file(args_str, data_dir),
            "calculate" => dispatch_calculate(args_str),
            unknown => {
                format!("Error: unknown tool '{}'", unknown)
            }
        };

        results.push((tool_call_id, result));
    }

    results
}

fn dispatch_search_documents(
    args_str: &str,
    db_conn: &rusqlite::Connection,
    vector_index: &VectorIndex,
    embedding_provider: &dyn EmbeddingProvider,
) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse search_documents args: {}", e),
    };

    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q.to_string(),
        None => return "Error: search_documents requires 'query' parameter".to_string(),
    };

    let top_k = args
        .get("top_k")
        .and_then(|v| v.as_i64())
        .map(|n| n.max(1).min(20) as usize)
        .unwrap_or(4);

    // Embed the query using the embedding provider
    let embeddings = embedding_provider.embed(vec![query.clone()]);
    if embeddings.is_empty() {
        return "No results: embedding provider returned empty result".to_string();
    }

    // Search the vector index
    let search_results = match vector_index.search(&embeddings, top_k) {
        Ok(r) => r,
        Err(e) => return format!("Error: vector search failed: {}", e),
    };

    if search_results.is_empty() {
        return "No relevant documents found".to_string();
    }

    // Fetch chunk text from SQLite
    let rowids: Vec<i64> = search_results.iter().map(|(k, _)| *k as i64).collect();
    let chunk_texts = match queries::get_chunk_text_by_rowids(db_conn, &rowids) {
        Ok(texts) => texts,
        Err(e) => return format!("Error: failed to fetch chunk text: {}", e),
    };

    // Build result as JSON array
    let chunk_map: std::collections::HashMap<i64, String> = chunk_texts.into_iter().collect();
    let result_items: Vec<serde_json::Value> = search_results
        .iter()
        .filter_map(|(key, score)| {
            chunk_map.get(&(*key as i64)).map(|text| {
                json!({
                    "chunk_id": key,
                    "text": text,
                    "score": score
                })
            })
        })
        .collect();

    match serde_json::to_string_pretty(&result_items) {
        Ok(json) => json,
        Err(_) => "Error: failed to serialize results".to_string(),
    }
}

fn dispatch_read_document(args_str: &str, db_conn: &rusqlite::Connection) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse read_document args: {}", e),
    };

    let doc_id = match args.get("doc_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return "Error: read_document requires 'doc_id' parameter".to_string(),
    };

    let chunks = match queries::list_chunks_for_document(db_conn, &doc_id) {
        Ok(c) => c,
        Err(e) => return format!("Error: failed to read document '{}': {}", doc_id, e),
    };

    if chunks.is_empty() {
        return format!("Document '{}' not found or has no content", doc_id);
    }

    // Concatenate all chunks in order
    chunks
        .into_iter()
        .map(|c| c.text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn dispatch_finish(args_str: &str) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse finish args: {}", e),
    };

    match args.get("result").and_then(|v| v.as_str()) {
        Some(result) => result.to_string(),
        None => "Error: finish requires 'result' parameter".to_string(),
    }
}

/// Dispatch Brave web search. Returns formatted JSON of results or an error string.
pub(crate) fn dispatch_web_search(
    args_str: &str,
    runtime: &tokio::runtime::Runtime,
    brave_api_key: &str,
) -> String {
    if brave_api_key.is_empty() {
        return "Error: Brave Search API key not configured. Set brave_api_key in settings."
            .to_string();
    }

    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse web_search args: {}", e),
    };

    let query = match args.get("query").and_then(|v| v.as_str()) {
        Some(q) => q.to_string(),
        None => return "Error: web_search requires 'query' parameter".to_string(),
    };

    let count = args
        .get("count")
        .and_then(|v| v.as_u64())
        .unwrap_or(5)
        .min(10);
    let count_str = count.to_string();

    let key = brave_api_key.to_string();
    let result = runtime.block_on(async move {
        let client = reqwest::Client::new();
        client
            .get("https://api.search.brave.com/res/v1/web/search")
            .query(&[("q", query.as_str()), ("count", count_str.as_str())])
            .header("X-Subscription-Token", &key)
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("Error: web search failed: {}", e))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Error: failed to parse search response: {}", e))
    });

    match result {
        Ok(json) => format_brave_results(&json),
        Err(e) => e,
    }
}

/// Format Brave Search API response into a human-readable JSON string.
fn format_brave_results(json: &serde_json::Value) -> String {
    let results = match json
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(|r| r.as_array())
    {
        Some(arr) if !arr.is_empty() => arr,
        _ => return "No results found".to_string(),
    };

    let items: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "title": r.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "url": r.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                "description": r.get("description").and_then(|v| v.as_str()).unwrap_or("")
            })
        })
        .collect();

    match serde_json::to_string_pretty(&items) {
        Ok(s) => s,
        Err(_) => "Error: failed to serialize search results".to_string(),
    }
}

/// Dispatch URL fetching. Returns plain text extracted from the body element.
pub(crate) fn dispatch_fetch_url(
    args_str: &str,
    runtime: &tokio::runtime::Runtime,
) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse fetch_url args: {}", e),
    };

    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.to_string(),
        None => return "Error: fetch_url requires 'url' parameter".to_string(),
    };

    let url_for_error = url.clone();
    let result = runtime.block_on(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Error: failed to fetch '{}': {}", url, e))?
            .text()
            .await
            .map_err(|e| format!("Error: failed to read response from '{}': {}", url, e))
    });

    let html = match result {
        Ok(h) => h,
        Err(e) => return e,
    };

    // Parse HTML and extract text from body
    let document = Html::parse_document(&html);
    let text = if let Ok(body_sel) = Selector::parse("body") {
        if let Some(body) = document.select(&body_sel).next() {
            body.text().collect::<Vec<_>>().join(" ")
        } else {
            // Fallback: extract text from root
            document.root_element().text().collect::<Vec<_>>().join(" ")
        }
    } else {
        document.root_element().text().collect::<Vec<_>>().join(" ")
    };

    // Normalize whitespace
    let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

    let _ = url_for_error; // suppress unused warning
    const MAX_CHARS: usize = 8000;
    if text.len() > MAX_CHARS {
        format!("{}... [truncated at 8000 chars]", &text[..MAX_CHARS])
    } else {
        text
    }
}

/// Resolve a relative path within the agent sandbox directory.
///
/// Returns `Err` if `data_dir` is empty, the path contains `..`, or the
/// resolved path escapes the sandbox.
pub(crate) fn resolve_sandbox_path(
    data_dir: &str,
    relative_path: &str,
) -> Result<std::path::PathBuf, String> {
    if data_dir.is_empty() {
        return Err("Error: file tool unavailable in test/in-memory mode".to_string());
    }

    if relative_path.contains("..") {
        return Err(format!(
            "Error: path '{}' contains '..' which is not allowed",
            relative_path
        ));
    }

    let sandbox = std::path::Path::new(data_dir).join("agent_files");
    if let Err(e) = std::fs::create_dir_all(&sandbox) {
        return Err(format!("Error: failed to create sandbox directory: {}", e));
    }

    let candidate = sandbox.join(relative_path);

    // Canonicalize sandbox to resolve symlinks, then check prefix
    let sandbox_canon = sandbox
        .canonicalize()
        .map_err(|e| format!("Error: sandbox path resolution failed: {}", e))?;
    // Canonicalize the candidate's parent so we can check the prefix without the file existing yet
    let candidate_parent = candidate.parent().unwrap_or(&candidate);
    if let Ok(parent_canon) = candidate_parent.canonicalize() {
        if !parent_canon.starts_with(&sandbox_canon) {
            return Err("Error: path escapes sandbox".to_string());
        }
    }
    // Also do a non-canonicalized prefix check as a belt-and-suspenders guard
    if !candidate.starts_with(&sandbox) {
        return Err("Error: path escapes sandbox".to_string());
    }

    Ok(candidate)
}

/// Dispatch file operations (read, write, append) within the agent sandbox.
pub(crate) fn dispatch_file(args_str: &str, data_dir: &str) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse file args: {}", e),
    };

    let operation = match args.get("operation").and_then(|v| v.as_str()) {
        Some(op) => op.to_string(),
        None => return "Error: file requires 'operation' parameter".to_string(),
    };

    let path = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return "Error: file requires 'path' parameter".to_string(),
    };

    let resolved = match resolve_sandbox_path(data_dir, &path) {
        Ok(p) => p,
        Err(e) => return e,
    };

    match operation.as_str() {
        "read" => match std::fs::read_to_string(&resolved) {
            Ok(content) => content,
            Err(e) => format!("Error: failed to read '{}': {}", path, e),
        },
        "write" => {
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => {
                    return "Error: file 'write' operation requires 'content' parameter".to_string()
                }
            };
            if let Some(parent) = resolved.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return format!("Error: failed to create parent directories: {}", e);
                }
            }
            match std::fs::write(&resolved, &content) {
                Ok(()) => format!("Wrote {} bytes to '{}'", content.len(), path),
                Err(e) => format!("Error: failed to write '{}': {}", path, e),
            }
        }
        "append" => {
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => {
                    return "Error: file 'append' operation requires 'content' parameter"
                        .to_string()
                }
            };
            if let Some(parent) = resolved.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return format!("Error: failed to create parent directories: {}", e);
                }
            }
            let mut file = match OpenOptions::new()
                .append(true)
                .create(true)
                .open(&resolved)
            {
                Ok(f) => f,
                Err(e) => return format!("Error: failed to open '{}' for append: {}", path, e),
            };
            match file.write_all(content.as_bytes()) {
                Ok(()) => format!("Appended {} bytes to '{}'", content.len(), path),
                Err(e) => format!("Error: failed to append to '{}': {}", path, e),
            }
        }
        other => format!(
            "Error: unknown file operation '{}'. Use read, write, or append.",
            other
        ),
    }
}

/// Dispatch mathematical expression evaluation via evalexpr.
pub(crate) fn dispatch_calculate(args_str: &str) -> String {
    let args: serde_json::Value = match serde_json::from_str(args_str) {
        Ok(v) => v,
        Err(e) => return format!("Error: failed to parse calculate args: {}", e),
    };

    let expression = match args.get("expression").and_then(|v| v.as_str()) {
        Some(e) => e.to_string(),
        None => return "Error: calculate requires 'expression' parameter".to_string(),
    };

    if expression.len() > 200 {
        return "Error: expression too long (max 200 chars)".to_string();
    }

    match evalexpr::eval(&expression) {
        Ok(value) => value.to_string(),
        Err(e) => format!("Error: math evaluation failed: {}", e),
    }
}
