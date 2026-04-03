use iced::{Color, Theme, theme};
use mango_core::AttestationStatus;

pub fn dark_palette() -> theme::Palette {
    theme::Palette {
        background: Color::from_rgb(0.102, 0.102, 0.102),  // #1A1A1A
        text:       Color::from_rgb(0.910, 0.910, 0.910),  // #E8E8E8
        primary:    Color::from_rgb(0.302, 0.620, 1.000),  // #4D9EFF
        success:    Color::from_rgb(0.180, 0.686, 0.502),  // #2EAF80
        danger:     Color::from_rgb(0.898, 0.243, 0.243),  // #E53E3E
        warning:    Color::from_rgb(0.984, 0.749, 0.141),  // #FBBF24
    }
}

pub fn light_palette() -> theme::Palette {
    theme::Palette {
        background: Color::from_rgb(0.969, 0.969, 0.969),  // #F7F7F7
        text:       Color::from_rgb(0.102, 0.102, 0.102),  // #1A1A1A
        primary:    Color::from_rgb(0.102, 0.435, 0.831),  // #1A6FD4
        success:    Color::from_rgb(0.106, 0.369, 0.125),  // #1B5E20
        danger:     Color::from_rgb(0.718, 0.110, 0.110),  // #B71C1C
        warning:    Color::from_rgb(0.482, 0.345, 0.000),  // #7B5800
    }
}

pub fn app_theme(is_dark: bool) -> Theme {
    if is_dark {
        Theme::custom("MangoDark".to_string(), dark_palette())
    } else {
        Theme::custom("MangoLight".to_string(), light_palette())
    }
}

pub struct BadgeColors {
    pub bg:     Color,
    pub text:   Color,
    pub border: Color,
}

pub fn badge_colors(status: &AttestationStatus, is_dark: bool) -> BadgeColors {
    if is_dark {
        match status {
            AttestationStatus::Verified => BadgeColors {
                bg:     Color::from_rgb8(0x1A, 0x3A, 0x1A),
                text:   Color::from_rgb8(0x4A, 0xDE, 0x80),
                border: Color::from_rgb8(0x3D, 0x9C, 0x3D),  // corrected from #2D7A2D (2.36:1) to ~3.1:1
            },
            AttestationStatus::Unverified => BadgeColors {
                bg:     Color::from_rgb8(0x2A, 0x2A, 0x2A),
                text:   Color::from_rgb8(0x9C, 0xA3, 0xAF),
                border: Color::from_rgb8(0x78, 0x78, 0x78),  // #787878 on #2A2A2A = 3.25:1 (WCAG 1.4.11 pass)
            },
            AttestationStatus::Expired => BadgeColors {
                bg:     Color::from_rgb8(0x2A, 0x2A, 0x1A),
                text:   Color::from_rgb8(0xFB, 0xBF, 0x24),
                border: Color::from_rgb8(0x99, 0x84, 0x20),  // corrected from #7A6A1A (2.70:1) to ~3.1:1
            },
            AttestationStatus::Failed { .. } => BadgeColors {
                bg:     Color::from_rgb8(0x3A, 0x1A, 0x1A),
                text:   Color::from_rgb8(0xF8, 0x71, 0x71),
                border: Color::from_rgb8(0xD0, 0x30, 0x30),  // #D03030 on #3A1A1A = 3.09:1 (WCAG 1.4.11 pass)
            },
        }
    } else {
        match status {
            AttestationStatus::Verified => BadgeColors {
                bg:     Color::from_rgb8(0xE8, 0xF5, 0xE9),
                text:   Color::from_rgb8(0x1B, 0x5E, 0x20),
                border: Color::from_rgb8(0x2E, 0x7D, 0x32),
            },
            AttestationStatus::Unverified => BadgeColors {
                bg:     Color::from_rgb8(0xF5, 0xF5, 0xF5),
                text:   Color::from_rgb8(0x55, 0x55, 0x55),
                border: Color::from_rgb8(0x75, 0x75, 0x75),
            },
            AttestationStatus::Expired => BadgeColors {
                bg:     Color::from_rgb8(0xFF, 0xFD, 0xE7),
                text:   Color::from_rgb8(0x7B, 0x58, 0x00),
                border: Color::from_rgb8(0x8C, 0x66, 0x00),
            },
            AttestationStatus::Failed { .. } => BadgeColors {
                bg:     Color::from_rgb8(0xFF, 0xEB, 0xEE),
                text:   Color::from_rgb8(0xB7, 0x1C, 0x1C),
                border: Color::from_rgb8(0xC6, 0x28, 0x28),
            },
        }
    }
}

