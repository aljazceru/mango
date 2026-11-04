package dev.disobey.mango.ui.theme

import androidx.compose.ui.graphics.Color

// "Ripe" palette. Light = warm cream + mango; dark ("Enclave") = warm near-black where mango reads as a glow.
// Text-bearing pairs are >= 4.5:1, borders/indicators >= 3:1 (WCAG 1.4.3 / 1.4.11).

// -- Light --
val RipeBg = Color(0xFFFFF7EC)
val RipeInk = Color(0xFF2A1A0C)
val RipeMuted = Color(0xFF7A6249)          // 5.4:1 on RipeBg
val RipeOutline = Color(0xFF9C8467)        // 3.3:1 on RipeBg
val RipeOutlineVariant = Color(0xFFF0DEC6)
val RipeContainerLowest = Color(0xFFFFFFFF)
val RipeContainerLow = Color(0xFFFFFBF4)
val RipeContainer = Color(0xFFFDF1E1)
val RipeContainerHigh = Color(0xFFFCEBD3)
val RipeContainerHighest = Color(0xFFF9E6CC)
val RipePrimary = Color(0xFFB85300)        // deep mango: text/icons/filled buttons, 4.6:1 on RipeBg, white on it 4.9:1
val RipePrimaryContainer = Color(0xFFFFDDB0)
val RipeOnPrimaryContainer = Color(0xFF3A1F00)
val RipeLeaf = Color(0xFF2A7148)          // 5.1:1 on RipeLeafContainer (status pills)
val RipeLeafContainer = Color(0xFFE1F2E6)
val RipeOnLeafContainer = Color(0xFF14532D)
val RipeCoral = Color(0xFFA2401C)
val RipeCoralContainer = Color(0xFFFFE0D6)
val RipeOnCoralContainer = Color(0xFF5C1E08)
val RipeError = Color(0xFFB3261E)
val RipeErrorContainer = Color(0xFFFCE4E1)
val RipeOnErrorContainer = Color(0xFF5C0F0A)
val RipeOlive = Color(0xFF6B5A00)          // "warning/expired": deliberately not mango-hued
val RipeOliveContainer = Color(0xFFF4EFC8)
val RipeAccent = Color(0xFFFF8A1F)         // bright mango: decorative fills only, never text on cream
val RipeOnAccent = Color(0xFF1F1206)       // white on mango fails contrast; always dark ink
val RipeUserBubble = Color(0xFFFFE3BF)
val RipeGradient = listOf(Color(0xFFFFC93C), Color(0xFFFF8A1F), Color(0xFFF2542D))

// -- Dark ("Enclave") --
val EnclaveBg = Color(0xFF0F0E0B)
val EnclaveInk = Color(0xFFF5EBDD)
val EnclaveMuted = Color(0xFFA39581)       // 6.4:1 on EnclaveBg
val EnclaveOutline = Color(0xFF8A7C68)     // 4.7:1 on EnclaveBg
val EnclaveOutlineVariant = Color(0xFF2E2A22)
val EnclaveContainerLowest = Color(0xFF0B0A08)
val EnclaveContainerLow = Color(0xFF161511)
val EnclaveContainer = Color(0xFF1B1915)
val EnclaveContainerHigh = Color(0xFF26221B)
val EnclaveContainerHighest = Color(0xFF2E2A22)
val EnclavePrimary = Color(0xFFFFB13B)
val EnclaveOnPrimary = Color(0xFF1A1205)
val EnclavePrimaryContainer = Color(0xFF5A3A0E)
val EnclaveOnPrimaryContainer = Color(0xFFFFDDB0)
val EnclaveLeaf = Color(0xFF7BD88F)
val EnclaveOnLeaf = Color(0xFF0B2614)
val EnclaveLeafContainer = Color(0xFF15301E)
val EnclaveOnLeafContainer = Color(0xFFB8F0C4)
val EnclaveCoral = Color(0xFFFF9F7A)
val EnclaveOnCoral = Color(0xFF3A1306)
val EnclaveCoralContainer = Color(0xFF3A1E15)
val EnclaveOnCoralContainer = Color(0xFFFFD2C2)
val EnclaveError = Color(0xFFFF8A80)
val EnclaveOnError = Color(0xFF3A0906)
val EnclaveErrorContainer = Color(0xFF3A1714)
val EnclaveOnErrorContainer = Color(0xFFFFD9D4)
val EnclaveOlive = Color(0xFFD8C95A)
val EnclaveOliveContainer = Color(0xFF2C2A12)
val EnclaveAccent = Color(0xFFFFB13B)
val EnclaveOnAccent = Color(0xFF1A1205)
val EnclaveUserBubble = Color(0xFF3A2913)
val EnclaveOnUserBubble = Color(0xFFFFE9CC)
val EnclaveGradient = listOf(Color(0xFFFFD25A), Color(0xFFFF9A2E), Color(0xFFFF6A3D))

// -- Attestation badge (border >= 3:1 vs its background, WCAG 1.4.11) --
val AttestVerifiedBgDark = Color(0xFF15301E)
val AttestVerifiedTextDark = Color(0xFF7BD88F)
val AttestVerifiedBorderDark = Color(0xFF3D9C5A)

val AttestUnverifiedBgDark = Color(0xFF26221B)
val AttestUnverifiedTextDark = Color(0xFFA39581)
val AttestUnverifiedBorderDark = Color(0xFF7A6E5C)

// Expired is olive, not amber, so it never reads as the mango accent.
val AttestExpiredBgDark = Color(0xFF2C2A12)
val AttestExpiredTextDark = Color(0xFFD8C95A)
val AttestExpiredBorderDark = Color(0xFF8F8530)

val AttestFailedBgDark = Color(0xFF3A1714)
val AttestFailedTextDark = Color(0xFFFF8A80)
val AttestFailedBorderDark = Color(0xFFD03030)

val AttestVerifiedBgLight = Color(0xFFE1F2E6)
val AttestVerifiedTextLight = Color(0xFF14532D)
val AttestVerifiedBorderLight = Color(0xFF2E7D4F)

val AttestUnverifiedBgLight = Color(0xFFFCEBD3)
val AttestUnverifiedTextLight = Color(0xFF5E4A35)
val AttestUnverifiedBorderLight = Color(0xFF9C8467)

val AttestExpiredBgLight = Color(0xFFF4EFC8)
val AttestExpiredTextLight = Color(0xFF574900)
val AttestExpiredBorderLight = Color(0xFF7A6A00)

val AttestFailedBgLight = Color(0xFFFCE4E1)
val AttestFailedTextLight = Color(0xFFB3261E)
val AttestFailedBorderLight = Color(0xFFC62828)
