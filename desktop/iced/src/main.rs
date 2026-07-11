use iced::widget::{center, column, row, text};
use iced::{Element, Subscription, Task, Theme};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use mango_core::embedding::desktop::DesktopEmbeddingProvider;
use mango_core::{
    AppAction, AppReconciler, AppState, AppUpdate, BackendRole, DesktopKeychainProvider,
    DirectoryFileEntry, FfiApp, NullBiometricProvider, NullEmbeddingProvider, OnboardingStep,
    Screen, TeeType,
};

mod lock_screen;
mod pin_setup_screen;
mod theme;
mod views;

// ── ThemeOverride ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum ThemeOverride {
    FollowSystem,
    ForceDark,
    ForceLight,
}

impl ThemeOverride {
    const ALL: &[ThemeOverride] = &[
        ThemeOverride::FollowSystem,
        ThemeOverride::ForceDark,
        ThemeOverride::ForceLight,
    ];
}

impl std::fmt::Display for ThemeOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeOverride::FollowSystem => write!(f, "Follow System"),
            ThemeOverride::ForceDark => write!(f, "Force Dark"),
            ThemeOverride::ForceLight => write!(f, "Force Light"),
        }
    }
}

// ── Preferences persistence ──────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct Preferences {
    theme_override: ThemeOverride,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme_override: ThemeOverride::FollowSystem,
        }
    }
}

fn preferences_path() -> std::path::PathBuf {
    let base = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| std::env::temp_dir())
    } else {
        // Linux: XDG_CONFIG_HOME or ~/.config
        std::env::var("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".config"))
                    .unwrap_or_else(|_| std::env::temp_dir())
            })
    };
    base.join("mango").join("preferences.json")
}

fn load_preferences() -> Preferences {
    let path = preferences_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn preset_allows_empty_api_key(preset_id: &str) -> bool {
    mango_core::known_provider_presets()
        .into_iter()
        .any(|preset| {
            preset.id == preset_id
                && (preset.id == "qvac-local" || preset.tee_type == TeeType::Unknown)
        })
}

fn action_clears_force_remote_next(action: &AppAction) -> bool {
    matches!(
        action,
        AppAction::SetActiveBackend { .. }
            | AppAction::NewConversation
            | AppAction::LoadConversation { .. }
            | AppAction::ForkConversation { .. }
            | AppAction::DeleteConversation { .. }
            | AppAction::DeleteAllConversations
            | AppAction::DeleteAllData
            | AppAction::SelectModel { .. }
            | AppAction::RemoveBackend { .. }
            | AppAction::SetDefaultBackend { .. }
            | AppAction::SaveHybridProfile { .. }
            | AppAction::DeleteHybridProfile { .. }
            | AppAction::SetActiveHybridProfile { .. }
            | AppAction::OverrideConversationBackend { .. }
    )
}

fn save_preferences(prefs: &Preferences) {
    let path = preferences_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(prefs) {
        let _ = std::fs::write(&path, json);
    }
}

#[allow(dead_code)]
/// Sanitize a conversation title into a safe filename stem.
///
/// Keeps ASCII alphanumerics, space, underscore, and hyphen. Everything else is
/// replaced with `_`. Runs of whitespace collapse to a single `_`. Truncates to
/// 60 bytes. Empty input falls back to "conversation".
fn sanitize_filename(title: &str) -> String {
    let collapsed: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else if c.is_whitespace() {
                '_'
            } else {
                '_'
            }
        })
        .collect();
    // Collapse runs of underscores introduced above.
    let mut out = String::with_capacity(collapsed.len());
    let mut prev_us = false;
    for c in collapsed.chars() {
        if c == '_' {
            if !prev_us {
                out.push('_');
            }
            prev_us = true;
        } else {
            out.push(c);
            prev_us = false;
        }
    }
    let trimmed = out.trim_matches(|c: char| c == '_' || c == '-').to_string();
    let bounded: String = trimmed.chars().take(60).collect();
    if bounded.is_empty() {
        "conversation".to_string()
    } else {
        bounded
    }
}

#[allow(dead_code)]
fn tee_type_to_str(tee: &TeeType) -> &'static str {
    match tee {
        TeeType::IntelTdx => "IntelTdx",
        TeeType::NvidiaH100Cc => "NvidiaH100Cc",
        TeeType::AmdSevSnp => "AmdSevSnp",
        TeeType::Unknown => "Unknown",
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Mango")
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}

// ── AppManager ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppManager {
    ffi: Arc<FfiApp>,
    update_rx: flume::Receiver<()>,
}

impl Hash for AppManager {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.ffi).hash(state);
    }
}

/// Return the platform-appropriate application data directory.
///
/// On macOS: `~/Library/Application Support/mango`
/// On Linux: `$XDG_DATA_HOME/mango` → `~/.local/share/mango` → `/tmp/mango` (fallback)
///
/// Using a user-owned directory rather than `/tmp` avoids world-readable exposure of
/// the bootstrap DB (which contains the wrapped DEK) on multi-user Linux systems (WR-01).
fn app_data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join("Library/Application Support/mango"))
            .unwrap_or_else(|_| std::env::temp_dir().join("mango"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| std::env::temp_dir())
            })
            .join("mango")
    }
}

fn stage_desktop_image_attachment(
    source: &std::path::Path,
    ext: &str,
) -> Result<std::path::PathBuf, String> {
    let staging_dir = app_data_dir().join("image-attachments");
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Failed to prepare image attachment: {e}"))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let staged_path = staging_dir.join(format!("desktop_{}_{}.{}", std::process::id(), nanos, ext));
    std::fs::copy(source, &staged_path)
        .map_err(|e| format!("Failed to read selected image: {e}"))?;
    Ok(staged_path.canonicalize().unwrap_or(staged_path))
}

impl AppManager {
    fn new() -> Result<Self, String> {
        let data_dir = app_data_dir().to_string_lossy().to_string();
        let _ = std::fs::create_dir_all(&data_dir);

        // Phase 8: use DesktopEmbeddingProvider (fastembed) with NullEmbeddingProvider fallback
        // if model loading fails (e.g. first run without model cache).
        // Phase 15: capture EmbeddingStatus so the UI can inform the user when degraded.
        let (embedding_provider, embedding_status): (
            Box<dyn mango_core::embedding::EmbeddingProvider>,
            mango_core::EmbeddingStatus,
        ) = match DesktopEmbeddingProvider::new() {
            Ok(ep) => (
                Box::new(ep) as Box<dyn mango_core::embedding::EmbeddingProvider>,
                mango_core::EmbeddingStatus::Active,
            ),
            Err(e) => {
                eprintln!("[documents] DesktopEmbeddingProvider init failed: {e}; falling back to NullEmbeddingProvider");
                (
                    Box::new(NullEmbeddingProvider)
                        as Box<dyn mango_core::embedding::EmbeddingProvider>,
                    mango_core::EmbeddingStatus::Degraded,
                )
            }
        };
        let ffi = FfiApp::new(
            data_dir,
            Box::new(DesktopKeychainProvider),
            embedding_provider,
            embedding_status,
            Box::new(mango_core::NullLocalLlmProvider),
            Box::new(NullBiometricProvider),
        );
        let (notify_tx, update_rx) = flume::unbounded();
        ffi.listen_for_updates(Box::new(DesktopReconciler { tx: notify_tx }));

        Ok(Self { ffi, update_rx })
    }

    fn state(&self) -> AppState {
        self.ffi.state()
    }

    fn dispatch(&self, action: AppAction) {
        self.ffi.dispatch(action);
    }

    fn subscribe_updates(&self) -> flume::Receiver<()> {
        self.update_rx.clone()
    }
}

struct DesktopReconciler {
    tx: flume::Sender<()>,
}

impl AppReconciler for DesktopReconciler {
    fn reconcile(&self, _update: AppUpdate) {
        let _ = self.tx.send(());
    }
}

fn manager_update_stream(manager: &AppManager) -> impl iced::futures::Stream<Item = ()> {
    let rx = manager.subscribe_updates();
    iced::futures::stream::unfold(rx, |rx| async move {
        match rx.recv_async().await {
            Ok(()) => Some(((), rx)),
            Err(_) => None,
        }
    })
}

// ── App ─────────────────────────────────────────────────────────────────────

