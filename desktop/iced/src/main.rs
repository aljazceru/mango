use iced::widget::{center, column, row, text};
use iced::{Element, Subscription, Task, Theme};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mango_core::{
    AppAction, AppReconciler, AppState, AppUpdate, DesktopKeychainProvider, FfiApp,
    NullEmbeddingProvider, OnboardingStep, Screen, TeeType,
};
use mango_core::embedding::desktop::DesktopEmbeddingProvider;

mod theme;
mod views;
mod widgets;

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
        Self { theme_override: ThemeOverride::FollowSystem }
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

impl AppManager {
    fn new() -> Result<Self, String> {
        let data_dir = std::env::temp_dir()
            .join("mango")
            .to_string_lossy()
            .to_string();
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
                    Box::new(NullEmbeddingProvider) as Box<dyn mango_core::embedding::EmbeddingProvider>,
                    mango_core::EmbeddingStatus::Degraded,
                )
            }
        };
        let ffi = FfiApp::new(data_dir, Box::new(DesktopKeychainProvider), embedding_provider, embedding_status);
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
        // OS dark/light theme state (updated via SystemThemeChanged subscription)
        is_dark: bool,
        // Cached theme derived from is_dark; updated whenever is_dark changes
        cached_theme: Theme,
        // Manual theme override preference (per D-06, D-07)
        theme_override: ThemeOverride,
        // AGENTS HIDDEN: agent_task_input removed until polished
    },
}

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
    SettingsSubmitAddBackend { name: String, url: String, key: String, tee: TeeType },
    // Simple "enable provider" flow: per-preset API key field changes
    SettingsPresetKeyChanged { preset_id: String, key: String },
    // Submit the simple "enable provider" flow for a preset
    SettingsEnablePreset { preset_id: String },
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
    RunSetupWizard,
    // Documents screen messages (Phase 8, LRAG-06)
    OpenDocuments,
    PickDocumentFile,
    DeleteDocument(String),
    ToggleDocumentAttachment(String),
    ToggleDocAttachmentOverlay,
    // AGENTS HIDDEN: OpenAgents, AgentTaskInputChanged, LaunchAgent removed until polished
    // Window close request (D-12: checkpoint running agent sessions on exit)
    WindowCloseRequested,
    // OS dark/light theme change
    SystemThemeChanged(bool),
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
                    is_dark: initial_dark,
                    cached_theme: theme::app_theme(initial_dark),
                    theme_override: prefs.theme_override,
                    // AGENTS HIDDEN: agent_task_input removed
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
            App::Loaded { manager, .. } => {
                let core_updates = Subscription::run_with(manager.clone(), manager_update_stream)
                    .map(|_| Message::CoreUpdated);

                // D-12: Listen for window close to checkpoint running agent sessions
                let window_close = iced::event::listen_with(|event, _status, _id| {
                    if let iced::Event::Window(iced::window::Event::CloseRequested) = event {
                        Some(Message::WindowCloseRequested)
                    } else {
                        None
                    }
                });

                let theme_sub = iced::system::theme_changes()
                    .map(|mode| Message::SystemThemeChanged(mode == iced::theme::Mode::Dark));

                Subscription::batch(vec![core_updates, window_close, theme_sub])
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
                is_dark,
                cached_theme,
                theme_override,
            } => {
                match message {
                    Message::CoreUpdated => {
                        let latest = manager.state();
                        if latest.rev > state.rev {
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
                                        *streaming_content =
                                            iced::widget::markdown::Content::new();
                                        streaming_content.push_str(new_text);
                                        *prev_streaming_len = new_len;
                                    }
                                }
                                (None, Some(_)) => {
                                    // StreamDone: reset streaming content
                                    *streaming_content =
                                        iced::widget::markdown::Content::new();
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
                            *state = latest;
                        }
                    }

                    Message::DispatchAction(action) => {
                        manager.dispatch(action);
                    }

                    Message::InputChanged(val) => {
                        *input_text = val;
                    }

                    Message::SubmitMessage => {
                        let text_to_send = input_text.trim().to_string();
                        if !text_to_send.is_empty() {
                            manager.dispatch(AppAction::SendMessage { text: text_to_send });
                            *input_text = String::new();
                        }
                    }

                    Message::OpenConversation(id) => {
                        *show_system_prompt_input = false;
                        *system_prompt_text = String::new();
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
                                manager.dispatch(AppAction::RenameConversation {
                                    id,
                                    title: trimmed,
                                });
                            }
                        }
                    }

                    Message::CancelRename => {
                        *rename_state = None;
                    }

                    Message::ConfirmDelete(id) => {
                        manager.dispatch(AppAction::DeleteConversation { id });
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
                        // Use rfd for native file dialog (blocking, run via spawn_blocking)
                        let manager_clone = manager.clone();
                        let fut = async move {
                            let result = tokio::task::spawn_blocking(move || -> Option<()> {
                                let path = rfd::FileDialog::new().pick_file()?;
                                let filename = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "attachment".to_string());
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
                                            message: "This file type cannot be read as text.".to_string(),
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

                    Message::ClearAttachment => {
                        manager.dispatch(AppAction::ClearAttachment);
                    }

                    Message::SelectModel(model_id) => {
                        manager.dispatch(AppAction::SelectModel { model_id });
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
                        // URL opening via `open` crate would go here (not a required dep)
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
                    Message::SettingsSubmitAddBackend { name, url, key, tee } => {
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
                        let api_key = settings_preset_keys.get(&preset_id).cloned().unwrap_or_default();
                        if !api_key.trim().is_empty() {
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
                        if !api_key.is_empty() && !preset_id.is_empty() {
                            // First, add/enable the backend from the preset (idempotent).
                            manager.dispatch(AppAction::AddBackendFromPreset {
                                preset_id: preset_id.clone(),
                                api_key: api_key.clone(),
                            });
                            // Then trigger the health-check / attestation flow.
                            manager.dispatch(AppAction::ValidateApiKey { backend_id: preset_id });
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
                        manager.dispatch(AppAction::ValidateApiKey { backend_id: preset_id });
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
                                            message: "Failed to read the selected file.".to_string(),
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

                    // AGENTS HIDDEN: OpenAgents, AgentTaskInputChanged, LaunchAgent handlers removed

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
                        save_preferences(&Preferences { theme_override: new_override });
                    }

                    // D-12: On window close, checkpoint all running agent sessions to SQLite
                    Message::WindowCloseRequested => {
                        for session in &state.agent_sessions {
                            if session.status == "running" {
                                manager.dispatch(AppAction::PauseAgentSession {
                                    session_id: session.id.clone(),
                                });
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
                column![
                    text("Mango").size(24),
                    text(format!("Error: {error}")),
                ]
                .spacing(12),
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
                is_dark,
                cached_theme,
                theme_override,
                ..
            } => {
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
                        settings_add_name,
                        settings_add_url,
                        settings_add_key,
                        settings_add_tee,
                        settings_default_model,
                        settings_preset_keys,
                        *settings_show_advanced,
                        settings_attestation_interval,
                        settings_default_instructions,
                        *theme_override,
                    );
                }

                // Documents screen: full-screen overlay (no sidebar)
                if matches!(&state.router.current_screen, Screen::Documents) {
                    return views::documents::view(state, *is_dark);
                }

                // AGENTS HIDDEN: Screen::Agents overlay removed until polished

                let sidebar = views::home::sidebar_view(state, rename_state, *is_dark);

                let chat_area = match &state.router.current_screen {
                    Screen::Chat { .. } => views::chat::chat_view(
                        state,
                        cached_theme,
                        *is_dark,
                        streaming_content,
                        input_text,
                        edit_state,
                        *show_attestation_detail,
                        *show_system_prompt_input,
                        system_prompt_text,
                        parsed_messages,
                        *show_docs_attachment_overlay,
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
