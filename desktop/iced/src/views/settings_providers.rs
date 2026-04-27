use std::collections::HashMap;

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use mango_core::{
    known_provider_presets, AppAction, AppState, AttestationStatus, HealthStatus,
    OrchestratedComponent, TeeType,
};

use crate::Message;

const TEE_OPTIONS: &[&str] = &["IntelTdx", "NvidiaH100Cc", "AmdSevSnp", "Unknown"];

fn health_color(s: &HealthStatus, vc: crate::theme::ViewColors) -> Color {
    match s {
        HealthStatus::Healthy => vc.success,
        HealthStatus::Degraded => vc.warning,
        HealthStatus::Failed => vc.destructive,
        HealthStatus::Unknown => vc.muted,
    }
}

fn health_label(s: &HealthStatus) -> &'static str {
    match s {
        HealthStatus::Healthy => "Healthy",
        HealthStatus::Degraded => "Degraded",
        HealthStatus::Failed => "Failed",
        HealthStatus::Unknown => "Unknown",
    }
}

fn tee_label(t: &TeeType) -> &'static str {
    match t {
        TeeType::IntelTdx => "Intel TDX",
        TeeType::NvidiaH100Cc => "NVIDIA H100 CC",
        TeeType::AmdSevSnp => "AMD SEV-SNP",
        TeeType::Unknown => "Unknown TEE",
    }
}

fn attest_label(s: &AttestationStatus, vc: crate::theme::ViewColors) -> (&'static str, Color) {
    match s {
        AttestationStatus::Verified { .. } => ("Verified", vc.success),
        AttestationStatus::Unverified => ("Unverified", vc.muted),
        AttestationStatus::Failed { .. } => ("Failed", vc.destructive),
        AttestationStatus::Expired => ("Expired", vc.warning),
    }
}

/// Phase 34.1: Build the orchestrated breakdown sub-line text per UI-SPEC.md (LOCKED copy).
/// Component label map: compose_manager → compose; unknown verbatim.
/// Separator: " • "; success glyph: " ✓".
fn orchestrated_breakdown_line(comps: &[OrchestratedComponent]) -> String {
    comps
        .iter()
        .map(|c| {
            let mapped = match c.label.as_str() {
                "gateway" => "gateway",
                "model" => "model",
                "compose_manager" => "compose",
                other => other,
            };
            format!("{} ✓", mapped)
        })
        .collect::<Vec<_>>()
        .join(" • ")
}

fn pill<'a>(label: &'a str, fg: Color, bg: Color) -> Element<'a, Message> {
    container(text(label).size(11).color(fg))
        .padding(Padding::from([2u16, 7]))
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 20.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn ghost_btn<'a>(
    label: &'a str,
    msg: Message,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    button(text(label).size(12).color(vc.text_dim))
        .on_press(msg)
        .padding(Padding::from([4u16, 10]))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.ghost_overlay)),
            border: Border {
                radius: 5.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        })
        .into()
}

fn action_btn<'a>(
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

fn section_header<'a>(label: &'a str, muted: Color) -> Element<'a, Message> {
    container(text(label).size(11).color(muted))
        .padding(Padding {
            top: 20.0,
            bottom: 6.0,
            left: 16.0,
            right: 16.0,
        })
        .into()
}

