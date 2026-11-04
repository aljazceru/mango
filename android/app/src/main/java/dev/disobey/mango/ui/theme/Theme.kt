package dev.disobey.mango.ui.theme

import android.app.Activity
import android.content.ContextWrapper
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Surface
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat

private val LightColorScheme = lightColorScheme(
    primary = RipePrimary,
    onPrimary = Color.White,
    primaryContainer = RipePrimaryContainer,
    onPrimaryContainer = RipeOnPrimaryContainer,
    inversePrimary = EnclavePrimary,
    secondary = RipeLeaf,
    onSecondary = Color.White,
    secondaryContainer = RipeLeafContainer,
    onSecondaryContainer = RipeOnLeafContainer,
    tertiary = RipeCoral,
    onTertiary = Color.White,
    tertiaryContainer = RipeCoralContainer,
    onTertiaryContainer = RipeOnCoralContainer,
    background = RipeBg,
    onBackground = RipeInk,
    surface = RipeBg,
    onSurface = RipeInk,
    surfaceVariant = RipeContainerHigh,
    onSurfaceVariant = RipeMuted,
    surfaceTint = RipePrimary,
    inverseSurface = Color(0xFF3A2A1A),
    inverseOnSurface = RipeBg,
    error = RipeError,
    onError = Color.White,
    errorContainer = RipeErrorContainer,
    onErrorContainer = RipeOnErrorContainer,
    outline = RipeOutline,
    outlineVariant = RipeOutlineVariant,
    surfaceBright = RipeContainerLowest,
    surfaceDim = RipeContainerHighest,
    surfaceContainerLowest = RipeContainerLowest,
    surfaceContainerLow = RipeContainerLow,
    surfaceContainer = RipeContainer,
    surfaceContainerHigh = RipeContainerHigh,
    surfaceContainerHighest = RipeContainerHighest,
)

private val DarkColorScheme = darkColorScheme(
    primary = EnclavePrimary,
    onPrimary = EnclaveOnPrimary,
    primaryContainer = EnclavePrimaryContainer,
    onPrimaryContainer = EnclaveOnPrimaryContainer,
    inversePrimary = RipePrimary,
    secondary = EnclaveLeaf,
    onSecondary = EnclaveOnLeaf,
    secondaryContainer = EnclaveLeafContainer,
    onSecondaryContainer = EnclaveOnLeafContainer,
    tertiary = EnclaveCoral,
    onTertiary = EnclaveOnCoral,
    tertiaryContainer = EnclaveCoralContainer,
    onTertiaryContainer = EnclaveOnCoralContainer,
    background = EnclaveBg,
    onBackground = EnclaveInk,
    surface = EnclaveBg,
    onSurface = EnclaveInk,
    surfaceVariant = EnclaveContainerHigh,
    onSurfaceVariant = EnclaveMuted,
    surfaceTint = EnclavePrimary,
    inverseSurface = EnclaveInk,
    inverseOnSurface = EnclaveBg,
    error = EnclaveError,
    onError = EnclaveOnError,
    errorContainer = EnclaveErrorContainer,
    onErrorContainer = EnclaveOnErrorContainer,
    outline = EnclaveOutline,
    outlineVariant = EnclaveOutlineVariant,
    surfaceBright = EnclaveContainerHighest,
    surfaceDim = EnclaveBg,
    surfaceContainerLowest = EnclaveContainerLowest,
    surfaceContainerLow = EnclaveContainerLow,
    surfaceContainer = EnclaveContainer,
    surfaceContainerHigh = EnclaveContainerHigh,
    surfaceContainerHighest = EnclaveContainerHighest,
)

/** Container + content color pair for tinted icons, avatars and chips. */
@Immutable
data class TintPair(val container: Color, val content: Color)

/** Brand roles Material's ColorScheme has no slot for. Read via [MangoTheme.colors]. */
@Immutable
data class MangoColors(
    val isDark: Boolean,
    /** Bright mango for decorative fills (progress, meters). Never text on the light background. */
    val accent: Color,
    val accentGradient: Brush,
    /** Content on [accent] / [accentGradient]: always dark ink. */
    val onAccent: Color,
    /** Cards that sit on the background (white on cream, raised on near-black). */
    val card: Color,
    val userBubble: Color,
    val onUserBubble: Color,
    /** Verified / healthy / running. */
    val ok: Color,
    val okContainer: Color,
    /** Expired / degraded / paused. Olive, so status never reads as the accent. */
    val warn: Color,
    val warnContainer: Color,
    /** Inference routes: confidential cloud (TEE), on-device, hybrid. */
    val tee: TintPair,
    val local: TintPair,
    val hybrid: TintPair,
    /** Settings group icon tints. */
    val tintModels: TintPair,
    val tintKnowledge: TintPair,
    val tintTools: TintPair,
    val tintPrivacy: TintPair,
    val tintApp: TintPair,
)

