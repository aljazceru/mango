use iced::widget::{button, center, column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding};

use mango_core::{AppState, AttestationStatus, OnboardingStep, known_provider_presets}; // AppAction and Screen used via fully-qualified path below

use crate::Message;

// Progress dot indicator: 4 dots, current step highlighted
fn progress_dots(current_step: &OnboardingStep, accent: Color, muted: Color) -> Element<'static, Message> {
    let step_idx = match current_step {
        OnboardingStep::Welcome => 0,
        OnboardingStep::BackendSetup => 1,
        OnboardingStep::AttestationDemo => 2,
        OnboardingStep::ReadyToChat => 3,
    };

    let dots: Vec<Element<'_, Message>> = (0..4)
        .map(|i| {
            let (size, color) = if i == step_idx {
                (12.0_f32, accent)
            } else if i < step_idx {
                (10.0_f32, muted)
            } else {
                (10.0_f32, Color { r: muted.r, g: muted.g, b: muted.b, a: 0.5 })
            };

            container(iced::widget::Space::new().width(size).height(size))
                .style(move |_theme| container::Style {
                    background: Some(Background::Color(color)),
                    border: Border {
                        radius: (size / 2.0).into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        })
        .collect();

    row(dots).spacing(8).align_y(Alignment::Center).into()
}

/// Main onboarding view entry point.
pub fn view<'a>(
    state: &'a AppState,
    step: &'a OnboardingStep,
    selected_preset: &'a str,
    api_key: &'a str,
    show_learn_more: bool,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);
    let dots = progress_dots(step, vc.accent, vc.muted);

    let step_content: Element<'_, Message> = match step {
        OnboardingStep::Welcome => welcome_step(vc),
        OnboardingStep::BackendSetup => backend_setup_step(state, selected_preset, api_key, vc),
        OnboardingStep::AttestationDemo => attestation_demo_step(state, show_learn_more, vc),
        OnboardingStep::ReadyToChat => ready_to_chat_step(vc),
    };

    let dots_row = container(
        row![dots].align_y(Alignment::Center),
    )
    .padding(Padding { top: 0.0, bottom: 16.0, left: 0.0, right: 0.0 })
    .width(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center);

    let card = container(
        column![dots_row, step_content]
            .spacing(0)
            .width(Length::Fill),
    )
    .max_width(560.0)
    .padding(Padding::from([32u16, 40]))
    .style(move |_theme| container::Style {
        background: Some(Background::Color(vc.secondary_surface)),
        border: Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let page = center(card).width(Length::Fill).height(Length::Fill);

    container(page)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(vc.surface)),
            ..Default::default()
        })
        .into()
}

// ── Step 1: Welcome ───────────────────────────────────────────────────────────

