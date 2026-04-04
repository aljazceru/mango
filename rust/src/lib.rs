use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use flume::{Receiver, Sender};
use tokio_util::sync::CancellationToken;

pub mod agent;
mod attestation;
pub mod embedding;
mod llm;
pub mod memory;
mod net;
mod persistence;
pub mod rag;

pub use attestation::{AttestationError, AttestationStatus};
pub use embedding::{EmbeddingProvider, NullEmbeddingProvider};

/// Embedding provider operational status (Phase 15 / SAFE-03).
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum EmbeddingStatus {
    /// Embedding provider loaded and operational. Semantic RAG is available.
    Active,
    /// Embedding provider failed to initialize. Fell back to NullEmbeddingProvider.
    /// RAG returns zero-vector context; chat still works but search is non-semantic.
    Degraded,
    /// No embedding provider was supplied (e.g. mobile platform without native impl).
    Unavailable,
}
pub use llm::known_provider_presets;
pub use llm::{BackendSummary, HealthStatus, LlmError, ProviderPreset, TeeType};
pub use persistence::PersistenceError;

uniffi::setup_scaffolding!();

// ── State ────────────────────────────────────────────────────────────────────

/// Per-backend attestation status entry for the UI (crosses UniFFI boundary).
///
/// Per D-11: AppState carries Vec<AttestationStatusEntry> so the UI can render
/// a trust badge for each backend without the raw report blob.
#[derive(uniffi::Record, Clone, Debug)]
pub struct AttestationStatusEntry {
    pub backend_id: String,
    pub status: AttestationStatus,
}

/// Lightweight conversation summary for the UI conversation list.
///
/// Phase 4: loaded from SQLite on startup and carried in AppState so the UI
/// can render the conversation list without additional queries.
/// Phase 5 will expand this with full message loading per conversation.
#[derive(uniffi::Record, Clone, Debug)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub model_id: String,
    pub backend_id: String,
    pub updated_at: i64,
    /// Per-conversation system prompt ("Instructions"), if set.
    /// None means no per-conversation override; the global fallback applies at inference time.
    pub system_prompt: Option<String>,
}

/// Lightweight agent session summary for the UI agent session list.
///
/// Phase 4 (gap closure): loaded from SQLite on startup and carried in AppState
/// so the UI can render the agent session list. Phase 9 adds full agent orchestration
/// with step_count and elapsed_secs for richer UI display.
#[derive(uniffi::Record, Clone, Debug)]
pub struct AgentSessionSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub backend_id: String,
    pub updated_at: i64,
    // Phase 9 additions:
    /// Number of steps taken so far in this session.
    pub step_count: u32,
    /// Elapsed time in seconds (updated_at - created_at).
    pub elapsed_secs: i64,
}

/// A single agent step summary for the UI detail view (Phase 9, AGNT-06).
///
/// Carried in AppState.current_agent_steps when the user has loaded a session.
/// Maps from AgentStepRow in the persistence layer with display-safe fields.
#[derive(uniffi::Record, Clone, Debug)]
pub struct AgentStepSummary {
    pub id: String,
    pub step_number: u32,
    /// One of: "tool_call", "tool_result", "final_answer"
    pub action_type: String,
    /// Name of the tool called, if this is a tool_call step.
    pub tool_name: Option<String>,
    /// First 200 chars of the result, if available.
    pub result_snippet: Option<String>,
    /// One of: "pending", "completed", "failed"
    pub status: String,
}

/// A single message in the active conversation, for UI rendering.
///
/// Phase 5: carried in AppState.messages so the UI can render the chat thread
/// without additional queries. Maps from MessageRow with extra UI fields.
#[derive(uniffi::Record, Clone, Debug)]
pub struct UiMessage {
    pub id: String,
    /// Message role: "user", "assistant", or "system"
    pub role: String,
    pub content: String,
    pub created_at: i64,
    /// True if this message has an attached file (shown as attachment pill in UI)
    pub has_attachment: bool,
    /// Filename of the attached file, if any
    pub attachment_name: Option<String>,
    /// Number of distinct documents that contributed RAG context to this message (D-07).
    /// None if RAG was not active for this turn.
    pub rag_context_count: Option<u32>,
}

/// Info about a pending file attachment (shown in the composer bar before send).
///
/// Phase 5 (D-17): stored in AppState so the UI can render the attachment pill
/// in the input area. The actual file content lives in ActorState.pending_attachment
/// (never crosses UniFFI boundary).
#[derive(uniffi::Record, Clone, Debug)]
pub struct AttachmentInfo {
    pub filename: String,
    /// Human-readable size string, e.g. "42 KB" or "1 MB"
    pub size_display: String,
}

/// File picker result returned by the native FilePickerProvider callback.
///
/// Phase 5 (D-17): native layers implement the file picker UI and return the
/// selected file's name and content back to Rust. The actor stores content in
/// PendingAttachment (actor-internal) and the filename in AppState.pending_attachment.
#[derive(uniffi::Record, Clone, Debug)]
pub struct FilePickResult {
    pub filename: String,
    /// Full text content of the file (UTF-8)
    pub content: String,
    pub size_bytes: u64,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct AppState {
    pub rev: u64,
    pub router: Router,
    pub busy_state: BusyState,
    pub toast: Option<String>,
    // Phase 2 additions:
    /// Display-safe backend summaries for UI rendering (api_key excluded)
    pub backends: Vec<BackendSummary>,
    /// ID of the currently selected backend provider
    pub active_backend_id: Option<String>,
    /// Accumulated tokens during an active streaming response
    pub streaming_text: Option<String>,
    /// Human-readable error message from the last LLM error (per D-11)
    pub last_error: Option<String>,
    // Phase 3 additions:
    /// Per-backend attestation statuses (D-10, D-11). Vec for UniFFI compatibility.
    pub attestation_statuses: Vec<AttestationStatusEntry>,
    // Phase 4 additions:
    /// Conversation summaries loaded from SQLite on startup (PERS-02).
    /// Populated by list_conversations in FfiApp::new actor thread.
    pub conversations: Vec<ConversationSummary>,
    /// Agent session summaries loaded from SQLite on startup (PERS-05, gap closure).
    /// Populated by list_agent_sessions in FfiApp::new actor thread.
    /// Phase 9 will add full agent orchestration (write path).
    pub agent_sessions: Vec<AgentSessionSummary>,
    // Phase 5 additions:
    /// ID of the currently loaded conversation (None when on Home screen).
    pub current_conversation_id: Option<String>,
    /// Messages for the currently loaded conversation (D-05).
    /// Populated by LoadConversation and updated incrementally by SendMessage/StreamDone.
    pub messages: Vec<UiMessage>,
    /// Info about a pending file attachment in the composer bar (D-17).
    /// None when no attachment is pending. Cleared after SendMessage.
    pub pending_attachment: Option<AttachmentInfo>,
    // Phase 7 additions:
    /// Onboarding wizard transient state (D-18). Holds API key validation status,
    /// attestation demo progress, and selected backend during the wizard flow.
    pub onboarding: OnboardingState,
    /// True after CompleteOnboarding until the first message is sent (D-17).
    /// Platform UIs render a welcome placeholder: "You're all set! Send your first
    /// message to start a confidential conversation." Cleared by SendMessage.
    pub show_first_chat_placeholder: bool,
    // Phase 8 additions:
    /// All documents in the local library (LRAG-06).
    /// Loaded from SQLite on startup; updated by IngestDocument/DeleteDocument.
    pub documents: Vec<DocumentSummary>,
    /// Progress of an active document ingestion, if any.
    /// Set to Some during the extract/chunk/embed pipeline; cleared by EmbeddingComplete.
    pub ingestion_progress: Option<IngestionProgress>,
    /// Document IDs attached to the current conversation for RAG context (D-08).
    /// Loaded by LoadConversation; updated by AttachDocumentToConversation/DetachDocumentFromConversation.
    pub current_conversation_attached_docs: Vec<String>,
    // Phase 9 additions:
    /// ID of the currently viewed agent session (None when on Agents list screen).
    /// Set by LoadAgentSession; cleared by ClearAgentDetail.
    pub current_agent_session_id: Option<String>,
    /// Steps for the currently loaded agent session (AGNT-06).
    /// Populated by LoadAgentSession and updated incrementally as steps arrive.
    pub current_agent_steps: Vec<AgentStepSummary>,
    // Phase 10 additions:
    /// How often (in minutes) attestation is automatically re-run for the active backend.
    /// Stored in the settings table as "attestation_interval_minutes".
    /// Default: 15. Exposed in Advanced Settings on all platforms.
    pub attestation_interval_minutes: u32,
    /// Global default system prompt used when a conversation has no per-conversation instructions.
    /// Stored in the settings table as "global_system_prompt".
    /// None means no default instructions are set.
    pub global_system_prompt: Option<String>,
    // Phase 15 additions:
    /// Embedding provider operational status (SAFE-03).
    /// Active: real provider running. Degraded: init failed, NullEmbeddingProvider in use.
    /// Unavailable: no provider supplied by design.
    pub embedding_status: EmbeddingStatus,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            rev: 0,
            router: Router {
                current_screen: Screen::Home,
                screen_stack: vec![],
            },
            busy_state: BusyState::Idle,
            toast: None,
            backends: vec![],
            active_backend_id: None,
            streaming_text: None,
            last_error: None,
            attestation_statuses: vec![],
            conversations: vec![],
            agent_sessions: vec![],
            current_conversation_id: None,
            messages: vec![],
            pending_attachment: None,
            onboarding: OnboardingState::default(),
            show_first_chat_placeholder: false,
            documents: vec![],
            ingestion_progress: None,
            current_conversation_attached_docs: vec![],
            current_agent_session_id: None,
            current_agent_steps: vec![],
            attestation_interval_minutes: 15,
            global_system_prompt: None,
            embedding_status: EmbeddingStatus::Active,
        }
    }
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct Router {
    pub current_screen: Screen,
    pub screen_stack: Vec<Screen>,
}

/// Onboarding wizard step (per D-18).
///
/// Phase 7: defines the four steps of the onboarding wizard. Carried in
/// Screen::Onboarding { step } so the UI can render the current wizard page.
/// UniFFI-exported so all platforms share the same step enum.
#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum OnboardingStep {
    /// Welcome screen: app intro and privacy guarantee overview.
    Welcome,
    /// Backend setup: user picks/adds a confidential inference backend and API key.
    BackendSetup,
    /// Attestation demo: live TEE attestation verification shown to the user.
    AttestationDemo,
    /// Ready to chat: completion screen before navigating to the main chat UI.
    ReadyToChat,
}

/// Onboarding wizard transient state (per D-18).
///
/// Phase 7: tracks wizard progress through backend selection, API key validation,
/// and attestation demo. Carried in AppState so the UI can render validation
/// spinners and error messages without additional queries.
/// UniFFI-exported as a Record so all platforms can destructure the fields.
#[derive(uniffi::Record, Clone, Debug)]
pub struct OnboardingState {
    /// The backend ID selected or entered by the user during BackendSetup.
    pub selected_backend_id: Option<String>,
    /// Human-readable progress label during attestation (e.g. "Connecting to backend...").
    /// None when attestation is not in progress.
    pub attestation_stage: Option<String>,
    /// Result of the attestation demo. None until attestation completes.
    pub attestation_result: Option<AttestationStatus>,
    /// Human-readable TEE label for the attested backend (e.g. "Intel TDX").
    /// None until attestation completes.
    pub attestation_tee_label: Option<String>,
    /// True while the ValidateApiKey health-check is in flight.
    pub validating_api_key: bool,
    /// Error message from the last failed API key validation. Cleared on retry.
    pub api_key_error: Option<String>,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            selected_backend_id: None,
            attestation_stage: None,
            attestation_result: None,
            attestation_tee_label: None,
            validating_api_key: false,
            api_key_error: None,
        }
    }
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum Screen {
    /// Main screen -- will become conversation list in Phase 5
    Home,
    /// Placeholder for Phase 6 settings
    Settings,
    /// Placeholder for Phase 5 chat
    Chat { conversation_id: String },
    /// Onboarding wizard -- shown on first launch and when user re-triggers from settings.
    Onboarding { step: OnboardingStep },
    /// Document library -- shown when user navigates to the RAG document management screen (D-09).
    Documents,
    /// Agent session list -- shown when user navigates to the agent management screen (Phase 9).
    Agents,
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq)]
pub enum BusyState {
    /// No async work in progress
    Idle,
    /// Generic loading with descriptive message
    Loading { message: String },
    /// LLM streaming in progress; includes model name for UI display
    Streaming { model: String },
}

// ── Actions & Updates ────────────────────────────────────────────────────────

#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppAction {
    /// Push a screen onto the navigation stack
    PushScreen { screen: Screen },
    /// Pop the top screen from the navigation stack
    PopScreen,
    /// Set the busy/loading indicator
    SetBusyState { state: BusyState },
    /// Show an ephemeral toast message
    ShowToast { message: String },
    /// Clear the current toast message
    ClearToast,
    /// Proof-of-life action for testing round-trip
    Noop,
    /// Send a chat message to the active backend (per D-04, D-05)
    SendMessage { text: String },
    /// Stop the active generation (per D-06, LLMC-07)
    StopGeneration,
    /// Set the active backend by ID (per D-09)
    SetActiveBackend { backend_id: String },
    // Phase 5 additions:
    /// Create a new conversation and navigate to the Chat screen (per D-12)
    NewConversation,
    /// Load a conversation's messages into AppState.messages (per D-05)
    LoadConversation { conversation_id: String },
    /// Rename a conversation title (per D-13)
    RenameConversation { id: String, title: String },
    /// Delete a conversation and all its messages (per D-13)
    DeleteConversation { id: String },
    /// Retry: delete last assistant message and re-send the last user message (per D-07)
    RetryLastMessage,
    /// Edit a prior message: truncate history after it and re-submit (per D-08)
    EditMessage {
        message_id: String,
        new_text: String,
    },
    /// Store a pending file attachment to be sent with the next message (per D-17, D-18)
    AttachFile {
        filename: String,
        content: String,
        size_bytes: u64,
    },
    /// Clear the pending file attachment without sending
    ClearAttachment,
    /// Select a model for the current conversation (per D-06)
    SelectModel { model_id: String },
    /// Set a system prompt for the current conversation (per D-09)
    SetSystemPrompt { prompt: Option<String> },
    // Phase 6 additions: backend CRUD and settings
    /// Add a new backend, persist to SQLite, and store API key in keychain.
    AddBackend {
        name: String,
        base_url: String,
        api_key: String,
        tee_type: llm::TeeType,
        models: Vec<String>,
    },
    /// Remove a backend, clean up health state, and reassign its conversations.
    RemoveBackend { backend_id: String },
    /// Update the display_order for a backend (drag-to-reorder).
    ReorderBackend {
        backend_id: String,
        new_display_order: i64,
    },
    /// Refresh the model list for a backend (from provider discovery).
    UpdateBackendModels {
        backend_id: String,
        models: Vec<String>,
    },
    /// Persist a backend as the default and set it active for the current session.
    SetDefaultBackend { backend_id: String },
    /// Persist a model as the global default for new conversations.
    SetDefaultModel { model_id: String },
    /// Override the backend used for a specific conversation.
    OverrideConversationBackend {
        conversation_id: String,
        backend_id: String,
    },
    // Phase 7 additions: onboarding wizard actions
    /// Advance the onboarding wizard to the next step.
    NextOnboardingStep,
    /// Retreat the onboarding wizard to the previous step. No-op on Welcome.
    PreviousOnboardingStep,
    /// Store an updated API key for an existing backend in the platform keychain.
    ///
    /// Used by the onboarding wizard's BackendSetup step so the user can enter
    /// their API key and have it persisted before ValidateApiKey runs the health check.
    /// Only stores to keychain -- does not modify the SQLite backends table.
    UpdateBackendApiKey { backend_id: String, api_key: String },
    /// Validate the selected backend's API key by running a health check.
    /// Sets onboarding.validating_api_key=true; on success advances to AttestationDemo.
    ValidateApiKey { backend_id: String },
    /// Complete the onboarding wizard: persist has_completed_onboarding=true, create
    /// a conversation, navigate to Screen::Chat, and set show_first_chat_placeholder=true.
    CompleteOnboarding,
    /// Skip the onboarding wizard without adding a provider.
    ///
    /// Persists has_completed_onboarding=true and navigates to Screen::Home so the wizard
    /// is never shown again. The user can add providers later from Settings.
    SkipOnboarding,
    /// Add a backend from a known provider preset using only the API key.
    ///
    /// Looks up the preset by preset_id, uses the preset's base_url and tee_type,
    /// and inserts a backend row with id=preset_id (stable, human-readable identifier).
    /// This is the "simple enable" path from the onboarding wizard and settings provider list.
    AddBackendFromPreset { preset_id: String, api_key: String },
    // Phase 8 additions: document management and RAG context
    /// Ingest a document into the local library: extract text, chunk, embed, index (LRAG-02).
    ///
    /// The pipeline is asynchronous: text extraction and chunking happen synchronously,
    /// then embedding is dispatched via spawn_blocking (D-15), and EmbeddingComplete
    /// delivers the result back to the actor loop for final indexing.
    IngestDocument { filename: String, content: Vec<u8> },
    /// Delete a document and its chunks/vectors from the library and index (LRAG-04, D-10).
    DeleteDocument { document_id: String },
    /// Attach a library document to the active conversation for RAG context (D-08, D-11).
    AttachDocumentToConversation { document_id: String },
    /// Detach a document from the active conversation.
    DetachDocumentFromConversation { document_id: String },
    // Phase 9 additions: agent session management
    /// Launch a new autonomous agent session with the given task description (AGNT-01).
    LaunchAgentSession { task_description: String },
    /// Pause a running agent session -- stops the loop, preserves state (AGNT-06).
    PauseAgentSession { session_id: String },
    /// Resume a paused agent session -- rebuilds message history and continues (AGNT-06).
    ResumeAgentSession { session_id: String },
    /// Cancel an agent session -- marks it cancelled and clears in-flight state (AGNT-06).
    CancelAgentSession { session_id: String },
    /// Load agent steps for a session into AppState for UI display (AGNT-06).
    LoadAgentSession { session_id: String },
    /// Clear the agent detail view state (back-navigation from detail screen).
    ClearAgentDetail,
    // Phase 10 additions: periodic attestation refresh
    /// Set the periodic re-attestation interval in minutes.
    ///
    /// Persists to the settings table as "attestation_interval_minutes".
    /// Resets the background timer to use the new interval immediately.
    /// A value of 0 disables periodic re-attestation.
    SetAttestationInterval { minutes: u32 },
    /// Set the global default system prompt used when a conversation has no per-conversation instructions.
    /// None or empty string clears the setting.
    SetGlobalSystemPrompt { prompt: Option<String> },
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppUpdate {
    /// Full state snapshot delivered to UI after every action
    FullState(AppState),
}