private val LightMango = MangoColors(
    isDark = false,
    accent = RipeAccent,
    accentGradient = Brush.linearGradient(RipeGradient),
    onAccent = RipeOnAccent,
    card = RipeContainerLowest,
    userBubble = RipeUserBubble,
    onUserBubble = RipeInk,
    ok = RipeLeaf,
    okContainer = RipeLeafContainer,
    warn = RipeOlive,
    warnContainer = RipeOliveContainer,
    tee = TintPair(Color(0xFFFFE0BA), Color(0xFF9A4A00)),
    local = TintPair(Color(0xFFDDF0D6), Color(0xFF2E6B1E)),
    hybrid = TintPair(Color(0xFFFFE1D6), Color(0xFFA23A16)),
    tintModels = TintPair(Color(0xFFFFE7C7), Color(0xFF9A5200)),
    tintKnowledge = TintPair(Color(0xFFE2F1D8), Color(0xFF3B6E1F)),
    tintTools = TintPair(Color(0xFFFFE0D6), Color(0xFFA2401C)),
    tintPrivacy = TintPair(Color(0xFFF3E6D8), Color(0xFF6E4A2A)),
    tintApp = TintPair(Color(0xFFFFF1C2), Color(0xFF7A5A00)),
)

private val DarkMango = MangoColors(
    isDark = true,
    accent = EnclaveAccent,
    accentGradient = Brush.linearGradient(EnclaveGradient),
    onAccent = EnclaveOnAccent,
    card = EnclaveContainer,
    userBubble = EnclaveUserBubble,
    onUserBubble = EnclaveOnUserBubble,
    ok = EnclaveLeaf,
    okContainer = EnclaveLeafContainer,
    warn = EnclaveOlive,
    warnContainer = EnclaveOliveContainer,
    tee = TintPair(Color(0xFF3A2A12), Color(0xFFFFC370)),
    local = TintPair(Color(0xFF1C2E17), Color(0xFFA6DE8C)),
    hybrid = TintPair(Color(0xFF3A1E15), Color(0xFFFF9F7A)),
    tintModels = TintPair(Color(0xFF3A2A12), Color(0xFFFFC370)),
    tintKnowledge = TintPair(Color(0xFF1C2E17), Color(0xFFA6DE8C)),
    tintTools = TintPair(Color(0xFF3A1E15), Color(0xFFFF9F7A)),
    tintPrivacy = TintPair(Color(0xFF2B241B), Color(0xFFE0C4A0)),
    tintApp = TintPair(Color(0xFF33290F), Color(0xFFF5D46B)),
)

private val LocalMangoColors = staticCompositionLocalOf { LightMango }

object MangoTheme {
    val colors: MangoColors
        @Composable @ReadOnlyComposable get() = LocalMangoColors.current
}

val MangoShapes = Shapes(
    extraSmall = RoundedCornerShape(8.dp),
    small = RoundedCornerShape(14.dp),
    medium = RoundedCornerShape(22.dp),
    large = RoundedCornerShape(28.dp),
    extraLarge = RoundedCornerShape(32.dp),
)

@Composable
fun AppTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    // System bar icons follow the app's theme, which can be forced in Appearance.
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            var ctx = view.context
            while (ctx is ContextWrapper && ctx !is Activity) ctx = ctx.baseContext
            (ctx as? Activity)?.window?.let { window ->
                WindowCompat.getInsetsController(window, view).apply {
                    isAppearanceLightStatusBars = !darkTheme
                    isAppearanceLightNavigationBars = !darkTheme
                }
            }
        }
    }
    CompositionLocalProvider(LocalMangoColors provides if (darkTheme) DarkMango else LightMango) {
        MaterialTheme(
            colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme,
            typography = MangoTypography,
            shapes = MangoShapes,
        ) {
            // Bare screens inherit the app's canvas and text colors, not the window's theme.
            Surface(
                modifier = Modifier.fillMaxSize(),
                color = MaterialTheme.colorScheme.background,
                contentColor = MaterialTheme.colorScheme.onBackground,
                content = content,
            )
        }
    }
}
