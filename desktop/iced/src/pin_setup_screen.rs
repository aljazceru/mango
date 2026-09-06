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
///
/// Resume mode (when `enrollment_resume_pending` is true): an interrupted
/// enrollment has staged pending auth. The user must re-enter the PIN they
/// chose earlier; the duress/biometric choices were already captured. The
/// confirmation and duress fields are hidden and a single PIN field is shown.
use iced::widget::{button, center, column, container, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding};

use mango_core::AppAction;

use crate::Message;

/// Validation error for the PIN setup form.
#[derive(Debug, PartialEq)]
enum ValidationError {
    TooShort,
    NoMatch,
    DuressMatchesPin,
    DuressTooShort,
    ResumeEmpty,
}

impl ValidationError {
    fn message(&self) -> &'static str {
        match self {
            ValidationError::TooShort => "PIN must be at least 4 characters.",
            ValidationError::NoMatch => "PINs do not match.",
            ValidationError::DuressMatchesPin => "Duress PIN must differ from your real PIN.",
            ValidationError::DuressTooShort => "Emergency PIN must be at least 4 characters.",
            ValidationError::ResumeEmpty => "Enter the PIN you chose earlier.",
        }
    }
}

/// Validate the PIN setup form fields.
///
/// In resume mode only an empty PIN is rejected, so the previously chosen
/// (possibly legacy-length) passphrase can finish the interrupted enrollment.
///
/// In fresh mode the main PIN must be at least 4 characters and must match the
/// confirmation; an optional duress PIN must differ and also be at least 4
/// characters.
fn validate(
    pin: &str,
    confirm: &str,
    duress: &str,
    is_resume: bool,
) -> Result<Option<String>, ValidationError> {
    if is_resume {
        if pin.trim().is_empty() {
            return Err(ValidationError::ResumeEmpty);
        }
        return Ok(None);
    }

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
        if duress.len() < 4 {
            return Err(ValidationError::DuressTooShort);
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
    is_resume: bool,
    is_dark: bool,
) -> Element<'a, Message> {
    let vc = crate::theme::view_colors(is_dark);

    let (title, subtitle, pin_label, button_label) = if is_resume {
        (
            text("Finish Encryption Setup").size(24).color(vc.text),
            text("Setup was interrupted before encryption finished. Enter the PIN you chose earlier to finish protecting your data.")
                .size(14)
                .color(vc.text_dim),
            text("PIN").size(12).color(vc.muted),
            "Resume Setup",
        )
    } else {
        (
            text("Set Your PIN").size(24).color(vc.text),
            text("Protect your conversations with a PIN.")
                .size(14)
                .color(vc.text_dim),
            text("PIN (min 4 characters)").size(12).color(vc.muted),
            "Set PIN",
        )
    };

    let pin_field = text_input("Enter PIN", pin_input)
        .secure(true)
        .on_input(Message::PinSetupPinChanged)
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

    // In resume mode the confirmation and duress fields are hidden — those
    // choices were already captured when the enrollment was first staged.
    let maybe_confirm = if is_resume {
        None
    } else {
        let confirm_label = text("Confirm PIN").size(12).color(vc.muted);
        let confirm_field = text_input("Confirm PIN", confirm_input)
            .secure(true)
            .on_input(Message::PinSetupConfirmChanged)
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
        Some(column![confirm_label, confirm_field].spacing(4))
    };

    let maybe_duress = if is_resume {
        None
    } else {
        let duress_label = text("Emergency PIN (optional — erases all data if entered)")
            .size(12)
            .color(vc.muted);

        let duress_hint =
            text("If you enter this PIN at the lock screen, all data is silently wiped.")
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
        Some(column![duress_label, duress_hint, duress_field].spacing(4))
    };

    // Validate inline to show the action button as enabled/disabled.
    let validation_result = validate(pin_input, confirm_input, duress_input, is_resume);
    let can_submit = validation_result.is_ok();

    let action_btn = if can_submit {
        button(
            text(button_label)
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
            text(button_label)
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

    // Derive inline validation error. In resume mode prefer the local
    // "Enter the PIN" message if the field is empty, otherwise show any core
    // error (e.g. "Incorrect PIN.") as the primary failure text. In fresh mode,
    // prefer field-level validation over core toasts.
    let inline_err: Option<String> =
        match validate(pin_input, confirm_input, duress_input, is_resume) {
            Err(e) if !pin_input.is_empty() || is_resume => Some(e.message().to_string()),
            Ok(_) => error_message.map(|s| s.to_string()),
            _ => error_message.map(|s| s.to_string()),
        };

    let mut content_col = column![title, subtitle, column![pin_label, pin_field].spacing(4)]
        .spacing(14)
        .align_x(Alignment::Center)
        .width(Length::Fixed(360.0));

    if let Some(confirm_col) = maybe_confirm {
        content_col = content_col.push(confirm_col);
    }
    if let Some(duress_col) = maybe_duress {
        content_col = content_col.push(duress_col);
    }
    content_col = content_col.push(action_btn);

    if let Some(err) = inline_err {
        // Use owned String so the widget doesn't borrow from a local variable.
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

/// Build the `AppAction::SetupPin` from the validated form fields.
///
/// Returns `None` if validation fails (should not happen if button is gated).
pub fn build_setup_pin_action(
    pin: &str,
    confirm: &str,
    duress: &str,
    is_resume: bool,
) -> Option<AppAction> {
    match validate(pin, confirm, duress, is_resume) {
        Ok(duress_pin) => Some(AppAction::SetupPin {
            pin: pin.to_string(),
            duress_pin,
            enable_biometric: false, // Desktop: PIN-only (D-23)
        }),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_validation_requires_four_chars_and_match() {
        assert!(matches!(
            validate("123", "123", "", false),
            Err(ValidationError::TooShort)
        ));
        assert!(matches!(
            validate("1234", "1235", "", false),
            Err(ValidationError::NoMatch)
        ));
        assert!(matches!(
            validate("1234", "1234", "1234", false),
            Err(ValidationError::DuressMatchesPin)
        ));
        assert!(matches!(
            validate("1234", "1234", "12", false),
            Err(ValidationError::DuressTooShort)
        ));
        assert_eq!(validate("1234", "1234", "", false), Ok(None));
        assert_eq!(
            validate("1234", "1234", "9876", false),
            Ok(Some("9876".into()))
        );
    }

    #[test]
    fn resume_validation_only_requires_non_empty_pin() {
        assert!(matches!(
            validate("", "", "", true),
            Err(ValidationError::ResumeEmpty)
        ));
        assert!(matches!(
            validate("   ", "", "", true),
            Err(ValidationError::ResumeEmpty)
        ));
        // Legacy short PIN must be accepted for resuming staged enrollment.
        assert_eq!(validate("abc", "", "", true), Ok(None));
    }

    #[test]
    fn build_setup_pin_action_fresh_and_resume() {
        assert!(build_setup_pin_action("1234", "1234", "9876", false).is_some());
        assert!(build_setup_pin_action("1234", "1234", "1234", false).is_none());
        // Resume uses the entered PIN and sends no duress/biometric options.
        let action = build_setup_pin_action("legacy", "", "", true);
        assert!(
            matches!(action, Some(AppAction::SetupPin { pin, duress_pin, enable_biometric }) if pin == "legacy" && duress_pin.is_none() && !enable_biometric)
        );
    }
}
