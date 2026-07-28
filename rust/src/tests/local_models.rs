use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::llm::local::{
    DeviceCapability, LocalGenerationContext, LocalLlmError, LocalLlmProvider,
    LocalModelDownloadContext, PlatformHttpRequest, PlatformHttpResponse,
};
use crate::llm::local_models::{
    local_backend_base_url, local_backend_id, local_model_catalog, local_model_path,
    local_model_verified, remove_verified_marker, verified_marker_path, LocalModelPreset,
};
use crate::llm::streaming::{ChatMessage, ChatRole, InternalEvent};
use crate::llm::{BackendConfig, ProviderTransportKind, TeeType};
use crate::persistence::{self, Database};
use crate::CoreMsg;

fn local_backend(model_id: &str) -> BackendConfig {
    BackendConfig {
        id: local_backend_id(model_id),
        name: "Local test".to_string(),
        base_url: local_backend_base_url(model_id),
        api_key: String::new(),
        models: vec![model_id.to_string()],
        tee_type: TeeType::Unknown,
        max_concurrent_requests: 1,
        supports_tool_use: false,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn tiny_local_preset(filename: &str, bytes: &[u8]) -> LocalModelPreset {
    LocalModelPreset {
        id: format!("tiny-{filename}"),
        name: "Tiny local test model".to_string(),
        description: "Tiny local model fixture".to_string(),
        filename: filename.to_string(),
        url: "https://example.invalid/tiny.gguf".to_string(),
        sha256: sha256_hex(bytes),
        size_bytes: bytes.len() as u64,
        quantization: "TEST".to_string(),
        min_ram_bytes: 1,
        chat_template: "chatml".to_string(),
    }
}

fn write_tiny_model(temp: &tempfile::TempDir, preset: &LocalModelPreset, bytes: &[u8]) {
    let model_path = local_model_path(temp.path().to_str().unwrap(), preset);
    std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    std::fs::write(&model_path, bytes).unwrap();
}

#[derive(Default)]
struct FakeLocalProvider {
    loaded: Mutex<Option<String>>,
    loads: Mutex<Vec<String>>,
    fail_load: bool,
    fail_generate: bool,
    wait_for_cancel: bool,
}

impl LocalLlmProvider for FakeLocalProvider {
    fn load_model(&self, model_path: String) -> Result<(), LocalLlmError> {
        self.loads.lock().unwrap().push(model_path.clone());
        if self.fail_load {
            return Err(LocalLlmError::LoadFailed {
                reason: "boom".to_string(),
            });
        }
        *self.loaded.lock().unwrap() = Some(model_path);
        Ok(())
    }

    fn download_model_file(
        &self,
        _url: String,
        _destination_path: String,
        _context: Arc<LocalModelDownloadContext>,
    ) -> Result<(), LocalLlmError> {
        Err(LocalLlmError::Unsupported {
            reason: "fake provider does not download files".to_string(),
        })
    }

    fn platform_http_request(
        &self,
        _request: PlatformHttpRequest,
    ) -> Result<PlatformHttpResponse, LocalLlmError> {
        Err(LocalLlmError::Unsupported {
            reason: "fake provider does not make platform HTTP requests".to_string(),
        })
    }

    fn generate(
        &self,
        prompt_json: String,
        context: Arc<LocalGenerationContext>,
    ) -> Result<(), LocalLlmError> {
        assert!(prompt_json.contains("\"role\":\"user\""));
        if self.fail_generate {
            return Err(LocalLlmError::GenerationFailed {
                reason: "generate boom".to_string(),
            });
        }
        if self.wait_for_cancel {
            context.emit_token("started".to_string());
            while !context.is_cancelled() {
                std::thread::sleep(Duration::from_millis(10));
            }
            return Err(LocalLlmError::Cancelled);
        }
        context.emit_token("local ".to_string());
        context.emit_token("ok".to_string());
        Ok(())
    }

    fn unload(&self) {
        *self.loaded.lock().unwrap() = None;
    }

    fn loaded_model_path(&self) -> Option<String> {
        self.loaded.lock().unwrap().clone()
    }

    fn device_capability(&self) -> DeviceCapability {
        DeviceCapability {
            abi: "test".to_string(),
            total_ram_bytes: 8 * 1024 * 1024 * 1024,
            available_ram_bytes: 4 * 1024 * 1024 * 1024,
            supports_mmap: true,
            status: crate::llm::local::LocalLlmCapabilityStatus::Supported,
            reason_code: "supported".to_string(),
            reason: None,
            available_storage_bytes: 20 * 1024 * 1024 * 1024,
        }
    }
}

#[test]
fn local_transport_is_selected_for_local_scheme() {
    let preset = local_model_catalog().remove(0);
    let backend = local_backend(&preset.id);

    assert_eq!(
        backend.transport_kind(),
        ProviderTransportKind::LocalOnDevice
    );
    assert!(backend.transport_kind().openai_api_base(&backend).is_err());
    assert!(backend.transport_kind().model_list_url(&backend).is_err());
}

#[test]
fn catalog_contains_qwen_2_5_1_5b_dod_model() {
    let catalog = local_model_catalog();
    let preset = catalog
        .iter()
        .find(|preset| preset.id == "qwen2_5-1_5b-instruct-q4_0")
        .expect("phase 37/38 DoD requires Qwen2.5 1.5B in the built-in catalog");

    assert_eq!(catalog.first(), Some(preset));
    assert_eq!(preset.name, "Qwen2.5 1.5B Instruct");
    assert_eq!(preset.filename, "qwen2.5-1.5b-instruct-q4_0.gguf");
    assert_eq!(
        preset.url,
        "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_0.gguf?download=true"
    );
    assert_eq!(
        preset.sha256,
        "dcd819ff094852c38faba6873d8ff0c9d51eadb2844539e52042ae5d647bbfdb"
    );
    assert_eq!(preset.size_bytes, 1_066_227_232);
    assert_eq!(preset.quantization, "Q4_0");
    assert_eq!(preset.chat_template, "chatml");
    assert!(preset.min_ram_bytes <= 4_294_967_296);
    assert!(
        preset.size_bytes <= 1_500_000_000,
        "AndroidLocalLlmProvider caps model files at 1.5 GB"
    );
}

#[test]
fn eight_gb_class_models_allow_os_reserved_ram() {
    let catalog = local_model_catalog();
    for model_id in ["gemma3-4b-it-q4_k_m", "phi3_5-mini-instruct-q4_k_m"] {
        let preset = catalog
            .iter()
            .find(|preset| preset.id == model_id)
            .unwrap_or_else(|| panic!("missing local model preset {model_id}"));

        assert_eq!(
            preset.min_ram_bytes, 7_000_000_000,
            "8 GB-class phones report less RAM after hardware/OS reservations"
        );
    }
}

#[test]
fn local_model_verified_creates_marker_after_hash_match() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"tiny gguf bytes";
    let preset = tiny_local_preset("tiny.gguf", bytes);
    let model_path = local_model_path(temp.path().to_str().unwrap(), &preset);
    std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    std::fs::write(&model_path, bytes).unwrap();

    assert!(local_model_verified(temp.path().to_str().unwrap(), &preset).unwrap());
    let marker_path = verified_marker_path(temp.path().to_str().unwrap(), &preset);
    let marker = std::fs::read_to_string(marker_path).unwrap();
    assert!(marker.contains(&preset.id));
    assert!(marker.contains(&preset.sha256));
}

#[test]
fn local_model_verified_recovers_from_stale_marker() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"tiny gguf bytes";
    let preset = tiny_local_preset("tiny-stale.gguf", bytes);
    let model_path = local_model_path(temp.path().to_str().unwrap(), &preset);
    std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    std::fs::write(&model_path, bytes).unwrap();
    let marker_path = verified_marker_path(temp.path().to_str().unwrap(), &preset);
    std::fs::write(&marker_path, br#"{"model_id":"wrong"}"#).unwrap();

    assert!(local_model_verified(temp.path().to_str().unwrap(), &preset).unwrap());
    let marker = std::fs::read_to_string(marker_path).unwrap();
    assert!(marker.contains(&preset.id));
    assert!(marker.contains(&preset.sha256));
}

#[test]
fn local_model_verified_removes_marker_after_hash_mismatch() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"tiny gguf bytes";
    let preset = tiny_local_preset("tiny-corrupt.gguf", bytes);
    let model_path = local_model_path(temp.path().to_str().unwrap(), &preset);
    std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
    std::fs::write(&model_path, bytes).unwrap();

    assert!(local_model_verified(temp.path().to_str().unwrap(), &preset).unwrap());
    let marker_path = verified_marker_path(temp.path().to_str().unwrap(), &preset);
    assert!(marker_path.is_file());

    std::fs::write(&model_path, b"corrupt tiny gguf bytes").unwrap();
    assert!(!local_model_verified(temp.path().to_str().unwrap(), &preset).unwrap());
    assert!(!marker_path.exists());
}

