use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionTools, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, CreateChatCompletionResponse, CreateChatCompletionStreamResponse,
};
use base64::Engine;
use flate2::read::GzDecoder;
use futures::StreamExt;
use hkdf::Hkdf;
use hpke::{
    aead::AesGcm256, kdf::HkdfSha256, kem::X25519HkdfSha256, setup_sender, Deserializable,
    Kem as KemTrait, OpModeS, Serializable,
};
use hpke::rand_core::TryRngCore;
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use sev::certs::snp::Verifiable;
use sev::parser::ByteParser;
use sha2::{Digest, Sha256};
use x509_parser::extensions::GeneralName;
use x509_parser::pem::parse_x509_pem;
use x509_parser::prelude::{FromDer, X509Certificate};

use zeroize::{Zeroize, ZeroizeOnDrop};

use super::backend::BackendConfig;
use super::error::LlmError;
use crate::attestation::{AttestationError, AttestationEvent};

const ATTESTATION_PATH: &str = "/attestation";
const HPKE_KEYS_PATH: &str = "/.well-known/hpke-keys";
const REQUEST_INFO: &[u8] = b"ehbp request";
const EXPORT_LABEL: &[u8] = b"ehbp response";
const RESPONSE_KEY_LABEL: &[u8] = b"key";
const RESPONSE_NONCE_LABEL: &[u8] = b"nonce";
const RESPONSE_NONCE_LEN: usize = 32;
const EXPORT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const EHBP_ENCAPSULATED_KEY: &str = "ehbp-encapsulated-key";
const EHBP_RESPONSE_NONCE: &str = "ehbp-response-nonce";
const EHBP_KEY_CONFIG_PROBLEM: &str = "urn:ietf:params:ehbp:error:key-config";
const X_TINFOIL_ENCLAVE_URL: &str = "x-tinfoil-enclave-url";
const X_PRIVATE_MODEL: &str = "x-private-model";
const X_QUERY_SOURCE: &str = "x-query-source";
const APPLICATION_OHTTP_KEYS: &str = "application/ohttp-keys";

static VERIFIED_ATTESTATIONS: Lazy<Mutex<HashMap<String, VerifiedPpqAttestation>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, Zeroize, ZeroizeOnDrop)]
struct VerifiedPpqAttestation {
    #[zeroize(skip)]
    request_base_url: String,
    #[zeroize(skip)]
    enclave_url: String,
    hpke_public_key: [u8; 32],
    #[zeroize(skip)]
    report_blob: Vec<u8>,
    #[zeroize(skip)]
    expires_at: u64,
}

#[derive(Debug, Deserialize)]
struct AttestationBundle {
    domain: String,
    #[serde(rename = "enclaveAttestationReport")]
    enclave_attestation_report: AttestationDoc,
    #[serde(rename = "digest")]
    _digest: String,
    #[serde(rename = "sigstoreBundle")]
    _sigstore_bundle: Value,
    vcek: String,
    #[serde(rename = "enclaveCert")]
    enclave_cert: String,
}

#[derive(Debug, Deserialize, Clone)]
struct AttestationDoc {
    format: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct ProblemDetails {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    detail: String,
}

pub fn model_list_url(backend: &BackendConfig) -> Result<String, LlmError> {
    Ok(format!("{}/models", public_api_base(backend)?))
}

pub fn build_http_client(timeout: Duration) -> Result<reqwest::Client, LlmError> {
    reqwest::Client::builder()
        .hickory_dns(false)
        .timeout(timeout)
        .build()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })
}

pub async fn verify_backend_attestation(
    backend: &BackendConfig,
    snp_policy: &crate::attestation::SnpPolicy,
) -> Result<AttestationEvent, AttestationError> {
    let verified = ensure_verified_attestation(backend, snp_policy)
        .await
        .map_err(llm_to_attestation_error)?;
    Ok(AttestationEvent::Verified {
        backend_id: backend.id.clone(),
        tee_type: "AmdSevSnp".to_string(),
        report_blob: verified.report_blob.clone(),
        expires_at: verified.expires_at,
        tls_public_key_fp: None,
        vcek_url: None,
        vcek_der: None,
    })
}

pub async fn create_chat_completion(
    backend: &BackendConfig,
    model: &str,
    messages: Vec<ChatCompletionRequestMessage>,
    tools: Vec<ChatCompletionTools>,
) -> Result<CreateChatCompletionResponse, LlmError> {
    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .tools(tools)
        .build()
        .map_err(
            |error: async_openai::error::OpenAIError| LlmError::NetworkError {
                reason: error.to_string(),
            },
        )?;
    let body = build_private_chat_body(model, &request)?;
    let response = send_private_request(backend, "/chat/completions", model, body, true).await?;
    let decrypted = decrypt_response_bytes(response).await?;
    serde_json::from_slice::<CreateChatCompletionResponse>(&decrypted).map_err(|error| {
        LlmError::NetworkError {
            reason: format!("Invalid PPQ private response JSON: {error}"),
        }
    })
}

