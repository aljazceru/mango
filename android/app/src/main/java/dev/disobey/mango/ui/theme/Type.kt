package dev.disobey.mango.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import dev.disobey.mango.R

// Bundled (OFL, see assets/licenses): no downloadable fonts, no extra network calls.
val Figtree = FontFamily(
    Font(R.font.figtree_regular, FontWeight.Normal),
    Font(R.font.figtree_medium, FontWeight.Medium),
    Font(R.font.figtree_semibold, FontWeight.SemiBold),
    Font(R.font.figtree_bold, FontWeight.Bold),
)
val Fraunces = FontFamily(Font(R.font.fraunces_semibold, FontWeight.SemiBold))

/** Hashes, model IDs, npubs, code. */
val MonoFamily = FontFamily(
    Font(R.font.jetbrainsmono_regular, FontWeight.Normal),
    Font(R.font.jetbrainsmono_medium, FontWeight.Medium),
)

private val base = Typography()
private fun TextStyle.display() = copy(fontFamily = Fraunces, fontWeight = FontWeight.SemiBold, letterSpacing = (-0.02 * fontSize.value).sp)
private fun TextStyle.body(weight: FontWeight? = fontWeight) = copy(fontFamily = Figtree, fontWeight = weight)

val MangoTypography = Typography(
    displayLarge = base.displayLarge.display(),
    displayMedium = base.displayMedium.display(),
    displaySmall = base.displaySmall.display(),
    headlineLarge = base.headlineLarge.display(),
    headlineMedium = base.headlineMedium.display(),
    headlineSmall = base.headlineSmall.display(),
    titleLarge = base.titleLarge.display(),
    titleMedium = base.titleMedium.body(FontWeight.SemiBold),
    titleSmall = base.titleSmall.body(FontWeight.SemiBold),
    bodyLarge = base.bodyLarge.body(),
    bodyMedium = base.bodyMedium.body(),
    bodySmall = base.bodySmall.body(),
    labelLarge = base.labelLarge.body(FontWeight.SemiBold),
    labelMedium = base.labelMedium.body(FontWeight.SemiBold),
    labelSmall = base.labelSmall.body(FontWeight.SemiBold),
)