pub fn view<'a>(
    state: &'a AppState,
    is_dark: bool,
    add_name: &'a str,
    add_url: &'a str,
    add_key: &'a str,
    add_tee: &'a str,
    preset_keys: &'a HashMap<String, String>,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    // ── Header ────────────────────────────────────────────────────────────────
    let header = container(
        row![
            button(text("< Settings").size(13).color(vc.text_dim))
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
            text("Providers").size(17).color(vc.text),
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

    // ── Providers ─────────────────────────────────────────────────────────────
    let presets = known_provider_presets();
    let enabled_ids: Vec<String> = state
        .backends
        .iter()
        .filter(|b| b.has_api_key)
        .map(|b| b.id.clone())
        .collect();

    struct PresetRow {
        id: String,
        name: String,
        tee_type: TeeType,
    }
    let rows: Vec<PresetRow> = presets
        .iter()
        .map(|p| PresetRow {
            id: p.id.clone(),
            name: p.name.clone(),
            tee_type: p.tee_type.clone(),
        })
        .collect();

    let provider_cards: Vec<Element<'_, Message>> = rows
        .into_iter()
        .map(|p| {
            let is_enabled = enabled_ids.contains(&p.id);
            let name_text = text(p.name.clone()).size(14).color(vc.text);
            let tee_text = text(tee_label(&p.tee_type)).size(11).color(vc.muted);

            if is_enabled {
                let health = state
                    .backends
                    .iter()
                    .find(|b| b.id == p.id)
                    .map(|b| {
                        (
                            health_label(&b.health_status),
                            health_color(&b.health_status, vc),
                        )
                    })
                    .unwrap_or(("Unknown", vc.muted));

                let att_status: Option<&AttestationStatus> = state
                    .attestation_statuses
                    .iter()
                    .find(|a| a.backend_id == p.id)
                    .map(|a| &a.status);
                let (att_str, att_col) = att_status
                    .map(|s| attest_label(s, vc))
                    .unwrap_or(("—", vc.muted));

                let is_active = state
                    .backends
                    .iter()
                    .find(|b| b.id == p.id)
                    .map(|b| b.is_active)
                    .unwrap_or(false);

                let default_el: Element<'_, Message> = if is_active {
                    pill(
                        "Default",
                        vc.accent,
                        Color {
                            r: vc.accent.r,
                            g: vc.accent.g,
                            b: vc.accent.b,
                            a: 0.15,
                        },
                    )
                } else {
                    ghost_btn(
                        "Set Default",
                        Message::DispatchAction(AppAction::SetDefaultBackend {
                            backend_id: p.id.clone(),
                        }),
                        vc,
                    )
                };

                let remove_el = button(text("Remove").size(11).color(vc.destructive))
                    .on_press(Message::DispatchAction(AppAction::RemoveBackend {
                        backend_id: p.id.clone(),
                    }))
                    .padding(Padding::from([4u16, 10]))
                    .style(move |_, _| button::Style {
                        background: Some(Background::Color(Color {
                            r: vc.destructive.r,
                            g: vc.destructive.g,
                            b: vc.destructive.b,
                            a: 0.10,
                        })),
                        border: Border {
                            radius: 5.0.into(),
                            color: Color {
                                r: vc.destructive.r,
                                g: vc.destructive.g,
                                b: vc.destructive.b,
                                a: 0.3,
                            },
                            width: 1.0,
                        },
                        ..Default::default()
                    });

                let name_col = column![name_text, tee_text].spacing(2);

                let status_row = row![
                    text(health.0).size(11).color(health.1),
                    text("·").size(11).color(vc.muted),
                    text(att_str).size(11).color(att_col),
                ]
                .spacing(5)
                .align_y(Alignment::Center);

                let mut right_col = iced::widget::Column::new()
                    .spacing(5)
                    .align_x(iced::alignment::Horizontal::Right)
                    .push(
                        row![
                            pill(
                                "Enabled",
                                vc.success,
                                Color {
                                    r: vc.success.r,
                                    g: vc.success.g,
                                    b: vc.success.b,
                                    a: 0.12
                                }
                            ),
                            default_el,
                            remove_el
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .push(status_row);

                // Phase 34.1: trust-UI sub-lines for Redpill freshness +
                // orchestrated breakdown. Copy LOCKED in 34.1-UI-SPEC.md.
                // Render conditions per UI-SPEC §"Interaction & State Contract".
                if let Some(AttestationStatus::Verified {
                    freshness,
                    orchestrated_components,
                    ..
                }) = att_status
                {
                    if freshness.as_deref() == Some("PerEnclave") {
                        right_col = right_col.push(
                            text("Verified for this enclave instance")
                                .size(11)
                                .color(vc.muted),
                        );
                    }
                    if let Some(comps) = orchestrated_components {
                        if !comps.is_empty() {
                            right_col = right_col.push(
                                text(orchestrated_breakdown_line(comps))
                                    .size(11)
                                    .color(vc.muted),
                            );
                        }
                    }
                }

                let right_col = right_col;

                container(
                    row![
                        name_col,
                        iced::widget::Space::new().width(Length::Fill),
                        right_col,
                    ]
                    .align_y(Alignment::Center)
                    .spacing(8),
                )
                .padding(Padding::from([12u16, 14]))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(vc.card_enabled)),
                    border: Border {
                        radius: 8.0.into(),
                        color: vc.border_enabled,
                        width: 1.0,
                    },
                    ..Default::default()
                })
                .into()
            } else {
                let current_key = preset_keys.get(&p.id).map(|s| s.as_str()).unwrap_or("");
                let can_enable = !current_key.trim().is_empty();

                let key_input = text_input("API Key", current_key)
                    .secure(true)
                    .on_input({
                        let pid = p.id.clone();
                        move |v| Message::SettingsPresetKeyChanged {
                            preset_id: pid.clone(),
                            key: v,
                        }
                    })
                    .size(13)
                    .padding(Padding::from([7u16, 10]));

                let enable_el = action_btn(
                    "Enable",
                    Message::SettingsEnablePreset {
                        preset_id: p.id.clone(),
                    },
                    can_enable,
                    vc,
                );

                let card_content = column![
                    row![
                        column![name_text, tee_text].spacing(2),
                        iced::widget::Space::new().width(Length::Fill),
                    ]
                    .align_y(Alignment::Center),
                    row![key_input, enable_el]
                        .spacing(8)
                        .align_y(Alignment::Center),
                ]
                .spacing(10);

                container(card_content)
                    .padding(Padding::from([12u16, 14]))
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
        })
        .collect();

    let providers_col = column(provider_cards)
        .spacing(6)
        .padding(Padding::from([0u16, 16]));

    // ── Advanced: Custom provider form ────────────────────────────────────────
    let name_input = text_input("Name", add_name)
        .on_input(Message::SettingsAddNameChanged)
        .size(13)
        .padding(Padding::from([7u16, 10]));

    let url_input = text_input("https://inference.example.com/v1", add_url)
        .on_input(Message::SettingsAddUrlChanged)
        .size(13)
        .padding(Padding::from([7u16, 10]));

    let key_input_custom = text_input("API Key", add_key)
        .secure(true)
        .on_input(Message::SettingsAddKeyChanged)
        .size(13)
        .padding(Padding::from([7u16, 10]));

    let tee_opts: Vec<String> = TEE_OPTIONS.iter().map(|s| s.to_string()).collect();
    let sel_tee: Option<String> = tee_opts.iter().find(|o| o.as_str() == add_tee).cloned();
    let tee_picker = iced::widget::pick_list(tee_opts, sel_tee, Message::SettingsAddTeeChanged)
        .placeholder("TEE Type")
        .text_size(13);

    let can_add =
        !add_name.trim().is_empty() && !add_url.trim().is_empty() && !add_key.trim().is_empty();
    let add_tee_type = tee_type_from_str(add_tee);

    let submit_el = action_btn(
        "Add Provider",
        Message::SettingsSubmitAddBackend {
            name: add_name.trim().to_string(),
            url: add_url.trim().to_string(),
            key: add_key.to_string(),
            tee: add_tee_type,
        },
        can_add,
        vc,
    );

    let custom_block = container(
        column![
            text("Custom Provider").size(13).color(vc.text_dim),
            text("For self-hosted or experimental confidential inference endpoints.")
                .size(11)
                .color(vc.muted),
            column![text("Name").size(11).color(vc.muted), name_input].spacing(4),
            column![text("Base URL").size(11).color(vc.muted), url_input].spacing(4),
            column![text("API Key").size(11).color(vc.muted), key_input_custom].spacing(4),
            column![text("TEE Type").size(11).color(vc.muted), tee_picker].spacing(4),
            submit_el,
        ]
        .spacing(8),
    )
    .padding(Padding::from([12u16, 14]))
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

    let custom_body = container(custom_block)
        .padding(Padding::from([0u16, 16]))
        .width(Length::Fill);

    // ── Compose ───────────────────────────────────────────────────────────────
    let content = column![
        section_header("PROVIDERS", vc.muted),
        providers_col,
        section_header("CUSTOM", vc.muted),
        custom_body,
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

fn tee_type_from_str(s: &str) -> TeeType {
    match s {
        "NvidiaH100Cc" => TeeType::NvidiaH100Cc,
        "Unknown" => TeeType::Unknown,
        _ => TeeType::IntelTdx,
    }
}