// ── Callback interface ───────────────────────────────────────────────────────

#[uniffi::export(callback_interface)]
pub trait AppReconciler: Send + Sync + 'static {
    fn reconcile(&self, update: AppUpdate);
}

// ── Keychain interface ────────────────────────────────────────────────────────

/// Platform keychain interface. Implemented natively on iOS/Android/Desktop.
///
/// Per D-07: API keys are never written to SQLite. They are stored exclusively
/// via the platform keychain and loaded at runtime into BackendConfig.api_key.
/// This is a UniFFI callback_interface so native layers can inject their
/// platform keychain implementation (Keychain Services on iOS, Keystore on Android,
/// keyring on Desktop).
#[uniffi::export(callback_interface)]
pub trait KeychainProvider: Send + Sync + 'static {
    fn store(&self, service: String, key: String, value: String);
    fn load(&self, service: String, key: String) -> Option<String>;
    fn delete(&self, service: String, key: String);
}

/// File picker capability bridge. Implemented natively on each platform.
///
/// Per D-17: native layers implement the file picker UI (system file picker dialog
/// on desktop, document picker on iOS/Android), read the file content, and return it
/// to Rust. The actor stores the content internally as PendingAttachment and exposes
/// only display-safe AttachmentInfo (filename, size_display) in AppState.
///
/// Note: native layers do NOT call pick_file() directly and dispatch the result;
/// instead they dispatch `AppAction::AttachFile { filename, content, size_bytes }` back
/// to the Rust actor. This trait is the UniFFI callback_interface contract so Swift/Kotlin
/// can implement it as a capability bridge.
#[uniffi::export(callback_interface)]
pub trait FilePickerProvider: Send + Sync + 'static {
    fn pick_file(&self) -> Option<FilePickResult>;
}

/// Lightweight document summary for the UI document library (Phase 8, LRAG-06).
///
/// Carried in AppState.documents so the UI can render the document list without
/// additional queries. Maps from DocumentRow in the persistence layer.
#[derive(uniffi::Record, Clone, Debug)]
pub struct DocumentSummary {
    pub id: String,
    pub name: String,
    pub format: String,
    pub size_bytes: u64,
    pub ingestion_date: i64,
    pub chunk_count: u64,
}

/// Progress of an ongoing document ingestion (Phase 8).
///
/// Shown in the UI during the extract -> chunk -> embed -> index pipeline.
/// Cleared when EmbeddingComplete is handled.
#[derive(uniffi::Record, Clone, Debug)]
pub struct IngestionProgress {
    pub document_name: String,
    /// One of: "extracting", "chunking", "embedding", "indexing", "complete"
    pub stage: String,
}

/// No-op keychain provider for testing and platforms without a keychain.
///
/// `load` always returns `None`, `store` and `delete` are no-ops.
/// This means API keys cannot be persisted -- suitable for in-memory tests.
pub struct NullKeychainProvider;

impl KeychainProvider for NullKeychainProvider {
    fn store(&self, _: String, _: String, _: String) {}
    fn load(&self, _: String, _: String) -> Option<String> {
        None
    }
    fn delete(&self, _: String, _: String) {}
}

/// Desktop keychain provider using the OS-native credential store.
///
/// Uses `keyring` crate: Keychain Services on macOS, libsecret on Linux,
/// Windows Credential Manager on Windows.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub struct DesktopKeychainProvider;

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl KeychainProvider for DesktopKeychainProvider {
    fn store(&self, service: String, key: String, value: String) {
        if let Ok(entry) = keyring::Entry::new(&service, &key) {
            let _ = entry.set_password(&value);
        }
    }
    fn load(&self, service: String, key: String) -> Option<String> {
        keyring::Entry::new(&service, &key)
            .ok()?
            .get_password()
            .ok()
    }
    fn delete(&self, service: String, key: String) {
        if let Ok(entry) = keyring::Entry::new(&service, &key) {
            let _ = entry.delete_credential();
        }
    }
}

// ── Internal messages ────────────────────────────────────────────────────────

/// Internal actor messages -- not UniFFI-exported
pub enum CoreMsg {
    /// Wraps user-dispatched actions
    Action(AppAction),
    /// Delivers async LLM streaming events into the synchronous actor loop
    InternalEvent(Box<llm::InternalEvent>),
}

// ── Actor-internal state ─────────────────────────────────────────────────────

/// Actor-internal pending file attachment -- never crosses UniFFI boundary.
///
/// Stored in ActorState until the next SendMessage, when its content is prepended
/// to the user message as a context block. Cleared after SendMessage or ClearAttachment.
struct PendingAttachment {
    filename: String,
    /// Full UTF-8 text content of the file
    content: String,
}

/// Actor-internal state -- holds secrets and Tokio types that must NOT enter AppState.
/// Per Pitfall 3: BackendConfig (with api_key) and CancellationToken never cross FFI.
/// Per Pitfall 6 in RESEARCH.md: rusqlite::Connection is not Send -- must stay on actor thread.
///
/// Phase 4: AttestationCache is created transiently from db.conn() per use (Option A from
/// Plan 04-02). This avoids the self-referential lifetime issue with storing AttestationCache<'a>
/// alongside its source Database in the same struct.
struct ActorState {
    app_state: AppState,
    backends: Vec<llm::BackendConfig>,
    /// Latest attested TLS leaf public key fingerprint per backend.
    /// Used to opportunistically pin transport to the attested endpoint.
    attested_tls_public_keys: HashMap<String, String>,
    active_stream_token: Option<CancellationToken>,
    runtime: tokio::runtime::Runtime,
    /// Unified application database -- opened in FfiApp::new actor thread.
    db: persistence::Database,
    /// Platform keychain for loading API keys at runtime.
    keychain: Box<dyn KeychainProvider>,
    /// Pending file attachment waiting to be sent with the next message (D-17).
    pending_attachment: Option<PendingAttachment>,
    // Phase 6 additions:
    /// In-memory failover router -- health state persisted to SQLite on failure/success.
    router: llm::FailoverRouter,
    /// The backend_id that is currently streaming, for failover on StreamError.
    current_streaming_backend_id: Option<String>,
    /// Backend IDs already excluded in the current failover chain (tried and failed).
    failover_exclude: Vec<String>,
    // Phase 8 additions:
    /// On-device embedding provider (UniFFI callback_interface or DesktopEmbeddingProvider).
    embedding_provider: Arc<dyn EmbeddingProvider>,
    /// HNSW vector index for RAG retrieval (usearch, serialised to disk on mutation).
    vector_index: rag::VectorIndex,
    /// Pending RAG doc count for the current LLM turn (set before API call in SendMessage).
    /// Consumed in StreamDone to set rag_context_count on the assistant UiMessage.
    pending_rag_doc_count: Option<u32>,
    // Phase 9 additions:
    /// In-flight agent sessions keyed by session_id.
    /// A session is present here while it is actively running (spawning steps).
    /// Removed on pause, cancel, or terminal step result.
    active_agent_sessions: HashMap<String, agent::AgentExecutionState>,
    // Phase 10 additions:
    /// Cancellation token for the current periodic attestation timer task.
    /// Cancel + replace whenever the interval changes or the active backend changes.
    attestation_timer_token: Option<CancellationToken>,
    // Phase 10 fix: provider certificate cache (currently AMD VCEK for SNP).
    /// In-memory cache of provider collateral bytes keyed by certificate URL.
    /// Pre-populated from SQLite vcek_cert_cache table at startup.
    /// Shared with async attestation tasks via Arc<RwLock>.
    vcek_cache: attestation::task::CertificateCache,
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Parse a TeeType string stored in the backends table into the TeeType enum.
///
/// Matches the serialization used in the migration v1 seed data.
/// Unknown strings (e.g. future TEE types) map to TeeType::Unknown.
fn parse_tee_type(s: &str) -> llm::TeeType {
    match s {
        "IntelTdx" => llm::TeeType::IntelTdx,
        "NvidiaH100Cc" => llm::TeeType::NvidiaH100Cc,
        "AmdSevSnp" => llm::TeeType::AmdSevSnp,
        _ => llm::TeeType::Unknown,
    }
}

/// Return the current time as Unix milliseconds (i64).
///
/// Using milliseconds instead of seconds ensures unique timestamps even when
/// multiple messages are created within the same second (common in tests and
/// fast conversations). The `delete_messages_after` query uses `> created_at`
/// for edit/retry truncation, which requires distinct per-message timestamps.
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Generate a new UUID v4 as a lowercase hyphenated string.
fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Format a byte count as a human-readable size string.
fn format_size_display(size_bytes: u64) -> String {
    if size_bytes < 1024 {
        format!("{} B", size_bytes)
    } else if size_bytes < 1_048_576 {
        format!("{} KB", size_bytes / 1024)
    } else {
        format!("{} MB", size_bytes / 1_048_576)
    }
}

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
fn truncate_title(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let mut out: String = trimmed.chars().take(max_chars).collect();
        out.push_str("...");
        out
    }
}

/// Refresh the conversations list in AppState from SQLite.
fn refresh_conversations(actor_state: &mut ActorState) {
    let rows = persistence::queries::list_conversations(actor_state.db.conn()).unwrap_or_default();
    actor_state.app_state.conversations = rows
        .iter()
        .map(|row| ConversationSummary {
            id: row.id.clone(),
            title: row.title.clone(),
            model_id: row.model_id.clone(),
            backend_id: row.backend_id.clone(),
            updated_at: row.updated_at,
            system_prompt: row.system_prompt.clone(),
        })
        .collect();
}

/// Refresh app_state.messages from the DB for the given conversation_id.
fn refresh_messages(actor_state: &mut ActorState, conversation_id: &str) {
    let rows = persistence::queries::list_messages(actor_state.db.conn(), conversation_id)
        .unwrap_or_default();
    actor_state.app_state.messages = rows
        .iter()
        .map(|row| UiMessage {
            id: row.id.clone(),
            role: row.role.clone(),
            content: row.content.clone(),
            created_at: row.created_at,
            has_attachment: false,
            attachment_name: None,
            rag_context_count: None,
        })
        .collect();
}

/// Refresh app_state.backends summaries using live router health state.
///
/// Called after any action that changes backend health or the active backend.
fn refresh_backend_summaries(actor_state: &mut ActorState) {
    let active_id = actor_state.app_state.active_backend_id.clone();
    actor_state.app_state.backends = actor_state
        .backends
        .iter()
        .map(|b| {
            b.to_summary(
                active_id.as_deref() == Some(b.id.as_str()),
                actor_state.router.health_status(&b.id),
            )
        })
        .collect();
}

/// Reload actor_state.backends from SQLite + keychain.
///
/// Called after any AddBackend/RemoveBackend/ReorderBackend operation so the in-memory
/// list stays consistent with the database.
fn reload_backends(actor_state: &mut ActorState) {
    let rows = persistence::queries::list_backends(actor_state.db.conn()).unwrap_or_default();
    actor_state.backends = rows
        .iter()
        .map(|row| {
            let raw_models: Vec<String> = serde_json::from_str(&row.model_list).unwrap_or_default();
            // Apply per-backend model prefix filter at read time so stale SQLite rows
            // (written before the filter was added) are always cleaned up, not just when
            // a new health check runs. PPQ.AI returns 300+ models but only "private/" ones
            // run inside AMD SEV-SNP TEEs -- filter all others regardless of DB state.
            let models = filter_models_for_backend(&row.id, raw_models);
            llm::BackendConfig {
                id: row.id.clone(),
                name: row.name.clone(),
                base_url: row.base_url.clone(),
                api_key: actor_state
                    .keychain
                    .load("mango".to_string(), row.id.clone())
                    .unwrap_or_default(),
                models,
                tee_type: parse_tee_type(&row.tee_type),
                max_concurrent_requests: row.max_concurrent_requests.max(1) as u32,
                supports_tool_use: row.supports_tool_use,
            }
        })
        .collect();
    // Rebuild concurrency semaphores after config reload. Old Arc is dropped from the
    // HashMap; in-flight tasks retain their OwnedSemaphorePermit until completion.
    for backend in &actor_state.backends {
        actor_state.router.init_semaphore(
            &backend.id,
            backend.max_concurrent_requests as usize,
        );
    }
}

/// Apply per-backend model prefix filtering.
///
/// Some backends (e.g. PPQ.AI) expose hundreds of models via /v1/models but only a
/// small subset run inside a TEE. This function enforces the correct subset at read
/// time so stale SQLite rows never surface wrong models in the UI.
fn filter_models_for_backend(backend_id: &str, models: Vec<String>) -> Vec<String> {
    match backend_id {
        "ppq-ai" => models
            .into_iter()
            .filter(|id| id.starts_with("private/"))
            .collect(),
        _ => models,
    }
}

/// Build a Vec<ChatMessage> from the current conversation's in-memory messages and system prompt.
///
/// Used both in do_send_message (initial send) and StreamError failover (retry).
fn build_chat_messages(actor_state: &ActorState) -> Vec<llm::streaming::ChatMessage> {
    let mut msgs = Vec::new();
    if let Some(conv_id) = &actor_state.app_state.current_conversation_id {
        let conv_system_prompt = persistence::queries::list_conversations(actor_state.db.conn())
            .unwrap_or_default()
            .into_iter()
            .find(|c| &c.id == conv_id)
            .and_then(|c| c.system_prompt);
        let system_prompt = conv_system_prompt.or_else(|| {
            persistence::queries::get_setting(actor_state.db.conn(), "global_system_prompt")
                .unwrap_or(None)
        });
        if let Some(sp) = system_prompt {
            if !sp.is_empty() {
                msgs.push(llm::streaming::ChatMessage {
                    role: llm::streaming::ChatRole::System,
                    content: sp,
                });
            }
        }
    }
    for m in &actor_state.app_state.messages {
        let role = match m.role.as_str() {
            "user" => llm::streaming::ChatRole::User,
            "assistant" => llm::streaming::ChatRole::Assistant,
            "system" => llm::streaming::ChatRole::System,
            _ => llm::streaming::ChatRole::User,
        };
        msgs.push(llm::streaming::ChatMessage {
            role,
            content: m.content.clone(),
        });
    }
    msgs
}

fn pinned_tls_public_key_fp_for_backend(
    actor_state: &ActorState,
    backend_id: &str,
) -> Option<String> {
    actor_state
        .attested_tls_public_keys
        .get(backend_id)
        .cloned()
}

/// Spawn a background health-check probe for a backend.
///
/// Sends a GET /models request and delivers HealthCheckResult back to the actor loop.
fn spawn_health_check(
    runtime: &tokio::runtime::Runtime,
    backend_id: String,
    base_url: String,
    api_key: String,
    pinned_tls_public_key_fp: Option<String>,
    core_tx: flume::Sender<CoreMsg>,
) {
    runtime.spawn(async move {
        let backend = llm::BackendConfig {
            id: backend_id.clone(),
            name: backend_id.clone(),
            base_url,
            api_key,
            models: Vec::new(),
            tee_type: llm::TeeType::Unknown,
            max_concurrent_requests: 5,
            supports_tool_use: true,
        };
        let transport = backend.transport_kind();
        let url = match transport.model_list_url(&backend) {
            Ok(url) => url,
            Err(error) => {
                log::warn!(
                    target: "health_check",
                    "[health_check] backend={} unsupported transport for model probe: {}",
                    backend_id,
                    error
                );
                let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
                    llm::InternalEvent::HealthCheckResult { backend_id, success: false, models: vec![] },
                )));
                return;
            }
        };
        log::debug!(target: "health_check", "[health_check] backend={} url={} probing", backend_id, url);
        let (response, used_pin) = match transport.build_reqwest_client(
            &backend,
            pinned_tls_public_key_fp.as_deref(),
            std::time::Duration::from_secs(5),
        ) {
            Ok((client, used_pin)) => match client
                .get(&url)
                .bearer_auth(&backend.api_key)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(resp) => (Ok(resp), used_pin),
                Err(error) if used_pin => {
                    log::warn!(
                        target: "health_check",
                        "[health_check] backend={} pinned probe failed, retrying unpinned: {}",
                        backend_id,
                        error
                    );
                    match transport.build_reqwest_client(&backend, None, std::time::Duration::from_secs(5)) {
                        Ok((retry_client, _)) => (
                            retry_client
                                .get(&url)
                                .bearer_auth(&backend.api_key)
                                .timeout(std::time::Duration::from_secs(5))
                                .send()
                                .await,
                            false,
                        ),
                        Err(error) => {
                            log::warn!(
                                target: "health_check",
                                "[health_check] backend={} failed to build fallback client: {}",
                                backend_id,
                                error
                            );
                            let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
                                llm::InternalEvent::HealthCheckResult { backend_id, success: false, models: vec![] },
                            )));
                            return;
                        }
                    }
                }
                Err(error) => (Err(error), used_pin),
            },
            Err(error) => {
                log::warn!(
                    target: "health_check",
                    "[health_check] backend={} failed to build client: {}",
                    backend_id,
                    error
                );
                let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
                    llm::InternalEvent::HealthCheckResult { backend_id, success: false, models: vec![] },
                )));
                return;
            }
        };

        let (success, models) = match response {
            Ok(resp) if resp.status().is_success() => {
                let status = resp.status().as_u16();
                let json_val = resp.json::<serde_json::Value>().await.ok();
                let raw_size = json_val.as_ref().map(|v| v.to_string().len()).unwrap_or(0);
                log::debug!(
                    target: "health_check",
                    "[health_check] backend={} status={} response_bytes={} pinned={}",
                    backend_id,
                    status,
                    raw_size,
                    used_pin
                );
                // PPQ.AI exposes 300+ models but only those with the "private/" prefix
                // run inside AMD SEV-SNP TEEs via Tinfoil. Filter to TEE-only models
                // so the UI never offers non-confidential models from this backend.
                let model_prefix_filter: Option<&str> = if backend_id == "ppq-ai" {
                    Some("private/")
                } else {
                    None
                };
                let models = json_val
                    .and_then(|v| v.get("data").cloned())
                    .and_then(|d| d.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(str::to_owned))
                    .filter(|id| {
                        model_prefix_filter.map_or(true, |prefix| id.starts_with(prefix))
                    })
                    .collect::<Vec<String>>();
                let preview: Vec<&str> = models.iter().take(5).map(|s| s.as_str()).collect();
                log::debug!(target: "health_check", "[health_check] backend={} model_count={} models_preview={:?}", backend_id, models.len(), preview);

                // /v1/models is unauthenticated on most providers, so a 200 here
                // doesn't prove the API key is valid.  When a key was provided,
                // send a minimal chat completion (max_tokens=1) to verify auth.
                if !backend.api_key.is_empty() {
                    if let Some(probe_model) = models.first() {
                        let completions_url = format!(
                            "{}/chat/completions",
                            backend.base_url.trim_end_matches('/')
                        );
                        let probe_body = serde_json::json!({
                            "model": probe_model,
                            "max_tokens": 1,
                            "messages": [{"role": "user", "content": "hi"}]
                        });
                        let (probe_client, _) = match transport.build_reqwest_client(
                            &backend,
                            pinned_tls_public_key_fp.as_deref(),
                            std::time::Duration::from_secs(5),
                        ) {
                            Ok(pair) => pair,
                            Err(e) => {
                                log::warn!(
                                    target: "health_check",
                                    "[health_check] backend={} auth probe client build failed: {}",
                                    backend_id,
                                    e
                                );
                                let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
                                    llm::InternalEvent::HealthCheckResult { backend_id, success: false, models },
                                )));
                                return;
                            }
                        };
                        match probe_client
                            .post(&completions_url)
                            .bearer_auth(&backend.api_key)
                            .json(&probe_body)
                            .timeout(std::time::Duration::from_secs(5))
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => {
                                log::debug!(
                                    target: "health_check",
                                    "[health_check] backend={} api_key auth verified via chat/completions",
                                    backend_id
                                );
                                (true, models)
                            }
                            Ok(resp) => {
                                let status = resp.status().as_u16();
                                log::warn!(
                                    target: "health_check",
                                    "[health_check] backend={} api_key auth failed status={}",
                                    backend_id,
                                    status
                                );
                                (false, models)
                            }
                            Err(e) => {
                                log::warn!(
                                    target: "health_check",
                                    "[health_check] backend={} api_key auth probe error: {}",
                                    backend_id,
                                    e
                                );
                                (false, models)
                            }
                        }
                    } else {
                        // No models available — can't verify key, treat as success
                        (true, models)
                    }
                } else {
                    // No API key (e.g. Tinfoil) — models reachability is sufficient
                    (true, models)
                }
            }
            Ok(resp) => {
                let status = resp.status().as_u16();
                log::warn!(target: "health_check", "[health_check] backend={} url={} status={} probe failed (non-success)", backend_id, url, status);
                (false, vec![])
            }
            Err(e) => {
                log::warn!(target: "health_check", "[health_check] backend={} url={} error={}", backend_id, url, e);
                (false, vec![])
            }
        };
        let _ = core_tx.send(CoreMsg::InternalEvent(Box::new(
            llm::InternalEvent::HealthCheckResult { backend_id, success, models },
        )));
    });
}

