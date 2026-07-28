use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use once_cell::sync::Lazy;
use tokio_util::sync::CancellationToken;

use super::error::LlmError;
use super::local_models::{
    find_local_model, local_model_path, local_model_verified, LocalModelDownloadProgress,
    LocalModelPreset,
};
use super::streaming::InternalEvent;

/// Device-side capability summary for local LLM inference.
#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum LocalLlmCapabilityStatus {
    Unknown,
    Supported,
    DisabledByFeatureFlag,
    UnsupportedApiLevel,
    UnsupportedArchitecture,
    UnsupportedProcessBitness,
    UnsupportedCpuFeatures,
    InsufficientMemory,
    InsufficientStorage,
    ProbeUnavailable,
    RuntimeNotPackaged,
    RuntimeLoadFailed,
}

impl LocalLlmCapabilityStatus {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }

    pub fn stable_code(&self) -> String {
        match self {
            Self::Unknown => "unknown".to_string(),
            Self::Supported => "supported".to_string(),
            Self::DisabledByFeatureFlag => "disabled_by_feature_flag".to_string(),
            Self::UnsupportedApiLevel => "unsupported_api_level".to_string(),
            Self::UnsupportedArchitecture => "unsupported_architecture".to_string(),
            Self::UnsupportedProcessBitness => "unsupported_process_bitness".to_string(),
            Self::UnsupportedCpuFeatures => "unsupported_cpu_features".to_string(),
            Self::InsufficientMemory => "insufficient_memory".to_string(),
            Self::InsufficientStorage => "insufficient_storage".to_string(),
            Self::ProbeUnavailable => "probe_unavailable".to_string(),
            Self::RuntimeNotPackaged => "runtime_not_packaged".to_string(),
            Self::RuntimeLoadFailed => "runtime_load_failed".to_string(),
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct DeviceCapability {
    pub abi: String,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub supports_mmap: bool,
    pub status: LocalLlmCapabilityStatus,
    pub reason_code: String,
    pub reason: Option<String>,
    pub available_storage_bytes: u64,
}

impl DeviceCapability {
    pub fn supported(status: LocalLlmCapabilityStatus) -> bool {
        status.is_supported()
    }

    pub fn is_supported(&self) -> bool {
        self.status.is_supported()
    }

    pub fn blocked_reason(&self) -> Option<&str> {
        if self.is_supported() {
            return None;
        }
        self.reason.as_deref()
    }

    pub fn blocked_reason_or_default(&self, fallback: &str) -> String {
        self.reason.clone().unwrap_or_else(|| fallback.to_string())
    }
}

impl Default for DeviceCapability {
    fn default() -> Self {
        Self {
            abi: std::env::consts::ARCH.to_string(),
            total_ram_bytes: 0,
            available_ram_bytes: 0,
            supports_mmap: false,
            status: LocalLlmCapabilityStatus::Unknown,
            reason_code: "unknown".to_string(),
            reason: Some("local inference provider unavailable".to_string()),
            available_storage_bytes: 0,
        }
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LocalLlmError {
    #[error("local inference is unsupported on this platform: {reason}")]
    Unsupported { reason: String },
    #[error("model file is missing: {path}")]
    ModelMissing { path: String },
    #[error("failed to load local model: {reason}")]
    LoadFailed { reason: String },
    #[error("local model is not loaded")]
    NotLoaded,
    #[error("local generation failed: {reason}")]
    GenerationFailed { reason: String },
    #[error("local generation was cancelled")]
    Cancelled,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PlatformHttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PlatformHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<PlatformHttpHeader>,
    pub body: Vec<u8>,
    pub timeout_secs: u64,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PlatformHttpResponse {
    pub status_code: u16,
    pub headers: Vec<PlatformHttpHeader>,
    pub body: Vec<u8>,
}

impl LocalLlmError {
    pub fn into_llm_error(self) -> LlmError {
        match self {
            LocalLlmError::ModelMissing { path } => LlmError::ModelNotFound { model_id: path },
            LocalLlmError::Unsupported { reason }
            | LocalLlmError::LoadFailed { reason }
            | LocalLlmError::GenerationFailed { reason } => LlmError::NetworkError { reason },
            LocalLlmError::NotLoaded => LlmError::NetworkError {
                reason: "local model is not loaded".to_string(),
            },
            LocalLlmError::Cancelled => LlmError::NetworkError {
                reason: "local generation was cancelled".to_string(),
            },
        }
    }
}

#[derive(uniffi::Object)]
pub struct LocalGenerationContext {
    core_tx: flume::Sender<crate::CoreMsg>,
    cancel_token: CancellationToken,
    error_sent: AtomicBool,
}

impl LocalGenerationContext {
    fn new(core_tx: flume::Sender<crate::CoreMsg>, cancel_token: CancellationToken) -> Arc<Self> {
        Arc::new(Self {
            core_tx,
            cancel_token,
            error_sent: AtomicBool::new(false),
        })
    }

    fn error_sent(&self) -> bool {
        self.error_sent.load(Ordering::SeqCst)
    }
}

#[uniffi::export]
impl LocalGenerationContext {
    pub fn emit_token(&self, token: String) {
        let _ = self.core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            InternalEvent::StreamChunk { token },
        )));
    }

    pub fn emit_error(&self, message: String) {
        self.error_sent.store(true, Ordering::SeqCst);
        let _ = self.core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            InternalEvent::StreamError {
                error: LlmError::NetworkError { reason: message },
            },
        )));
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

#[derive(uniffi::Object)]
pub struct LocalModelDownloadContext {
    model_id: String,
    core_tx: flume::Sender<crate::CoreMsg>,
}

impl LocalModelDownloadContext {
    pub(crate) fn new(model_id: String, core_tx: flume::Sender<crate::CoreMsg>) -> Arc<Self> {
        Arc::new(Self { model_id, core_tx })
    }
}

#[uniffi::export]
impl LocalModelDownloadContext {
    pub fn emit_progress(&self, downloaded_bytes: u64, total_bytes: Option<u64>) {
        let _ = self.core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            InternalEvent::LocalModelDownloadProgress(LocalModelDownloadProgress {
                model_id: self.model_id.clone(),
                downloaded_bytes,
                total_bytes,
                stage: "downloading".to_string(),
            }),
        )));
    }
}

