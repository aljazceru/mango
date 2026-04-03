/// Agent ReAct loop step executor.
///
/// Phase 9 (D-02, D-03, AGNT-01, AGNT-03): Implements the non-streaming LLM call
/// with function calling tools. Each step is a single chat completions request
/// (not streaming) that returns either tool calls or a final text answer.
use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
        ChatCompletionRequestMessage, ChatCompletionTools, CreateChatCompletionRequestArgs,
        FinishReason,
    },
    Client,
};

use crate::llm::error::LlmError;
use crate::llm::{BackendConfig, ProviderTransportKind};

/// Result of a single agent step.
///
/// The caller (actor loop) matches on this to decide whether to:
/// - Continue the loop by dispatching tool results (`ToolCalls`)
/// - Terminate the session with a final answer (`FinalAnswer` or `FinishTool`)
#[derive(Debug)]
pub enum AgentStepResult {
    /// LLM returned tool calls to execute. Contains the unwrapped Function tool call objects.
    ToolCalls(Vec<ChatCompletionMessageToolCall>),
    /// LLM returned a natural text answer (stop finish reason, no tools).
    FinalAnswer(String),
    /// LLM called the `finish` tool -- the String is the result argument.
    FinishTool(String),
}

/// Execution state for an in-progress agent session.
///
/// Stored in `ActorState.active_agent_sessions` while a session is running.
/// Checkpointed to SQLite after each step; rebuilt from SQLite on resume.
#[derive(Clone, Debug)]
pub struct AgentExecutionState {
    /// Agent session ID (matches the `agent_sessions.id` row in SQLite).
    pub session_id: String,
    /// Current conversation history including system prompt, user task, and
    /// all prior assistant + tool messages.
    pub messages: Vec<ChatCompletionRequestMessage>,
    /// Step counter (1-indexed). Incremented before each LLM call.
    pub step_number: i64,
    /// Backend ID used for this agent session.
    pub backend_id: String,
    /// Model ID used for this agent session.
    pub model: String,
}

/// Execute a single non-streaming agent step.
///
/// Calls `client.chat().create()` (NOT `create_stream`) with the provided messages
/// and tools. Returns `AgentStepResult` based on the response finish reason:
///
/// - `finish_reason == ToolCalls` and any tool is `finish` -> `FinishTool(result)`
/// - `finish_reason == ToolCalls` (no finish tool) -> `ToolCalls(calls)`
/// - `finish_reason == Stop` or content present -> `FinalAnswer(content)`
pub async fn run_agent_step(
    client: &Client<OpenAIConfig>,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<ChatCompletionTools>,
) -> Result<AgentStepResult, LlmError> {
    use crate::llm::error::map_openai_error;

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .tools(tools)
        .build()
        .map_err(
            |e: async_openai::error::OpenAIError| LlmError::NetworkError {
                reason: e.to_string(),
            },
        )?;

    let response = client
        .chat()
        .create(request)
        .await
        .map_err(map_openai_error)?;

    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LlmError::NetworkError {
            reason: "No choices in response".to_string(),
        })?;

    match choice.finish_reason {
        Some(FinishReason::ToolCalls) => {
            // Unwrap the Vec<ChatCompletionMessageToolCalls> enum into Vec<ChatCompletionMessageToolCall>
            let raw_calls = choice.message.tool_calls.unwrap_or_default();
            let tool_calls: Vec<ChatCompletionMessageToolCall> = raw_calls
                .into_iter()
                .filter_map(|c| match c {
                    ChatCompletionMessageToolCalls::Function(call) => Some(call),
                    _ => None,
                })
                .collect();

            // Check if any tool call is the `finish` tool
            for call in &tool_calls {
                if call.function.name == "finish" {
                    // Extract the result argument
                    let result = extract_finish_result(&call.function.arguments);
                    return Ok(AgentStepResult::FinishTool(result));
                }
            }

            Ok(AgentStepResult::ToolCalls(tool_calls))
        }
        _ => {
            // Stop or other finish reason -- return text content as final answer
            let content = choice
                .message
                .content
                .unwrap_or_else(|| "(no content)".to_string());
            Ok(AgentStepResult::FinalAnswer(content))
        }
    }
}

pub async fn run_agent_step_for_backend(
    backend: &BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<ChatCompletionTools>,
) -> Result<AgentStepResult, LlmError> {
    match backend.transport_kind() {
        ProviderTransportKind::TinfoilSecure => {
            let response =
                crate::llm::tinfoil_secure::create_chat_completion(backend, model, messages, tools)
                    .await?;
            return agent_step_result_from_response(response);
        }
        ProviderTransportKind::PpqPrivateE2ee => {
            let response =
                crate::llm::ppq_private::create_chat_completion(backend, model, messages, tools)
                    .await?;
            return agent_step_result_from_response(response);
        }
        ProviderTransportKind::OpenAiCompatible => {}
    }

    let (client, _) = backend.transport_kind().build_openai_client(
        backend,
        None,
        std::time::Duration::from_secs(60),
    )?;
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .tools(tools)
        .build()
        .map_err(
            |e: async_openai::error::OpenAIError| LlmError::NetworkError {
                reason: e.to_string(),
            },
        )?;
    let response = client
        .chat()
        .create(request)
        .await
        .map_err(crate::llm::error::map_openai_error)?;
    agent_step_result_from_response(response)
}

fn agent_step_result_from_response(
    response: async_openai::types::chat::CreateChatCompletionResponse,
) -> Result<AgentStepResult, LlmError> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LlmError::NetworkError {
            reason: "No choices in response".to_string(),
        })?;

    match choice.finish_reason {
        Some(FinishReason::ToolCalls) => {
            let raw_calls = choice.message.tool_calls.unwrap_or_default();
            let tool_calls: Vec<ChatCompletionMessageToolCall> = raw_calls
                .into_iter()
                .filter_map(|c| match c {
                    ChatCompletionMessageToolCalls::Function(call) => Some(call),
                    _ => None,
                })
                .collect();

            for call in &tool_calls {
                if call.function.name == "finish" {
                    let result = extract_finish_result(&call.function.arguments);
                    return Ok(AgentStepResult::FinishTool(result));
                }
            }

            Ok(AgentStepResult::ToolCalls(tool_calls))
        }
        _ => {
            let content = choice
                .message
                .content
                .unwrap_or_else(|| "(no content)".to_string());
            Ok(AgentStepResult::FinalAnswer(content))
        }
    }
}

/// Extract the `result` field from a `finish` tool call's arguments JSON.
fn extract_finish_result(args_str: &str) -> String {
    serde_json::from_str::<serde_json::Value>(args_str)
        .ok()
        .and_then(|v| {
            v.get("result")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| args_str.to_string())
}