/// Spawn a background periodic attestation timer task.
///
/// The task sends `AttestationTick` to the actor loop every `interval_minutes` minutes.
/// Pass the returned `CancellationToken` to `cancel()` when the timer should stop.
/// A value of 0 for `interval_minutes` skips spawning (timer disabled).
fn spawn_attestation_timer(
    runtime: &tokio::runtime::Runtime,
    interval_minutes: u32,
    core_tx: flume::Sender<CoreMsg>,
) -> Option<CancellationToken> {
    if interval_minutes == 0 {
        return None;
    }
    let token = CancellationToken::new();
    let token_clone = token.clone();
    let duration = std::time::Duration::from_secs((interval_minutes as u64) * 60);
    runtime.spawn(async move {
        let mut interval = tokio::time::interval(duration);
        // Tick immediately to establish the rhythm; first tick fires right away.
        // We skip the first tick so attestation fires after the full interval, not immediately.
        interval.tick().await; // consume the immediate first tick
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if core_tx.send(CoreMsg::InternalEvent(Box::new(
                        llm::InternalEvent::AttestationTick,
                    ))).is_err() {
                        break; // actor loop gone
                    }
                }
                _ = token_clone.cancelled() => {
                    break;
                }
            }
        }
    });
    Some(token)
}

/// Core send-message logic called by both SendMessage and RetryLastMessage/EditMessage.
///
/// Looks up the active backend, builds the full message history with system prompt,
/// persists the user message to SQLite, appends it to app_state.messages,
/// handles the pending attachment if any, and spawns the streaming task.
fn do_send_message(actor_state: &mut ActorState, text: String, core_tx: &flume::Sender<CoreMsg>) {
    // Guard: must have an active conversation
    let conv_id = match actor_state.app_state.current_conversation_id.clone() {
        Some(id) => id,
        None => {
            actor_state.app_state.last_error = Some("No active conversation".into());
            return;
        }
    };

    // Determine preferred backend: conversation override > active_backend_id
    let conv_backend_id = actor_state
        .app_state
        .conversations
        .iter()
        .find(|c| c.id == conv_id)
        .map(|c| c.backend_id.clone());
    let preferred_id = conv_backend_id.or(actor_state.app_state.active_backend_id.clone());

    // Determine model from conversation row, falling back to first model on backend
    let model = actor_state
        .app_state
        .conversations
        .iter()
        .find(|c| c.id == conv_id)
        .map(|c| c.model_id.clone())
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| {
            preferred_id
                .as_deref()
                .and_then(|pid| actor_state.backends.iter().find(|b| b.id == pid))
                .and_then(|b| b.models.first().cloned())
                .unwrap_or_default()
        });

    // Route through the FailoverRouter
    let backend = match actor_state.router.select_backend(
        &actor_state.backends,
        preferred_id.as_deref(),
        &model,
        &[],
    ) {
        Some(b) => b.clone(),
        None => {
            actor_state.app_state.last_error = Some("No healthy backend available".into());
            return;
        }
    };

    // Track which backend is streaming for failover
    actor_state.current_streaming_backend_id = Some(backend.id.clone());
    actor_state.failover_exclude = vec![];
    let now = now_secs();

    // Handle pending attachment: prepend content to the message text
    let (final_text, has_attachment, attachment_name) =
        if let Some(att) = actor_state.pending_attachment.take() {
            let augmented = format!(
                "[Attached: {}]\n\n{}\n\n---\n\n{}",
                att.filename, att.content, text
            );
            let name = att.filename.clone();
            (augmented, true, Some(name))
        } else {
            (text, false, None)
        };

    // Clear pending_attachment from AppState too
    actor_state.app_state.pending_attachment = None;

    // Persist the user message to SQLite
    let msg_id = new_uuid();
    let user_row = persistence::MessageRow {
        id: msg_id.clone(),
        conversation_id: conv_id.clone(),
        role: "user".to_string(),
        content: final_text.clone(),
        created_at: now,
        token_count: None,
    };
    let _ = persistence::queries::insert_message(actor_state.db.conn(), &user_row);

    // Build the UiMessage and append to app_state.messages
    let user_ui_msg = UiMessage {
        id: msg_id,
        role: "user".to_string(),
        content: final_text.clone(),
        created_at: now,
        has_attachment,
        attachment_name,
        rag_context_count: None,
    };
    actor_state.app_state.messages.push(user_ui_msg);

    // Update conversation updated_at
    let _ =
        persistence::queries::update_conversation_updated_at(actor_state.db.conn(), &conv_id, now);
    refresh_conversations(actor_state);

    // Build full message history for the LLM (system prompt + all messages)
    let mut chat_messages: Vec<llm::streaming::ChatMessage> = Vec::new();

    // Resolve system prompt: per-conversation first, then global default
    let conv_system_prompt = persistence::queries::list_conversations(actor_state.db.conn())
        .unwrap_or_default()
        .into_iter()
        .find(|c| c.id == conv_id)
        .and_then(|c| c.system_prompt);
    let base_system_prompt = conv_system_prompt
        .or_else(|| {
            persistence::queries::get_setting(actor_state.db.conn(), "global_system_prompt")
                .unwrap_or(None)
        })
        .unwrap_or_default();

    // Hoist embedding: used by both RAG context and memory injection (computed once).
    let query_emb = actor_state
        .embedding_provider
        .embed(vec![final_text.clone()]);

    // Phase 8: RAG context injection (D-04, D-05, D-06).
    // If documents are attached to this conversation, search the vector index for
    // relevant chunks and inject them into the system prompt.
    let mut rag_doc_count: Option<u32> = None;
    let system_prompt_after_rag = if !actor_state
        .app_state
        .current_conversation_attached_docs
        .is_empty()
    {
        if !query_emb.is_empty() {
            // Search the vector index for top-k chunks
            match actor_state
                .vector_index
                .search(&query_emb, rag::DEFAULT_TOP_K)
            {
                Ok(results) => {
                    // Filter results to chunks belonging to attached documents.
                    // We need to map usearch rowids back to their document -- look up in SQLite.
                    let all_rowids: Vec<i64> = results.iter().map(|(k, _)| *k as i64).collect();
                    let chunk_texts = persistence::queries::get_chunk_text_by_rowids(
                        actor_state.db.conn(),
                        &all_rowids,
                    )
                    .unwrap_or_default();

                    let chunk_results: Vec<rag::ChunkResult> = chunk_texts
                        .into_iter()
                        .zip(results.iter())
                        .map(|((_, text), (_, score))| rag::ChunkResult {
                            text,
                            score: *score,
                        })
                        .collect();

                    if !chunk_results.is_empty() {
                        let doc_count = actor_state
                            .app_state
                            .current_conversation_attached_docs
                            .len() as u32;
                        rag_doc_count = Some(doc_count);
                        rag::build_system_with_context(&base_system_prompt, &chunk_results)
                    } else {
                        base_system_prompt
                    }
                }
                Err(_) => base_system_prompt,
            }
        } else {
            base_system_prompt
        }
    } else {
        base_system_prompt
    };

    // Phase 21: Memory injection (MEM-03).
    // Search shared usearch index for memories relevant to the user's message.
    // Keys that hit the memories table are memory content; chunk keys are silently ignored.
    let system_prompt = if !query_emb.is_empty() {
        match actor_state
            .vector_index
            .search(&query_emb, memory::retrieve::DEFAULT_MEMORY_TOP_K)
        {
            Ok(results) => {
                let keys: Vec<i64> = results.iter().map(|(k, _)| *k as i64).collect();
                let memory_hits = persistence::queries::get_memory_content_by_usearch_keys(
                    actor_state.db.conn(),
                    &keys,
                )
                .unwrap_or_default();
                if !memory_hits.is_empty() {
                    let mem_results: Vec<memory::retrieve::MemoryResult> = memory_hits
                        .into_iter()
                        .zip(results.iter())
                        .map(|((_, content), (_, score))| memory::retrieve::MemoryResult {
                            content,
                            score: *score,
                        })
                        .collect();
                    memory::retrieve::build_system_with_memories(
                        &system_prompt_after_rag,
                        &mem_results,
                    )
                } else {
                    system_prompt_after_rag
                }
            }
            Err(_) => system_prompt_after_rag,
        }
    } else {
        system_prompt_after_rag
    };

    // Store pending RAG doc count for StreamDone to attach to the assistant message
    actor_state.pending_rag_doc_count = rag_doc_count;

    if !system_prompt.is_empty() {
        chat_messages.push(llm::streaming::ChatMessage {
            role: llm::streaming::ChatRole::System,
            content: system_prompt,
        });
    }

    // Add all existing messages (including the one we just appended)
    for msg in &actor_state.app_state.messages {
        let role = match msg.role.as_str() {
            "user" => llm::streaming::ChatRole::User,
            "assistant" => llm::streaming::ChatRole::Assistant,
            "system" => llm::streaming::ChatRole::System,
            _ => llm::streaming::ChatRole::User,
        };
        chat_messages.push(llm::streaming::ChatMessage {
            role,
            content: msg.content.clone(),
        });
    }

    // Set busy state and start streaming
    actor_state.app_state.busy_state = BusyState::Streaming {
        model: model.clone(),
    };
    actor_state.app_state.streaming_text = Some(String::new());
    actor_state.app_state.last_error = None;

    let token = llm::spawn_streaming_task(
        &actor_state.runtime,
        &backend,
        &model,
        chat_messages,
        pinned_tls_public_key_fp_for_backend(actor_state, &backend.id),
        core_tx.clone(),
        actor_state.router.get_semaphore(&backend.id),
    );
    actor_state.active_stream_token = Some(token);
    refresh_backend_summaries(actor_state);
}

// ── Phase 9: Agent helpers ────────────────────────────────────────────────────

/// Refresh app_state.agent_sessions from SQLite (mirrors refresh_conversations).
fn refresh_agent_sessions(actor_state: &mut ActorState) {
    let rows = persistence::queries::list_agent_sessions(actor_state.db.conn()).unwrap_or_default();
    actor_state.app_state.agent_sessions = rows
        .iter()
        .map(|row| {
            let step_count = persistence::queries::count_agent_steps(actor_state.db.conn(), &row.id)
                .unwrap_or(0) as u32;
            AgentSessionSummary {
                id: row.id.clone(),
                title: row.title.clone(),
                status: row.status.clone(),
                backend_id: row.backend_id.clone(),
                updated_at: row.updated_at,
                step_count,
                elapsed_secs: row.updated_at - row.created_at,
            }
        })
        .collect();
}

/// Launch a new agent session and spawn the first step.
fn handle_launch_agent_session(
    actor_state: &mut ActorState,
    task_description: String,
    core_tx: &flume::Sender<CoreMsg>,
) {
    use async_openai::types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs,
    };

    // Find a tool-capable backend
    let backend = actor_state
        .backends
        .iter()
        .find(|b| b.id == "tinfoil")
        .cloned();
    let backend = match backend {
        Some(b) => b,
        None => {
            actor_state.app_state.toast = Some(
                "No tool-capable backend available. Add Tinfoil or a tool-capable backend.".into(),
            );
            return;
        }
    };

    // Load API key from keychain if not already set
    let api_key = if backend.api_key.is_empty() {
        actor_state
            .keychain
            .load("mango".to_string(), backend.id.clone())
            .unwrap_or_default()
    } else {
        backend.api_key.clone()
    };

    let session_id = new_uuid();
    let now = now_secs();
    let model = backend
        .models
        .first()
        .cloned()
        .unwrap_or_else(|| "meta-llama/Llama-3.3-70B-Instruct".to_string());

    // Insert session row
    let session_row = persistence::queries::AgentSessionRow {
        id: session_id.clone(),
        title: task_description.clone(),
        status: "running".to_string(),
        backend_id: backend.id.clone(),
        created_at: now,
        updated_at: now,
    };
    if let Err(e) = persistence::queries::insert_agent_session(actor_state.db.conn(), &session_row)
    {
        actor_state.app_state.toast = Some(format!("Failed to create agent session: {}", e));
        return;
    }

    // Build system + user messages
    let system_content = format!(
        "You are an autonomous agent. You have the following tools: search_documents, read_document, finish. \
         Use search_documents to find relevant information, read_document to read full documents, \
         and finish to provide your final answer. The user's task is: {}",
        task_description
    );

    let system_msg: Result<ChatCompletionRequestMessage, _> =
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_content)
            .build()
            .map(ChatCompletionRequestMessage::from);
    let user_msg: Result<ChatCompletionRequestMessage, _> =
        ChatCompletionRequestUserMessageArgs::default()
            .content(task_description.clone())
            .build()
            .map(ChatCompletionRequestMessage::from);

    let messages = match (system_msg, user_msg) {
        (Ok(s), Ok(u)) => vec![s, u],
        _ => {
            actor_state.app_state.toast = Some("Failed to build agent messages".into());
            return;
        }
    };

    let exec_state = agent::AgentExecutionState {
        session_id: session_id.clone(),
        messages: messages.clone(),
        step_number: 1,
        backend_id: backend.id.clone(),
        model: model.clone(),
    };

    // Store in active sessions
    actor_state
        .active_agent_sessions
        .insert(session_id.clone(), exec_state);

    // Spawn first step
    let backend_for_client = llm::BackendConfig {
        api_key,
        ..backend.clone()
    };
    let tools = agent::build_agent_tools();
    let sid = session_id.clone();
    let core_tx_clone = core_tx.clone();

    actor_state.runtime.spawn(async move {
        let result = agent::run_agent_step_for_backend(&backend_for_client, &model, messages, tools).await;
        let _ = core_tx_clone.send(CoreMsg::InternalEvent(Box::new(
            llm::InternalEvent::AgentStepComplete {
                session_id: sid,
                step_number: 1,
                result,
            },
        )));
    });

    refresh_agent_sessions(actor_state);
}