#[test]
fn remove_verified_marker_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"tiny gguf bytes";
    let preset = tiny_local_preset("tiny-remove.gguf", bytes);

    remove_verified_marker(temp.path().to_str().unwrap(), &preset).unwrap();
    remove_verified_marker(temp.path().to_str().unwrap(), &preset).unwrap();
}

#[test]
fn insert_backend_persists_local_capabilities() {
    let db = Database::open(":memory:").unwrap();
    let row = persistence::BackendRow {
        id: "local-test".to_string(),
        name: "Local Test".to_string(),
        base_url: "local://test".to_string(),
        model_list: "[\"test\"]".to_string(),
        tee_type: "Unknown".to_string(),
        display_order: 99,
        is_active: 0,
        created_at: 123,
        max_concurrent_requests: 1,
        supports_tool_use: false,
    };

    persistence::queries::insert_backend(db.conn(), &row).unwrap();
    let loaded = persistence::queries::list_backends(db.conn())
        .unwrap()
        .into_iter()
        .find(|b| b.id == "local-test")
        .unwrap();

    assert_eq!(loaded.max_concurrent_requests, 1);
    assert!(!loaded.supports_tool_use);
}

#[test]
fn upsert_local_backend_reasserts_capabilities_for_stale_row() {
    let db = Database::open(":memory:").unwrap();
    let stale_row = persistence::BackendRow {
        id: "local-stale".to_string(),
        name: "Local Stale".to_string(),
        base_url: "local://stale".to_string(),
        model_list: "[\"stale\"]".to_string(),
        tee_type: "Unknown".to_string(),
        display_order: 99,
        is_active: 0,
        created_at: 123,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    };

    persistence::queries::insert_backend(db.conn(), &stale_row).unwrap();
    let stale = persistence::queries::list_backends(db.conn())
        .unwrap()
        .into_iter()
        .find(|backend| backend.id == "local-stale")
        .unwrap();
    assert_eq!(stale.max_concurrent_requests, 5);
    assert!(stale.supports_tool_use);

    persistence::queries::upsert_local_backend(db.conn(), &stale_row).unwrap();
    let corrected = persistence::queries::list_backends(db.conn())
        .unwrap()
        .into_iter()
        .find(|backend| backend.id == "local-stale")
        .unwrap();

    assert_eq!(corrected.max_concurrent_requests, 1);
    assert!(!corrected.supports_tool_use);
}