pub async fn run_streaming_chat_completion(
    backend: BackendConfig,
    model: String,
    messages: Vec<crate::llm::streaming::ChatMessage>,
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: flume::Sender<crate::CoreMsg>,
) {
    use crate::llm::error::LlmError;
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    };

    let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();
    for msg in &messages {
        let result: Result<ChatCompletionRequestMessage, String> = match msg.role {
            crate::llm::streaming::ChatRole::System => ChatCompletionRequestSystemMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map(ChatCompletionRequestMessage::from)
                .map_err(|e| e.to_string()),
            crate::llm::streaming::ChatRole::User => ChatCompletionRequestUserMessageArgs::default()
                .content(msg.content.clone())
                .build()
                .map(ChatCompletionRequestMessage::from)
                .map_err(|e| e.to_string()),
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

    let request = match CreateChatCompletionRequestArgs::default()
        .model(model.as_str())
        .messages(openai_messages)
        .stream(true)
        .build()
    {
        Ok(request) => request,
        Err(error) => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                crate::llm::streaming::InternalEvent::StreamError {
                    error: LlmError::NetworkError {
                        reason: error.to_string(),
                    },
                },
            )));
            return;
        }
    };

    let body = match build_private_chat_body(&model, &request) {
        Ok(body) => body,
        Err(error) => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                crate::llm::streaming::InternalEvent::StreamError { error },
            )));
            return;
        }
    };

    let response = match send_private_request(&backend, "/chat/completions", &model, body, true).await
    {
        Ok(response) => response,
        Err(error) => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                crate::llm::streaming::InternalEvent::StreamError { error },
            )));
            return;
        }
    };

    if let Err(error) = stream_decrypted_sse(response, cancel_token, &core_tx).await {
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            crate::llm::streaming::InternalEvent::StreamError { error },
        )));
    }
}

async fn stream_decrypted_sse(
    response: EncryptedResponse,
    cancel_token: tokio_util::sync::CancellationToken,
    core_tx: &flume::Sender<crate::CoreMsg>,
) -> Result<(), LlmError> {
    let mut body_stream = response.response.bytes_stream();
    let mut framed = Vec::new();
    let mut sse = String::new();
    let mut seq = 0u64;

    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    crate::llm::streaming::InternalEvent::StreamCancelled,
                )));
                return Ok(());
            }
            next = body_stream.next() => {
                match next {
                    Some(Ok(bytes)) => {
                        framed.extend_from_slice(&bytes);
                        while let Some(chunk) = try_take_frame(&mut framed)? {
                            let plaintext = decrypt_chunk(&response.key_material, seq, &chunk)?;
                            seq = seq.saturating_add(1);
                            sse.push_str(&String::from_utf8_lossy(&plaintext));
                            while let Some(event) = take_sse_event(&mut sse) {
                                if !handle_sse_event(&event, &core_tx)? {
                                    let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                                        crate::llm::streaming::InternalEvent::StreamDone,
                                    )));
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Some(Err(error)) => {
                        return Err(LlmError::NetworkError {
                            reason: error.to_string(),
                        });
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

fn handle_sse_event(
    raw_event: &str,
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
        serde_json::from_str(&payload).map_err(|error| LlmError::NetworkError {
            reason: format!("Invalid PPQ private SSE chunk: {error}"),
        })?;

    if let Some(content) = chunk
        .choices
        .first()
        .and_then(|choice| choice.delta.content.as_deref())
    {
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            crate::llm::streaming::InternalEvent::StreamChunk {
                token: content.to_string(),
            },
        )));
    }

    Ok(true)
}

async fn decrypt_response_bytes(response: EncryptedResponse) -> Result<Vec<u8>, LlmError> {
    let mut stream = response.response.bytes_stream();
    let mut framed = Vec::new();
    let mut plaintext = Vec::new();
    let mut seq = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?;
        framed.extend_from_slice(&chunk);
        while let Some(frame) = try_take_frame(&mut framed)? {
            plaintext.extend_from_slice(&decrypt_chunk(&response.key_material, seq, &frame)?);
            seq = seq.saturating_add(1);
        }
    }

    if !framed.is_empty() {
        return Err(LlmError::NetworkError {
            reason: "Truncated PPQ private encrypted response".to_string(),
        });
    }

    Ok(plaintext)
}

struct EncryptedResponse {
    response: reqwest::Response,
    key_material: ResponseKeyMaterial,
}