fn welcome_step<'a>(vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let title = text("Mango")
        .size(32)
        .color(vc.text);

    let tagline = text("Your conversations, provably private.")
        .size(18)
        .color(vc.accent);

    let description = text(
        "Every message is processed inside a Trusted Execution Environment \
         -- a sealed hardware enclave that no one can access, not even the server operator.",
    )
    .size(15)
    .color(vc.muted);

    let accent_color = vc.accent;
    let text_color = vc.bg;
    let get_started_btn = button(
        text("Get Started").size(16).color(text_color),
    )
    .on_press(Message::OnboardingNext)
    .padding(Padding::from([10u16, 32]))
    .style(move |_theme, _status| button::Style {
        background: Some(Background::Color(accent_color)),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let muted_color = vc.muted;
    let skip_btn = button(
        text("Skip setup").size(13).color(muted_color),
    )
    .on_press(Message::OnboardingSkip)
    .padding(Padding::from([4u16, 0]))
    .style(|_theme, _status| button::Style {
        background: None,
        ..Default::default()
    });

    column![
        title,
        tagline,
        description,
        container(get_started_btn)
            .padding(Padding { top: 16.0, bottom: 0.0, left: 0.0, right: 0.0 })
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        container(skip_btn)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    ]
    .spacing(12)
    .align_x(Alignment::Center)
    .into()
}

// ── Step 2: Backend Setup ─────────────────────────────────────────────────────

fn backend_setup_step<'a>(
    _state: &'a AppState,
    selected_preset: &'a str,
    api_key: &'a str,
    vc: crate::theme::ViewColors,
) -> Element<'a, Message> {
    let heading = text("Choose your provider").size(24).color(vc.text);

    let subtitle = text(
        "Select a confidential inference provider and enter your API key.",
    )
    .size(14)
    .color(vc.muted);

    // Collect preset data as owned values to avoid borrowing across closures.
    struct PresetData {
        id: String,
        name: String,
        description: String,
        tee_type: mango_core::TeeType,
    }
    let preset_data: Vec<PresetData> = known_provider_presets().into_iter().map(|p| PresetData {
        id: p.id,
        name: p.name,
        description: p.description,
        tee_type: p.tee_type,
    }).collect();

    let accent_color = vc.accent;
    let muted_color = vc.muted;
    let surface_color = vc.surface;
    let card_enabled_color = vc.card_enabled;
    let border_color = vc.border;
    // Preset selection rows
    let preset_rows: Vec<Element<'_, Message>> = preset_data
        .into_iter()
        .map(|preset| {
            let is_selected = preset.id == selected_preset;
            let bg_color = if is_selected {
                card_enabled_color
            } else {
                surface_color
            };
            let row_border_color = if is_selected {
                accent_color
            } else {
                border_color
            };
            let tee_str = tee_short_label(&preset.tee_type);

            let row_content = row![
                column![
                    text(preset.name).size(15).color(vc.text),
                    text(preset.description)
                        .size(12)
                        .color(muted_color),
                ]
                .spacing(2),
                iced::widget::Space::new().width(Length::Fill),
                text(tee_str)
                    .size(12)
                    .color(if is_selected { accent_color } else { muted_color }),
            ]
            .align_y(Alignment::Center)
            .spacing(8);

            let preset_id = preset.id;
            button(row_content)
                .on_press(Message::OnboardingSelectBackend(preset_id))
                .padding(Padding::from([10u16, 14]))
                .width(Length::Fill)
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        radius: 6.0.into(),
                        color: row_border_color,
                        width: if is_selected { 1.5 } else { 1.0 },
                    },
                    ..Default::default()
                })
                .into()
        })
        .collect();

    let presets_list = column(preset_rows).spacing(6);

    let api_key_label = text("API Key").size(13).color(muted_color);
    let api_key_input = text_input("Enter your API key...", api_key)
        .secure(true)
        .on_input(Message::OnboardingApiKeyChanged)
        .size(14)
        .padding(Padding::from([8u16, 10]));

    // Validation state
    let validate_area: Element<'_, Message> = if _state.onboarding.validating_api_key {
        row![
            text("Validating...").size(14).color(muted_color),
        ]
        .into()
    } else {
        let can_validate = !selected_preset.is_empty() && !api_key.trim().is_empty();
        let validate_btn: Element<'_, Message> = if can_validate {
            button(text("Validate & Continue").size(14).color(vc.bg))
                .on_press(Message::OnboardingValidateKey)
                .padding(Padding::from([8u16, 20]))
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(accent_color)),
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        } else {
            button(text("Validate & Continue").size(14).color(muted_color))
                .padding(Padding::from([8u16, 20]))
                .style(move |_theme, _status| button::Style {
                    background: Some(Background::Color(surface_color)),
                    border: Border {
                        radius: 6.0.into(),
                        color: border_color,
                        width: 1.0,
                    },
                    ..Default::default()
                })
                .into()
        };
        validate_btn
    };

    let error_area: Element<'_, Message> = if let Some(err) = &_state.onboarding.api_key_error {
        text(err.as_str())
            .size(13)
            .color(vc.destructive)
            .into()
    } else {
        iced::widget::Space::new().into()
    };

    let back_btn = button(text("Back").size(13).color(muted_color))
        .on_press(Message::OnboardingBack)
        .padding(Padding::from([4u16, 10]))
        .style(|_theme, _status| button::Style {
            background: None,
            ..Default::default()
        });

    let skip_btn = button(text("Skip for now").size(13).color(muted_color))
        .on_press(Message::OnboardingSkip)
        .padding(Padding::from([4u16, 10]))
        .style(|_theme, _status| button::Style {
            background: None,
            ..Default::default()
        });

    let nav_row = row![back_btn, iced::widget::Space::new().width(Length::Fill), skip_btn]
        .align_y(Alignment::Center);

    column![
        heading,
        subtitle,
        presets_list,
        column![api_key_label, api_key_input].spacing(4),
        error_area,
        validate_area,
        nav_row,
    ]
    .spacing(10)
    .into()
}

