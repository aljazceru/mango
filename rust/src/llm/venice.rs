//! Venice.ai provider transport (Phase 33, Plan 03).
//!
//! Wraps the OpenAI-compatible `/api/v1/chat/completions` endpoint with the
//! Venice E2EE protocol:
//!
//! 1. Per-request secp256k1 ephemeral key pair (`k256::ecdh::EphemeralSecret`).
//! 2. ECDH against the attested signing pubkey returned by Plan 02
//!    (`crate::attestation::venice::ensure_verified_venice_attestation`).
//! 3. HKDF-SHA256 expansion to a 32-byte AES key with `info=b"ecdsa_encryption"`.
//! 4. Per-message AES-256-GCM seal/open with a fresh `OsRng`-derived 12-byte
//!    nonce (Pitfall 7 — never counter-derived).
//! 5. Hex envelope format: `[eph_pub 65B][nonce 12B][ct+tag]`.
//! 6. Text-SSE response framing (`data: …\n\n`) — NOT the binary length-prefixed
//!    framing used by `ppq_private` (Pitfall 8).
//!
//! All cryptographic primitives live in this module; the attestation layer
//! (Plan 02) supplies the verified peer pubkey and the cache-eviction hook used
//! on stale-key 422 retries.

#![allow(dead_code)]

use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionTools, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, CreateChatCompletionResponse,
    CreateChatCompletionStreamResponse,
};
use futures::StreamExt;
use hkdf::Hkdf;
use k256::ecdh::EphemeralSecret;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::PublicKey;
use rand::RngCore;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde_json::Value;
use sha2::Sha256;

use super::backend::BackendConfig;
use super::error::LlmError;
use crate::attestation::policy::TdxPolicy;
use crate::attestation::venice::{
    ensure_verified_venice_attestation, invalidate_cached_venice_attestation,
    VerifiedVeniceAttestation,
};
use crate::attestation::{AttestationError, AttestationEvent};

// ── Wire-format constants ────────────────────────────────────────────────────

const ATTESTATION_PATH: &str = "/api/v1/tee/attestation";
const CHAT_COMPLETIONS_PATH: &str = "/api/v1/chat/completions";

const X_VENICE_TEE_CLIENT_PUB_KEY: &str = "x-venice-tee-client-pub-key";
const X_VENICE_TEE_MODEL_PUB_KEY: &str = "x-venice-tee-model-pub-key";
const X_VENICE_TEE_SIGNING_ALGO: &str = "x-venice-tee-signing-algo";

const HKDF_INFO: &[u8] = b"ecdsa_encryption";
const AES_KEY_LEN: usize = 32;
const AES_NONCE_LEN: usize = 12;
const EPH_PUB_LEN: usize = 65; // uncompressed secp256k1 with 04 prefix
const AES_GCM_TAG_LEN: usize = 16;

/// Default E2EE model. `e2ee-venice-uncensored-24b-p` was removed upstream
/// (2026-08); `e2ee-gpt-oss-20b-p` verified live against
/// api.venice.ai/api/v1/tee/attestation.
const DEFAULT_VENICE_MODEL: &str = "e2ee-gpt-oss-20b-p";
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 90;

// ── Public helpers ───────────────────────────────────────────────────────────

/// `https://api.venice.ai/api/v1/models` (or whatever root the backend was
/// configured with). Mirrors the shape of `ppq_private::model_list_url`.
pub fn model_list_url(backend: &BackendConfig) -> Result<String, LlmError> {
    let root = backend
        .base_url
        .trim_end_matches('/')
        .trim_end_matches("/api/v1");
    Ok(format!("{}/api/v1/models", root))
}

/// Construct an HTTP client with the same TLS profile used by the rest of the
/// transport stack. Verbatim from `ppq_private::build_http_client`.
pub fn build_http_client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    crate::net::tls::ensure_default_crypto_provider();
    reqwest::Client::builder()
        .no_hickory_dns()
        .timeout(timeout)
        .build()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })
}

/// Format the public attestation URL (VEN-02). The endpoint is unauthenticated;
/// per-request 32-byte nonce is hex-encoded into the query string.
pub fn format_attestation_url(model: &str, nonce_hex: &str, base_url: &str) -> String {
    let root = base_url.trim_end_matches('/').trim_end_matches("/api/v1");
    format!(
        "{}{}?model={}&nonce={}",
        root,
        ATTESTATION_PATH,
        urlencoding::encode(model),
        nonce_hex,
    )
}

