package dev.disobey.mango.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.AltRoute
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Help
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.Dns
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.ui.draw.shadow
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.InferenceRoute
import dev.disobey.mango.rust.InferenceStatus
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.ui.theme.MangoTheme

private enum class HeroIssueTone { ERROR, WARN }

private data class HeroIssue(val message: String, val icon: ImageVector, val tone: HeroIssueTone)

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InferenceStatusCard(appState: AppState, onDispatch: (AppAction) -> Unit, modifier: Modifier = Modifier) {
    val status = appState.inferenceStatus
    val c = MangoTheme.colors
    val onDevice = status.route == InferenceRoute.ON_DEVICE
    val attestation = status.attestedBackendId?.let { id ->
        appState.attestationStatuses.firstOrNull { it.backendId == id }?.status ?: AttestationStatus.Unverified
    }
    val issue = statusIssue(status, appState, attestation)

    Column(modifier = modifier.fillMaxWidth()) {
        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = MaterialTheme.shapes.large,
            color = Color.Transparent,
            contentColor = c.onAccent,
        ) {
            Column(Modifier.clip(MaterialTheme.shapes.large).background(c.accentGradient).padding(18.dp)) {
                Text(
                    "New chats use",
                    style = MaterialTheme.typography.labelMedium,
                    color = c.onAccent.copy(alpha = 0.75f),
                )
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
                // FlowRow so a long provider/profile name wraps instead of starving
                // the verification chip; the badge keeps its own Trust Status dialog.
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.padding(top = 6.dp),
                ) {
                    if (onDevice) {
                        HeroChip(Icons.Outlined.PhoneAndroid, "Never leaves this phone")
                    } else {
                        if (status.backendName.isNotBlank()) HeroChip(Icons.Outlined.Dns, status.backendName)
                        if (attestation is AttestationStatus.Verified) {
                            AttestationBadge(attestation)
                        }
                    }
                    if (status.route == InferenceRoute.HYBRID) {
                        status.localModelName?.let { HeroChip(Icons.Outlined.PhoneAndroid, "+ $it locally") }
                    }
                }
                RoutePillSelector(
                    selected = status.route,
                    hybridAvailable = status.hybridAvailable,
                    onDeviceAvailable = status.onDeviceAvailable,
                    onSelect = { route -> if (route != status.route) onDispatch(AppAction.SetInferenceRoute(route = route)) },
                    modifier = Modifier.padding(top = 14.dp),
                )
            }
        }
        issue?.let { HeroIssueRow(it, onFix = { onDispatch(AppAction.PushScreen(screen = Screen.SettingsProviders)) }) }
    }
}

private fun statusIssue(
    status: InferenceStatus,
    appState: AppState,
    attestation: AttestationStatus?,
): HeroIssue? {
    // Hybrid routes encode `hybrid:<profile>` in backendId; health belongs to the
    // concrete remote provider surfaced as attestedBackendId.
    val backend = appState.backends.firstOrNull { it.id == (status.attestedBackendId ?: status.backendId) }
    val name = status.backendName.takeIf { it.isNotBlank() } ?: backend?.name ?: "The backend"
    return when {
        status.route == InferenceRoute.ON_DEVICE -> null
        backend?.healthStatus == HealthStatus.FAILED ->
            HeroIssue("$name didn't answer the health check", Icons.Filled.Warning, HeroIssueTone.ERROR)
        attestation is AttestationStatus.Failed ->
            HeroIssue("$name attestation failed — ${attestation.reason}", Icons.Filled.Warning, HeroIssueTone.ERROR)
        backend?.healthStatus == HealthStatus.DEGRADED ->
            HeroIssue("$name health is degraded", Icons.Filled.Warning, HeroIssueTone.WARN)
        attestation is AttestationStatus.Expired ->
            HeroIssue("$name attestation expired — re-verify it", Icons.Filled.Schedule, HeroIssueTone.WARN)
        attestation is AttestationStatus.Unverified ->
            HeroIssue("$name isn't attested yet", Icons.Filled.Help, HeroIssueTone.WARN)
        else -> null
    }
}

/** Translucent pill on the gradient hero (white wash in light, dark wash in dark). */
@Composable
private fun HeroChip(icon: ImageVector, text: String) {
    val c = MangoTheme.colors
    val wash = if (c.isDark) Color.Black.copy(alpha = 0.18f) else Color.White.copy(alpha = 0.55f)
    Row(
        modifier = Modifier
            .clip(CircleShape)
            .background(wash)
            .padding(horizontal = 9.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Icon(icon, contentDescription = null, tint = c.onAccent, modifier = Modifier.size(12.dp))
        Text(text, style = MaterialTheme.typography.labelMedium, color = c.onAccent, maxLines = 1)
    }
}

/** Health/attestation failure below the hero, with a "Fix" path to Providers. */
@Composable
private fun HeroIssueRow(issue: HeroIssue, onFix: () -> Unit) {
    val c = MangoTheme.colors
    val (bg, fg) = when (issue.tone) {
        HeroIssueTone.ERROR -> MaterialTheme.colorScheme.errorContainer to MaterialTheme.colorScheme.onErrorContainer
        HeroIssueTone.WARN -> c.warnContainer to c.warn
    }
    Surface(
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
        shape = MaterialTheme.shapes.small,
        color = bg,
    ) {
        Row(
            modifier = Modifier.padding(start = 12.dp, top = 4.dp, bottom = 4.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(issue.icon, contentDescription = null, tint = fg, modifier = Modifier.size(18.dp))
            Text(
                issue.message,
                style = MaterialTheme.typography.bodyMedium,
                color = fg,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onFix, modifier = Modifier.heightIn(min = 48.dp)) {
                Text("Fix", color = fg, style = MaterialTheme.typography.labelLarge)
            }
        }
    }
}

/** Pill segmented control: dark translucent track, raised surface thumb on the selection.
 * Unavailable segments stay tappable: the core answers with a toast and opens the setup screen. */
@Composable
private fun RoutePillSelector(
    selected: InferenceRoute,
    hybridAvailable: Boolean,
    onDeviceAvailable: Boolean,
    onSelect: (InferenceRoute) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = MangoTheme.colors
    val options = listOf(
        Triple(InferenceRoute.CLOUD, "Cloud TEE", Icons.Outlined.Cloud),
        Triple(InferenceRoute.HYBRID, "Hybrid", if (hybridAvailable) Icons.AutoMirrored.Outlined.AltRoute else Icons.Outlined.Add),
        Triple(InferenceRoute.ON_DEVICE, "On-device", if (onDeviceAvailable) Icons.Outlined.PhoneAndroid else Icons.Outlined.Add),
    )
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(CircleShape)
            .background(Color.Black.copy(alpha = 0.12f))
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        options.forEach { (route, label, icon) ->
            val isSelected = route == selected
            val bg = if (isSelected) MaterialTheme.colorScheme.surface else Color.Transparent
            val fg = if (isSelected) MaterialTheme.colorScheme.onSurface else c.onAccent
            Box(
                modifier = Modifier
                    .weight(1f)
                    .heightIn(min = 56.dp)
                    .then(if (isSelected) Modifier.shadow(2.dp, CircleShape) else Modifier)
                    .clip(CircleShape)
                    .background(bg)
                    .clickable(role = Role.Button) { onSelect(route) }
                    .semantics { contentDescription = "Run new chats: $label" },
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Icon(icon, contentDescription = null, tint = fg, modifier = Modifier.size(15.dp))
                    Text(
                        label,
                        style = MaterialTheme.typography.labelMedium,
                        color = fg,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                        textAlign = TextAlign.Center,
                    )
                }
            }
        }
    }
}