// ── Step 3: Attestation Demo ──────────────────────────────────────────────────

fn attestation_demo_step<'a>(state: &'a AppState, show_learn_more: bool, vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let heading = text("Verifying your backend").size(24).color(vc.text);

    let accent_color = vc.accent;
    let muted_color = vc.muted;
    // Stage indicator: show animated progress if attestation stage is set
    let stage_area: Element<'_, Message> = if let Some(stage) = &state.onboarding.attestation_stage {
        row![
            text(stage.as_str()).size(14).color(accent_color),
            text("...").size(14).color(muted_color),
        ]
        .spacing(4)
        .into()
    } else {
        iced::widget::Space::new().into()
    };

    // Result area
    let result_area: Element<'_, Message> = if let Some(result) = &state.onboarding.attestation_result {
        match result {
            AttestationStatus::Verified => {
                let tee_label = state.onboarding.attestation_tee_label
                    .as_deref()
                    .unwrap_or("TEE Verified");

                let shield_row = row![
                    text("VERIFIED").size(14).color(vc.success),
                    text(format!(" -- {}", tee_label)).size(14).color(muted_color),
                ]
                .spacing(4)
                .align_y(Alignment::Center);

                let vault_metaphor = text(
                    "Think of a Trusted Execution Environment like a sealed, tamper-proof vault \
                     inside the server. Your data goes in, the AI processes it, and the result \
                     comes out -- but nobody (not even the server operator) can see what's inside. \
                     Attestation is the cryptographic proof that the vault is real and hasn't \
                     been tampered with.",
                )
                .size(14)
                .color(muted_color);

                // Learn More expandable section
                let learn_more_toggle = button(
                    text(if show_learn_more { "Learn More (hide)" } else { "Learn More" })
                        .size(13)
                        .color(accent_color),
                )
                .on_press(Message::OnboardingToggleLearnMore)
                .padding(Padding::from([2u16, 0]))
                .style(|_theme, _status| button::Style {
                    background: None,
                    ..Default::default()
                });

                let surface_color = vc.surface;
                let learn_more_content: Element<'_, Message> = if show_learn_more {
                    let learn_text = text(
                        "What is a TEE?\n\
                         A Trusted Execution Environment is a hardware-isolated region of a processor. \
                         Code and data inside the TEE cannot be read or modified by the operating system, \
                         hypervisor, or server administrator.\n\n\
                         What does attestation prove?\n\
                         Attestation is a cryptographic certificate from the hardware itself. It proves: \
                         (1) the TEE is genuine hardware, not a simulation, \
                         (2) the software running inside hasn't been tampered with, \
                         (3) your data is being processed in the secure enclave right now.\n\n\
                         Self-verified vs Provider-verified:\n\
                         Self-verified means this app checked the cryptographic proof directly. \
                         Provider-verified means the backend's own attestation service confirmed the TEE. \
                         Both guarantee your data is protected."
                    )
                    .size(13)
                    .color(muted_color);

                    container(learn_text)
                        .padding(Padding::from([10u16, 14]))
                        .width(Length::Fill)
                        .style(move |_theme| container::Style {
                            background: Some(Background::Color(surface_color)),
                            border: Border {
                                radius: 6.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into()
                } else {
                    iced::widget::Space::new().into()
                };

                let bg_color = vc.bg;
                let next_btn = button(text("Next").size(14).color(bg_color))
                    .on_press(Message::OnboardingNext)
                    .padding(Padding::from([8u16, 24]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(accent_color)),
                        border: Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    });

                let back_btn = button(text("Back").size(13).color(muted_color))
                    .on_press(Message::OnboardingBack)
                    .padding(Padding::from([4u16, 10]))
                    .style(|_theme, _status| button::Style {
                        background: None,
                        ..Default::default()
                    });

                column![
                    shield_row,
                    vault_metaphor,
                    learn_more_toggle,
                    learn_more_content,
                    next_btn,
                    back_btn,
                ]
                .spacing(10)
                .into()
            }
            AttestationStatus::Failed { reason } => {
                let error_text = text(format!("Verification failed: {}", reason))
                    .size(14)
                    .color(vc.destructive);

                let destructive = vc.destructive;
                let retry_btn = button(text("Retry").size(14).color(vc.bg))
                    .on_press(Message::OnboardingRetryAttestation)
                    .padding(Padding::from([8u16, 20]))
                    .style(move |_theme, _status| button::Style {
                        background: Some(Background::Color(Color { r: destructive.r, g: destructive.g, b: destructive.b, a: 0.2 })),
                        border: Border {
                            radius: 6.0.into(),
                            color: destructive,
                            width: 1.0,
                        },
                        ..Default::default()
                    });

                let continue_anyway = button(
                    text("Continue anyway").size(13).color(muted_color),
                )
                .on_press(Message::OnboardingNext)
                .padding(Padding::from([4u16, 0]))
                .style(|_theme, _status| button::Style {
                    background: None,
                    ..Default::default()
                });

                let back_btn = button(text("Back").size(13).color(muted_color))
                    .on_press(Message::OnboardingBack)
                    .padding(Padding::from([4u16, 10]))
                    .style(|_theme, _status| button::Style {
                        background: None,
                        ..Default::default()
                    });

                column![
                    error_text,
                    retry_btn,
                    continue_anyway,
                    back_btn,
                ]
                .spacing(8)
                .into()
            }
            _ => {
                // Unverified / Expired -- treat as pending
                let waiting_text = text("Waiting for attestation result...")
                    .size(14)
                    .color(muted_color);

                let back_btn = button(text("Back").size(13).color(muted_color))
                    .on_press(Message::OnboardingBack)
                    .padding(Padding::from([4u16, 10]))
                    .style(|_theme, _status| button::Style {
                        background: None,
                        ..Default::default()
                    });

                column![waiting_text, back_btn]
                    .spacing(8)
                    .into()
            }
        }
    } else {
        // No result yet -- show waiting / stage indicator
        let waiting_text = text("Starting verification...")
            .size(14)
            .color(muted_color);

        let back_btn = button(text("Back").size(13).color(muted_color))
            .on_press(Message::OnboardingBack)
            .padding(Padding::from([4u16, 10]))
            .style(|_theme, _status| button::Style {
                background: None,
                ..Default::default()
            });

        column![waiting_text, back_btn]
            .spacing(8)
            .into()
    };

    column![
        heading,
        stage_area,
        result_area,
    ]
    .spacing(10)
    .into()
}

