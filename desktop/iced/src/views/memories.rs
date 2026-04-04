use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, MemorySummary};

use crate::Message;

fn format_date(unix_timestamp: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = now - unix_timestamp;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        let days = diff / 86400;
        if days == 1 {
            "yesterday".to_string()
        } else {
            format!("{}d ago", days)
        }
    }
}

/// Memories screen: view, edit, and delete stored memories (MEM-04, MEM-05, MEM-06).
/// Full-screen overlay following the same pattern as Settings and Documents.
pub fn view<'a>(
    state: &'a AppState,
    memory_edit_state: &'a Option<(String, String)>,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    // ── Header ───────────────────────────────────────────────────────────────
    let surface_color = vc.surface;
    let back_btn = button(text("Back").size(14))
        .on_press(Message::DispatchAction(AppAction::PopScreen))
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(surface_color)),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let secondary_surface = vc.secondary_surface;
    let bg_color = vc.bg;
    let header = container(
        row![
            back_btn,
            text("Memories").size(22),
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(secondary_surface)),
        ..Default::default()
    });

    // ── Memory list or empty state ───────────────────────────────────────────
    let muted_color = vc.muted;
    let content_section: Element<'_, Message> = if state.memories.is_empty() {
        container(
            column![
                text("No memories yet.").size(16),
                text("Memories are automatically extracted from your conversations.")
                    .size(14)
                    .color(muted_color),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .padding(48)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .into()
    } else {
        let memory_rows: Vec<Element<'_, Message>> = state
            .memories
            .iter()
            .map(|memory| build_memory_row(memory, memory_edit_state, vc))
            .collect();

        let list = column(memory_rows)
            .spacing(8)
            .padding(Padding::from([8u16, 16]));

        scrollable(list).height(Length::Fill).width(Length::Fill).into()
    };

    // ── Compose full layout ───────────────────────────────────────────────────
    let page = column![header, content_section]
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

fn build_memory_row<'a>(
    memory: &'a MemorySummary,
    memory_edit_state: &'a Option<(String, String)>,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let is_editing = memory_edit_state
        .as_ref()
        .map(|(id, _)| id == &memory.id)
        .unwrap_or(false);

    let secondary_surface = vc.secondary_surface;
    let muted = vc.muted;

    if is_editing {
        let edit_text = memory_edit_state
            .as_ref()
            .map(|(_, t)| t.as_str())
            .unwrap_or("");

        let accent = vc.accent;
        let bg = vc.bg;
        let destructive = vc.destructive;
        let destructive_bg = Color {
            r: destructive.r,
            g: destructive.g,
            b: destructive.b,
            a: 0.15,
        };

        let input = text_input("Memory content", edit_text)
            .on_input(Message::MemoryEditChanged)
            .on_submit(Message::MemorySaveEdit)
            .size(14)
            .padding(Padding::from([6u16, 8]));

        // Save dispatches AppAction::UpdateMemory via MemorySaveEdit handler in update()
        let save_btn = button(text("Save").size(12).color(bg))
            .on_press(Message::MemorySaveEdit)
            .padding(Padding::from([4u16, 10]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let cancel_btn = button(text("Cancel").size(12))
            .on_press(Message::MemoryCancelEdit)
            .padding(Padding::from([4u16, 10]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(destructive_bg)),
                border: Border {
                    radius: 4.0.into(),
                    color: destructive,
                    width: 1.0,
                },
                text_color: destructive,
                ..Default::default()
            });

        let btn_row = row![save_btn, cancel_btn].spacing(8);

        container(
            column![input, btn_row].spacing(8),
        )
        .padding(Padding::from([10u16, 14]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else {
        let memory_id = memory.id.clone();
        let memory_content = memory.content.clone();
        let conv_title = memory
            .conversation_title
            .clone()
            .unwrap_or_else(|| "Unknown conversation".to_string());
        let date_text = format_date(memory.created_at);

        let destructive = vc.destructive;
        let destructive_bg = Color {
            r: destructive.r,
            g: destructive.g,
            b: destructive.b,
            a: 0.15,
        };

        // Delete dispatches AppAction::DeleteMemory via MemoryConfirmDelete handler in update()
        let delete_id = memory_id.clone();
        let delete_btn = button(text("Delete").size(12))
            .on_press(Message::MemoryConfirmDelete(delete_id))
            .padding(Padding::from([4u16, 10]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(destructive_bg)),
                border: Border {
                    radius: 4.0.into(),
                    color: destructive,
                    width: 1.0,
                },
                text_color: destructive,
                ..Default::default()
            });

        let memory_info = column![
            text(&memory.content_preview).size(14),
            text(conv_title).size(12).color(muted),
            text(date_text).size(11).color(muted),
        ]
        .spacing(3);

        let edit_id = memory_id.clone();
        let row_content = row![
            button(container(memory_info).width(Length::Fill))
                .on_press(Message::MemoryStartEdit(edit_id, memory_content))
                .padding(0)
                .style(|_theme, _status| button::Style {
                    background: None,
                    ..Default::default()
                }),
            delete_btn,
        ]
        .align_y(Alignment::Center)
        .spacing(12);

        container(row_content)
            .padding(Padding::from([10u16, 14]))
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(secondary_surface)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}
