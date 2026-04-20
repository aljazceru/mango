//! Directory Sources view (Phase 32, DIR-05).
//!
//! Lets the user add a local folder as a RAG source, edit its exclusion globs,
//! trigger a manual sync, and remove a source (cascades docs/chunks/usearch keys
//! via the actor's RemoveDirectorySource handler).
//!
//! Folder picking uses `rfd::AsyncFileDialog::pick_folder()` (non-blocking, runs
//! via `iced::Task::perform`). Exclusion globs are validated inline via
//! `mango_core::rag::directory_sync::validate_glob_pattern` so the user gets
//! immediate feedback before saving. A watcher-fallback warning banner is
//! rendered when the ENOSPC / watch-limit exhaustion case trips and the main
//! loop falls back to `PollWatcher`.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, DirectorySourceSummary, DirectorySyncStatus};

use crate::Message as RootMessage;

/// Default exclusion presets applied to new directory sources (D-30/D-32).
/// Matches the Obsidian-friendly defaults referenced in the phase context.
pub fn default_exclusion_presets() -> Vec<String> {
    vec![
        ".obsidian/".to_string(),
        ".trash/".to_string(),
        "*.tmp".to_string(),
        "*.canvas".to_string(),
        ".git/".to_string(),
    ]
}

/// Local view messages. Wrapped into `crate::Message::DirSources(_)` in main.rs.
#[derive(Debug, Clone)]
pub enum Message {
    /// User clicked "Add folder" → main.rs runs rfd::AsyncFileDialog::pick_folder().
    AddFolder,
    /// Folder picker resolved (Some = picked, None = cancelled).
    FolderPicked(Option<PathBuf>),
    /// User requested removal of a source (shows confirm modal).
    RemoveSource(String),
    /// User confirmed removal.
    ConfirmRemove(String),
    /// User cancelled the remove confirm modal.
    CancelRemove,
    /// User clicked "Edit exclusions" for a source — toggles the editor.
    EditExclusions(String),
    /// Exclusion editor text changed for (source_id, text).
    ExclusionsChanged(String, String),
    /// User saved exclusion edits.
    SaveExclusions(String),
    /// "Restore defaults" button inside the exclusion editor.
    RestoreDefaultExclusions(String),
    /// User clicked "Sync now" for a source.
    SyncNow(String),
    /// Close the exclusion editor without saving.
    CancelExclusions,
    /// User clicked "Open folder" — open path in the native file browser.
    OpenFolder(String),
}

/// Group a file count with thousands separators (e.g. 1234 → "1,234").
/// Uses ASCII comma (locale-agnostic simple form) for consistency on desktop.
fn format_file_count(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    if n < 0 {
        format!("-{}", out)
    } else {
        out
    }
}

fn status_pill<'a>(
    status: &DirectorySyncStatus,
    vc: crate::theme::ViewColors,
) -> Element<'a, RootMessage> {
    let (label, color): (&'static str, Color) = match status {
        DirectorySyncStatus::Idle => ("Idle", vc.muted),
        DirectorySyncStatus::Syncing => ("Syncing…", vc.accent),
        DirectorySyncStatus::Error { .. } => ("Error", vc.destructive),
    };
    let bg = Color {
        a: 0.15,
        ..color
    };
    container(text(label).size(11).color(color))
        .padding(Padding::from([2u16, 8]))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 8.0.into(),
                color,
                width: 1.0,
            },
            ..Default::default()
        })
        .into()
}