#[derive(Clone)]
struct ResponseKeyMaterial {
    key: [u8; KEY_LEN],
    nonce_base: [u8; NONCE_LEN],
}

async fn send_private_request(
    backend: &BackendConfig,
    path: &str,
    model: &str,
    body: Vec<u8>,
    allow_retry: bool,
) -> Result<EncryptedResponse, LlmError> {
    // The attestation task will have already run with the loaded policy (and
    // populated the cache). If the cache is cold here we fall back to defaults,
    // which match the previously hardcoded constants.
    let verified = ensure_verified_attestation(backend, &crate::attestation::SnpPolicy::default()).await?;
    let encrypted = encrypt_request_body(&verified.hpke_public_key, &body)?;
    let client = build_http_client(Duration::from_secs(90))?;

    let endpoint = format!("{}{}", verified.request_base_url, path);
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&format!("Bearer {}", backend.api_key)).map_err(|error| {
            LlmError::AuthError {
                reason: error.to_string(),
            }
        })?,
    );
    headers.insert(
        HeaderName::from_static(X_PRIVATE_MODEL),
        HeaderValue::from_str(resolve_private_model(model)?.0).map_err(|error| {
            LlmError::NetworkError {
                reason: error.to_string(),
            }
        })?,
    );
    headers.insert(
        HeaderName::from_static(X_QUERY_SOURCE),
        HeaderValue::from_static("api"),
    );
    headers.insert(
        HeaderName::from_static(X_TINFOIL_ENCLAVE_URL),
        HeaderValue::from_str(&verified.enclave_url).map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?,
    );
    headers.insert(
        HeaderName::from_static(EHBP_ENCAPSULATED_KEY),
        HeaderValue::from_str(&hex::encode(encrypted.request_enc)).map_err(|error| {
            LlmError::NetworkError {
                reason: error.to_string(),
            }
        })?,
    );

    let response = client
        .post(endpoint)
        .headers(headers)
        .body(encrypted.encrypted_body)
        .send()
        .await
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?;

    if !response.status().is_success() {
        let (status, body_text, problem) = parse_problem_response(response).await;
        if status == 422
            && allow_retry
            && problem
                .as_ref()
                .map(|problem| problem.r#type == EHBP_KEY_CONFIG_PROBLEM)
                .unwrap_or(false)
        {
            invalidate_cached_attestation(backend);
            return Box::pin(send_private_request(backend, path, model, body, false)).await;
        }
        return Err(map_plain_error_body(status, &body_text, problem.as_ref()));
    }

    let response_nonce = response
        .headers()
        .get(EHBP_RESPONSE_NONCE)
        .ok_or_else(|| LlmError::NetworkError {
            reason: format!("Missing {EHBP_RESPONSE_NONCE} header from PPQ private response"),
        })?
        .to_str()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?;
    let response_nonce = hex::decode(response_nonce).map_err(|error| LlmError::NetworkError {
        reason: format!("Invalid PPQ private response nonce: {error}"),
    })?;
    let key_material =
        derive_response_key_material(&encrypted.exported_secret, &encrypted.request_enc, &response_nonce)?;

    Ok(EncryptedResponse {
        response,
        key_material,
    })
}

async fn parse_problem_response(response: reqwest::Response) -> (u16, String, Option<ProblemDetails>) {
    let status = response.status().as_u16();
    let body_text = response.text().await.unwrap_or_default();
    let problem = serde_json::from_str::<ProblemDetails>(&body_text).ok();
    (status, body_text, problem)
}

fn map_plain_error_body(status: u16, body_text: &str, problem: Option<&ProblemDetails>) -> LlmError {
    let parsed = serde_json::from_str::<Value>(&body_text).ok();
    let message = parsed
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(|error| error.get("message"))
        .and_then(|value| value.as_str())
        .or_else(|| problem.and_then(|problem| (!problem.title.is_empty()).then_some(problem.title.as_str())))
        .or_else(|| problem.and_then(|problem| (!problem.detail.is_empty()).then_some(problem.detail.as_str())))
        .unwrap_or_else(|| body_text.trim());
    match status {
        401 | 403 => LlmError::AuthError {
            reason: if message.is_empty() {
                "Invalid or missing API key".to_string()
            } else {
                message.to_string()
            },
        },
        404 => LlmError::ModelNotFound {
            model_id: "unknown".to_string(),
        },
        429 => LlmError::RateLimited {
            reason: if message.is_empty() {
                "Please try again later".to_string()
            } else {
                message.to_string()
            },
            retry_after_secs: None,
        },
        _ => LlmError::ApiError {
            status_code: status,
            reason: if message.is_empty() {
                format!("HTTP {status}")
            } else {
                message.to_string()
            },
        },
    }
}

