package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.BackendSummary
import dev.disobey.mango.rust.HybridProfile
import dev.disobey.mango.rust.InferenceRoute
import dev.disobey.mango.rust.LocalPreprocessing
import dev.disobey.mango.rust.RoutingPolicy
import dev.disobey.mango.ui.theme.TintPair

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsHybridRoutingScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
) {
    val isActive = appState.activeBackendId?.startsWith("hybrid:") == true
    val hasProfile = appState.hybridProfiles.isNotEmpty()
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Hybrid routing") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    Switch(
                        modifier = Modifier.semantics { contentDescription = "Use hybrid for new chats" },
                        checked = isActive,
                        enabled = hasProfile,
                        onCheckedChange = { enable ->
                            onDispatch(
                                AppAction.SetInferenceRoute(
                                    route = if (enable) InferenceRoute.HYBRID else InferenceRoute.CLOUD,
                                ),
                            )
                        },
                    )
                    Spacer(Modifier.width(12.dp))
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
            item { HybridDiagramCard(appState = appState) }
            item {
                SectionLabel(
                    "Send to the enclave when…",
                    Modifier.padding(start = 8.dp, top = 6.dp, bottom = 2.dp),
                )
            }
            item { HybridRoutingPolicyCard(appState = appState, onDispatch = onDispatch) }
            item { HybridRoutingConfigCard(appState = appState, onDispatch = onDispatch) }
            item { Spacer(Modifier.height(14.dp)) }
        }
    }
}

private fun localModelOptions(appState: AppState): List<Triple<BackendSummary, String, String>> =
    appState.backends
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

private fun remoteTeeBackends(appState: AppState): List<BackendSummary> =
    appState.backends
        .filter { backend ->
            !isOnDeviceBackend(backend.id) &&
                backend.teeType != dev.disobey.mango.rust.TeeType.UNKNOWN &&
                backend.hasApiKey &&
                backend.models.isNotEmpty() &&
                backend.healthStatus != dev.disobey.mango.rust.HealthStatus.FAILED
        }

/** Phone ↔ enclave diagram; lanes reflect the active routing policy. */
@Composable
private fun HybridDiagramCard(appState: AppState) {
    val localOptions = localModelOptions(appState)
    val remoteBackends = remoteTeeBackends(appState)
    val profile = appState.hybridProfiles.firstOrNull()
    val local = localOptions.firstOrNull { it.second == profile?.localModelId } ?: localOptions.firstOrNull()
    val remote = remoteBackends.firstOrNull { it.id == profile?.remoteBackendId } ?: remoteBackends.firstOrNull()

    if (local == null || remote == null) {
        MangoCard(modifier = Modifier.fillMaxWidth()) {
            Text(
                text = when {
                    local == null -> "Install a verified on-device model to configure hybrid routing."
                    else -> "Enable a confidential remote provider to pair with ${local.third}."
                },
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(16.dp),
            )
        }
        return
    }

    val remoteModel = profile?.remoteModelId?.takeIf { it in remote.models } ?: remote.models.first()
    val policy = profile?.policy ?: RoutingPolicy(
        escalateIfAttachment = true,
        preferLocalWhenOffline = true,
        escalateIfMessageLongerThan = 4000UL,
    )
    val lanes = buildList {
        add("default")
        if (policy.escalateIfAttachment && policy.escalateIfMessageLongerThan != null) add("files, long →")
        else if (policy.escalateIfAttachment) add("files →")
        else if (policy.escalateIfMessageLongerThan != null) add("long →")
        if (policy.preferLocalWhenOffline) add("← offline")
    }

    MangoCard(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            DiagramNode(
                icon = RouteKind.LOCAL.icon,
                tint = RouteKind.LOCAL.tint,
                name = local.third,
                sub = "this phone",
                modifier = Modifier.width(96.dp),
            )
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                lanes.forEach { lane -> WireLane(lane) }
            }
            DiagramNode(
                icon = RouteKind.TEE.icon,
                tint = RouteKind.TEE.tint,
                name = compactModelName(remoteModel),
                sub = remote.name,
                modifier = Modifier.width(96.dp),
            )
        }
    }
}