#[allow(clippy::large_enum_variant)]
enum App {
    BootError {
        error: String,
    },
    Loaded {
        manager: AppManager,
        state: AppState,
        // iced-local state (not in AppState -- markdown::Content can't cross UniFFI boundary)
        streaming_content: iced::widget::markdown::Content,
        /// Tracks byte-length of state.streaming_text from last CoreUpdated for delta extraction
        prev_streaming_len: usize,
        input_text: String,
        system_prompt_text: String,
        show_system_prompt_input: bool,
        rename_state: Option<(String, String)>,
        edit_state: Option<(String, String)>,
        show_attestation_detail: bool,
        /// Pre-parsed markdown items for completed assistant messages (msg_id -> items)
        /// Per iced docs: store Vec<markdown::Item> in app state, not parsed in view()
        parsed_messages: HashMap<String, Vec<iced::widget::markdown::Item>>,
        // Settings form local state (not in AppState -- pure UI form fields)
        settings_add_name: String,
        settings_add_url: String,
        settings_add_key: String,
        settings_add_tee: String,
        settings_default_model: String,
        // Per-preset API key fields for the "Enable Provider" simple flow (preset_id -> api_key)
        settings_preset_keys: std::collections::HashMap<String, String>,
        // Whether the "Advanced: Add Custom Provider" section is expanded
        settings_show_advanced: bool,
        // Re-attestation interval input (local form state before dispatch)
        settings_attestation_interval: String,
        // Default instructions text (local form state before dispatch)
        settings_default_instructions: String,
        // Whether settings_default_instructions has been initialized from AppState
        settings_default_instructions_initialized: bool,
        // Onboarding wizard local state (not in AppState -- pure UI form fields)
        onboarding_selected_backend: String,
        onboarding_api_key: String,
        onboarding_show_learn_more: bool,
        // Documents attachment overlay local state (not in AppState -- pure UI)
        show_docs_attachment_overlay: bool,
        // Conversation options menu (Docs/Instructions/Tools panel) local state
        show_conv_menu: bool,
        // Hybrid routing one-shot override for the next submitted message.
        force_remote_next: bool,
        // Tools sub-panel within the conv menu (individual tool toggles)
        show_tools_panel: bool,
        // Memory edit state: (memory_id, current_edit_text) when user is editing a memory
        memory_edit_state: Option<(String, String)>,
        // Brave Search API key input field (local form state before dispatch)
        settings_brave_api_key: String,
        // Inline feedback message after Brave API key validation (success or error text)
        settings_brave_api_key_message: Option<String>,
        // OS dark/light theme state (updated via SystemThemeChanged subscription)
        is_dark: bool,
        // Cached theme derived from is_dark; updated whenever is_dark changes
        cached_theme: Theme,
        // Manual theme override preference (per D-06, D-07)
        theme_override: ThemeOverride,
        // Agent task input text (local form state before dispatch)
        agent_task_input: String,
        // Phase 28: lock screen PIN input (cleared after submit, T-28-23)
        lock_pin_input: String,
        // Phase 28: PIN setup screen inputs
        setup_pin_input: String,
        setup_confirm_input: String,
        setup_duress_input: String,
        // IMG-07: decrypted image thumbnails keyed by message_id
        image_cache: HashMap<String, iced::widget::image::Handle>,
        // Phase 32 DIR-05: directory sources view local state
        dir_editing_exclusions_for: Option<String>,
        dir_exclusion_edit_text: String,
        dir_exclusion_validation: HashMap<String, String>,
        dir_pending_remove_id: Option<String>,
        dir_watcher_warning: Option<String>,
        /// Receiver wired to `dir_watcher_tx`; a channel fed by the notify
        /// debouncer + 5-minute fallback ticker. Consumed by the iced
        /// subscription so sync triggers are processed on the main loop.
        dir_trigger_rx: flume::Receiver<Message>,
        /// Cloneable sender used by the startup watcher/ticker threads.
        dir_trigger_tx: flume::Sender<Message>,
        /// Set of source_ids currently being enumerated/synced to avoid
        /// overlapping pipelines (D-25).
        dir_in_flight: Arc<Mutex<HashSet<String>>>,
        /// Per-source absolute path cache for the watcher thread (source_id
        /// → path). Populated whenever the core emits an updated source list.
        dir_watched_paths: Arc<Mutex<HashMap<String, String>>>,

        // ── Phase 36 — Tool Discovery / Tool Detail UI-state ──
        /// Live search query for the Tool Discovery list (no debounce).
        contextvm_search_query: String,
        /// Selected provider filter for tool discovery (None = all providers).
        contextvm_provider_filter: Option<String>,
        /// Inline copy-confirmation status line shown on the Tool Detail
        /// screen for ~2 seconds after a Copy action.
        contextvm_copy_status: Option<String>,
        /// Whether the SCHEMA expander on the Tool Detail screen is open.
        contextvm_schema_expanded: bool,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone)]
