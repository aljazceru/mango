//! Phase 35 — dispatch layer wiring remote tools into the existing
//! agent + chat tool-call infrastructure.
//!
//! Design (LOCKED, see RESEARCH §E):
//! - In-memory `HashMap<String, ContextvmToolDescriptor>` keyed by
//!   `tool_name`, hydrated at conversation start by the actor (Plan 35-05).
//! - `dispatch_tools` consults the map after exhausting local match arms.
//! - Local tools ALWAYS win on collision; remote tools with reserved
//!   names are filtered out at assembly time.
//! - Per-turn cap of 8 remote tools, sorted by `last_seen_at DESC` then
//!   alphabetical by `tool_name`.
//! - Tool descriptions are length-capped at 500 chars to bound the
//!   prompt-injection surface area in auto-discover mode.

use std::collections::HashMap;

use async_openai::types::chat::{ChatCompletionTool, ChatCompletionTools, FunctionObject};

use crate::persistence::queries::ContextvmToolRow;

/// Reserved tool names: the existing local dispatch arms in
/// `agent::tools::dispatch_tools`. Any remote announcement with one of
/// these names is silently filtered at assembly time. Logged via
/// `log::warn!` once per filter event so operators can see when a
/// provider tries to shadow a local tool.
pub const RESERVED_LOCAL_NAMES: &[&str] = &[
    "search_documents",
    "read_document",
    "finish",
    "web_search",
    "fetch_url",
    "file",
    "calculate",
];

pub const MAX_REMOTE_TOOLS_PER_TURN: usize = 8;
pub const DESCRIPTION_CAP_CHARS: usize = 500;

/// In-memory descriptor used by the dispatch path. A pure projection of
/// `ContextvmToolRow` with parsed `schema_json` (kept as
/// `serde_json::Value` so we don't reparse per tool-call build).
#[derive(Debug, Clone)]
pub struct ContextvmToolDescriptor {
    pub tool_name: String,
    /// Already capped to `DESCRIPTION_CAP_CHARS` (with ellipsis if truncated).
    pub description: String,
    /// Parsed JSON Schema for `parameters`.
    pub schema: serde_json::Value,
    pub provider_pubkey_hex: String,
    pub provider_display_name: Option<String>,
    pub last_seen_at: i64,
}

impl ContextvmToolDescriptor {
    pub fn from_row(row: &ContextvmToolRow) -> Result<Self, String> {
        let schema: serde_json::Value = serde_json::from_str(&row.schema_json)
            .map_err(|e| format!("bad schema JSON for {}: {}", row.tool_name, e))?;
        Ok(Self {
            tool_name: row.tool_name.clone(),
            description: cap_description(&row.description),
            schema,
            provider_pubkey_hex: row.provider_pubkey.clone(),
            provider_display_name: row.provider_display_name.clone(),
            last_seen_at: row.last_seen_at,
        })
    }
}

fn cap_description(s: &str) -> String {
    if s.chars().count() <= DESCRIPTION_CAP_CHARS {
        return s.to_string();
    }
    let truncated: String = s.chars().take(DESCRIPTION_CAP_CHARS).collect();
    format!("{}…", truncated)
}

/// Filter, sort, and cap a candidate descriptor set per the locked rules.
/// Pure function — easy to unit-test.
pub fn finalise_for_turn(
    descriptors: Vec<ContextvmToolDescriptor>,
) -> Vec<ContextvmToolDescriptor> {
    let mut filtered: Vec<_> = descriptors
        .into_iter()
        .filter(|d| {
            if RESERVED_LOCAL_NAMES.contains(&d.tool_name.as_str()) {
                log::warn!(
                    "filtering remote tool '{}' from provider {} — \
                     conflicts with local tool name",
                    d.tool_name,
                    d.provider_pubkey_hex,
                );
                return false;
            }
            true
        })
        .collect();
    // Sort: last_seen_at DESC, then tool_name ASC.
    filtered.sort_by(|a, b| {
        b.last_seen_at
            .cmp(&a.last_seen_at)
            .then_with(|| a.tool_name.cmp(&b.tool_name))
    });
    filtered.truncate(MAX_REMOTE_TOOLS_PER_TURN);
    filtered
}

/// Build a `HashMap<tool_name, descriptor>` for O(1) dispatch lookup.
pub fn build_dispatch_map(
    descriptors: &[ContextvmToolDescriptor],
) -> HashMap<String, ContextvmToolDescriptor> {
    descriptors
        .iter()
        .map(|d| (d.tool_name.clone(), d.clone()))
        .collect()
}

/// Render descriptors as `ChatCompletionTools::Function` entries for
/// appending to the OpenAI-compatible `tools` array. Returns the
/// project's existing wrapper enum so this slots straight into the
/// existing `Vec<ChatCompletionTools>` returned by `build_chat_tools`.
pub fn descriptors_to_chat_tools(
    descriptors: &[ContextvmToolDescriptor],
) -> Vec<ChatCompletionTools> {
    descriptors
        .iter()
        .map(|d| {
            ChatCompletionTools::Function(ChatCompletionTool {
                function: FunctionObject {
                    name: d.tool_name.clone(),
                    description: Some(d.description.clone()),
                    parameters: Some(d.schema.clone()),
                    strict: None,
                },
            })
        })
        .collect()
}

/// Hydrate the in-memory descriptor list from the database. Reads all
/// currently-enabled `contextvm_tools` rows, parses them into
/// descriptors, and runs them through `finalise_for_turn` so the actor
/// gets a ready-to-use, filtered, capped, sorted slice.
///
/// Rows whose `schema_json` fails to parse are silently dropped (logged
/// via `log::warn!`). Plan 35-05 calls this once at conversation
/// start.
#[allow(dead_code)]
pub fn hydrate_from_db(conn: &rusqlite::Connection) -> Vec<ContextvmToolDescriptor> {
    let rows = match crate::persistence::queries::list_enabled_contextvm_tools(conn) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("hydrate_from_db: list_enabled failed: {}", e);
            return Vec::new();
        }
    };
    let descriptors: Vec<_> = rows
        .iter()
        .filter_map(|r| match ContextvmToolDescriptor::from_row(r) {
            Ok(d) => Some(d),
            Err(e) => {
                log::warn!("hydrate_from_db: skipping row '{}': {}", r.tool_name, e);
                None
            }
        })
        .collect();
    finalise_for_turn(descriptors)
}
