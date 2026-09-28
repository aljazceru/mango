package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.AltRoute
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.InferenceRoute
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.ui.theme.*

@Composable
fun InferenceStatusCard(appState: AppState, onDispatch: (AppAction) -> Unit, modifier: Modifier = Modifier) {
    val status = appState.inferenceStatus
    val dark = isDarkScheme()
    val bg = if (dark) StatusCardBgDark else StatusCardBgLight
    val on = if (dark) StatusCardOnDark else StatusCardOnLight
    val onDevice = status.route == InferenceRoute.ON_DEVICE
    val attestation = status.attestedBackendId?.let { id ->
        appState.attestationStatuses.firstOrNull { it.backendId == id }?.status ?: AttestationStatus.Unverified
    }

    Surface(modifier = modifier.fillMaxWidth(), shape = RoundedCornerShape(26.dp), color = bg, contentColor = on) {
        Column(Modifier.padding(18.dp)) {
            Text("New chats use", style = MaterialTheme.typography.labelMedium, color = on.copy(alpha = 0.8f))
            Row(
                modifier = Modifier
                    .clickable(onClickLabel = "Change default model") {
                        onDispatch(AppAction.PushScreen(screen = Screen.SettingsDefaults))
                    }
                    .padding(vertical = 2.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    statusCardModelLabel(onDevice, status.modelId, status.localModelName),
                    style = MaterialTheme.typography.headlineSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f, fill = false),
                )
                Icon(Icons.Filled.ExpandMore, contentDescription = null)
            }
            Row(
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.padding(top = 6.dp, bottom = 14.dp),
            ) {
                when {
                    onDevice -> StatusChip("Never leaves this phone")
                    attestation != null -> AttestationBadge(status = attestation)
                }
                if (!onDevice && status.backendName.isNotBlank()) StatusChip(status.backendName)
                if (status.route == InferenceRoute.HYBRID) status.localModelName?.let { StatusChip("+ $it locally") }
            }
            Text("Where inference runs", style = MaterialTheme.typography.labelMedium, color = on.copy(alpha = 0.8f))
            Spacer(Modifier.height(6.dp))
            InferenceRouteSelector(
                selected = status.route,
                hybridAvailable = status.hybridAvailable,
                onDeviceAvailable = status.onDeviceAvailable,
                onSelect = { route -> if (route != status.route) onDispatch(AppAction.SetInferenceRoute(route = route)) },
            )
        }
    }
}

@Composable
private fun StatusChip(text: String) {
    Surface(shape = RoundedCornerShape(8.dp), color = MaterialTheme.colorScheme.surface) {
        Text(text, style = MaterialTheme.typography.labelSmall, modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp))
    }
}

/** Unavailable segments stay tappable: the core answers with a toast and opens the setup screen. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun InferenceRouteSelector(
    selected: InferenceRoute,
    hybridAvailable: Boolean,
    onDeviceAvailable: Boolean,
    onSelect: (InferenceRoute) -> Unit,
) {
    val options = listOf(
        Triple(InferenceRoute.CLOUD, "Cloud TEE", Icons.Outlined.Cloud),
        Triple(InferenceRoute.HYBRID, "Hybrid", if (hybridAvailable) Icons.AutoMirrored.Outlined.AltRoute else Icons.Outlined.Add),
        Triple(InferenceRoute.ON_DEVICE, "On-device", if (onDeviceAvailable) Icons.Outlined.PhoneAndroid else Icons.Outlined.Add),
    )
    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
        options.forEachIndexed { i, (route, label, icon) ->
            SegmentedButton(
                selected = route == selected,
                onClick = { onSelect(route) },
                shape = SegmentedButtonDefaults.itemShape(index = i, count = options.size),
                icon = { Icon(icon, contentDescription = null, modifier = Modifier.size(18.dp)) },
                colors = SegmentedButtonDefaults.colors(inactiveContainerColor = MaterialTheme.colorScheme.surface),
                modifier = Modifier.semantics { contentDescription = "Run new chats: $label" },
            ) { Text(label, maxLines = 1) }
        }
    }
}