struct EncryptedRequest {
    encrypted_body: Vec<u8>,
    request_enc: [u8; 32],
    exported_secret: [u8; EXPORT_LEN],
}

fn encrypt_request_body(server_public_key: &[u8; 32], body: &[u8]) -> Result<EncryptedRequest, LlmError> {
    type Kem = X25519HkdfSha256;
    type Kdf = HkdfSha256;
    type Aead = AesGcm256;

    let public_key = <Kem as KemTrait>::PublicKey::from_bytes(server_public_key).map_err(|error| {
        LlmError::NetworkError {
            reason: format!("Invalid attested HPKE public key: {error}"),
        }
    })?;

    let mut rng = hpke::rand_core::OsRng.unwrap_err();
    let (encapped_key, mut ctx) =
        setup_sender::<Aead, Kdf, Kem, _>(&OpModeS::Base, &public_key, REQUEST_INFO, &mut rng).map_err(
            |error| LlmError::NetworkError {
                reason: format!("Failed to initialize HPKE sender context: {error}"),
            },
        )?;

    let ciphertext = ctx.seal(body, &[]).map_err(|error| LlmError::NetworkError {
        reason: format!("Failed to encrypt PPQ private request body: {error}"),
    })?;

    let mut exported_secret = [0u8; EXPORT_LEN];
    ctx.export(EXPORT_LABEL, &mut exported_secret)
        .map_err(|error| LlmError::NetworkError {
            reason: format!("Failed to export PPQ private response secret: {error}"),
        })?;

    let encapped_bytes = encapped_key.to_bytes();
    let mut request_enc = [0u8; 32];
    request_enc.copy_from_slice(encapped_bytes.as_ref());

    let mut encrypted_body = Vec::with_capacity(4 + ciphertext.len());
    encrypted_body.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
    encrypted_body.extend_from_slice(&ciphertext);

    Ok(EncryptedRequest {
        encrypted_body,
        request_enc,
        exported_secret,
    })
}

fn derive_response_key_material(
    exported_secret: &[u8; EXPORT_LEN],
    request_enc: &[u8; 32],
    response_nonce: &[u8],
) -> Result<ResponseKeyMaterial, LlmError> {
    if response_nonce.len() != RESPONSE_NONCE_LEN {
        return Err(LlmError::NetworkError {
            reason: format!(
                "Invalid PPQ private response nonce length: expected {RESPONSE_NONCE_LEN}, got {}",
                response_nonce.len()
            ),
        });
    }

    let mut salt = Vec::with_capacity(request_enc.len() + response_nonce.len());
    salt.extend_from_slice(request_enc);
    salt.extend_from_slice(response_nonce);

    let hkdf = Hkdf::<Sha256>::new(Some(&salt), exported_secret);
    let mut key = [0u8; KEY_LEN];
    hkdf.expand(RESPONSE_KEY_LABEL, &mut key)
        .map_err(|_| LlmError::NetworkError {
            reason: "Failed to derive PPQ private response key".to_string(),
        })?;
    let mut nonce_base = [0u8; NONCE_LEN];
    hkdf.expand(RESPONSE_NONCE_LABEL, &mut nonce_base)
        .map_err(|_| LlmError::NetworkError {
            reason: "Failed to derive PPQ private response nonce".to_string(),
        })?;

    Ok(ResponseKeyMaterial { key, nonce_base })
}

fn decrypt_chunk(
    key_material: &ResponseKeyMaterial,
    seq: u64,
    ciphertext: &[u8],
) -> Result<Vec<u8>, LlmError> {
    let cipher = Aes256Gcm::new_from_slice(&key_material.key).map_err(|error| LlmError::NetworkError {
        reason: error.to_string(),
    })?;
    let nonce = compute_chunk_nonce(&key_material.nonce_base, seq);
    let nonce = Nonce::from(nonce);
    cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| LlmError::NetworkError {
            reason: format!("Failed to decrypt PPQ private response chunk {}", seq),
        })
}

fn compute_chunk_nonce(base_nonce: &[u8; NONCE_LEN], seq: u64) -> [u8; NONCE_LEN] {
    let mut nonce = *base_nonce;
    for i in 0..8 {
        nonce[NONCE_LEN - 1 - i] ^= ((seq >> (i * 8)) & 0xff) as u8;
    }
    nonce
}

fn try_take_frame(buffer: &mut Vec<u8>) -> Result<Option<Vec<u8>>, LlmError> {
    if buffer.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    if len == 0 {
        buffer.drain(..4);
        return Ok(None);
    }
    if buffer.len() < 4 + len {
        return Ok(None);
    }
    let frame = buffer[4..4 + len].to_vec();
    buffer.drain(..4 + len);
    Ok(Some(frame))
}

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

