package dev.disobey.mango.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable

private val DarkColorScheme = darkColorScheme(
    primary = DarkAccent,
    surface = DarkSurface,
    onSurface = DarkOnSurface,
    surfaceVariant = DarkSecondarySurface,
    onSurfaceVariant = DarkOnSurfaceSecondary,
    error = DarkDestructive,
    primaryContainer = DarkUserBubble,
    secondaryContainer = DarkAssistantBubble,
)

private val LightColorScheme = lightColorScheme(
    primary = LightAccent,
    surface = LightSurface,
    onSurface = LightOnSurface,
    surfaceVariant = LightSecondarySurface,
    onSurfaceVariant = LightOnSurfaceSecondary,
    error = LightDestructive,
    primaryContainer = LightUserBubble,
    secondaryContainer = LightAssistantBubble,
)

@Composable
fun AppTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    dynamicColor: Boolean = false,  // Explicitly false — prevents Material You from overriding badge colors
    content: @Composable () -> Unit,
) {
    val colorScheme = if (darkTheme) DarkColorScheme else LightColorScheme

    MaterialTheme(
        colorScheme = colorScheme,
        content = content,
    )
}