#[uniffi::export(callback_interface)]
pub trait LocalLlmProvider: Send + Sync + 'static {
    fn load_model(&self, model_path: String) -> Result<(), LocalLlmError>;
    fn download_model_file(
        &self,
        url: String,
        destination_path: String,
        context: Arc<LocalModelDownloadContext>,
    ) -> Result<(), LocalLlmError>;
    fn platform_http_request(
        &self,
        request: PlatformHttpRequest,
    ) -> Result<PlatformHttpResponse, LocalLlmError>;
    fn generate(
        &self,
        prompt_json: String,
        context: Arc<LocalGenerationContext>,
    ) -> Result<(), LocalLlmError>;
    fn unload(&self);
    fn loaded_model_path(&self) -> Option<String>;
    fn device_capability(&self) -> DeviceCapability;
}

pub struct NullLocalLlmProvider;

impl LocalLlmProvider for NullLocalLlmProvider {
    fn load_model(&self, _model_path: String) -> Result<(), LocalLlmError> {
        Err(LocalLlmError::Unsupported {
            reason: "no local LLM provider is installed".to_string(),
        })
    }

    fn download_model_file(
        &self,
        _url: String,
        _destination_path: String,
        _context: Arc<LocalModelDownloadContext>,
    ) -> Result<(), LocalLlmError> {
        Err(LocalLlmError::Unsupported {
            reason: "no local LLM provider is installed".to_string(),
        })
    }

    fn platform_http_request(
        &self,
        _request: PlatformHttpRequest,
    ) -> Result<PlatformHttpResponse, LocalLlmError> {
        Err(LocalLlmError::Unsupported {
            reason: "no platform HTTP provider is installed".to_string(),
        })
    }

    fn generate(
        &self,
        _prompt_json: String,
        context: Arc<LocalGenerationContext>,
    ) -> Result<(), LocalLlmError> {
        context.emit_error("no local LLM provider is installed".to_string());
        Err(LocalLlmError::Unsupported {
            reason: "no local LLM provider is installed".to_string(),
        })
    }