/// Handle an AgentStepComplete event: checkpoint step, dispatch tools or terminate.
fn handle_agent_step_complete(
    actor_state: &mut ActorState,
    session_id: String,
    step_number: i64,
    result: Result<agent::AgentStepResult, llm::LlmError>,
    core_tx: &flume::Sender<CoreMsg>,
    shared: &Arc<RwLock<AppState>>,
    update_tx: &flume::Sender<AppUpdate>,
) {
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestToolMessageArgs,
    };

    // If session was cancelled/paused, ignore stale events
    if !actor_state.active_agent_sessions.contains_key(&session_id) {
        return;
    }

    let now = now_secs();

    match result {
        Ok(agent::AgentStepResult::ToolCalls(calls)) => {
            // Checkpoint tool call step to SQLite
            let step_id = new_uuid();
            let payload = serde_json::to_string(
                &calls
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "name": c.function.name,
                            "arguments": c.function.arguments,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[]".to_string());

            let step_row = persistence::queries::AgentStepRow {
                id: step_id.clone(),
                session_id: session_id.clone(),
                step_number,
                action_type: "tool_call".to_string(),
                action_payload: payload,
                result: None,
                status: "completed".to_string(),
                created_at: now,
            };
            let _ = persistence::queries::insert_agent_step(actor_state.db.conn(), &step_row);

            // Check step limit (max 20 steps per D-02)
            let current_count =
                persistence::queries::count_agent_steps(actor_state.db.conn(), &session_id)
                    .unwrap_or(0);
            if current_count >= 20 {
                // Step limit reached
                let _ = persistence::queries::update_agent_session_status(
                    actor_state.db.conn(),
                    &session_id,
                    "failed",
                    now,
                );
                actor_state.active_agent_sessions.remove(&session_id);
                actor_state.app_state.toast =
                    Some("Agent session stopped: step limit reached (20 steps)".into());
                refresh_agent_sessions(actor_state);
                actor_state.app_state.rev += 1;
                emit_state(&actor_state.app_state, shared, update_tx);
                return;
            }

            // Dispatch tools synchronously on the actor thread
            // NOTE: runtime/data_dir/brave_api_key are temporary stubs here;
            // Plan 22-02 will wire these properly from ActorState.
            let tool_results = agent::dispatch_tools(
                &calls,
                actor_state.db.conn(),
                &actor_state.vector_index,
                actor_state.embedding_provider.as_ref(),
                &actor_state.runtime,
                "",
                "",
            );

            // Get mutable exec_state
            let exec_state = match actor_state.active_agent_sessions.get_mut(&session_id) {
                Some(s) => s,
                None => return,
            };

            // Append assistant message (with tool_calls) to history
            let assistant_tool_calls: Vec<
                async_openai::types::chat::ChatCompletionMessageToolCalls,
            > = calls
                .iter()
                .map(|c| {
                    async_openai::types::chat::ChatCompletionMessageToolCalls::Function(c.clone())
                })
                .collect();

            if let Ok(assistant_msg) = ChatCompletionRequestAssistantMessageArgs::default()
                .tool_calls(assistant_tool_calls)
                .build()
                .map(ChatCompletionRequestMessage::from)
            {
                exec_state.messages.push(assistant_msg);
            }

            // Append tool result messages
            for (tool_call_id, result_text) in &tool_results {
                if let Ok(tool_msg) = ChatCompletionRequestToolMessageArgs::default()
                    .tool_call_id(tool_call_id.as_str())
                    .content(result_text.as_str())
                    .build()
                    .map(ChatCompletionRequestMessage::from)
                {
                    exec_state.messages.push(tool_msg);
                }
            }

            exec_state.step_number += 1;
            let next_step = exec_state.step_number;
            let messages = exec_state.messages.clone();
            let model = exec_state.model.clone();
            let backend_id = exec_state.backend_id.clone();

            // Look up backend config for the API call
            let backend = actor_state
                .backends
                .iter()
                .find(|b| b.id == backend_id)
                .cloned();
            let backend = match backend {
                Some(b) => b,
                None => {
                    let _ = persistence::queries::update_agent_session_status(
                        actor_state.db.conn(),
                        &session_id,
                        "failed",
                        now,
                    );
                    actor_state.active_agent_sessions.remove(&session_id);
                    refresh_agent_sessions(actor_state);
                    actor_state.app_state.rev += 1;
                    emit_state(&actor_state.app_state, shared, update_tx);
                    return;
                }
            };
            let api_key = if backend.api_key.is_empty() {
                actor_state
                    .keychain
                    .load("mango".to_string(), backend_id.clone())
                    .unwrap_or_default()
            } else {
                backend.api_key.clone()
            };

            let backend_for_client = llm::BackendConfig {
                api_key,
                ..backend.clone()
            };
            let tools = agent::build_agent_tools();
            let sid = session_id.clone();
            let core_tx_clone = core_tx.clone();

            actor_state.runtime.spawn(async move {
                let result =
                    agent::run_agent_step_for_backend(&backend_for_client, &model, messages, tools)
                        .await;
                let _ = core_tx_clone.send(CoreMsg::InternalEvent(Box::new(
                    llm::InternalEvent::AgentStepComplete {
                        session_id: sid,
                        step_number: next_step,
                        result,
                    },
                )));
            });

            // Update session status timestamp
            let _ = persistence::queries::update_agent_session_status(
                actor_state.db.conn(),
                &session_id,
                "running",
                now,
            );
            refresh_agent_sessions(actor_state);
            actor_state.app_state.rev += 1;
            emit_state(&actor_state.app_state, shared, update_tx);
        }

        Ok(agent::AgentStepResult::FinishTool(text))
        | Ok(agent::AgentStepResult::FinalAnswer(text)) => {
            // Session completed
            let step_id = new_uuid();
            let step_row = persistence::queries::AgentStepRow {
                id: step_id,
                session_id: session_id.clone(),
                step_number,
                action_type: "final_answer".to_string(),
                action_payload: "{}".to_string(),
                result: Some(text.clone()),
                status: "completed".to_string(),
                created_at: now,
            };
            let _ = persistence::queries::insert_agent_step(actor_state.db.conn(), &step_row);
            let _ = persistence::queries::update_agent_session_status(
                actor_state.db.conn(),
                &session_id,
                "completed",
                now,
            );
            actor_state.active_agent_sessions.remove(&session_id);
            actor_state.app_state.toast = Some("Agent session completed".into());
            refresh_agent_sessions(actor_state);

            // Update current_agent_steps if this session is loaded
            if actor_state.app_state.current_agent_session_id.as_deref() == Some(&session_id) {
                handle_load_agent_session(actor_state, &session_id);
            }

            actor_state.app_state.rev += 1;
            emit_state(&actor_state.app_state, shared, update_tx);
        }

        Err(error) => {
            // Mark session as failed
            let _ = persistence::queries::update_agent_session_status(
                actor_state.db.conn(),
                &session_id,
                "failed",
                now,
            );
            actor_state.active_agent_sessions.remove(&session_id);
            actor_state.app_state.toast =
                Some(format!("Agent step failed: {}", error.display_message()));
            refresh_agent_sessions(actor_state);
            actor_state.app_state.rev += 1;
            emit_state(&actor_state.app_state, shared, update_tx);
        }
    }
}

/// Pause an active agent session.
fn handle_pause_agent_session(actor_state: &mut ActorState, session_id: String) {
    let now = now_secs();
    let _ = persistence::queries::update_agent_session_status(
        actor_state.db.conn(),
        &session_id,
        "paused",
        now,
    );
    // Remove from active sessions -- next AgentStepComplete will be ignored
    actor_state.active_agent_sessions.remove(&session_id);
    refresh_agent_sessions(actor_state);
}

/// Resume a paused agent session by rebuilding message history from SQLite.
fn handle_resume_agent_session(
    actor_state: &mut ActorState,
    session_id: String,
    core_tx: &flume::Sender<CoreMsg>,
) {
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    };

    // Idempotent: if already running, return
    if actor_state.active_agent_sessions.contains_key(&session_id) {
        return;
    }

    let now = now_secs();

    // Load session from SQLite
    let session_row = persistence::queries::list_agent_sessions(actor_state.db.conn())
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.id == session_id);
    let session_row = match session_row {
        Some(r) => r,
        None => return,
    };

    // Load steps
    let steps = persistence::queries::list_agent_steps(actor_state.db.conn(), &session_id)
        .unwrap_or_default();

    // Rebuild message history
    let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

    // System + user message (title is the original task description)
    let system_content = format!(
        "You are an autonomous agent. You have the following tools: search_documents, read_document, finish. \
         Use search_documents to find relevant information, read_document to read full documents, \
         and finish to provide your final answer. The user's task is: {}",
        session_row.title
    );
    if let Ok(msg) = ChatCompletionRequestSystemMessageArgs::default()
        .content(system_content)
        .build()
        .map(ChatCompletionRequestMessage::from)
    {
        messages.push(msg);
    }
    if let Ok(msg) = ChatCompletionRequestUserMessageArgs::default()
        .content(session_row.title.clone())
        .build()
        .map(ChatCompletionRequestMessage::from)
    {
        messages.push(msg);
    }

    // Replay completed steps into message history
    for step in &steps {
        if step.action_type == "tool_call" {
            if let Ok(payload) =
                serde_json::from_str::<Vec<serde_json::Value>>(&step.action_payload)
            {
                let tool_calls: Vec<async_openai::types::chat::ChatCompletionMessageToolCalls> =
                    payload
                        .iter()
                        .filter_map(|v| {
                            Some(
                                async_openai::types::chat::ChatCompletionMessageToolCalls::Function(
                                    async_openai::types::chat::ChatCompletionMessageToolCall {
                                        id: v.get("id")?.as_str()?.to_string(),
                                        function: async_openai::types::chat::FunctionCall {
                                            name: v.get("name")?.as_str()?.to_string(),
                                            arguments: v.get("arguments")?.as_str()?.to_string(),
                                        },
                                    },
                                ),
                            )
                        })
                        .collect();
                if !tool_calls.is_empty() {
                    if let Ok(msg) = ChatCompletionRequestAssistantMessageArgs::default()
                        .tool_calls(tool_calls)
                        .build()
                        .map(ChatCompletionRequestMessage::from)
                    {
                        messages.push(msg);
                    }
                }
            }
        }
    }

    let next_step = (steps.len() as i64) + 1;
    let backend = actor_state
        .backends
        .iter()
        .find(|b| b.id == session_row.backend_id)
        .cloned();
    let backend = match backend {
        Some(b) => b,
        None => return,
    };
    let api_key = if backend.api_key.is_empty() {
        actor_state
            .keychain
            .load("mango".to_string(), backend.id.clone())
            .unwrap_or_default()
    } else {
        backend.api_key.clone()
    };
    let model = backend
        .models
        .first()
        .cloned()
        .unwrap_or_else(|| "meta-llama/Llama-3.3-70B-Instruct".to_string());

    let exec_state = agent::AgentExecutionState {
        session_id: session_id.clone(),
        messages: messages.clone(),
        step_number: next_step,
        backend_id: backend.id.clone(),
        model: model.clone(),
    };
    actor_state
        .active_agent_sessions
        .insert(session_id.clone(), exec_state);

    let _ = persistence::queries::update_agent_session_status(
        actor_state.db.conn(),
        &session_id,
        "running",
        now,
    );

    let backend_for_client = llm::BackendConfig {
        api_key,
        ..backend.clone()
    };
    let tools = agent::build_agent_tools();
    let sid = session_id.clone();
    let core_tx_clone = core_tx.clone();

    actor_state.runtime.spawn(async move {
        let result = agent::run_agent_step_for_backend(&backend_for_client, &model, messages, tools).await;
        let _ = core_tx_clone.send(CoreMsg::InternalEvent(Box::new(
            llm::InternalEvent::AgentStepComplete {
                session_id: sid,
                step_number: next_step,
                result,
            },
        )));
    });

    refresh_agent_sessions(actor_state);
}

/// Cancel an agent session.
fn handle_cancel_agent_session(actor_state: &mut ActorState, session_id: String) {
    let now = now_secs();
    let _ = persistence::queries::update_agent_session_status(
        actor_state.db.conn(),
        &session_id,
        "cancelled",
        now,
    );
    actor_state.active_agent_sessions.remove(&session_id);
    // Clear detail view if this session is loaded
    if actor_state.app_state.current_agent_session_id.as_deref() == Some(&session_id) {
        actor_state.app_state.current_agent_steps = vec![];
    }
    refresh_agent_sessions(actor_state);
}

