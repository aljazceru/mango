//! Phase 36 — Per-tool detail view (Desktop iced).
//!
//! Reached via `PushScreen { Screen::ContextvmToolDetail { tool_id } }`.
//! Reads the tool out of `state.contextvm_tools` by id; renders heading,
//! `ADVERTISED BY`, `USAGE`, `SCHEMA` expander, and `Tool ID:` row.
//!
//! All copy strings are locked verbatim from `36-UI-SPEC.md` §Copywriting
//! (Phase 36 introduces 23 new locked strings — every one of those that is
//! a fixed literal appears in this file).

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding};

use mango_core::{AppAction, AppState};

use crate::theme::ViewColors;
use crate::Message;

/// Locked copy string surfaced on the inline status line when a clipboard
/// write fails (UI-SPEC §O failure variant). v1 does not have an explicit
/// error-propagation path from `iced::clipboard::write`; this constant
/// exists so the failure-path literal is grep-able in the source and is
/// trivially reachable by a future error-routing patch that sets
/// `contextvm_copy_status = Some(COPY_FAILED.to_string())`.
#[allow(dead_code)]
pub(crate) const COPY_FAILED: &str = "Couldn't copy — try again";

/// Tool Detail screen entry point.
pub fn view<'a>(
    state: &'a AppState,
    tool_id: &'a str,
    copy_status: Option<&'a str>,
    schema_expanded: bool,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);
    let surface_color = vc.surface;

    // ── Header bar (back arrow + locked title "Tool details") ────────────────
    let back_btn = button(text("Back").size(14).color(vc.text_dim))
        .on_press(Message::DispatchAction(AppAction::PopScreen))
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

    let header = container(
        row![
            back_btn,
            text("Tool details").size(17).color(vc.text),
            iced::widget::Space::new().width(Length::Fill),
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

    // ── Body — find the tool, render or "Tool not found" fallback ────────────
    let bg_color = vc.bg;
    let tool_opt = state.contextvm_tools.iter().find(|t| t.id == tool_id);

    let body: Element<'_, Message> = match tool_opt {
        None => container(text("Tool not found").size(14).color(vc.muted))
            .padding(32)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .into(),
        Some(tool) => render_tool_body(tool, copy_status, schema_expanded, vc),
    };

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

/// Build the scrollable body of the detail screen.
///
/// Vertical sections (UI-SPEC §Layout):
/// 1. Heading block: tool name (size 20), optional `Used N× — last used …`
///    caption, full description.
/// 2. ADVERTISED BY: provider display name, npub row + Copy, Hex row + Copy.
/// 3. USAGE: `Never used` or `Used {N} time(s)` + `Last used {relative}`.
/// 4. SCHEMA expander: `Show` / `Hide` button + monospace scrollable card,
///    or `No schema published` when empty.
/// 5. Tool ID row + Copy.
/// 6. Inline copy-confirmation status line (when set).
fn render_tool_body<'a>(
    tool: &'a mango_core::DiscoverableTool,
    copy_status: Option<&'a str>,
    schema_expanded: bool,
    vc: ViewColors,
) -> Element<'a, Message> {
    let mut col = column![]
        .spacing(24)
        .padding(Padding::from([16u16, 16]))
        .width(Length::Fill);

    // ── Section 1: Heading block ─────────────────────────────────────────────
    let mut heading = column![text(tool.name.clone()).size(20).color(vc.text)].spacing(4);

    // Caption only when usage_count > 0.
    if tool.usage_count > 0 {
        if let Some(label) = &tool.last_used_label {
            // Locked copy: `Used {N}× — last used {relative}` (em-dash U+2014).
            let caption = if tool.usage_count == 1 {
                format!("Used 1× — last used {}", label)
            } else {
                format!("Used {}× — last used {}", tool.usage_count, label)
            };
            heading = heading.push(text(caption).size(12).color(vc.muted));
        }
    }

    if !tool.description.trim().is_empty() {
        heading = heading.push(text(tool.description.clone()).size(13).color(vc.text));
    }

    col = col.push(heading);

    // ── Section 2: ADVERTISED BY ─────────────────────────────────────────────
    // Phase 37: Use provider_name from Nostr profile if available, fallback to
    // provider_display_name, then "Unnamed provider"
    let provider = tool
        .provider_name
        .as_ref()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            tool.provider_display_name
                .as_ref()
                .filter(|s| !s.is_empty())
        })
        .cloned()
        // Locked fallback copy.
        .unwrap_or_else(|| "Unnamed provider".to_string());

    let advertised = column![
        text("ADVERTISED BY").size(11).color(vc.muted),
        text(provider).size(14).color(vc.text),
        copy_row(
            tool.npub.clone(),
            tool.npub.clone(),
            None,
            Message::CopyNpub(tool.npub.clone()),
            vc,
        ),
        copy_row(
            // Display: first 8 hex + ellipsis. Copy: FULL hex.
            short_hex(&tool.provider_pubkey),
            tool.provider_pubkey.clone(),
            Some("Hex:"),
            Message::CopyHex(tool.provider_pubkey.clone()),
            vc,
        ),
    ]
    .spacing(4);
    col = col.push(advertised);

    // ── Section 3: USAGE ─────────────────────────────────────────────────────
    let mut usage = column![text("USAGE").size(11).color(vc.muted)].spacing(4);
    if tool.usage_count == 0 {
        // Locked copy.
        usage = usage.push(text("Never used").size(13).color(vc.muted));
    } else {
        let line1 = if tool.usage_count == 1 {
            // Locked copy (singular).
            "Used 1 time".to_string()
        } else {
            // Locked copy (plural).
            format!("Used {} times", tool.usage_count)
        };
        usage = usage.push(text(line1).size(13).color(vc.text));
        if let Some(label) = &tool.last_used_label {
            // Locked copy: `Last used {relative}`.
            usage = usage.push(
                text(format!("Last used {}", label))
                    .size(13)
                    .color(vc.muted),
            );
        }
    }
    col = col.push(usage);

    // ── Section 4: SCHEMA expander ───────────────────────────────────────────
    let has_schema = !tool.schema_pretty.trim().is_empty();
    let schema_section: Element<'a, Message> = if !has_schema {
        // Locked copy when schema is absent.
        column![
            text("SCHEMA").size(11).color(vc.muted),
            text("No schema published").size(13).color(vc.muted),
        ]
        .spacing(4)
        .into()
    } else {
        // Locked copy: `Show` (collapsed) / `Hide` (expanded). The plain
        // labels are wrapped in iced glyphs (▼/▲) for affordance only —
        // the locked words remain present and grep-able.
        let toggle_label = if schema_expanded {
            "▲ Hide".to_string()
        } else {
            "▼ Show".to_string()
        };
        let toggle_btn = button(text(toggle_label).size(11).color(vc.text_dim))
            .on_press(Message::ToggleSchemaExpanded)
            .padding(Padding::from([4u16, 8]))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(vc.ghost_overlay)),
                border: Border {
                    radius: 4.0.into(),
                    color: vc.border,
                    width: 1.0,
                },
                ..Default::default()
            });

        let schema_header = row![
            text("SCHEMA").size(11).color(vc.muted),
            iced::widget::Space::new().width(Length::Fill),
            toggle_btn,
        ]
        .align_y(Alignment::Center);

        let mut s = column![schema_header].spacing(8);
        if schema_expanded {
            // Pretty-printed JSON inside a scrollable card. Plain text only —
            // never rendered as Markdown (UI-SPEC injection-surface note).
            let card_bg = vc.card;
            let card_border = vc.border;
            let body_text = text(tool.schema_pretty.clone())
                .size(12)
                .font(Font::MONOSPACE)
                .color(vc.text);
            let scroll = scrollable(container(body_text).padding(12).width(Length::Fill))
                .height(Length::Fixed(320.0))
                .width(Length::Fill);
            let card = container(scroll).style(move |_theme| container::Style {
                background: Some(Background::Color(card_bg)),
                border: Border {
                    radius: 8.0.into(),
                    color: card_border,
                    width: 1.0,
                },
                ..Default::default()
            });
            s = s.push(card);
        }
        s.into()
    };
    col = col.push(schema_section);

    // ── Section 5: Tool ID row ───────────────────────────────────────────────
    col = col.push(copy_row(
        short_hex(&tool.id),
        tool.id.clone(),
        // Locked copy: `Tool ID:`.
        Some("Tool ID:"),
        Message::CopyToolId(tool.id.clone()),
        vc,
    ));

    // ── Section 6: Inline copy-confirmation status line (UI-SPEC §O) ─────────
    if let Some(status) = copy_status {
        let color = if status.starts_with("Couldn't") {
            vc.destructive
        } else {
            vc.success
        };
        col = col.push(text(status.to_string()).size(11).color(color));
    }

    // Spacer + scrollable so long schemas / descriptions don't overflow.
    col = col.push(Space::new().height(Length::Fixed(24.0)));
    scrollable(col)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