fn build_source_row<'a>(
    src: &'a DirectorySourceSummary,
    vc: crate::theme::ViewColors,
    editing_exclusions_for: Option<&'a str>,
    exclusion_edit_text: &'a str,
    exclusion_validation_msg: Option<&'a str>,
    pending_remove: Option<&'a str>,
) -> Element<'a, RootMessage> {
    let display_name = src.display_name.clone();
    // Use the Rust-core-provided relative-time label so desktop/iOS/Android
    // render identical strings (Plan 32-07).
    let last_synced = src.last_synced_label.clone();
    let file_count_str = format!("{} files", format_file_count(src.file_count));

    // Build name row: display name + optional full path underneath + status pill.
    let name_col = if let Some(ref path) = src.path {
        let path_text = text(path.as_str()).size(11).color(vc.muted);
        iced::widget::column![
            text(display_name).size(15),
            path_text,
        ]
        .spacing(2)
    } else {
        iced::widget::column![text(display_name).size(15)]
            .spacing(0)
    };

    let name_row = row![
        name_col,
        status_pill(&src.sync_status, vc),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let meta_row = row![
        text(file_count_str).size(12).color(vc.muted),
        text(" · ").size(12).color(vc.muted),
        text(format!("Last synced: {}", last_synced))
            .size(12)
            .color(vc.muted),
    ]
    .align_y(Alignment::Center);

    // Per-row action buttons
    let src_id_for_sync = src.id.clone();
    let sync_btn = button(text("Sync now").size(12))
        .on_press(RootMessage::DirSources(Message::SyncNow(src_id_for_sync)))
        .padding(Padding::from([4u16, 10]));

    let src_id_for_edit = src.id.clone();
    let edit_btn = button(text("Edit exclusions").size(12))
        .on_press(RootMessage::DirSources(Message::EditExclusions(src_id_for_edit)))
        .padding(Padding::from([4u16, 10]));

    let src_id_for_open = src.id.clone();
    let open_btn = button(text("Open folder").size(12))
        .on_press(RootMessage::DirSources(Message::OpenFolder(src_id_for_open)))
        .padding(Padding::from([4u16, 10]));

    let src_id_for_remove = src.id.clone();
    let remove_btn = button(text("Remove").size(12))
        .on_press(RootMessage::DirSources(Message::RemoveSource(src_id_for_remove)))
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(Color {
                a: 0.15,
                ..vc.destructive
            })),
            border: Border {
                radius: 4.0.into(),
                color: vc.destructive,
                width: 1.0,
            },
            text_color: vc.destructive,
            ..Default::default()
        });

    let actions = row![sync_btn, edit_btn, open_btn, remove_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    // Error message row (if status is Error)
    let error_row: Option<Element<'_, RootMessage>> = match &src.sync_status {
        DirectorySyncStatus::Error { message } => Some(
            text(format!("Last error: {}", message))
                .size(11)
                .color(vc.destructive)
                .into(),
        ),
        _ => None,
    };

    let mut col_children: Vec<Element<'_, RootMessage>> = vec![
        name_row.into(),
        meta_row.into(),
        actions.into(),
    ];
    if let Some(err) = error_row {
        col_children.push(err);
    }

    // Inline exclusion editor for this source (if active)
    if editing_exclusions_for == Some(src.id.as_str()) {
        col_children.push(build_exclusion_editor(
            &src.id,
            exclusion_edit_text,
            exclusion_validation_msg,
            vc,
        ));
    }

    // Inline remove-confirmation modal for this source
    if pending_remove == Some(src.id.as_str()) {
        col_children.push(build_remove_confirm(&src.id, src.file_count, vc));
    }

    container(column(col_children).spacing(6))
        .padding(Padding::from([10u16, 14]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(vc.secondary_surface)),
            border: Border {
                radius: 6.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        })
        .into()
}

fn build_exclusion_editor<'a>(
    source_id: &'a str,
    edit_text: &'a str,
    validation_msg: Option<&'a str>,
    vc: crate::theme::ViewColors,
) -> Element<'a, RootMessage> {
    let sid_changed = source_id.to_string();
    let input = text_input("one glob per line, e.g. *.tmp", edit_text)
        .on_input(move |v| RootMessage::DirSources(Message::ExclusionsChanged(sid_changed.clone(), v)))
        .padding(6);

    let sid_save = source_id.to_string();
    let save_btn = button(text("Save").size(12))
        .on_press(RootMessage::DirSources(Message::SaveExclusions(sid_save)))
        .padding(Padding::from([4u16, 10]));

    let sid_default = source_id.to_string();
    let restore_btn = button(text("Restore defaults").size(12))
        .on_press(RootMessage::DirSources(Message::RestoreDefaultExclusions(sid_default)))
        .padding(Padding::from([4u16, 10]));

    let cancel_btn = button(text("Cancel").size(12))
        .on_press(RootMessage::DirSources(Message::CancelExclusions))
        .padding(Padding::from([4u16, 10]));

    let help = text(
        "Globs ending in / exclude directories (e.g. .obsidian/). Use *.ext for extensions.",
    )
    .size(11)
    .color(vc.muted);

    let buttons = row![save_btn, restore_btn, cancel_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    let mut children: Vec<Element<'_, RootMessage>> = vec![
        help.into(),
        input.into(),
        buttons.into(),
    ];
    if let Some(msg) = validation_msg {
        children.push(text(msg).size(11).color(vc.destructive).into());
    }

    container(column(children).spacing(6))
        .padding(Padding::from([8u16, 10]))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(vc.card)),
            border: Border {
                radius: 4.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        })
        .into()
}