fn build_private_chat_body(
    model: &str,
    request: &CreateChatCompletionRequest,
) -> Result<Vec<u8>, LlmError> {
    let (_external_model, internal_model) = resolve_private_model(model)?;
    let mut value = serde_json::to_value(request).map_err(|error| LlmError::NetworkError {
        reason: error.to_string(),
    })?;
    let object = value.as_object_mut().ok_or_else(|| LlmError::NetworkError {
        reason: "Invalid chat completion request shape".to_string(),
    })?;
    object.insert("model".to_string(), Value::String(internal_model.to_string()));
    serde_json::to_vec(&value).map_err(|error| LlmError::NetworkError {
        reason: error.to_string(),
    })
}

fn resolve_private_model(model: &str) -> Result<(&'static str, &'static str), LlmError> {
    match model {
        "private/kimi-k2-5" | "kimi-k2-5" => Ok(("private/kimi-k2-5", "kimi-k2-5")),
        "private/deepseek-r1-0528" | "deepseek-r1-0528" => {
            Ok(("private/deepseek-r1-0528", "deepseek-r1-0528"))
        }
        "private/gpt-oss-120b" | "gpt-oss-120b" => Ok(("private/gpt-oss-120b", "gpt-oss-120b")),
        "private/llama3-3-70b" | "llama3-3-70b" => Ok(("private/llama3-3-70b", "llama3-3-70b")),
        "private/qwen3-vl-30b" | "qwen3-vl-30b" => Ok(("private/qwen3-vl-30b", "qwen3-vl-30b")),
        _ => Err(LlmError::ModelNotFound {
            model_id: model.to_string(),
        }),
    }
}

async fn ensure_verified_attestation(
    backend: &BackendConfig,
    snp_policy: &crate::attestation::SnpPolicy,
) -> Result<VerifiedPpqAttestation, LlmError> {
    let cache_key = backend.base_url.trim_end_matches('/').to_string();
    let now_secs = now_secs();

    {
        let mut cache = VERIFIED_ATTESTATIONS
            .lock()
            .map_err(|_| LlmError::NetworkError {
                reason: "Attestation cache lock poisoned".to_string(),
            })?;
        if let Some(cached) = cache.get(&cache_key) {
            if cached.expires_at > now_secs {
                return Ok(cached.clone());
            }
            // Per D-02: proactively evict expired entry so key bytes are zeroed
            // via ZeroizeOnDrop when the removed value is dropped.
            cache.remove(&cache_key);
        }
    }

    let verified = fetch_and_verify_attestation(backend, snp_policy).await?;
    VERIFIED_ATTESTATIONS
        .lock()
        .map_err(|_| LlmError::NetworkError {
            reason: "Attestation cache lock poisoned".to_string(),
        })?
        .insert(cache_key, verified.clone());
    Ok(verified)
}

fn invalidate_cached_attestation(backend: &BackendConfig) {
    if let Ok(mut cache) = VERIFIED_ATTESTATIONS.lock() {
        cache.remove(backend.base_url.trim_end_matches('/'));
    }
}

async fn fetch_and_verify_attestation(
    backend: &BackendConfig,
    snp_policy: &crate::attestation::SnpPolicy,
) -> Result<VerifiedPpqAttestation, LlmError> {
    let request_base_url = backend.base_url.trim_end_matches('/').to_string();
    let attestation_base_url = private_attestation_base(backend)?;
    let client = build_http_client(Duration::from_secs(30))?;
    let bundle_url = format!("{attestation_base_url}{ATTESTATION_PATH}");

    let bundle: AttestationBundle = client
        .get(&bundle_url)
        .send()
        .await
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?
        .error_for_status()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?
        .json()
        .await
        .map_err(|error| LlmError::NetworkError {
            reason: format!("Invalid PPQ private attestation bundle JSON: {error}"),
        })?;

    let report_blob = decode_attestation_body(&bundle.enclave_attestation_report.body)?;
    let (_tls_public_key_fp, hpke_public_key_hex, hpke_public_key) =
        verify_sev_attestation_bundle(&bundle, &report_blob, snp_policy)?;
    verify_certificate_binding(
        &bundle.enclave_cert,
        &bundle.domain,
        &bundle.enclave_attestation_report,
        &hpke_public_key_hex,
    )?;
    verify_hpke_key_endpoint(&client, &bundle.domain, &hpke_public_key).await?;

    Ok(VerifiedPpqAttestation {
        request_base_url,
        enclave_url: format!("https://{}", bundle.domain),
        hpke_public_key,
        report_blob,
        expires_at: now_secs() + 4 * 3600,
    })
}

