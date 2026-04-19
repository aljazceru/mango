use std::collections::HashMap;

use iced::widget::markdown;
use iced::widget::{
    button, center, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::Theme;
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, BusyState, UiMessage};

use crate::Message;

// Streaming cursor character (per UI-SPEC)
const STREAM_CURSOR: char = '\u{258B}';

fn md_settings(theme: &Theme) -> markdown::Settings {
    markdown::Settings::with_style(markdown::Style::from_palette(theme.palette()))
}

#[allow(clippy::too_many_arguments)]
pub fn chat_view<'a>(
    state: &'a AppState,
    theme: &'a Theme,
    is_dark: bool,
    streaming_content: &'a markdown::Content,
    input_text: &'a str,
    edit_state: &'a Option<(String, String)>,
    _show_attestation_detail: bool,
    show_system_prompt_input: bool,
    system_prompt_text: &'a str,
    parsed_messages: &'a HashMap<String, Vec<markdown::Item>>,
    show_docs_attachment_overlay: bool,
    show_conv_menu: bool,
    show_tools_panel: bool,
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

    // Model picker: collect available models from active backend.
    // Show a small colored dot before the pick_list to indicate attestation status.
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

    // Attestation dot next to the model picker (replaces separate badge widget in header)
    let attest_status = state
        .active_backend_id
        .as_deref()
        .and_then(|id| {
            state
                .attestation_statuses
                .iter()
                .find(|e| e.backend_id == id)
        })
        .map(|e| &e.status);
    let attest_dot: Option<Element<'_, Message>> = attest_status.and_then(|s| {
        use mango_core::AttestationStatus;
        let dot_color = match s {
            AttestationStatus::Verified => Some(Color::from_rgb(0.20, 0.75, 0.30)),
            AttestationStatus::Expired => Some(Color::from_rgb(0.98, 0.75, 0.14)),
            AttestationStatus::Failed { .. } => Some(Color::from_rgb(0.90, 0.24, 0.24)),
            AttestationStatus::Unverified => None,
        };
        dot_color.map(|c| {
            container(iced::widget::Space::new().width(7).height(7))
                .style(move |_| container::Style {
                    background: Some(Background::Color(c)),
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        })
    });

    // "..." button: toggles the conv options panel (Docs / Instructions / Tools)
    let menu_active_bg = vc.accent_dim;
    let menu_inactive_bg = vc.surface;
    let conv_menu_btn = button(text("···").size(16))
        .on_press(Message::ToggleConvMenu)
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(if show_conv_menu {
                menu_active_bg
            } else {
                menu_inactive_bg
            })),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            text_color: vc.text_dim,
            ..Default::default()
        });

    let mut header_children: Vec<Element<'_, Message>> = vec![title_elem.into()];
    header_children.push(iced::widget::Space::new().width(Length::Fill).into());
    if let Some(dot) = attest_dot {
        header_children.push(dot);
    }
    header_children.push(conv_menu_btn.into());
    header_children.push(model_picker);

    let header_row = row(header_children)
        .align_y(Alignment::Center)
        .spacing(8)
        .padding(Padding::from([8u16, 16]));

    // ── Conversation options panel (replaces separate Instructions row + inline buttons) ──
    // Shown below the header when show_conv_menu is true.
    let tools_enabled = state
        .current_conversation_id
        .as_deref()
        .and_then(|cid| state.conversations.iter().find(|c| c.id == cid))
        .map(|c| c.tools_enabled)
        .unwrap_or(false);
    let attached_count = state.current_conversation_attached_docs.len();

    let instructions_section: Element<'_, Message> = if show_conv_menu {
        // ── RAG row ──
        let rag_label = if attached_count > 0 {
            format!("RAG ({})", attached_count)
        } else {
            "RAG".to_string()
        };
        let docs_active_bg = vc.accent_dim;
        let docs_inactive_bg = vc.surface;
        let docs_btn = button(text(rag_label).size(13))
            .on_press(Message::ToggleDocAttachmentOverlay)
            .padding(Padding::from([4u16, 8]))
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

        // ── Tools row: button opens sub-panel ──
        let tools_label = if tools_enabled { "Tools: On" } else { "Tools" };
        let tools_panel_bg = if show_tools_panel {
            vc.accent_dim
        } else {
            vc.surface
        };
        let tools_btn = button(text(tools_label).size(13))
            .on_press(Message::ToggleToolsPanel)
            .padding(Padding::from([4u16, 8]))
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(tools_panel_bg)),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                text_color: vc.text_dim,
                ..Default::default()
            });

        // ── Tools sub-panel: individual tool toggles ──
        let brave_key_set = state.brave_api_key_set;
        let tools_sub_panel: Option<Element<'_, Message>> = if show_tools_panel {
            // Brave Search toggle row
            let brave_label = if tools_enabled {
                "Brave Search: On"
            } else {
                "Brave Search: Off"
            };
            let brave_note = if !brave_key_set {
                Some(
                    text("API key not configured — set in Settings")
                        .size(11)
                        .color(vc.muted)
                        .into(),
                )
            } else {
                None
            };
            let brave_toggle_color = if tools_enabled && brave_key_set {
                vc.accent
            } else {
                vc.surface
            };
            let brave_text_color = if tools_enabled && brave_key_set {
                Color::WHITE
            } else {
                vc.text_dim
            };
            let brave_btn: Element<'_, Message> = if brave_key_set {
                button(text(brave_label).size(12).color(brave_text_color))
                    .on_press(Message::ToggleConvToolsEnabled)
                    .padding(Padding::from([3u16, 8]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(brave_toggle_color)),
                        border: Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        text_color: brave_text_color,
                        ..Default::default()
                    })
                    .into()
            } else {
                text(brave_label).size(12).color(vc.muted).into()
            };

            let mut brave_col: Vec<Element<'_, Message>> = vec![brave_btn];
            if let Some(note) = brave_note {
                brave_col.push(note);
            }

            let secondary_surface = vc.secondary_surface;
            let accent = vc.accent;
            Some(
                container(column(brave_col).spacing(4))
                    .padding(Padding::from([6u16, 12]))
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(secondary_surface)),
                        border: Border {
                            color: accent,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into(),
            )
        } else {
            None
        };

        // ── Instructions row ──
        let instructions_section_inner: Element<'_, Message> = if show_system_prompt_input {
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

            column![
                text("Instructions").size(13).color(vc.text_dim),
                prompt_input,
                action_row,
            ]
            .spacing(6)
            .into()
        } else {
            let text_dim = vc.text_dim;
            let border = vc.border;
            button(text("Instructions").size(13).color(text_dim))
                .on_press(Message::ToggleSystemPromptInput)
                .padding(Padding::from([4u16, 8]))
                .style(move |_theme, _status| button::Style {
                    background: None,
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        };

        let secondary_surface = vc.secondary_surface;
        let accent = vc.accent;
        let menu_row = container(
            row![docs_btn, tools_btn, instructions_section_inner]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            border: Border {
                color: accent,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        // Stack tools sub-panel below the menu row when expanded
        if let Some(sub_panel) = tools_sub_panel {
            column![menu_row, sub_panel].spacing(0).into()
        } else {
            menu_row.into()
        }
    } else {
        // Hidden: zero-height spacer so the layout doesn't jump
        iced::widget::Space::new().height(0).into()
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
            render_message(
                msg,
                is_last,
                is_streaming,
                streaming_content,
                edit_state,
                parsed_messages,
                theme,
                vc,
            )
        })
        .collect();

    // If actively streaming with content not yet in messages list, append streaming bubble
    let mut all_widgets: Vec<Element<'_, Message>> = message_widgets;
    if is_streaming && !streaming_content.items().is_empty() {
        all_widgets.push(render_streaming_bubble(streaming_content, theme, vc));
    }

    // Thinking indicator: show when Loading (waiting for first token)
    let is_loading = matches!(&state.busy_state, BusyState::Loading { .. });
    if is_loading {
        let loading_msg = if let BusyState::Loading { message } = &state.busy_state {
            message.as_str()
        } else {
            "Thinking…"
        };
        let muted = vc.muted;
        let secondary_surface = vc.secondary_surface;
        all_widgets.push(
            container(text(loading_msg).size(14).color(muted))
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
                .into(),
        );
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
        msg_column
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
                    let is_attached = state.current_conversation_attached_docs.contains(&doc.id);
                    let check_label = if is_attached { "[x]" } else { "[ ]" };
                    button(
                        row![
                            text(check_label).size(12).color(if is_attached {
                                accent
                            } else {
                                muted_color
                            }),
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
    let mut col_children: Vec<Element<'_, Message>> = vec![container(header_row)
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(secondary_surface)),
            ..Default::default()
        })
        .into()];
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

#[allow(clippy::too_many_arguments)]
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
            render_assistant_message(
                msg,
                is_last,
                show_streaming,
                streaming_content,
                parsed_messages,
                theme,
                vc,
            )
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

fn render_user_message<'a>(
    msg: &'a UiMessage,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
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
    let rag_indicator: Option<Element<'_, Message>> = msg.rag_context_count.and_then(|n| {
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
        let md_elem = markdown::view(items_vec, md_settings(theme))
            .map(|_uri| Message::ToggleAttestationDetail);
        if with_cursor {
            column![md_elem, text(STREAM_CURSOR).size(16)]
                .spacing(0)
                .into()
        } else {
            md_elem
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
    let error_bg = Color {
        r: destructive.r,
        g: destructive.g,
        b: destructive.b,
        a: 0.15,
    };
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
    let attachment_row: Option<Element<'_, Message>> =
        state.pending_attachment.as_ref().map(|att| {
            let filename = att.filename.clone();
            let size_display = att.size_display.clone();
            // Phase 31 IMG-06: prefix pill with [image] when attachment is an image.
            let label = if att.is_image {
                format!("[image] {} ({})", filename, size_display)
            } else {
                format!("{} ({})", filename, size_display)
            };
            row![
                text(label)
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

fn action_btn_style(
    _theme: &iced::Theme,
    _status: button::Status,
    bg: Color,
    text_color: Color,
) -> button::Style {
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
