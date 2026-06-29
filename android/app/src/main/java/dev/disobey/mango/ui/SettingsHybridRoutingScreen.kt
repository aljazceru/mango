package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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
import dev.disobey.mango.rust.BackendSummary
import dev.disobey.mango.rust.HybridProfile
import dev.disobey.mango.rust.LocalPreprocessing
import dev.disobey.mango.rust.RoutingPolicy

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsHybridRoutingScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Hybrid Routing", fontWeight = FontWeight.Medium) },
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
            item { Spacer(Modifier.padding(top = 2.dp)) }
            item { HybridRoutingToggleCard(appState = appState, onDispatch = onDispatch) }
            item { HybridRoutingConfigCard(appState = appState, onDispatch = onDispatch) }
            item { HybridRoutingPolicyCard(appState = appState, onDispatch = onDispatch) }
        }
    }
}

@Composable
private fun HybridRoutingToggleCard(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val isActive = appState.activeBackendId?.startsWith("hybrid:") == true
    val hasProfile = appState.hybridProfiles.isNotEmpty()
    val toggleHybrid: (Boolean) -> Unit = { enable ->
        onDispatch(
            if (enable && hasProfile) {
                AppAction.SetActiveHybridProfile(profileId = appState.hybridProfiles.first().id)
            } else {
                val fallback = appState.backends.firstOrNull {
                    !isOnDeviceBackend(it.id) &&
                        it.hasApiKey &&
                        it.healthStatus.name != "FAILED"
                }
                fallback?.let { AppAction.SetActiveBackend(backendId = it.id) }
                    ?: AppAction.PopScreen
            },
        )
    }
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(enabled = hasProfile) { toggleHybrid(!isActive) }
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    "Use hybrid routing",
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                )
                Text(
                    when {
                        !hasProfile -> "Create a profile below to enable."
                        isActive -> "Local model paired with a confidential remote."
                        else -> "Off — a single backend handles all turns."
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(checked = isActive, enabled = hasProfile, onCheckedChange = toggleHybrid)
        }
    }
}

@Composable
private fun HybridRoutingConfigCard(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val localOptions = appState.backends
        .filter { isOnDeviceBackend(it.id) && it.models.isNotEmpty() }
        .mapNotNull { backend ->
            val modelId = backend.models.firstOrNull() ?: return@mapNotNull null
            val model = appState.localModels.firstOrNull { it.id == modelId }
            if (model?.downloaded == true && model.verified) {
                Triple(backend, modelId, model.name)
            } else {
                null
            }
        }
    val remoteBackends = appState.backends
        .filter { backend ->
            !isOnDeviceBackend(backend.id) &&
                backend.teeType != dev.disobey.mango.rust.TeeType.UNKNOWN &&
                backend.hasApiKey &&
                backend.models.isNotEmpty() &&
                backend.healthStatus != dev.disobey.mango.rust.HealthStatus.FAILED
        }
    val defaultLocal = localOptions.firstOrNull()
    val defaultRemote = remoteBackends.firstOrNull()

    // Early-out messages rendered as a compact card so the screen stays consistent.
    if (defaultLocal == null || defaultRemote == null) {
        Card(modifier = Modifier.fillMaxWidth()) {
            Text(
                text = when {
                    defaultLocal == null ->
                        "Install a verified on-device model to configure hybrid routing."
                    else ->
                        "Enable a confidential remote provider to pair with ${defaultLocal.third}."
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(16.dp),
            )
        }
        return
    }

    val profile = appState.hybridProfiles.firstOrNull()
    val local = localOptions.firstOrNull { it.second == profile?.localModelId } ?: defaultLocal
    val remote = remoteBackends.firstOrNull { it.id == profile?.remoteBackendId } ?: defaultRemote
    val remoteModel = profile?.remoteModelId?.takeIf { it in remote.models } ?: remote.models.first()

    // Commit the current selection as the (single) profile. Auto-creates on first change.
    val commit: (localBackend: BackendSummary, localModelId: String, remoteBackend: BackendSummary, remoteModelId: String) -> Unit =
        { lb, lm, rb, rm ->
            onDispatch(
                AppAction.SaveHybridProfile(
                    profile = defaultHybridProfile(
                        localBackend = lb,
                        localModelId = lm,
                        remoteBackend = rb,
                        remoteModelId = rm,
                        existingProfile = profile,
                    ),
                ),
            )
        }

    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(vertical = 8.dp)) {
            HybridSelectorRow(
                label = "Local model",
                value = local.third,
                options = localOptions.map { it.second to it.third },
                onSelect = { modelId ->
                    val lb = localOptions.first { it.second == modelId }.first
                    commit(lb, modelId, remote, remoteModel)
                },
            )
            HybridSelectorRow(
                label = "Remote backend",
                value = remote.name,
                options = remoteBackends.map { it.id to it.name },
                onSelect = { backendId ->
                    val rb = remoteBackends.first { it.id == backendId }
                    val rm = rb.models.first()
                    commit(local.first, local.second, rb, rm)
                },
            )
            HybridSelectorRow(
                label = "Remote model",
                value = compactModelName(remoteModel),
                options = remote.models.map { it to compactModelName(it) },
                onSelect = { modelId -> commit(local.first, local.second, remote, modelId) },
                last = true,
            )
        }
    }
}

