package dev.disobey.mango.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.DiscoverableTool
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.LocalModelSummary
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.TeeType
import dev.disobey.mango.rust.TrustedProvider

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
    themeMode: String = "system",
    onThemeModeChanged: (String) -> Unit = {},
    fontSize: String = "normal",
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
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            item {
                Spacer(Modifier.height(4.dp))
                SettingsLinkCard(
                    title = "Providers",
                    subtitle = "${providerEnabledCount(appState)} enabled",
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsProviders)) },
                )
            }

            item {
                SettingsLinkCard(
                    title = "Browse local models",
                    subtitle = localModelsSubtitle(appState),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsLocalModels)) },
                )
                LocalInferenceToggleRow(appState = appState, onDispatch = onDispatch)
            }

            item {
                SettingsLinkCard(
                    title = "Hybrid routing",
                    subtitle = hybridRoutingSubtitle(appState),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsHybridRouting)) },
                )
            }

            item {
                SettingsLinkCard(
                    title = "Defaults",
                    subtitle = appState.backends.firstOrNull { it.isActive }?.models?.firstOrNull(),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsDefaults)) },
                )
            }

            item {
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
                SettingsLinkCard(
                    title = "Security",
                    subtitle = securitySummary(appState),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsSecurity)) },
                )
            }

            item {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    SettingsLinkCard(
                        title = "Tools",
                        subtitle = if (appState.braveApiKeySet) "Web search configured" else "Web search not configured",
                        onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsTools)) },
                    )
                    SettingsLinkCard(
                        title = "Trusted providers",
                        subtitle = trustedProvidersSubtitle(appState.trustedProviders),
                        onClick = { onDispatch(AppAction.PushScreen(screen = Screen.TrustedProviders)) },
                    )
                    Card(modifier = Modifier.fillMaxWidth()) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 14.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(
                                modifier = Modifier.weight(1f),
                                verticalArrangement = Arrangement.spacedBy(2.dp),
                        ) {
                            Text(
                                text = "Automatically discover and use tools",
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = FontWeight.Medium,
                            )
                            Text(
                                text = if (appState.trustedProviders.isEmpty())
                                    "Add trusted providers first to enable auto-discovery."
                                else
                                    "Find new tools each conversation from trusted providers only.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Spacer(Modifier.width(16.dp))
                        Switch(
                            modifier = Modifier.semantics {
                                contentDescription = "Automatically discover and use tools"
                            },
                            checked = appState.autoDiscoverToolsEnabled,
                            onCheckedChange = { checked ->
                                onDispatch(AppAction.SetAutoDiscoverTools(enabled = checked))
                            },
                        )
                    }
                }
                }
            }

            item {
                SettingsLinkCard(
                    title = "Appearance",
                    subtitle = appearanceSummary(themeMode) + " • " + fontSizeSummary(fontSize),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsAppearance)) },
                )
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
private fun LocalInferenceToggleRow(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val capability = appState.localDeviceCapability
    val localRuntimeAvailable = capability.maxModelBytes > 0UL
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(enabled = localRuntimeAvailable) {
                    onDispatch(
                        AppAction.SetLocalInferenceEnabled(
                            enabled = !appState.localInferenceEnabled,
                        ),
                    )
                }
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    "On-device inference",
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                )
                Text(
                    localCapabilitySummary(appState),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
            Switch(
                checked = appState.localInferenceEnabled,
                onCheckedChange = { enabled ->
                    onDispatch(AppAction.SetLocalInferenceEnabled(enabled = enabled))
                },
                enabled = localRuntimeAvailable,
            )
        }
    }
}

@Composable
internal fun LocalModelRow(
    model: LocalModelSummary,
    capabilityMaxBytes: ULong,
    capabilityTotalBytes: ULong,
    activeProgressModelId: String?,
    onDispatch: (AppAction) -> Unit,
) {
    val busy = activeProgressModelId == model.id
    val anyDownloadActive = activeProgressModelId != null
    val supported = capabilityMaxBytes >= model.sizeBytes &&
        capabilityMaxBytes > 0UL &&
        capabilityTotalBytes >= model.minRamBytes
    val installed = model.downloaded && model.verified
    val downloadEnabled = !installed && supported && !anyDownloadActive
    val status = when {
        installed -> "Installed"
        model.downloaded && !model.verified -> "Needs verification"
        !supported -> "Unavailable"
        else -> "Available"
    }
    val supportDetail = when {
        supported -> "Requires ${formatLocalBytes(model.minRamBytes)} RAM"
        capabilityMaxBytes == 0UL -> "This device cannot run packaged local models"
        capabilityTotalBytes < model.minRamBytes -> "Requires ${formatLocalBytes(model.minRamBytes)} RAM"
        else -> "Too large for this device"
    }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(enabled = downloadEnabled) {
                    onDispatch(AppAction.DownloadLocalModel(modelId = model.id))
                },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    model.name,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                )
                Text(
                    listOf(
                        model.quantization,
                        formatLocalBytes(model.sizeBytes),
                        status,
                    ).joinToString(" • "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    model.description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    supportDetail,
                    style = MaterialTheme.typography.labelSmall,
                    color = if (supported || installed) {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    } else {
                        MaterialTheme.colorScheme.error
                    },
                )
            }
            Spacer(Modifier.width(12.dp))
            if (installed) {
                LocalModelActionButton(
                    text = "Delete",
                    filled = false,
                    onClick = { onDispatch(AppAction.DeleteLocalModel(modelId = model.id)) },
                    enabled = !anyDownloadActive,
                    compact = true,
                )
            } else {
                LocalModelActionButton(
                    text = when {
                        busy -> "Downloading"
                        supported -> "Download"
                        else -> "Unavailable"
                    },
                    filled = supported,
                    onClick = { onDispatch(AppAction.DownloadLocalModel(modelId = model.id)) },
                    enabled = downloadEnabled,
                    compact = true,
                )
            }
        }
    }
}