fn private_attestation_base(backend: &BackendConfig) -> Result<String, LlmError> {
    let trimmed = backend.base_url.trim_end_matches('/');
    if let Some(stripped) = trimmed.strip_suffix("/v1") {
        return Ok(stripped.to_string());
    }
    if trimmed.ends_with("/private") {
        return Ok(trimmed.to_string());
    }
    Err(LlmError::NetworkError {
        reason: format!("Invalid PPQ private base URL: {}", backend.base_url),
    })
}

fn public_api_base(backend: &BackendConfig) -> Result<String, LlmError> {
    let private_base = private_attestation_base(backend)?;
    let public_root = private_base.trim_end_matches("/private");
    Ok(format!("{public_root}/v1"))
}

fn verify_sev_attestation_bundle(
    bundle: &AttestationBundle,
    report_blob: &[u8],
    snp_policy: &crate::attestation::SnpPolicy,
) -> Result<(String, String, [u8; 32]), LlmError> {
    if bundle.enclave_attestation_report.format != "https://tinfoil.sh/predicate/sev-snp-guest/v2" {
        return Err(LlmError::NetworkError {
            reason: format!(
                "Unsupported PPQ private attestation format: {}",
                bundle.enclave_attestation_report.format
            ),
        });
    }

    if report_blob.len() != 1184 {
        return Err(LlmError::NetworkError {
            reason: format!("Unexpected SNP report size: {}", report_blob.len()),
        });
    }

    let report = sev::firmware::guest::AttestationReport::from_bytes(report_blob).map_err(|error| {
        LlmError::NetworkError {
            reason: format!("Invalid SEV-SNP report: {error}"),
        }
    })?;
    let generation = match (report.cpuid_fam_id, report.cpuid_mod_id) {
        (Some(family), Some(model)) => sev::Generation::identify_cpu(family, model).map_err(|error| {
            LlmError::NetworkError {
                reason: format!(
                    "Unknown AMD generation family={family:#x} model={model:#x}: {error}"
                ),
            }
        })?,
        _ => sev::Generation::Genoa,
    };

    let (ark_pem, ask_pem): (&[u8], &[u8]) = match generation {
        sev::Generation::Milan => (sev::certs::snp::builtin::milan::ARK, sev::certs::snp::builtin::milan::ASK),
        sev::Generation::Genoa => (sev::certs::snp::builtin::genoa::ARK, sev::certs::snp::builtin::genoa::ASK),
        sev::Generation::Turin => (sev::certs::snp::builtin::turin::ARK, sev::certs::snp::builtin::turin::ASK),
    };
    let ca_chain = sev::certs::snp::ca::Chain::from_pem(ark_pem, ask_pem).map_err(|error| {
        LlmError::NetworkError {
            reason: format!("Invalid AMD SEV CA chain: {error}"),
        }
    })?;

    let vcek_der = base64::engine::general_purpose::STANDARD
        .decode(&bundle.vcek)
        .map_err(|error| LlmError::NetworkError {
            reason: format!("Invalid base64 VCEK certificate: {error}"),
        })?;
    verify_snp_signature_with_vcek(&ca_chain, &report, &vcek_der)?;
    verify_snp_policy(&report, snp_policy)?;

    let report_data = &report.report_data;
    let tls_public_key_fp = hex::encode(&report_data[..32]);
    let hpke_public_key_hex = hex::encode(&report_data[32..64]);
    let mut hpke_public_key = [0u8; 32];
    hpke_public_key.copy_from_slice(&report_data[32..64]);

    Ok((tls_public_key_fp, hpke_public_key_hex, hpke_public_key))
}

fn verify_snp_signature_with_vcek(
    ca_chain: &sev::certs::snp::ca::Chain,
    report: &sev::firmware::guest::AttestationReport,
    vcek_der: &[u8],
) -> Result<(), LlmError> {
    let vcek = sev::certs::snp::Certificate::from_der(vcek_der).map_err(|error| LlmError::NetworkError {
        reason: format!("Invalid VCEK certificate: {error}"),
    })?;
    let chain = sev::certs::snp::Chain {
        ca: ca_chain.clone(),
        vek: vcek,
    };
    (&chain, report)
        .verify()
        .map_err(|error| LlmError::NetworkError {
            reason: format!("SNP signature verification failed: {error}"),
        })?;
    Ok(())
}

