use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding, Shadow, Vector};

use mango_core::{AppAction, AppState};

use crate::Message;

fn action_btn<'a>(
    label: &'a str,
    msg: Message,
    enabled: bool,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let (bg_color, muted, border) = (vc.bg, vc.muted, vc.border);
    let (accent_color, accent_text) = (vc.accent, bg_color);
    if enabled {
        button(text(label).size(13).color(accent_text))
            .on_press(msg)
            .padding(Padding::from([6u16, 16]))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(accent_color)),
                border: Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        button(text(label).size(13).color(muted))
            .padding(Padding::from([6u16, 16]))
            .style(move |_, _| button::Style {
                background: Some(Background::Color(vc.ghost_overlay)),
                border: Border {
                    radius: 6.0.into(),
                    color: border,
                    width: 1.0,
                },
                ..Default::default()
            })
            .into()
    }
}

fn section_header<'a>(label: &'a str, muted: iced::Color) -> Element<'a, Message> {
    container(text(label).size(11).color(muted))
        .padding(Padding {
            top: 20.0,
            bottom: 6.0,
            left: 16.0,
            right: 16.0,
        })
        .into()
}

pub fn view<'a>(
    state: &'a AppState,
    is_dark: bool,
    default_model_input: &'a str,
    default_instructions: &'a str,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    // ── Header ────────────────────────────────────────────────────────────────
    let header = container(
        row![
            button(text("< Settings").size(13).color(vc.text_dim))
                .on_press(Message::DispatchAction(AppAction::PopScreen))
                .padding(Padding::from([4u16, 10]))
                .style(move |_, _| button::Style {
                    background: Some(Background::Color(vc.ghost_overlay)),
                    border: Border {
                        radius: 5.0.into(),
                        color: vc.border,
                        width: 1.0
                    },
                    ..Default::default()
                }),
            text("Defaults").size(17).color(vc.text),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([10u16, 16]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(vc.surface)),
        border: Border {
            color: vc.border,
            width: 0.0,
            ..Default::default()
        },
        shadow: Shadow {
            color: vc.shadow,
            blur_radius: 8.0,
            offset: Vector::new(0.0, 2.0),
        },
        ..Default::default()
    });

    // ── Default Model ─────────────────────────────────────────────────────────
    let all_models: Vec<String> = {
        let mut m: Vec<String> = state
            .backends
            .iter()
            .flat_map(|b| b.models.iter().cloned())
            .collect();
        m.dedup();
        m
    };

    let model_picker_el: Element<'_, Message> = if all_models.is_empty() {
        container(
            text("Enable a provider to select a default model.")
                .size(13)
                .color(vc.muted),
        )
        .padding(Padding::from([4u16, 0]))
        .into()
    } else {
        let selected = if all_models.contains(&default_model_input.to_string()) {
            Some(default_model_input.to_string())
        } else {
            None
        };
        let picker = pick_list(all_models, selected, Message::SettingsDefaultModelChanged)
            .placeholder("Select default model")
            .text_size(14);

        let save_el: Element<'_, Message> = if !default_model_input.is_empty() {
            action_btn(
                "Save",
                Message::DispatchAction(AppAction::SetDefaultModel {
                    model_id: default_model_input.to_string(),
                }),
                true,
                vc,
            )
        } else {
            iced::widget::Space::new().into()
        };

        container(row![picker, save_el].spacing(8).align_y(Alignment::Center))
            .padding(Padding::from([4u16, 0]))
            .width(Length::Fill)
            .into()
    };

    // ── Default Instructions ──────────────────────────────────────────────────
    let instructions_input =
        text_input("e.g. You are a helpful assistant...", default_instructions)
            .on_input(Message::SettingsDefaultInstructionsChanged)
            .size(13)
            .padding(Padding::from([7u16, 10]));

    let instructions_save = action_btn("Save", Message::SettingsSaveDefaultInstructions, true, vc);

    let instructions_block = column![
        text("Default Instructions").size(13).color(vc.text),
        text("Fallback system prompt for conversations without custom instructions.")
            .size(11)
            .color(vc.muted),
        row![instructions_input, instructions_save]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(6);

    let defaults_content: Element<'_, Message> = container(
        column![
            column![
                text("Default Model").size(13).color(vc.text),
                model_picker_el,
            ]
            .spacing(4),
            instructions_block,
        ]
        .spacing(14),
    )
    .padding(Padding::from([4u16, 16]))
    .width(Length::Fill)
    .into();

    // ── Compose ───────────────────────────────────────────────────────────────
    let content = column![
        section_header("DEFAULTS", vc.muted),
        defaults_content,
        iced::widget::Space::new().height(24),
    ]
    .spacing(0);

    let page = column![
        header,
        scrollable(content).height(Length::Fill).width(Length::Fill),
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill);

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(vc.bg)),
            ..Default::default()
        })
        .into()
}