#[test]
fn local_streaming_loads_model_and_emits_chunks() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"test gguf placeholder";
    let preset = tiny_local_preset("stream-ok.gguf", bytes);
    write_tiny_model(&temp, &preset, bytes);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (tx, rx) = flume::unbounded();
    let provider = Arc::new(FakeLocalProvider::default());
    let backend = local_backend(&preset.id);

    let _cancel = crate::llm::local::spawn_local_streaming_task_for_preset(
        &runtime,
        provider.clone(),
        temp.path().to_string_lossy().to_string(),
        &backend,
        &preset.id,
        preset.clone(),
        vec![ChatMessage {
            role: ChatRole::User,
            content: "hello".to_string(),
        }],
        tx,
    );

    let mut text = String::new();
    for _ in 0..4 {
        let msg = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if let CoreMsg::InternalEvent(event) = msg {
            match *event {
                InternalEvent::StreamChunk { token } => text.push_str(&token),
                InternalEvent::StreamDone => break,
                InternalEvent::StreamError { error } => panic!("unexpected error: {error}"),
                _ => {}
            }
        }
    }

    assert_eq!(text, "local ok");
    assert_eq!(provider.loads.lock().unwrap().len(), 1);
}

#[test]
fn local_streaming_rejects_tampered_model_before_native_load() {
    let temp = tempfile::tempdir().unwrap();
    let expected = b"expected gguf bytes";
    let preset = tiny_local_preset("stream-tampered.gguf", expected);
    write_tiny_model(&temp, &preset, b"tampered gguf bytes");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (tx, rx) = flume::unbounded();
    let provider = Arc::new(FakeLocalProvider::default());
    let backend = local_backend(&preset.id);

    let _cancel = crate::llm::local::spawn_local_streaming_task_for_preset(
        &runtime,
        provider.clone(),
        temp.path().to_string_lossy().to_string(),
        &backend,
        &preset.id,
        preset.clone(),
        vec![ChatMessage {
            role: ChatRole::User,
            content: "hello".to_string(),
        }],
        tx,
    );

    let msg = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    match msg {
        CoreMsg::InternalEvent(event) => match *event {
            InternalEvent::StreamError { error } => {
                assert!(
                    error.to_string().contains("integrity verification"),
                    "expected integrity error, got {error}"
                );
            }
            other => panic!("unexpected event: {other:?}"),
        },
        _ => panic!("unexpected message"),
    }
    assert!(
        provider.loads.lock().unwrap().is_empty(),
        "tampered model must not reach native load_model"
    );
}