// The default minimum_tee is 0x00 (no minimum enforced for the tee field).
// Clippy correctly flags `u8 < 0` as always-false; the check is retained
// for future minimum bumps without code changes.
#[allow(clippy::absurd_extreme_comparisons)]
fn verify_snp_policy(
    report: &sev::firmware::guest::AttestationReport,
    snp_policy: &crate::attestation::SnpPolicy,
) -> Result<(), LlmError> {
    let guest_policy = report.policy;
    if !guest_policy.smt_allowed() {
        return Err(LlmError::NetworkError {
            reason: "SNP guest policy disallows SMT".to_string(),
        });
    }
    if guest_policy.migrate_ma_allowed() {
        return Err(LlmError::NetworkError {
            reason: "SNP guest policy allows migration agents".to_string(),
        });
    }
    if guest_policy.debug_allowed() {
        return Err(LlmError::NetworkError {
            reason: "SNP guest policy allows debug mode".to_string(),
        });
    }
    if report.current_tcb.bootloader < snp_policy.minimum_bootloader
        || report.current_tcb.tee < snp_policy.minimum_tee
        || report.current_tcb.snp < snp_policy.minimum_snp
        || report.current_tcb.microcode < snp_policy.minimum_microcode
        || report.reported_tcb.bootloader < snp_policy.minimum_bootloader
        || report.reported_tcb.tee < snp_policy.minimum_tee
        || report.reported_tcb.snp < snp_policy.minimum_snp
        || report.reported_tcb.microcode < snp_policy.minimum_microcode
    {
        return Err(LlmError::NetworkError {
            reason: "SNP TCB is below the required minimum".to_string(),
        });
    }
    let info = report.plat_info;
    if !info.smt_enabled() {
        return Err(LlmError::NetworkError {
            reason: "SNP platform info shows SMT disabled".to_string(),
        });
    }
    if !info.tsme_enabled() {
        return Err(LlmError::NetworkError {
            reason: "SNP platform info shows TSME disabled".to_string(),
        });
    }
    Ok(())
}

fn verify_certificate_binding(
    cert_pem: &str,
    expected_domain: &str,
    attestation_doc: &AttestationDoc,
    expected_hpke_key_hex: &str,
) -> Result<(), LlmError> {
    let (_, pem) = parse_x509_pem(cert_pem.as_bytes()).map_err(|error| LlmError::NetworkError {
        reason: format!("Failed to parse enclave certificate PEM: {error}"),
    })?;
    let (_, cert) = X509Certificate::from_der(&pem.contents).map_err(|error| LlmError::NetworkError {
        reason: format!("Failed to parse enclave certificate DER: {error}"),
    })?;

    let sans = extract_dns_sans(&cert)?;
    if !domain_matches_sans(&sans, expected_domain) {
        return Err(LlmError::NetworkError {
            reason: format!(
                "Certificate domain mismatch: enclave certificate is not valid for {expected_domain}"
            ),
        });
    }

    let hpke_bytes = decode_prefixed_san_data(&sans, "hpke")?;
    let hpke_public_key_hex = hex::encode(hpke_bytes);
    if hpke_public_key_hex != expected_hpke_key_hex {
        return Err(LlmError::NetworkError {
            reason: "HPKE key mismatch between certificate and attestation report".to_string(),
        });
    }

    let attestation_hash_bytes = decode_prefixed_san_data(&sans, "hatt")?;
    let attestation_hash = String::from_utf8(attestation_hash_bytes).map_err(|error| {
        LlmError::NetworkError {
            reason: format!("Invalid attestation hash encoding in certificate SAN: {error}"),
        }
    })?;
    let expected_hash = hex::encode(Sha256::digest(format!(
        "{}{}",
        attestation_doc.format, attestation_doc.body
    )));
    if attestation_hash != expected_hash {
        return Err(LlmError::NetworkError {
            reason: "Attestation hash mismatch between certificate and attestation bundle".to_string(),
        });
    }

    Ok(())
}

async fn verify_hpke_key_endpoint(
    client: &Client,
    domain: &str,
    expected_hpke_key: &[u8; 32],
) -> Result<(), LlmError> {
    let url = format!("https://{domain}{HPKE_KEYS_PATH}");
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?
        .error_for_status()
        .map_err(|error| LlmError::NetworkError {
            reason: error.to_string(),
        })?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if content_type != APPLICATION_OHTTP_KEYS {
        return Err(LlmError::NetworkError {
            reason: format!(
                "Invalid HPKE key endpoint content type: expected {APPLICATION_OHTTP_KEYS}, got {content_type}"
            ),
        });
    }
    let bytes = response.bytes().await.map_err(|error| LlmError::NetworkError {
        reason: error.to_string(),
    })?;
    let parsed_key = parse_ohttp_public_key(&bytes)?;
    if &parsed_key != expected_hpke_key {
        return Err(LlmError::NetworkError {
            reason: "HPKE key endpoint returned a different public key than the attested key".to_string(),
        });
    }
    Ok(())
}

