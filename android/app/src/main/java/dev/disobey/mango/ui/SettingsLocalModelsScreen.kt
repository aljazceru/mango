package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.LocalLlmCapabilityStatus

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsLocalModelsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
) {
    val capability = appState.localDeviceCapability
    val progress = appState.localDownloadProgress

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Local Models", fontWeight = FontWeight.Medium) },
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
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            progress?.let {
                item {
                    Column(
                        modifier = Modifier.padding(top = 8.dp),
                        verticalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        Text(
                            "${localProgressLabel(it.stage)}: ${localProgressBytes(it.downloadedBytes, it.totalBytes)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                    }
                }
            }

            appState.lastError?.let { error ->
                item {
                    Text(
                        error,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }

            appState.localModels.forEach { model ->
                item {
                    val capabilitySupported = capability.status == LocalLlmCapabilityStatus.SUPPORTED
                    LocalModelRow(
                        model = model,
                        capabilityMaxBytes = capability.maxModelBytes,
                        capabilityTotalBytes = capability.totalRamBytes,
                        capabilitySupported = capabilitySupported,
                        capabilityReason = capability.reason,
                        capabilityReasonCode = capability.reasonCode,
                        activeProgressModelId = progress?.modelId,
                        onDispatch = onDispatch,
                    )
                }
            }
        }
    }
}
