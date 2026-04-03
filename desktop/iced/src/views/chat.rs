use std::collections::HashMap;

use iced::widget::{
    button, center, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};
use iced::widget::markdown;
use iced::Theme;

use mango_core::{AppAction, AppState, BusyState, UiMessage};

use crate::widgets::attestation_badge;
use crate::Message;

// Streaming cursor character (per UI-SPEC)
const STREAM_CURSOR: char = '\u{258B}';

fn md_settings(theme: &Theme) -> markdown::Settings {
    markdown::Settings::with_style(markdown::Style::from_palette(theme.palette()))
}

pub fn chat_view<'a>(
    state: &'a AppState,
    theme: &'a Theme,
    is_dark: bool,
    streaming_content: &'a markdown::Content,
    input_text: &'a str,
    edit_state: &'a Option<(String, String)>,
    show_attestation_detail: bool,
    show_system_prompt_input: bool,
    system_prompt_text: &'a str,
    parsed_messages: &'a HashMap<String, Vec<markdown::Item>>,
    show_docs_attachment_overlay: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);
    let is_streaming = matches!(&state.busy_state, BusyState::Streaming { .. });

    // ── Header ──────────────────────────────────────────────────────────────
    let conv_title = state
        .current_conversation_id
        .as_deref()
        .and_then(|cid| state.conversations.iter().find(|c| c.id == cid))
        .map(|c| c.title.as_str())
        .unwrap_or("New Conversation");

    let title_elem = text(conv_title).size(20);
    let badge_elem = attestation_badge::view(state, show_attestation_detail, is_dark);

    // Model picker: collect available models from active backend
    let available_models: Vec<String> = state
        .active_backend_id
        .as_deref()
        .and_then(|bid| state.backends.iter().find(|b| b.id == bid))
        .map(|b| b.models.clone())
        .unwrap_or_default();

    let current_model = state
        .current_conversation_id
        .as_deref()
        .and_then(|cid| state.conversations.iter().find(|c| c.id == cid))
        .map(|c| c.model_id.clone())
        .unwrap_or_default();

    let model_picker: Element<'_, Message> = if available_models.is_empty() {
        text("No models").size(14).into()
    } else {
        let selected = if available_models.contains(&current_model) {
            Some(current_model.clone())
        } else {
            None
        };
        pick_list(available_models, selected, Message::SelectModel)
            .placeholder("Select model")
            .text_size(14)
            .into()
    };

    // Docs button: shows attached count, toggles attachment overlay (D-08)
    let attached_count = state.current_conversation_attached_docs.len();
    let docs_label = if attached_count > 0 {
        format!("Docs ({})", attached_count)
    } else {
        "Docs".to_string()
    };
    let docs_active_bg = vc.accent_dim;
    let docs_inactive_bg = vc.surface;
    let docs_btn = button(text(docs_label).size(13))
        .on_press(Message::ToggleDocAttachmentOverlay)
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(if show_docs_attachment_overlay {
                docs_active_bg
            } else {
                docs_inactive_bg
            })),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            text_color: vc.text_dim,
            ..Default::default()
        });

    let header_row = row![
        title_elem,
        iced::widget::Space::new().width(Length::Fill),
        badge_elem,
        docs_btn,
        model_picker,
    ]
    .align_y(Alignment::Center)
    .spacing(8)
    .padding(Padding::from([8u16, 16]));

    // ── System prompt (Instructions) section ────────────────────────────────
    let instructions_section: Element<'_, Message> = if show_system_prompt_input {
        let prompt_input = text_input(
            "Optional: give the assistant a role or set of instructions.",
            system_prompt_text,
        )
        .on_input(Message::SystemPromptChanged)
        .on_submit(Message::SubmitSystemPrompt)
        .size(14)
        .padding(Padding::from([6u16, 10]));

        let text_dim = vc.text_dim;
        let action_row = row![
            button(text("Save").size(13))
                .on_press(Message::SubmitSystemPrompt)
                .padding(Padding::from([4u16, 10])),
            button(text("Cancel").size(13))
                .on_press(Message::ToggleSystemPromptInput)
                .padding(Padding::from([4u16, 10]))
                .style(move |_theme, _status| button::Style {
                    background: None,
                    text_color: text_dim,
                    ..Default::default()
                }),
        ]
        .spacing(8);

        let secondary_surface = vc.secondary_surface;
        container(
            column![
                text("Instructions").size(14),
                prompt_input,
                action_row,
            ]
            .spacing(6),
        )
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            ..Default::default()
        })
        .into()
    } else {
        let text_dim = vc.text_dim;
        let border = vc.border;
        let instructions_btn = button(
            text("Instructions").size(13).color(text_dim),
        )
        .on_press(Message::ToggleSystemPromptInput)
        .padding(Padding::from([2u16, 8]))
        .style(move |_theme, _status| button::Style {
            background: None,
            border: Border {
                color: border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });

        container(instructions_btn)
            .padding(Padding::from([4u16, 16]))
            .width(Length::Fill)
            .into()
    };

    // ── Message thread ───────────────────────────────────────────────────────
    let messages_count = state.messages.len();

    // D-17: show welcome placeholder when flag is true and messages list is empty
    let show_placeholder = state.show_first_chat_placeholder && state.messages.is_empty();

    let message_widgets: Vec<Element<'_, Message>> = state
        .messages
        .iter()
        .enumerate()
        .map(|(i, msg)| {
            let is_last = i == messages_count.saturating_sub(1);
            render_message(msg, is_last, is_streaming, streaming_content, edit_state, parsed_messages, theme, vc)
        })
        .collect();

    // If actively streaming with content not yet in messages list, append streaming bubble
    let mut all_widgets: Vec<Element<'_, Message>> = message_widgets;
    if is_streaming && !streaming_content.items().is_empty() {
        all_widgets.push(render_streaming_bubble(streaming_content, theme, vc));
    }

    let muted_color = vc.muted;
    let msg_column: Element<'_, Message> = if show_placeholder {
        // D-17 welcome placeholder
        center(
            text("You're all set! Send your first message to start a confidential conversation.")
                .size(16)
                .color(muted_color),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        column(all_widgets)
            .spacing(8)
            .padding(Padding::from([8u16, 16]))
            .into()
    };

    // Error bubble inline if last_error is set
    let thread_with_error: Element<'_, Message> = if let Some(err) = &state.last_error {
        let error_bubble = build_error_bubble(err, vc);
        column![msg_column, error_bubble].spacing(4).into()
    } else {
        msg_column.into()
    };

    let messages_scroll = scrollable(thread_with_error)
        .anchor_bottom()
        .height(Length::Fill)
        .width(Length::Fill);

    // ── Compose bar ──────────────────────────────────────────────────────────
    let compose_area = build_compose_bar(state, input_text, is_streaming, vc);

    // ── Document attachment overlay ───────────────────────────────────────────
    let docs_overlay: Option<Element<'_, Message>> = if show_docs_attachment_overlay {
        let doc_items: Vec<Element<'_, Message>> = if state.documents.is_empty() {
            vec![text("No documents in library.")
                .size(13)
                .color(muted_color)
                .into()]
        } else {
            let accent = vc.accent;
            state
                .documents
                .iter()
                .map(|doc| {
                    let is_attached = state
                        .current_conversation_attached_docs
                        .contains(&doc.id);
                    let check_label = if is_attached { "[x]" } else { "[ ]" };
                    button(
                        row![
                            text(check_label).size(12).color(if is_attached { accent } else { muted_color }),
                            text(&doc.name).size(13),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    )
                    .on_press(Message::ToggleDocumentAttachment(doc.id.clone()))
                    .padding(Padding::from([4u16, 8]))
                    .width(Length::Fill)
                    .style(|_theme, _status| button::Style {
                        background: None,
                        ..Default::default()
                    })
                    .into()
                })
                .collect()
        };

        let overlay_bg = vc.accent_dim;
        let accent = vc.accent;
        Some(
            container(
                column![
                    text("Attach Documents").size(14),
                    column(doc_items).spacing(2),
                ]
                .spacing(8),
            )
            .padding(Padding::from([10u16, 14]))
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(overlay_bg)),
                border: Border {
                    color: accent,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
            .into(),
        )
    } else {
        None
    };

    // ── Full layout ──────────────────────────────────────────────────────────
    let secondary_surface = vc.secondary_surface;
    let mut col_children: Vec<Element<'_, Message>> = vec![
        container(header_row)
            .width(Length::Fill)
            .style(move |_theme| container::Style {
                background: Some(Background::Color(secondary_surface)),
                ..Default::default()
            })
            .into(),
    ];
    if let Some(overlay) = docs_overlay {
        col_children.push(overlay);
    }
    col_children.push(instructions_section);
    col_children.push(messages_scroll.into());
    col_children.push(compose_area);

    let chat_col = column(col_children)
        .width(Length::Fill)
        .height(Length::Fill);

    let bg_color = vc.bg;
    container(chat_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..Default::default()
        })
        .into()
}