    fn unload(&self) {}

    fn loaded_model_path(&self) -> Option<String> {
        None
    }

    fn device_capability(&self) -> DeviceCapability {
        DeviceCapability::default()
    }
}

static PLATFORM_LOCAL_PROVIDER: Lazy<RwLock<Option<Arc<dyn LocalLlmProvider>>>> =
    Lazy::new(|| RwLock::new(None));

pub fn set_platform_local_provider(provider: Arc<dyn LocalLlmProvider>) {
    if let Ok(mut slot) = PLATFORM_LOCAL_PROVIDER.write() {
        *slot = Some(provider);
    }
}

pub fn platform_http_request(
    request: PlatformHttpRequest,
) -> Result<PlatformHttpResponse, LocalLlmError> {
    let provider = PLATFORM_LOCAL_PROVIDER
        .read()
        .ok()
        .and_then(|slot| slot.clone())
        .ok_or_else(|| LocalLlmError::Unsupported {
            reason: "no platform HTTP provider is installed".to_string(),
        })?;
    provider.platform_http_request(request)
}

/// Spawn a local on-device streaming task.
///
/// Loading and generation both run inside `spawn_blocking` so GGUF mmap/load work
/// cannot stall the actor runtime. The caller still owns stream lifecycle through
/// the returned cancellation token.
pub fn spawn_local_streaming_task(
    runtime: &tokio::runtime::Runtime,
    local_llm_provider: Arc<dyn LocalLlmProvider>,
    data_dir: String,
    backend: &super::backend::BackendConfig,
    model: &str,
    messages: Vec<super::streaming::ChatMessage>,
    core_tx: flume::Sender<crate::CoreMsg>,
    semaphore: Option<Arc<tokio::sync::Semaphore>>,
) -> CancellationToken {
    let cancel_token = CancellationToken::new();
    let token_for_task = cancel_token.clone();
    let backend_id = backend.id.clone();
    let model_id = model.to_string();

    runtime.spawn(async move {
        let _permit = if let Some(sem) = semaphore {
            match sem.acquire_owned().await {
                Ok(permit) => Some(permit),
                Err(_) => {
                    let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                        InternalEvent::StreamError {
                            error: LlmError::NetworkError {
                                reason: "local concurrency limiter closed".to_string(),
                            },
                        },
                    )));
                    return;
                }
            }
        } else {
            None
        };

        let blocking_core_tx = core_tx.clone();
        let blocking_token = token_for_task.clone();
        let join = tokio::task::spawn_blocking(move || {
            run_local_generation_blocking(
                local_llm_provider,
                data_dir,
                backend_id,
                model_id,
                messages,
                blocking_core_tx,
                blocking_token,
            )
        })
        .await;

        match join {
            Ok(()) => {}
            Err(error) => {
                let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                    InternalEvent::StreamError {
                        error: LlmError::NetworkError {
                            reason: format!("local generation task failed: {error}"),
                        },
                    },
                )));
            }
        }
    });

    cancel_token
}

#[cfg(test)]
pub(crate) fn spawn_local_streaming_task_for_preset(
    runtime: &tokio::runtime::Runtime,
    local_llm_provider: Arc<dyn LocalLlmProvider>,
    data_dir: String,
    backend: &super::backend::BackendConfig,
    model: &str,
    preset: LocalModelPreset,
    messages: Vec<super::streaming::ChatMessage>,
    core_tx: flume::Sender<crate::CoreMsg>,
) -> CancellationToken {
    let cancel_token = CancellationToken::new();
    let token_for_task = cancel_token.clone();
    let backend_id = backend.id.clone();
    let model_id = model.to_string();

    runtime.spawn(async move {
        let blocking_token = token_for_task.clone();
        let join = tokio::task::spawn_blocking(move || {
            run_local_generation_with_preset_blocking(
                local_llm_provider,
                data_dir,
                backend_id,
                model_id,
                preset,
                messages,
                core_tx,
                blocking_token,
            )
        })
        .await;

        if let Err(error) = join {
            log::warn!("[local-model] test local generation task failed: {error}");
        }
    });

    cancel_token
}