enum Message {
    CoreUpdated,
    DispatchAction(AppAction),
    InputChanged(String),
    SubmitMessage,
    // Sidebar
    OpenConversation(String),
    StartRename(String, String),
    RenameChanged(String),
    SubmitRename,
    CancelRename,
    ConfirmDelete(String),
    // Chat
    CopyMessage(String),
    RetryMessage,
    StartEdit(String, String),
    EditChanged(String),
    SubmitEdit,
    CancelEdit,
    AttachFile,
    ClearAttachment,
    SelectModel(String),
    UseHybridProfile(String),
    ToggleForceRemoteNext,
    /// Fork the currently-loaded conversation (quick/260423-93w).
    ForkConversation,
    // System prompt (per CHAT-11 / D-09)
    ToggleSystemPromptInput,
    SystemPromptChanged(String),
    SubmitSystemPrompt,
    // Attestation
    ToggleAttestationDetail,
    // Markdown link clicked (Uri = String in iced 0.14)
    #[allow(dead_code)]
    MarkdownLinkClicked(String),
    // Settings form inputs
    SettingsAddNameChanged(String),
    SettingsAddUrlChanged(String),
    SettingsAddKeyChanged(String),
    SettingsAddTeeChanged(String),
    SettingsDefaultModelChanged(String),
    SettingsSubmitAddBackend {
        name: String,
        url: String,
        key: String,
        tee: TeeType,
    },
    // Simple "enable provider" flow: per-preset API key field changes
    SettingsPresetKeyChanged {
        preset_id: String,
        key: String,
    },
    // Submit the simple "enable provider" flow for a preset
    SettingsEnablePreset {
        preset_id: String,
    },
    // Toggle the Advanced custom provider section
    SettingsToggleAdvanced,
    // Re-attestation interval field changed
    SettingsAttestationIntervalChanged(String),
    // Apply the re-attestation interval from the input field
    SettingsApplyAttestationInterval,
    // Default instructions field changed
    SettingsDefaultInstructionsChanged(String),
    // Save the default instructions to the Rust core
    SettingsSaveDefaultInstructions,
    // Brave Search API key field changed
    SettingsBraveApiKeyChanged(String),
    // Save the Brave Search API key to the Rust core
    SettingsSaveBraveApiKey,
    // Toggle the memories enabled setting (Phase 25, MEM-TOGGLE-04)
    SettingsMemoriesEnabledToggled(bool),
    // Phase 35 — Settings → TOOLS → Auto-discover toggle flipped.
    SettingsAutoDiscoverToolsToggled(bool),
    // Phase 35 — Tool Discovery screen "Discover" / first-open trigger.
    ContextvmDiscoverToolsClicked,
    // Phase 35 — Tool Discovery "Try again" / refresh tap.
    ContextvmRetryClicked,
    // Phase 35 — per-tool Switch toggled in Tool Discovery list.
    ContextvmToolToggled {
        tool_id: String,
        enabled: bool,
    },
    // Phase 36 — Tool Discovery search filter input changed.
    ContextvmSearchChanged(String),
    /// Provider filter changed for tool discovery (None = all providers).
    ContextvmProviderFilterChanged(Option<String>),
    // Phase 36 — Copy actions on the Tool Detail sub-screen.
    CopyNpub(String),
    CopyHex(String),
    CopyToolId(String),
    // Phase 36 — Toggle the SCHEMA expander on the Tool Detail sub-screen.
    ToggleSchemaExpanded,
    // Phase 36 — Clear the inline copy-confirmation status line (fires ~2s
    // after a CopyNpub / CopyHex / CopyToolId Message via Task::perform).
    ClearCopyStatus,
    // Theme override preference changed (per D-07)
    SettingsThemeOverrideChanged(ThemeOverride),
    // Onboarding wizard messages
    OnboardingSelectBackend(String),
    OnboardingApiKeyChanged(String),
    OnboardingValidateKey,
    OnboardingNext,
    OnboardingBack,
    OnboardingComplete,
    OnboardingRetryAttestation,
    OnboardingToggleLearnMore,
    OnboardingSkip,
    OpenUrl(String),
    #[allow(dead_code)]
    RunSetupWizard,
    // Documents screen messages (Phase 8, LRAG-06)
    OpenDocuments,
    PickDocumentFile,
    DeleteDocument(String),
    ToggleDocumentAttachment(String),
    ToggleDocAttachmentOverlay,
    // Memory screen messages (Phase 23, MEM-04/05/06)
    #[allow(dead_code)]
    OpenMemories,
    MemoryStartEdit(String, String), // (memory_id, full_content)
    MemoryEditChanged(String),
    MemorySaveEdit,
    MemoryCancelEdit,
    MemoryConfirmDelete(String), // memory_id
    // Agent screen messages
    OpenAgents,
    AgentTaskInputChanged(String),
    LaunchAgent,
    // Toggle tools enabled for the current conversation (Phase 27, CHAT-TOOL-07)
    ToggleConvToolsEnabled,
    // Toggle the conversation options menu (Docs / Instructions / Tools panel)
    ToggleConvMenu,
    // Toggle the tools sub-panel within the conv menu
    ToggleToolsPanel,
    // Export the current conversation to a .md file (quick/260421-tg6).
    ExportConversationMarkdown,
    // Result of the save-file dialog + fs::write. Ok(Some(path)) = saved,
    // Ok(None) = user cancelled, Err = dialog/write failure.
    ExportMarkdownReady {
        result: Result<Option<std::path::PathBuf>, String>,
    },
    // Window close request (D-12: checkpoint running agent sessions on exit)
    WindowCloseRequested,
    // OS dark/light theme change
    SystemThemeChanged(bool),
    // Phase 28: lock screen PIN input
    UnlockPinChanged(String),
    UnlockSubmit,
    // Phase 28: PIN setup screen inputs
    PinSetupPinChanged(String),
    PinSetupConfirmChanged(String),
    PinSetupDuressChanged(String),
    PinSetupSubmit,
    // IMG-07: thumbnail decrypted and ready for display
    ThumbnailLoaded {
        message_id: String,
        handle: iced::widget::image::Handle,
    },
    // Phase 32 DIR-05: directory sources screen entry + messages
    OpenDirectorySources,
    DirSources(views::directory_sources::Message),
    /// Internal signal: the notify watcher reported a change for this source.
    DirSyncTriggered(String),
    /// Internal signal: 5-minute fallback interval fired (phase 32, D-21).
    DirSyncIntervalTick,
    /// Internal signal: surface or clear the PollWatcher fallback warning banner.
    DirWatcherFallbackWarning(Option<String>),
    /// Internal signal: a directory sync pipeline run completed (no-op update).
    DirSyncCompleted,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = match AppManager::new() {
            Ok(manager) => {
                let state = manager.state();
                let prefs = load_preferences();
                let initial_dark = match prefs.theme_override {
                    ThemeOverride::ForceDark => true,
                    ThemeOverride::ForceLight => false,
                    ThemeOverride::FollowSystem => true,
                };

                // Phase 32 DIR-05: directory sync channels + watcher state.
                // The watcher thread (notify debouncer + PollWatcher fallback) and the
                // 5-minute tokio interval ticker both push Message values into
                // dir_trigger_tx; the iced subscription consumes dir_trigger_rx and
                // delivers them to the main update loop.
                let (dir_trigger_tx, dir_trigger_rx) = flume::unbounded::<Message>();
                let dir_in_flight: Arc<Mutex<HashSet<String>>> =
                    Arc::new(Mutex::new(HashSet::new()));
                let dir_watched_paths: Arc<Mutex<HashMap<String, String>>> =
                    Arc::new(Mutex::new(HashMap::new()));

                spawn_directory_sync_workers(
                    manager.clone(),
                    dir_trigger_tx.clone(),
                    dir_watched_paths.clone(),
                );

                Self::Loaded {
                    manager,
                    state,
                    streaming_content: iced::widget::markdown::Content::new(),
                    prev_streaming_len: 0,
                    input_text: String::new(),
                    system_prompt_text: String::new(),
                    show_system_prompt_input: false,
                    rename_state: None,
                    edit_state: None,
                    show_attestation_detail: false,
                    parsed_messages: HashMap::new(),
                    settings_add_name: String::new(),
                    settings_add_url: String::new(),
                    settings_add_key: String::new(),
                    settings_add_tee: "IntelTdx".to_string(),
                    settings_default_model: String::new(),
                    settings_preset_keys: std::collections::HashMap::new(),
                    settings_show_advanced: false,
                    settings_attestation_interval: String::new(),
                    settings_default_instructions: String::new(),
                    settings_default_instructions_initialized: false,
                    onboarding_selected_backend: String::new(),
                    onboarding_api_key: String::new(),
                    onboarding_show_learn_more: false,
                    show_docs_attachment_overlay: false,
                    show_conv_menu: false,
                    force_remote_next: false,
                    show_tools_panel: false,
                    memory_edit_state: None,
                    settings_brave_api_key: String::new(),
                    settings_brave_api_key_message: None,
                    is_dark: initial_dark,
                    cached_theme: theme::app_theme(initial_dark),
                    theme_override: prefs.theme_override,
                    agent_task_input: String::new(),
                    lock_pin_input: String::new(),
                    setup_pin_input: String::new(),
                    setup_confirm_input: String::new(),
                    setup_duress_input: String::new(),
                    image_cache: HashMap::new(),
                    dir_editing_exclusions_for: None,
                    dir_exclusion_edit_text: String::new(),
                    dir_exclusion_validation: HashMap::new(),
                    dir_pending_remove_id: None,
                    dir_watcher_warning: None,
                    dir_trigger_rx: dir_trigger_rx.clone(),
                    dir_trigger_tx: dir_trigger_tx.clone(),
                    dir_in_flight: dir_in_flight.clone(),
                    dir_watched_paths: dir_watched_paths.clone(),
                    // Phase 36
                    contextvm_search_query: String::new(),
                    contextvm_provider_filter: None,
                    contextvm_copy_status: None,
                    contextvm_schema_expanded: false,
                }
            }
            Err(error) => Self::BootError { error },
        };
        (app, Task::none())
    }

    fn theme(&self) -> Theme {
        match self {
            App::Loaded { cached_theme, .. } => cached_theme.clone(),
            App::BootError { .. } => theme::app_theme(true),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self {
            App::BootError { .. } => Subscription::none(),
            App::Loaded {
                manager,
                dir_trigger_rx,
                ..
            } => {
                let core_updates = Subscription::run_with(manager.clone(), manager_update_stream)
                    .map(|_| Message::CoreUpdated);

                // Phase 32: directory-sync trigger stream (notify watcher, 5-min
                // interval, PollWatcher fallback warnings, pipeline completion).
                let dir_rx = dir_trigger_rx.clone();
                let dir_triggers = Subscription::run_with(
                    DirTriggerId(dir_rx.clone()),
                    move |id: &DirTriggerId| {
                        let rx = id.0.clone();
                        iced::futures::stream::unfold(rx, |rx| async move {
                            match rx.recv_async().await {
                                Ok(m) => Some((m, rx)),
                                Err(_) => None,
                            }
                        })
                    },
                );

                // D-12: Listen for window close to checkpoint running agent sessions
                // D-10 (desktop): iced has no background/foreground lifecycle API.
                // Background lock timeout is not supported on desktop. The app locks on
                // cold launch (Screen::Locked initial state) and never auto-locks during a
                // running session. This is a documented desktop limitation per the plan.
                let window_close = iced::event::listen_with(|event, _status, _id| {
                    if let iced::Event::Window(iced::window::Event::CloseRequested) = event {
                        Some(Message::WindowCloseRequested)
                    } else {
                        None
                    }
                });

                let theme_sub = iced::system::theme_changes()
                    .map(|mode| Message::SystemThemeChanged(mode == iced::theme::Mode::Dark));

                Subscription::batch(vec![core_updates, window_close, theme_sub, dir_triggers])
            }
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match self {
            App::BootError { .. } => {}
            App::Loaded {
                manager,
                state,
                streaming_content,
                prev_streaming_len,
                input_text,
                system_prompt_text,
                show_system_prompt_input,
                rename_state,
                edit_state,
                show_attestation_detail,
                parsed_messages,
                settings_add_name,
                settings_add_url,
                settings_add_key,
                settings_add_tee,
                settings_default_model,
                settings_preset_keys,
                settings_show_advanced,
                settings_attestation_interval,
                settings_default_instructions,
                settings_default_instructions_initialized,
                onboarding_selected_backend,
                onboarding_api_key,
                onboarding_show_learn_more,
                show_docs_attachment_overlay,
                show_conv_menu,
                force_remote_next,
                show_tools_panel,
                memory_edit_state,
                settings_brave_api_key,
                settings_brave_api_key_message,
                is_dark,
                cached_theme,
                theme_override,
                agent_task_input,
                lock_pin_input,
                setup_pin_input,
                setup_confirm_input,
                setup_duress_input,
                image_cache,
                dir_editing_exclusions_for,
                dir_exclusion_edit_text,
                dir_exclusion_validation,
                dir_pending_remove_id,
                dir_watcher_warning,
                dir_trigger_rx: _,
                dir_trigger_tx,
                dir_in_flight,
                dir_watched_paths,
                contextvm_search_query,
                contextvm_provider_filter,
                contextvm_copy_status,
                contextvm_schema_expanded,
            } => {
                match message {
                    Message::CoreUpdated => {
                        let latest = manager.state();
                        // Parse new completed assistant messages for markdown rendering
                        // (per iced docs: store Vec<markdown::Item> in app state)
                        for msg in &latest.messages {
                            if msg.role == "assistant" && !parsed_messages.contains_key(&msg.id) {
                                let items: Vec<iced::widget::markdown::Item> =
                                    iced::widget::markdown::parse(&msg.content).collect();
                                parsed_messages.insert(msg.id.clone(), items);
                            }
                        }
                        // Streaming delta extraction via prev_streaming_len
                        match (&latest.streaming_text, &state.streaming_text) {
                            (Some(new_text), _) => {
                                let new_len = new_text.len();
                                if new_len > *prev_streaming_len {
                                    // Normal: append delta
                                    let delta = &new_text[*prev_streaming_len..];
                                    streaming_content.push_str(delta);
                                    *prev_streaming_len = new_len;
                                } else if new_len <= *prev_streaming_len {
                                    // Unexpected reset or restart: full re-parse
                                    *streaming_content = iced::widget::markdown::Content::new();
                                    streaming_content.push_str(new_text);
                                    *prev_streaming_len = new_len;
                                }
                            }
                            (None, Some(_)) => {
                                // StreamDone: reset streaming content
                                *streaming_content = iced::widget::markdown::Content::new();
                                *prev_streaming_len = 0;
                            }
                            (None, None) => {}
                        }
                        // Sync default instructions from core state on first load.
                        if !*settings_default_instructions_initialized {
                            if let Some(sp) = &latest.global_system_prompt {
                                *settings_default_instructions = sp.clone();
                            }
                            *settings_default_instructions_initialized = true;
                        }
                        // Mirror Brave API key validation toast into the inline
                        // settings message, then clear it from the core state.
                        if let Some(toast) = &latest.toast.clone() {
                            *settings_brave_api_key_message = Some(toast.clone());
                            manager.dispatch(AppAction::ClearToast);
                        }
                        // IMG-07: spawn Tasks to decrypt-on-read any new user messages
                        // with image_path not yet in the cache.
                        let mut thumb_tasks: Vec<Task<Message>> = Vec::new();
                        for msg in &latest.messages {
                            if msg.image_path.is_some() && !image_cache.contains_key(&msg.id) {
                                let msg_id = msg.id.clone();
                                let ffi = manager.ffi.clone();
                                thumb_tasks.push(Task::perform(
                                    async move {
                                        tokio::task::spawn_blocking(move || {
                                            ffi.read_encrypted_image(msg_id.clone())
                                                .map(|bytes| (msg_id, bytes))
                                        })
                                        .await
                                        .ok()
                                        .and_then(|r| r.ok())
                                    },
                                    |result| match result {
                                        Some((message_id, bytes)) => Message::ThumbnailLoaded {
                                            message_id,
                                            handle: iced::widget::image::Handle::from_bytes(bytes),
                                        },
                                        None => Message::CoreUpdated, // no-op on failure
                                    },
                                ));
                            }
                        }
                        // CRITICAL: commit the latest core snapshot to UI state.
                        // Without this, view() renders against a frozen default state
                        // (Screen::Home, conversations=[]) and the lock screen,
                        // onboarding, settings, and all live updates never appear.
                        // Regression introduced in commit a7c204b (IMG-07) which
                        // accidentally removed the `*state = latest` assignment
                        // along with the (now redundant) `rev > state.rev` guard.
                        *state = latest;
                        if !thumb_tasks.is_empty() {
                            return Task::batch(thumb_tasks);
                        }
                        return Task::none();
                    }

                    Message::ThumbnailLoaded { message_id, handle } => {
                        image_cache.insert(message_id, handle);
                    }

                    Message::DispatchAction(action) => {
                        if action_clears_force_remote_next(&action) {
                            *force_remote_next = false;
                        }
                        manager.dispatch(action);
                    }

                    Message::InputChanged(val) => {
                        *input_text = val;
                    }

                    Message::SubmitMessage => {
                        let text_to_send = input_text.trim().to_string();
                        if !text_to_send.is_empty() {
                            manager.dispatch(AppAction::SendMessage {
                                text: text_to_send,
                                force_role: if *force_remote_next {
                                    Some(BackendRole::Remote)
                                } else {
                                    None
                                },
                            });
                            *force_remote_next = false;
                            *input_text = String::new();
                        }
                    }

                    Message::OpenConversation(id) => {
                        *show_system_prompt_input = false;
                        *system_prompt_text = String::new();
                        *force_remote_next = false;
                        manager.dispatch(AppAction::LoadConversation {
                            conversation_id: id,
                        });
                    }

                    Message::StartRename(id, current_title) => {
                        *rename_state = Some((id, current_title));
                    }

                    Message::RenameChanged(val) => {
                        if let Some((_, ref mut text)) = rename_state {
                            *text = val;
                        }
                    }

                    Message::SubmitRename => {
                        if let Some((id, title)) = rename_state.take() {
                            let trimmed = title.trim().to_string();
                            if !trimmed.is_empty() {
                                manager
                                    .dispatch(AppAction::RenameConversation { id, title: trimmed });
                            }
                        }
                    }

                    Message::CancelRename => {
                        *rename_state = None;
                    }

                    Message::ConfirmDelete(id) => {
                        manager.dispatch(AppAction::DeleteConversation { id });
                    }

                    Message::ForkConversation => {
                        // quick/260423-93w: fork the currently-loaded conversation.
                        // The core handler (AppAction::ForkConversation) does all the
                        // work: db-locked guard, transactional copy, nav into the new
                        // Screen::Chat. No-op if there is no current conversation —
                        // the button's on_press_maybe guard in chat.rs already hides
                        // this case for empty conversations, but we also re-check here
                        // for defense-in-depth (e.g. hotkey bindings).
                        if let Some(cid) = manager.state().current_conversation_id.clone() {
                            manager.dispatch(AppAction::ForkConversation { id: cid });
                        }
                    }

                    Message::CopyMessage(content) => {
                        return iced::clipboard::write(content);
                    }

                    Message::RetryMessage => {
                        manager.dispatch(AppAction::RetryLastMessage);
                    }

                    Message::StartEdit(msg_id, current_text) => {
                        *edit_state = Some((msg_id, current_text));
                    }

                    Message::EditChanged(val) => {
                        if let Some((_, ref mut t)) = edit_state {
                            *t = val;
                        }
                    }

                    Message::SubmitEdit => {
                        if let Some((msg_id, new_text)) = edit_state.take() {
                            let trimmed = new_text.trim().to_string();
                            if !trimmed.is_empty() {
                                manager.dispatch(AppAction::EditMessage {
                                    message_id: msg_id,
                                    new_text: trimmed,
                                });
                            }
                        }
                    }

                    Message::CancelEdit => {
                        *edit_state = None;
                    }

                    Message::AttachFile => {
                        // Use rfd for native file dialog (blocking, run via spawn_blocking).
                        // Phase 31 IMG-05: accept both image extensions (jpg/jpeg/png) and
                        // the existing text-file extensions via a single filter. When the
                        // selected file is an image, dispatch AppAction::AttachImage with
                        // the absolute path + MIME; otherwise keep the existing text path.
                        //
                        // Vision capability gating (follow-up to image-upload-still-broken-after-fix):
                        // when the current conversation's model does not support vision,
                        // omit the image extensions from the filter so the user cannot
                        // pick a photo that would silently fail at the model.
                        let current_model_id = manager
                            .state()
                            .current_conversation_id
                            .as_ref()
                            .and_then(|id| {
                                manager
                                    .state()
                                    .conversations
                                    .iter()
                                    .find(|c| &c.id == id)
                                    .map(|c| c.model_id.clone())
                            })
                            .unwrap_or_default();
                        let allow_images = !current_model_id.is_empty()
                            && mango_core::is_vision_model(&current_model_id);
                        let manager_clone = manager.clone();
                        let fut = async move {
                            let result = tokio::task::spawn_blocking(move || -> Option<()> {
                                let mut dialog = rfd::FileDialog::new();
                                dialog = if allow_images {
                                    dialog
                                        .add_filter(
                                            "Attachable",
                                            &[
                                                "jpg", "jpeg", "png", "txt", "md", "json", "csv",
                                                "log",
                                            ],
                                        )
                                        .add_filter("Images", &["jpg", "jpeg", "png"])
                                        .add_filter("Text", &["txt", "md", "json", "csv", "log"])
                                } else {
                                    dialog.add_filter("Text", &["txt", "md", "json", "csv", "log"])
                                };
                                let path = dialog.pick_file()?;
                                let filename = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "attachment".to_string());
                                let ext = path
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("")
                                    .to_ascii_lowercase();
                                let is_image = matches!(ext.as_str(), "jpg" | "jpeg" | "png");

                                if is_image {
                                    let mime = if ext == "png" {
                                        "image/png".to_string()
                                    } else {
                                        "image/jpeg".to_string()
                                    };
                                    match stage_desktop_image_attachment(&path, &ext) {
                                        Ok(staged_path) => {
                                            manager_clone.dispatch(AppAction::AttachImage {
                                                filename,
                                                file_path: staged_path
                                                    .to_string_lossy()
                                                    .into_owned(),
                                                mime_type: mime,
                                            });
                                        }
                                        Err(message) => {
                                            manager_clone
                                                .dispatch(AppAction::ShowToast { message });
                                        }
                                    }
                                } else {
                                    match std::fs::read_to_string(&path) {
                                        Ok(content) => {
                                            let size_bytes = content.len() as u64;
                                            manager_clone.dispatch(AppAction::AttachFile {
                                                filename,
                                                content,
                                                size_bytes,
                                            });
                                        }
                                        Err(_) => {
                                            manager_clone.dispatch(AppAction::ShowToast {
                                                message: "This file type cannot be read as text."
                                                    .to_string(),
                                            });
                                        }
                                    }
                                }
                                Some(())
                            })
                            .await;
                            let _ = result;
                        };
                        return Task::perform(fut, |_| Message::CoreUpdated);
                    }

                    Message::ClearAttachment => {
                        manager.dispatch(AppAction::ClearAttachment);
                    }

                    Message::SelectModel(model_id) => {
                        manager.dispatch(AppAction::SelectModel { model_id });
                    }

                    Message::UseHybridProfile(profile_id) => {
                        if let Some(conversation_id) = state.current_conversation_id.clone() {
                            manager.dispatch(AppAction::OverrideConversationBackend {
                                conversation_id,
                                backend_id: format!("hybrid:{profile_id}"),
                            });
                        }
                        manager.dispatch(AppAction::SetActiveHybridProfile { profile_id });
                        *force_remote_next = false;
                    }

                    Message::ToggleForceRemoteNext => {
                        *force_remote_next = !*force_remote_next;
                    }

                    Message::ToggleSystemPromptInput => {
                        *show_system_prompt_input = !*show_system_prompt_input;
                        if *show_system_prompt_input {
                            // Pre-populate with the current conversation's system prompt
                            // so the user can view and edit it rather than re-entering from scratch.
                            *system_prompt_text = state
                                .current_conversation_id
                                .as_deref()
                                .and_then(|cid| state.conversations.iter().find(|c| c.id == cid))
                                .and_then(|c| c.system_prompt.clone())
                                .unwrap_or_default();
                        }
                    }

                    Message::SystemPromptChanged(val) => {
                        *system_prompt_text = val;
                    }

                    Message::SubmitSystemPrompt => {
                        let prompt = {
                            let trimmed = system_prompt_text.trim().to_string();
                            if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed)
                            }
                        };
                        manager.dispatch(AppAction::SetSystemPrompt { prompt });
                        *show_system_prompt_input = false;
                    }

                    Message::ToggleAttestationDetail => {
                        *show_attestation_detail = !*show_attestation_detail;
                    }

                    Message::MarkdownLinkClicked(_url) => {
                        let _ = open::that(&_url);
                    }
                    Message::OpenUrl(url) => {
                        let _ = open::that(&url);
                    }

                    // Settings form handlers
                    Message::SettingsAddNameChanged(val) => {
                        *settings_add_name = val;
                    }
                    Message::SettingsAddUrlChanged(val) => {
                        *settings_add_url = val;
                    }
                    Message::SettingsAddKeyChanged(val) => {
                        *settings_add_key = val;
                    }
                    Message::SettingsAddTeeChanged(val) => {
                        *settings_add_tee = val.to_string();
                    }
                    Message::SettingsDefaultModelChanged(val) => {
                        *settings_default_model = val.clone();
                        manager.dispatch(AppAction::SetDefaultModel { model_id: val });
                    }
                    Message::SettingsSubmitAddBackend {
                        name,
                        url,
                        key,
                        tee,
                    } => {
                        manager.dispatch(AppAction::AddBackend {
                            name,
                            base_url: url,
                            api_key: key,
                            tee_type: tee,
                            models: vec![],
                        });
                        *settings_add_name = String::new();
                        *settings_add_url = String::new();
                        *settings_add_key = String::new();
                        *settings_add_tee = "IntelTdx".to_string();
                    }

                    Message::SettingsPresetKeyChanged { preset_id, key } => {
                        settings_preset_keys.insert(preset_id, key);
                    }

                    Message::SettingsEnablePreset { preset_id } => {
                        let api_key = settings_preset_keys
                            .get(&preset_id)
                            .cloned()
                            .unwrap_or_default();
                        if !api_key.trim().is_empty() || preset_allows_empty_api_key(&preset_id) {
                            manager.dispatch(AppAction::AddBackendFromPreset {
                                preset_id: preset_id.clone(),
                                api_key,
                            });
                            settings_preset_keys.remove(&preset_id);
                        }
                    }

                    Message::SettingsToggleAdvanced => {
                        *settings_show_advanced = !*settings_show_advanced;
                    }

                    Message::SettingsAttestationIntervalChanged(val) => {
                        *settings_attestation_interval = val;
                    }

                    Message::SettingsApplyAttestationInterval => {
                        if let Ok(minutes) = settings_attestation_interval.trim().parse::<u32>() {
                            manager.dispatch(AppAction::SetAttestationInterval { minutes });
                        }
                    }

                    Message::SettingsDefaultInstructionsChanged(val) => {
                        *settings_default_instructions = val;
                    }

                    Message::SettingsSaveDefaultInstructions => {
                        let prompt = if settings_default_instructions.trim().is_empty() {
                            None
                        } else {
                            Some(settings_default_instructions.clone())
                        };
                        manager.dispatch(AppAction::SetGlobalSystemPrompt { prompt });
                    }

                    Message::SettingsBraveApiKeyChanged(val) => {
                        *settings_brave_api_key = val;
                    }

                    Message::SettingsSaveBraveApiKey => {
                        let trimmed = settings_brave_api_key.trim().to_string();
                        if !trimmed.is_empty() {
                            *settings_brave_api_key_message = None;
                            manager.dispatch(AppAction::ValidateBraveApiKey { api_key: trimmed });
                            *settings_brave_api_key = String::new();
                        }
                    }

                    Message::SettingsMemoriesEnabledToggled(enabled) => {
                        manager.dispatch(AppAction::SetMemoriesEnabled { enabled });
                    }

                    // Phase 35 — Settings → TOOLS → Auto-discover toggle.
                    Message::SettingsAutoDiscoverToolsToggled(enabled) => {
                        manager.dispatch(AppAction::SetAutoDiscoverTools { enabled });
                    }
                    // Phase 35 — push Tool Discovery screen + kick off discovery.
                    Message::ContextvmDiscoverToolsClicked => {
                        manager.dispatch(AppAction::PushScreen {
                            screen: Screen::ToolDiscovery,
                        });
                        manager.dispatch(AppAction::DiscoverContextvmTools);
                    }
                    // Phase 35 — Refresh / Try again button on Tool Discovery.
                    Message::ContextvmRetryClicked => {
                        manager.dispatch(AppAction::RetryContextvmDiscovery);
                    }
                    // Phase 35 — per-tool toggler in Tool Discovery list.
                    Message::ContextvmToolToggled { tool_id, enabled } => {
                        manager.dispatch(AppAction::SetContextvmToolEnabled { tool_id, enabled });
                    }
                    // Phase 36 — Tool Discovery search filter (no debounce).
                    Message::ContextvmSearchChanged(q) => {
                        *contextvm_search_query = q;
                    }
                    Message::ContextvmProviderFilterChanged(provider) => {
                        *contextvm_provider_filter = provider;
                    }
                    // Phase 36 — Tool Detail Copy actions. Each writes the
                    // FULL value to the clipboard, surfaces the locked status
                    // string, and schedules a ClearCopyStatus 2s later.
                    Message::CopyNpub(value) => {
                        *contextvm_copy_status = Some("npub copied".to_string());
                        return Task::batch([
                            iced::clipboard::write(value),
                            Task::perform(
                                tokio::time::sleep(std::time::Duration::from_secs(2)),
                                |_| Message::ClearCopyStatus,
                            ),
                        ]);
                    }
                    Message::CopyHex(value) => {
                        *contextvm_copy_status = Some("Pubkey copied".to_string());
                        return Task::batch([
                            iced::clipboard::write(value),
                            Task::perform(
                                tokio::time::sleep(std::time::Duration::from_secs(2)),
                                |_| Message::ClearCopyStatus,
                            ),
                        ]);
                    }
                    Message::CopyToolId(value) => {
                        *contextvm_copy_status = Some("Tool ID copied".to_string());
                        return Task::batch([
                            iced::clipboard::write(value),
                            Task::perform(
                                tokio::time::sleep(std::time::Duration::from_secs(2)),
                                |_| Message::ClearCopyStatus,
                            ),
                        ]);
                    }
                    Message::ToggleSchemaExpanded => {
                        *contextvm_schema_expanded = !*contextvm_schema_expanded;
                    }
                    Message::ClearCopyStatus => {
                        *contextvm_copy_status = None;
                    }

                    // Onboarding wizard handlers
                    Message::OnboardingSelectBackend(id) => {
                        *onboarding_selected_backend = id;
                    }
                    Message::OnboardingApiKeyChanged(val) => {
                        *onboarding_api_key = val;
                    }
                    Message::OnboardingValidateKey => {
                        let preset_id = onboarding_selected_backend.clone();
                        let api_key = onboarding_api_key.trim().to_string();
                        if !preset_id.is_empty()
                            && (!api_key.is_empty() || preset_allows_empty_api_key(&preset_id))
                        {
                            // First, add/enable the backend from the preset (idempotent).
                            manager.dispatch(AppAction::AddBackendFromPreset {
                                preset_id: preset_id.clone(),
                                api_key: api_key.clone(),
                            });
                            // Then trigger the health-check / attestation flow.
                            manager.dispatch(AppAction::ValidateApiKey {
                                backend_id: preset_id,
                            });
                        }
                    }
                    Message::OnboardingNext => {
                        manager.dispatch(AppAction::NextOnboardingStep);
                    }
                    Message::OnboardingBack => {
                        manager.dispatch(AppAction::PreviousOnboardingStep);
                    }
                    Message::OnboardingComplete => {
                        manager.dispatch(AppAction::CompleteOnboarding);
                    }
                    Message::OnboardingRetryAttestation => {
                        let preset_id = onboarding_selected_backend.clone();
                        manager.dispatch(AppAction::ValidateApiKey {
                            backend_id: preset_id,
                        });
                    }
                    Message::OnboardingToggleLearnMore => {
                        *onboarding_show_learn_more = !*onboarding_show_learn_more;
                    }
                    Message::OnboardingSkip => {
                        manager.dispatch(AppAction::SkipOnboarding);
                    }
                    Message::RunSetupWizard => {
                        manager.dispatch(AppAction::PushScreen {
                            screen: Screen::Onboarding {
                                step: OnboardingStep::Welcome,
                            },
                        });
                    }

                    // Documents screen handlers (Phase 8, LRAG-06)
                    Message::OpenDocuments => {
                        manager.dispatch(AppAction::PushScreen {
                            screen: Screen::Documents,
                        });
                    }

                    Message::PickDocumentFile => {
                        let manager_clone = manager.clone();
                        let fut = async move {
                            let result = tokio::task::spawn_blocking(move || -> Option<()> {
                                let path = rfd::FileDialog::new()
                                    .add_filter("Documents", &["pdf", "txt", "md"])
                                    .pick_file()?;
                                let filename = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "document".to_string());
                                match std::fs::read(&path) {
                                    Ok(content) => {
                                        manager_clone.dispatch(AppAction::IngestDocument {
                                            filename,
                                            content,
                                        });
                                    }
                                    Err(_) => {
                                        manager_clone.dispatch(AppAction::ShowToast {
                                            message: "Failed to read the selected file."
                                                .to_string(),
                                        });
                                    }
                                }
                                Some(())
                            })
                            .await;
                            let _ = result;
                        };
                        return Task::perform(fut, |_| Message::CoreUpdated);
                    }

                    Message::DeleteDocument(doc_id) => {
                        manager.dispatch(AppAction::DeleteDocument {
                            document_id: doc_id,
                        });
                    }

                    Message::ToggleDocAttachmentOverlay => {
                        *show_docs_attachment_overlay = !*show_docs_attachment_overlay;
                    }

                    Message::ToggleConvMenu => {
                        *show_conv_menu = !*show_conv_menu;
                        if !*show_conv_menu {
                            *show_tools_panel = false;
                        }
                    }

                    Message::ToggleToolsPanel => {
                        *show_tools_panel = !*show_tools_panel;
                    }

                    // Export current conversation as Markdown (quick/260421-tg6).
                    // Round-trip: core render → rfd save dialog → std::fs::write.
                    // No cloud/network; stays on-device per project privacy constraint.
                    Message::ExportConversationMarkdown => {
                        let cid = match state.current_conversation_id.clone() {
                            Some(id) => id,
                            None => {
                                manager.dispatch(AppAction::ShowToast {
                                    message: "No conversation to export".into(),
                                });
                                return Task::none();
                            }
                        };
                        // Look up title for filename pre-fill.
                        let title = state
                            .conversations
                            .iter()
                            .find(|c| c.id == cid)
                            .map(|c| c.title.clone())
                            .unwrap_or_default();
                        let sanitized = sanitize_filename(&title);

                        // Render markdown on the actor thread.
                        let markdown = match manager.ffi.export_conversation_markdown(cid) {
                            Ok(md) => md,
                            Err(e) => {
                                manager.dispatch(AppAction::ShowToast {
                                    message: format!("Export failed: {e}"),
                                });
                                return Task::none();
                            }
                        };

                        // Run the blocking save dialog + write off the UI thread.
                        let fut = async move {
                            tokio::task::spawn_blocking(move || {
                                let picked = rfd::FileDialog::new()
                                    .set_file_name(format!("{sanitized}.md"))
                                    .add_filter("Markdown", &["md"])
                                    .save_file();
                                match picked {
                                    Some(path) => match std::fs::write(&path, &markdown) {
                                        Ok(_) => Ok(Some(path)),
                                        Err(e) => Err(format!("write failed: {e}")),
                                    },
                                    None => Ok(None), // user cancelled
                                }
                            })
                            .await
                            .unwrap_or_else(|e| Err(format!("task join error: {e}")))
                        };
                        return Task::perform(fut, |result| Message::ExportMarkdownReady {
                            result,
                        });
                    }

                    Message::ExportMarkdownReady { result } => {
                        match result {
                            Ok(Some(path)) => {
                                manager.dispatch(AppAction::ShowToast {
                                    message: format!("Exported to {}", path.display()),
                                });
                            }
                            Ok(None) => { /* cancelled — no toast, no error */ }
                            Err(reason) => {
                                manager.dispatch(AppAction::ShowToast {
                                    message: format!("Export failed: {reason}"),
                                });
                            }
                        }
                    }

                    Message::ToggleDocumentAttachment(doc_id) => {
                        let attached = state.current_conversation_attached_docs.contains(&doc_id);
                        if attached {
                            manager.dispatch(AppAction::DetachDocumentFromConversation {
                                document_id: doc_id,
                            });
                        } else {
                            manager.dispatch(AppAction::AttachDocumentToConversation {
                                document_id: doc_id,
                            });
                        }
                        *show_docs_attachment_overlay = false;
                    }

                    Message::OpenAgents => {
                        if mango_core::features::AGENTS_ENABLED {
                            manager.dispatch(AppAction::PushScreen {
                                screen: Screen::Agents,
                            });
                        }
                    }
                    Message::AgentTaskInputChanged(text) => {
                        *agent_task_input = text;
                    }
                    Message::LaunchAgent => {
                        if mango_core::features::AGENTS_ENABLED && !agent_task_input.is_empty() {
                            manager.dispatch(AppAction::LaunchAgentSession {
                                task_description: agent_task_input.clone(),
                            });
                            *agent_task_input = String::new();
                        }
                    }

                    Message::OpenMemories => {
                        manager.dispatch(AppAction::PushScreen {
                            screen: Screen::Memories,
                        });
                    }

                    Message::MemoryStartEdit(id, content) => {
                        *memory_edit_state = Some((id, content));
                    }

                    Message::MemoryEditChanged(text) => {
                        if let Some((_, ref mut edit_text)) = memory_edit_state {
                            *edit_text = text;
                        }
                    }

                    Message::MemorySaveEdit => {
                        if let Some((ref id, ref content)) = memory_edit_state {
                            manager.dispatch(AppAction::UpdateMemory {
                                memory_id: id.clone(),
                                content: content.clone(),
                            });
                        }
                        *memory_edit_state = None;
                    }

                    Message::MemoryCancelEdit => {
                        *memory_edit_state = None;
                    }

                    Message::MemoryConfirmDelete(id) => {
                        manager.dispatch(AppAction::DeleteMemory { memory_id: id });
                    }

                    Message::SystemThemeChanged(dark) => {
                        if *theme_override == ThemeOverride::FollowSystem {
                            *is_dark = dark;
                            *cached_theme = theme::app_theme(dark);
                        }
                    }

                    Message::SettingsThemeOverrideChanged(new_override) => {
                        *theme_override = new_override;
                        match new_override {
                            ThemeOverride::ForceDark => {
                                *is_dark = true;
                                *cached_theme = theme::app_theme(true);
                            }
                            ThemeOverride::ForceLight => {
                                *is_dark = false;
                                *cached_theme = theme::app_theme(false);
                            }
                            ThemeOverride::FollowSystem => {
                                // Will pick up OS theme on next SystemThemeChanged event;
                                // no immediate change needed (current is_dark stays until OS notifies)
                            }
                        }
                        save_preferences(&Preferences {
                            theme_override: new_override,
                        });
                    }

                    // Phase 27: Toggle tools enabled for the current conversation (CHAT-TOOL-07)
                    Message::ToggleConvToolsEnabled => {
                        if let Some(conv_id) = state.current_conversation_id.clone() {
                            let current = state
                                .conversations
                                .iter()
                                .find(|c| c.id == conv_id)
                                .map(|c| c.tools_enabled)
                                .unwrap_or(false);
                            manager.dispatch(AppAction::SetConversationToolsEnabled {
                                conversation_id: conv_id,
                                enabled: !current,
                            });
                        }
                    }

                    // Phase 28: lock screen PIN handlers
                    Message::UnlockPinChanged(val) => {
                        *lock_pin_input = val;
                        // Clear any previous error toast when user starts typing again
                        if state.toast.is_some() {
                            manager.dispatch(AppAction::ClearToast);
                        }
                    }
                    Message::UnlockSubmit => {
                        let pin = lock_pin_input.trim().to_string();
                        if !pin.is_empty() {
                            manager.dispatch(AppAction::UnlockWithPin { pin });
                            // Clear PIN from local state immediately after dispatch (T-28-23)
                            *lock_pin_input = String::new();
                        }
                    }
                    // Phase 28: PIN setup screen handlers
                    Message::PinSetupPinChanged(val) => {
                        *setup_pin_input = val;
                    }
                    Message::PinSetupConfirmChanged(val) => {
                        *setup_confirm_input = val;
                    }
                    Message::PinSetupDuressChanged(val) => {
                        *setup_duress_input = val;
                    }
                    Message::PinSetupSubmit => {
                        if let Some(action) = pin_setup_screen::build_setup_pin_action(
                            setup_pin_input,
                            setup_confirm_input,
                            setup_duress_input,
                        ) {
                            manager.dispatch(action);
                            // Clear setup inputs after dispatch (T-28-23)
                            *setup_pin_input = String::new();
                            *setup_confirm_input = String::new();
                            *setup_duress_input = String::new();
                        }
                    }

                    // ── Phase 32 DIR-05: directory sources handlers ────────────
                    Message::OpenDirectorySources => {
                        manager.dispatch(AppAction::PushScreen {
                            screen: Screen::DirectorySources,
                        });
                    }

                    Message::DirWatcherFallbackWarning(msg) => {
                        *dir_watcher_warning = msg;
                    }

                    Message::DirSyncCompleted => { /* no-op: UI refresh comes via CoreUpdated */ }

                    Message::DirSyncTriggered(source_id) => {
                        // Watcher fired for source_id → enqueue a sync pipeline run.
                        if let Some((path, globs)) =
                            lookup_source_path_and_globs(state, dir_watched_paths, &source_id)
                        {
                            manager.dispatch(AppAction::TriggerDirectorySync {
                                source_id: source_id.clone(),
                            });
                            run_desktop_sync(
                                manager.clone(),
                                source_id,
                                path,
                                globs,
                                dir_in_flight.clone(),
                                dir_trigger_tx.clone(),
                            );
                        }
                    }

                    Message::DirSyncIntervalTick => {
                        // 5-minute fallback: resync all known sources.
                        let ids: Vec<String> = state
                            .directory_sources
                            .iter()
                            .map(|s| s.id.clone())
                            .collect();
                        for sid in ids {
                            if let Some((path, globs)) =
                                lookup_source_path_and_globs(state, dir_watched_paths, &sid)
                            {
                                manager.dispatch(AppAction::TriggerDirectorySync {
                                    source_id: sid.clone(),
                                });
                                run_desktop_sync(
                                    manager.clone(),
                                    sid,
                                    path,
                                    globs,
                                    dir_in_flight.clone(),
                                    dir_trigger_tx.clone(),
                                );
                            }
                        }
                    }

                    Message::DirSources(dm) => {
                        use views::directory_sources::Message as DM;
                        match dm {
                            DM::AddFolder => {
                                return Task::perform(
                                    async move {
                                        rfd::AsyncFileDialog::new()
                                            .pick_folder()
                                            .await
                                            .map(|h| h.path().to_path_buf())
                                    },
                                    |opt| Message::DirSources(DM::FolderPicked(opt)),
                                );
                            }
                            DM::FolderPicked(Some(path)) => {
                                let path_str = path.to_string_lossy().to_string();
                                let display_name = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| path_str.clone());
                                let globs = views::directory_sources::default_exclusion_presets();
                                manager.dispatch(AppAction::AddDirectorySource {
                                    display_name,
                                    path: Some(path_str.clone()),
                                    bookmark_data: None,
                                    tree_uri: None,
                                    exclusion_globs: globs.clone(),
                                });
                                // The actor will emit a fresh directory_sources list; we
                                // grab the newly-added id by diffing the next state. For
                                // robustness we defer path registration to the next
                                // CoreUpdated tick (see below) by also spawning a
                                // background retry that reads manager.state() for 2s.
                                let watched_paths_clone = dir_watched_paths.clone();
                                let manager_clone = manager.clone();
                                let globs_clone = globs.clone();
                                let trigger_tx_clone = dir_trigger_tx.clone();
                                let in_flight_clone = dir_in_flight.clone();
                                std::thread::spawn(move || {
                                    // Poll for up to 2 seconds for the new source row.
                                    for _ in 0..20 {
                                        std::thread::sleep(StdDuration::from_millis(100));
                                        let sources = manager_clone.state().directory_sources;
                                        if let Some(s) = sources
                                            .iter()
                                            .find(|s| s.exclusion_globs == globs_clone)
                                        {
                                            if let Ok(mut g) = watched_paths_clone.lock() {
                                                g.insert(s.id.clone(), path_str.clone());
                                            }
                                            // Kick off the initial sync for the freshly
                                            // added source.
                                            manager_clone.dispatch(
                                                AppAction::TriggerDirectorySync {
                                                    source_id: s.id.clone(),
                                                },
                                            );
                                            run_desktop_sync(
                                                manager_clone.clone(),
                                                s.id.clone(),
                                                path_str.clone(),
                                                globs_clone.clone(),
                                                in_flight_clone.clone(),
                                                trigger_tx_clone.clone(),
                                            );
                                            break;
                                        }
                                    }
                                });
                            }
                            DM::FolderPicked(None) => {
                                // User cancelled — no-op.
                            }
                            DM::RemoveSource(id) => {
                                *dir_pending_remove_id = Some(id);
                            }
                            DM::CancelRemove => {
                                *dir_pending_remove_id = None;
                            }
                            DM::ConfirmRemove(id) => {
                                manager.dispatch(AppAction::RemoveDirectorySource {
                                    source_id: id.clone(),
                                });
                                if let Ok(mut g) = dir_watched_paths.lock() {
                                    g.remove(&id);
                                }
                                *dir_pending_remove_id = None;
                            }
                            DM::EditExclusions(id) => {
                                // Pre-populate editor with current source's globs.
                                let globs = state
                                    .directory_sources
                                    .iter()
                                    .find(|s| s.id == id)
                                    .map(|s| s.exclusion_globs.join("\n"))
                                    .unwrap_or_default();
                                *dir_exclusion_edit_text = globs;
                                dir_exclusion_validation.remove(&id);
                                *dir_editing_exclusions_for = Some(id);
                            }
                            DM::ExclusionsChanged(id, txt) => {
                                *dir_exclusion_edit_text = txt;
                                // Live-validate: store any validation error keyed by id.
                                match views::directory_sources::parse_and_validate_exclusions(
                                    dir_exclusion_edit_text,
                                ) {
                                    Ok(_) => {
                                        dir_exclusion_validation.remove(&id);
                                    }
                                    Err(e) => {
                                        dir_exclusion_validation.insert(id, e);
                                    }
                                }
                            }
                            DM::RestoreDefaultExclusions(id) => {
                                *dir_exclusion_edit_text =
                                    views::directory_sources::default_exclusion_presets()
                                        .join("\n");
                                dir_exclusion_validation.remove(&id);
                            }
                            DM::SaveExclusions(id) => {
                                match views::directory_sources::parse_and_validate_exclusions(
                                    dir_exclusion_edit_text,
                                ) {
                                    Ok(globs) => {
                                        manager.dispatch(AppAction::SetDirectoryExclusions {
                                            source_id: id.clone(),
                                            globs,
                                        });
                                        dir_exclusion_validation.remove(&id);
                                        *dir_editing_exclusions_for = None;
                                        *dir_exclusion_edit_text = String::new();
                                    }
                                    Err(e) => {
                                        dir_exclusion_validation.insert(id, e);
                                    }
                                }
                            }
                            DM::CancelExclusions => {
                                *dir_editing_exclusions_for = None;
                                *dir_exclusion_edit_text = String::new();
                                dir_exclusion_validation.clear();
                            }
                            DM::SyncNow(id) => {
                                if let Some((path, globs)) =
                                    lookup_source_path_and_globs(state, dir_watched_paths, &id)
                                {
                                    manager.dispatch(AppAction::TriggerDirectorySync {
                                        source_id: id.clone(),
                                    });
                                    run_desktop_sync(
                                        manager.clone(),
                                        id,
                                        path,
                                        globs,
                                        dir_in_flight.clone(),
                                        dir_trigger_tx.clone(),
                                    );
                                }
                            }
                            DM::OpenFolder(id) => {
                                // Resolve the path from AppState (the `path` field added in
                                // this polish pass) and open it in the native file browser
                                // via the `open` crate (xdg-open on Linux, Finder on macOS).
                                if let Some(src) =
                                    state.directory_sources.iter().find(|s| s.id == id)
                                {
                                    if let Some(ref path) = src.path {
                                        let _ = open::that(path);
                                    }
                                }
                            }
                        }
                    }

                    // D-12: On window close, checkpoint all running agent sessions to SQLite
                    Message::WindowCloseRequested => {
                        if mango_core::features::AGENTS_ENABLED {
                            for session in &state.agent_sessions {
                                if session.status == "running" {
                                    manager.dispatch(AppAction::PauseAgentSession {
                                        session_id: session.id.clone(),
                                    });
                                }
                            }
                        }
                        return iced::exit();
                    }
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            App::BootError { error } => center(
                column![text("Mango").size(24), text(format!("Error: {error}")),].spacing(12),
            )
            .into(),

            App::Loaded {
                state,
                streaming_content,
                input_text,
                system_prompt_text,
                show_system_prompt_input,
                rename_state,
                edit_state,
                show_attestation_detail,
                parsed_messages,
                settings_add_name,
                settings_add_url,
                settings_add_key,
                settings_add_tee,
                settings_default_model,
                settings_preset_keys,
                settings_show_advanced,
                settings_attestation_interval,
                settings_default_instructions,
                onboarding_selected_backend,
                onboarding_api_key,
                onboarding_show_learn_more,
                show_docs_attachment_overlay,
                show_conv_menu,
                force_remote_next,
                show_tools_panel,
                memory_edit_state,
                settings_brave_api_key,
                settings_brave_api_key_message,
                is_dark,
                cached_theme,
                theme_override,
                agent_task_input,
                lock_pin_input,
                setup_pin_input,
                setup_confirm_input,
                setup_duress_input,
                image_cache,
                dir_editing_exclusions_for,
                dir_exclusion_edit_text,
                dir_exclusion_validation,
                dir_pending_remove_id,
                dir_watcher_warning,
                contextvm_search_query,
                contextvm_provider_filter,
                contextvm_copy_status,
                contextvm_schema_expanded,
                ..
            } => {
                // Phase 28: Lock screen -- shown on cold launch when auth is required.
                // Must be checked before any other screen so no content leaks (T-28-23).
                if matches!(&state.router.current_screen, Screen::Locked) {
                    return lock_screen::view(lock_pin_input, state.toast.as_deref(), *is_dark);
                }

                // Phase 28: PIN setup screen -- shown on first launch (no auth params yet, D-14).
                if matches!(&state.router.current_screen, Screen::PinSetup) {
                    return pin_setup_screen::view(
                        setup_pin_input,
                        setup_confirm_input,
                        setup_duress_input,
                        state.toast.as_deref(),
                        *is_dark,
                    );
                }

                // Onboarding screen: full-screen overlay (no sidebar)
                if let Screen::Onboarding { step } = &state.router.current_screen {
                    return views::onboarding::view(
                        state,
                        step,
                        onboarding_selected_backend,
                        onboarding_api_key,
                        *onboarding_show_learn_more,
                        *is_dark,
                    );
                }

                // Settings screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::Settings) {
                    return views::settings::view(
                        state,
                        *is_dark,
                        *settings_show_advanced,
                        settings_attestation_interval,
                        settings_brave_api_key,
                        settings_brave_api_key_message.as_deref(),
                        *theme_override,
                    );
                }

                // SettingsProviders screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::SettingsProviders) {
                    return views::settings_providers::view(
                        state,
                        *is_dark,
                        settings_add_name,
                        settings_add_url,
                        settings_add_key,
                        settings_add_tee,
                        settings_preset_keys,
                    );
                }

                // SettingsDefaults screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::SettingsDefaults) {
                    return views::settings_defaults::view(
                        state,
                        *is_dark,
                        settings_default_model,
                        settings_default_instructions,
                    );
                }

                // Documents screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::Documents) {
                    return views::documents::view(state, *is_dark);
                }

                // DirectorySources screen (Phase 32 DIR-05): full-screen overlay.
                if matches!(&state.router.current_screen, Screen::DirectorySources) {
                    return views::directory_sources::view(
                        state,
                        dir_watcher_warning.as_deref(),
                        dir_editing_exclusions_for.as_deref(),
                        dir_exclusion_edit_text,
                        dir_exclusion_validation,
                        dir_pending_remove_id.as_deref(),
                        *is_dark,
                    );
                }

                // Memories screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::Memories) {
                    return views::memories::view(state, memory_edit_state, *is_dark);
                }

                // Phase 35 — Tool Discovery screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::ToolDiscovery) {
                    return views::tool_discovery::view(
                        state,
                        contextvm_search_query,
                        contextvm_provider_filter,
                        *is_dark,
                    );
                }

                // Phase 36 — Tool Detail screen: full-screen overlay (no sidebar)
                if let Screen::ContextvmToolDetail { tool_id } = &state.router.current_screen {
                    return views::tool_detail::view(
                        state,
                        tool_id,
                        contextvm_copy_status.as_deref(),
                        *contextvm_schema_expanded,
                        *is_dark,
                    );
                }

                // Agents screen: full-screen overlay (no sidebar)
                if mango_core::features::AGENTS_ENABLED
                    && matches!(&state.router.current_screen, Screen::Agents)
                {
                    return views::agents::agent_list_view(state, agent_task_input, *is_dark);
                }

                let sidebar = views::home::sidebar_view(state, rename_state, *is_dark);

                let chat_area = match &state.router.current_screen {
                    Screen::Chat { .. } => views::chat::chat_view(
                        state,
                        cached_theme,
                        *is_dark,
                        streaming_content,
                        input_text,
                        edit_state,
                        rename_state,
                        *show_attestation_detail,
                        *show_system_prompt_input,
                        system_prompt_text,
                        parsed_messages,
                        *show_docs_attachment_overlay,
                        *show_conv_menu,
                        *force_remote_next,
                        *show_tools_panel,
                        image_cache,
                    ),
                    _ => {
                        // Home: show welcome/empty chat area
                        center(
                            column![
                                text("Mango").size(28),
                                text("Select or create a conversation to begin.").size(16),
                            ]
                            .spacing(12)
                            .align_x(iced::Alignment::Center),
                        )
                        .into()
                    }
                };

                row![sidebar, chat_area].into()
            }
        }
    }
}