fn render_message<'a>(
    msg: &'a UiMessage,
    is_last: bool,
    is_streaming: bool,
    streaming_content: &'a markdown::Content,
    edit_state: &'a Option<(String, String)>,
    parsed_messages: &'a HashMap<String, Vec<markdown::Item>>,
    theme: &'a Theme,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    // Check if this message is in edit mode
    if let Some((edit_id, edit_text)) = edit_state {
        if edit_id == &msg.id {
            return render_edit_mode(edit_text, vc);
        }
    }

    match msg.role.as_str() {
        "user" => render_user_message(msg, vc),
        "assistant" => {
            // If this is the last assistant message AND currently streaming, show streaming content
            let show_streaming = is_last && is_streaming;
            render_assistant_message(msg, is_last, show_streaming, streaming_content, parsed_messages, theme, vc)
        }
        _ => {
            // System messages: simple display
            let muted = vc.muted;
            container(text(&msg.content).size(13).color(muted))
                .padding(Padding::from([4u16, 16]))
                .width(Length::Fill)
                .into()
        }
    }
}

fn render_user_message<'a>(msg: &'a UiMessage, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let user_bubble = vc.user_bubble;
    let text_dim = vc.text_dim;
    let surface = vc.surface;
    let content_elem: Element<'_, Message> = if msg.has_attachment {
        let attach_label = msg.attachment_name.as_deref().unwrap_or("attachment");
        column![
            container(text(attach_label).size(12).color(text_dim))
                .padding(Padding::from([2u16, 8]))
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(surface)),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            text(&msg.content).size(16),
        ]
        .spacing(4)
        .into()
    } else {
        text(&msg.content).size(16).into()
    };

    let msg_bubble = container(content_elem)
        .padding(Padding::from([8u16, 12]))
        .max_width(480.0)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(user_bubble)),
            border: Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let copy_btn = button(text("Copy").size(12))
        .on_press(Message::CopyMessage(msg.content.clone()))
        .padding(Padding::from([2u16, 6]))
        .style(move |theme, status| action_btn_style(theme, status, vc.surface, vc.text_dim));

    let edit_btn = button(text("Edit").size(12))
        .on_press(Message::StartEdit(msg.id.clone(), msg.content.clone()))
        .padding(Padding::from([2u16, 6]))
        .style(move |theme, status| action_btn_style(theme, status, vc.surface, vc.text_dim));

    let action_row = row![copy_btn, edit_btn].spacing(4);

    let bubble_col = column![msg_bubble, action_row]
        .spacing(4)
        .align_x(Alignment::End);

    container(bubble_col)
        .width(Length::Fill)
        .align_right(Length::Fill)
        .into()
}

