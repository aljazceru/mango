//! Redpill (api.redpill.ai) LLM transport.
//!
//! Vanilla OpenAI-compatible chat completions over HTTPS, gated by TEE attestation
//! (see `crate::attestation::redpill`). NO E2EE wrapper in this phase — TEE attestation
//! is the v1 confidentiality root. If E2EE becomes a requirement (Chutes HPKE / Phala
//! per-message ECDSA), it's a follow-up phase.
//!
//! Provider-shape contract:
//! - `format_redpill_attestation_url` builds `{base}/v1/attestation/report?model=<urlenc>&nonce=<hex>`
//!   (no `Authorization` header — endpoint is public per CONTEXT D-02).
//! - `model_list_url` returns `{base}/v1/models`.
//! - `verify_backend_attestation` calls into `attestation::redpill::ensure_verified_redpill_attestation`
//!   for the backend's preferred model and surfaces `AttestationEvent::Verified` (T-34-09).
//! - `create_chat_completion` + the two streaming entry points gate on attestation FIRST,
//!   then POST a vanilla OpenAI-compatible request via `async_openai`.
//! - `check_model_routable` (Task 2) refuses Tinfoil-routed models with `RedpillError::TinfoilUnsupported`
//!   BEFORE any attestation fetch (T-34-08).

#![allow(dead_code)]

use std::time::Duration;

use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionTools, CreateChatCompletionRequestArgs,
    CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use async_openai::Client;
use futures::StreamExt;

use super::backend::BackendConfig;
use super::error::LlmError;
use crate::attestation::error::AttestationError;
use crate::attestation::policy::TdxPolicy;
use crate::attestation::redpill::{
    ensure_verified_redpill_attestation, Freshness, RedpillError, RedpillShape,
};
use crate::attestation::AttestationEvent;

// ── Wire-format constants ────────────────────────────────────────────────────

pub(crate) const ATTESTATION_PATH: &str = "/v1/attestation/report";
pub(crate) const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
pub(crate) const MODELS_PATH: &str = "/v1/models";
const HTTP_TIMEOUT_SECS: u64 = 60;
const MODELS_LIST_TIMEOUT_SECS: u64 = 30;

const DEFAULT_REDPILL_MODEL: &str = "openai/gpt-oss-20b";

// ── Public helpers ───────────────────────────────────────────────────────────

/// Construct an HTTP client with rustls TLS and the given timeout.
pub fn build_http_client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    crate::net::tls::ensure_default_crypto_provider();
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })
}

/// `https://api.redpill.ai/v1/models` (or whatever root the backend was configured with).
/// Trims trailing `/` and a trailing `/v1` segment so we always produce one canonical URL.
pub fn model_list_url(backend: &BackendConfig) -> Result<String, LlmError> {
    let root = backend
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1");
    Ok(format!("{}{}", root, MODELS_PATH))
}

/// Format the public attestation URL (RED-02). The endpoint is unauthenticated;
/// per-request 32-byte client nonce is hex-encoded into the query string.
///
/// Examples:
///   `format_redpill_attestation_url("openai/gpt-oss-20b", "abc123", "https://api.redpill.ai/v1")`
///   → `https://api.redpill.ai/v1/attestation/report?model=openai%2Fgpt-oss-20b&nonce=abc123`
pub fn format_redpill_attestation_url(model: &str, nonce_hex: &str, base_url: &str) -> String {
    let root = base_url.trim_end_matches('/').trim_end_matches("/v1");
    format!(
        "{}{}?model={}&nonce={}",
        root,
        ATTESTATION_PATH,
        urlencoding::encode(model),
        nonce_hex,
    )
}

/// Trigger an attestation handshake (and populate the in-memory cache) for `backend`.
/// Returns `AttestationEvent::Verified` with `tee_type: "IntelTdx"` on success.
pub async fn verify_backend_attestation(
    backend: &BackendConfig,
    tdx_policy: &TdxPolicy,
) -> Result<AttestationEvent, AttestationError> {
    let model = pick_attestation_model(backend);
    let verified = ensure_verified_redpill_attestation(backend, &model, tdx_policy)
        .await
        .map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Redpill attestation: {e:?}"),
        })?;

    // Map shape + freshness + orchestrated_components into the additive
    // AttestationEvent fields so the native UI can render the badge breakdown
    // (RED-09 freshness sub-line, RED-11 three-way Orchestrated breakdown).
    let shape_str: &'static str = match verified.shape {
        RedpillShape::Flat => "Flat",
        RedpillShape::Orchestrated { .. } => "Orchestrated",
        RedpillShape::Chutes => "Chutes",
    };
    let freshness_str: &'static str = match verified.freshness {
        Freshness::PerRequest => "PerRequest",
        Freshness::PerEnclave => "PerEnclave",
    };
    let components: Option<Vec<(String, String)>> =
        verified.orchestrated_components.as_ref().map(|c| {
            vec![
                ("gateway".to_string(), c.gateway_signing_address_hex.clone()),
                ("model".to_string(), c.model_signing_address_hex.clone()),
                (
                    "compose_manager".to_string(),
                    c.compose_manager_actions_hash_hex.clone(),
                ),
            ]
        });

    Ok(AttestationEvent::Verified {
        backend_id: backend.id.clone(),
        tee_type: "IntelTdx".to_string(),
        report_blob: Vec::new(),
        expires_at: verified.expires_at,
        tls_public_key_fp: None,
        vcek_url: None,
        vcek_der: None,
        shape: Some(shape_str.to_string()),
        freshness: Some(freshness_str.to_string()),
        orchestrated_components: components,
    })
}

