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

// -- Settings status card (container = user-bubble blue) --
val StatusCardBgLight = LightUserBubble            // #DDEEFF
val StatusCardOnLight = Color(0xFF0B3A75)
val StatusCardBgDark = DarkUserBubble              // #2E4A7A
val StatusCardOnDark = Color(0xFFDDEEFF)

// -- Settings group icon tints (bg / fg) --
val TintModelsBgLight = Color(0xFFDDEEFF);    val TintModelsFgLight = Color(0xFF1A6FD4)
val TintKnowledgeBgLight = Color(0xFFFFF0D4); val TintKnowledgeFgLight = Color(0xFF8A5A00)
val TintToolsBgLight = Color(0xFFDDF4E6);     val TintToolsFgLight = Color(0xFF1B6B3A)
val TintPrivacyBgLight = Color(0xFFF3E3FF);   val TintPrivacyFgLight = Color(0xFF6A2FA0)
val TintAppBgLight = Color(0xFFFFE2EC);       val TintAppFgLight = Color(0xFFA3224F)
val TintModelsBgDark = Color(0xFF1E3553);     val TintModelsFgDark = Color(0xFF8EC2FF)
val TintKnowledgeBgDark = Color(0xFF3A2E14);  val TintKnowledgeFgDark = Color(0xFFF5C66B)
val TintToolsBgDark = Color(0xFF173524);      val TintToolsFgDark = Color(0xFF7BD9A0)
val TintPrivacyBgDark = Color(0xFF33224A);    val TintPrivacyFgDark = Color(0xFFD2A8FF)
val TintAppBgDark = Color(0xFF3E1E2A);        val TintAppFgDark = Color(0xFFFF9EC0)
