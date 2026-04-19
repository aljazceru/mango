package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.Screen

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
    themeMode: String = "system",
    onThemeModeChanged: (String) -> Unit = {},
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings", fontWeight = FontWeight.Medium) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { pad ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(pad)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            item {
                Spacer(Modifier.height(8.dp))
                SettingsSectionLabel("Providers")
                SettingsLinkCard(
                    title = "Providers",
                    subtitle = "${appState.backends.count { it.hasApiKey }} enabled",
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsProviders)) },
                )
            }

            item {
                Spacer(Modifier.height(16.dp))
                SettingsSectionLabel("Defaults")
                SettingsLinkCard(
                    title = "Defaults",
                    subtitle = appState.backends.firstOrNull { it.isActive }?.models?.firstOrNull(),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsDefaults)) },
                )
            }

            item {
                Spacer(Modifier.height(16.dp))
                SettingsSectionLabel("Directory Sources")
                SettingsLinkCard(
                    title = "Directory Sources",
                    subtitle = appState.directorySources.size.let { n ->
                        when (n) {
                            0 -> "No folders added"
                            1 -> "1 folder"
                            else -> "$n folders"
                        }
                    },
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.DirectorySources)) },
                )
            }

            item {
                Spacer(Modifier.height(16.dp))
                SettingsSectionLabel("Memory")
                SettingsLinkCard(
                    title = "Memory",
                    subtitle = buildString {
                        append(if (appState.memoriesEnabled) "Auto-extract on" else "Auto-extract off")
                        if (appState.memoryCount > 0UL) {
                            append(" • ${appState.memoryCount}")
                        }
                    },
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsMemory)) },
                )
            }

            item {
                Spacer(Modifier.height(16.dp))
                SettingsSectionLabel("Security")
                SettingsLinkCard(
                    title = "Security",
                    subtitle = securitySummary(appState),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsSecurity)) },
                )
            }

            item {
                Spacer(Modifier.height(16.dp))
                SettingsSectionLabel("Tools")
                SettingsLinkCard(
                    title = "Tools",
                    subtitle = if (appState.braveApiKeySet) "Web search configured" else "Web search not configured",
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsTools)) },
                )
            }

            item {
                Spacer(Modifier.height(16.dp))
                SettingsSectionLabel("Appearance")
                SettingsLinkCard(
                    title = "Appearance",
                    subtitle = appearanceSummary(themeMode),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsAppearance)) },
                )
            }

            item { Spacer(Modifier.height(32.dp)) }
        }
    }
}

internal fun appearanceSummary(themeMode: String): String = when (themeMode) {
    "light" -> "Force Light"
    "dark" -> "Force Dark"
    else -> "Follow System"
}

@Composable
internal fun SettingsSectionLabel(label: String) {
    Text(
        text = label.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 4.dp, bottom = 4.dp),
    )
}

@Composable
private fun SettingsLinkCard(
    title: String,
    subtitle: String? = null,
    onClick: () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
                if (!subtitle.isNullOrBlank()) {
                    Text(
                        subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = "Open",
                modifier = Modifier.size(18.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

private fun securitySummary(appState: AppState): String {
    val parts = mutableListOf<String>()
    parts += lockTimeoutLabel(appState.lockTimeoutSeconds)
    if (appState.duressPinConfigured) {
        parts += "Duress PIN set"
    }
    parts += if (appState.biometricLoginEnabled) "Biometrics on" else "Biometrics off"
    return parts.joinToString(" • ")
}

internal data class LockTimeoutOption(val label: String, val seconds: Long)

internal val lockTimeoutOptions = listOf(
    LockTimeoutOption("Immediately", 0L),
    LockTimeoutOption("1 minute", 60L),
    LockTimeoutOption("5 minutes", 300L),
    LockTimeoutOption("15 minutes", 900L),
    LockTimeoutOption("Never", -1L),
)

internal fun lockTimeoutLabel(seconds: Long): String =
    lockTimeoutOptions.firstOrNull { it.seconds == seconds }?.label ?: "5 minutes"
