package dev.disobey.mango.ui

import android.os.Build
import android.os.Environment
import android.os.StatFs
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.FileDownload
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.InferenceRoute
import dev.disobey.mango.rust.LocalLlmCapabilityStatus
import dev.disobey.mango.rust.LocalModelSummary
import dev.disobey.mango.ui.theme.MangoTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsLocalModelsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
) {
    val capability = appState.localDeviceCapability
    val progress = appState.localDownloadProgress
    val supported = capability.status == LocalLlmCapabilityStatus.SUPPORTED

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("On-device models") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
            )
        },
    ) { pad ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(pad)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            item(key = "storage") {
                Column(Modifier.padding(top = 4.dp)) {
                    DeviceStorageCard(appState)
                }
            }

            // Featured pick: the largest catalog model that fits this device's RAM.
            val bestFit = if (supported) {
                appState.localModels
                    .filter { !(it.downloaded && it.verified) && it.minRamBytes <= capability.totalRamBytes }
                    .maxByOrNull { it.minRamBytes }
            } else {
                null
            }
            bestFit?.let { model ->
                item(key = "best-fit") {
                    BestFitCard(
                        model = model,
                        enabled = progress == null,
                        onDownload = { onDispatch(AppAction.DownloadLocalModel(modelId = model.id)) },
                    )
                }
            }

            item(key = "local-toggle") {
                LocalInferenceToggleRow(appState = appState, onDispatch = onDispatch)
            }

            item(key = "models") {
                MangoCard(modifier = Modifier.fillMaxWidth()) {
                    // Only the model the current route actually uses is "Active":
                    // the on-device route's model, or the active hybrid profile's
                    // local leg (status.backendId encodes `hybrid:<profileId>`).
                    val status = appState.inferenceStatus
                    val activeModelIds = buildSet {
                        when (status.route) {
                            InferenceRoute.ON_DEVICE ->
                                status.modelId.takeIf { it.isNotBlank() }?.let { add(it) }
                            InferenceRoute.HYBRID ->
                                appState.hybridProfiles
                                    .firstOrNull { it.id == status.backendId.removePrefix("hybrid:") }
                                    ?.let { add(it.localModelId) }
                            else -> {}
                        }
                    }
                    appState.localModels.forEachIndexed { i, model ->
                        LocalModelRow(
                            model = model,
                            isActive = model.id in activeModelIds,
                            downloadProgress = progress?.takeIf { it.modelId == model.id },
                            anyDownloadActive = progress != null,
                            capabilityTotalRamBytes = capability.totalRamBytes,
                            capabilitySupported = supported,
                            capabilityReason = capability.reason,
                            capabilityReasonCode = capability.reasonCode,
                            onDispatch = onDispatch,
                        )
                        if (i < appState.localModels.lastIndex) {
                            HorizontalDivider(
                                modifier = Modifier.padding(start = 14.dp),
                                color = MaterialTheme.colorScheme.outlineVariant,
                            )
                        }
                    }
                }
            }
            item { Spacer(Modifier.height(14.dp)) }
        }
    }
}

/** Storage header: device + RAM, free space, and a used-by-Mango meter. */
@Composable
private fun DeviceStorageCard(appState: AppState) {
    val c = MangoTheme.colors
    val capability = appState.localDeviceCapability
    val free = capability.availableStorageBytes
    val mangoBytes = appState.localModels.filter { it.downloaded }.sumOf { it.sizeBytes }
    // StatFs gives true total+free for /data; fall back to free+used so the bar still renders.
    val stats = runCatching { StatFs(Environment.getDataDirectory().absolutePath) }.getOrNull()
    val total = stats?.let { it.totalBytes.toULong() } ?: (free + mangoBytes)
    val freeNow = stats?.let { it.availableBytes.toULong() } ?: free
    val used = (total - freeNow)
    val other = if (used > mangoBytes) used - mangoBytes else 0UL

    MangoCard(modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    "${Build.MODEL} · ${formatLocalBytes(capability.totalRamBytes)} RAM",
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                Text(
                    "${formatLocalBytes(freeNow)} free",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(Modifier.height(10.dp))
            if (total > 0UL) {
                val totalD = total.toDouble()
                val otherF = (other.toDouble() / totalD).toFloat()
                // Give a sliver to any installed bytes so the accent segment stays visible.
                val mangoF = (mangoBytes.toDouble() / totalD).toFloat().let {
                    if (mangoBytes > 0UL) it.coerceAtLeast(0.03f) else 0f
                }
                val restF = (1f - otherF - mangoF).coerceAtLeast(0f)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(8.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.surfaceContainerHigh),
                ) {
                    if (otherF > 0f) {
                        Box(
                            Modifier
                                .fillMaxWidth(otherF)
                                .height(8.dp)
                                .background(MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.45f)),
                        )
                    }
                    if (mangoF > 0f) {
                        Box(
                            Modifier
                                .fillMaxWidth(if (restF > 0f) mangoF / (mangoF + restF) else 1f)
                                .height(8.dp)
                                .background(c.accent),
                        )
                    }
                }
                Spacer(Modifier.height(8.dp))
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                LegendDot(MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.45f))
                Text(
                    "other apps",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.width(6.dp))
                LegendDot(c.accent)
                Text(
                    "Mango models ${formatLocalBytes(mangoBytes)}",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun LegendDot(color: Color) {
    Box(Modifier.size(8.dp).clip(CircleShape).background(color))
}

/** Featured pick on the mango gradient; the dark download pill contrasts on it. */
@Composable
private fun BestFitCard(
    model: LocalModelSummary,
    enabled: Boolean,
    onDownload: () -> Unit,
) {
    val c = MangoTheme.colors
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        color = Color.Transparent,
        contentColor = c.onAccent,
    ) {
        Column(Modifier.clip(MaterialTheme.shapes.large).background(c.accentGradient).padding(18.dp)) {
            Text(
                "BEST FIT FOR THIS PHONE",
                style = MaterialTheme.typography.labelSmall,
                color = c.onAccent.copy(alpha = 0.75f),
            )
            Text(
                model.name,
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.padding(top = 4.dp, bottom = 2.dp),
            )
            Text(
                "${model.description} · ${formatLocalBytes(model.sizeBytes)}",
                style = MaterialTheme.typography.bodyMedium,
                color = c.onAccent.copy(alpha = 0.85f),
                maxLines = 2,
            )
            Box(
                modifier = Modifier
                    .padding(top = 12.dp)
                    .heightIn(min = 44.dp)
                    .clip(CircleShape)
                    .background(if (enabled) c.onAccent else c.onAccent.copy(alpha = 0.5f))
                    .clickable(enabled = enabled, onClick = onDownload)
                    .padding(horizontal = 16.dp),
                contentAlignment = Alignment.Center,
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Icon(
                        Icons.Outlined.FileDownload,
                        contentDescription = null,
                        tint = c.accent,
                        modifier = Modifier.size(16.dp),
                    )
                    Text(
                        "Download",
                        style = MaterialTheme.typography.labelLarge,
                        color = c.accent,
                    )
                }
            }
        }
    }
}