fn render_assistant_message<'a>(
    msg: &'a UiMessage,
    is_last: bool,
    show_streaming: bool,
    streaming_content: &'a markdown::Content,
    parsed_messages: &'a HashMap<String, Vec<markdown::Item>>,
    theme: &'a Theme,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let md_content: Element<'_, Message> = if show_streaming {
        // Streaming: use streaming_content (not lazy-wrapped -- changes every frame)
        build_md_view(streaming_content.items(), true, theme)
    } else {
        // Completed message: use pre-parsed items from parsed_messages (app state)
        // Per iced docs: store Vec<markdown::Item> in app state, not parsed in view()
        if let Some(items) = parsed_messages.get(&msg.id) {
            if items.is_empty() {
                text(&msg.content).size(16).into()
            } else {
                markdown::view(items.iter(), md_settings(theme))
                    .map(|_uri| Message::ToggleAttestationDetail)
                    .into()
            }
        } else {
            // Not yet parsed (e.g., freshly received) -- show plain text
            text(&msg.content).size(16).into()
        }
    };

    let secondary_surface = vc.secondary_surface;
    let msg_bubble = container(md_content)
        .padding(Padding::from([8u16, 12]))
        .max_width(640.0)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            border: Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let copy_btn = button(text("Copy").size(12))
        .on_press(Message::CopyMessage(msg.content.clone()))
        .padding(Padding::from([2u16, 6]))
        .style(move |theme, status| action_btn_style(theme, status, vc.surface, vc.text_dim));

    let mut actions: Vec<Element<'_, Message>> = vec![copy_btn.into()];

    if is_last && !show_streaming {
        let retry_btn = button(text("Retry").size(12))
            .on_press(Message::RetryMessage)
            .padding(Padding::from([2u16, 6]))
            .style(move |theme, status| action_btn_style(theme, status, vc.surface, vc.text_dim));
        actions.push(retry_btn.into());
    }

    let action_row = row(actions).spacing(4);

    // RAG context indicator (D-07): show subtle label when RAG contributed context
    let muted = vc.muted;
    let rag_indicator: Option<Element<'_, Message>> =
        msg.rag_context_count.and_then(|n| {
            if n > 0 {
                Some(
                    text(format!("[context from {} doc(s)]", n))
                        .size(11)
                        .color(muted)
                        .into(),
                )
            } else {
                None
            }
        });

    let mut bubble_col_children: Vec<Element<'_, Message>> = vec![msg_bubble.into()];
    if let Some(rag_elem) = rag_indicator {
        bubble_col_children.push(rag_elem);
    }
    bubble_col_children.push(action_row.into());

    let bubble_col = column(bubble_col_children)
        .spacing(4)
        .align_x(Alignment::Start);

    container(bubble_col).width(Length::Fill).into()
}