// ── Chat completions (non-streaming) ─────────────────────────────────────────

/// Vanilla OpenAI-compatible chat completion gated by Redpill TEE attestation.
///
/// Sequence:
/// 1. `check_model_routable` — refuse Tinfoil-routed models early (Task 2).
/// 2. `ensure_verified_redpill_attestation` — fail closed on attestation error (T-34-09).
/// 3. Build an `async_openai::Client` with `OpenAIConfig::with_api_base(base + "/v1")`.
/// 4. POST the chat completion via `client.chat().create(request).await`.
/// 5. Return the response as-is — NO envelope decryption.
pub async fn create_chat_completion(
    backend: BackendConfig,
    model: String,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Option<Vec<ChatCompletionTools>>,
) -> Result<CreateChatCompletionResponse, LlmError> {
    // 1. Refuse Tinfoil-routed models BEFORE attestation fetch (T-34-08).
    if let Err(e) = check_model_routable(&backend, &model).await {
        return Err(map_redpill_error_for_user(e));
    }

    // 2. Gate on attestation (fail closed on T-34-09).
    let policy = TdxPolicy::default();
    ensure_verified_redpill_attestation(&backend, &model, &policy).await?;

    // 3. Build OpenAI client with custom base.
    let client = build_redpill_client(&backend)?;

    // 4. Build request.
    let mut req_builder = CreateChatCompletionRequestArgs::default();
    req_builder.model(&model).messages(messages);
    if let Some(t) = tools {
        req_builder.tools(t);
    }
    let request = req_builder.build().map_err(|e| LlmError::NetworkError {
        reason: format!("Build Redpill chat request: {e}"),
    })?;

    // 5. POST.
    client
        .chat()
        .create(request)
        .await
        .map_err(super::error::map_openai_error)
}

// ── Streaming entry points ───────────────────────────────────────────────────

/// Bridges `Vec<crate::llm::streaming::ChatMessage>` into the OpenAI-typed
/// stream entry — same shape as `venice::run_streaming_chat_completion`.
pub async fn run_streaming_chat_completion(
    backend: BackendConfig,
    model: String,
    messages: Vec<crate::llm::streaming::ChatMessage>,
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: flume::Sender<crate::CoreMsg>,
) {
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
    };

    let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
    for msg in &messages {
        let result: Result<ChatCompletionRequestMessage, String> = match msg.role {
            crate::llm::streaming::ChatRole::System => {
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                    .map_err(|e| e.to_string())
            }
            crate::llm::streaming::ChatRole::User => {
                ChatCompletionRequestUserMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                    .map_err(|e| e.to_string())
            }
            crate::llm::streaming::ChatRole::Assistant => {
                ChatCompletionRequestAssistantMessageArgs::default()
                    .content(msg.content.clone())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                    .map_err(|e| e.to_string())
            }
        };
        match result {
            Ok(message) => openai_messages.push(message),
            Err(error) => {
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    crate::llm::streaming::InternalEvent::StreamError {
                        error: LlmError::NetworkError { reason: error },
                    },
                )));
                return;
            }
        }
    }

    run_streaming_chat_completion_from_api_messages(
        backend,
        model,
        openai_messages,
        None,
        cancel_token,
        core_tx,
    )
    .await;
}

/// Run a streaming Redpill chat completion against pre-built OpenAI-typed messages.
///
/// Sequence per request:
/// 1. `check_model_routable` (Tinfoil-routed gate).
/// 2. `ensure_verified_redpill_attestation`.
/// 3. Build async-openai stream and forward chunks via `InternalEvent::StreamChunk`.
pub async fn run_streaming_chat_completion_from_api_messages(
    backend: BackendConfig,
    model: String,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Option<Vec<ChatCompletionTools>>,
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: flume::Sender<crate::CoreMsg>,
) {
    if let Err(error) =
        run_streaming_inner(&backend, &model, messages, tools, &cancel_token, &core_tx).await
    {
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            crate::llm::streaming::InternalEvent::StreamError { error },
        )));
    }
}