internal fun isOnDeviceBackend(id: String): Boolean = id.startsWith("local-")

private fun hybridRoutingSubtitle(appState: AppState): String {
    val isActive = appState.activeBackendId?.startsWith("hybrid:") == true
    val profile = appState.hybridProfiles.firstOrNull()
    return when {
        isActive && profile != null ->
            "On \u2022 ${compactModelName(profile.localModelId)} -> ${compactModelName(profile.remoteModelId)}"
        profile != null -> "Configured \u2022 Off"
        else -> "Off"
    }
}

private fun providerEnabledCount(appState: AppState): Int {
    return appState.backends.count { backend ->
        !isOnDeviceBackend(backend.id) &&
            backend.id != "qvac-local" &&
            backend.hasApiKey
    }
}

private fun localModelsSubtitle(appState: AppState): String {
    val installed = appState.localModels.count { it.downloaded && it.verified }
    val total = appState.localModels.size
    val runtime = appState.localDeviceCapability.reason
        ?: "Up to ${formatLocalBytes(appState.localDeviceCapability.maxModelBytes)} per model"
    return "$total available • $installed installed • $runtime"
}

internal fun compactModelName(modelId: String): String {
    return modelId
        .substringAfterLast('/')
        .replace('_', ' ')
        .replace('-', ' ')
        .take(32)
}

@Composable
internal fun LocalModelActionButton(
    text: String,
    filled: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
    compact: Boolean = false,
) {
    val colors = MaterialTheme.colorScheme
    val background = when {
        !enabled -> colors.onSurface.copy(alpha = 0.12f)
        filled -> colors.primary
        else -> colors.surface
    }
    val content = when {
        !enabled -> colors.onSurface.copy(alpha = 0.38f)
        filled -> colors.onPrimary
        else -> colors.primary
    }
    val border = if (filled) {
        null
    } else {
        BorderStroke(
            width = 1.dp,
            color = if (enabled) colors.outline else colors.onSurface.copy(alpha = 0.12f),
        )
    }

    Surface(
        modifier = Modifier
            .height(if (compact) 36.dp else 48.dp)
            .clickable(enabled = enabled, onClick = onClick),
        shape = MaterialTheme.shapes.extraLarge,
        color = background,
        contentColor = content,
        border = border,
    ) {
        Box(
            modifier = Modifier.padding(horizontal = if (compact) 14.dp else 24.dp),
            contentAlignment = Alignment.Center,
        ) {
            Text(text, style = if (compact) MaterialTheme.typography.labelMedium else MaterialTheme.typography.labelLarge)
        }
    }
}

private fun localCapabilitySummary(appState: AppState): String {
    val capability = appState.localDeviceCapability
    capability.reason?.let { return it }
    val installedCount = appState.localModels.count { it.downloaded && it.verified }
    return "${capability.abi} • ${formatLocalBytes(capability.totalRamBytes)} RAM • $installedCount installed"
}

internal fun localProgressLabel(stage: String): String =
    stage.replaceFirstChar { if (it.isLowerCase()) it.titlecase() else it.toString() }

internal fun localProgressBytes(downloaded: ULong, total: ULong?): String {
    val downloadedLabel = formatLocalBytes(downloaded)
    return total?.let { "$downloadedLabel / ${formatLocalBytes(it)}" } ?: downloadedLabel
}

internal fun formatLocalBytes(bytes: ULong): String {
    val value = bytes.toDouble()
    val mib = 1024.0 * 1024.0
    val gib = mib * 1024.0
    return when {
        value >= gib -> String.format("%.1f GiB", value / gib)
        value >= mib -> String.format("%.0f MiB", value / mib)
        else -> "$bytes B"
    }
}

private fun trustedProvidersSubtitle(providers: List<TrustedProvider>): String = when (providers.size) {
    0 -> "No trusted providers"
    1 -> "1 trusted provider"
    else -> "${providers.size} trusted providers"
}

internal fun discoverToolsSubtitle(tools: List<DiscoverableTool>): String {
    val n = tools.count { it.enabled }
    return when (n) {
        0 -> "No tools enabled"
        1 -> "1 tool enabled"
        else -> "$n tools enabled"
    }
}

internal fun appearanceSummary(themeMode: String): String = when (themeMode) {
    "light" -> "Force Light"
    "dark" -> "Force Dark"
    else -> "Follow System"
}

internal fun fontSizeSummary(fontSize: String): String = when (fontSize) {
    "small" -> "Small text"
    "large" -> "Large text"
    "xlarge" -> "Extra large text"
    else -> "Normal text"
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
internal fun SettingsLinkCard(
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