fn build_md_view<'a>(
    items: impl IntoIterator<Item = &'a markdown::Item>,
    with_cursor: bool,
    theme: &'a Theme,
) -> Element<'a, Message> {
    let items_vec: Vec<&'a markdown::Item> = items.into_iter().collect();
    if items_vec.is_empty() {
        if with_cursor {
            text(STREAM_CURSOR).size(16).into()
        } else {
            iced::widget::Space::new().into()
        }
    } else {
        let md_elem = markdown::view(items_vec.into_iter(), md_settings(theme))
            .map(|_uri| Message::ToggleAttestationDetail);
        if with_cursor {
            column![md_elem, text(STREAM_CURSOR).size(16)]
                .spacing(0)
                .into()
        } else {
            md_elem.into()
        }
    }
}

fn render_streaming_bubble<'a>(
    streaming_content: &'a markdown::Content,
    theme: &'a Theme,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let md_elem = build_md_view(streaming_content.items(), true, theme);
    let secondary_surface = vc.secondary_surface;
    container(md_elem)
        .padding(Padding::from([8u16, 12]))
        .max_width(640.0)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            border: Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn render_edit_mode<'a>(edit_text: &'a str, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let edit_input = text_input("Edit message...", edit_text)
        .on_input(Message::EditChanged)
        .on_submit(Message::SubmitEdit)
        .size(14)
        .padding(Padding::from([6u16, 10]));

    let text_dim = vc.text_dim;
    let action_row = row![
        button(text("Save").size(12))
            .on_press(Message::SubmitEdit)
            .padding(Padding::from([2u16, 8])),
        button(text("Cancel").size(12))
            .on_press(Message::CancelEdit)
            .padding(Padding::from([2u16, 8]))
            .style(move |_theme, _status| button::Style {
                background: None,
                text_color: text_dim,
                ..Default::default()
            }),
    ]
    .spacing(8);

    let user_bubble = vc.user_bubble;
    let accent = vc.accent;
    let edit_bubble = container(column![edit_input, action_row].spacing(6))
        .padding(Padding::from([8u16, 12]))
        .max_width(480.0)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(user_bubble)),
            border: Border {
                radius: 12.0.into(),
                color: accent,
                width: 1.0,
            },
            ..Default::default()
        });

    container(edit_bubble)
        .width(Length::Fill)
        .align_right(Length::Fill)
        .into()
}