async fn run_streaming_inner(
    backend: &BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Option<Vec<ChatCompletionTools>>,
    cancel_token: &tokio_util::sync::CancellationToken,
    core_tx: &flume::Sender<crate::CoreMsg>,
) -> Result<(), LlmError> {
    // 1. Refuse Tinfoil-routed models BEFORE attestation fetch.
    if let Err(e) = check_model_routable(backend, model).await {
        return Err(map_redpill_error_for_user(e));
    }

    // 2. Gate on attestation.
    let policy = TdxPolicy::default();
    ensure_verified_redpill_attestation(backend, model, &policy).await?;

    // 3. Build streaming request.
    let client = build_redpill_client(backend)?;
    let mut builder = CreateChatCompletionRequestArgs::default();
    builder.model(model).messages(messages).stream(true);
    if let Some(t) = tools {
        builder.tools(t);
    }
    let request = builder.build().map_err(|e| LlmError::NetworkError {
        reason: format!("Build Redpill chat request: {e}"),
    })?;

    let mut stream = client
        .chat()
        .create_stream(request)
        .await
        .map_err(super::error::map_openai_error)?;

    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    crate::llm::streaming::InternalEvent::StreamCancelled,
                )));
                return Ok(());
            }
            chunk_opt = stream.next() => {
                match chunk_opt {
                    Some(Ok(chunk)) => {
                        forward_stream_chunk(&chunk, core_tx);
                    }
                    Some(Err(e)) => {
                        return Err(super::error::map_openai_error(e));
                    }
                    None => {
                        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                            crate::llm::streaming::InternalEvent::StreamDone,
                        )));
                        return Ok(());
                    }
                }
            }
        }
    }
}

fn forward_stream_chunk(
    chunk: &CreateChatCompletionStreamResponse,
    core_tx: &flume::Sender<crate::CoreMsg>,
) {
    if let Some(content) = chunk
        .choices
        .first()
        .and_then(|c| c.delta.content.as_deref())
    {
        if !content.is_empty() {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                crate::llm::streaming::InternalEvent::StreamChunk {
                    token: content.to_string(),
                },
            )));
        }
    }
}

// ── Tinfoil-routed model detection (RED-10 / T-34-08) ────────────────────────

