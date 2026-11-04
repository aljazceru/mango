package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.ui.theme.MangoTheme
import dev.disobey.mango.ui.theme.TintPair

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
fun isDarkScheme(): Boolean = MangoTheme.colors.isDark

private val SettingsTint.pair: TintPair
    @Composable get() = when (this) {
        SettingsTint.MODELS -> MangoTheme.colors.tintModels
        SettingsTint.KNOWLEDGE -> MangoTheme.colors.tintKnowledge
        SettingsTint.TOOLS -> MangoTheme.colors.tintTools
        SettingsTint.PRIVACY -> MangoTheme.colors.tintPrivacy
        SettingsTint.APP -> MangoTheme.colors.tintApp
    }

/** Section header + one grouped card with hairline dividers between rows. */
fun LazyListScope.settingsGroup(label: String, rows: List<SettingsRowSpec>) {
    item(key = "hdr-$label") {
        SectionLabel(label, Modifier.padding(start = 8.dp, top = 22.dp, bottom = 8.dp))
    }
    item(key = "group-$label") {
        MangoCard(modifier = Modifier.fillMaxWidth()) {
            rows.forEachIndexed { i, spec ->
                SettingsGroupRow(spec)
                if (i < rows.lastIndex) {
                    HorizontalDivider(
                        modifier = Modifier.padding(start = 66.dp),
                        color = MaterialTheme.colorScheme.outlineVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun SettingsGroupRow(spec: SettingsRowSpec, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = spec.onClick)
            .padding(horizontal = 14.dp, vertical = 12.dp)
            .heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        TintedIcon(spec.icon, spec.tint.pair, size = 36.dp)
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