/// Trigger an attestation handshake (and populate the in-memory cache) for
/// `backend`. Returns `AttestationEvent::Verified` with `tee_type: "IntelTdx"`.
pub async fn verify_backend_attestation(
    backend: &BackendConfig,
    tdx_policy: &TdxPolicy,
) -> Result<AttestationEvent, AttestationError> {
    let model = pick_attestation_model(backend);
    let verified = ensure_verified_venice_attestation(backend, &model, tdx_policy)
        .await
        .map_err(|e| AttestationError::QuoteVerification {
            reason: format!("Venice attestation: {e:?}"),
        })?;
    Ok(AttestationEvent::Verified {
        backend_id: backend.id.clone(),
        tee_type: "IntelTdx".to_string(),
        report_blob: verified.report_blob.clone(),
        expires_at: verified.expires_at,
        tls_public_key_fp: None,
        vcek_url: None,
        vcek_der: None,
        shape: None,
        freshness: None,
        orchestrated_components: None,
    })
}

// ── Crypto primitives ────────────────────────────────────────────────────────

/// ECDH(secp256k1) + HKDF-SHA256 → 32-byte AES key.
///
/// `attested_pub_bytes_65` is the uncompressed peer pubkey (`0x04 || X || Y`)
/// extracted from a `VerifiedVeniceAttestation`. The HKDF info string is the
/// fixed protocol value `b"ecdsa_encryption"` — DO NOT change this without
/// coordinating with the server.
pub fn derive_session_key(
    eph_secret: &EphemeralSecret,
    attested_pub_bytes_65: &[u8; 65],
) -> Result<[u8; 32], LlmError> {
    let attested_pub =
        PublicKey::from_sec1_bytes(attested_pub_bytes_65).map_err(|_| LlmError::NetworkError {
            reason: "invalid Venice signing pubkey for ECDH".into(),
        })?;
    let shared = eph_secret.diffie_hellman(&attested_pub);
    let mut aes_key = [0u8; AES_KEY_LEN];
    Hkdf::<Sha256>::new(None, shared.raw_secret_bytes())
        .expand(HKDF_INFO, &mut aes_key)
        .map_err(|_| LlmError::NetworkError {
            reason: "HKDF expand failed".into(),
        })?;
    Ok(aes_key)
}

/// Seal a plaintext message into a Venice envelope.
///
/// **Wire format:** `[eph_pub 65B || nonce 12B || ct+tag]`, hex-encoded.
///
/// **Pitfall 7:** A fresh 12-byte nonce is generated from `rand::thread_rng()`
/// (which delegates to `OsRng`) on EVERY call. Same key + same plaintext must
/// produce a different envelope each time — the `envelope_round_trip` unit test
/// asserts this directly.
pub fn seal_message(
    plaintext: &[u8],
    aes_key: &[u8; 32],
    eph_pub_uncompressed: &[u8; 65],
) -> Result<String, LlmError> {
    let cipher = Aes256Gcm::new_from_slice(aes_key).map_err(|e| LlmError::NetworkError {
        reason: format!("AES key: {e}"),
    })?;
    let mut nonce_12 = [0u8; AES_NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_12);
    let ct_tag = cipher
        .encrypt(Nonce::from_slice(&nonce_12), plaintext)
        .map_err(|e| LlmError::NetworkError {
            reason: format!("AES-GCM seal: {e}"),
        })?;
    let mut envelope = Vec::with_capacity(EPH_PUB_LEN + AES_NONCE_LEN + ct_tag.len());
    envelope.extend_from_slice(eph_pub_uncompressed);
    envelope.extend_from_slice(&nonce_12);
    envelope.extend_from_slice(&ct_tag);
    Ok(hex::encode(&envelope))
}

