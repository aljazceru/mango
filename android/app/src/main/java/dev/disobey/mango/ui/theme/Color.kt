package dev.disobey.mango.ui.theme

import androidx.compose.ui.graphics.Color

// -- Dark Palette --
val DarkSurface = Color(0xFF1A1A1A)
val DarkSecondarySurface = Color(0xFF262626)
val DarkOnSurface = Color(0xFFE8E8E8)
val DarkOnSurfaceSecondary = Color(0xFF8C8C8C)
val DarkAccent = Color(0xFF4D9EFF)
val DarkDestructive = Color(0xFFE53E3E)
val DarkUserBubble = Color(0xFF2E4A7A)
val DarkAssistantBubble = Color(0xFF262626)

// -- Light Palette --
val LightSurface = Color(0xFFF7F7F7)
val LightSecondarySurface = Color(0xFFEDEDED)
val LightOnSurface = Color(0xFF1A1A1A)
val LightOnSurfaceSecondary = Color(0xFF737373)
val LightAccent = Color(0xFF1A6FD4)
val LightDestructive = Color(0xFFB71C1C)
val LightUserBubble = Color(0xFFDDEEFF)
val LightAssistantBubble = Color(0xFFEBEBEB)

// -- Attestation Badge Dark (borders corrected for WCAG 1.4.11 >=3:1) --
val AttestVerifiedBgDark = Color(0xFF1A3A1A)
val AttestVerifiedTextDark = Color(0xFF4ADE80)
val AttestVerifiedBorderDark = Color(0xFF3D9C3D)       // corrected from 0xFF2D7A2D (2.36:1)

val AttestUnverifiedBgDark = Color(0xFF2A2A2A)
val AttestUnverifiedTextDark = Color(0xFF9CA3AF)
val AttestUnverifiedBorderDark = Color(0xFF787878)     // #787878 on #2A2A2A = 3.25:1 (WCAG 1.4.11 pass)

val AttestExpiredBgDark = Color(0xFF2A2A1A)
val AttestExpiredTextDark = Color(0xFFFBBF24)
val AttestExpiredBorderDark = Color(0xFF998420)         // corrected from 0xFF7A6A1A (2.70:1)

val AttestFailedBgDark = Color(0xFF3A1A1A)
val AttestFailedTextDark = Color(0xFFF87171)
val AttestFailedBorderDark = Color(0xFFD03030)          // #D03030 on #3A1A1A = 3.09:1 (WCAG 1.4.11 pass)

// -- Attestation Badge Light (WCAG-verified) --
val AttestVerifiedBgLight = Color(0xFFE8F5E9)
val AttestVerifiedTextLight = Color(0xFF1B5E20)
val AttestVerifiedBorderLight = Color(0xFF2E7D32)

val AttestUnverifiedBgLight = Color(0xFFF5F5F5)
val AttestUnverifiedTextLight = Color(0xFF555555)
val AttestUnverifiedBorderLight = Color(0xFF757575)

val AttestExpiredBgLight = Color(0xFFFFFDE7)
val AttestExpiredTextLight = Color(0xFF7B5800)
val AttestExpiredBorderLight = Color(0xFF8C6600)

val AttestFailedBgLight = Color(0xFFFFEBEE)
val AttestFailedTextLight = Color(0xFFB71C1C)
val AttestFailedBorderLight = Color(0xFFC62828)

// -- Health Status (SettingsScreen) --
val DarkHealthy  = Color(0xFF00C896)   // teal
val DarkDegraded = Color(0xFFF2C12E)   // yellow
val DarkFailed   = Color(0xFFE53E3E)   // red (same as DarkDestructive)
val DarkHealthUnknown = Color(0xFF888888) // gray

val LightHealthy  = Color(0xFF008C64)  // darker teal for light bg
val LightDegraded = Color(0xFF7B5800)  // darker yellow for light bg
val LightFailed   = Color(0xFFB71C1C)  // darker red (same as LightDestructive)
val LightHealthUnknown = Color(0xFF737373) // darker gray for light bg

// -- Health Dim (semi-transparent, for backgrounds) --
val DarkHealthyDim = DarkHealthy.copy(alpha = 0.15f)
val DarkFailedDim  = DarkFailed.copy(alpha = 0.15f)
val LightHealthyDim = LightHealthy.copy(alpha = 0.15f)
val LightFailedDim  = LightFailed.copy(alpha = 0.15f)

// -- Agent Session Status (AgentScreen) --
val DarkAgentRunning   = Color(0xFF33CC66)  // green
val DarkAgentPaused    = Color(0xFFFFCC33)  // yellow
val DarkAgentCompleted = Color(0xFF4D9EFF)  // blue (same as DarkAccent)
val DarkAgentFailed    = Color(0xFFE53E3E)  // red

val LightAgentRunning   = Color(0xFF1B5E20) // dark green for light bg
val LightAgentPaused    = Color(0xFF7B5800) // dark yellow for light bg
val LightAgentCompleted = Color(0xFF1A6FD4) // dark blue (same as LightAccent)
val LightAgentFailed    = Color(0xFFB71C1C) // dark red

// -- Onboarding --
val DarkOnboardingSuccess  = Color(0xFF22BE59)  // green
val LightOnboardingSuccess = Color(0xFF1B7A3A)  // darker green for light bg