/// Check whether `model` is routable through Redpill. Returns
/// `Err(RedpillError::TinfoilUnsupported)` when the model's `/v1/models` entry
/// has `providers: ["tinfoil"]`. Tinfoil-via-Redpill is broken at the relay
/// (HTTP 502 "Unsupported Tinfoil attestation format: sev-snp-guest/v2") and
/// we have a stronger direct-Tinfoil SEV-SNP path the user should use instead.
pub async fn check_model_routable(
    backend: &BackendConfig,
    model: &str,
) -> Result<(), RedpillError> {
    let url = model_list_url(backend)
        .map_err(|e| RedpillError::Network(format!("model_list_url: {e:?}")))?;

    let client = build_http_client(Duration::from_secs(MODELS_LIST_TIMEOUT_SECS))
        .map_err(|e| RedpillError::Network(format!("build_http_client: {e:?}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| RedpillError::Network(format!("models GET: {e}")))?;

    if !resp.status().is_success() {
        // Don't fail closed on a transient /v1/models error — let the attestation
        // path produce the precise error. Network failures here surface elsewhere.
        return Ok(());
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| RedpillError::Network(format!("models JSON parse: {e}")))?;

    // Models response shape (per spike captures):
    //   { data: [{ id: "...", providers: ["phala"|"tinfoil"|"chutes"|...] }, ...] }
    let Some(entries) = body.get("data").and_then(|d| d.as_array()) else {
        return Ok(());
    };

    for entry in entries {
        if entry.get("id").and_then(|i| i.as_str()) == Some(model) {
            if let Some(providers) = entry.get("providers").and_then(|p| p.as_array()) {
                if providers.iter().any(|p| p.as_str() == Some("tinfoil")) {
                    return Err(RedpillError::TinfoilUnsupported);
                }
            }
            return Ok(());
        }
    }
    // Unknown model — let the attestation fetch surface a more precise error.
    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn pick_attestation_model(backend: &BackendConfig) -> String {
    backend
        .models
        .first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_REDPILL_MODEL.to_string())
}

fn build_redpill_client(backend: &BackendConfig) -> Result<Client<OpenAIConfig>, LlmError> {
    let api_base = backend
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1");
    let api_base = format!("{}/v1", api_base);
    let config = OpenAIConfig::new()
        .with_api_base(api_base)
        .with_api_key(&backend.api_key);
    let http_client = build_http_client(Duration::from_secs(HTTP_TIMEOUT_SECS))?;
    Ok(Client::with_config(config).with_http_client(http_client))
}

/// Map RedpillError variants to a user-visible LlmError. The Tinfoil-route error
/// gets the precise hint pointing the user to the existing direct-Tinfoil provider.
fn map_redpill_error_for_user(e: RedpillError) -> LlmError {
    match e {
        RedpillError::TinfoilUnsupported => LlmError::NetworkError {
            reason:
                "Redpill: this model is routed via Tinfoil; use the direct-Tinfoil provider in Settings → Providers"
                    .to_string(),
        },
        other => LlmError::NetworkError {
            reason: format!("Redpill: {other}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::backend::TeeType;

    fn redpill_backend(base: &str) -> BackendConfig {
        BackendConfig {
            id: "redpill".into(),
            name: "Redpill".into(),
            base_url: base.into(),
            api_key: "test-key".into(),
            models: vec!["openai/gpt-oss-20b".into()],
            tee_type: TeeType::IntelTdx,
            max_concurrent_requests: 5,
            supports_tool_use: true,
        }
    }

    #[test]
    fn model_list_url_trims_trailing_v1_and_slash() {
        let with_v1 = model_list_url(&redpill_backend("https://api.redpill.ai/v1")).unwrap();
        assert_eq!(with_v1, "https://api.redpill.ai/v1/models");
        let with_slash = model_list_url(&redpill_backend("https://api.redpill.ai/v1/")).unwrap();
        assert_eq!(with_slash, "https://api.redpill.ai/v1/models");
        let bare = model_list_url(&redpill_backend("https://api.redpill.ai")).unwrap();
        assert_eq!(bare, "https://api.redpill.ai/v1/models");
    }

    #[test]
    fn tinfoil_user_facing_error_mentions_direct_tinfoil() {
        let err = map_redpill_error_for_user(RedpillError::TinfoilUnsupported);
        let msg = match err {
            LlmError::NetworkError { reason } => reason,
            other => panic!("expected NetworkError, got {other:?}"),
        };
        let lc = msg.to_lowercase();
        // The user-facing message MUST hint at the direct-Tinfoil provider so
        // the user knows where to go (T-34-08 mitigation surfaces in the UI).
        assert!(lc.contains("tinfoil"), "missing 'tinfoil': {msg}");
        assert!(
            lc.contains("direct-tinfoil") || lc.contains("direct"),
            "missing direct-Tinfoil hint: {msg}"
        );
        assert!(
            lc.contains("settings") || lc.contains("provider"),
            "missing UI navigation hint: {msg}"
        );
    }

    #[test]
    fn non_tinfoil_redpill_error_passes_through_with_prefix() {
        let err = map_redpill_error_for_user(RedpillError::UnknownShape);
        let msg = match err {
            LlmError::NetworkError { reason } => reason,
            other => panic!("expected NetworkError, got {other:?}"),
        };
        // Non-Tinfoil errors are forwarded with a 'Redpill:' prefix so the
        // user can tell which provider failed.
        assert!(msg.starts_with("Redpill:"), "missing prefix: {msg}");
    }

    #[test]
    fn format_attestation_url_urlencodes_model_id() {
        let url = format_redpill_attestation_url(
            "openai/gpt-oss-20b",
            "deadbeef",
            "https://api.redpill.ai/v1",
        );
        assert!(url.contains("model=openai%2Fgpt-oss-20b"));
        assert!(url.ends_with("&nonce=deadbeef"));
        assert!(url.contains("/v1/attestation/report?"));
    }

    #[test]
    fn format_attestation_url_does_not_double_v1() {
        // Production BackendConfig stores base_url with a trailing `/v1/` (the
        // OpenAI-compatible root). The fetch path must NOT produce
        // `…/v1/v1/attestation/report`. Regression for the live test failure
        // observed against api.redpill.ai (HTTP 400 "endpoint is not supported").
        for base in [
            "https://api.redpill.ai/v1",
            "https://api.redpill.ai/v1/",
            "https://api.redpill.ai",
        ] {
            let url = format_redpill_attestation_url("m", "n", base);
            assert!(
                !url.contains("/v1/v1/"),
                "double /v1/ in {url} (from base {base})"
            );
            assert!(url.contains("/v1/attestation/report?"));
        }
    }
}
