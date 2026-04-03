/// TEE type for a backend provider -- UniFFI-exported for UI attestation badges
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum TeeType {
    IntelTdx,
    NvidiaH100Cc,
    AmdSevSnp,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Tinfoil,
    Ppq,
    Custom,
}

/// Health status for a backend -- UniFFI-exported for UI display.
///
/// Maps from the internal HealthState enum in router.rs to a simpler UI-facing enum.
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum HealthStatus {
    /// Backend is responding normally.
    Healthy,
    /// Backend recently recovered from failures -- display with caution indicator.
    Degraded,
    /// Backend is in exponential backoff -- requests will not be routed here.
    Failed,
    /// Health is unknown (router not yet integrated or backend not yet checked).
    Unknown,
}

/// Full backend configuration -- internal to Rust core, NEVER crosses FFI.
/// Contains api_key which must not be exposed to native UI layers.
#[derive(Clone, Debug)]
pub struct BackendConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub tee_type: TeeType,
    /// Maximum concurrent requests to this backend (enforced via Semaphore in Plan 02).
    /// Loaded from the backends SQLite table. Default: 5.
    pub max_concurrent_requests: u32,
    /// Whether this backend supports function calling (tool use).
    /// Persisted in `backends.supports_tool_use` column (Phase 17, CONF-01).
    /// Default: true (D-01).
    pub supports_tool_use: bool,
}

/// Display-safe summary for native UI rendering -- UniFFI-exported.
/// No secrets (api_key omitted), only display fields.
#[derive(uniffi::Record, Clone, Debug)]
pub struct BackendSummary {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
    pub tee_type: TeeType,
    pub is_active: bool,
    pub health_status: HealthStatus,
    /// Whether this backend supports function calling (tool use).
    ///
    /// Phase 9 (D-10): Used by the agent executor to select a capable backend.
    /// Tinfoil supports OpenAI-compatible function calling.
    /// Hardcoded to true for v1 known backends; defaults to false for custom backends.
    pub supports_tool_use: bool,
    /// Whether the user has stored an API key for this backend.
    ///
    /// Backends can be seeded into the DB by migrations (e.g. PPQ.AI via MIGRATION_V10)
    /// without the user having entered a key. The UI uses this field to distinguish
    /// "seeded but not yet configured" from "user actively enabled" so it can show the
    /// API key input form for unconfigured seeded backends.
    pub has_api_key: bool,
}

impl BackendConfig {
    /// Convert to FFI-safe summary for UI consumption.
    pub fn to_summary(&self, is_active: bool, health_status: HealthStatus) -> BackendSummary {
        BackendSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            models: self.models.clone(),
            tee_type: self.tee_type.clone(),
            is_active,
            health_status,
            supports_tool_use: self.supports_tool_use,
            has_api_key: !self.api_key.is_empty(),
        }
    }

    /// Resolve the outbound transport implementation for this backend.
    ///
    /// Backends may share the same model API shape while requiring different
    /// connection semantics (plain HTTPS, attested encrypted tunnel, local proxy).
    pub fn transport_kind(&self) -> super::transport::ProviderTransportKind {
        super::transport::ProviderTransportKind::for_backend(self)
    }

    pub fn provider_kind(&self) -> ProviderKind {
        match self.id.as_str() {
            "tinfoil" => ProviderKind::Tinfoil,
            "ppq-ai" => ProviderKind::Ppq,
            _ => ProviderKind::Custom,
        }
    }
}

/// Hardcoded Tinfoil backend for Phase 2 (Phase 4 will load from SQLite).
/// Base URL verified: https://docs.tinfoil.sh/tutorials/cline (2026-03-23)
#[allow(dead_code)]
pub fn tinfoil_backend() -> BackendConfig {
    BackendConfig {
        id: "tinfoil".into(),
        name: "Tinfoil".into(),
        base_url: "https://inference.tinfoil.sh/v1/".into(),
        api_key: std::env::var("TINFOIL_API_KEY").unwrap_or_default(),
        models: vec!["meta-llama/Llama-3.3-70B-Instruct".into()],
        tee_type: TeeType::IntelTdx,
        max_concurrent_requests: 5,
        supports_tool_use: true,
    }
}

/// A known provider preset for the Add Backend form.
/// UniFFI-exported so all platforms share the same preset data.
#[derive(uniffi::Record, Clone, Debug)]
pub struct ProviderPreset {
    /// Short identifier (e.g. "tinfoil") -- used as suggested backend id
    pub id: String,
    /// Display name (e.g. "Tinfoil")
    pub name: String,
    /// Pre-filled base URL
    pub base_url: String,
    /// Pre-filled TEE type
    pub tee_type: TeeType,
    /// Brief description for the UI (e.g. "Intel TDX + NVIDIA H100 CC")
    pub description: String,
}

/// Returns the list of known confidential inference provider presets.
/// Single source of truth -- all native UIs consume this via UniFFI.
#[uniffi::export]
pub fn known_provider_presets() -> Vec<ProviderPreset> {
    vec![
        ProviderPreset {
            id: "tinfoil".into(),
            name: "Tinfoil".into(),
            base_url: "https://inference.tinfoil.sh/v1/".into(),
            tee_type: TeeType::IntelTdx,
            description: "Intel TDX + NVIDIA H100 CC".into(),
        },
        ProviderPreset {
            id: "ppq-ai".into(),
            name: "PPQ.AI".into(),
            base_url: "https://api.ppq.ai/private/v1/".into(),
            tee_type: TeeType::AmdSevSnp,
            description: "AMD SEV-SNP \u{00b7} Private TEE models".into(),
        },
    ]
}