/// Open a Venice envelope produced by either side.
///
/// The 12-byte nonce is read from the envelope (offsets `[65..77]`) — the
/// caller must NOT supply or derive a counter-based nonce (Pitfall 8).
/// Returns `Err(LlmError::NetworkError { reason: "Venice E2EE decrypt failed" })`
/// on AEAD tag failure (T-33-13: fail-closed).
pub fn open_envelope(envelope_hex: &str, aes_key: &[u8; 32]) -> Result<Vec<u8>, LlmError> {
    let bytes = hex::decode(envelope_hex).map_err(|e| LlmError::NetworkError {
        reason: format!("envelope hex: {e}"),
    })?;
    if bytes.len() < EPH_PUB_LEN + AES_NONCE_LEN + AES_GCM_TAG_LEN {
        return Err(LlmError::NetworkError {
            reason: format!("envelope too short: {} bytes", bytes.len()),
        });
    }
    let nonce_offset = EPH_PUB_LEN;
    let ct_offset = nonce_offset + AES_NONCE_LEN;
    let nonce_12 = &bytes[nonce_offset..ct_offset];
    let ct_tag = &bytes[ct_offset..];
    let cipher = Aes256Gcm::new_from_slice(aes_key).map_err(|e| LlmError::NetworkError {
        reason: format!("AES key: {e}"),
    })?;
    cipher
        .decrypt(Nonce::from_slice(nonce_12), ct_tag)
        .map_err(|_| LlmError::NetworkError {
            reason: "Venice E2EE decrypt failed".into(),
        })
}

// ── Request body builder ─────────────────────────────────────────────────────

/// Serialize a chat completion request, encrypt every user/system/tool message
/// `content` field with the session AES key, and set `enable_e2ee: true` +
/// `stream: true` at the top level. Assistant messages stay plaintext on
/// outbound (the model emits them encrypted on inbound; we don't re-encrypt).
///
/// Multipart `Array` content (vision parts) is rejected — D9 deferred to a
/// later phase. All other top-level fields (`model`, `temperature`, `tools`,
/// `max_tokens`, …) are preserved unchanged.
fn build_venice_chat_body(
    request: &CreateChatCompletionRequest,
    aes_key: &[u8; 32],
    eph_pub_uncompressed: &[u8; 65],
) -> Result<Vec<u8>, LlmError> {
    let mut value = serde_json::to_value(request).map_err(|e| LlmError::NetworkError {
        reason: format!("serialize request: {e}"),
    })?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| LlmError::NetworkError {
            reason: "Invalid chat completion request shape".into(),
        })?;

    if let Some(Value::Array(messages)) = object.get_mut("messages") {
        for msg in messages.iter_mut() {
            let Some(m) = msg.as_object_mut() else {
                continue;
            };
            let role = m
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            if !matches!(role.as_str(), "user" | "system" | "tool") {
                continue;
            }
            match m.get("content").cloned() {
                Some(Value::String(text)) => {
                    let env = seal_message(text.as_bytes(), aes_key, eph_pub_uncompressed)?;
                    m.insert("content".into(), Value::String(env));
                }
                Some(Value::Array(_)) => {
                    return Err(LlmError::NetworkError {
                        reason: "Venice multipart content not supported in v1 (D9 deferred)".into(),
                    });
                }
                _ => {}
            }
        }
    }

    object.insert("enable_e2ee".into(), Value::Bool(true));
    object.insert("stream".into(), Value::Bool(true));

    serde_json::to_vec(&value).map_err(|e| LlmError::NetworkError {
        reason: format!("serialize body: {e}"),
    })
}

/// Test-only re-export. The body builder is module-private otherwise — only
/// `request_body_shape` in `tests/venice.rs` reaches in.
#[doc(hidden)]
pub fn build_venice_chat_body_for_test(
    request: &CreateChatCompletionRequest,
    aes_key: &[u8; 32],
    eph_pub_uncompressed: &[u8; 65],
) -> Result<Vec<u8>, LlmError> {
    build_venice_chat_body(request, aes_key, eph_pub_uncompressed)
}

// ── Header construction + send ───────────────────────────────────────────────

fn build_venice_headers(
    api_key: &str,
    eph_pub_uncompressed: &[u8; 65],
    signing_pubkey_uncompressed: &[u8; 65],
) -> Result<HeaderMap, LlmError> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {}", api_key)).map_err(|e| LlmError::AuthError {
            reason: e.to_string(),
        })?,
    );
    headers.insert(
        HeaderName::from_static(X_VENICE_TEE_CLIENT_PUB_KEY),
        HeaderValue::from_str(&hex::encode(eph_pub_uncompressed)).map_err(|e| {
            LlmError::NetworkError {
                reason: e.to_string(),
            }
        })?,
    );
    headers.insert(
        HeaderName::from_static(X_VENICE_TEE_MODEL_PUB_KEY),
        HeaderValue::from_str(&hex::encode(signing_pubkey_uncompressed)).map_err(|e| {
            LlmError::NetworkError {
                reason: e.to_string(),
            }
        })?,
    );
    headers.insert(
        HeaderName::from_static(X_VENICE_TEE_SIGNING_ALGO),
        HeaderValue::from_static("ecdsa"),
    );
    Ok(headers)
}