// ── Step 4: Ready to Chat ─────────────────────────────────────────────────────

fn ready_to_chat_step<'a>(vc: crate::theme::ViewColors) -> Element<'a, Message> {
    let heading = text("You're ready!").size(28).color(vc.text);

    let body = text("Your backend is verified and ready. Start a confidential conversation.")
        .size(15)
        .color(vc.muted);

    let accent_color = vc.accent;
    let bg_color = vc.bg;
    let start_btn = button(
        text("Start Chatting").size(16).color(bg_color),
    )
    .on_press(Message::OnboardingComplete)
    .padding(Padding::from([12u16, 40]))
    .style(move |_theme, _status| button::Style {
        background: Some(Background::Color(accent_color)),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let muted_color = vc.muted;
    let back_btn = button(text("Back").size(13).color(muted_color))
        .on_press(Message::OnboardingBack)
        .padding(Padding::from([4u16, 10]))
        .style(|_theme, _status| button::Style {
            background: None,
            ..Default::default()
        });

    column![
        heading,
        body,
        container(start_btn)
            .padding(Padding { top: 16.0, bottom: 0.0, left: 0.0, right: 0.0 })
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        back_btn,
    ]
    .spacing(8)
    .align_x(Alignment::Center)
    .into()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn tee_short_label(tee: &mango_core::TeeType) -> &'static str {
    match tee {
        mango_core::TeeType::IntelTdx => "Intel TDX",
        mango_core::TeeType::NvidiaH100Cc => "NVIDIA H100 CC",
        mango_core::TeeType::AmdSevSnp => "AMD SEV-SNP",
        mango_core::TeeType::Unknown => "Unknown TEE",
    }
}
