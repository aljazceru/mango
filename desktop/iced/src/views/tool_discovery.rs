//! Phase 35 — Tool Discovery sub-screen for the Desktop (iced) UI.
//! Phase 36 extensions: cache-first render, always-visible search input,
//! `Used N×` muted badge, trailing chevron, whole-row tap-for-detail.
//!
//! Renders the five UI-SPEC states (Idle/Loading, Empty, Error, Loaded,
//! Loaded-with-tools) for Nostr-based tool discovery. Mirrors the Android
//! equivalent shipped in 35-06 / 36-02.

use iced::widget::{button, column, container, row, scrollable, text, text_input, toggler};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, ContextvmDiscoveryState, DiscoverableTool, Screen};

use crate::theme::ViewColors;
use crate::Message;

/// Tool Discovery screen entry point.
pub fn view<'a>(
    state: &'a AppState,
    search_query: &'a str,
    is_dark: bool,
) -> Element<'a, Message> {
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

    // ── Always-visible search input (UI-SPEC §L) ─────────────────────────────
    // Locked placeholder copy: `Search tools`.
    let search_input = text_input("Search tools", search_query)
        .on_input(Message::ContextvmSearchChanged)
        .size(14)
        .padding(Padding::from([7u16, 10]));

    let search_block = container(search_input)
        .padding(Padding {
            top: 8.0,
            bottom: 8.0,
            left: 16.0,
            right: 16.0,
        })
        .width(Length::Fill);

    // Apply live filter (case-insensitive substring across name +
    // description + provider_display_name).
    let q = search_query.trim().to_lowercase();
    let filtered: Vec<&DiscoverableTool> = state
        .contextvm_tools
        .iter()
        .filter(|t| {
            if q.is_empty() {
                return true;
            }
            t.name.to_lowercase().contains(&q)
                || t.description.to_lowercase().contains(&q)
                || t.provider_display_name
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
        })
        .collect();

    // ── Cache-first state-dependent body ─────────────────────────────────────
    // During Loading the cached list stays visible if non-empty (UI-SPEC §C
    // refinement). The spinner only shows when there is genuinely nothing.
    let body: Element<'_, Message> = match &state.contextvm_discovery_state {
        ContextvmDiscoveryState::Idle | ContextvmDiscoveryState::Loading => {
            if state.contextvm_tools.is_empty() {
                loading_view(vc)
            } else {
                tool_list_or_empty_search(&filtered, search_query, vc)
            }
        }
        ContextvmDiscoveryState::Error { .. } => error_view(vc),
        ContextvmDiscoveryState::Loaded => {
            if state.contextvm_tools.is_empty() {
                empty_view(vc)
            } else {
                tool_list_or_empty_search(&filtered, search_query, vc)
            }
        }
    };

    let bg_color = vc.bg;
    let page = column![header, search_block, body]
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

/// Render either the filtered tool list, or, when the search query yielded
/// no matches, the empty-search caption (UI-SPEC §M).
fn tool_list_or_empty_search<'a>(
    tools: &[&'a DiscoverableTool],
    query: &'a str,
    vc: ViewColors,
) -> Element<'a, Message> {
    if tools.is_empty() && !query.trim().is_empty() {
        // Locked copy per UI-SPEC §M — straight ASCII quotes.
        container(
            text(format!("No tools match \"{}\"", query))
                .size(14)
                .color(vc.muted),
        )
        .padding(32)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
    } else {
        let rows: Vec<Element<'a, Message>> =
            tools.iter().map(|t| tool_row(t, vc)).collect();

        let list = column(rows)
            .spacing(8)
            .padding(Padding::from([8u16, 16]));

        scrollable(list)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }
}

/// One list row. Phase 36 additions:
/// - `Used N×` muted pill when `usage_count > 0` (UI-SPEC §J).
/// - Trailing `>` chevron between badge slot and toggler (UI-SPEC §K).
/// - Whole-row tap → PushScreen { ContextvmToolDetail { tool_id } }.
///
/// **iced 0.13 click absorption note (W-08 mitigation):** rather than
/// wrapping the whole row in a `button`, we render the `toggler` OUTSIDE
/// the wrapping click target. The left-hand text column is wrapped in a
/// transparent `button` whose `on_press` dispatches PushScreen; the
/// toggler then sits in its own column position to the right and absorbs
/// its own clicks unchanged. This is the split-row pattern from
/// `views/settings.rs:143` and avoids the iced 0.13 gotcha where a
/// `toggler` inside a `button` either consumes both events or neither.
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

    let id_for_toggle = tool.id.clone();
    let id_for_push = tool.id.clone();
    let enabled = tool.enabled;

    // Body (inside the clickable area): name, provider, description.
    let body_col = column![
        text(tool.name.clone()).size(14).color(vc.text),
        text(provider_label).size(12).color(vc.muted),
        text(tool.description.clone()).size(12).color(vc.muted),
    ]
    .spacing(2)
    .width(Length::Fill);

    // Transparent click target — pressing anywhere in this region dispatches
    // PushScreen → ContextvmToolDetail. iced renders `button` styles on
    // hover; we keep it visually flat by using the existing card background.
    let body_btn = button(body_col)
        .on_press(Message::DispatchAction(AppAction::PushScreen {
            screen: Screen::ContextvmToolDetail {
                tool_id: id_for_push,
            },
        }))
        .style(move |_, _| button::Style {
            background: None,
            border: Border {
                radius: 0.0.into(),
                ..Default::default()
            },
            text_color: vc.text,
            ..Default::default()
        })
        .padding(0)
        .width(Length::Fill);

    // Used N× badge (only when usage_count > 0). UI-SPEC §J: muted pill
    // matching the Phase 35 `Remote` provenance label styling.
    let used_badge: Option<Element<'a, Message>> = if tool.usage_count > 0 {
        Some(used_badge_pill(tool.usage_count, vc))
    } else {
        None
    };

    // Trailing chevron — always present (UI-SPEC §K). Conveys drillable.
    let chevron = text(">").size(12).color(vc.muted);

    // Toggler — outside the click target. Retains its own absorption.
    let tog = toggler(enabled)
        .on_toggle(move |new_enabled| Message::ContextvmToolToggled {
            tool_id: id_for_toggle.clone(),
            enabled: new_enabled,
        })
        .size(20);

    // Compose the row — body fills, then optional badge, chevron, toggler.
    let mut trailing = row![]
        .spacing(8)
        .align_y(Alignment::Center);
    if let Some(b) = used_badge {
        trailing = trailing.push(b);
    }
    trailing = trailing.push(chevron).push(tog);

    container(
        row![body_btn, trailing]
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

/// `Used 1×` (singular) / `Used {N}×` (plural) muted pill — matches the
/// Phase 35 `Remote` provenance badge style at `views/agents.rs:170-185`.
/// The `×` glyph is U+00D7 (multiplication sign).
fn used_badge_pill<'a>(n: u32, vc: ViewColors) -> Element<'a, Message> {
    let label = if n == 1 {
        "Used 1×".to_string()
    } else {
        format!("Used {}×", n)
    };
    let muted = vc.muted;
    container(text(label).size(11).color(muted))
        .padding(Padding::from([2u16, 6]))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color {
                r: muted.r,
                g: muted.g,
                b: muted.b,
                a: 0.15,
            })),
            border: Border {
                color: muted,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}