/// Stable identity wrapper used by iced's `Subscription::run_with` to dedupe
/// the directory-trigger subscription across renders. We hash by the Arc
/// pointer of the receiver (one receiver per App instance).
#[derive(Clone)]
struct DirTriggerId(flume::Receiver<Message>);

impl Hash for DirTriggerId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use the address of the underlying inner as identity surrogate.
        (&self.0 as *const _ as usize).hash(state);
    }
}

// ── Phase 32 DIR-05: helpers ──────────────────────────────────────────────────

/// Resolve (absolute_path, exclusion_globs) for a source_id by looking up the
/// watched_paths map (populated by AddFolder) for the path and AppState for the
/// globs. Returns None if the source is not yet registered on the desktop side.
fn lookup_source_path_and_globs(
    state: &AppState,
    watched: &Arc<Mutex<HashMap<String, String>>>,
    source_id: &str,
) -> Option<(String, Vec<String>)> {
    let path = watched.lock().ok()?.get(source_id).cloned()?;
    let globs = state
        .directory_sources
        .iter()
        .find(|s| s.id == source_id)
        .map(|s| s.exclusion_globs.clone())
        .unwrap_or_default();
    Some((path, globs))
}

// ── Phase 32 DIR-05: directory sync workers + walker pipeline ─────────────────

