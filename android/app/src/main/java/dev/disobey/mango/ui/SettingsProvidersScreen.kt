package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.clickable
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.HorizontalDivider
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.AttestationStatusEntry
import dev.disobey.mango.rust.BackendSummary
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.TeeType
import dev.disobey.mango.rust.knownProviderPresets
import dev.disobey.mango.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsProvidersScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    val isDark = isSystemInDarkTheme()
    val presetKeys = remember { mutableStateMapOf<String, String>() }
    val expanded = remember { mutableStateMapOf<String, Boolean>() }
    val presets = knownProviderPresets()
        .filterNot { it.id == "qvac-local" }
    var addName by remember { mutableStateOf("") }
    var addUrl by remember { mutableStateOf("") }
    var addApiKey by remember { mutableStateOf("") }
    var addModel by remember { mutableStateOf("") }
    var showApiKey by remember { mutableStateOf(false) }
    var addTeeType by remember { mutableStateOf("IntelTdx") }
    var teeExpanded by remember { mutableStateOf(false) }
    var attestationInterval by remember { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Providers", fontWeight = FontWeight.Medium) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            // ── Providers header ──────────────────────────────────────────────
            item {
                Spacer(Modifier.height(8.dp))
                Text(
                    "PROVIDERS",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
            }

            items(presets) { preset ->
                val keyOptional = presetKeyOptionalProviders(preset.id, preset.teeType)
                val isEnabled = appState.backends.any {
                    it.id == preset.id && (it.hasApiKey || keyOptional)
                }
                val backend   = appState.backends.find { it.id == preset.id }
                val att       = appState.attestationStatuses.find { it.backendId == preset.id }
                val isOpen    = expanded[preset.id] == true

                Card(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    shape = RoundedCornerShape(10.dp),
                    elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
                    colors = CardDefaults.cardColors(
                        containerColor = if (isEnabled)
                            MaterialTheme.colorScheme.surface
                        else
                            MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
                    )
                ) {
                    Column {
                        // Compact header — always visible, tap to expand
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { expanded[preset.id] = !isOpen }
                                .padding(horizontal = 12.dp, vertical = 10.dp)
                        ) {
                            Text(
                                preset.name,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = FontWeight.Medium,
                                modifier = Modifier.weight(1f)
                            )
                            ProviderStatusPill(
                                isEnabled = isEnabled,
                                backend = backend,
                                att = att,
                                isDark = isDark
                            )
                            Spacer(Modifier.width(6.dp))
                            Icon(
                                imageVector = if (isOpen) Icons.Filled.KeyboardArrowUp else Icons.Filled.KeyboardArrowDown,
                                contentDescription = if (isOpen) "Collapse" else "Expand",
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.size(20.dp)
                            )
                        }

                        AnimatedVisibility(visible = isOpen) {
                            Column(modifier = Modifier.padding(start = 12.dp, end = 12.dp, bottom = 12.dp)) {
                                Text(
                                    teeTypeLabelProviders(preset.teeType),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )

                                if (isEnabled && backend != null) {
                                    Spacer(Modifier.height(6.dp))
                                    Row(
                                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                                        verticalAlignment = Alignment.CenterVertically
                                    ) {
                                        Surface(
                                            color = healthColorProviders(backend.healthStatus, isDark).copy(alpha = 0.12f),
                                            shape = RoundedCornerShape(20.dp)
                                        ) {
                                            Text(
                                                healthLabelProviders(backend.healthStatus),
                                                style = MaterialTheme.typography.labelSmall,
                                                fontWeight = FontWeight.Medium,
                                                color = healthColorProviders(backend.healthStatus, isDark),
                                                modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                                            )
                                        }
                                        if (att != null) {
                                            val (label, color) = attestationStyleProviders(att.status, isDark)
                                            Text(
                                                label,
                                                style = MaterialTheme.typography.labelSmall,
                                                color = color
                                            )
                                        }
                                    }

                                    // Phase 34.1: trust-UI sub-lines for Redpill freshness + orchestrated breakdown.
                                    // Copy LOCKED in 34.1-UI-SPEC.md. Sub-lines render only on Settings → Providers
                                    // expanded row, only when status is Verified with the relevant fields populated.
                                    val verified = att?.status as? AttestationStatus.Verified
                                    if (verified != null) {
                                        if (verified.freshness == "PerEnclave") {
                                            Spacer(Modifier.height(4.dp))
                                            Text(
                                                "Verified for this enclave instance",
                                                style = MaterialTheme.typography.labelSmall,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            )
                                        }
                                        val comps = verified.orchestratedComponents
                                        if (!comps.isNullOrEmpty()) {
                                            val labelMap = mapOf(
                                                "gateway" to "gateway",
                                                "model" to "model",
                                                "compose_manager" to "compose",
                                            )
                                            val line = comps.joinToString(separator = " • ") { c ->
                                                "${labelMap[c.label] ?: c.label} ✓"
                                            }
                                            Spacer(Modifier.height(4.dp))
                                            Text(
                                                line,
                                                style = MaterialTheme.typography.labelSmall,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            )
                                        }
                                    }

                                    if (backend.models.isNotEmpty()) {
                                        Spacer(Modifier.height(4.dp))
                                        Text(
                                            backend.models.take(3).joinToString(" · "),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant
                                        )
                                    }

                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        if (backend.isActive) {
                                            Surface(
                                                color = if (isDark) DarkHealthyDim else LightHealthyDim,
                                                shape = RoundedCornerShape(20.dp)
                                            ) {
                                                Text(
                                                    "Default",
                                                    style = MaterialTheme.typography.labelSmall,
                                                    fontWeight = FontWeight.SemiBold,
                                                    color = if (isDark) DarkHealthy else LightHealthy,
                                                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
                                                )
                                            }
                                        } else {
                                            TextButton(
                                                onClick = { onDispatch(AppAction.SetDefaultBackend(backendId = preset.id)) }
                                            ) { Text("Set Default", style = MaterialTheme.typography.labelMedium) }
                                        }
                                        Spacer(Modifier.weight(1f))
                                        TextButton(
                                            onClick = { onDispatch(AppAction.RemoveBackend(backendId = preset.id)) },
                                            colors = ButtonDefaults.textButtonColors(
                                                contentColor = MaterialTheme.colorScheme.error
                                            )
                                        ) { Text("Remove", style = MaterialTheme.typography.labelMedium) }
                                    }
                                } else {
                                    Spacer(Modifier.height(6.dp))
                                    Text(
                                        preset.description,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                    Spacer(Modifier.height(8.dp))
                                    OutlinedTextField(
                                        value = presetKeys[preset.id] ?: "",
                                        onValueChange = { presetKeys[preset.id] = it },
                                        label = { Text(if (keyOptional) "API Key (optional)" else "API Key") },
                                        modifier = Modifier.fillMaxWidth(),
                                        singleLine = true,
                                        shape = RoundedCornerShape(8.dp),
                                        visualTransformation = PasswordVisualTransformation()
                                    )
                                    Spacer(Modifier.height(6.dp))
                                    Button(
                                        onClick = {
                                            val key = (presetKeys[preset.id] ?: "").trim()
                                            if (key.isNotEmpty() || keyOptional) {
                                                onDispatch(AppAction.AddBackendFromPreset(presetId = preset.id, apiKey = key))
                                                presetKeys[preset.id] = ""
                                            }
                                        },
                                        enabled = keyOptional || (presetKeys[preset.id] ?: "").isNotBlank(),
                                        modifier = Modifier.fillMaxWidth(),
                                        shape = RoundedCornerShape(8.dp),
                                        colors = ButtonDefaults.buttonColors(containerColor = if (isDark) DarkHealthy else LightHealthy)
                                    ) { Text("Enable", color = Color.Black, fontWeight = FontWeight.Medium) }
                                }
                            }
                        }
                    }
                }
            }

            item {
                Spacer(Modifier.height(16.dp))
                Text(
                    "PROVIDER DEFAULTS",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(10.dp),
                    elevation = CardDefaults.cardElevation(defaultElevation = 0.dp)
                ) {
                    Column(
                        modifier = Modifier.padding(14.dp),
                        verticalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        Text(
                            "Re-attestation interval",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Medium
                        )
                        Text(
                            "How often the active provider is automatically re-attested. Set 0 to disable.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            OutlinedTextField(
                                value = if (attestationInterval.isEmpty()) {
                                    appState.attestationIntervalMinutes.toString()
                                } else {
                                    attestationInterval
                                },
                                onValueChange = { attestationInterval = it },
                                label = { Text("Minutes") },
                                modifier = Modifier.weight(1f),
                                singleLine = true,
                            )
                            Button(
                                onClick = {
                                    val value = attestationInterval.trim().toUIntOrNull()
                                    if (value != null) {
                                        onDispatch(AppAction.SetAttestationInterval(minutes = value))
                                        attestationInterval = ""
                                    }
                                },
                                enabled = attestationInterval.trim().toUIntOrNull() != null
                            ) {
                                Text("Apply")
                            }
                        }

                        HorizontalDivider()

                        Text(
                            "Custom provider",
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Medium
                        )
                        Text(
                            "For self-hosted or experimental confidential inference endpoints.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        OutlinedTextField(
                            value = addName,
                            onValueChange = { addName = it },
                            label = { Text("Name") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        OutlinedTextField(
                            value = addUrl,
                            onValueChange = { addUrl = it },
                            label = { Text("Base URL") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        OutlinedTextField(
                            value = addApiKey,
                            onValueChange = { addApiKey = it },
                            label = { Text("API Key") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            visualTransformation = if (showApiKey) VisualTransformation.None
                            else PasswordVisualTransformation(),
                            trailingIcon = {
                                TextButton(onClick = { showApiKey = !showApiKey }) {
                                    Text(if (showApiKey) "Hide" else "Show")
                                }
                            }
                        )
                        OutlinedTextField(
                            value = addModel,
                            onValueChange = { addModel = it },
                            label = { Text("Model ID") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        ExposedDropdownMenuBox(
                            expanded = teeExpanded,
                            onExpandedChange = { teeExpanded = it },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            OutlinedTextField(
                                value = teeTypeLabelProviders(parseTeeTypeProviders(addTeeType)),
                                onValueChange = {},
                                readOnly = true,
                                label = { Text("TEE Type") },
                                trailingIcon = {
                                    ExposedDropdownMenuDefaults.TrailingIcon(expanded = teeExpanded)
                                },
                                modifier = Modifier.menuAnchor().fillMaxWidth()
                            )
                            DropdownMenu(
                                expanded = teeExpanded,
                                onDismissRequest = { teeExpanded = false }
                            ) {
                                listOf("IntelTdx", "NvidiaH100Cc", "AmdSevSnp", "Unknown").forEach { option ->
                                    DropdownMenuItem(
                                        text = { Text(teeTypeLabelProviders(parseTeeTypeProviders(option))) },
                                        onClick = {
                                            addTeeType = option
                                            teeExpanded = false
                                        }
                                    )
                                }
                            }
                        }
                        Button(
                            onClick = {
                                onDispatch(
                                    AppAction.AddBackend(
                                        name = addName,
                                        baseUrl = addUrl,
                                        apiKey = addApiKey,
                                        teeType = parseTeeTypeProviders(addTeeType),
                                        models = listOf(addModel.trim()).filter { it.isNotEmpty() },
                                    )
                                )
                                addName = ""
                                addUrl = ""
                                addApiKey = ""
                                addModel = ""
                                addTeeType = "IntelTdx"
                            },
                            enabled = addName.isNotBlank() && addUrl.isNotBlank() && addApiKey.isNotBlank() && addModel.isNotBlank(),
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Add Provider")
                        }
                    }
                }
            }

            item { Spacer(Modifier.height(32.dp)) }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

private fun healthLabelProviders(s: HealthStatus): String = when (s) {
    HealthStatus.HEALTHY  -> "Healthy"
    HealthStatus.DEGRADED -> "Degraded"
    HealthStatus.FAILED   -> "Failed"
    HealthStatus.UNKNOWN  -> "Unknown"
}

private fun healthColorProviders(s: HealthStatus, isDark: Boolean): Color = when (s) {
    HealthStatus.HEALTHY  -> if (isDark) DarkHealthy else LightHealthy
    HealthStatus.DEGRADED -> if (isDark) DarkDegraded else LightDegraded
    HealthStatus.FAILED   -> if (isDark) DarkFailed else LightFailed
    HealthStatus.UNKNOWN  -> if (isDark) DarkHealthUnknown else LightHealthUnknown
}

private fun attestationStyleProviders(s: AttestationStatus, isDark: Boolean): Pair<String, Color> = when (s) {
    is AttestationStatus.Verified    -> "Attested"       to (if (isDark) DarkHealthy else LightHealthy)
    is AttestationStatus.Unverified  -> "Unverified"     to (if (isDark) DarkHealthUnknown else LightHealthUnknown)
    is AttestationStatus.Failed      -> "Attest Failed"  to (if (isDark) DarkFailed else LightFailed)
    is AttestationStatus.Expired     -> "Attest Expired" to (if (isDark) DarkDegraded else LightDegraded)
}

private fun teeTypeLabelProviders(t: TeeType): String = when (t) {
    TeeType.INTEL_TDX      -> "Intel TDX"
    TeeType.NVIDIA_H100_CC -> "NVIDIA H100 CC"
    TeeType.AMD_SEV_SNP    -> "AMD SEV-SNP"
    TeeType.UNKNOWN        -> "Unknown"
}

private fun presetKeyOptionalProviders(id: String, teeType: TeeType): Boolean {
    return id == "qvac-local" || teeType == TeeType.UNKNOWN
}

private fun parseTeeTypeProviders(value: String): TeeType = when (value) {
    "NvidiaH100Cc" -> TeeType.NVIDIA_H100_CC
    "AmdSevSnp" -> TeeType.AMD_SEV_SNP
    "Unknown" -> TeeType.UNKNOWN
    else -> TeeType.INTEL_TDX
}

@Composable
private fun ProviderStatusPill(
    isEnabled: Boolean,
    backend: BackendSummary?,
    att: AttestationStatusEntry?,
    isDark: Boolean,
) {
    if (!isEnabled || backend == null) {
        Text(
            "Disabled",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        return
    }
    val (label, color) = when {
        att?.status is AttestationStatus.Failed ->
            "Attest Failed" to (if (isDark) DarkFailed else LightFailed)
        att?.status is AttestationStatus.Expired ->
            "Attest Expired" to (if (isDark) DarkDegraded else LightDegraded)
        backend.healthStatus == HealthStatus.FAILED ->
            "Failed" to (if (isDark) DarkFailed else LightFailed)
        backend.healthStatus == HealthStatus.DEGRADED ->
            "Degraded" to (if (isDark) DarkDegraded else LightDegraded)
        att?.status is AttestationStatus.Verified ->
            "Attested" to (if (isDark) DarkHealthy else LightHealthy)
        backend.healthStatus == HealthStatus.HEALTHY ->
            "Healthy" to (if (isDark) DarkHealthy else LightHealthy)
        else ->
            "Enabled" to (if (isDark) DarkHealthUnknown else LightHealthUnknown)
    }
    Surface(color = color.copy(alpha = 0.12f), shape = RoundedCornerShape(20.dp)) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.Medium,
            color = color,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
        )
    }
}
