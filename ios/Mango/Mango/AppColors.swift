import SwiftUI

/// Single authoritative semantic color token file for the entire iOS app.
/// All views reference these functions — no numeric Color literals in view files.
///
/// NOTE: AttestationStatus is available app-wide from the UniFFI-generated Rust bindings.
/// No explicit module import is needed — it is part of the app target automatically.
/// (Confirmed: AttestationBadgeView.swift uses AttestationStatus with only `import SwiftUI`.)
enum AppColors {

    // MARK: - Surface

    static func backgroundPrimary(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.102, green: 0.102, blue: 0.102)  // #1A1A1A
                        : Color(red: 0.969, green: 0.969, blue: 0.969)  // #F7F7F7
    }

    static func secondarySurface(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.149, green: 0.149, blue: 0.149)  // #262626
                        : Color(red: 0.929, green: 0.929, blue: 0.929)  // #EDEDED
    }

    // MARK: - Text

    static func textPrimary(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.910, green: 0.910, blue: 0.910)  // #E8E8E8
                        : Color(red: 0.102, green: 0.102, blue: 0.102)  // #1A1A1A
    }

    static func textSecondary(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.550, green: 0.550, blue: 0.550)  // #8C8C8C
                        : Color(red: 0.450, green: 0.450, blue: 0.450)  // #737373
    }

    // MARK: - Accent

    static func accent(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.302, green: 0.620, blue: 1.000)  // #4D9EFF
                        : Color(red: 0.102, green: 0.435, blue: 0.831)  // #1A6FD4
    }

    static func destructive(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.898, green: 0.243, blue: 0.243)  // #E53E3E
                        : Color(red: 0.718, green: 0.110, blue: 0.110)  // #B71C1C
    }

    // MARK: - Bubbles

    static func userBubble(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.180, green: 0.290, blue: 0.478)  // #2E4A7A
                        : Color(red: 0.867, green: 0.929, blue: 1.000)  // #DDEEFF
    }

    static func assistantBubble(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(white: 0.15)   // ~#262626
                        : Color(white: 0.92)   // ~#EBEBEB
    }

    static func userBubbleText(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.910, green: 0.910, blue: 0.910)  // #E8E8E8
                        : Color(red: 0.102, green: 0.102, blue: 0.102)  // #1A1A1A
    }

    // MARK: - Health Status (Settings)

    static func healthSuccess(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.0, green: 0.784, blue: 0.588)   // #00C896
                        : Color(red: 0.0, green: 0.549, blue: 0.392)   // #008C64
    }

    static func healthWarning(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color.yellow
                        : Color(red: 0.600, green: 0.500, blue: 0.0)   // darker yellow for light bg
    }

    static func healthMuted(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? Color.gray
                        : Color(red: 0.450, green: 0.450, blue: 0.450) // #737373
    }

    // MARK: - Attestation Badge

    static func attestBg(_ status: AttestationStatus, _ scheme: ColorScheme) -> Color {
        switch (status, scheme) {
        case (.verified, .dark):    return Color(red: 0.102, green: 0.227, blue: 0.102)  // #1A3A1A
        case (.verified, _):        return Color(red: 0.910, green: 0.961, blue: 0.914)  // #E8F5E9
        case (.unverified, .dark):  return Color(red: 0.165, green: 0.165, blue: 0.165)  // #2A2A2A
        case (.unverified, _):      return Color(red: 0.961, green: 0.961, blue: 0.961)  // #F5F5F5
        case (.expired, .dark):     return Color(red: 0.165, green: 0.165, blue: 0.102)  // #2A2A1A
        case (.expired, _):         return Color(red: 1.000, green: 0.992, blue: 0.906)  // #FFFDE7
        case (.failed, .dark):      return Color(red: 0.227, green: 0.102, blue: 0.102)  // #3A1A1A
        case (.failed, _):          return Color(red: 1.000, green: 0.922, blue: 0.933)  // #FFEBEE
        default:                    return Color(red: 0.165, green: 0.165, blue: 0.165)  // #2A2A2A fallback
        }
    }

    static func attestText(_ status: AttestationStatus, _ scheme: ColorScheme) -> Color {
        switch (status, scheme) {
        case (.verified, .dark):    return Color(red: 0.290, green: 0.871, blue: 0.502)  // #4ADE80
        case (.verified, _):        return Color(red: 0.106, green: 0.369, blue: 0.125)  // #1B5E20
        case (.unverified, .dark):  return Color(red: 0.612, green: 0.639, blue: 0.686)  // #9CA3AF
        case (.unverified, _):      return Color(red: 0.333, green: 0.333, blue: 0.333)  // #555555
        case (.expired, .dark):     return Color(red: 0.984, green: 0.749, blue: 0.141)  // #FBBF24
        case (.expired, _):         return Color(red: 0.482, green: 0.345, blue: 0.000)  // #7B5800
        case (.failed, .dark):      return Color(red: 0.973, green: 0.443, blue: 0.443)  // #F87171
        case (.failed, _):          return Color(red: 0.718, green: 0.110, blue: 0.110)  // #B71C1C
        default:                    return Color(red: 0.612, green: 0.639, blue: 0.686)  // #9CA3AF fallback
        }
    }

    static func attestBorder(_ status: AttestationStatus, _ scheme: ColorScheme) -> Color {
        switch (status, scheme) {
        case (.verified, .dark):    return Color(red: 0.239, green: 0.612, blue: 0.239)  // #3D9C3D (corrected from #2D7A2D)
        case (.verified, _):        return Color(red: 0.180, green: 0.490, blue: 0.196)  // #2E7D32
        case (.unverified, .dark):  return Color(red: 0.471, green: 0.471, blue: 0.471)  // #787878 on #2A2A2A = 3.25:1 (WCAG 1.4.11 pass)
        case (.unverified, _):      return Color(red: 0.459, green: 0.459, blue: 0.459)  // #757575
        case (.expired, .dark):     return Color(red: 0.600, green: 0.518, blue: 0.125)  // #998420 (corrected from #7A6A1A)
        case (.expired, _):         return Color(red: 0.549, green: 0.400, blue: 0.000)  // #8C6600
        case (.failed, .dark):      return Color(red: 0.816, green: 0.188, blue: 0.188)  // #D03030 on #3A1A1A = 3.09:1 (WCAG 1.4.11 pass)
        case (.failed, _):          return Color(red: 0.776, green: 0.157, blue: 0.157)  // #C62828
        default:                    return Color(red: 0.471, green: 0.471, blue: 0.471)  // #787878 fallback
        }
    }
}