/// First 8 hex chars + ellipsis — visual only. Copy actions emit the full
/// value via the Message payload, never this truncation.
fn short_hex(s: &str) -> String {
    let prefix: String = s.chars().take(8).collect();
    format!("{}…", prefix)
}

/// One copy-able row: optional prefix label, monospace display value,
/// trailing `Copy` button. Locked copy: `Copy`. Tap on the button dispatches
/// `on_copy` which the parent already configured to carry the FULL value.
fn copy_row<'a>(
    display: String,
    _full_value: String,
    prefix: Option<&'a str>,
    on_copy: Message,
    vc: ViewColors,
) -> Element<'a, Message> {
    let mut r = row![]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    if let Some(p) = prefix {
        r = r.push(text(p.to_string()).size(13).color(vc.muted));
    }
    r = r
        .push(text(display).size(12).font(Font::MONOSPACE).color(vc.text))
        .push(iced::widget::Space::new().width(Length::Fill));

    let muted = vc.muted;
    let border = vc.border;
    let copy_btn = button(text("Copy").size(11).color(vc.text_dim))
        .on_press(on_copy)
        .padding(Padding::from([4u16, 10]))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(Color {
                r: muted.r,
                g: muted.g,
                b: muted.b,
                a: 0.10,
            })),
            border: Border {
                radius: 4.0.into(),
                color: border,
                width: 1.0,
            },
            ..Default::default()
        });
    r = r.push(copy_btn);
    r.into()
}