fn build_error_bubble<'a>(error: &'a str, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let destructive = vc.destructive;
    let error_bg = Color { r: destructive.r, g: destructive.g, b: destructive.b, a: 0.15 };
    container(
        row![
            text("!").size(14).color(destructive),
            text(error).size(14).color(destructive),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([8u16, 12]))
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(error_bg)),
        border: Border {
            color: destructive,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn build_compose_bar<'a>(
    state: &'a AppState,
    input_text: &'a str,
    is_streaming: bool,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    // Pending attachment indicator above the input
    let text_dim = vc.text_dim;
    let destructive = vc.destructive;
    let attachment_row: Option<Element<'_, Message>> = state.pending_attachment.as_ref().map(|att| {
        let filename = att.filename.clone();
        let size_display = att.size_display.clone();
        row![
            text(format!("{} ({})", filename, size_display))
                .size(13)
                .color(text_dim),
            button(text("X").size(12))
                .on_press(Message::ClearAttachment)
                .padding(Padding::from([1u16, 4]))
                .style(move |_theme, _status| button::Style {
                    background: None,
                    text_color: destructive,
                    ..Default::default()
                }),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    });

    // Attach button
    let surface = vc.surface;
    let attach_btn = button(text("Attach").size(14))
        .on_press(Message::AttachFile)
        .padding(Padding::from([6u16, 12]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(surface)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    // Text input (disabled while streaming)
    let msg_input: Element<'_, Message> = if is_streaming {
        text_input("Streaming...", input_text)
            .size(14)
            .padding(Padding::from([6u16, 10]))
            .into()
    } else {
        text_input("Message...", input_text)
            .on_input(Message::InputChanged)
            .on_submit(Message::SubmitMessage)
            .size(14)
            .padding(Padding::from([6u16, 10]))
            .into()
    };

    // Send or Stop button
    let accent = vc.accent;
    let muted = vc.muted;
    let secondary_surface = vc.secondary_surface;
    let cta_btn: Element<'_, Message> = if is_streaming {
        button(text("Stop").size(14))
            .on_press(Message::DispatchAction(AppAction::StopGeneration))
            .padding(Padding::from([6u16, 16]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(secondary_surface)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else if !input_text.is_empty() {
        button(text("Send").size(14))
            .on_press(Message::SubmitMessage)
            .padding(Padding::from([6u16, 16]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        button(text("Send").size(14))
            .padding(Padding::from([6u16, 16]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(secondary_surface)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                text_color: muted,
                ..Default::default()
            })
            .into()
    };

    let input_row = row![attach_btn, msg_input, cta_btn]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let compose_content: Element<'_, Message> = if let Some(att_row) = attachment_row {
        column![att_row, input_row].spacing(6).into()
    } else {
        input_row.into()
    };

    container(compose_content)
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            ..Default::default()
        })
        .into()
}

fn action_btn_style(_theme: &iced::Theme, _status: button::Status, bg: Color, text_color: Color) -> button::Style {
    button::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        text_color,
        ..Default::default()
    }
}
