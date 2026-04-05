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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatus
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
    val presets = knownProviderPresets()

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
                val isEnabled = appState.backends.any { it.id == preset.id && it.hasApiKey }
                val backend   = appState.backends.find { it.id == preset.id }
                val att       = appState.attestationStatuses.find { it.backendId == preset.id }

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
                    Column(modifier = Modifier.padding(12.dp)) {
                        // Name + Enabled badge
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    preset.name,
                                    style = MaterialTheme.typography.bodyMedium,
                                    fontWeight = FontWeight.Medium
                                )
                                Text(
                                    teeTypeLabelProviders(preset.teeType),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            if (isEnabled) {
                                Surface(
                                    color = if (isDark) DarkHealthyDim else LightHealthyDim,
                                    shape = RoundedCornerShape(20.dp)
                                ) {
                                    Text(
                                        "Enabled",
                                        style = MaterialTheme.typography.labelSmall,
                                        fontWeight = FontWeight.SemiBold,
                                        color = if (isDark) DarkHealthy else LightHealthy,
                                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
                                    )
                                }
                            }
                        }

                        if (isEnabled && backend != null) {
                            Spacer(Modifier.height(6.dp))

                            // Health + attestation row
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

                            if (backend.models.isNotEmpty()) {
                                Spacer(Modifier.height(2.dp))
                                Text(
                                    backend.models.take(3).joinToString(" · "),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }

                            // Actions
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

                        } else if (!isEnabled) {
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
                                label = { Text("API Key") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                                shape = RoundedCornerShape(8.dp),
                                visualTransformation = PasswordVisualTransformation()
                            )
                            Spacer(Modifier.height(6.dp))
                            Button(
                                onClick = {
                                    val key = (presetKeys[preset.id] ?: "").trim()
                                    if (key.isNotEmpty()) {
                                        onDispatch(AppAction.AddBackendFromPreset(presetId = preset.id, apiKey = key))
                                        presetKeys[preset.id] = ""
                                    }
                                },
                                enabled = (presetKeys[preset.id] ?: "").isNotBlank(),
                                modifier = Modifier.fillMaxWidth(),
                                shape = RoundedCornerShape(8.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = if (isDark) DarkHealthy else LightHealthy)
                            ) { Text("Enable", color = Color.Black, fontWeight = FontWeight.Medium) }
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
