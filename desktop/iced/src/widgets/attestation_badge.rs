use iced::widget::{button, column, container, text};
use iced::{Border, Color, Element, Length, Padding};

use mango_core::{AppState, AttestationStatus};

use crate::Message;

struct BadgeStyle {
    bg: Color,
    text_color: Color,
    border: Color,
}

fn status_style(status: &AttestationStatus, is_dark: bool) -> BadgeStyle {
    let colors = crate::theme::badge_colors(status, is_dark);
    BadgeStyle {
        bg: colors.bg,
        text_color: colors.text,
        border: colors.border,
    }
}

fn status_label(status: &AttestationStatus) -> &'static str {
    match status {
        AttestationStatus::Verified => "Verified",
        AttestationStatus::Unverified => "Not Verified",
        AttestationStatus::Expired => "Expired",
        AttestationStatus::Failed { .. } => "Failed",
    }
}

fn detail_text(status: &AttestationStatus) -> &'static str {
    match status {
        AttestationStatus::Verified => {
            "This conversation is routed to a Trusted Execution Environment. \
             The TEE attestation report has been independently verified by this app."
        }
        AttestationStatus::Unverified => {
            "Attestation has not been checked for this backend yet."
        }
        AttestationStatus::Expired => {
            "The attestation result has expired. Re-verification is pending."
        }
        AttestationStatus::Failed { .. } => {
            "Attestation verification failed. \
             The backend could not prove it is running in a trusted environment."
        }
    }
}

pub fn view<'a>(
    state: &'a AppState,
    show_detail: bool,
    is_dark: bool,
) -> Element<'a, Message> {
    // Find attestation status for the active backend
    let attest_status = state
        .active_backend_id
        .as_deref()
        .and_then(|id| state.attestation_statuses.iter().find(|e| e.backend_id == id))
        .map(|e| &e.status);

    let (label, style) = if let Some(status) = attest_status {
        (status_label(status), status_style(status, is_dark))
    } else {
        let fallback = crate::theme::badge_colors(&AttestationStatus::Unverified, is_dark);
        (
            "Not Verified",
            BadgeStyle {
                bg: fallback.bg,
                text_color: fallback.text,
                border: fallback.border,
            },
        )
    };

    let badge_text = text(label).size(12).color(style.text_color);

    let badge_container = container(badge_text)
        .padding(Padding::from([3u16, 8]))
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(style.bg)),
            border: Border {
                color: style.border,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        });

    let badge_button = button(badge_container)
        .on_press(Message::ToggleAttestationDetail)
        .padding(0)
        .style(|_theme, _status| button::Style {
            background: None,
            ..Default::default()
        });

    if show_detail {
        let detail = if let Some(status) = attest_status {
            detail_text(status)
        } else {
            "Attestation has not been checked for this backend yet."
        };

        let detail_content = column![
            text("Trust Status").size(14),
            text(detail).size(12),
        ]
        .spacing(4);

        let detail_bg = if is_dark { Color::from_rgb(0.18, 0.18, 0.18) } else { Color::from_rgb(0.95, 0.95, 0.95) };
        let detail_border = if is_dark { Color::from_rgb(0.3, 0.3, 0.3) } else { Color::from_rgb(0.8, 0.8, 0.8) };

        let detail_box = container(detail_content)
            .padding(10)
            .width(Length::Fixed(280.0))
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(detail_bg)),
                border: Border {
                    color: detail_border,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

        column![badge_button, detail_box].spacing(4).into()
    } else {
        badge_button.into()
    }
}
