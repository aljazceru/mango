use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_input, toggler,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};
use std::fmt;

use mango_core::{
    AppAction, AppState, BackendSummary, HealthStatus, HybridProfile, LocalPreprocessing,
    RoutingPolicy, Screen, TeeType,
};

use crate::Message;

// ── Small helpers ─────────────────────────────────────────────────────────────

pub(crate) fn section_header<'a>(label: &'a str, muted: Color) -> Element<'a, Message> {
    container(text(label).size(11).color(muted))
        .padding(Padding {
            top: 20.0,
            bottom: 6.0,
            left: 16.0,
            right: 16.0,
        })
        .into()
}

pub(crate) fn divider() -> Element<'static, Message> {
    container(rule::horizontal(1))
        .padding(Padding::from([0u16, 16]))
        .into()
}

pub(crate) fn action_btn<'a>(
    label: &'a str,
    msg: Message,
    enabled: bool,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let (bg_color, muted, border) = (vc.bg, vc.muted, vc.border);
    let (accent_color, accent_text) = (vc.accent, bg_color);
    if enabled {
        button(text(label).size(13).color(accent_text))
            .on_press(msg)
            .padding(Padding::from([6u16, 16]))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(accent_color)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        button(text(label).size(13).color(muted))
            .padding(Padding::from([6u16, 16]))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(vc.ghost_overlay)),
                border: Border {
                    radius: 6.0.into(),
                    color: border,
                    width: 1.0,
                },
                ..Default::default()
            })
            .into()
    }
}

// ── Main view ─────────────────────────────────────────────────────────────────

/// Lock timeout options: (display label, seconds). -1 = Never.
const LOCK_TIMEOUT_OPTIONS: &[(&str, i64)] = &[
    ("Immediately", 0),
    ("1 minute", 60),
    ("5 minutes", 300),
    ("15 minutes", 900),
    ("Never", -1),
];

fn lock_timeout_label(seconds: i64) -> &'static str {
    LOCK_TIMEOUT_OPTIONS
        .iter()
        .find(|&&(_, s)| s == seconds)
        .map(|(label, _)| *label)
        .unwrap_or("5 minutes")
}

fn is_hybrid_local_backend(backend: &BackendSummary) -> bool {
    (backend.id.starts_with("local-") || backend.id == "qvac-local") && !backend.models.is_empty()
}

fn is_hybrid_remote_backend(backend: &BackendSummary) -> bool {
    !is_hybrid_local_backend(backend)
        && backend.tee_type != TeeType::Unknown
        && backend.has_api_key
        && !backend.models.is_empty()
        && backend.health_status != HealthStatus::Failed
}

