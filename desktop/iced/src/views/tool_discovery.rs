//! Phase 35 — Tool Discovery sub-screen for the Desktop (iced) UI.
//!
//! Renders the five UI-SPEC states (Idle/Loading, Empty, Error, Loaded, Loaded
//! with tools) for Nostr-based tool discovery. Mirrors the Android equivalent
//! shipped in 35-06.

use iced::widget::{button, column, container, row, scrollable, text, toggler};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use mango_core::{AppState, ContextvmDiscoveryState, DiscoverableTool};

use crate::theme::ViewColors;
use crate::Message;

/// Tool Discovery screen entry point.
pub fn view<'a>(state: &'a AppState, is_dark: bool) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    // ── Header ───────────────────────────────────────────────────────────────
    let surface_color = vc.surface;

    let back_btn = button(text("Back").size(14).color(vc.text_dim))
        .on_press(Message::DispatchAction(mango_core::AppAction::PopScreen))
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(vc.ghost_overlay)),
            border: Border {
                radius: 5.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        });

    let refresh_disabled = matches!(
        state.contextvm_discovery_state,
        ContextvmDiscoveryState::Loading
    );
    let refresh_btn = if refresh_disabled {
        button(text("Refresh").size(13).color(vc.muted))
            .padding(Padding::from([4u16, 10]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(vc.ghost_overlay)),
                border: Border {
                    radius: 5.0.into(),
                    color: vc.border,
                    width: 1.0,
                },
                ..Default::default()
            })
    } else {
        button(text("Refresh").size(13).color(vc.text_dim))
            .on_press(Message::ContextvmRetryClicked)
            .padding(Padding::from([4u16, 10]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(vc.ghost_overlay)),
                border: Border {
                    radius: 5.0.into(),
                    color: vc.border,
                    width: 1.0,
                },
                ..Default::default()
            })
    };

    let header = container(
        row![
            back_btn,
            text("Discover Tools").size(17).color(vc.text),
            iced::widget::Space::new().width(Length::Fill),
            refresh_btn,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(surface_color)),
        ..Default::default()
    });

    // ── State-dependent body ─────────────────────────────────────────────────
    let body: Element<'_, Message> = match &state.contextvm_discovery_state {
        ContextvmDiscoveryState::Idle | ContextvmDiscoveryState::Loading => {
            loading_view(vc)
        }
        ContextvmDiscoveryState::Error { .. } => error_view(vc),
        ContextvmDiscoveryState::Loaded => {
            if state.contextvm_tools.is_empty() {
                empty_view(vc)
            } else {
                tool_list(&state.contextvm_tools, vc)
            }
        }
    };

    let bg_color = vc.bg;
    let page = column![header, body]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..Default::default()
        })
        .into()
}

// ── Centred state panes (mirror memories.rs:74-89) ───────────────────────────

fn loading_view(vc: ViewColors) -> Element<'static, Message> {
    container(
        text("Searching Nostr relays…")
            .size(14)
            .color(vc.muted),
    )
    .padding(48)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn empty_view(vc: ViewColors) -> Element<'static, Message> {
    container(
        column![
            text("No tools found").size(16).color(vc.text),
            text("Tools advertised on Nostr will appear here.")
                .size(14)
                .color(vc.muted),
            try_again_button(vc),
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .padding(48)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn error_view(vc: ViewColors) -> Element<'static, Message> {
    container(
        column![
            text("Couldn’t reach relays").size(16).color(vc.destructive),
            text("Check your connection and try again.")
                .size(14)
                .color(vc.muted),
            try_again_button(vc),
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .padding(48)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

/// Shared "Try again" button for the empty + error states.
fn try_again_button(vc: ViewColors) -> Element<'static, Message> {
    let accent = vc.accent;
    let bg_color = vc.bg;
    button(text("Try again").size(13).color(bg_color))
        .on_press(Message::ContextvmRetryClicked)
        .padding(Padding::from([6u16, 16]))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

// ── Loaded list (state F) ────────────────────────────────────────────────────

fn tool_list<'a>(
    tools: &'a [DiscoverableTool],
    vc: ViewColors,
) -> Element<'a, Message> {
    let rows: Vec<Element<'a, Message>> = tools.iter().map(|t| tool_row(t, vc)).collect();

    let list = column(rows)
        .spacing(8)
        .padding(Padding::from([8u16, 16]));

    scrollable(list)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn tool_row<'a>(tool: &'a DiscoverableTool, vc: ViewColors) -> Element<'a, Message> {
    // Provider label: display name → fallback to first 8 hex chars + ellipsis
    // (UI-SPEC §F).
    let provider_label = match &tool.provider_display_name {
        Some(name) if !name.is_empty() => name.clone(),
        _ => {
            let prefix: String = tool.provider_pubkey.chars().take(8).collect();
            format!("{}…", prefix)
        }
    };

    let id = tool.id.clone();
    let enabled = tool.enabled;

    container(
        row![
            column![
                text(tool.name.clone()).size(14).color(vc.text),
                text(provider_label).size(12).color(vc.muted),
                text(tool.description.clone()).size(12).color(vc.muted),
            ]
            .spacing(2)
            .width(Length::Fill),
            toggler(enabled)
                .on_toggle(move |new_enabled| Message::ContextvmToolToggled {
                    tool_id: id.clone(),
                    enabled: new_enabled,
                })
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
    })
    .into()
}