#[test]
fn local_streaming_reports_load_failure_without_generating() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"load failure gguf placeholder";
    let preset = tiny_local_preset("stream-load-fail.gguf", bytes);
    write_tiny_model(&temp, &preset, bytes);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (tx, rx) = flume::unbounded();
    let provider = Arc::new(FakeLocalProvider {
        fail_load: true,
        ..FakeLocalProvider::default()
    });
    let backend = local_backend(&preset.id);

    let _cancel = crate::llm::local::spawn_local_streaming_task_for_preset(
        &runtime,
        provider,
        temp.path().to_string_lossy().to_string(),
        &backend,
        &preset.id,
        preset.clone(),
        vec![ChatMessage {
            role: ChatRole::User,
            content: "hello".to_string(),
        }],
        tx,
    );

    let msg = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    match msg {
        CoreMsg::InternalEvent(event) => match *event {
            InternalEvent::StreamError { error } => {
                assert!(error.to_string().contains("boom"));
            }
            other => panic!("unexpected event: {other:?}"),
        },
        _ => panic!("unexpected message"),
    }
}

#[test]
fn local_streaming_reports_cancelled_when_token_is_cancelled() {
    let temp = tempfile::tempdir().unwrap();
    let bytes = b"cancel gguf placeholder";
    let preset = tiny_local_preset("stream-cancel.gguf", bytes);
    write_tiny_model(&temp, &preset, bytes);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (tx, rx) = flume::unbounded();
    let provider = Arc::new(FakeLocalProvider {
        wait_for_cancel: true,
        ..FakeLocalProvider::default()
    });
    let backend = local_backend(&preset.id);

    let cancel = crate::llm::local::spawn_local_streaming_task_for_preset(
        &runtime,
        provider,
        temp.path().to_string_lossy().to_string(),
        &backend,
        &preset.id,
        preset.clone(),
        vec![ChatMessage {
            role: ChatRole::User,
            content: "write until cancelled".to_string(),
        }],
        tx,
    );

    let msg = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    match msg {
        CoreMsg::InternalEvent(event) => match *event {
            InternalEvent::StreamChunk { token } => assert_eq!(token, "started"),
            other => panic!("unexpected first event: {other:?}"),
        },
        _ => panic!("unexpected message"),
    }

    cancel.cancel();

    for _ in 0..10 {
        let msg = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if let CoreMsg::InternalEvent(event) = msg {
            if matches!(*event, InternalEvent::StreamCancelled) {
                return;
            }
        }
    }

    panic!("local streaming task did not emit StreamCancelled");
}