@Composable
private fun HybridRoutingPolicyCard(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val profile = appState.hybridProfiles.firstOrNull() ?: return
    val isActive = appState.activeBackendId?.startsWith("hybrid:") == true
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(vertical = 8.dp)) {
            Text(
                "Routing rules",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            HybridPolicyRow(
                label = "Attachments to remote",
                description = "Send images and files to the confidential remote model.",
                checked = profile.policy.escalateIfAttachment,
                onCheckedChange = { v ->
                    onDispatch(
                        AppAction.SaveHybridProfile(
                            profile = profile.copy(
                                policy = profile.policy.copy(escalateIfAttachment = v),
                            ),
                        ),
                    )
                },
            )
            HybridPolicyRow(
                label = "Offline local fallback",
                description = "Use the on-device model when the remote is unreachable.",
                checked = profile.policy.preferLocalWhenOffline,
                onCheckedChange = { v ->
                    onDispatch(
                        AppAction.SaveHybridProfile(
                            profile = profile.copy(
                                policy = profile.policy.copy(preferLocalWhenOffline = v),
                            ),
                        ),
                    )
                },
            )
            HybridPolicyRow(
                label = "Long prompts to remote",
                description = "Escalate messages over ~4000 tokens to the remote model.",
                checked = profile.policy.escalateIfMessageLongerThan != null,
                onCheckedChange = { v ->
                    onDispatch(
                        AppAction.SaveHybridProfile(
                            profile = profile.copy(
                                policy = profile.policy.copy(
                                    escalateIfMessageLongerThan = if (v) 4000UL else null,
                                ),
                            ),
                        ),
                    )
                },
                last = true,
            )
            if (!isActive) {
                Text(
                    "Enable hybrid routing above to apply these rules.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
        }
    }
}

/** Compact selector row: label + value on the left, a "Change" chip on the right. */
@Composable
private fun HybridSelectorRow(
    label: String,
    value: String,
    options: List<Pair<String, String>>,
    onSelect: (String) -> Unit,
    last: Boolean = false,
) {
    var expanded by remember { mutableStateOf(false) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(enabled = options.size > 1) { expanded = true }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(value, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
        }
        if (options.size > 1) {
            Spacer(Modifier.width(12.dp))
            Box {
                Text(
                    "Change",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.clickable { expanded = true },
                )
                DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                    options.forEach { (id, name) ->
                        DropdownMenuItem(
                            text = { Text(name, style = MaterialTheme.typography.bodyMedium) },
                            onClick = { expanded = false; onSelect(id) },
                        )
                    }
                }
            }
        }
    }
    if (!last) {
        androidx.compose.material3.HorizontalDivider(
            modifier = Modifier.padding(start = 16.dp),
            color = MaterialTheme.colorScheme.outlineVariant,
        )
    }
}

/** Compact policy toggle row matching the on-device inference toggle density. */
@Composable
private fun HybridPolicyRow(
    label: String,
    description: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    last: Boolean = false,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
    if (!last) {
        androidx.compose.material3.HorizontalDivider(
            modifier = Modifier.padding(start = 16.dp),
            color = MaterialTheme.colorScheme.outlineVariant,
        )
    }
}

private fun defaultHybridProfile(
    localBackend: BackendSummary,
    localModelId: String,
    remoteBackend: BackendSummary,
    remoteModelId: String,
    existingProfile: HybridProfile?,
): HybridProfile {
    return HybridProfile(
        id = existingProfile?.id ?: "default_hybrid",
        name = "${localBackend.name} -> ${remoteBackend.name}",
        localBackendId = localBackend.id,
        localModelId = localModelId,
        remoteBackendId = remoteBackend.id,
        remoteModelId = remoteModelId,
        policy = existingProfile?.policy ?: RoutingPolicy(
            escalateIfAttachment = true,
            preferLocalWhenOffline = true,
            escalateIfMessageLongerThan = 4000UL,
        ),
        preprocessing = existingProfile?.preprocessing ?: LocalPreprocessing(
            compressHistory = false,
            rewriteRagQuery = false,
        ),
    )
}