fn run_local_generation_blocking(
    provider: Arc<dyn LocalLlmProvider>,
    data_dir: String,
    backend_id: String,
    model_id: String,
    messages: Vec<super::streaming::ChatMessage>,
    core_tx: flume::Sender<crate::CoreMsg>,
    cancel_token: CancellationToken,
) {
    let Some(preset) = find_local_model(&model_id) else {
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            InternalEvent::StreamError {
                error: LlmError::ModelNotFound {
                    model_id: model_id.clone(),
                },
            },
        )));
        return;
    };

    run_local_generation_with_preset_blocking(
        provider,
        data_dir,
        backend_id,
        model_id,
        preset,
        messages,
        core_tx,
        cancel_token,
    );
}

fn run_local_generation_with_preset_blocking(
    provider: Arc<dyn LocalLlmProvider>,
    data_dir: String,
    backend_id: String,
    model_id: String,
    preset: LocalModelPreset,
    messages: Vec<super::streaming::ChatMessage>,
    core_tx: flume::Sender<crate::CoreMsg>,
    cancel_token: CancellationToken,
) {
    let path = local_model_path(&data_dir, &preset);
    if !path.is_file() {
        let err = LocalLlmError::ModelMissing {
            path: path.to_string_lossy().to_string(),
        };
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            InternalEvent::StreamError {
                error: err.into_llm_error(),
            },
        )));
        return;
    }

    match local_model_verified(&data_dir, &preset) {
        Ok(true) => {}
        Ok(false) => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamError {
                    error: LlmError::NetworkError {
                        reason: format!(
                            "local model failed integrity verification: {}",
                            preset.name
                        ),
                    },
                },
            )));
            return;
        }
        Err(error) => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamError {
                    error: LlmError::NetworkError {
                        reason: format!("failed to verify local model before loading: {error}"),
                    },
                },
            )));
            return;
        }
    }

    let path_string = path.to_string_lossy().to_string();
    if provider.loaded_model_path().as_deref() != Some(path_string.as_str()) {
        if provider.loaded_model_path().is_some() {
            provider.unload();
        }
        if let Err(error) = provider.load_model(path_string.clone()) {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamError {
                    error: error.into_llm_error(),
                },
            )));
            return;
        }
    }

    if cancel_token.is_cancelled() {
        let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
            InternalEvent::StreamCancelled,
        )));
        return;
    }

    let prompt_json = prompt_json_for_local(&backend_id, &model_id, &messages);
    let context = LocalGenerationContext::new(core_tx.clone(), cancel_token.clone());

    match provider.generate(prompt_json, context.clone()) {
        Ok(()) if cancel_token.is_cancelled() => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamCancelled,
            )));
        }
        Ok(()) if !context.error_sent() => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamDone,
            )));
        }
        Ok(()) => {}
        Err(LocalLlmError::Cancelled) => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamCancelled,
            )));
        }
        Err(error) if !context.error_sent() => {
            let _ = core_tx.send(crate::CoreMsg::InternalEvent(Box::new(
                InternalEvent::StreamError {
                    error: error.into_llm_error(),
                },
            )));
        }
        Err(_) => {}
    }
}

fn prompt_json_for_local(
    backend_id: &str,
    model_id: &str,
    messages: &[super::streaming::ChatMessage],
) -> String {
    let messages_json: Vec<serde_json::Value> = messages
        .iter()
        .map(|message| {
            let role = match message.role {
                super::streaming::ChatRole::System => "system",
                super::streaming::ChatRole::User => "user",
                super::streaming::ChatRole::Assistant => "assistant",
            };
            serde_json::json!({
                "role": role,
                "content": message.content,
            })
        })
        .collect();

    serde_json::json!({
        "backend_id": backend_id,
        "model": model_id,
        "messages": messages_json,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_provider_reports_unavailable_and_emits_error() {
        let provider = NullLocalLlmProvider;
        let capability = provider.device_capability();
        assert!(matches!(
            capability.status,
            LocalLlmCapabilityStatus::Unknown | LocalLlmCapabilityStatus::RuntimeNotPackaged
        ));
        assert!(provider.load_model("/tmp/model.gguf".to_string()).is_err());
    }
}
