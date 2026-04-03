/// Agent tool definitions and dispatch for the ReAct loop.
///
/// Phase 9 (D-01, D-02, AGNT-02): Provides the three v1 tools available to the
/// agent: search_documents, read_document, and finish. Tool dispatch is synchronous
/// and runs on the actor thread -- per RESEARCH.md guidance, tool calls are I/O bound
/// (SQLite + usearch) and complete in microseconds.
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionTool, ChatCompletionTools, FunctionObject,
};
use serde_json::json;

use crate::embedding::EmbeddingProvider;
use crate::persistence::queries;
use crate::rag::VectorIndex;

/// Build the three v1 agent tool schemas as a Vec<ChatCompletionTools>.
///
/// Tools:
/// - `search_documents(query: String, top_k: Option<i64>)` -- semantic search
/// - `read_document(doc_id: String)` -- retrieve full document text
/// - `finish(result: String)` -- signal task completion with final answer
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
    ]
}

/// Dispatch tool calls synchronously on the actor thread.
///
/// Returns a Vec of `(tool_call_id, result_text)` pairs. One pair per call in `calls`.
/// Tool execution is synchronous -- SQLite and usearch queries are sub-millisecond.
///
/// # Tool behaviour
/// - `search_documents`: Embeds query, searches vector index, fetches chunk text from SQLite.
/// - `read_document`: Loads all chunks for the document from SQLite, concatenates text.
/// - `finish`: Extracts result text (signals termination to the caller via FinishTool result).
pub fn dispatch_tools(
    calls: &[ChatCompletionMessageToolCall],
    db_conn: &rusqlite::Connection,
    vector_index: &VectorIndex,
    embedding_provider: &dyn EmbeddingProvider,
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
