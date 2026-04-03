use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, DocumentSummary};

use crate::Message;

fn format_size(size_bytes: u64) -> String {
    if size_bytes < 1024 {
        format!("{} B", size_bytes)
    } else if size_bytes < 1024 * 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_date(unix_timestamp: i64) -> String {
    // Simple date formatting without chrono dep in the shell
    // unix_timestamp is seconds since epoch
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

fn format_badge(format: &str) -> &'static str {
    match format {
        "pdf" => "PDF",
        "md" => "MD",
        _ => "TXT",
    }
}

/// Documents screen: document library management (LRAG-06, D-09, D-10).
/// Full-screen overlay following the same pattern as Settings.
pub fn view(state: &AppState, is_dark: bool) -> Element<'_, Message> {
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

    let accent_color = vc.accent;
    let bg_color = vc.bg;
    let add_btn = button(text("Add Document").size(14).color(bg_color))
        .on_press(Message::PickDocumentFile)
        .padding(Padding::from([4u16, 12]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(accent_color)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let secondary_surface = vc.secondary_surface;
    let header = container(
        row![
            back_btn,
            text("Document Library").size(22),
            iced::widget::Space::new().width(Length::Fill),
            add_btn,
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

    // ── Ingestion progress indicator ─────────────────────────────────────────
    let muted_color = vc.muted;
    let progress_section: Option<Element<'_, Message>> =
        state.ingestion_progress.as_ref().map(|progress| {
            container(
                row![
                    text("Ingesting:").size(13).color(muted_color),
                    text(format!(
                        "{} — {}...",
                        progress.document_name, progress.stage
                    ))
                    .size(13)
                    .color(accent_color),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(Padding::from([8u16, 16]))
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(Color { r: accent_color.r, g: accent_color.g, b: accent_color.b, a: 0.1 })),
                border: Border {
                    color: accent_color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into()
        });

    // ── Document list ────────────────────────────────────────────────────────
    let content_section: Element<'_, Message> = if state.documents.is_empty()
        && state.ingestion_progress.is_none()
    {
        // Empty state
        container(
            column![
                text("No documents ingested yet.").size(16),
                text("Add a document to get started.")
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
        let doc_rows: Vec<Element<'_, Message>> = state
            .documents
            .iter()
            .map(|doc| build_document_row(doc, vc))
            .collect();

        let list = column(doc_rows)
            .spacing(8)
            .padding(Padding::from([8u16, 16]));

        scrollable(list).height(Length::Fill).width(Length::Fill).into()
    };

    // ── Compose full layout ───────────────────────────────────────────────────
    let mut page_children: Vec<Element<'_, Message>> = vec![header.into()];

    if let Some(prog) = progress_section {
        page_children.push(prog);
    }

    page_children.push(content_section);

    let page = column(page_children)
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

fn build_document_row<'a>(doc: &'a DocumentSummary, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let badge_text = format_badge(&doc.format);
    let size_text = format_size(doc.size_bytes);
    let date_text = format_date(doc.ingestion_date);
    let chunks_text = format!("{} chunks", doc.chunk_count);

    // Format badge pill
    let surface = vc.surface;
    let border = vc.border;
    let badge = container(
        text(badge_text).size(11),
    )
    .padding(Padding::from([2u16, 6]))
    .style(move |_theme| container::Style {
        background: Some(Background::Color(surface)),
        border: Border {
            radius: 4.0.into(),
            color: border,
            width: 1.0,
        },
        ..Default::default()
    });

    let name_row = row![
        text(&doc.name).size(15),
        badge,
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let muted = vc.muted;
    let meta_row = row![
        text(size_text).size(12).color(muted),
        text(" · ").size(12).color(muted),
        text(date_text).size(12).color(muted),
        text(" · ").size(12).color(muted),
        text(chunks_text).size(12).color(muted),
    ]
    .align_y(Alignment::Center);

    let doc_info = column![name_row, meta_row].spacing(4);

    let destructive = vc.destructive;
    let destructive_bg = Color { r: destructive.r, g: destructive.g, b: destructive.b, a: 0.15 };
    let delete_btn = button(text("Delete").size(12))
        .on_press(Message::DeleteDocument(doc.id.clone()))
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

    let secondary_surface = vc.secondary_surface;
    container(
        row![
            container(doc_info).width(Length::Fill),
            delete_btn,
        ]
        .align_y(Alignment::Center)
        .spacing(12),
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
}