/// Load agent steps for a session into AppState for UI display.
fn handle_load_agent_session(actor_state: &mut ActorState, session_id: &str) {
    let steps = persistence::queries::list_agent_steps(actor_state.db.conn(), session_id)
        .unwrap_or_default();

    actor_state.app_state.current_agent_steps = steps
        .iter()
        .map(|step| {
            // Extract tool_name from action_payload JSON if it's a tool_call step
            let tool_name = if step.action_type == "tool_call" {
                serde_json::from_str::<Vec<serde_json::Value>>(&step.action_payload)
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .and_then(|v| {
                        v.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
            } else {
                None
            };

            // Truncate result to 200 chars for the snippet
            let result_snippet = step.result.as_ref().map(|r| {
                let chars: String = r.chars().take(200).collect();
                chars
            });

            AgentStepSummary {
                id: step.id.clone(),
                step_number: step.step_number as u32,
                action_type: step.action_type.clone(),
                tool_name,
                result_snippet,
                status: step.status.clone(),
            }
        })
        .collect();

    actor_state.app_state.current_agent_session_id = Some(session_id.to_string());
}

/// Emit state snapshot to shared RwLock and update channel.
///
/// Extracted as a free function so agent event handlers (which run outside the closure)
/// can emit state without capturing the closure's bindings.
fn emit_state(state: &AppState, shared: &Arc<RwLock<AppState>>, tx: &flume::Sender<AppUpdate>) {
    let snapshot = state.clone();
    match shared.write() {
        Ok(mut g) => *g = snapshot.clone(),
        Err(p) => *p.into_inner() = snapshot.clone(),
    }
    let _ = tx.send(AppUpdate::FullState(snapshot));
}

// ── FFI entry point ──────────────────────────────────────────────────────────

#[derive(uniffi::Object)]
pub struct FfiApp {
    core_tx: Sender<CoreMsg>,
    update_rx: Receiver<AppUpdate>,
    listening: AtomicBool,
    shared_state: Arc<RwLock<AppState>>,
    /// Raw attestation report blobs keyed by backend_id (D-12).
    /// Written by the actor thread, read via get_raw_attestation_report FFI.
    /// Separate from AppState so large blobs don't inflate every state snapshot.
    shared_reports: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

#[uniffi::export]
impl FfiApp {
    /// Create the app core, spawn the actor thread, and return the FFI handle.
    ///
    /// `data_dir` is the on-disk directory where `mango.db` will be created.
    /// Pass an empty string (or use `:memory:`) for in-memory tests.
    /// `keychain` provides platform-native API key storage; use `NullKeychainProvider` for tests.
    /// `embedding_provider` provides on-device embedding inference; use `NullEmbeddingProvider` for tests.
    /// `embedding_status` reflects whether the real embedding provider loaded successfully
    /// (`Active`) or fell back to `NullEmbeddingProvider` due to an init failure (`Degraded`).
    #[uniffi::constructor]
    pub fn new(
        data_dir: String,
        keychain: Box<dyn KeychainProvider>,
        embedding_provider: Box<dyn EmbeddingProvider>,
        embedding_status: EmbeddingStatus,
    ) -> Arc<Self> {
        // Initialize logger once -- idempotent if FfiApp::new is called more than once.
        static LOG_INIT: std::sync::Once = std::sync::Once::new();
        LOG_INIT.call_once(|| {
            #[cfg(target_os = "android")]
            android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(if cfg!(debug_assertions) {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    })
                    .with_tag("mango"),
            );
            #[cfg(not(target_os = "android"))]
            env_logger::init();
        });
        log::info!("mango_core initialized");

        let (update_tx, update_rx) = flume::unbounded::<AppUpdate>();
        let (core_tx, core_rx) = flume::unbounded::<CoreMsg>();
        let shared_state = Arc::new(RwLock::new(AppState::default()));
        let shared_reports = Arc::new(RwLock::new(HashMap::<String, Vec<u8>>::new()));

        let shared_for_core = shared_state.clone();
        let shared_reports_for_core = shared_reports.clone();
        let core_tx_for_thread = core_tx.clone();

        // Compute the unified database path before moving data_dir into the thread.
        // Empty data_dir means in-memory (":memory:") -- used in tests and development.
        let db_path = if data_dir.is_empty() {
            ":memory:".to_string()
        } else {
            format!("{}/mango.db", data_dir)
        };

        // Compute data_dir for VectorIndex (used inside the actor thread).
        // Empty data_dir means in-memory DB -- use a temp-like path that will not persist.
        let vector_data_dir = data_dir.clone();

        thread::spawn(move || {
            // Create the Tokio runtime owned by the actor thread.
            // Multi-thread with 2 workers so streaming tasks run in background
            // while the actor loop continues processing messages synchronously.
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_time()
                .enable_io()
                .build()
                .expect("tokio runtime");

            // Open the unified database on the actor thread.
            // Per Pitfall 6: rusqlite::Connection is not Send -- must stay here.
            // Database::open runs migration v1 which creates all tables including
            // attestation_cache, backends, conversations, messages, etc.
            let db = persistence::Database::open(&db_path).expect("database open and migration");

            // Load backends from SQLite (seeded in migration v1).
            // API keys are loaded from the keychain at this point -- never stored in SQLite.
            let backend_rows = persistence::queries::list_backends(db.conn()).unwrap_or_default();
            let active_id = persistence::queries::get_active_backend_id(db.conn()).unwrap_or(None);

            let backends: Vec<llm::BackendConfig> = backend_rows
                .iter()
                .map(|row| {
                    let api_key = keychain
                        .load("mango".to_string(), row.id.clone())
                        .unwrap_or_default();
                    let raw_models: Vec<String> =
                        serde_json::from_str(&row.model_list).unwrap_or_default();
                    let models = filter_models_for_backend(&row.id, raw_models);
                    llm::BackendConfig {
                        id: row.id.clone(),
                        name: row.name.clone(),
                        base_url: row.base_url.clone(),
                        api_key,
                        models,
                        tee_type: parse_tee_type(&row.tee_type),
                        max_concurrent_requests: row.max_concurrent_requests.max(1) as u32,
                        supports_tool_use: row.supports_tool_use,
                    }
                })
                .collect();

            // Load existing conversations from SQLite into AppState on startup (PERS-02).
            let conversation_rows =
                persistence::queries::list_conversations(db.conn()).unwrap_or_default();
            let conversations: Vec<ConversationSummary> = conversation_rows
                .iter()
                .map(|row| ConversationSummary {
                    id: row.id.clone(),
                    title: row.title.clone(),
                    model_id: row.model_id.clone(),
                    backend_id: row.backend_id.clone(),
                    updated_at: row.updated_at,
                    system_prompt: row.system_prompt.clone(),
                })
                .collect();

            // Load existing agent sessions from SQLite into AppState on startup (PERS-05, gap closure).
            let agent_session_rows =
                persistence::queries::list_agent_sessions(db.conn()).unwrap_or_default();
            let agent_sessions: Vec<AgentSessionSummary> = agent_session_rows
                .iter()
                .map(|row| {
                    let step_count = persistence::queries::count_agent_steps(db.conn(), &row.id)
                        .unwrap_or(0) as u32;
                    AgentSessionSummary {
                        id: row.id.clone(),
                        title: row.title.clone(),
                        status: row.status.clone(),
                        backend_id: row.backend_id.clone(),
                        updated_at: row.updated_at,
                        step_count,
                        elapsed_secs: row.updated_at - row.created_at,
                    }
                })
                .collect();

            // Build initial AppState with conversations and agent sessions.
            // backends and active_backend_id will be set after router init below.
            let active_backend_id = active_id;
            let mut initial_state = AppState::default();
            initial_state.conversations = conversations;
            initial_state.agent_sessions = agent_sessions;
            initial_state.embedding_status = embedding_status;

            // Initialize FailoverRouter with health state loaded from SQLite
            let mut router = llm::FailoverRouter::new();
            let health_rows =
                persistence::queries::list_backend_health(db.conn()).unwrap_or_default();
            let now_instant = std::time::Instant::now();
            let now_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            for row in &health_rows {
                if row.state == "failed" {
                    let entry = router.health.entry(row.backend_id.clone()).or_default();
                    entry.consecutive_failures = row.consecutive_failures;
                    entry.last_failure = Some(now_instant);
                    // Restore backoff if still in the future
                    if let Some(until_millis) = row.backoff_until {
                        let remaining_millis = until_millis - now_millis;
                        if remaining_millis > 0 {
                            entry.state = llm::router::HealthState::Failed {
                                until: now_instant
                                    + std::time::Duration::from_millis(remaining_millis as u64),
                            };
                        }
                        // else: backoff expired -- leave as Healthy (default)
                    }
                }
            }

            // Override active_backend_id with default_backend_id from settings if set and valid
            let settings_default_bid =
                persistence::queries::get_setting(db.conn(), "default_backend_id")
                    .ok()
                    .flatten();
            let final_active_id = settings_default_bid
                .filter(|id| backends.iter().any(|b| &b.id == id))
                .or(active_backend_id.clone());

            // Rebuild backend summaries with live health state
            initial_state.backends = backends
                .iter()
                .map(|b| {
                    b.to_summary(
                        final_active_id.as_deref() == Some(b.id.as_str()),
                        router.health_status(&b.id),
                    )
                })
                .collect();
            initial_state.active_backend_id = final_active_id.clone();

            // Per D-02 (Phase 7): detect first launch by reading has_completed_onboarding.
            // Read before moving db into actor_state below.
            // MIGRATION_V5 seeds the setting as 'false' so this is always present on new installs.
            // On existing installs that completed onboarding, the value is 'true'.
            let has_completed =
                persistence::queries::get_setting(db.conn(), "has_completed_onboarding")
                    .ok()
                    .flatten()
                    .map(|v| v == "true")
                    .unwrap_or(false);
            if !has_completed {
                initial_state.router.current_screen = Screen::Onboarding {
                    step: OnboardingStep::Welcome,
                };
            }

            // Phase 8: load or create the HNSW vector index from disk.
            // On first launch (or in-memory tests), VectorIndex::new creates an empty index.
            let vector_index = rag::VectorIndex::new(&vector_data_dir).unwrap_or_else(|_e| {
                // Fallback: create index with empty path (no persistence for this run).
                rag::VectorIndex::new("").expect("fallback VectorIndex creation failed")
            });

            // Phase 8: load documents from SQLite into initial AppState.
            let doc_rows = persistence::queries::list_documents(db.conn()).unwrap_or_default();
            initial_state.documents = doc_rows
                .iter()
                .map(|row| DocumentSummary {
                    id: row.id.clone(),
                    name: row.name.clone(),
                    format: row.format.clone(),
                    size_bytes: row.size_bytes as u64,
                    ingestion_date: row.ingestion_date,
                    chunk_count: row.chunk_count as u64,
                })
                .collect();

            // Load cached attestation results so badges appear immediately at startup
            // rather than waiting for the async attestation task to complete.
            // Uses get_latest_for_backend (ignores tee_type) because the tee_type stored
            // in the cache may differ from BackendConfig.tee_type (e.g., an IntelTdx
            // backend that returns AmdSevSnp from its attestation endpoint).
            let cached_attested_tls_public_keys = {
                let cache = attestation::cache::AttestationCache::new(db.conn());
                let mut map = HashMap::new();
                for b in &backends {
                    if let Ok(Some(record)) = cache.get_latest_for_backend(&b.id) {
                        initial_state
                            .attestation_statuses
                            .push(AttestationStatusEntry {
                                backend_id: b.id.clone(),
                                status: record.status,
                            });
                        if let Ok(fp) = attestation::endpoint::extract_tls_public_key_fp_from_report(
                            &record.tee_type,
                            &record.report_blob,
                        ) {
                            map.insert(b.id.clone(), fp);
                        }
                    }
                }
                map
            };

            // Load VCEK DER bytes from SQLite into the in-memory VCEK cache.
            // This pre-warms the cache so the first attestation run after process restart
            // does not need to hit AMD KDS again (avoiding rate-limit 429s).
            let vcek_cache_map: std::collections::HashMap<String, Vec<u8>> = {
                let mut map = std::collections::HashMap::new();
                let rows: Vec<(String, Vec<u8>)> = db
                    .conn()
                    .prepare("SELECT vcek_url, der FROM vcek_cert_cache")
                    .and_then(|mut stmt| {
                        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                            .and_then(|mapped| mapped.collect())
                    })
                    .unwrap_or_default();
                for (url, der) in rows {
                    map.insert(url, der);
                }
                log::info!(target: "attestation", "[attestation] loaded {} VCEK entries from SQLite cache", map.len());
                map
            };
            let vcek_cache: attestation::task::CertificateCache =
                Arc::new(std::sync::RwLock::new(vcek_cache_map));

            let embedding_provider_arc: Arc<dyn EmbeddingProvider> = Arc::from(embedding_provider);

            let mut actor_state = ActorState {
                app_state: initial_state,
                backends,
                attested_tls_public_keys: cached_attested_tls_public_keys,
                active_stream_token: None,
                runtime,
                db,
                keychain,
                pending_attachment: None,
                router,
                current_streaming_backend_id: None,
                failover_exclude: vec![],
                embedding_provider: embedding_provider_arc,
                vector_index,
                pending_rag_doc_count: None,
                active_agent_sessions: HashMap::new(),
                attestation_timer_token: None,
                vcek_cache,
            };

            // Initialize per-backend concurrency semaphores from max_concurrent_requests.
            // One Semaphore per backend; streaming tasks acquire a permit before sending HTTP.
            for backend in &actor_state.backends {
                actor_state.router.init_semaphore(
                    &backend.id,
                    backend.max_concurrent_requests as usize,
                );
            }

            // Load the attestation interval setting (default 15 minutes if not set).
            let attestation_interval_minutes: u32 = persistence::queries::get_setting(
                actor_state.db.conn(),
                "attestation_interval_minutes",
            )
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(15);
            actor_state.app_state.attestation_interval_minutes = attestation_interval_minutes;

            // Load the global system prompt setting.
            let global_system_prompt = persistence::queries::get_setting(
                actor_state.db.conn(),
                "global_system_prompt",
            )
            .ok()
            .flatten()
            .and_then(|v| if v.trim().is_empty() { None } else { Some(v) });
            actor_state.app_state.global_system_prompt = global_system_prompt;

            // Per D-03: auto-trigger attestation for the active backend on init.
            if let Some(active_id) = &final_active_id {
                if let Some(backend) = actor_state.backends.iter().find(|b| &b.id == active_id) {
                    let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                        .unwrap_or_else(|e| {
                            log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                            crate::attestation::TeePolicy::default()
                        });
                    attestation::spawn_attestation_task(
                        &actor_state.runtime,
                        backend,
                        core_tx_for_thread.clone(),
                        Arc::clone(&actor_state.vcek_cache),
                        tee_policy,
                    );
                }
            }

            // Start the periodic attestation timer.
            actor_state.attestation_timer_token = spawn_attestation_timer(
                &actor_state.runtime,
                attestation_interval_minutes,
                core_tx_for_thread.clone(),
            );

            // Emit a state snapshot: write to shared RwLock and send via channel
            let emit =
                |state: &AppState, shared: &Arc<RwLock<AppState>>, tx: &Sender<AppUpdate>| {
                    let snapshot = state.clone();
                    match shared.write() {
                        Ok(mut g) => *g = snapshot.clone(),
                        Err(p) => *p.into_inner() = snapshot.clone(),
                    }
                    let _ = tx.send(AppUpdate::FullState(snapshot));
                };

            // Emit initial state so UI can render before first action
            emit(&actor_state.app_state, &shared_for_core, &update_tx);

            while let Ok(msg) = core_rx.recv() {
                match msg {
                    CoreMsg::Action(action) => {
                        match action {
                            AppAction::PushScreen { screen } => {
                                actor_state
                                    .app_state
                                    .router
                                    .screen_stack
                                    .push(screen.clone());
                                actor_state.app_state.router.current_screen = screen;
                            }
                            AppAction::PopScreen => {
                                actor_state.app_state.router.screen_stack.pop();
                                actor_state.app_state.router.current_screen = actor_state
                                    .app_state
                                    .router
                                    .screen_stack
                                    .last()
                                    .cloned()
                                    .unwrap_or(Screen::Home);
                            }
                            AppAction::SetBusyState { state: busy } => {
                                actor_state.app_state.busy_state = busy;
                            }
                            AppAction::ShowToast { message } => {
                                actor_state.app_state.toast = Some(message);
                            }
                            AppAction::ClearToast => {
                                actor_state.app_state.toast = None;
                            }
                            AppAction::Noop => {
                                // Proof-of-life: no state mutation, just increment rev below
                            }
                            AppAction::SendMessage { text } => {
                                // Per D-17: clear the first-chat welcome placeholder on first send.
                                actor_state.app_state.show_first_chat_placeholder = false;
                                // If no active conversation, create one first (auto-create per D-04)
                                if actor_state.app_state.current_conversation_id.is_none() {
                                    let conv_id = new_uuid();
                                    let now = now_secs();
                                    // Auto-title from first 50 chars of the user message
                                    let title = truncate_title(&text, 50);
                                    let active_backend_id = actor_state
                                        .app_state
                                        .active_backend_id
                                        .clone()
                                        .unwrap_or_default();
                                    let model = actor_state
                                        .backends
                                        .iter()
                                        .find(|b| b.id == active_backend_id)
                                        .and_then(|b| b.models.first().cloned())
                                        .unwrap_or_default();
                                    let row = persistence::ConversationRow {
                                        id: conv_id.clone(),
                                        title,
                                        model_id: model,
                                        backend_id: active_backend_id,
                                        system_prompt: None,
                                        created_at: now,
                                        updated_at: now,
                                    };
                                    let _ = persistence::queries::insert_conversation(
                                        actor_state.db.conn(),
                                        &row,
                                    );
                                    actor_state.app_state.current_conversation_id =
                                        Some(conv_id.clone());
                                    actor_state.app_state.messages = vec![];
                                    actor_state.app_state.router.current_screen = Screen::Chat {
                                        conversation_id: conv_id,
                                    };
                                    refresh_conversations(&mut actor_state);
                                }
                                do_send_message(&mut actor_state, text, &core_tx_for_thread);
                            }
                            AppAction::StopGeneration => {
                                // Signal the streaming task to stop cooperatively.
                                // Do NOT set BusyState::Idle here -- wait for the
                                // StreamCancelled InternalEvent to confirm the task exited.
                                if let Some(token) = actor_state.active_stream_token.take() {
                                    token.cancel();
                                }
                            }
                            AppAction::SetActiveBackend { backend_id } => {
                                if actor_state.backends.iter().any(|b| b.id == backend_id) {
                                    actor_state.app_state.active_backend_id =
                                        Some(backend_id.clone());
                                    refresh_backend_summaries(&mut actor_state);
                                    // Per D-01: trigger attestation on backend switch
                                    if let Some(backend) =
                                        actor_state.backends.iter().find(|b| b.id == backend_id)
                                    {
                                        let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                                            .unwrap_or_else(|e| {
                                                log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                                                crate::attestation::TeePolicy::default()
                                            });
                                        attestation::spawn_attestation_task(
                                            &actor_state.runtime,
                                            backend,
                                            core_tx_for_thread.clone(),
                                            Arc::clone(&actor_state.vcek_cache),
                                            tee_policy,
                                        );
                                    }
                                    // Reset the periodic timer so the next tick is relative
                                    // to this switch, not the old schedule.
                                    if let Some(token) = actor_state.attestation_timer_token.take()
                                    {
                                        token.cancel();
                                    }
                                    let interval =
                                        actor_state.app_state.attestation_interval_minutes;
                                    actor_state.attestation_timer_token = spawn_attestation_timer(
                                        &actor_state.runtime,
                                        interval,
                                        core_tx_for_thread.clone(),
                                    );
                                    actor_state.app_state.rev += 1;
                                    emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                }
                            }

                            // ── Phase 5 action handlers ───────────────────────
                            AppAction::NewConversation => {
                                let conv_id = new_uuid();
                                let now = now_secs();
                                // Prefer default_backend_id from settings, fall back to active
                                let default_backend = persistence::queries::get_setting(
                                    actor_state.db.conn(),
                                    "default_backend_id",
                                )
                                .ok()
                                .flatten()
                                .or(actor_state.app_state.active_backend_id.clone())
                                .unwrap_or_default();
                                let default_model = persistence::queries::get_setting(
                                    actor_state.db.conn(),
                                    "default_model_id",
                                )
                                .ok()
                                .flatten()
                                .or_else(|| {
                                    actor_state
                                        .backends
                                        .iter()
                                        .find(|b| b.id == default_backend)
                                        .and_then(|b| b.models.first().cloned())
                                })
                                .unwrap_or_default();
                                let row = persistence::ConversationRow {
                                    id: conv_id.clone(),
                                    title: "New Conversation".to_string(),
                                    model_id: default_model,
                                    backend_id: default_backend,
                                    system_prompt: None,
                                    created_at: now,
                                    updated_at: now,
                                };
                                let _ = persistence::queries::insert_conversation(
                                    actor_state.db.conn(),
                                    &row,
                                );
                                refresh_conversations(&mut actor_state);
                                actor_state.app_state.current_conversation_id =
                                    Some(conv_id.clone());
                                actor_state.app_state.messages = vec![];
                                actor_state.app_state.router.current_screen = Screen::Chat {
                                    conversation_id: conv_id,
                                };
                            }

                            AppAction::LoadConversation { conversation_id } => {
                                refresh_messages(&mut actor_state, &conversation_id);
                                actor_state.app_state.current_conversation_id =
                                    Some(conversation_id.clone());
                                // Phase 8: load attached document IDs for this conversation.
                                let attached_docs =
                                    persistence::queries::get_conversation_attached_docs(
                                        actor_state.db.conn(),
                                        &conversation_id,
                                    )
                                    .unwrap_or_default();
                                actor_state.app_state.current_conversation_attached_docs =
                                    attached_docs;
                                actor_state.app_state.router.current_screen =
                                    Screen::Chat { conversation_id };
                            }

                            AppAction::RenameConversation { id, title } => {
                                let now = now_secs();
                                let _ = persistence::queries::rename_conversation(
                                    actor_state.db.conn(),
                                    &id,
                                    &title,
                                    now,
                                );
                                refresh_conversations(&mut actor_state);
                            }

                            AppAction::DeleteConversation { id } => {
                                let _ = persistence::queries::delete_conversation(
                                    actor_state.db.conn(),
                                    &id,
                                );
                                refresh_conversations(&mut actor_state);
                                // If this was the active conversation, go home and clear messages
                                if actor_state.app_state.current_conversation_id == Some(id.clone())
                                {
                                    actor_state.app_state.current_conversation_id = None;
                                    actor_state.app_state.messages = vec![];
                                    actor_state.app_state.router.current_screen = Screen::Home;
                                }
                            }

                            AppAction::RetryLastMessage => {
                                let conv_id = match actor_state
                                    .app_state
                                    .current_conversation_id
                                    .clone()
                                {
                                    Some(id) => id,
                                    None => {
                                        actor_state.app_state.last_error =
                                            Some("No active conversation".into());
                                        // still bump rev and emit below
                                        actor_state.app_state.rev += 1;
                                        emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                        continue;
                                    }
                                };
                                // Find and remove the last assistant message
                                let last_assistant_pos = actor_state
                                    .app_state
                                    .messages
                                    .iter()
                                    .rposition(|m| m.role == "assistant");
                                if let Some(pos) = last_assistant_pos {
                                    let msg_id = actor_state.app_state.messages[pos].id.clone();
                                    actor_state.app_state.messages.remove(pos);
                                    let _ = persistence::queries::delete_message(
                                        actor_state.db.conn(),
                                        &msg_id,
                                    );
                                }
                                // Find last user message text to re-send
                                let last_user_text = actor_state
                                    .app_state
                                    .messages
                                    .iter()
                                    .rfind(|m| m.role == "user")
                                    .map(|m| m.content.clone());
                                if let Some(text) = last_user_text {
                                    // Remove the last user message too (do_send_message will re-insert)
                                    let last_user_pos = actor_state
                                        .app_state
                                        .messages
                                        .iter()
                                        .rposition(|m| m.role == "user");
                                    if let Some(pos) = last_user_pos {
                                        let uid = actor_state.app_state.messages[pos].id.clone();
                                        actor_state.app_state.messages.remove(pos);
                                        let _ = persistence::queries::delete_message(
                                            actor_state.db.conn(),
                                            &uid,
                                        );
                                    }
                                    // Keep conv_id active and re-send
                                    actor_state.app_state.current_conversation_id = Some(conv_id);
                                    do_send_message(&mut actor_state, text, &core_tx_for_thread);
                                }
                            }

                            AppAction::EditMessage {
                                message_id,
                                new_text,
                            } => {
                                let conv_id = match actor_state
                                    .app_state
                                    .current_conversation_id
                                    .clone()
                                {
                                    Some(id) => id,
                                    None => {
                                        actor_state.app_state.last_error =
                                            Some("No active conversation".into());
                                        actor_state.app_state.rev += 1;
                                        emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                        continue;
                                    }
                                };
                                // Find the edited message's created_at
                                let edited_at = actor_state
                                    .app_state
                                    .messages
                                    .iter()
                                    .find(|m| m.id == message_id)
                                    .map(|m| m.created_at);
                                if let Some(at) = edited_at {
                                    // Delete messages after the edited point
                                    let _ = persistence::queries::delete_messages_after(
                                        actor_state.db.conn(),
                                        &conv_id,
                                        at,
                                    );
                                    // Delete the edited message itself
                                    let _ = persistence::queries::delete_message(
                                        actor_state.db.conn(),
                                        &message_id,
                                    );
                                    // Rebuild in-memory message list from DB
                                    refresh_messages(&mut actor_state, &conv_id);
                                    // Re-send with the new text
                                    do_send_message(
                                        &mut actor_state,
                                        new_text,
                                        &core_tx_for_thread,
                                    );
                                }
                            }

                            AppAction::AttachFile {
                                filename,
                                content,
                                size_bytes,
                            } => {
                                let size_display = format_size_display(size_bytes);
                                actor_state.pending_attachment = Some(PendingAttachment {
                                    filename: filename.clone(),
                                    content,
                                });
                                actor_state.app_state.pending_attachment = Some(AttachmentInfo {
                                    filename,
                                    size_display,
                                });
                            }

                            AppAction::ClearAttachment => {
                                actor_state.pending_attachment = None;
                                actor_state.app_state.pending_attachment = None;
                            }

                            AppAction::SelectModel { model_id } => {
                                if let Some(conv_id) =
                                    actor_state.app_state.current_conversation_id.clone()
                                {
                                    let now = now_secs();
                                    let _ = persistence::queries::update_conversation_model(
                                        actor_state.db.conn(),
                                        &conv_id,
                                        &model_id,
                                        now,
                                    );
                                    refresh_conversations(&mut actor_state);
                                }
                            }

                            AppAction::SetSystemPrompt { prompt } => {
                                if let Some(conv_id) =
                                    actor_state.app_state.current_conversation_id.clone()
                                {
                                    let now = now_secs();
                                    let _ = persistence::queries::update_conversation_system_prompt(
                                        actor_state.db.conn(),
                                        &conv_id,
                                        prompt.as_deref(),
                                        now,
                                    );
                                }
                            }

                            // ── Phase 6 action handlers ────────────────────────
                            AppAction::AddBackend {
                                name,
                                base_url,
                                api_key,
                                tee_type,
                                models,
                            } => {
                                let id = new_uuid();
                                let now = now_secs();
                                let model_list = serde_json::to_string(&models)
                                    .unwrap_or_else(|_| "[]".to_string());
                                let next_order = actor_state.backends.len() as i64;
                                let tee_str = format!("{:?}", tee_type);
                                let row = persistence::BackendRow {
                                    id: id.clone(),
                                    name: name.clone(),
                                    base_url,
                                    model_list,
                                    tee_type: tee_str,
                                    display_order: next_order,
                                    is_active: 0,
                                    created_at: now,
                                    max_concurrent_requests: 5,
                                    supports_tool_use: true,
                                };
                                let _ = persistence::queries::insert_backend(
                                    actor_state.db.conn(),
                                    &row,
                                );
                                actor_state.keychain.store(
                                    "mango".to_string(),
                                    id.clone(),
                                    api_key,
                                );
                                reload_backends(&mut actor_state);
                                // If no active backend exists yet, auto-promote this first backend
                                // to default so the app is immediately usable without a manual
                                // "Set Default" click (Issue 4: first added backend becomes default).
                                if actor_state.app_state.active_backend_id.is_none() {
                                    let _ = persistence::queries::set_setting(
                                        actor_state.db.conn(),
                                        "default_backend_id",
                                        &id,
                                    );
                                    actor_state.app_state.active_backend_id = Some(id.clone());
                                }
                                refresh_backend_summaries(&mut actor_state);
                                actor_state.app_state.toast =
                                    Some(format!("Added backend: {} (verifying…)", name));
                                // Auto-probe the new backend so health state is known immediately.
                                // Also spawn attestation so the settings row shows attestation status.
                                if let Some(b) = actor_state.backends.iter().find(|b| b.id == id) {
                                    spawn_health_check(
                                        &actor_state.runtime,
                                        b.id.clone(),
                                        b.base_url.clone(),
                                        b.api_key.clone(),
                                        pinned_tls_public_key_fp_for_backend(&actor_state, &b.id),
                                        core_tx_for_thread.clone(),
                                    );
                                    let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                                        .unwrap_or_else(|e| {
                                            log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                                            crate::attestation::TeePolicy::default()
                                        });
                                    attestation::spawn_attestation_task(
                                        &actor_state.runtime,
                                        b,
                                        core_tx_for_thread.clone(),
                                        Arc::clone(&actor_state.vcek_cache),
                                        tee_policy,
                                    );
                                }
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }

                            AppAction::RemoveBackend { backend_id } => {
                                // Determine replacement before deleting (first remaining backend
                                // that is not the one being removed).
                                let replacement_id: Option<String> = actor_state
                                    .backends
                                    .iter()
                                    .find(|b| b.id != backend_id)
                                    .map(|b| b.id.clone());

                                // Reassign conversations on this backend to the replacement (if any)
                                if let Some(ref repl_id) = replacement_id {
                                    let conv_ids: Vec<String> = actor_state
                                        .app_state
                                        .conversations
                                        .iter()
                                        .filter(|c| c.backend_id == backend_id)
                                        .map(|c| c.id.clone())
                                        .collect();
                                    for cid in conv_ids {
                                        let _ = persistence::queries::update_conversation_backend(
                                            actor_state.db.conn(),
                                            &cid,
                                            repl_id,
                                            now_secs(),
                                        );
                                    }
                                }
                                let _ = persistence::queries::delete_backend(
                                    actor_state.db.conn(),
                                    &backend_id,
                                );
                                let _ = persistence::queries::delete_backend_health(
                                    actor_state.db.conn(),
                                    &backend_id,
                                );
                                actor_state
                                    .keychain
                                    .delete("mango".to_string(), backend_id.clone());
                                reload_backends(&mut actor_state);
                                // If the removed backend was the active/default one, auto-promote
                                // the first remaining backend so the app stays in a valid state
                                // (Issue 3: deleting the default provider should not leave a void).
                                let was_active = actor_state.app_state.active_backend_id.as_deref()
                                    == Some(&backend_id);
                                if was_active {
                                    match replacement_id {
                                        Some(ref repl_id) => {
                                            let _ = persistence::queries::set_setting(
                                                actor_state.db.conn(),
                                                "default_backend_id",
                                                repl_id,
                                            );
                                            actor_state.app_state.active_backend_id =
                                                Some(repl_id.clone());
                                        }
                                        None => {
                                            // No backends remain -- clear active and default
                                            let _ = persistence::queries::set_setting(
                                                actor_state.db.conn(),
                                                "default_backend_id",
                                                "",
                                            );
                                            actor_state.app_state.active_backend_id = None;
                                        }
                                    }
                                }
                                refresh_backend_summaries(&mut actor_state);
                                refresh_conversations(&mut actor_state);
                            }

                            AppAction::ReorderBackend {
                                backend_id,
                                new_display_order,
                            } => {
                                let _ = persistence::queries::update_backend_display_order(
                                    actor_state.db.conn(),
                                    &backend_id,
                                    new_display_order,
                                );
                                reload_backends(&mut actor_state);
                                refresh_backend_summaries(&mut actor_state);
                            }

                            AppAction::UpdateBackendModels { backend_id, models } => {
                                let model_list = serde_json::to_string(&models)
                                    .unwrap_or_else(|_| "[]".to_string());
                                let _ = persistence::queries::update_backend_models(
                                    actor_state.db.conn(),
                                    &backend_id,
                                    &model_list,
                                );
                                reload_backends(&mut actor_state);
                                refresh_backend_summaries(&mut actor_state);
                            }

                            AppAction::SetDefaultBackend { backend_id } => {
                                let _ = persistence::queries::set_setting(
                                    actor_state.db.conn(),
                                    "default_backend_id",
                                    &backend_id,
                                );
                                actor_state.app_state.active_backend_id = Some(backend_id.clone());
                                refresh_backend_summaries(&mut actor_state);
                                // Spawn attestation for the new default so the settings row
                                // shows attestation status without requiring SetActiveBackend.
                                if let Some(b) =
                                    actor_state.backends.iter().find(|b| b.id == backend_id)
                                {
                                    let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                                        .unwrap_or_else(|e| {
                                            log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                                            crate::attestation::TeePolicy::default()
                                        });
                                    attestation::spawn_attestation_task(
                                        &actor_state.runtime,
                                        b,
                                        core_tx_for_thread.clone(),
                                        Arc::clone(&actor_state.vcek_cache),
                                        tee_policy,
                                    );
                                }
                                // Reset the periodic timer so the next tick is relative
                                // to this backend switch, not the old schedule.
                                if let Some(token) = actor_state.attestation_timer_token.take() {
                                    token.cancel();
                                }
                                let interval = actor_state.app_state.attestation_interval_minutes;
                                actor_state.attestation_timer_token = spawn_attestation_timer(
                                    &actor_state.runtime,
                                    interval,
                                    core_tx_for_thread.clone(),
                                );
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }

                            AppAction::SetDefaultModel { model_id } => {
                                let _ = persistence::queries::set_setting(
                                    actor_state.db.conn(),
                                    "default_model_id",
                                    &model_id,
                                );
                            }

                            AppAction::OverrideConversationBackend {
                                conversation_id,
                                backend_id,
                            } => {
                                let now = now_secs();
                                let _ = persistence::queries::update_conversation_backend(
                                    actor_state.db.conn(),
                                    &conversation_id,
                                    &backend_id,
                                    now,
                                );
                                refresh_conversations(&mut actor_state);
                            }

                            // ── Phase 7 action handlers ────────────────────────
                            AppAction::NextOnboardingStep => {
                                // Advance the wizard step in order.
                                // BackendSetup -> AttestationDemo only if at least one backend has an api_key.
                                let next = match &actor_state.app_state.router.current_screen {
                                    Screen::Onboarding { step } => match step {
                                        OnboardingStep::Welcome => {
                                            Some(OnboardingStep::BackendSetup)
                                        }
                                        OnboardingStep::BackendSetup => {
                                            // Require at least one backend with a non-empty api_key
                                            let has_key = actor_state
                                                .backends
                                                .iter()
                                                .any(|b| !b.api_key.is_empty());
                                            if has_key {
                                                Some(OnboardingStep::AttestationDemo)
                                            } else {
                                                None // no-op: no api key set yet
                                            }
                                        }
                                        OnboardingStep::AttestationDemo => {
                                            Some(OnboardingStep::ReadyToChat)
                                        }
                                        OnboardingStep::ReadyToChat => None, // no-op: last step
                                    },
                                    _ => None, // not in wizard
                                };
                                if let Some(step) = next {
                                    actor_state.app_state.router.current_screen =
                                        Screen::Onboarding { step };
                                }
                            }

                            AppAction::PreviousOnboardingStep => {
                                // Retreat the wizard step. No-op on Welcome.
                                let prev = match &actor_state.app_state.router.current_screen {
                                    Screen::Onboarding { step } => match step {
                                        OnboardingStep::Welcome => None, // no-op
                                        OnboardingStep::BackendSetup => {
                                            Some(OnboardingStep::Welcome)
                                        }
                                        OnboardingStep::AttestationDemo => {
                                            Some(OnboardingStep::BackendSetup)
                                        }
                                        OnboardingStep::ReadyToChat => {
                                            Some(OnboardingStep::AttestationDemo)
                                        }
                                    },
                                    _ => None,
                                };
                                if let Some(step) = prev {
                                    actor_state.app_state.router.current_screen =
                                        Screen::Onboarding { step };
                                }
                            }

                            AppAction::UpdateBackendApiKey {
                                backend_id,
                                api_key,
                            } => {
                                // Persist the new API key to keychain and reload backends so
                                // subsequent ValidateApiKey uses the updated key.
                                if !api_key.is_empty() {
                                    actor_state.keychain.store(
                                        "mango".to_string(),
                                        backend_id.clone(),
                                        api_key,
                                    );
                                    reload_backends(&mut actor_state);
                                    refresh_backend_summaries(&mut actor_state);
                                }
                            }

                            AppAction::ValidateApiKey { backend_id } => {
                                // Start API key validation health check for the selected backend.
                                // Sets validating_api_key=true and clears any previous error.
                                actor_state.app_state.onboarding.validating_api_key = true;
                                actor_state.app_state.onboarding.api_key_error = None;
                                actor_state.app_state.onboarding.selected_backend_id =
                                    Some(backend_id.clone());
                                // Find the backend config and spawn a health check
                                if let Some(b) =
                                    actor_state.backends.iter().find(|b| b.id == backend_id)
                                {
                                    spawn_health_check(
                                        &actor_state.runtime,
                                        b.id.clone(),
                                        b.base_url.clone(),
                                        b.api_key.clone(),
                                        pinned_tls_public_key_fp_for_backend(&actor_state, &b.id),
                                        core_tx_for_thread.clone(),
                                    );
                                }
                            }

                            AppAction::CompleteOnboarding => {
                                // Persist completion, create conversation, navigate to chat.
                                let _ = persistence::queries::set_setting(
                                    actor_state.db.conn(),
                                    "has_completed_onboarding",
                                    "true",
                                );
                                // Create a new conversation (same logic as NewConversation)
                                let conv_id = new_uuid();
                                let now = now_secs();
                                let default_backend = persistence::queries::get_setting(
                                    actor_state.db.conn(),
                                    "default_backend_id",
                                )
                                .ok()
                                .flatten()
                                .or(actor_state.app_state.active_backend_id.clone())
                                .unwrap_or_default();
                                let default_model = persistence::queries::get_setting(
                                    actor_state.db.conn(),
                                    "default_model_id",
                                )
                                .ok()
                                .flatten()
                                .or_else(|| {
                                    actor_state
                                        .backends
                                        .iter()
                                        .find(|b| b.id == default_backend)
                                        .and_then(|b| b.models.first().cloned())
                                })
                                .unwrap_or_default();
                                let row = persistence::ConversationRow {
                                    id: conv_id.clone(),
                                    title: "New Conversation".to_string(),
                                    model_id: default_model,
                                    backend_id: default_backend,
                                    system_prompt: None,
                                    created_at: now,
                                    updated_at: now,
                                };
                                let _ = persistence::queries::insert_conversation(
                                    actor_state.db.conn(),
                                    &row,
                                );
                                refresh_conversations(&mut actor_state);
                                actor_state.app_state.current_conversation_id =
                                    Some(conv_id.clone());
                                actor_state.app_state.messages = vec![];
                                actor_state.app_state.router.current_screen = Screen::Chat {
                                    conversation_id: conv_id,
                                };
                                // Set the D-17 welcome placeholder flag
                                actor_state.app_state.show_first_chat_placeholder = true;
                                // Clear onboarding transient state
                                actor_state.app_state.onboarding = OnboardingState::default();
                            }

                            AppAction::SkipOnboarding => {
                                // Mark onboarding complete without adding a provider.
                                // User will be taken to Home and can add providers from Settings later.
                                let _ = persistence::queries::set_setting(
                                    actor_state.db.conn(),
                                    "has_completed_onboarding",
                                    "true",
                                );
                                actor_state.app_state.router.current_screen = Screen::Home;
                                actor_state.app_state.onboarding = OnboardingState::default();
                            }

                            AppAction::AddBackendFromPreset { preset_id, api_key } => {
                                // Look up the preset by id.
                                let presets = llm::known_provider_presets();
                                if let Some(preset) = presets.iter().find(|p| p.id == preset_id) {
                                    // Always persist the API key to the keychain regardless of
                                    // whether the backend row already exists. Tinfoil is seeded by
                                    // MIGRATION_V1 so it already exists on first launch -- the old
                                    // `if !already` guard silently discarded the user's key for
                                    // this backend, causing api_key = "" and "chat completion
                                    // request body is empty" errors at send time.
                                    if !api_key.is_empty() {
                                        actor_state.keychain.store(
                                            "mango".to_string(),
                                            preset_id.clone(),
                                            api_key.clone(),
                                        );
                                    }

                                    let already =
                                        actor_state.backends.iter().any(|b| b.id == preset_id);
                                    if !already {
                                        let now = now_secs();
                                        let tee_str = format!("{:?}", preset.tee_type);
                                        let next_order = actor_state.backends.len() as i64;
                                        let row = persistence::BackendRow {
                                            id: preset_id.clone(),
                                            name: preset.name.clone(),
                                            base_url: preset.base_url.clone(),
                                            model_list: "[]".to_string(),
                                            tee_type: tee_str,
                                            display_order: next_order,
                                            is_active: 0,
                                            created_at: now,
                                            max_concurrent_requests: 5,
                                            supports_tool_use: true,
                                        };
                                        let _ = persistence::queries::insert_backend(
                                            actor_state.db.conn(),
                                            &row,
                                        );
                                    }

                                    // Reload backends so api_key is fresh in memory (whether the
                                    // backend was just inserted or already existed from migration).
                                    reload_backends(&mut actor_state);

                                    // Always promote the explicitly chosen backend to default.
                                    // The user selected this provider during onboarding, so their
                                    // intent should be honored even if a seeded backend (Tinfoil)
                                    // is already active.
                                    let _ = persistence::queries::set_setting(
                                        actor_state.db.conn(),
                                        "default_backend_id",
                                        &preset_id,
                                    );
                                    actor_state.app_state.active_backend_id =
                                        Some(preset_id.clone());

                                    // Spawn health check and attestation so the settings row
                                    // shows status immediately and models are populated.
                                    if let Some(b) =
                                        actor_state.backends.iter().find(|b| b.id == preset_id)
                                    {
                                        spawn_health_check(
                                            &actor_state.runtime,
                                            b.id.clone(),
                                            b.base_url.clone(),
                                            b.api_key.clone(),
                                            pinned_tls_public_key_fp_for_backend(
                                                &actor_state,
                                                &b.id,
                                            ),
                                            core_tx_for_thread.clone(),
                                        );
                                        let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                                            .unwrap_or_else(|e| {
                                                log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                                                crate::attestation::TeePolicy::default()
                                            });
                                        attestation::spawn_attestation_task(
                                            &actor_state.runtime,
                                            b,
                                            core_tx_for_thread.clone(),
                                            Arc::clone(&actor_state.vcek_cache),
                                            tee_policy,
                                        );
                                    }
                                    refresh_backend_summaries(&mut actor_state);
                                    if !already {
                                        actor_state.app_state.toast =
                                            Some(format!("Enabled: {}", preset.name));
                                    }
                                    actor_state.app_state.rev += 1;
                                    emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                }
                            }

                            // ── Phase 8 action handlers ────────────────────────
                            AppAction::IngestDocument { filename, content } => {
                                // Stage 1: set ingestion progress = "extracting"
                                actor_state.app_state.ingestion_progress =
                                    Some(IngestionProgress {
                                        document_name: filename.clone(),
                                        stage: "extracting".into(),
                                    });
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);

                                // Extract text
                                let text = match rag::extract_text_from_file(&filename, &content) {
                                    Ok(t) => t,
                                    Err(e) => {
                                        actor_state.app_state.ingestion_progress = None;
                                        actor_state.app_state.toast =
                                            Some(format!("Failed to extract text: {}", e));
                                        actor_state.app_state.rev += 1;
                                        emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                        continue;
                                    }
                                };

                                // Determine format from extension
                                let lower = filename.to_lowercase();
                                let format = if lower.ends_with(".pdf") {
                                    "pdf"
                                } else if lower.ends_with(".md") {
                                    "md"
                                } else {
                                    "txt"
                                };

                                // Generate document ID and insert into SQLite
                                let document_id = new_uuid();
                                let now = now_secs();
                                let doc_row = persistence::queries::DocumentRow {
                                    id: document_id.clone(),
                                    name: filename.clone(),
                                    format: format.to_string(),
                                    size_bytes: content.len() as i64,
                                    ingestion_date: now,
                                    chunk_count: 0,
                                };
                                if let Err(e) = persistence::queries::insert_document(
                                    actor_state.db.conn(),
                                    &doc_row,
                                ) {
                                    actor_state.app_state.ingestion_progress = None;
                                    actor_state.app_state.toast =
                                        Some(format!("Failed to save document: {}", e));
                                    actor_state.app_state.rev += 1;
                                    emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                    continue;
                                }

                                // Stage 2: chunking
                                actor_state.app_state.ingestion_progress =
                                    Some(IngestionProgress {
                                        document_name: filename.clone(),
                                        stage: "chunking".into(),
                                    });
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);

                                let chunks = rag::chunk_text(
                                    &text,
                                    rag::DEFAULT_MAX_TOKENS,
                                    rag::DEFAULT_OVERLAP_TOKENS,
                                );

                                // Insert each chunk into SQLite, collecting rowids
                                let mut chunk_rowids: Vec<i64> = Vec::new();
                                let mut chunk_texts: Vec<String> = Vec::new();
                                for (i, chunk) in chunks.iter().enumerate() {
                                    match persistence::queries::insert_chunk(
                                        actor_state.db.conn(),
                                        &document_id,
                                        i as i64,
                                        &chunk.text,
                                        chunk.char_offset as i64,
                                    ) {
                                        Ok(rowid) => {
                                            chunk_rowids.push(rowid);
                                            chunk_texts.push(chunk.text.clone());
                                        }
                                        Err(_) => {}
                                    }
                                }

                                // Update chunk_count
                                let _ = persistence::queries::update_document_chunk_count(
                                    actor_state.db.conn(),
                                    &document_id,
                                    chunk_rowids.len() as i64,
                                );

                                // Add DocumentSummary to AppState
                                actor_state.app_state.documents.push(DocumentSummary {
                                    id: document_id.clone(),
                                    name: filename.clone(),
                                    format: format.to_string(),
                                    size_bytes: content.len() as u64,
                                    ingestion_date: now,
                                    chunk_count: chunk_rowids.len() as u64,
                                });

                                // Stage 3: embedding
                                actor_state.app_state.ingestion_progress =
                                    Some(IngestionProgress {
                                        document_name: filename.clone(),
                                        stage: "embedding".into(),
                                    });
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);

                                // Dispatch async embedding via spawn_blocking
                                let provider = actor_state.embedding_provider.clone();
                                let doc_id_for_task = document_id.clone();
                                let rowids_for_task = chunk_rowids.clone();
                                let core_tx_clone = core_tx_for_thread.clone();
                                actor_state.runtime.spawn(async move {
                                    let embeddings = tokio::task::spawn_blocking(move || {
                                        provider.embed(chunk_texts)
                                    })
                                    .await
                                    .unwrap_or_default();
                                    let _ = core_tx_clone.send(CoreMsg::InternalEvent(Box::new(
                                        llm::InternalEvent::EmbeddingComplete {
                                            document_id: doc_id_for_task,
                                            chunk_rowids: rowids_for_task,
                                            embeddings,
                                        },
                                    )));
                                });
                                // Do NOT bump rev here -- EmbeddingComplete will do it.
                                continue;
                            }

                            AppAction::DeleteDocument { document_id } => {
                                // Collect chunk rowids before deleting (for usearch removal)
                                let rowids = persistence::queries::delete_chunks_for_document(
                                    actor_state.db.conn(),
                                    &document_id,
                                )
                                .unwrap_or_default();

                                // Remove vectors from usearch
                                for rowid in &rowids {
                                    let _ = actor_state.vector_index.remove(*rowid as u64);
                                }
                                if !rowids.is_empty() {
                                    let _ = actor_state.vector_index.save();
                                }

                                // Delete document from SQLite (chunks already deleted above)
                                let _ = persistence::queries::delete_document(
                                    actor_state.db.conn(),
                                    &document_id,
                                );

                                // Remove from AppState.documents
                                actor_state
                                    .app_state
                                    .documents
                                    .retain(|d| d.id != document_id);

                                // Remove from any conversation's attached_docs that reference it
                                let affected_convs: Vec<String> = actor_state
                                    .app_state
                                    .conversations
                                    .iter()
                                    .map(|c| c.id.clone())
                                    .collect();
                                for conv_id in affected_convs {
                                    let mut current_docs =
                                        persistence::queries::get_conversation_attached_docs(
                                            actor_state.db.conn(),
                                            &conv_id,
                                        )
                                        .unwrap_or_default();
                                    if current_docs.contains(&document_id) {
                                        current_docs.retain(|id| id != &document_id);
                                        let _ =
                                            persistence::queries::update_conversation_attached_docs(
                                                actor_state.db.conn(),
                                                &conv_id,
                                                &current_docs,
                                            );
                                    }
                                }

                                // Also clear from current_conversation_attached_docs if present
                                actor_state
                                    .app_state
                                    .current_conversation_attached_docs
                                    .retain(|id| id != &document_id);
                            }

                            AppAction::AttachDocumentToConversation { document_id } => {
                                let conv_id = match actor_state
                                    .app_state
                                    .current_conversation_id
                                    .clone()
                                {
                                    Some(id) => id,
                                    None => {
                                        actor_state.app_state.toast =
                                            Some("No active conversation".into());
                                        actor_state.app_state.rev += 1;
                                        emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                        continue;
                                    }
                                };
                                if !actor_state
                                    .app_state
                                    .current_conversation_attached_docs
                                    .contains(&document_id)
                                {
                                    actor_state
                                        .app_state
                                        .current_conversation_attached_docs
                                        .push(document_id.clone());
                                    let _ = persistence::queries::update_conversation_attached_docs(
                                        actor_state.db.conn(),
                                        &conv_id,
                                        &actor_state
                                            .app_state
                                            .current_conversation_attached_docs
                                            .clone(),
                                    );
                                }
                            }

                            AppAction::DetachDocumentFromConversation { document_id } => {
                                let conv_id = match actor_state
                                    .app_state
                                    .current_conversation_id
                                    .clone()
                                {
                                    Some(id) => id,
                                    None => {
                                        actor_state.app_state.toast =
                                            Some("No active conversation".into());
                                        actor_state.app_state.rev += 1;
                                        emit(&actor_state.app_state, &shared_for_core, &update_tx);
                                        continue;
                                    }
                                };
                                actor_state
                                    .app_state
                                    .current_conversation_attached_docs
                                    .retain(|id| id != &document_id);
                                let _ = persistence::queries::update_conversation_attached_docs(
                                    actor_state.db.conn(),
                                    &conv_id,
                                    &actor_state
                                        .app_state
                                        .current_conversation_attached_docs
                                        .clone(),
                                );
                            }

                            // ── Phase 9 action handlers (Task 1: stubs for compile; Task 2: full impl) ──
                            AppAction::LaunchAgentSession { task_description } => {
                                handle_launch_agent_session(
                                    &mut actor_state,
                                    task_description,
                                    &core_tx_for_thread,
                                );
                            }

                            AppAction::PauseAgentSession { session_id } => {
                                handle_pause_agent_session(&mut actor_state, session_id);
                            }

                            AppAction::ResumeAgentSession { session_id } => {
                                handle_resume_agent_session(
                                    &mut actor_state,
                                    session_id,
                                    &core_tx_for_thread,
                                );
                            }

                            AppAction::CancelAgentSession { session_id } => {
                                handle_cancel_agent_session(&mut actor_state, session_id);
                            }

                            AppAction::LoadAgentSession { session_id } => {
                                handle_load_agent_session(&mut actor_state, &session_id);
                            }

                            AppAction::ClearAgentDetail => {
                                actor_state.app_state.current_agent_session_id = None;
                                actor_state.app_state.current_agent_steps = vec![];
                            }

                            AppAction::SetAttestationInterval { minutes } => {
                                // Persist the new interval to settings.
                                let _ = persistence::queries::set_setting(
                                    actor_state.db.conn(),
                                    "attestation_interval_minutes",
                                    &minutes.to_string(),
                                );
                                actor_state.app_state.attestation_interval_minutes = minutes;
                                // Cancel the current timer and start a new one with the updated interval.
                                if let Some(token) = actor_state.attestation_timer_token.take() {
                                    token.cancel();
                                }
                                actor_state.attestation_timer_token = spawn_attestation_timer(
                                    &actor_state.runtime,
                                    minutes,
                                    core_tx_for_thread.clone(),
                                );
                            }

                            AppAction::SetGlobalSystemPrompt { prompt } => {
                                match &prompt {
                                    Some(p) if !p.trim().is_empty() => {
                                        let trimmed = p.trim().to_string();
                                        let _ = persistence::queries::set_setting(
                                            actor_state.db.conn(),
                                            "global_system_prompt",
                                            &trimmed,
                                        );
                                        actor_state.app_state.global_system_prompt = Some(trimmed);
                                    }
                                    _ => {
                                        // Clear: store empty string in DB, None in state.
                                        let _ = persistence::queries::set_setting(
                                            actor_state.db.conn(),
                                            "global_system_prompt",
                                            "",
                                        );
                                        actor_state.app_state.global_system_prompt = None;
                                    }
                                }
                            }
                        }

                        actor_state.app_state.rev += 1;
                        emit(&actor_state.app_state, &shared_for_core, &update_tx);
                    }

                    CoreMsg::InternalEvent(event) => {
                        match *event {
                            llm::InternalEvent::StreamChunk { token } => {
                                // Append token to the in-flight streaming_text buffer
                                let text = actor_state
                                    .app_state
                                    .streaming_text
                                    .get_or_insert_with(String::new);
                                text.push_str(&token);
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }
                            llm::InternalEvent::StreamDone => {
                                // Stream completed: persist the assistant message,
                                // update AppState.messages, clear streaming_text.
                                let content = actor_state
                                    .app_state
                                    .streaming_text
                                    .take()
                                    .unwrap_or_default();

                                if let Some(conv_id) =
                                    actor_state.app_state.current_conversation_id.clone()
                                {
                                    let now = now_secs();
                                    let msg_id = new_uuid();
                                    let row = persistence::MessageRow {
                                        id: msg_id.clone(),
                                        conversation_id: conv_id.clone(),
                                        role: "assistant".to_string(),
                                        content: content.clone(),
                                        created_at: now,
                                        token_count: None,
                                    };
                                    let _ = persistence::queries::insert_message(
                                        actor_state.db.conn(),
                                        &row,
                                    );
                                    let rag_count = actor_state.pending_rag_doc_count.take();
                                    actor_state.app_state.messages.push(UiMessage {
                                        id: msg_id,
                                        role: "assistant".to_string(),
                                        content,
                                        created_at: now,
                                        has_attachment: false,
                                        attachment_name: None,
                                        rag_context_count: rag_count,
                                    });

                                    // Update conversation updated_at
                                    let _ = persistence::queries::update_conversation_updated_at(
                                        actor_state.db.conn(),
                                        &conv_id,
                                        now,
                                    );

                                    // Auto-title: if title is still the placeholder,
                                    // update to truncated text of first user message
                                    let is_placeholder = actor_state
                                        .app_state
                                        .conversations
                                        .iter()
                                        .find(|c| c.id == conv_id)
                                        .map(|c| c.title == "New Conversation")
                                        .unwrap_or(false);
                                    if is_placeholder {
                                        if let Some(first_user_text) = actor_state
                                            .app_state
                                            .messages
                                            .iter()
                                            .find(|m| m.role == "user")
                                            .map(|m| m.content.clone())
                                        {
                                            let new_title = truncate_title(&first_user_text, 50);
                                            let _ = persistence::queries::rename_conversation(
                                                actor_state.db.conn(),
                                                &conv_id,
                                                &new_title,
                                                now,
                                            );
                                        }
                                    }
                                    refresh_conversations(&mut actor_state);
                                }

                                // Phase 20: Capture backend ID before take() for extraction task
                                let extraction_backend_id =
                                    actor_state.current_streaming_backend_id.clone();

                                // Mark the streaming backend as healthy and clear failover state
                                if let Some(backend_id) =
                                    actor_state.current_streaming_backend_id.take()
                                {
                                    actor_state.router.mark_success(&backend_id);
                                    let _ = persistence::queries::upsert_backend_health(
                                        actor_state.db.conn(),
                                        &persistence::BackendHealthRow {
                                            backend_id,
                                            consecutive_failures: 0,
                                            last_failure_at: None,
                                            state: "healthy".to_string(),
                                            backoff_until: None,
                                        },
                                    );
                                }
                                actor_state.failover_exclude.clear();

                                actor_state.app_state.busy_state = BusyState::Idle;
                                actor_state.active_stream_token = None;
                                refresh_backend_summaries(&mut actor_state);

                                // Phase 20: Spawn background memory extraction (MEM-01, MEM-07)
                                if actor_state.app_state.current_conversation_id.is_some() {
                                    let messages_snapshot: Vec<(String, String)> = actor_state
                                        .app_state
                                        .messages
                                        .iter()
                                        .map(|m| (m.role.clone(), m.content.clone()))
                                        .collect();

                                    if memory::extract::should_extract(&messages_snapshot) {
                                        let bid = extraction_backend_id
                                            .as_ref()
                                            .or_else(|| {
                                                actor_state.app_state.active_backend_id.as_ref()
                                            });
                                        if let Some(bid) = bid {
                                            if let Some(backend) = actor_state
                                                .backends
                                                .iter()
                                                .find(|b| b.id == *bid)
                                                .cloned()
                                            {
                                                let conv_id = actor_state
                                                    .app_state
                                                    .current_conversation_id
                                                    .clone()
                                                    .unwrap();
                                                let model = actor_state
                                                    .app_state
                                                    .conversations
                                                    .iter()
                                                    .find(|c| c.id == conv_id)
                                                    .map(|c| c.model_id.clone())
                                                    .filter(|m| !m.is_empty())
                                                    .unwrap_or_else(|| {
                                                        backend
                                                            .models
                                                            .first()
                                                            .cloned()
                                                            .unwrap_or_default()
                                                    });
                                                let core_tx_clone =
                                                    core_tx_for_thread.clone();

                                                actor_state.runtime.spawn(async move {
                                                    let memories =
                                                        memory::extract::call_extraction_llm(
                                                            &backend,
                                                            &messages_snapshot,
                                                            &model,
                                                        )
                                                        .await
                                                        .unwrap_or_default();

                                                    let _ =
                                                        core_tx_clone.send(CoreMsg::InternalEvent(
                                                            Box::new(
                                                                llm::InternalEvent::MemoryExtractionComplete {
                                                                    conversation_id: conv_id,
                                                                    memories,
                                                                },
                                                            ),
                                                        ));
                                                });
                                            }
                                        }
                                    }
                                }

                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }
                            llm::InternalEvent::StreamError { error } => {
                                // Determine if this error class warrants failover
                                let is_rate_limited =
                                    matches!(&error, llm::LlmError::RateLimited { .. });
                                let should_failover = match &error {
                                    llm::LlmError::NetworkError { .. } => true,
                                    llm::LlmError::ApiError { status_code, .. } => {
                                        *status_code == 0 || *status_code >= 500
                                    }
                                    llm::LlmError::RateLimited { retry_after_secs, .. } => {
                                        // Mark 429-specific backoff (shorter curve than general failure)
                                        if let Some(failed_id) =
                                            &actor_state.current_streaming_backend_id
                                        {
                                            actor_state.router.mark_failed_429(
                                                failed_id,
                                                std::time::Instant::now(),
                                                *retry_after_secs,
                                            );
                                        }
                                        true // trigger failover attempt
                                    }
                                    _ => false, // AuthError, ModelNotFound: no failover
                                };

                                if should_failover {
                                    if let Some(failed_id) =
                                        actor_state.current_streaming_backend_id.take()
                                    {
                                        // Only call mark_failed for non-429 errors; mark_failed_429
                                        // was already called above in the RateLimited arm to avoid
                                        // double-incrementing consecutive_failures.
                                        if !is_rate_limited {
                                            actor_state
                                                .router
                                                .mark_failed(&failed_id, std::time::Instant::now());
                                        }
                                        actor_state.failover_exclude.push(failed_id.clone());

                                        // Persist failed health state to SQLite
                                        let backoff_until_val = if let Some(h) =
                                            actor_state.router.health.get(&failed_id)
                                        {
                                            match &h.state {
                                                llm::router::HealthState::Failed { until } => {
                                                    let dur = until
                                                        .duration_since(std::time::Instant::now());
                                                    Some(now_secs() + dur.as_millis() as i64)
                                                }
                                                _ => None,
                                            }
                                        } else {
                                            None
                                        };
                                        let consec = actor_state
                                            .router
                                            .health
                                            .get(&failed_id)
                                            .map(|h| h.consecutive_failures)
                                            .unwrap_or(1);
                                        let _ = persistence::queries::upsert_backend_health(
                                            actor_state.db.conn(),
                                            &persistence::BackendHealthRow {
                                                backend_id: failed_id.clone(),
                                                consecutive_failures: consec,
                                                last_failure_at: Some(now_secs()),
                                                state: "failed".to_string(),
                                                backoff_until: backoff_until_val,
                                            },
                                        );

                                        // Spawn a health probe so the backend recovers automatically
                                        if let Some(b) =
                                            actor_state.backends.iter().find(|b| b.id == failed_id)
                                        {
                                            spawn_health_check(
                                                &actor_state.runtime,
                                                b.id.clone(),
                                                b.base_url.clone(),
                                                b.api_key.clone(),
                                                pinned_tls_public_key_fp_for_backend(
                                                    &actor_state,
                                                    &b.id,
                                                ),
                                                core_tx_for_thread.clone(),
                                            );
                                        }

                                        // Try next backend in the failover chain
                                        let model_id = actor_state
                                            .app_state
                                            .current_conversation_id
                                            .as_ref()
                                            .and_then(|cid| {
                                                actor_state
                                                    .app_state
                                                    .conversations
                                                    .iter()
                                                    .find(|c| &c.id == cid)
                                            })
                                            .map(|c| c.model_id.clone())
                                            .unwrap_or_default();
                                        let exclude_refs: Vec<&str> = actor_state
                                            .failover_exclude
                                            .iter()
                                            .map(|s| s.as_str())
                                            .collect();
                                        let next = actor_state
                                            .router
                                            .select_backend(
                                                &actor_state.backends,
                                                actor_state.app_state.active_backend_id.as_deref(),
                                                &model_id,
                                                &exclude_refs,
                                            )
                                            .cloned();

                                        if let Some(next_backend) = next {
                                            actor_state.current_streaming_backend_id =
                                                Some(next_backend.id.clone());
                                            actor_state.app_state.streaming_text =
                                                Some(String::new());
                                            let chat_messages = build_chat_messages(&actor_state);
                                            let next_model = if !model_id.is_empty() {
                                                model_id.clone()
                                            } else {
                                                next_backend
                                                    .models
                                                    .first()
                                                    .cloned()
                                                    .unwrap_or_default()
                                            };
                                            actor_state.app_state.busy_state =
                                                BusyState::Streaming {
                                                    model: next_model.clone(),
                                                };
                                            let token = llm::spawn_streaming_task(
                                                &actor_state.runtime,
                                                &next_backend,
                                                &next_model,
                                                chat_messages,
                                                pinned_tls_public_key_fp_for_backend(
                                                    &actor_state,
                                                    &next_backend.id,
                                                ),
                                                core_tx_for_thread.clone(),
                                                actor_state
                                                    .router
                                                    .get_semaphore(&next_backend.id),
                                            );
                                            actor_state.active_stream_token = Some(token);
                                            refresh_backend_summaries(&mut actor_state);
                                            actor_state.app_state.rev += 1;
                                            emit(
                                                &actor_state.app_state,
                                                &shared_for_core,
                                                &update_tx,
                                            );
                                            continue; // skip the normal error handling below
                                        }
                                    }
                                }

                                // No failover or all exhausted: surface the error
                                actor_state.app_state.busy_state = BusyState::Idle;
                                actor_state.app_state.last_error = Some(error.display_message());
                                actor_state.active_stream_token = None;
                                actor_state.current_streaming_backend_id = None;
                                actor_state.failover_exclude.clear();
                                refresh_backend_summaries(&mut actor_state);
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }
                            llm::InternalEvent::StreamCancelled => {
                                // User-initiated stop -- return to Idle.
                                // streaming_text preserved as partial response.
                                actor_state.app_state.busy_state = BusyState::Idle;
                                actor_state.active_stream_token = None;
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }
                            llm::InternalEvent::AttestationResult(att_event) => {
                                // Destructure the attestation event into components.
                                let now_secs = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);

                                // record_opt: Some((record, report_blob, tls_public_key_fp, vcek_url, vcek_der)) on success
                                // failed_is_transient: true when the failure was a network/fetch error
                                // (not a genuine cryptographic verification failure).
                                let (backend_id, status, record_opt, failed_is_transient) =
                                    match att_event {
                                        attestation::AttestationEvent::Verified {
                                            backend_id,
                                            tee_type,
                                            report_blob,
                                            expires_at,
                                            tls_public_key_fp,
                                            vcek_url,
                                            vcek_der,
                                        } => {
                                            let record = attestation::AttestationRecord {
                                                backend_id: backend_id.clone(),
                                                tee_type: tee_type.clone(),
                                                status: AttestationStatus::Verified,
                                                report_blob: report_blob.clone(),
                                                verified_at: now_secs,
                                                expires_at,
                                            };
                                            (
                                                backend_id,
                                                AttestationStatus::Verified,
                                                Some((
                                                    record,
                                                    report_blob,
                                                    tls_public_key_fp,
                                                    vcek_url,
                                                    vcek_der,
                                                )),
                                                false, // not relevant for Verified
                                            )
                                        }
                                        attestation::AttestationEvent::Failed {
                                            backend_id,
                                            reason,
                                            is_transient,
                                        } => (
                                            backend_id,
                                            AttestationStatus::Failed { reason },
                                            None,
                                            is_transient,
                                        ),
                                    };

                                // Upsert into AppState.attestation_statuses.
                                //
                                // Stickiness guard: only transient process errors (network failures,
                                // rate-limits, collateral-fetch failures) preserve a previously-Verified
                                // status.  These errors mean we never reached the TEE report itself, so
                                // the prior verified state is still valid.
                                //
                                // Genuine verification failures (bad signature, measurement mismatch,
                                // nonce mismatch, invalid certificate chain) must always update the status
                                // regardless of the prior value — the TEE report was inspected and found
                                // to be invalid.
                                let statuses = &mut actor_state.app_state.attestation_statuses;
                                if let Some(entry) =
                                    statuses.iter_mut().find(|e| e.backend_id == backend_id)
                                {
                                    let should_update = match (&entry.status, &status) {
                                        // Always accept a new Verified result (updates expires_at).
                                        (_, AttestationStatus::Verified) => true,
                                        // Transient fetch/network error AND current status is Verified:
                                        // preserve the Verified status — the backend is probably fine,
                                        // we just couldn't reach AMD KDS / the attestation endpoint.
                                        (AttestationStatus::Verified, _) if failed_is_transient => {
                                            false
                                        }
                                        // Genuine verification failure: always downgrade, even from Verified.
                                        // Any other transition (Unverified→Failed, Expired→Failed, etc.): fine.
                                        _ => true,
                                    };
                                    if should_update {
                                        entry.status = status;
                                    } else {
                                        log::info!(
                                            target: "attestation",
                                            "[attestation] ignoring transient fetch failure for backend={} — keeping Verified status (is_transient=true)",
                                            backend_id
                                        );
                                    }
                                } else {
                                    statuses.push(AttestationStatusEntry {
                                        backend_id: backend_id.clone(),
                                        status,
                                    });
                                }

                                // Per Phase 7: if we are in the onboarding AttestationDemo step,
                                // update onboarding state with the attestation result.
                                if matches!(
                                    actor_state.app_state.router.current_screen,
                                    Screen::Onboarding {
                                        step: OnboardingStep::AttestationDemo
                                    }
                                ) {
                                    let onboarding_status = actor_state
                                        .app_state
                                        .attestation_statuses
                                        .iter()
                                        .find(|e| e.backend_id == backend_id)
                                        .map(|e| e.status.clone());
                                    if let Some(s) = onboarding_status {
                                        actor_state.app_state.onboarding.attestation_result =
                                            Some(s);
                                    }
                                    // Extract tee_label from the original event (stored in record if available)
                                    if let Some((ref record, _, _, _, _)) = record_opt {
                                        actor_state.app_state.onboarding.attestation_tee_label =
                                            Some(record.tee_type.clone());
                                    }
                                    actor_state.app_state.onboarding.attestation_stage = None;
                                }

                                // Persist to SQLite cache and update shared_reports
                                if let Some((record, blob, tls_public_key_fp, vcek_url, vcek_der)) =
                                    record_opt
                                {
                                    // Write attestation result to SQLite cache via transient AttestationCache.
                                    // Per Option A from Plan 04-02: AttestationCache<'a> is created
                                    // transiently from db.conn() to avoid self-referential lifetime.
                                    // The cache is a zero-cost thin wrapper -- no I/O at creation.
                                    let cache = attestation::cache::AttestationCache::new(
                                        actor_state.db.conn(),
                                    );
                                    let _ = cache.put(&record);
                                    if let Some(fp) = tls_public_key_fp {
                                        actor_state
                                            .attested_tls_public_keys
                                            .insert(record.backend_id.clone(), fp);
                                    }

                                    // Persist newly-fetched VCEK DER bytes to SQLite so future process
                                    // starts can warm the in-memory cache without hitting AMD KDS.
                                    // Only written when vcek_url/vcek_der are Some (i.e. fresh KDS fetch).
                                    if let (Some(url), Some(der)) = (vcek_url, vcek_der) {
                                        if let Ok(mut cache) = actor_state.vcek_cache.write() {
                                            cache.insert(url.clone(), der.clone());
                                        }
                                        let vcek_cached_at = now_secs as i64;
                                        let _ = actor_state.db.conn().execute(
                                            "INSERT OR REPLACE INTO vcek_cert_cache (vcek_url, der, cached_at) VALUES (?1, ?2, ?3)",
                                            rusqlite::params![url, der, vcek_cached_at],
                                        );
                                        log::debug!(
                                            target: "attestation",
                                            "[attestation] persisted VCEK DER to SQLite url={}", url
                                        );
                                    }

                                    // Write blob to shared HashMap for FFI access (D-12)
                                    match shared_reports_for_core.write() {
                                        Ok(mut g) => {
                                            g.insert(backend_id, blob);
                                        }
                                        Err(p) => {
                                            p.into_inner().insert(backend_id, blob);
                                        }
                                    }
                                } else {
                                    if !failed_is_transient {
                                        actor_state.attested_tls_public_keys.remove(&backend_id);
                                    }
                                    // Failed attestation: also clear attestation_stage for onboarding
                                    if matches!(
                                        actor_state.app_state.router.current_screen,
                                        Screen::Onboarding {
                                            step: OnboardingStep::AttestationDemo
                                        }
                                    ) {
                                        actor_state.app_state.onboarding.attestation_stage = None;
                                    }
                                }

                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }
                            llm::InternalEvent::HealthCheckResult {
                                backend_id,
                                success,
                                models,
                            } => {
                                // Per Phase 7: if we're in BackendSetup onboarding step,
                                // handle ValidateApiKey result.
                                let in_onboarding_backend_setup =
                                    matches!(
                                        actor_state.app_state.router.current_screen,
                                        Screen::Onboarding {
                                            step: OnboardingStep::BackendSetup
                                        }
                                    ) && actor_state.app_state.onboarding.validating_api_key
                                        && actor_state
                                            .app_state
                                            .onboarding
                                            .selected_backend_id
                                            .as_deref()
                                            == Some(backend_id.as_str());

                                if in_onboarding_backend_setup {
                                    actor_state.app_state.onboarding.validating_api_key = false;
                                    if success {
                                        // Advance to AttestationDemo and kick off attestation
                                        actor_state.app_state.router.current_screen =
                                            Screen::Onboarding {
                                                step: OnboardingStep::AttestationDemo,
                                            };
                                        actor_state.app_state.onboarding.attestation_stage =
                                            Some("Connecting to backend...".to_string());
                                        if let Some(b) =
                                            actor_state.backends.iter().find(|b| b.id == backend_id)
                                        {
                                            let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                                                .unwrap_or_else(|e| {
                                                    log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                                                    crate::attestation::TeePolicy::default()
                                                });
                                            attestation::spawn_attestation_task(
                                                &actor_state.runtime,
                                                b,
                                                core_tx_for_thread.clone(),
                                                Arc::clone(&actor_state.vcek_cache),
                                                tee_policy,
                                            );
                                        }
                                    } else {
                                        actor_state.app_state.onboarding.api_key_error = Some(
                                            "Could not connect. Check your API key and try again."
                                                .to_string(),
                                        );
                                    }
                                }

                                // Non-wizard health check logic runs unconditionally
                                if success {
                                    log::info!(target: "health_check", "[health_check] backend={} success model_count={}", backend_id, models.len());
                                    // Persist discovered models if the probe returned any
                                    if !models.is_empty() {
                                        let model_list = serde_json::to_string(&models)
                                            .unwrap_or_else(|_| "[]".to_string());
                                        let _ = persistence::queries::update_backend_models(
                                            actor_state.db.conn(),
                                            &backend_id,
                                            &model_list,
                                        );
                                        reload_backends(&mut actor_state);
                                    }
                                    actor_state
                                        .router
                                        .maybe_restore(&backend_id, std::time::Instant::now());
                                    actor_state.router.mark_success(&backend_id);
                                    let _ = persistence::queries::upsert_backend_health(
                                        actor_state.db.conn(),
                                        &persistence::BackendHealthRow {
                                            backend_id: backend_id.clone(),
                                            consecutive_failures: 0,
                                            last_failure_at: None,
                                            state: "healthy".to_string(),
                                            backoff_until: None,
                                        },
                                    );
                                } else {
                                    let failures = actor_state
                                        .router
                                        .health
                                        .get(&backend_id)
                                        .map(|h| h.consecutive_failures)
                                        .unwrap_or(0);
                                    log::warn!(target: "health_check", "[health_check] backend={} probe failed consecutive_failures={}", backend_id, failures);
                                }
                                refresh_backend_summaries(&mut actor_state);
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }
                            llm::InternalEvent::AgentStepComplete {
                                session_id,
                                step_number,
                                result,
                            } => {
                                handle_agent_step_complete(
                                    &mut actor_state,
                                    session_id,
                                    step_number,
                                    result,
                                    &core_tx_for_thread,
                                    &shared_for_core,
                                    &update_tx,
                                );
                                continue; // handler does its own emit
                            }

                            llm::InternalEvent::EmbeddingComplete {
                                document_id: _,
                                chunk_rowids,
                                embeddings,
                            } => {
                                // Add each chunk's embedding to the HNSW index.
                                // Embeddings are a flat Vec<f32> of length chunk_rowids.len() * EMBEDDING_DIM.
                                let dim = embedding::EMBEDDING_DIM;
                                for (i, rowid) in chunk_rowids.iter().enumerate() {
                                    let start = i * dim;
                                    let end = start + dim;
                                    if end <= embeddings.len() {
                                        let _ = actor_state
                                            .vector_index
                                            .add(*rowid as u64, &embeddings[start..end]);
                                    }
                                }
                                if !chunk_rowids.is_empty() {
                                    let _ = actor_state.vector_index.save();
                                }

                                // Clear ingestion progress and show success toast
                                actor_state.app_state.ingestion_progress = None;
                                actor_state.app_state.toast =
                                    Some("Document indexed successfully".into());
                                actor_state.app_state.rev += 1;
                                emit(&actor_state.app_state, &shared_for_core, &update_tx);
                            }

                            llm::InternalEvent::AttestationTick => {
                                // Periodic re-attestation: re-run attestation for the active backend.
                                if let Some(active_id) =
                                    actor_state.app_state.active_backend_id.clone()
                                {
                                    if let Some(backend) =
                                        actor_state.backends.iter().find(|b| b.id == active_id)
                                    {
                                        let tee_policy = crate::persistence::queries::get_tee_policy(actor_state.db.conn())
                                            .unwrap_or_else(|e| {
                                                log::warn!(target: "attestation", "Failed to load TEE policy, using defaults: {e}");
                                                crate::attestation::TeePolicy::default()
                                            });
                                        attestation::spawn_attestation_task(
                                            &actor_state.runtime,
                                            backend,
                                            core_tx_for_thread.clone(),
                                            Arc::clone(&actor_state.vcek_cache),
                                            tee_policy,
                                        );
                                    }
                                }
                                // No state mutation needed — attestation result arrives separately.
                                continue;
                            }

                            llm::InternalEvent::MemoryExtractionComplete {
                                conversation_id,
                                memories,
                            } => {
                                if !memories.is_empty() {
                                    let now = now_secs();
                                    let mut added_count = 0u32;
                                    for content in &memories {
                                        let id = new_uuid();
                                        let usearch_key =
                                            uuid::Uuid::new_v4().as_u128() as i64;

                                        let row = persistence::queries::MemoryRow {
                                            id,
                                            conversation_id: conversation_id.clone(),
                                            content: content.clone(),
                                            usearch_key,
                                            created_at: now,
                                        };
                                        if persistence::queries::insert_memory(
                                            actor_state.db.conn(),
                                            &row,
                                        )
                                        .is_ok()
                                        {
                                            // Embed and add to vector index
                                            let embedding = actor_state
                                                .embedding_provider
                                                .embed(vec![content.clone()]);
                                            if embedding.len()
                                                == crate::embedding::EMBEDDING_DIM
                                            {
                                                let _ = actor_state.vector_index.add(
                                                    usearch_key as u64,
                                                    &embedding,
                                                );
                                                added_count += 1;
                                            }
                                        }
                                    }
                                    if added_count > 0 {
                                        let _ = actor_state.vector_index.save();
                                    }
                                    log::info!(
                                        "[memory] extracted {} memories ({} embedded) from conv={}",
                                        memories.len(),
                                        added_count,
                                        conversation_id
                                    );
                                }
                                // No AppState rev increment -- memories are invisible in Phase 20 UI
                                continue;
                            }
                        }
                    }
                }
            }
        });

        Arc::new(Self {
            core_tx,
            update_rx,
            listening: AtomicBool::new(false),
            shared_state,
            shared_reports,
        })
    }

    /// Read the latest state snapshot from the shared RwLock.
    pub fn state(&self) -> AppState {
        match self.shared_state.read() {
            Ok(g) => g.clone(),
            Err(poison) => poison.into_inner().clone(),
        }
    }

    /// Dispatch an action to the actor loop.
    pub fn dispatch(&self, action: AppAction) {
        let _ = self.core_tx.send(CoreMsg::Action(action));
    }

    /// Return the raw attestation report blob for `backend_id`, if available.
    ///
    /// Per D-12: raw blobs are not carried in AppState (too large for frequent snapshots).
    /// This dedicated FFI method reads from the shared_reports HashMap populated by the
    /// actor when attestation verification succeeds.
    pub fn get_raw_attestation_report(&self, backend_id: String) -> Option<Vec<u8>> {
        match self.shared_reports.read() {
            Ok(g) => g.get(&backend_id).cloned(),
            Err(p) => p.into_inner().get(&backend_id).cloned(),
        }
    }

    /// Start listening for state updates and delivering them to the reconciler.
    /// Guard with AtomicBool so only one listener thread is spawned.
    pub fn listen_for_updates(&self, reconciler: Box<dyn AppReconciler>) {
        if self
            .listening
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let rx = self.update_rx.clone();
        thread::spawn(move || {
            while let Ok(update) = rx.recv() {
                reconciler.reconcile(update);
            }
        });
    }
}

#[cfg(test)]
impl FfiApp {
    /// Inject an InternalEvent directly into the actor loop for testing.
    /// This bypasses the HTTP/streaming layer and tests the actor's
    /// event processing logic in isolation.
    pub fn test_send_internal(&self, event: llm::InternalEvent) {
        let _ = self.core_tx.send(CoreMsg::InternalEvent(Box::new(event)));
    }
}

#[cfg(test)]
mod tests;