async fn send_venice_request(
    client: &reqwest::Client,
    backend: &BackendConfig,
    verified: &VerifiedVeniceAttestation,
    eph_pub_uncompressed: &[u8; 65],
    body_bytes: Vec<u8>,
) -> Result<reqwest::Response, LlmError> {
    let endpoint = format!("{}{}", verified.request_base_url, "/chat/completions");
    let headers = build_venice_headers(
        &backend.api_key,
        eph_pub_uncompressed,
        &verified.signing_pubkey_uncompressed,
    )?;

    log::debug!(
        "[venice] POST {} bytes={} model={}",
        endpoint,
        body_bytes.len(),
        verified.model
    );

    client
        .post(&endpoint)
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .map_err(|e| LlmError::NetworkError {
            reason: e.to_string(),
        })
}

// ── SSE plumbing (text-SSE — Pitfall 8) ──────────────────────────────────────

/// Pop the next complete SSE event from the running buffer. Mirrors
/// `tinfoil_secure::take_sse_event` — text-SSE only, NOT a binary frame parser.
fn take_sse_event(buffer: &mut String) -> Option<String> {
    let separators = ["\n\n", "\r\n\r\n"];
    for separator in separators {
        if let Some(index) = buffer.find(separator) {
            let event = buffer[..index].to_string();
            buffer.drain(..index + separator.len());
            return Some(event);
        }
    }
    None
}

/// Parse one `data: …` SSE event, decrypt the inner `delta.content` envelope,
/// and forward the recovered plaintext as a `StreamChunk`.
///
/// Returns `Ok(false)` on `data: [DONE]` (terminator), `Ok(true)` otherwise.
fn handle_venice_sse_event(
    raw_event: &str,
    aes_key: &[u8; 32],
    core_tx: &flume::Sender<crate::CoreMsg>,
) -> Result<bool, LlmError> {
    let mut data_lines = Vec::new();
    for line in raw_event.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start());
        }
    }
    if data_lines.is_empty() {
        return Ok(true);
    }
    let payload = data_lines.join("\n");
    if payload == "[DONE]" {
        return Ok(false);
    }

    let chunk: CreateChatCompletionStreamResponse =
        serde_json::from_str(&payload).map_err(|e| LlmError::NetworkError {
            reason: format!("Invalid Venice SSE chunk: {e}"),
        })?;

    if let Some(envelope_hex) = chunk
        .choices
        .first()
        .and_then(|c| c.delta.content.as_deref())
    {
        let plaintext_bytes = open_envelope(envelope_hex, aes_key)?;
        let plaintext_str =
            String::from_utf8(plaintext_bytes).map_err(|e| LlmError::NetworkError {
                reason: format!("Venice plaintext not UTF-8: {e}"),
            })?;
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            crate::llm::streaming::InternalEvent::StreamChunk {
                token: plaintext_str,
            },
        )));
    }

    Ok(true)
}