fn compact_model_name(model_id: &str) -> String {
    model_id
        .rsplit('/')
        .next()
        .unwrap_or(model_id)
        .replace(['_', '-'], " ")
        .chars()
        .take(32)
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HybridModelOption {
    id: String,
    label: String,
}

impl fmt::Display for HybridModelOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn hybrid_model_options(models: &[String]) -> Vec<HybridModelOption> {
    models
        .iter()
        .map(|model| HybridModelOption {
            id: model.clone(),
            label: compact_model_name(model),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HybridBackendOption {
    id: String,
    label: String,
}

impl fmt::Display for HybridBackendOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

fn hybrid_backend_options(backends: &[BackendSummary]) -> Vec<HybridBackendOption> {
    backends
        .iter()
        .map(|backend| HybridBackendOption {
            id: backend.id.clone(),
            label: backend.name.clone(),
        })
        .collect()
}

fn selected_backend_for_role(
    backends: &[BackendSummary],
    existing_backend_id: Option<&str>,
) -> Option<BackendSummary> {
    existing_backend_id
        .and_then(|backend_id| backends.iter().find(|backend| backend.id == backend_id))
        .cloned()
        .or_else(|| backends.first().cloned())
}

fn selected_model_for_backend(
    backend: &BackendSummary,
    existing_model: Option<&str>,
    selected_model: Option<&str>,
) -> String {
    selected_model
        .filter(|model| backend.models.iter().any(|candidate| candidate == model))
        .or(existing_model
            .filter(|model| backend.models.iter().any(|candidate| candidate == model)))
        .map(ToOwned::to_owned)
        .or_else(|| backend.models.first().cloned())
        .unwrap_or_default()
}

fn default_hybrid_profile(
    local_backend: &BackendSummary,
    local_model_id: String,
    remote_backend: &BackendSummary,
    remote_model_id: String,
    existing_profile: Option<&HybridProfile>,
) -> HybridProfile {
    HybridProfile {
        id: existing_profile
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "default_hybrid".to_string()),
        name: format!("{} -> {}", local_backend.name, remote_backend.name),
        local_backend_id: local_backend.id.clone(),
        local_model_id,
        remote_backend_id: remote_backend.id.clone(),
        remote_model_id,
        policy: existing_profile
            .map(|p| p.policy.clone())
            .unwrap_or(RoutingPolicy {
                escalate_if_attachment: true,
                prefer_local_when_offline: true,
                escalate_if_message_longer_than: Some(4000),
            }),
        preprocessing: existing_profile.map(|p| p.preprocessing.clone()).unwrap_or(
            LocalPreprocessing {
                compress_history: false,
                rewrite_rag_query: false,
            },
        ),
    }
}

fn hybrid_profile_controls<'a>(
    profile: &'a HybridProfile,
    active: bool,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let mut attachment_profile = profile.clone();
    attachment_profile.policy.escalate_if_attachment =
        !attachment_profile.policy.escalate_if_attachment;
    let mut offline_profile = profile.clone();
    offline_profile.policy.prefer_local_when_offline =
        !offline_profile.policy.prefer_local_when_offline;
    let mut long_prompt_profile = profile.clone();
    long_prompt_profile.policy.escalate_if_message_longer_than = if long_prompt_profile
        .policy
        .escalate_if_message_longer_than
        .is_some()
    {
        None
    } else {
        Some(4000)
    };

    let activate_btn = action_btn(
        if active { "Default" } else { "Use by default" },
        Message::DispatchAction(AppAction::SetActiveHybridProfile {
            profile_id: profile.id.clone(),
        }),
        !active,
        vc,
    );
    let delete_btn = action_btn(
        "Delete",
        Message::DispatchAction(AppAction::DeleteHybridProfile {
            profile_id: profile.id.clone(),
        }),
        true,
        vc,
    );

    let attachments_label = if profile.policy.escalate_if_attachment {
        "Attachments remote: On"
    } else {
        "Attachments remote: Off"
    };
    let offline_label = if profile.policy.prefer_local_when_offline {
        "Offline local: On"
    } else {
        "Offline local: Off"
    };
    let long_label = if profile.policy.escalate_if_message_longer_than.is_some() {
        "Long prompts remote: On"
    } else {
        "Long prompts remote: Off"
    };

    container(
        column![
            text(&profile.name).size(13).color(vc.text),
            text(format!(
                "{} -> {}",
                compact_model_name(&profile.local_model_id),
                compact_model_name(&profile.remote_model_id)
            ))
            .size(11)
            .color(vc.muted),
            row![activate_btn, delete_btn]
                .spacing(8)
                .align_y(Alignment::Center),
            row![
                action_btn(
                    attachments_label,
                    Message::DispatchAction(AppAction::SaveHybridProfile {
                        profile: attachment_profile,
                    }),
                    true,
                    vc,
                ),
                action_btn(
                    offline_label,
                    Message::DispatchAction(AppAction::SaveHybridProfile {
                        profile: offline_profile,
                    }),
                    true,
                    vc,
                ),
                action_btn(
                    long_label,
                    Message::DispatchAction(AppAction::SaveHybridProfile {
                        profile: long_prompt_profile,
                    }),
                    true,
                    vc,
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8),
    )
    .padding(Padding::from([10u16, 12]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.secondary_surface)),
        border: Border {
            radius: 8.0.into(),
            color: vc.border,
            width: 1.0,
        },
        ..Default::default()
    })
    .into()
}

fn hybrid_routing_card<'a>(
    state: &'a AppState,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let local_backends: Vec<BackendSummary> = state
        .backends
        .iter()
        .filter(|backend| is_hybrid_local_backend(backend))
        .cloned()
        .collect();
    let remote_backends: Vec<BackendSummary> = state
        .backends
        .iter()
        .filter(|backend| is_hybrid_remote_backend(backend))
        .cloned()
        .collect();
    let existing = state.hybrid_profiles.first();
    let local_backend = selected_backend_for_role(
        &local_backends,
        existing.map(|p| p.local_backend_id.as_str()),
    );
    let remote_backend = selected_backend_for_role(
        &remote_backends,
        existing.map(|p| p.remote_backend_id.as_str()),
    );

    let mut children: Vec<Element<'a, Message>> = vec![
        text("Local to confidential").size(14).color(vc.text).into(),
        text("Route ordinary turns locally and escalate attachments, long prompts, or explicit remote sends to a confidential provider.")
            .size(11)
            .color(vc.muted)
            .into(),
    ];

    match (local_backend, remote_backend) {
        (Some(local), Some(remote)) => {
            let local_model = selected_model_for_backend(
                &local,
                existing.map(|profile| profile.local_model_id.as_str()),
                existing.map(|profile| profile.local_model_id.as_str()),
            );
            let remote_model = selected_model_for_backend(
                &remote,
                existing.map(|profile| profile.remote_model_id.as_str()),
                existing.map(|profile| profile.remote_model_id.as_str()),
            );
            let profile = default_hybrid_profile(
                &local,
                local_model.clone(),
                &remote,
                remote_model.clone(),
                existing,
            );
            children.push(
                text(format!(
                    "{} / {} -> {} / {}",
                    local.name,
                    compact_model_name(profile.local_model_id.as_str()),
                    remote.name,
                    compact_model_name(profile.remote_model_id.as_str()),
                ))
                .size(11)
                .color(vc.muted)
                .into(),
            );
            let local_backend_options = hybrid_backend_options(&local_backends);
            let selected_local_backend = local_backend_options
                .iter()
                .find(|option| option.id == local.id)
                .cloned();
            let remote_backend_options = hybrid_backend_options(&remote_backends);
            let selected_remote_backend = remote_backend_options
                .iter()
                .find(|option| option.id == remote.id)
                .cloned();

            let local_backends_for_picker = local_backends.clone();
            let remote_for_local_backend = remote.clone();
            let existing_for_local_backend = existing.cloned();
            let remote_model_for_local_backend = remote_model.clone();
            children.push(
                row![
                    text("Local backend").size(12).color(vc.muted),
                    pick_list(
                        local_backend_options,
                        selected_local_backend,
                        move |option| {
                            let selected_local = selected_backend_for_role(
                                &local_backends_for_picker,
                                Some(option.id.as_str()),
                            )
                            .unwrap_or_else(|| local_backends_for_picker[0].clone());
                            let selected_local_model = selected_model_for_backend(
                                &selected_local,
                                existing_for_local_backend
                                    .as_ref()
                                    .map(|profile| profile.local_model_id.as_str()),
                                None,
                            );
                            Message::DispatchAction(AppAction::SaveHybridProfile {
                                profile: default_hybrid_profile(
                                    &selected_local,
                                    selected_local_model,
                                    &remote_for_local_backend,
                                    remote_model_for_local_backend.clone(),
                                    existing_for_local_backend.as_ref(),
                                ),
                            })
                        }
                    )
                    .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );

            let local_for_remote_backend = local.clone();
            let remote_backends_for_picker = remote_backends.clone();
            let existing_for_remote_backend = existing.cloned();
            let local_model_for_remote_backend = local_model.clone();
            children.push(
                row![
                    text("Remote backend").size(12).color(vc.muted),
                    pick_list(
                        remote_backend_options,
                        selected_remote_backend,
                        move |option| {
                            let selected_remote = selected_backend_for_role(
                                &remote_backends_for_picker,
                                Some(option.id.as_str()),
                            )
                            .unwrap_or_else(|| remote_backends_for_picker[0].clone());
                            let selected_remote_model = selected_model_for_backend(
                                &selected_remote,
                                existing_for_remote_backend
                                    .as_ref()
                                    .map(|profile| profile.remote_model_id.as_str()),
                                None,
                            );
                            Message::DispatchAction(AppAction::SaveHybridProfile {
                                profile: default_hybrid_profile(
                                    &local_for_remote_backend,
                                    local_model_for_remote_backend.clone(),
                                    &selected_remote,
                                    selected_remote_model,
                                    existing_for_remote_backend.as_ref(),
                                ),
                            })
                        }
                    )
                    .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );

            let local_options = hybrid_model_options(&local.models);
            let selected_local = local_options
                .iter()
                .find(|option| option.id == local_model)
                .cloned();
            let remote_options = hybrid_model_options(&remote.models);
            let selected_remote = remote_options
                .iter()
                .find(|option| option.id == remote_model)
                .cloned();

            let remote_for_local = remote.clone();
            let local_for_local = local.clone();
            let existing_for_local = existing.cloned();
            let remote_model_for_local = remote_model.clone();
            children.push(
                row![
                    text("Local model").size(12).color(vc.muted),
                    pick_list(local_options, selected_local, move |option| {
                        Message::DispatchAction(AppAction::SaveHybridProfile {
                            profile: default_hybrid_profile(
                                &local_for_local,
                                option.id,
                                &remote_for_local,
                                remote_model_for_local.clone(),
                                existing_for_local.as_ref(),
                            ),
                        })
                    })
                    .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );

            let local_for_remote = local.clone();
            let remote_for_remote = remote.clone();
            let existing_for_remote = existing.cloned();
            let local_model_for_remote = local_model.clone();
            children.push(
                row![
                    text("Remote model").size(12).color(vc.muted),
                    pick_list(remote_options, selected_remote, move |option| {
                        Message::DispatchAction(AppAction::SaveHybridProfile {
                            profile: default_hybrid_profile(
                                &local_for_remote,
                                local_model_for_remote.clone(),
                                &remote_for_remote,
                                option.id,
                                existing_for_remote.as_ref(),
                            ),
                        })
                    })
                    .width(Length::Fill),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into(),
            );
            children.push(action_btn(
                if existing.is_some() {
                    "Update profile"
                } else {
                    "Create profile"
                },
                Message::DispatchAction(AppAction::SaveHybridProfile { profile }),
                true,
                vc,
            ));
        }
        (None, _) => {
            children.push(
                text("No local-capable backend is available in this build. Android on-device models and keyless local servers appear here when installed and verified.")
                    .size(11)
                    .color(vc.muted)
                    .into(),
            );
        }
        (_, None) => {
            children.push(
                text("Enable a healthy confidential provider with at least one model to pair with local routing.")
                    .size(11)
                    .color(vc.muted)
                    .into(),
            );
        }
    }

    let active_profile_id = state
        .active_backend_id
        .as_deref()
        .and_then(|id| id.strip_prefix("hybrid:"));
    for profile in &state.hybrid_profiles {
        children.push(hybrid_profile_controls(
            profile,
            active_profile_id == Some(profile.id.as_str()),
            vc,
        ));
    }

    container(column(children).spacing(10))
        .padding(Padding::from([10u16, 16]))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        })
        .into()
}

pub fn view<'a>(
    state: &'a AppState,
    is_dark: bool,
    show_advanced: bool,
    attestation_interval_input: &'a str,
    brave_api_key_input: &'a str,
    brave_api_key_message: Option<&'a str>,
    theme_override: crate::ThemeOverride,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    // ── Header ────────────────────────────────────────────────────────────────
    let header = container(
        row![
            button(text("Back").size(13).color(vc.text_dim))
                .on_press(Message::DispatchAction(AppAction::PopScreen))
                .padding(Padding::from([4u16, 10]))
                .style(move |_, _| button::Style {
                    background: Some(Background::Color(vc.ghost_overlay)),
                    border: Border {
                        radius: 5.0.into(),
                        color: vc.border,
                        width: 1.0
                    },
                    ..Default::default()
                }),
            text("Settings").size(17).color(vc.text),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.surface)),
        border: Border {
            color: vc.border,
            width: 0.0,
            ..Default::default()
        },
        shadow: Shadow {
            color: vc.shadow,
            blur_radius: 8.0,
            offset: Vector::new(0.0, 2.0),
        },
        ..Default::default()
    });

    // ── Providers summary row ─────────────────────────────────────────────────
    let enabled_count = state
        .backends
        .iter()
        .filter(|b| b.has_api_key || b.id == "qvac-local")
        .count();
    let providers_summary = container(
        button(
            row![
                text("Providers").size(14).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                text(format!("{} enabled", enabled_count))
                    .size(12)
                    .color(vc.muted),
                text(">").size(12).color(vc.muted),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .on_press(Message::DispatchAction(AppAction::PushScreen {
            screen: Screen::SettingsProviders,
        }))
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .padding(Padding::from([0u16, 16]));

    // ── Defaults summary row ──────────────────────────────────────────────────
    let active_model = state
        .backends
        .iter()
        .find(|b| b.is_active)
        .and_then(|b| b.models.first())
        .map(|m| m.as_str())
        .unwrap_or("None");
    let defaults_summary = container(
        button(
            row![
                text("Defaults").size(14).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                text(active_model).size(12).color(vc.muted),
                text(">").size(12).color(vc.muted),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .on_press(Message::DispatchAction(AppAction::PushScreen {
            screen: Screen::SettingsDefaults,
        }))
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .padding(Padding::from([0u16, 16]));

    let hybrid_routing = container(hybrid_routing_card(state, vc))
        .padding(Padding::from([0u16, 16]))
        .width(Length::Fill);

    // ── Directory Sources summary row (Phase 32 Plan 07 entry point) ─────────
    let dir_count = state.directory_sources.len();
    let directory_sources_summary = container(
        button(
            row![
                text("Directory Sources").size(14).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                text(if dir_count == 1 {
                    "1 folder".to_string()
                } else {
                    format!("{} folders", dir_count)
                })
                .size(12)
                .color(vc.muted),
                text(">").size(12).color(vc.muted),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .on_press(Message::DispatchAction(AppAction::PushScreen {
            screen: Screen::DirectorySources,
        }))
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .padding(Padding::from([0u16, 16]));

    // ── Appearance (theme override) ────────────────────────────────────────────
    let appearance_picker = pick_list(
        crate::ThemeOverride::ALL,
        Some(theme_override),
        Message::SettingsThemeOverrideChanged,
    )
    .text_size(13)
    .padding(Padding::from([7u16, 10]));

    let appearance_row = container(
        row![
            text("Theme").size(13).color(vc.text),
            iced::widget::Space::new().width(Length::Fill),
            appearance_picker,
        ]
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([4u16, 16]))
    .width(Length::Fill);

    // ── Advanced Settings (toggle) ────────────────────────────────────────────
    let adv_toggle_lbl = if show_advanced {
        "Advanced Settings  ▲"
    } else {
        "Advanced Settings  ▼"
    };
    let adv_toggle = button(text(adv_toggle_lbl).size(13).color(vc.accent))
        .on_press(Message::SettingsToggleAdvanced)
        .padding(Padding::from([6u16, 0]))
        .style(|_, _| button::Style {
            background: None,
            ..Default::default()
        });

    let adv_toggle_row = container(
        column![
            adv_toggle,
            text("Custom providers, re-attestation interval, and other developer settings.")
                .size(11)
                .color(vc.muted),
        ]
        .spacing(3),
    )
    .padding(Padding {
        top: 8.0,
        bottom: 4.0,
        left: 16.0,
        right: 16.0,
    });

    let advanced_body: Element<'_, Message> = if show_advanced {
        // ── Re-attestation interval ──────────────────────────────────────────
        let interval_display = if attestation_interval_input.is_empty() {
            state.attestation_interval_minutes.to_string()
        } else {
            attestation_interval_input.to_string()
        };

        let interval_input = text_input("0 = disabled", &interval_display)
            .on_input(Message::SettingsAttestationIntervalChanged)
            .size(13)
            .padding(Padding::from([7u16, 10]));

        let apply_btn = button(text("Apply").size(12).color(vc.accent))
            .on_press(Message::SettingsApplyAttestationInterval)
            .padding(Padding::from([7u16, 14]))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(Color {
                    r: vc.accent.r,
                    g: vc.accent.g,
                    b: vc.accent.b,
                    a: 0.12,
                })),
                border: Border {
                    radius: 6.0.into(),
                    color: vc.accent_dim,
                    width: 1.0,
                },
                ..Default::default()
            });

        let interval_block = container(
            column![
                text("Re-attestation Interval").size(13).color(vc.text_dim),
                text("How often the active provider is re-attested automatically (minutes). Set 0 to disable.")
                    .size(11).color(vc.muted),
                row![interval_input, apply_btn].spacing(8).align_y(Alignment::Center),
            ].spacing(6),
        )
        .padding(Padding::from([12u16, 14]))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(vc.card)),
            border: Border { radius: 8.0.into(), color: vc.border, width: 1.0 },
            ..Default::default()
        });

        container(column![interval_block].spacing(6))
            .padding(Padding::from([0u16, 16]))
            .into()
    } else {
        iced::widget::Space::new().height(0).into()
    };

    // ── Memory Section ────────────────────────────────────────────────────────
    let memory_toggle = container(
        row![
            text("Auto-extract Memories").size(14).color(vc.text),
            iced::widget::Space::new().width(Length::Fill),
            toggler(state.memories_enabled)
                .on_toggle(Message::SettingsMemoriesEnabledToggled)
                .size(20),
        ]
        .align_y(Alignment::Center)
        .spacing(8),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.card)),
        border: Border {
            radius: 8.0.into(),
            color: vc.border,
            width: 1.0,
        },
        ..Default::default()
    });

    let memory_toggle_row = container(memory_toggle)
        .padding(Padding::from([0u16, 16]))
        .width(Length::Fill);

    let memory_count_el: Element<'_, Message> = if state.memory_count > 0 {
        text(format!("{}", state.memory_count))
            .size(12)
            .color(vc.muted)
            .into()
    } else {
        iced::widget::Space::new().width(0).into()
    };

    let memory_row = container(
        button(
            row![
                text("Memories").size(14).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                memory_count_el,
                text(">").size(12).color(vc.muted),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .on_press(Message::DispatchAction(AppAction::PushScreen {
            screen: Screen::Memories,
        }))
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .padding(Padding::from([0u16, 16]));

    // ── Tools Section ─────────────────────────────────────────────────────────
    let brave_placeholder = if state.brave_api_key_set {
        "Key configured — enter new key to update"
    } else {
        "Enter Brave Search API Key"
    };

    let configured_label: Element<'_, Message> = if state.brave_api_key_validating {
        text("Verifying…").size(11).color(vc.muted).into()
    } else if state.brave_api_key_set {
        text("Configured ✓")
            .size(11)
            .color(Color {
                r: 0.3,
                g: 0.75,
                b: 0.4,
                a: 1.0,
            })
            .into()
    } else {
        iced::widget::Space::new().width(0).into()
    };

    let brave_key_field = if state.brave_api_key_validating {
        text_input(brave_placeholder, brave_api_key_input)
            .secure(true)
            .size(14)
            .padding(Padding::from([7u16, 10]))
    } else {
        text_input(brave_placeholder, brave_api_key_input)
            .secure(true)
            .on_input(Message::SettingsBraveApiKeyChanged)
            .size(14)
            .padding(Padding::from([7u16, 10]))
    };

    let save_btn_label = if state.brave_api_key_validating {
        "Verifying…"
    } else {
        "Save API Key"
    };
    let brave_save_btn = action_btn(
        save_btn_label,
        Message::SettingsSaveBraveApiKey,
        !brave_api_key_input.trim().is_empty() && !state.brave_api_key_validating,
        vc,
    );

    let feedback_el: Element<'_, Message> = if let Some(msg) = brave_api_key_message {
        let color = if msg.contains("saved") {
            Color {
                r: 0.3,
                g: 0.75,
                b: 0.4,
                a: 1.0,
            }
        } else {
            Color {
                r: 0.85,
                g: 0.25,
                b: 0.25,
                a: 1.0,
            }
        };
        text(msg).size(11).color(color).into()
    } else {
        iced::widget::Space::new().height(0).into()
    };

    let tools_content = container(
        column![
            row![
                text("Web Search").size(14).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                configured_label,
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            text(
                "Required for web search. Keys are stored locally and never sent to third parties."
            )
            .size(11)
            .color(vc.muted),
            brave_key_field,
            feedback_el,
            brave_save_btn,
        ]
        .spacing(8),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.card)),
        border: Border {
            radius: 8.0.into(),
            color: vc.border,
            width: 1.0,
        },
        ..Default::default()
    });

    let tools_body = container(tools_content)
        .padding(Padding::from([0u16, 16]))
        .width(Length::Fill);

    // ── Phase 35 Row A — Discover tools summary row ──────────────────────────
    // Reuses the providers_summary closure style (lines 135-164):
    // background = vc.card, border = vc.border 1px, radius 8.0.
    let enabled_tool_count = state.contextvm_tools.iter().filter(|t| t.enabled).count();
    let discover_subtitle = match enabled_tool_count {
        0 => "No tools enabled".to_string(),
        1 => "1 tool enabled".to_string(),
        n => format!("{} tools enabled", n),
    };
    let discover_tools_summary = container(
        button(
            row![
                text("Discover tools").size(14).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                text(discover_subtitle).size(12).color(vc.muted),
                text(">").size(12).color(vc.muted),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .on_press(Message::ContextvmDiscoverToolsClicked)
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        }),
    )
    .width(Length::Fill)
    .padding(Padding::from([0u16, 16]));

    // ── Phase 35 Row B — Auto-discover toggle row ────────────────────────────
    // Reuses the memory_toggle closure style (lines 344-366):
    // background = vc.card, border = vc.border 1px, radius 8.0,
    // padding = [10, 16].
    let auto_discover_toggle = container(
        column![
            row![
                text("Automatically discover and use tools")
                    .size(14)
                    .color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                toggler(state.auto_discover_tools_enabled)
                    .on_toggle(Message::SettingsAutoDiscoverToolsToggled)
                    .size(20),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            text("Find new tools each conversation and offer them to the assistant automatically.")
                .size(11)
                .color(vc.muted),
        ]
        .spacing(4),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.card)),
        border: Border {
            radius: 8.0.into(),
            color: vc.border,
            width: 1.0,
        },
        ..Default::default()
    });

    let auto_discover_row = container(auto_discover_toggle)
        .padding(Padding::from([0u16, 16]))
        .width(Length::Fill);

    // ── Security Section (Lock Timeout) ──────────────────────────────────────
    let current_label = lock_timeout_label(state.lock_timeout_seconds);

    let lock_timeout_picker = pick_list(
        LOCK_TIMEOUT_OPTIONS
            .iter()
            .map(|(label, _)| *label)
            .collect::<Vec<_>>(),
        Some(current_label),
        |selected: &str| {
            let seconds = LOCK_TIMEOUT_OPTIONS
                .iter()
                .find(|(label, _)| *label == selected)
                .map(|(_, s)| *s)
                .unwrap_or(300);
            Message::DispatchAction(AppAction::SetLockTimeout { seconds })
        },
    )
    .text_size(13)
    .padding(Padding::from([7u16, 10]));

    let never_warning: Element<'_, Message> = if state.lock_timeout_seconds == -1 {
        text("Auto-lock disabled. The app will open without your PIN — it is protected only by your device unlock. If your device is unlocked, anyone with access can open the app.")
            .size(11)
            .color(vc.muted)
            .into()
    } else {
        iced::widget::Space::new().height(0).into()
    };

    let security_body = container(
        column![
            row![
                text("Lock Timeout").size(13).color(vc.text),
                iced::widget::Space::new().width(Length::Fill),
                lock_timeout_picker,
            ]
            .align_y(Alignment::Center),
            text("How long the app can be in the background before it locks.")
                .size(11)
                .color(vc.muted),
            never_warning,
        ]
        .spacing(6),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.card)),
        border: Border {
            radius: 8.0.into(),
            color: vc.border,
            width: 1.0,
        },
        ..Default::default()
    });

    let security_row = container(security_body)
        .padding(Padding::from([0u16, 16]))
        .width(Length::Fill);

    // ── Compose ───────────────────────────────────────────────────────────────
    let content = column![
        section_header("PROVIDERS", vc.muted),
        providers_summary,
        section_header("DEFAULTS", vc.muted),
        defaults_summary,
        section_header("HYBRID ROUTING", vc.muted),
        hybrid_routing,
        section_header("DIRECTORY SOURCES", vc.muted),
        directory_sources_summary,
        section_header("MEMORY", vc.muted),
        memory_toggle_row,
        memory_row,
        section_header("SECURITY", vc.muted),
        security_row,
        section_header("TOOLS", vc.muted),
        tools_body,
        iced::widget::Space::new().height(8),
        discover_tools_summary,
        iced::widget::Space::new().height(8),
        auto_discover_row,
        section_header("APPEARANCE", vc.muted),
        appearance_row,
        divider(),
        adv_toggle_row,
        advanced_body,
        iced::widget::Space::new().height(24),
    ]
    .spacing(0);

    let page = column![
        header,
        scrollable(content).height(Length::Fill).width(Length::Fill),
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill);

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(vc.bg)),
            ..Default::default()
        })
        .into()
}
