use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppAction, AppState, AgentSessionSummary, AgentStepSummary};

use crate::Message;

fn status_color(status: &str, vc: &crate::theme::ViewColors) -> Color {
    match status {
        "running"   => vc.status_running,
        "paused"    => vc.status_paused,
        "completed" => vc.status_completed,
        "failed"    => vc.status_failed,
        "cancelled" => vc.status_cancelled,
        _           => vc.muted,
    }
}

fn format_elapsed(elapsed_secs: i64) -> String {
    if elapsed_secs < 60 {
        format!("{}s", elapsed_secs)
    } else {
        let m = elapsed_secs / 60;
        let s = elapsed_secs % 60;
        format!("{}m {}s", m, s)
    }
}

fn action_type_label(action_type: &str) -> &'static str {
    match action_type {
        "tool_call" => "[Tool]",
        "final_answer" => "[Answer]",
        "error" => "[Error]",
        _ => "[Step]",
    }
}

/// Agent session list view. If a session is currently loaded, shows detail view instead.
pub fn agent_list_view<'a>(
    state: &'a AppState,
    agent_task_input: &'a str,
    is_dark: bool,
) -> Element<'a, Message> {
    if state.current_agent_session_id.is_some() {
        return agent_detail_view(state, is_dark);
    }

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
    let header = container(
        row![
            back_btn,
            text("Agent Sessions").size(22),
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

    // ── Launch input ─────────────────────────────────────────────────────────
    let task_input = text_input("Describe a task for the agent...", agent_task_input)
        .on_input(Message::AgentTaskInputChanged)
        .padding(Padding::from([6u16, 10]))
        .size(14);

    let accent_color = vc.accent;
    let launch_btn = button(text("Launch").size(14))
        .on_press(Message::LaunchAgent)
        .padding(Padding::from([6u16, 14]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(accent_color)),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let border_color = vc.border;
    let launch_row = container(
        row![
            container(task_input).width(Length::Fill),
            launch_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(secondary_surface)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    });

    // ── Session list ─────────────────────────────────────────────────────────
    let muted_color = vc.muted;
    let content: Element<'_, Message> = if state.agent_sessions.is_empty() {
        container(
            column![
                text("No agent sessions yet.").size(16),
                text("Launch an agent above to get started.")
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
        let session_rows: Vec<Element<'_, Message>> = state
            .agent_sessions
            .iter()
            .map(|s| build_session_row(s, vc))
            .collect();

        let list = column(session_rows)
            .spacing(8)
            .padding(Padding::from([8u16, 16]));

        scrollable(list).height(Length::Fill).width(Length::Fill).into()
    };

    let page = column![header, launch_row, content]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    let bg_color = vc.bg;
    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..Default::default()
        })
        .into()
}

fn build_session_row<'a>(session: &'a AgentSessionSummary, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let status_col = status_color(&session.status, &vc);
    let status_badge = container(
        text(&session.status).size(11).color(status_col),
    )
    .padding(Padding::from([2u16, 6]))
    .style(move |_theme| container::Style {
        background: Some(Background::Color(Color { r: status_col.r, g: status_col.g, b: status_col.b, a: 0.15 })),
        border: Border {
            color: status_col,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });

    let elapsed_text = format_elapsed(session.elapsed_secs);
    let steps_text = format!("{} steps", session.step_count);

    let muted = vc.muted;
    let meta_row = row![
        status_badge,
        text(steps_text).size(12).color(muted),
        text(" · ").size(12).color(muted),
        text(elapsed_text).size(12).color(muted),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let info_col = column![
        text(&session.title).size(15),
        meta_row,
    ]
    .spacing(4);

    let session_id = session.id.clone();
    let secondary_surface = vc.secondary_surface;
    button(
        container(info_col)
            .padding(Padding::from([10u16, 14]))
            .width(Length::Fill),
    )
    .on_press(Message::DispatchAction(AppAction::LoadAgentSession {
        session_id,
    }))
    .padding(0)
    .width(Length::Fill)
    .style(move |_theme, _status| button::Style {
        background: Some(Background::Color(secondary_surface)),
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Agent session detail view: shown when current_agent_session_id is Some.
pub fn agent_detail_view(state: &AppState, is_dark: bool) -> Element<'_, Message> {
    let vc = crate::theme::view_colors(is_dark);

    // Find the current session summary
    let current_session = state
        .current_agent_session_id
        .as_deref()
        .and_then(|id| state.agent_sessions.iter().find(|s| s.id == id));

    // ── Header with back button ───────────────────────────────────────────────
    let surface_color = vc.surface;
    let back_btn = button(text("Back").size(14))
        // Uses the dedicated ClearAgentDetail action (not empty-string LoadAgentSession)
        .on_press(Message::DispatchAction(AppAction::ClearAgentDetail))
        .padding(Padding::from([4u16, 10]))
        .style(move |_theme, _status| button::Style {
            background: Some(Background::Color(surface_color)),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let (title_str, status_str) = if let Some(s) = current_session {
        (s.title.as_str(), s.status.as_str())
    } else {
        ("Agent Session", "unknown")
    };

    let status_col = status_color(status_str, &vc);
    let status_badge = container(
        text(status_str).size(12).color(status_col),
    )
    .padding(Padding::from([2u16, 8]))
    .style(move |_theme| container::Style {
        background: Some(Background::Color(Color { r: status_col.r, g: status_col.g, b: status_col.b, a: 0.15 })),
        border: Border {
            color: status_col,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    });

    let secondary_surface = vc.secondary_surface;
    let header = container(
        row![
            back_btn,
            text(title_str).size(18),
            status_badge,
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

    // ── Action buttons row ────────────────────────────────────────────────────
    let mut action_btns: Vec<Element<'_, Message>> = vec![];

    if let Some(session) = current_session {
        let session_id = session.id.clone();

        if session.status == "running" {
            let sid = session_id.clone();
            let status_paused = vc.status_paused;
            let warning_bg = Color { r: vc.warning.r, g: vc.warning.g, b: vc.warning.b, a: 0.15 };
            action_btns.push(
                button(text("Pause").size(13))
                    .on_press(Message::DispatchAction(AppAction::PauseAgentSession {
                        session_id: sid,
                    }))
                    .padding(Padding::from([6u16, 14]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(warning_bg)),
                        border: Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        text_color: status_paused,
                        ..Default::default()
                    })
                    .into(),
            );
        }

        if session.status == "paused" {
            let sid = session_id.clone();
            let status_running = vc.status_running;
            let success_bg = Color { r: vc.success.r, g: vc.success.g, b: vc.success.b, a: 0.15 };
            action_btns.push(
                button(text("Resume").size(13))
                    .on_press(Message::DispatchAction(AppAction::ResumeAgentSession {
                        session_id: sid,
                    }))
                    .padding(Padding::from([6u16, 14]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(success_bg)),
                        border: Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        text_color: status_running,
                        ..Default::default()
                    })
                    .into(),
            );
        }

        if session.status == "running" || session.status == "paused" {
            let destructive = vc.destructive;
            let destructive_bg = Color { r: destructive.r, g: destructive.g, b: destructive.b, a: 0.15 };
            action_btns.push(
                button(text("Cancel").size(13))
                    .on_press(Message::DispatchAction(AppAction::CancelAgentSession {
                        session_id,
                    }))
                    .padding(Padding::from([6u16, 14]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(destructive_bg)),
                        border: Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        text_color: destructive,
                        ..Default::default()
                    })
                    .into(),
            );
        }
    }

    let card_color = vc.card;
    let action_section: Element<'_, Message> = if action_btns.is_empty() {
        container(iced::widget::Space::new().width(Length::Fill))
            .height(Length::Fixed(0.0))
            .into()
    } else {
        container(
            row(action_btns).spacing(8),
        )
        .padding(Padding::from([8u16, 16]))
        .width(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(card_color)),
            ..Default::default()
        })
        .into()
    };

    // ── Step list ─────────────────────────────────────────────────────────────
    let muted_color = vc.muted;
    let step_content: Element<'_, Message> = if state.current_agent_steps.is_empty() {
        container(
            column![
                text("No steps yet.").size(15).color(muted_color),
                text("Steps will appear here as the agent works.")
                    .size(13)
                    .color(muted_color),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .padding(32)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
    } else {
        let step_rows: Vec<Element<'_, Message>> = state
            .current_agent_steps
            .iter()
            .map(|step| build_step_row(step, vc))
            .collect();

        let list = column(step_rows)
            .spacing(6)
            .padding(Padding::from([8u16, 16]));

        scrollable(list).height(Length::Fill).width(Length::Fill).into()
    };

    let page = column![header, action_section, step_content]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    let bg_color = vc.bg;
    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(bg_color)),
            ..Default::default()
        })
        .into()
}

fn build_step_row<'a>(step: &'a AgentStepSummary, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let step_label = format!("#{}", step.step_number);
    let type_label = action_type_label(&step.action_type);

    let muted = vc.muted;
    let surface = vc.surface;
    let step_num = container(text(step_label).size(12).color(muted))
        .padding(Padding::from([2u16, 6]))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(surface)),
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let accent = vc.accent;
    let accent_dim = vc.accent_dim;
    let type_badge = container(text(type_label).size(11).color(accent))
        .padding(Padding::from([2u16, 6]))
        .style(move |_theme| container::Style {
            background: Some(Background::Color(accent_dim)),
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let text_dim = vc.text_dim;
    let mut header_row = row![step_num, type_badge].spacing(6).align_y(Alignment::Center);

    if let Some(tool_name) = &step.tool_name {
        header_row = header_row.push(
            text(tool_name).size(13).color(text_dim),
        );
    }

    let destructive = vc.destructive;
    let status_indicator = if step.status == "failed" {
        text("FAILED").size(11).color(destructive)
    } else {
        text("ok").size(11).color(muted)
    };

    let header_with_status = row![
        container(header_row).width(Length::Fill),
        status_indicator,
    ]
    .align_y(Alignment::Center);

    let mut col_children: Vec<Element<'_, Message>> = vec![header_with_status.into()];

    if let Some(snippet) = &step.result_snippet {
        if !snippet.is_empty() {
            // Truncate to 200 chars (already done server-side, but cap here too)
            let display = if snippet.len() > 200 {
                format!("{}...", &snippet[..197])
            } else {
                snippet.clone()
            };
            col_children.push(
                text(display).size(12).color(muted).into(),
            );
        }
    }

    let secondary_surface = vc.secondary_surface;
    container(
        column(col_children).spacing(4),
    )
    .padding(Padding::from([8u16, 12]))
    .width(Length::Fill)
    .style(move |_theme| container::Style {
        background: Some(Background::Color(secondary_surface)),
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}
