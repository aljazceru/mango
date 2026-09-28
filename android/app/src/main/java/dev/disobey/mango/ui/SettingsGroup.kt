package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.ui.theme.*

enum class SettingsTint { MODELS, KNOWLEDGE, TOOLS, PRIVACY, APP }

data class SettingsRowSpec(
    val key: String,
    val icon: ImageVector,
    val tint: SettingsTint,
    val title: String,
    val summary: String,
    val onClick: () -> Unit,
)

/** True when the app's (not the system's) scheme is dark; theme can be forced in Appearance. */
@Composable
fun isDarkScheme(): Boolean = MaterialTheme.colorScheme.surface.luminance() < 0.5f

@Composable
private fun tintColors(tint: SettingsTint): Pair<Color, Color> {
    val dark = isDarkScheme()
    return when (tint) {
        SettingsTint.MODELS -> if (dark) TintModelsBgDark to TintModelsFgDark else TintModelsBgLight to TintModelsFgLight
        SettingsTint.KNOWLEDGE -> if (dark) TintKnowledgeBgDark to TintKnowledgeFgDark else TintKnowledgeBgLight to TintKnowledgeFgLight
        SettingsTint.TOOLS -> if (dark) TintToolsBgDark to TintToolsFgDark else TintToolsBgLight to TintToolsFgLight
        SettingsTint.PRIVACY -> if (dark) TintPrivacyBgDark to TintPrivacyFgDark else TintPrivacyBgLight to TintPrivacyFgLight
        SettingsTint.APP -> if (dark) TintAppBgDark to TintAppFgDark else TintAppBgLight to TintAppFgLight
    }
}

private val Outer = 22.dp
private val Inner = 6.dp

private fun groupShape(index: Int, count: Int) = when {
    count == 1 -> RoundedCornerShape(Outer)
    index == 0 -> RoundedCornerShape(topStart = Outer, topEnd = Outer, bottomStart = Inner, bottomEnd = Inner)
    index == count - 1 -> RoundedCornerShape(topStart = Inner, topEnd = Inner, bottomStart = Outer, bottomEnd = Outer)
    else -> RoundedCornerShape(Inner)
}

/** Section header + segmented rows (2dp gaps, 22dp outer corners), as in the sketch. */
fun LazyListScope.settingsGroup(label: String, rows: List<SettingsRowSpec>) {
    item(key = "hdr-$label") {
        Text(
            label,
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.padding(start = 8.dp, top = 22.dp, bottom = 8.dp),
        )
    }
    rows.forEachIndexed { i, spec ->
        item(key = spec.key) {
            // One layout per lazy item: siblings would overlap, not stack.
            SettingsGroupRow(
                spec,
                groupShape(i, rows.size),
                Modifier.padding(bottom = if (i < rows.lastIndex) 2.dp else 0.dp),
            )
        }
    }
}

@Composable
private fun SettingsGroupRow(spec: SettingsRowSpec, shape: RoundedCornerShape, modifier: Modifier = Modifier) {
    val (bg, fg) = tintColors(spec.tint)
    Surface(shape = shape, color = MaterialTheme.colorScheme.surfaceContainerLowest, modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .clickable(onClick = spec.onClick)
                .padding(horizontal = 16.dp, vertical = 13.dp)
                .heightIn(min = 32.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Surface(shape = CircleShape, color = bg, modifier = Modifier.size(40.dp)) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(spec.icon, contentDescription = null, tint = fg, modifier = Modifier.size(21.dp))
                }
            }
            Column(Modifier.weight(1f)) {
                Text(spec.title, style = MaterialTheme.typography.bodyLarge)
                Text(
                    spec.summary,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(20.dp),
            )
        }
    }
}
