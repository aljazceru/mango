use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Built-in downloadable local model entry.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct LocalModelPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub quantization: String,
    pub min_ram_bytes: u64,
    pub chat_template: String,
}

/// UI-safe local model status, derived from catalog + verified file state.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct LocalModelSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub quantization: String,
    pub size_bytes: u64,
    pub min_ram_bytes: u64,
    pub downloaded: bool,
    pub verified: bool,
    pub path: Option<String>,
    pub backend_id: Option<String>,
}

/// Download progress for a single local model.
#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct LocalModelDownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    /// One of: "downloading", "verifying", "complete", "failed".
    pub stage: String,
}

const LOCAL_MODEL_DIR: &str = "local-models";
const VERIFIED_MARKER_SUFFIX: &str = ".verified.json";
// Android reserves part of physically installed RAM before ActivityManager reports
// totalMem. The tested 8 GB Pixel reports 7,678,017,536 bytes, so an 8,000,000,000
// byte catalog gate incorrectly rejects models that fit and run successfully.
const EIGHT_GB_CLASS_REPORTED_RAM_FLOOR: u64 = 7_000_000_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct VerifiedModelMarker {
    model_id: String,
    filename: String,
    sha256: String,
    size_bytes: u64,
    modified_unix_secs: u64,
    modified_subsec_nanos: u32,
    verified_at_unix_secs: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ModelFileSignature {
    size_bytes: u64,
    modified_unix_secs: u64,
    modified_subsec_nanos: u32,
}

/// Built-in local model catalog shared by all platforms.
///
/// Sources: Hugging Face LFS metadata (X-Linked-ETag = content sha256,
/// X-Linked-Size = byte length), verified 2026-06-26. Qwen presets verified
/// 2026-06-23. Android uses the platform HTTPS stack for model downloads; the
/// file is installed only after the pinned SHA-256 matches. Native inference
/// reads each model's embedded `tokenizer.chat_template`; Android renders it
/// with libllama-common and iOS uses llama.cpp's recognized template renderer.
/// The `chat_template` field below is informational only.
#[uniffi::export]
pub fn local_model_catalog() -> Vec<LocalModelPreset> {
    vec![
        LocalModelPreset {
            id: "qwen2_5-1_5b-instruct-q4_0".to_string(),
            name: "Qwen2.5 1.5B Instruct".to_string(),
            description: "Default on-device chat model for capable phones.".to_string(),
            filename: "qwen2.5-1.5b-instruct-q4_0.gguf".to_string(),
            url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_0.gguf?download=true".to_string(),
            sha256: "dcd819ff094852c38faba6873d8ff0c9d51eadb2844539e52042ae5d647bbfdb".to_string(),
            size_bytes: 1_066_227_232,
            quantization: "Q4_0".to_string(),
            min_ram_bytes: 4_294_967_296,
            chat_template: "chatml".to_string(),
        },
        LocalModelPreset {
            id: "qwen2_5-0_5b-instruct-q4_0".to_string(),
            name: "Qwen2.5 0.5B Instruct".to_string(),
            description: "Small general chat model for on-device testing and offline fallback."
                .to_string(),
            filename: "Qwen2.5-0.5B-Instruct.Q4_0.gguf".to_string(),
            url: "https://huggingface.co/QuantFactory/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/Qwen2.5-0.5B-Instruct.Q4_0.gguf?download=true".to_string(),
            sha256: "0a1ec9ce531ebde1e683da2f23fd74c227212d7ecce6357ad90b492edbfff07f".to_string(),
            size_bytes: 352_154_816,
            quantization: "Q4_0".to_string(),
            min_ram_bytes: 2_147_483_648,
            chat_template: "chatml".to_string(),
        },
        LocalModelPreset {
            id: "llama3_2-3b-instruct-q4_k_m".to_string(),
            name: "Llama 3.2 3B Instruct".to_string(),
            description: "Meta's compact instruct model; strong general chat on modern phones.".to_string(),
            filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf?download=true".to_string(),
            sha256: "6c1a2b41161032677be168d354123594c0e6e67d2b9227c84f296ad037c728ff".to_string(),
            size_bytes: 2_019_377_696,
            quantization: "Q4_K_M".to_string(),
            min_ram_bytes: 6_000_000_000,
            chat_template: "llama3".to_string(),
        },
        LocalModelPreset {
            id: "gemma3-4b-it-q4_k_m".to_string(),
            name: "Gemma 3 4B Instruct".to_string(),
            description: "Google's 4B instruct model; high-quality chat where RAM permits.".to_string(),
            filename: "gemma-3-4b-it-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/unsloth/gemma-3-4b-it-GGUF/resolve/main/gemma-3-4b-it-Q4_K_M.gguf?download=true".to_string(),
            sha256: "04a43a22e8d2003deda5acc262f68ec1005fa76c735a9962a8c77042a74a7d19".to_string(),
            size_bytes: 2_489_894_016,
            quantization: "Q4_K_M".to_string(),
            min_ram_bytes: EIGHT_GB_CLASS_REPORTED_RAM_FLOOR,
            chat_template: "gemma".to_string(),
        },
        LocalModelPreset {
            id: "phi3_5-mini-instruct-q4_k_m".to_string(),
            name: "Phi-3.5 mini Instruct".to_string(),
            description: "Microsoft's 3.8B model; compact reasoning and code on capable devices.".to_string(),
            filename: "Phi-3.5-mini-instruct-Q4_K_M.gguf".to_string(),
            url: "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf?download=true".to_string(),
            sha256: "e4165e3a71af97f1b4820da61079826d8752a2088e313af0c7d346796c38eff5".to_string(),
            size_bytes: 2_393_232_672,
            quantization: "Q4_K_M".to_string(),
            min_ram_bytes: EIGHT_GB_CLASS_REPORTED_RAM_FLOOR,
            chat_template: "phi3".to_string(),
        },
    ]
}

pub fn local_model_dir(data_dir: &str) -> PathBuf {
    if data_dir.is_empty() || data_dir == ":memory:" {
        std::env::temp_dir().join("mango-local-models")
    } else {
        Path::new(data_dir).join(LOCAL_MODEL_DIR)
    }
}

pub fn local_model_path(data_dir: &str, preset: &LocalModelPreset) -> PathBuf {
    local_model_dir(data_dir).join(&preset.filename)
}

pub fn verified_marker_path(data_dir: &str, preset: &LocalModelPreset) -> PathBuf {
    local_model_dir(data_dir).join(format!("{}{}", preset.filename, VERIFIED_MARKER_SUFFIX))
}

pub fn find_local_model(model_id: &str) -> Option<LocalModelPreset> {
    local_model_catalog()
        .into_iter()
        .find(|preset| preset.id == model_id)
}

pub fn local_backend_id(model_id: &str) -> String {
    format!("local-{model_id}")
}

pub fn local_backend_base_url(model_id: &str) -> String {
    format!("local://{model_id}")
}

pub fn local_model_id_from_backend_id(backend_id: &str) -> Option<String> {
    backend_id.strip_prefix("local-").map(ToOwned::to_owned)
}

pub fn is_local_backend_id(backend_id: &str) -> bool {
    backend_id.starts_with("local-")
}

pub fn is_local_base_url(base_url: &str) -> bool {
    base_url.trim().starts_with("local://")
}

pub fn verify_file_sha256(path: &Path, expected_sha256: &str) -> std::io::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    Ok(actual.eq_ignore_ascii_case(expected_sha256))
}