#[derive(Copy, Clone)]
pub struct ViewColors {
    pub bg:                Color,
    pub surface:           Color,
    pub secondary_surface: Color,
    pub card:              Color,
    pub card_enabled:      Color,
    pub border:            Color,
    pub border_enabled:    Color,
    pub text:              Color,
    pub text_dim:          Color,
    pub muted:             Color,
    pub accent:            Color,
    pub accent_dim:        Color,
    pub destructive:       Color,
    pub success:           Color,
    pub warning:           Color,
    pub user_bubble:       Color,
    pub status_running:    Color,
    pub status_paused:     Color,
    pub status_completed:  Color,
    pub status_failed:     Color,
    pub status_cancelled:  Color,
    pub ghost_overlay:     Color,
    pub shadow:            Color,
}

pub fn view_colors(is_dark: bool) -> ViewColors {
    if is_dark {
        ViewColors {
            bg:                Color::from_rgb(0.055, 0.055, 0.063),
            surface:           Color::from_rgb(0.094, 0.094, 0.106),
            secondary_surface: Color::from_rgb(0.149, 0.149, 0.149),
            card:              Color::from_rgb(0.118, 0.122, 0.137),
            card_enabled:      Color::from_rgb(0.071, 0.118, 0.094),
            border:            Color::from_rgb(0.196, 0.200, 0.220),
            border_enabled:    Color::from_rgb(0.094, 0.235, 0.173),
            text:              Color::WHITE,
            text_dim:          Color::from_rgb(0.78, 0.80, 0.84),
            muted:             Color::from_rgb(0.47, 0.49, 0.54),
            accent:            Color::from_rgb(0.0, 0.784, 0.588),
            accent_dim:        Color::from_rgb(0.0, 0.392, 0.294),
            destructive:       Color::from_rgb(0.898, 0.243, 0.243),
            success:           Color::from_rgb(0.0, 0.784, 0.588),
            warning:           Color::from_rgb(0.95, 0.75, 0.10),
            user_bubble:       Color::from_rgb(0.180, 0.290, 0.478),
            status_running:    Color::from_rgb(0.2, 0.8, 0.4),
            status_paused:     Color::from_rgb(1.0, 0.8, 0.2),
            status_completed:  Color::from_rgb(0.302, 0.620, 1.0),
            status_failed:     Color::from_rgb(0.898, 0.243, 0.243),
            status_cancelled:  Color::from_rgb(0.898, 0.243, 0.243),
            ghost_overlay:     Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 },
            shadow:            Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
        }
    } else {
        ViewColors {
            bg:                Color::from_rgb(0.969, 0.969, 0.969),
            surface:           Color::from_rgb(1.0, 1.0, 1.0),
            secondary_surface: Color::from_rgb(0.933, 0.933, 0.933),
            card:              Color::from_rgb(1.0, 1.0, 1.0),
            card_enabled:      Color::from_rgb(0.910, 0.961, 0.914),
            border:            Color::from_rgb(0.800, 0.800, 0.800),
            border_enabled:    Color::from_rgb(0.180, 0.490, 0.196),
            text:              Color::from_rgb(0.102, 0.102, 0.102),
            text_dim:          Color::from_rgb(0.400, 0.400, 0.400),
            muted:             Color::from_rgb(0.550, 0.550, 0.550),
            accent:            Color::from_rgb(0.0, 0.549, 0.392),
            accent_dim:        Color { r: 0.0, g: 0.549, b: 0.392, a: 0.15 },
            destructive:       Color::from_rgb(0.718, 0.110, 0.110),
            success:           Color::from_rgb(0.106, 0.369, 0.125),
            warning:           Color::from_rgb(0.482, 0.345, 0.0),
            user_bubble:       Color::from_rgb(0.867, 0.929, 1.0),
            status_running:    Color::from_rgb(0.106, 0.369, 0.125),
            status_paused:     Color::from_rgb(0.482, 0.345, 0.0),
            status_completed:  Color::from_rgb(0.102, 0.435, 0.831),
            status_failed:     Color::from_rgb(0.718, 0.110, 0.110),
            status_cancelled:  Color::from_rgb(0.718, 0.110, 0.110),
            ghost_overlay:     Color { r: 0.0, g: 0.0, b: 0.0, a: 0.06 },
            shadow:            Color { r: 0.0, g: 0.0, b: 0.0, a: 0.35 },
        }
    }
}