fn build_remove_confirm<'a>(
    source_id: &'a str,
    file_count: i64,
    vc: crate::theme::ViewColors,
) -> Element<'a, RootMessage> {
    let sid_confirm = source_id.to_string();
    let confirm_btn = button(text("Remove and delete indexed chunks").size(12))
        .on_press(RootMessage::DirSources(Message::ConfirmRemove(sid_confirm)))
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(vc.destructive)),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            text_color: Color::WHITE,
            ..Default::default()
        });

    let cancel_btn = button(text("Cancel").size(12))
        .on_press(RootMessage::DirSources(Message::CancelRemove))
        .padding(Padding::from([4u16, 10]));

    let warning = text(format!(
        "Remove source and delete {} indexed chunks? This cannot be undone.",
        format_file_count(file_count)
    ))
    .size(12)
    .color(vc.destructive);

    container(
        column![
            warning,
            row![confirm_btn, cancel_btn]
                .spacing(8)
                .align_y(Alignment::Center)
        ]
        .spacing(8),
    )
    .padding(Padding::from([8u16, 10]))
    .style(move |_theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.08,
            ..vc.destructive
        })),
        border: Border {
            radius: 4.0.into(),
            color: vc.destructive,
            width: 1.0,
        },
        ..Default::default()
    })
    .into()
}

/// Full-screen Directory Sources view. Rendered when
/// `state.router.current_screen == Screen::DirectorySources`.
///
/// `watcher_warning` = Some(message) surfaces the ENOSPC / PollWatcher fallback
/// banner per D-11.
pub fn view<'a>(
    state: &'a AppState,
    watcher_warning: Option<&'a str>,
    editing_exclusions_for: Option<&'a str>,
    exclusion_edit_text: &'a str,
    exclusion_validation: &'a HashMap<String, String>,
    pending_remove_id: Option<&'a str>,
    is_dark: bool,
) -> Element<'a, RootMessage> {
    let vc = crate::theme::view_colors(is_dark);

    // ── Header ────────────────────────────────────────────────────────────────
    let back_btn = button(text("Back").size(14))
        .on_press(RootMessage::DispatchAction(AppAction::PopScreen))
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(vc.surface)),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let add_btn = button(text("Add folder").size(14).color(vc.bg))
        .on_press(RootMessage::DirSources(Message::AddFolder))
        .padding(Padding::from([4u16, 12]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(vc.accent)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            text_color: vc.bg,
            ..Default::default()
        });

    let header = container(
        row![
            back_btn,
            text("Directory Sources").size(22),
            Space::new().width(Length::Fill),
            add_btn,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(vc.secondary_surface)),
        ..Default::default()
    });

    // ── Watcher fallback warning banner (ENOSPC / PollWatcher) ────────────────
    let warning_banner: Option<Element<'a, RootMessage>> = watcher_warning.map(|msg| {
        container(
            row![
                text("Warning").size(12).color(vc.warning),
                text(msg).size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([6u16, 12]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.12,
                ..vc.warning
            })),
            border: Border {
                color: vc.warning,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
    });

    // ── Source list or empty state ────────────────────────────────────────────
    let content_section: Element<'a, RootMessage> = if state.directory_sources.is_empty() {
        container(
            column![
                text("No directory sources yet. Add a folder to sync your notes.")
                    .size(14)
                    .color(vc.muted),
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
        let rows: Vec<Element<'a, RootMessage>> = state
            .directory_sources
            .iter()
            .map(|src| {
                let validation = exclusion_validation.get(&src.id).map(|s| s.as_str());
                build_source_row(
                    src,
                    vc,
                    editing_exclusions_for,
                    exclusion_edit_text,
                    validation,
                    pending_remove_id,
                )
            })
            .collect();

        let list = column(rows).spacing(10).padding(Padding::from([8u16, 16]));
        scrollable(list).height(Length::Fill).width(Length::Fill).into()
    };

    // ── Compose full layout ───────────────────────────────────────────────────
    let mut page_children: Vec<Element<'a, RootMessage>> = vec![header.into()];
    if let Some(banner) = warning_banner {
        page_children.push(banner);
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
            background: Some(Background::Color(vc.bg)),
            ..Default::default()
        })
        .into()
}

/// Parse a multi-line exclusion editor string into a Vec<String> (one glob per
/// non-empty line), validate each entry against `validate_glob_pattern`, and
/// return either the normalised list or the first error message.
pub fn parse_and_validate_exclusions(raw: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Err(e) = mango_core::rag::directory_sync::validate_glob_pattern(trimmed) {
            return Err(format!("Line {}: {}", idx + 1, e));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}
