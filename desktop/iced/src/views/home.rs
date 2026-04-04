use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, ConversationSummary, Screen};

use crate::Message;

fn relative_time(epoch_millis: i64) -> String {
    let now_millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    // Phase 5: now_secs() returns milliseconds for precision
    let diff_secs = (now_millis - epoch_millis) / 1000;

    if diff_secs < 60 {
        "just now".to_string()
    } else if diff_secs < 3600 {
        let mins = diff_secs / 60;
        format!("{}m ago", mins)
    } else if diff_secs < 86400 {
        let hours = diff_secs / 3600;
        format!("{}h ago", hours)
    } else if diff_secs < 172800 {
        "yesterday".to_string()
    } else {
        let days = diff_secs / 86400;
        if days < 365 {
            format!("{}d ago", days)
        } else {
            "long ago".to_string()
        }
    }
}

fn model_short_name(model_id: &str) -> &str {
    // Extract last path segment: "meta-llama/Llama-3.3-70B-Instruct" -> "Llama-3.3-70B-Instruct"
    model_id.rsplit('/').next().unwrap_or(model_id)
}

pub fn sidebar_view<'a>(
    state: &'a AppState,
    rename_state: &'a Option<(String, String)>,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    let new_conv_btn = button(
        container(text("New Conversation").size(14))
            .padding(Padding::from([6u16, 12]))
            .style(move |_theme| container::Style {
                background: Some(Background::Color(vc.accent)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .on_press(Message::DispatchAction(AppAction::NewConversation))
    .padding(0)
    .width(Length::Fill)
    .style(|_theme, _status| button::Style {
        background: None,
        ..Default::default()
    });

    if state.conversations.is_empty() {
        // Empty state
        let empty = column![
            container(new_conv_btn)
                .padding(Padding { top: 16.0, right: 16.0, bottom: 8.0, left: 16.0 })
                .width(Length::Fill),
            container(
                column![
                    text("No conversations yet").size(20),
                    text("Start a new conversation to chat with a private AI.").size(14),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill),
        ];
        return container(empty)
            .width(Length::Fixed(240.0))
            .height(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(vc.secondary_surface)),
                ..Default::default()
            })
            .into();
    }

    let conv_rows: Vec<Element<'_, Message>> = state
        .conversations
        .iter()
        .map(|conv| {
            let is_selected = state
                .current_conversation_id
                .as_deref()
                .map(|cid| cid == conv.id)
                .unwrap_or(false);
            build_conversation_row(conv, is_selected, rename_state, state, vc)
        })
        .collect();

    let conv_list = column(conv_rows).spacing(2).padding(Padding::from([4u16, 8]));
    let list_scroll = scrollable(conv_list).height(Length::Fill);

    // AGENTS HIDDEN: agents_btn removed until polished

    let memories_btn = button(
        container(text("Memories").size(13))
            .padding(Padding::from([4u16, 12])),
    )
    .on_press(Message::OpenMemories)
    .padding(0)
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: vc.text_dim,
        ..Default::default()
    });

    let docs_btn = button(
        container(text("Documents").size(13))
            .padding(Padding::from([4u16, 12])),
    )
    .on_press(Message::OpenDocuments)
    .padding(0)
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: vc.text_dim,
        ..Default::default()
    });

    let settings_btn = button(
        container(text("Settings").size(13))
            .padding(Padding::from([4u16, 12])),
    )
    .on_press(Message::DispatchAction(AppAction::PushScreen {
        screen: Screen::Settings,
    }))
    .padding(0)
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: None,
        text_color: vc.text_dim,
        ..Default::default()
    });

    let bottom_nav = column![
        // AGENTS HIDDEN: agents_btn entry removed until polished
        container(memories_btn)
            .padding(Padding { top: 0.0, right: 8.0, bottom: 4.0, left: 8.0 })
            .width(Length::Fill),
        container(docs_btn)
            .padding(Padding { top: 0.0, right: 8.0, bottom: 4.0, left: 8.0 })
            .width(Length::Fill),
        container(settings_btn)
            .padding(Padding { top: 0.0, right: 8.0, bottom: 8.0, left: 8.0 })
            .width(Length::Fill),
    ]
    .spacing(0);

    let sidebar_content = column![
        container(new_conv_btn)
            .padding(Padding { top: 16.0, right: 8.0, bottom: 8.0, left: 8.0 })
            .width(Length::Fill),
        list_scroll,
        bottom_nav,
    ]
    .spacing(0);

    container(sidebar_content)
        .width(Length::Fixed(240.0))
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(vc.secondary_surface)),
            ..Default::default()
        })
        .into()
}