@Composable
private fun DiagramNode(
    icon: ImageVector,
    tint: TintPair,
    name: String,
    sub: String,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        TintedIcon(icon, tint, size = 58.dp)
        Text(
            name,
            style = MaterialTheme.typography.labelLarge,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 6.dp),
        )
        Text(
            sub,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun WireLane(label: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        HorizontalDivider(modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.outlineVariant)
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
        )
        HorizontalDivider(modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.outlineVariant)
    }
}

@Composable
private fun HybridRoutingPolicyCard(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val profile = appState.hybridProfiles.firstOrNull() ?: return
    val isActive = appState.activeBackendId?.startsWith("hybrid:") == true
    MangoCard(modifier = Modifier.fillMaxWidth()) {
        HybridPolicyRow(
            label = "A message has attachments",
            tagIcon = Icons.Outlined.Cloud,
            tag = "images & files go to the enclave",
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
        HybridDivider()
        HybridPolicyRow(
            label = "A prompt is long",
            tagIcon = Icons.Outlined.Cloud,
            tag = "over ~4,000 tokens",
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
        )
        HybridDivider()
        HybridPolicyRow(
            label = "Fall back offline",
            tagIcon = Icons.Outlined.PhoneAndroid,
            tag = "use the phone when the enclave is unreachable",
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
        if (!isActive) {
            Text(
                "Enable hybrid routing to apply these rules.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
    }
}

@Composable
private fun HybridRoutingConfigCard(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
) {
    val localOptions = localModelOptions(appState)
    val remoteBackends = remoteTeeBackends(appState)
    val defaultLocal = localOptions.firstOrNull()
    val defaultRemote = remoteBackends.firstOrNull()
    if (defaultLocal == null || defaultRemote == null) return

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

    MangoCard(modifier = Modifier.fillMaxWidth()) {
        HybridSelectorRow(
            title = "Enclave provider",
            value = remote.name,
            options = remoteBackends.map { it.id to it.name },
            onSelect = { backendId ->
                val rb = remoteBackends.first { it.id == backendId }
                commit(local.first, local.second, rb, rb.models.first())
            },
        )
        HybridDivider()
        HybridSelectorRow(
            title = "Enclave model",
            value = compactModelName(remoteModel),
            options = remote.models.map { it to compactModelName(it) },
            onSelect = { modelId -> commit(local.first, local.second, remote, modelId) },
        )
        HybridDivider()
        HybridSelectorRow(
            title = "Local model",
            value = local.third,
            options = localOptions.map { it.second to it.third },
            onSelect = { modelId ->
                val lb = localOptions.first { it.second == modelId }.first
                commit(lb, modelId, remote, remoteModel)
            },
        )
    }
}

@Composable
private fun HybridDivider() {
    HorizontalDivider(
        modifier = Modifier.padding(start = 16.dp),
        color = MaterialTheme.colorScheme.outlineVariant,
    )
}

/** Title + current value row opening a dropdown of options. */
@Composable
private fun HybridSelectorRow(
    title: String,
    value: String,
    options: List<Pair<String, String>>,
    onSelect: (String) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable { expanded = true }
                .padding(horizontal = 16.dp, vertical = 13.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, style = MaterialTheme.typography.bodyLarge)
                Text(
                    value,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Icon(
                Icons.AutoMirrored.Filled.KeyboardArrowRight,
                contentDescription = "Change $title",
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(20.dp),
            )
        }
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

/** Policy toggle row: bold rule, a direction tag, accent switch. */
@Composable
private fun HybridPolicyRow(
    label: String,
    tagIcon: ImageVector,
    tag: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Icon(
                    tagIcon,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(13.dp),
                )
                Text(
                    tag,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Switch(checked = checked, onCheckedChange = onCheckedChange)
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