fn file_signature(path: &Path) -> std::io::Result<ModelFileSignature> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0));
    Ok(ModelFileSignature {
        size_bytes: metadata.len(),
        modified_unix_secs: duration.as_secs(),
        modified_subsec_nanos: duration.subsec_nanos(),
    })
}

fn marker_matches(
    marker: &VerifiedModelMarker,
    preset: &LocalModelPreset,
    signature: ModelFileSignature,
) -> bool {
    marker.model_id == preset.id
        && marker.filename == preset.filename
        && marker.sha256.eq_ignore_ascii_case(&preset.sha256)
        && marker.size_bytes == signature.size_bytes
        && marker.size_bytes == preset.size_bytes
        && marker.modified_unix_secs == signature.modified_unix_secs
        && marker.modified_subsec_nanos == signature.modified_subsec_nanos
}

fn read_verified_marker(
    data_dir: &str,
    preset: &LocalModelPreset,
) -> std::io::Result<VerifiedModelMarker> {
    let bytes = std::fs::read(verified_marker_path(data_dir, preset))?;
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}

pub fn write_verified_marker(data_dir: &str, preset: &LocalModelPreset) -> std::io::Result<()> {
    let path = local_model_path(data_dir, preset);
    let signature = file_signature(&path)?;
    let verified_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    let marker = VerifiedModelMarker {
        model_id: preset.id.clone(),
        filename: preset.filename.clone(),
        sha256: preset.sha256.clone(),
        size_bytes: signature.size_bytes,
        modified_unix_secs: signature.modified_unix_secs,
        modified_subsec_nanos: signature.modified_subsec_nanos,
        verified_at_unix_secs: verified_at,
    };
    let marker_path = verified_marker_path(data_dir, preset);
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(&marker).map_err(std::io::Error::other)?;
    std::fs::write(marker_path, bytes)
}

pub fn remove_verified_marker(data_dir: &str, preset: &LocalModelPreset) -> std::io::Result<()> {
    match std::fs::remove_file(verified_marker_path(data_dir, preset)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub fn local_model_verified(data_dir: &str, preset: &LocalModelPreset) -> std::io::Result<bool> {
    let path = local_model_path(data_dir, preset);
    let signature = file_signature(&path)?;
    if let Ok(marker) = read_verified_marker(data_dir, preset) {
        if marker_matches(&marker, preset, signature) {
            return Ok(true);
        }
    }

    let verified = verify_file_sha256(&path, &preset.sha256)?;
    if verified {
        if let Err(error) = write_verified_marker(data_dir, preset) {
            log::warn!(
                "[local-model] failed to persist verification marker for {}: {error}",
                preset.id
            );
        }
    } else {
        let _ = remove_verified_marker(data_dir, preset);
    }
    Ok(verified)
}

pub fn local_model_summaries(data_dir: &str) -> Vec<LocalModelSummary> {
    local_model_catalog()
        .into_iter()
        .map(|preset| {
            let path = local_model_path(data_dir, &preset);
            let downloaded = path.is_file();
            let verified = downloaded && local_model_verified(data_dir, &preset).unwrap_or(false);
            LocalModelSummary {
                id: preset.id.clone(),
                name: preset.name.clone(),
                description: preset.description.clone(),
                quantization: preset.quantization.clone(),
                size_bytes: preset.size_bytes,
                min_ram_bytes: preset.min_ram_bytes,
                downloaded,
                verified,
                path: if downloaded {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                },
                backend_id: if verified {
                    Some(local_backend_id(&preset.id))
                } else {
                    None
                },
            }
        })
        .collect()
}