/// Spawn the long-lived watcher + fallback-ticker threads on app startup.
///
/// Three workers are started:
///
/// 1. A notify-debouncer-mini debouncer (2s debounce per D-10) watching every
///    registered directory source's absolute path recursively. Change events
///    dispatch [`Message::DirSyncTriggered`] per affected source.
///
/// 2. A 5-minute tokio interval ticker (D-21 belt-and-braces fallback) that
///    fires [`Message::DirSyncIntervalTick`] so even if inotify is deaf the
///    sources still re-sync periodically.
///
/// 3. A state-subscriber that keeps the watched-paths map in sync with
///    AppState and rebuilds the watcher set as sources are added or removed.
///    Also flips to PollWatcher (60s) when ENOSPC / watch-limit is reported
///    by notify, emitting [`Message::DirWatcherFallbackWarning`].
fn spawn_directory_sync_workers(
    _manager: AppManager,
    trigger_tx: flume::Sender<Message>,
    watched_paths: Arc<Mutex<HashMap<String, String>>>,
) {
    use notify_debouncer_mini::new_debouncer;
    use notify_debouncer_mini::notify::{
        Config as NotifyConfig, Event as NotifyEvent, EventHandler, PollWatcher, RecursiveMode,
        Watcher,
    };

    // ── Watcher thread ────────────────────────────────────────────────────────
    // Strategy: build a RecommendedWatcher-backed debouncer (2s timeout per
    // D-10) as the primary watcher. If either the debouncer or a subsequent
    // `watch()` call fails with ENOSPC / watch-limit exhaustion, fall back to
    // a plain `PollWatcher` with a 60s poll interval (D-11) and surface a UI
    // warning banner. The 5-minute ticker (below) is the belt-and-braces
    // belt-and-braces fallback either way.
    {
        let trigger_tx = trigger_tx.clone();
        let watched_paths = watched_paths.clone();
        std::thread::spawn(move || {
            // Shared event-raw-stream channel: both the RecommendedWatcher
            // debouncer and the PollWatcher fallback push events here.
            let (ev_tx, ev_rx) = flume::unbounded::<Vec<std::path::PathBuf>>();

            // Primary: RecommendedWatcher-backed debouncer with 2s timeout.
            let ev_tx_primary = ev_tx.clone();
            let primary = new_debouncer(
                StdDuration::from_secs(2),
                move |res: notify_debouncer_mini::DebounceEventResult| match res {
                    Ok(events) => {
                        let paths: Vec<std::path::PathBuf> =
                            events.into_iter().map(|e| e.path).collect();
                        let _ = ev_tx_primary.send(paths);
                    }
                    Err(e) => {
                        eprintln!("[dir-sync] debouncer error: {e:?}");
                    }
                },
            );

            // If the RecommendedWatcher failed to init, spin up PollWatcher
            // with poll_interval(60s). This trait-object dance avoids the
            // generic-parameter mismatch between Debouncer<InotifyWatcher>
            // and Debouncer<PollWatcher>.
            struct PollHandler {
                tx: flume::Sender<Vec<std::path::PathBuf>>,
            }
            impl EventHandler for PollHandler {
                fn handle_event(
                    &mut self,
                    res: notify_debouncer_mini::notify::Result<NotifyEvent>,
                ) {
                    if let Ok(ev) = res {
                        let _ = self.tx.send(ev.paths);
                    }
                }
            }

            let mut poll_watcher: Option<PollWatcher> = None;
            let mut debouncer = match primary {
                Ok(d) => Some(d),
                Err(e) => {
                    eprintln!(
                        "[dir-sync] RecommendedWatcher init failed: {e}; using PollWatcher(60s)"
                    );
                    let _ = trigger_tx.send(Message::DirWatcherFallbackWarning(Some(
                        "File watching unavailable; syncing on schedule every 5 min".to_string(),
                    )));
                    let ev_tx_poll = ev_tx.clone();
                    let cfg =
                        NotifyConfig::default().with_poll_interval(StdDuration::from_secs(60));
                    match PollWatcher::new(PollHandler { tx: ev_tx_poll }, cfg) {
                        Ok(w) => {
                            poll_watcher = Some(w);
                        }
                        Err(e2) => eprintln!("[dir-sync] PollWatcher init failed: {e2}"),
                    }
                    None
                }
            };

            let mut using_poll_fallback = poll_watcher.is_some();

            // Main loop: reconcile watch registrations + drain events.
            let mut registered_paths: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            loop {
                // Apply any newly-registered paths.
                if let Ok(guard) = watched_paths.lock() {
                    for (_sid, p) in guard.iter() {
                        if registered_paths.contains(p) {
                            continue;
                        }
                        let path = std::path::Path::new(p);
                        let result: Result<(), notify_debouncer_mini::notify::Error> =
                            if let Some(d) = debouncer.as_mut() {
                                d.watcher().watch(path, RecursiveMode::Recursive)
                            } else if let Some(pw) = poll_watcher.as_mut() {
                                pw.watch(path, RecursiveMode::Recursive)
                            } else {
                                Ok(())
                            };
                        match result {
                            Ok(()) => {
                                registered_paths.insert(p.clone());
                            }
                            Err(e) => {
                                let msg = e.to_string();
                                if !using_poll_fallback
                                    && (msg.contains("No space left")
                                        || msg.contains("ENOSPC")
                                        || msg.contains("watch limit"))
                                {
                                    eprintln!(
                                        "[dir-sync] inotify exhausted ({msg}); switching to PollWatcher"
                                    );
                                    let _ = trigger_tx.send(
                                        Message::DirWatcherFallbackWarning(Some(
                                            "File watching unavailable; syncing on schedule every 5 min"
                                                .to_string(),
                                        )),
                                    );
                                    // Drop the primary debouncer and build
                                    // a PollWatcher fallback.
                                    debouncer = None;
                                    let ev_tx_poll = ev_tx.clone();
                                    let cfg = NotifyConfig::default()
                                        .with_poll_interval(StdDuration::from_secs(60));
                                    match PollWatcher::new(PollHandler { tx: ev_tx_poll }, cfg) {
                                        Ok(w) => {
                                            poll_watcher = Some(w);
                                            using_poll_fallback = true;
                                            // Force re-registration of all
                                            // paths on the new watcher.
                                            registered_paths.clear();
                                        }
                                        Err(e2) => {
                                            eprintln!("[dir-sync] PollWatcher init failed: {e2}")
                                        }
                                    }
                                } else {
                                    eprintln!("[dir-sync] watch({p}) failed: {msg}");
                                }
                            }
                        }
                    }
                }

                // Drain raw-path events with short timeout.
                match ev_rx.recv_timeout(StdDuration::from_millis(500)) {
                    Ok(paths) => {
                        let map_snapshot =
                            watched_paths.lock().map(|g| g.clone()).unwrap_or_default();
                        let mut fired: std::collections::HashSet<String> =
                            std::collections::HashSet::new();
                        for p in paths {
                            let ev_path_str = p.to_string_lossy().to_string();
                            let mut best_match: Option<(&String, usize)> = None;
                            for (sid, root) in &map_snapshot {
                                if ev_path_str.starts_with(root.as_str()) {
                                    let len = root.len();
                                    if best_match.map(|(_, l)| len > l).unwrap_or(true) {
                                        best_match = Some((sid, len));
                                    }
                                }
                            }
                            if let Some((sid, _)) = best_match {
                                fired.insert(sid.clone());
                            }
                        }
                        for sid in fired {
                            let _ = trigger_tx.send(Message::DirSyncTriggered(sid));
                        }
                    }
                    Err(flume::RecvTimeoutError::Timeout) => {
                        // Normal idle — continue loop to re-check new watches.
                    }
                    Err(flume::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });
    }

    // ── 5-minute fallback interval ticker (D-21) ─────────────────────────────
    {
        let trigger_tx_iv = trigger_tx.clone();
        std::thread::spawn(move || {
            // Simple sleep loop — we don't need tokio for a 5-minute tick.
            // Using std::thread::sleep keeps the worker self-contained and
            // avoids depending on the iced runtime executor for cadence.
            loop {
                std::thread::sleep(StdDuration::from_secs(60 * 5));
                let _ = trigger_tx_iv.send(Message::DirSyncIntervalTick);
            }
        });
    }
}

/// Run the desktop walker pipeline for a single directory source.
///
/// Steps:
/// 1. Enumerate files on disk via `walk_with_exclusions`.
/// 2. Fetch stored fingerprints via `FfiApp::list_directory_fingerprints`.
/// 3. Diff with `diff_files`.
/// 4. For each 50-file batch in (added ∪ modified), read bytes and dispatch
///    `SyncDirectoryFiles { files, removed_paths (first batch only), is_final_batch }`.
/// 5. If no adds/mods but there are removals, dispatch a single final batch
///    with empty `files` + `removed_paths`.
fn run_desktop_sync(
    manager: AppManager,
    source_id: String,
    root_path: String,
    exclusion_globs: Vec<String>,
    in_flight: Arc<Mutex<HashSet<String>>>,
    done_tx: flume::Sender<Message>,
) {
    std::thread::spawn(move || {
        // Guard against overlapping runs for the same source.
        {
            let mut guard = match in_flight.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            if !guard.insert(source_id.clone()) {
                return;
            }
        }
        let cleanup_in_flight = || {
            if let Ok(mut g) = in_flight.lock() {
                g.remove(&source_id);
            }
        };

        // 1. Enumerate.
        let current: Vec<(String, i64, i64)> =
            match mango_core::rag::directory_sync::walk_with_exclusions(
                &root_path,
                &exclusion_globs,
            ) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("[dir-sync] walk_with_exclusions({root_path}) failed: {e}");
                    cleanup_in_flight();
                    let _ = done_tx.send(Message::DirSyncCompleted);
                    return;
                }
            };

        // 2. Fetch stored fingerprints.
        let stored_rows = match manager.ffi.list_directory_fingerprints(source_id.clone()) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[dir-sync] list_directory_fingerprints failed: {e}");
                cleanup_in_flight();
                let _ = done_tx.send(Message::DirSyncCompleted);
                return;
            }
        };
        let stored: Vec<mango_core::rag::directory_sync::StoredFingerprint> = stored_rows
            .into_iter()
            .map(|f| mango_core::rag::directory_sync::StoredFingerprint {
                file_path: f.relative_path,
                mtime_secs: f.mtime_secs,
                size_bytes: f.size_bytes,
            })
            .collect();

        // 3. Diff.
        let diff = mango_core::rag::directory_sync::diff_files(&stored, &current);

        // Build the list of (relative_path, mtime, size) tuples we need bytes for.
        let mut to_read: Vec<(String, i64, i64)> = Vec::new();
        to_read.extend(diff.added.iter().cloned());
        to_read.extend(diff.modified.iter().cloned());
        let removed: Vec<String> = diff.removed.clone();

        // No-op fast path: nothing to add/modify AND nothing to remove.
        if to_read.is_empty() && removed.is_empty() {
            cleanup_in_flight();
            let _ = done_tx.send(Message::DirSyncCompleted);
            return;
        }

        // 4+5. Dispatch batches of up to 50 files. `removed_paths` attach to the
        // first batch; `is_final_batch` marks the last dispatched batch.
        if to_read.is_empty() && !removed.is_empty() {
            // Empty-files final batch carrying the removals.
            manager.dispatch(AppAction::SyncDirectoryFiles {
                source_id: source_id.clone(),
                files: vec![],
                removed_paths: removed,
                is_final_batch: true,
            });
            cleanup_in_flight();
            let _ = done_tx.send(Message::DirSyncCompleted);
            return;
        }

        // HI-03: skip oversized files before `std::fs::read` pulls the whole blob
        // into memory. 32 MiB is ample for the markdown/PDF/txt files that make
        // up realistic RAG corpora and avoids OOM on large bundled attachments.
        const MAX_FILE_BYTES: i64 = 32 * 1024 * 1024;

        let root = std::path::PathBuf::from(&root_path);
        let batches: Vec<Vec<(String, i64, i64)>> =
            to_read.chunks(50).map(|c| c.to_vec()).collect();
        let batch_count = batches.len();
        for (idx, batch) in batches.into_iter().enumerate() {
            let mut files: Vec<DirectoryFileEntry> = Vec::with_capacity(batch.len());
            for (rel, mtime, size) in batch {
                if size > MAX_FILE_BYTES {
                    eprintln!(
                        "[dir-sync] skipping oversized file {} ({} bytes > {} cap)",
                        rel, size, MAX_FILE_BYTES
                    );
                    continue;
                }
                let abs = root.join(&rel);
                match std::fs::read(&abs) {
                    Ok(bytes) => {
                        files.push(DirectoryFileEntry {
                            relative_path: rel,
                            mtime_secs: mtime,
                            size_bytes: size,
                            content: bytes,
                        });
                    }
                    Err(e) => {
                        eprintln!("[dir-sync] failed to read {}: {e}", abs.display());
                    }
                }
            }
            let is_final = idx + 1 == batch_count;
            let removed_paths = if idx == 0 {
                removed.clone()
            } else {
                Vec::new()
            };
            manager.dispatch(AppAction::SyncDirectoryFiles {
                source_id: source_id.clone(),
                files,
                removed_paths,
                is_final_batch: is_final,
            });
        }
        cleanup_in_flight();
        let _ = done_tx.send(Message::DirSyncCompleted);
    });
}
