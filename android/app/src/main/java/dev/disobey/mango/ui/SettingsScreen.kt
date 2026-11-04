package dev.disobey.mango.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.automirrored.outlined.AltRoute
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material.icons.outlined.EditNote
import androidx.compose.material.icons.outlined.Extension
import androidx.compose.material.icons.outlined.FileDownload
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material.icons.outlined.Handshake
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.Palette
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material.icons.outlined.Psychology
import androidx.compose.material.icons.outlined.Storage
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.DiscoverableTool
import dev.disobey.mango.rust.LocalLlmCapabilityStatus
import dev.disobey.mango.rust.LocalModelDownloadProgress
import dev.disobey.mango.rust.LocalModelSummary
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.TrustedProvider
import dev.disobey.mango.ui.theme.MangoTheme

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
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { pad ->
        val push = { screen: Screen -> { onDispatch(AppAction.PushScreen(screen = screen)) } }
        val hybrid = appState.hybridProfiles.firstOrNull()
        val capability = appState.localDeviceCapability
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            contentPadding = PaddingValues(bottom = 24.dp),
        ) {
            item(key = "status") {
                InferenceStatusCard(appState = appState, onDispatch = onDispatch, modifier = Modifier.padding(top = 4.dp))
            }
            settingsGroup("Models", listOf(
                SettingsRowSpec("providers", Icons.Outlined.Dns, SettingsTint.MODELS, "Providers",
                    providersSummary(appState.backends.filter { !isOnDeviceBackend(it.id) && it.id != "qvac-local" && it.hasApiKey }.map { it.name }),
                    push(Screen.SettingsProviders)),
                SettingsRowSpec("local", Icons.Outlined.PhoneAndroid, SettingsTint.MODELS, "On-device models",
                    onDeviceModelsSummary(
                        installed = appState.localModels.count { it.downloaded && it.verified },
                        freeRam = formatLocalBytes(capability.availableRamBytes),
                        unsupportedReason = capability.reason.takeIf { capability.status != LocalLlmCapabilityStatus.SUPPORTED },
                    ),
                    push(Screen.SettingsLocalModels)),
                SettingsRowSpec("hybrid", Icons.AutoMirrored.Outlined.AltRoute, SettingsTint.MODELS, "Hybrid rules",
                    hybridRulesSummary(
                        localModel = hybrid?.let { compactModelName(it.localModelId) },
                        remoteModel = hybrid?.let { compactModelName(it.remoteModelId) },
                        policiesOn = hybrid?.policy?.let { p ->
                            listOf(p.escalateIfAttachment, p.preferLocalWhenOffline, p.escalateIfMessageLongerThan != null).count { it }
                        } ?: 0,
                    ),
                    push(Screen.SettingsHybridRouting)),
                SettingsRowSpec("defaults", Icons.Outlined.EditNote, SettingsTint.MODELS, "Default instructions",
                    defaultInstructionsSummary(appState.globalSystemPrompt), push(Screen.SettingsDefaults)),
            ))
            settingsGroup("Knowledge", listOf(
                SettingsRowSpec("folders", Icons.Outlined.FolderOpen, SettingsTint.KNOWLEDGE, "Document folders",
                    documentFoldersSummary(appState.directorySources.size), push(Screen.DirectorySources)),
                SettingsRowSpec("memory", Icons.Outlined.Psychology, SettingsTint.KNOWLEDGE, "Memory",
                    memorySummary(appState.memoriesEnabled, appState.memoryCount.toLong()), push(Screen.SettingsMemory)),
            ))
            settingsGroup("Tools", listOf(
                SettingsRowSpec("tools", Icons.Outlined.Extension, SettingsTint.TOOLS, "Tools & web search",
                    toolsSummary(appState.braveApiKeySet, appState.autoDiscoverToolsEnabled), push(Screen.SettingsTools)),
                SettingsRowSpec("trusted", Icons.Outlined.Handshake, SettingsTint.TOOLS, "Trusted providers",
                    trustedProvidersSubtitle(appState.trustedProviders), push(Screen.TrustedProviders)),
            ))
            settingsGroup("Privacy & security", listOf(
                SettingsRowSpec("applock", Icons.Outlined.Lock, SettingsTint.PRIVACY, "App lock",
                    appLockSummary(appState.noLockMode, appState.lockTimeoutSeconds, appState.biometricLoginEnabled, appState.duressPinConfigured),
                    push(Screen.SettingsAppLock)),
                SettingsRowSpec("data", Icons.Outlined.Storage, SettingsTint.PRIVACY, "Data & retention",
                    dataRetentionSummary(
                        mode = appState.conversationRetentionMode.name.lowercase(),
                        days = appState.conversationRetentionDays.toInt(),
                        archived = appState.archivedConversations.size,
                    ),
                    push(Screen.SettingsDataRetention)),
            ))
            settingsGroup("App", listOf(
                SettingsRowSpec("appearance", Icons.Outlined.Palette, SettingsTint.APP, "Appearance",
                    appearanceSummary(themeMode) + " · " + fontSizeSummary(fontSize), push(Screen.SettingsAppearance)),
            ))
        }
    }
}

