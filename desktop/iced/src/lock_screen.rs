/// Desktop lock screen (Phase 28, D-23).
///
/// PIN-only on desktop — no biometric button (D-23). On macOS the keyring crate
/// may trigger Touch ID transparently when reading the keychain entry for the DEK;
/// no explicit biometric UI is needed.
///
/// Security: PIN input is masked (is_secure). Input is cleared after submit (T-28-23).
use iced::widget::{button, center, column, container, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use crate::Message;

/// Render the lock screen.
///
/// Shows a centered column: app name, PIN input, Unlock button, and an optional
/// error message (taken from AppState.toast when unlock fails).
pub fn view<'a>(
    pin_input: &'a str,
    error_message: Option<&'a str>,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    let logo = text("Mango").size(32).color(vc.text);

    let tagline = text("Enter your PIN to unlock").size(14).color(vc.text_dim);

    // Secure text input — PIN is masked (T-28-23).
    let pin_field = text_input("PIN", pin_input)
        .secure(true)
        .on_input(Message::UnlockPinChanged)
        .on_submit(Message::UnlockSubmit)
        .padding(Padding::from([10u16, 14]))
        .size(15)
        .style(move |_, _| text_input::Style {
            background: Background::Color(vc.surface),
            border: Border {
                color: vc.border,
                width: 1.0,
                radius: 8.0.into(),
            },
            icon: vc.muted,
            placeholder: vc.muted,
            value: vc.text,
            selection: vc.accent_dim,
        });

    let unlock_btn = button(
        text("Unlock")
            .size(14)
            .color(vc.bg)
            .align_x(Alignment::Center),
    )
    .on_press(Message::UnlockSubmit)
    .width(Length::Fill)
    .padding(Padding::from([10u16, 0]))
    .style(move |_, _| button::Style {
        background: Some(Background::Color(vc.accent)),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let mut content_col = column![logo, tagline, pin_field, unlock_btn]
        .spacing(14)
        .align_x(Alignment::Center)
        .width(Length::Fixed(320.0));

    // Show error text if unlock failed (e.g. wrong PIN).
    if let Some(err) = error_message {
        let err_text = text(err).size(13).color(vc.destructive);
        content_col = content_col.push(err_text);
    }

    let card = container(content_col)
        .padding(Padding::from([36u16, 40]))
        .style(move |_| container::Style {
            background: Some(Background::Color(vc.surface)),
            border: Border {
                color: vc.border,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        });

    center(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(vc.bg)),
            ..Default::default()
        })
        .into()
}
