use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_input, toggler,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use mango_core::{AppAction, AppState, Screen};

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
    let enabled_count = state.backends.iter().filter(|b| b.has_api_key).count();
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
            text("Required for agent web search. Keys are stored locally and never sent to third parties.")
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
        text("Not recommended. App will only lock on restart.")
            .size(11)
            .color(Color {
                r: 0.9,
                g: 0.4,
                b: 0.1,
                a: 1.0,
            })
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
        section_header("MEMORY", vc.muted),
        memory_toggle_row,
        memory_row,
        section_header("SECURITY", vc.muted),
        security_row,
        section_header("TOOLS", vc.muted),
        tools_body,
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