fn build_conversation_row<'a>(
    conv: &'a ConversationSummary,
    is_selected: bool,
    rename_state: &'a Option<(String, String)>,
    state: &'a AppState,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let id = conv.id.clone();

    // If this conversation is being renamed, show rename input
    if let Some((rename_id, rename_text)) = rename_state {
        if rename_id == &conv.id {
            let rename_input = text_input("Conversation name", rename_text)
                .on_input(Message::RenameChanged)
                .on_submit(Message::SubmitRename)
                .size(14)
                .padding(Padding::from([4u16, 8]));

            let rename_row = row![
                rename_input,
                button(text("OK").size(12))
                    .on_press(Message::SubmitRename)
                    .padding(Padding::from([2u16, 6])),
                button(text("X").size(12))
                    .on_press(Message::CancelRename)
                    .padding(Padding::from([2u16, 6])),
            ]
            .spacing(4);

            return container(rename_row)
                .padding(Padding::from([6u16, 8]))
                .width(Length::Fill)
                .into();
        }
    }

    let timestamp = relative_time(conv.updated_at);
    let model_name = model_short_name(&conv.model_id).to_string();
    let backend_name = state
        .backends
        .iter()
        .find(|b| b.id == conv.backend_id)
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let title_text = text(conv.title.clone()).size(14);
    let ts_text = text(timestamp).size(12).color(vc.text_dim);
    let model_text = text(model_name).size(11).color(vc.muted);
    let backend_text = text(backend_name).size(11).color(vc.muted);

    let meta_row = row![
        ts_text,
        text(" · ").size(11),
        model_text,
        text(" · ").size(11),
        backend_text,
    ]
    .align_y(Alignment::Center);

    let content_col = column![title_text, meta_row].spacing(2);

    let rename_btn = button(text("...").size(12))
        .on_press(Message::StartRename(id.clone(), conv.title.clone()))
        .padding(Padding::from([2u16, 4]))
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: vc.text_dim,
            ..Default::default()
        });

    let delete_btn = button(text("X").size(10))
        .on_press(Message::ConfirmDelete(id.clone()))
        .padding(Padding::from([2u16, 4]))
        .style(move |_theme, _status| button::Style {
            background: None,
            text_color: vc.destructive,
            ..Default::default()
        });

    let row_content = row![
        container(content_col).width(Length::Fill),
        rename_btn,
        delete_btn,
    ]
    .align_y(Alignment::Center)
    .spacing(4);

    let surface_color = vc.surface;
    let accent_color = vc.accent;
    let row_btn = button(container(row_content).padding(Padding::from([6u16, 8])).width(Length::Fill))
        .on_press(Message::OpenConversation(id.clone()))
        .padding(0)
        .width(Length::Fill)
        .style(move |_theme, _status| {
            if is_selected {
                button::Style {
                    background: Some(Background::Color(surface_color)),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            } else {
                button::Style {
                    background: None,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }
        });

    if is_selected {
        // Accent left border strip for selected conversation
        row![
            container(iced::widget::Space::new().width(2.0).height(Length::Fill))
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(accent_color)),
                    ..Default::default()
                }),
            row_btn,
        ]
        .into()
    } else {
        row_btn.into()
    }
}