@Composable
internal fun LocalInferenceToggleRow(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val capability = appState.localDeviceCapability
    val localRuntimeAvailable = capability.status == LocalLlmCapabilityStatus.SUPPORTED
    MangoCard(modifier = Modifier.fillMaxWidth()) {
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
                )
                appState.localModelsError?.let { error ->
                    Text(
                        error,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        maxLines = 2,
                    )
                }
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
    isActive: Boolean,
    downloadProgress: LocalModelDownloadProgress?,
    anyDownloadActive: Boolean,
    capabilityTotalRamBytes: ULong,
    capabilitySupported: Boolean,
    capabilityReason: String?,
    capabilityReasonCode: String,
    onDispatch: (AppAction) -> Unit,
) {
    val c = MangoTheme.colors
    val busy = downloadProgress != null
    val installed = model.downloaded && model.verified
    val supported = capabilitySupported && capabilityTotalRamBytes >= model.minRamBytes
    val downloadEnabled = !installed && supported && !anyDownloadActive

    Row(
        modifier = Modifier.fillMaxWidth().padding(14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Column(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Text(model.name, style = MaterialTheme.typography.titleMedium)
            Text(
                "${formatLocalBytes(model.sizeBytes)} · ${model.description}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
            )
            if (busy) {
                Text(
                    "${localProgressLabel(downloadProgress!!.stage)}: " +
                        localProgressBytes(downloadProgress.downloadedBytes, downloadProgress.totalBytes),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.primary,
                )
                LinearProgressIndicator(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp)
                        .height(5.dp)
                        .clip(CircleShape),
                    color = c.accent,
                    trackColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                )
            } else {
                val fit = localModelFit(model.minRamBytes, capabilityTotalRamBytes)
                val (segments, onSeg) = when {
                    !capabilitySupported -> 0 to MaterialTheme.colorScheme.error
                    fit == LocalModelFit.TIGHT -> fit.filledSegments to c.warn
                    fit == LocalModelFit.WONT_FIT -> fit.filledSegments to c.warn
                    else -> fit.filledSegments to c.ok
                }
                Row(
                    modifier = Modifier.padding(top = 3.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    repeat(4) { i ->
                        Box(
                            modifier = Modifier
                                .width(14.dp)
                                .height(5.dp)
                                .clip(CircleShape)
                                .background(
                                    if (i < segments) onSeg else MaterialTheme.colorScheme.surfaceContainerHigh
                                ),
                        )
                    }
                    Spacer(Modifier.width(3.dp))
                    Text(
                        if (!capabilitySupported) unsupportedLocalLlmReason(capabilityReason, capabilityReasonCode, "This device cannot run packaged local models")
                        else localModelFitLabel(fit, formatLocalBytes(model.minRamBytes)),
                        style = MaterialTheme.typography.labelSmall,
                        color = if (!capabilitySupported || fit == LocalModelFit.WONT_FIT) {
                            MaterialTheme.colorScheme.error
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                    )
                }
            }
        }
        if (installed) {
            IconButton(
                onClick = { onDispatch(AppAction.DeleteLocalModel(modelId = model.id)) },
                enabled = !anyDownloadActive,
            ) {
                Icon(
                    Icons.Outlined.Delete,
                    contentDescription = "Delete ${model.name}",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            StatusPill(
                text = if (isActive) "Active" else "Installed",
                tone = PillTone.OK,
                icon = Icons.Filled.CheckCircle,
            )
        } else if (busy) {
            // Progress is rendered inline; no cancel action exists in the core.
        } else if (model.downloaded && !model.verified) {
            // Corrupt/unverified file stays re-downloadable; the core replaces it.
            IconButton(
                onClick = { onDispatch(AppAction.DownloadLocalModel(modelId = model.id)) },
                enabled = downloadEnabled,
            ) {
                Icon(
                    Icons.Outlined.FileDownload,
                    contentDescription = "Download ${model.name}",
                    tint = if (downloadEnabled) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            StatusPill(text = "Needs verification", tone = PillTone.WARN, icon = Icons.Filled.Schedule)
        } else {
            IconButton(
                onClick = { onDispatch(AppAction.DownloadLocalModel(modelId = model.id)) },
                enabled = downloadEnabled,
            ) {
                Icon(
                    Icons.Outlined.FileDownload,
                    contentDescription = "Download ${model.name}",
                    tint = if (downloadEnabled) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

internal fun isOnDeviceBackend(id: String): Boolean = id.startsWith("local-")


private fun appStateLocalLlmReason(
    appState: AppState,
    fallback: String,
): String {
    val capability = appState.localDeviceCapability
    if (capability.status == LocalLlmCapabilityStatus.SUPPORTED) {
        return fallback
    }
    return capability.reason?.takeIf { it.isNotBlank() } ?: fallbackByReasonCode(capability.reasonCode)
}

private fun unsupportedLocalLlmReason(
    capabilityReason: String?,
    reasonCode: String,
    fallback: String,
): String = capabilityReason?.takeIf { it.isNotBlank() } ?: fallbackByReasonCode(reasonCode)

private fun fallbackByReasonCode(reasonCode: String): String = when (reasonCode) {
    "disabled_by_feature_flag" -> "Local LLM is disabled by feature flag"
    "unsupported_api_level" -> "Device API level is not supported"
    "unsupported_architecture" -> "Device architecture is not supported"
    "unsupported_process_bitness" -> "Device process is not 64-bit"
    "unsupported_cpu_features" -> "Device CPU features are not supported"
    "insufficient_memory" -> "Not enough memory for Local LLM"
    "insufficient_storage" -> "Not enough storage for Local LLM"
    "probe_unavailable" -> "Device capability probe unavailable"
    "runtime_not_packaged" -> "Local LLM runtime is not packaged"
    "runtime_load_failed" -> "Local LLM runtime failed to load"
    else -> "This device cannot run packaged local models"
}

internal fun compactModelName(modelId: String): String {
    return modelId
        .substringAfterLast('/')
        .replace('_', ' ')
        .replace('-', ' ')
        .take(32)
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
    SectionLabel(label, Modifier.padding(start = 4.dp, bottom = 4.dp))
}

@Composable
internal fun SettingsLinkCard(
    title: String,
    subtitle: String? = null,
    onClick: () -> Unit,
) {
    MangoCard(modifier = Modifier.fillMaxWidth(), onClick = onClick) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
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

@Composable
internal fun AutoDiscoverToolsRow(appState: AppState, onDispatch: (AppAction) -> Unit) {
    MangoCard(modifier = Modifier.fillMaxWidth()) {
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
