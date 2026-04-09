/// Desktop PIN setup screen (Phase 28, first-time auth setup, D-14).
///
/// Shown on first launch after onboarding wizard completes, and after a duress wipe.
/// No skip option — encryption is mandatory (D-14).
/// No biometric toggle on desktop (D-23).
///
/// Steps:
/// 1. User enters a PIN (min 4 characters).
/// 2. User confirms the PIN (must match).
/// 3. Optionally sets a duress PIN (must differ from real PIN by at least 1 char, D-18).
use iced::widget::{button, center, column, container, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use mango_core::AppAction;

use crate::Message;

/// Validation error for the PIN setup form.
enum ValidationError {
    TooShort,
    NoMatch,
    DuressMatchesPin,
}

impl ValidationError {
    fn message(&self) -> &'static str {
        match self {
            ValidationError::TooShort => "PIN must be at least 4 characters.",
            ValidationError::NoMatch => "PINs do not match.",
            ValidationError::DuressMatchesPin => "Duress PIN must differ from your real PIN.",
        }
    }
}

/// Validate the PIN setup form fields.
///
/// Returns `Ok(duress_pin_option)` on success, or `Err(ValidationError)` describing the problem.
fn validate(
    pin: &str,
    confirm: &str,
    duress: &str,
) -> Result<Option<String>, ValidationError> {
    if pin.len() < 4 {
        return Err(ValidationError::TooShort);
    }
    if pin != confirm {
        return Err(ValidationError::NoMatch);
    }
    if !duress.is_empty() {
        if duress == pin {
            return Err(ValidationError::DuressMatchesPin);
        }
        return Ok(Some(duress.to_string()));
    }
    Ok(None)
}

/// Render the PIN setup screen.
pub fn view<'a>(
    pin_input: &'a str,
    confirm_input: &'a str,
    duress_input: &'a str,
    error_message: Option<&'a str>,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    let title = text("Set Your PIN")
        .size(24)
        .color(vc.text);

    let subtitle = text("Protect your conversations with a PIN.")
        .size(14)
        .color(vc.text_dim);

    let pin_label = text("PIN (min 4 characters)")
        .size(12)
        .color(vc.muted);

    let pin_field = text_input("Enter PIN", pin_input)
        .secure(true)
        .on_input(Message::PinSetupPinChanged)
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

    let confirm_label = text("Confirm PIN")
        .size(12)
        .color(vc.muted);

    let confirm_field = text_input("Confirm PIN", confirm_input)
        .secure(true)
        .on_input(Message::PinSetupConfirmChanged)
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

    let duress_label = text("Emergency PIN (optional — erases all data if entered)")
        .size(12)
        .color(vc.muted);

    let duress_hint = text("If you enter this PIN at the lock screen, all data is silently wiped.")
        .size(11)
        .color(vc.muted);

    let duress_field = text_input("Emergency PIN (optional)", duress_input)
        .secure(true)
        .on_input(Message::PinSetupDuressChanged)
        .on_submit(Message::PinSetupSubmit)
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

    // Validate inline to show the "Set PIN" button as enabled/disabled.
    let validation_result = validate(pin_input, confirm_input, duress_input);
    let can_submit = validation_result.is_ok();

    let set_btn = if can_submit {
        button(
            text("Set PIN")
                .size(14)
                .color(vc.bg)
                .align_x(Alignment::Center),
        )
        .on_press(Message::PinSetupSubmit)
        .width(Length::Fill)
        .padding(Padding::from([10u16, 0]))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.accent)),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
    } else {
        button(
            text("Set PIN")
                .size(14)
                .color(vc.muted)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding(Padding::from([10u16, 0]))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(vc.ghost_overlay)),
            border: Border {
                radius: 8.0.into(),
                color: vc.border,
                width: 1.0,
            },
            ..Default::default()
        })
    };

    // Derive inline validation error (prefer field-level validation over toast).
    let inline_err: Option<String> = match validate(pin_input, confirm_input, duress_input) {
        Err(e) if !pin_input.is_empty() || !confirm_input.is_empty() => {
            Some(e.message().to_string())
        }
        _ => error_message.map(|s| s.to_string()),
    };

    let mut content_col = column![
        title,
        subtitle,
        column![pin_label, pin_field].spacing(4),
        column![confirm_label, confirm_field].spacing(4),
        column![duress_label, duress_hint, duress_field].spacing(4),
        set_btn,
    ]
    .spacing(14)
    .align_x(Alignment::Center)
    .width(Length::Fixed(360.0));

    if let Some(err) = inline_err {
        // Use owned String so the widget doesn't borrow from a local variable.
        let err_text = text(err)
            .size(13)
            .color(vc.destructive);
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

/// Build the `AppAction::SetupPin` from the validated form fields.
///
/// Returns `None` if validation fails (should not happen if button is gated).
pub fn build_setup_pin_action(pin: &str, confirm: &str, duress: &str) -> Option<AppAction> {
    match validate(pin, confirm, duress) {
        Ok(duress_pin) => Some(AppAction::SetupPin {
            pin: pin.to_string(),
            duress_pin,
            enable_biometric: false, // Desktop: PIN-only (D-23)
        }),
        Err(_) => None,
    }
}