async fn stream_decrypted_venice_sse(
    response: reqwest::Response,
    aes_key: [u8; 32],
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: &flume::Sender<crate::CoreMsg>,
) -> Result<(), LlmError> {
    let mut body_stream = response.bytes_stream();
    let mut buffer = String::new();

    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    crate::llm::streaming::InternalEvent::StreamCancelled,
                )));
                return Ok(());
            }
            chunk_opt = body_stream.next() => {
                match chunk_opt {
                    Some(Ok(chunk)) => {
                        let s = std::str::from_utf8(&chunk).map_err(|e| LlmError::NetworkError {
                            reason: format!("Venice SSE bytes not UTF-8: {e}"),
                        })?;
                        buffer.push_str(s);
                        while let Some(event) = take_sse_event(&mut buffer) {
                            match handle_venice_sse_event(&event, &aes_key, core_tx)? {
                                true => continue,
                                false => {
                                    let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                                        crate::llm::streaming::InternalEvent::StreamDone,
                                    )));
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        return Err(LlmError::NetworkError { reason: e.to_string() });
                    }
                    None => {
                        // Stream ended without [DONE]; treat as natural termination.
                        if !buffer.trim().is_empty() {
                            // Final partial event — try to flush.
                            let trailing = std::mem::take(&mut buffer);
                            let _ = handle_venice_sse_event(&trailing, &aes_key, core_tx);
                        }
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

// ── Public streaming entry points ────────────────────────────────────────────

/// Bridges `Vec<crate::llm::streaming::ChatMessage>` into the OpenAI-typed
/// stream entry — same shape as `ppq_private::run_streaming_chat_completion`.
pub async fn run_streaming_chat_completion(
    backend: BackendConfig,
    model: String,
    messages: Vec<crate::llm::streaming::ChatMessage>,
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: flume::Sender<crate::CoreMsg>,
) {
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
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

/// Run a streaming Venice chat completion against pre-built OpenAI-typed
/// messages. Mirrors `ppq_private::run_streaming_chat_completion_from_api_messages`.
///
/// Sequence per request:
/// 1. `ensure_verified_venice_attestation` (cache hit OR fresh handshake)
/// 2. Generate ephemeral secp256k1 secret + derive AES session key
/// 3. Build chat body (encrypts user/system/tool message contents)
/// 4. POST with Venice headers; on 422 invalidate cache and retry once
/// 5. Stream + decrypt SSE chunks via `stream_decrypted_venice_sse`
pub async fn run_streaming_chat_completion_from_api_messages(
    backend: BackendConfig,
    model: String,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Option<Vec<ChatCompletionTools>>,
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: flume::Sender<crate::CoreMsg>,
) {
    if let Err(error) = run_streaming_inner(
        &backend,
        &model,
        messages,
        tools,
        true,
        &cancel_token,
        &core_tx,
    )
    .await
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
    allow_retry: bool,
    cancel_token: &tokio_util::sync::CancellationToken,
    core_tx: &flume::Sender<crate::CoreMsg>,
) -> Result<(), LlmError> {
    let tdx_policy = TdxPolicy::default();
    let verified = ensure_verified_venice_attestation(backend, model, &tdx_policy).await?;

    let eph_secret = EphemeralSecret::random(&mut rand::thread_rng());
    let eph_pub_point = eph_secret.public_key().to_encoded_point(false);
    let mut eph_pub_65 = [0u8; EPH_PUB_LEN];
    if eph_pub_point.as_bytes().len() != EPH_PUB_LEN {
        return Err(LlmError::NetworkError {
            reason: "Ephemeral pubkey not 65 bytes uncompressed".into(),
        });
    }
    eph_pub_65.copy_from_slice(eph_pub_point.as_bytes());

    let aes_key = derive_session_key(&eph_secret, &verified.signing_pubkey_uncompressed)?;

    let mut builder = CreateChatCompletionRequestArgs::default();
    builder.model(model).messages(messages.clone()).stream(true);
    if let Some(tools) = tools.clone() {
        builder.tools(tools);
    }
    let request = builder.build().map_err(|e| LlmError::NetworkError {
        reason: format!("Build chat request: {e}"),
    })?;

    let body_bytes = build_venice_chat_body(&request, &aes_key, &eph_pub_65)?;

    let client = build_http_client(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))?;
    let response =
        send_venice_request(&client, backend, &verified, &eph_pub_65, body_bytes).await?;

    let status = response.status();
    if !status.is_success() {
        if status.as_u16() == 422 && allow_retry {
            let body_text = response.text().await.unwrap_or_default();
            let lower = body_text.to_lowercase();
            if lower.contains("stale") || lower.contains("attestation") || lower.contains("key") {
                log::warn!(
                    "[venice] 422 indicates stale attestation; invalidating cache and retrying once"
                );
                invalidate_cached_venice_attestation(backend, model);
                return Box::pin(run_streaming_inner(
                    backend,
                    model,
                    messages,
                    tools,
                    false,
                    cancel_token,
                    core_tx,
                ))
                .await;
            }
            return Err(LlmError::ApiError {
                status_code: 422,
                reason: body_text,
            });
        }
        let body_text = response.text().await.unwrap_or_default();
        return Err(LlmError::ApiError {
            status_code: status.as_u16(),
            reason: body_text,
        });
    }

    stream_decrypted_venice_sse(response, aes_key, cancel_token.clone(), core_tx).await
}

/// Non-streaming chat completion. Used by tool-calling rounds and any caller
/// that needs a single response synchronously. Tool calling itself is D7-
/// deferred — but the entry point exists for parity with the other providers.
pub async fn create_chat_completion(
    backend: BackendConfig,
    model: String,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Option<Vec<ChatCompletionTools>>,
) -> Result<CreateChatCompletionResponse, LlmError> {
    create_chat_completion_inner(&backend, &model, messages, tools, true).await
}

async fn create_chat_completion_inner(
    backend: &BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Option<Vec<ChatCompletionTools>>,
    allow_retry: bool,
) -> Result<CreateChatCompletionResponse, LlmError> {
    let tdx_policy = TdxPolicy::default();
    let verified = ensure_verified_venice_attestation(backend, model, &tdx_policy).await?;

    let eph_secret = EphemeralSecret::random(&mut rand::thread_rng());
    let eph_pub_point = eph_secret.public_key().to_encoded_point(false);
    let mut eph_pub_65 = [0u8; EPH_PUB_LEN];
    eph_pub_65.copy_from_slice(eph_pub_point.as_bytes());

    let aes_key = derive_session_key(&eph_secret, &verified.signing_pubkey_uncompressed)?;

    let mut builder = CreateChatCompletionRequestArgs::default();
    builder.model(model).messages(messages.clone());
    if let Some(t) = tools.clone() {
        builder.tools(t);
    }
    let mut request = builder.build().map_err(|e| LlmError::NetworkError {
        reason: format!("Build chat request: {e}"),
    })?;
    // create_chat_completion is the non-streaming variant; build_venice_chat_body
    // unconditionally sets stream:true, so we re-flip it after serialization.
    request.stream = Some(false);

    let mut body_bytes = build_venice_chat_body(&request, &aes_key, &eph_pub_65)?;
    // Re-stamp stream:false on the serialized body — body builder forces true.
    if let Ok(mut v) = serde_json::from_slice::<Value>(&body_bytes) {
        if let Some(obj) = v.as_object_mut() {
            obj.insert("stream".into(), Value::Bool(false));
        }
        body_bytes = serde_json::to_vec(&v).map_err(|e| LlmError::NetworkError {
            reason: format!("re-serialize body: {e}"),
        })?;
    }

    let client = build_http_client(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS))?;
    let response =
        send_venice_request(&client, backend, &verified, &eph_pub_65, body_bytes).await?;

    let status = response.status();
    if !status.is_success() {
        if status.as_u16() == 422 && allow_retry {
            let body_text = response.text().await.unwrap_or_default();
            let lower = body_text.to_lowercase();
            if lower.contains("stale") || lower.contains("attestation") || lower.contains("key") {
                invalidate_cached_venice_attestation(backend, model);
                return Box::pin(create_chat_completion_inner(
                    backend, model, messages, tools, false,
                ))
                .await;
            }
            return Err(LlmError::ApiError {
                status_code: 422,
                reason: body_text,
            });
        }
        let body_text = response.text().await.unwrap_or_default();
        return Err(LlmError::ApiError {
            status_code: status.as_u16(),
            reason: body_text,
        });
    }

    let mut parsed: CreateChatCompletionResponse =
        response.json().await.map_err(|e| LlmError::NetworkError {
            reason: format!("Venice non-stream response JSON: {e}"),
        })?;

    // Decrypt each choice.message.content (server returns hex envelopes).
    for choice in parsed.choices.iter_mut() {
        if let Some(envelope_hex) = choice.message.content.as_deref() {
            // If the body looks like hex of correct minimum length, attempt decrypt.
            let trimmed = envelope_hex.trim();
            let looks_hex = !trimmed.is_empty()
                && trimmed.len() >= 2 * (EPH_PUB_LEN + AES_NONCE_LEN + AES_GCM_TAG_LEN)
                && trimmed.chars().all(|c| c.is_ascii_hexdigit());
            if looks_hex {
                let pt_bytes = open_envelope(trimmed, &aes_key)?;
                let pt = String::from_utf8(pt_bytes).map_err(|e| LlmError::NetworkError {
                    reason: format!("Venice plaintext not UTF-8: {e}"),
                })?;
                choice.message.content = Some(pt);
            }
        }
    }

    Ok(parsed)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn pick_attestation_model(backend: &BackendConfig) -> String {
    backend
        .models
        .first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_VENICE_MODEL.to_string())
}