fn parse_ohttp_public_key(data: &[u8]) -> Result<[u8; 32], LlmError> {
    if data.len() < 39 {
        return Err(LlmError::NetworkError {
            reason: format!("Invalid OHTTP key config length: {}", data.len()),
        });
    }
    let public_key_len = 32usize;
    let public_key_start = 3usize;
    let public_key_end = public_key_start + public_key_len;
    if data.len() < public_key_end {
        return Err(LlmError::NetworkError {
            reason: "Truncated OHTTP key config".to_string(),
        });
    }
    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&data[public_key_start..public_key_end]);
    Ok(public_key)
}

fn extract_dns_sans(cert: &X509Certificate<'_>) -> Result<Vec<String>, LlmError> {
    let Ok(Some(extension)) = cert.subject_alternative_name() else {
        return Err(LlmError::NetworkError {
            reason: "Enclave certificate is missing Subject Alternative Names".to_string(),
        });
    };
    let mut sans = Vec::new();
    for general_name in &extension.value.general_names {
        if let GeneralName::DNSName(name) = general_name {
            sans.push(name.to_string());
        }
    }
    if sans.is_empty() {
        return Err(LlmError::NetworkError {
            reason: "Enclave certificate does not contain DNS Subject Alternative Names".to_string(),
        });
    }
    Ok(sans)
}

fn domain_matches_sans(sans: &[String], expected_domain: &str) -> bool {
    let parent = parent_domain(expected_domain);
    sans.iter().any(|san| {
        san == expected_domain
            || (san.starts_with("*.") && san.trim_start_matches("*.") == parent && expected_domain != parent)
    })
}

fn parent_domain(domain: &str) -> &str {
    match domain.split_once('.') {
        Some((_, rest)) if rest.contains('.') => rest,
        _ => domain,
    }
}

fn decode_prefixed_san_data(sans: &[String], prefix: &str) -> Result<Vec<u8>, LlmError> {
    let marker = format!(".{prefix}.");
    let mut chunks = sans
        .iter()
        .filter(|san| san.contains(&marker))
        .collect::<Vec<_>>();
    chunks.sort_by_key(|san| san.get(..2).and_then(|idx| idx.parse::<u8>().ok()).unwrap_or(0));
    let joined = chunks
        .into_iter()
        .filter_map(|san| san.split('.').next())
        .map(|label| label.get(2..).unwrap_or_default())
        .collect::<String>();
    if joined.is_empty() {
        return Err(LlmError::NetworkError {
            reason: format!("Certificate SANs do not contain {prefix} data"),
        });
    }
    decode_base32_upper(&joined)
}

fn decode_base32_upper(input: &str) -> Result<Vec<u8>, LlmError> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits = 0u32;
    let mut value = 0u32;
    let mut output = Vec::with_capacity(input.len() * 5 / 8);
    for byte in input.bytes() {
        let byte = byte.to_ascii_uppercase();
        let Some(index) = alphabet.iter().position(|candidate| *candidate == byte) else {
            return Err(LlmError::NetworkError {
                reason: format!("Invalid base32 character in certificate SAN data: {}", byte as char),
            });
        };
        value = (value << 5) | index as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            output.push(((value >> bits) & 0xff) as u8);
        }
    }
    Ok(output)
}

fn decode_attestation_body(body: &str) -> Result<Vec<u8>, LlmError> {
    let encoded = base64::engine::general_purpose::STANDARD
        .decode(body)
        .map_err(|error| LlmError::NetworkError {
            reason: format!("Invalid attestation body encoding: {error}"),
        })?;
    let mut decoder = GzDecoder::new(encoded.as_slice());
    let mut out = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut out).map_err(|error| LlmError::NetworkError {
        reason: format!("Invalid attestation body compression: {error}"),
    })?;
    Ok(out)
}

fn llm_to_attestation_error(error: LlmError) -> AttestationError {
    match error {
        LlmError::NetworkError { reason }
        | LlmError::AuthError { reason } => AttestationError::NetworkError { reason },
        LlmError::RateLimited { reason, .. } => AttestationError::NetworkError { reason },
        LlmError::ModelNotFound { model_id } => AttestationError::Unsupported.with_context(&model_id),
        LlmError::ApiError { reason, .. } => AttestationError::QuoteVerification { reason },
    }
}

trait AttestationContextExt {
    fn with_context(self, context: &str) -> AttestationError;
}

impl AttestationContextExt for AttestationError {
    fn with_context(self, context: &str) -> AttestationError {
        match self {
            AttestationError::Unsupported => AttestationError::QuoteVerification {
                reason: format!("Unsupported attestation configuration: {context}"),
            },
            other => other,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
