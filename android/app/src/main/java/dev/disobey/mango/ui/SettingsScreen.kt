package dev.disobey.mango.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Switch
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.isSystemInDarkTheme
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
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.TeeType
import dev.disobey.mango.rust.knownProviderPresets
import dev.disobey.mango.ui.theme.*

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
    themeMode: String = "system",
    onThemeModeChanged: (String) -> Unit = {},
) {
    val isDark              = isSystemInDarkTheme()
    val presetKeys          = remember { mutableStateMapOf<String, String>() }
    var showAdvanced        by remember { mutableStateOf(false) }
    var addName             by remember { mutableStateOf("") }
    var addUrl              by remember { mutableStateOf("") }
    var addApiKey           by remember { mutableStateOf("") }
    var showApiKey          by remember { mutableStateOf(false) }
    var addTeeType          by remember { mutableStateOf("IntelTdx") }
    var teeExpanded         by remember { mutableStateOf(false) }
    var attestationInterval by remember { mutableStateOf("") }
    var defaultModelExp     by remember { mutableStateOf(false) }
    var defaultModel        by remember { mutableStateOf("") }
    var defaultInstructions by remember { mutableStateOf(appState.globalSystemPrompt ?: "") }
    var braveApiKeyInput   by remember { mutableStateOf("") }
    var themeExpanded by remember { mutableStateOf(false) }
    val themeOptions = listOf("system" to "Follow System", "light" to "Force Light", "dark" to "Force Dark")
    val themeLabel = themeOptions.firstOrNull { it.first == themeMode }?.second ?: "Follow System"

    val teeOptions  = listOf("IntelTdx", "NvidiaH100Cc", "AmdSevSnp", "Unknown")
    // Aggregate (modelId, backendName) pairs from all non-failed backends so the
    // default model picker shows TEE models from every configured provider with
    // the provider name annotated. Do not deduplicate — preserve provider identity.
    val allModelEntries: List<Pair<String, String>> = appState.backends
        .filter { it.healthStatus != HealthStatus.FAILED && it.models.isNotEmpty() }
        .flatMap { backend -> backend.models.map { modelId -> Pair(modelId, backend.name) } }
        .sortedBy { (modelId, _) -> modelId }
    val presets     = knownProviderPresets()
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
                                    teeTypeLabel(preset.teeType),
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
                                    color = healthColor(backend.healthStatus, isDark).copy(alpha = 0.12f),
                                    shape = RoundedCornerShape(20.dp)
                                ) {
                                    Text(
                                        healthLabel(backend.healthStatus),
                                        style = MaterialTheme.typography.labelSmall,
                                        fontWeight = FontWeight.Medium,
                                        color = healthColor(backend.healthStatus, isDark),
                                        modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                                    )
                                }
                                if (att != null) {
                                    val (label, color) = attestationStyle(att.status, isDark)
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

            // ── Defaults ──────────────────────────────────────────────────────
            item {
                Spacer(Modifier.height(16.dp))
                Text(
                    "DEFAULTS",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                if (allModelEntries.isEmpty()) {
                    Text(
                        "Enable a provider to select a default model.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(vertical = 4.dp)
                    )
                } else {
                    ExposedDropdownMenuBox(
                        expanded = defaultModelExp,
                        onExpandedChange = { defaultModelExp = it },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        OutlinedTextField(
                            value = if (defaultModel.isEmpty()) "Select default model" else defaultModel,
                            onValueChange = {},
                            readOnly = true,
                            label = { Text("Default Model") },
                            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = defaultModelExp) },
                            shape = RoundedCornerShape(8.dp),
                            modifier = Modifier.menuAnchor().fillMaxWidth()
                        )
                        ExposedDropdownMenu(
                            expanded = defaultModelExp,
                            onDismissRequest = { defaultModelExp = false }
                        ) {
                            allModelEntries.forEach { (modelId, backendName) ->
                                DropdownMenuItem(
                                    text = {
                                        Column {
                                            Text(
                                                text = modelId,
                                                style = MaterialTheme.typography.bodyMedium,
                                                fontWeight = if (modelId == defaultModel)
                                                    FontWeight.Bold else FontWeight.Normal,
                                            )
                                            Text(
                                                text = backendName,
                                                style = MaterialTheme.typography.labelSmall,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                            )
                                        }
                                    },
                                    onClick = {
                                        defaultModel = modelId
                                        defaultModelExp = false
                                        onDispatch(AppAction.SetDefaultModel(modelId = modelId))
                                    }
                                )
                            }
                        }
                    }
                }

                Spacer(Modifier.height(12.dp))
                Text(
                    "Default Instructions",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                Text(
                    "Fallback system prompt used when a conversation has no custom instructions.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                OutlinedTextField(
                    value = defaultInstructions,
                    onValueChange = { defaultInstructions = it },
                    label = { Text("Default Instructions") },
                    modifier = Modifier.fillMaxWidth().height(120.dp),
                    shape = RoundedCornerShape(8.dp),
                    maxLines = 6
                )
                Spacer(Modifier.height(6.dp))
                Button(
                    onClick = {
                        val trimmed = defaultInstructions.trim()
                        onDispatch(AppAction.SetGlobalSystemPrompt(
                            prompt = if (trimmed.isEmpty()) null else trimmed
                        ))
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(8.dp)
                ) { Text("Save Instructions") }
            }

            // ── Memory ───────────────────────────────────────────────────────
            item {
                Spacer(Modifier.height(16.dp))
                Text(
                    "MEMORY",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(10.dp),
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            "Auto-extract Memories",
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium,
                            modifier = Modifier.weight(1f)
                        )
                        Switch(
                            checked = appState.memoriesEnabled,
                            onCheckedChange = { checked ->
                                onDispatch(AppAction.SetMemoriesEnabled(enabled = checked))
                            }
                        )
                    }
                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))
                    Row(
                        modifier = Modifier
                            .clickable { onDispatch(AppAction.PushScreen(screen = Screen.Memories)) }
                            .padding(16.dp)
                            .fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            "Memories",
                            style = MaterialTheme.typography.bodyMedium,
                            fontWeight = FontWeight.Medium
                        )
                        Spacer(Modifier.weight(1f))
                        if (appState.memoryCount > 0UL) {
                            Text(
                                appState.memoryCount.toString(),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Spacer(Modifier.width(8.dp))
                        }
                        Icon(
                            Icons.AutoMirrored.Filled.KeyboardArrowRight,
                            contentDescription = "View memories",
                            modifier = Modifier.size(16.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }

            // ── Tools ────────────────────────────────────────────────────────
            item {
                Spacer(Modifier.height(16.dp))
                Text(
                    "TOOLS",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(10.dp),
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                "Web Search",
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = FontWeight.Medium
                            )
                            Spacer(Modifier.weight(1f))
                            if (appState.braveApiKeySet) {
                                Text(
                                    "Configured",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }
                        Spacer(Modifier.height(4.dp))
                        Text(
                            "Required for agent web search. Keys are stored locally and never sent to third parties.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(Modifier.height(8.dp))
                        OutlinedTextField(
                            value = braveApiKeyInput,
                            onValueChange = { braveApiKeyInput = it },
                            label = {
                                Text(
                                    if (appState.braveApiKeySet)
                                        "Key configured — enter new key to update"
                                    else
                                        "Enter Brave Search API Key"
                                )
                            },
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(8.dp),
                            singleLine = true,
                            visualTransformation = PasswordVisualTransformation()
                        )
                        Spacer(Modifier.height(8.dp))
                        Button(
                            onClick = {
                                val trimmed = braveApiKeyInput.trim()
                                if (trimmed.isNotEmpty()) {
                                    onDispatch(AppAction.SetBraveApiKey(apiKey = trimmed))
                                    braveApiKeyInput = ""
                                }
                            },
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(8.dp),
                            enabled = braveApiKeyInput.trim().isNotEmpty()
                        ) { Text("Save API Key") }
                    }
                }
            }

            // ── Appearance ────────────────────────────────────────────────────
            item {
                Spacer(Modifier.height(16.dp))
                Text(
                    "APPEARANCE",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                ExposedDropdownMenuBox(
                    expanded = themeExpanded,
                    onExpandedChange = { themeExpanded = it },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    OutlinedTextField(
                        value = themeLabel,
                        onValueChange = {},
                        readOnly = true,
                        label = { Text("Theme") },
                        trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = themeExpanded) },
                        shape = RoundedCornerShape(8.dp),
                        modifier = Modifier.menuAnchor().fillMaxWidth()
                    )
                    ExposedDropdownMenu(
                        expanded = themeExpanded,
                        onDismissRequest = { themeExpanded = false }
                    ) {
                        themeOptions.forEach { (value, label) ->
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        text = label,
                                        style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = if (value == themeMode) FontWeight.Bold else FontWeight.Normal,
                                    )
                                },
                                onClick = {
                                    onThemeModeChanged(value)
                                    themeExpanded = false
                                }
                            )
                        }
                    }
                }
            }

            // ── Advanced Settings toggle ───────────────────────────────────────
            item {
                Spacer(Modifier.height(16.dp))
                HorizontalDivider()
                Spacer(Modifier.height(8.dp))
                OutlinedButton(
                    onClick = { showAdvanced = !showAdvanced },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(8.dp)
                ) {
                    Icon(
                        imageVector = Icons.Filled.Settings,
                        contentDescription = null,
                        modifier = Modifier.padding(end = 6.dp)
                    )
                    Text(
                        "Advanced Settings",
                        modifier = Modifier.weight(1f),
                        fontWeight = FontWeight.Medium
                    )
                    Icon(
                        imageVector = if (showAdvanced) Icons.Filled.KeyboardArrowUp
                                      else Icons.Filled.KeyboardArrowDown,
                        contentDescription = if (showAdvanced) "Collapse" else "Expand"
                    )
                }
            }

            // ── Advanced content (animated) ───────────────────────────────────
            item {
                AnimatedVisibility(
                    visible = showAdvanced,
                    enter = expandVertically(),
                    exit = shrinkVertically()
                ) {
                    Card(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                        shape = RoundedCornerShape(10.dp),
                        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp)
                    ) {
                        Column(
                            modifier = Modifier.padding(14.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp)
                        ) {
                            // Re-attestation interval
                            Text(
                                "Re-attestation Interval",
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.Medium
                            )
                            Text(
                                "How often the active provider is automatically re-attested (minutes). 0 = disabled.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            val display = if (attestationInterval.isEmpty())
                                appState.attestationIntervalMinutes.toString()
                            else
                                attestationInterval
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                OutlinedTextField(
                                    value = display,
                                    onValueChange = { attestationInterval = it },
                                    label = { Text("Minutes") },
                                    modifier = Modifier.weight(1f),
                                    singleLine = true,
                                    shape = RoundedCornerShape(8.dp)
                                )
                                Button(
                                    onClick = {
                                        val m = attestationInterval.trim().toUIntOrNull()
                                        if (m != null) {
                                            onDispatch(AppAction.SetAttestationInterval(minutes = m))
                                            attestationInterval = ""
                                        }
                                    },
                                    enabled = attestationInterval.trim().toUIntOrNull() != null,
                                    shape = RoundedCornerShape(8.dp)
                                ) { Text("Apply") }
                            }

                            HorizontalDivider()

                            // Custom provider
                            Text(
                                "Custom Provider",
                                style = MaterialTheme.typography.titleSmall,
                                fontWeight = FontWeight.Medium
                            )
                            Text(
                                "For self-hosted or experimental confidential inference endpoints.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            OutlinedTextField(
                                value = addName, onValueChange = { addName = it },
                                label = { Text("Name") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true, shape = RoundedCornerShape(8.dp)
                            )
                            OutlinedTextField(
                                value = addUrl, onValueChange = { addUrl = it },
                                label = { Text("Base URL") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true, shape = RoundedCornerShape(8.dp)
                            )
                            OutlinedTextField(
                                value = addApiKey, onValueChange = { addApiKey = it },
                                label = { Text("API Key") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                                shape = RoundedCornerShape(8.dp),
                                visualTransformation = if (showApiKey) VisualTransformation.None
                                                       else PasswordVisualTransformation(),
                                trailingIcon = {
                                    TextButton(onClick = { showApiKey = !showApiKey }) {
                                        Text(if (showApiKey) "Hide" else "Show")
                                    }
                                }
                            )
                            ExposedDropdownMenuBox(
                                expanded = teeExpanded,
                                onExpandedChange = { teeExpanded = it },
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                OutlinedTextField(
                                    value = teeTypeLabel(parseTeeType(addTeeType)),
                                    onValueChange = {},
                                    readOnly = true,
                                    label = { Text("TEE Type") },
                                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = teeExpanded) },
                                    shape = RoundedCornerShape(8.dp),
                                    modifier = Modifier.menuAnchor().fillMaxWidth()
                                )
                                ExposedDropdownMenu(
                                    expanded = teeExpanded,
                                    onDismissRequest = { teeExpanded = false }
                                ) {
                                    teeOptions.forEach { opt ->
                                        DropdownMenuItem(
                                            text = { Text(teeTypeLabel(parseTeeType(opt))) },
                                            onClick = { addTeeType = opt; teeExpanded = false }
                                        )
                                    }
                                }
                            }
                            Button(
                                onClick = {
                                    onDispatch(AppAction.AddBackend(
                                        name = addName, baseUrl = addUrl, apiKey = addApiKey,
                                        teeType = parseTeeType(addTeeType), models = emptyList()
                                    ))
                                    addName = ""; addUrl = ""; addApiKey = ""; addTeeType = "IntelTdx"
                                },
                                enabled = addName.isNotBlank() && addUrl.isNotBlank() && addApiKey.isNotEmpty(),
                                modifier = Modifier.fillMaxWidth(),
                                shape = RoundedCornerShape(8.dp)
                            ) { Text("Add Provider") }
                        }
                    }
                }
            }

            item { Spacer(Modifier.height(32.dp)) }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

private fun healthLabel(s: HealthStatus): String = when (s) {
    HealthStatus.HEALTHY  -> "Healthy"
    HealthStatus.DEGRADED -> "Degraded"
    HealthStatus.FAILED   -> "Failed"
    HealthStatus.UNKNOWN  -> "Unknown"
}

private fun healthColor(s: HealthStatus, isDark: Boolean): Color = when (s) {
    HealthStatus.HEALTHY  -> if (isDark) DarkHealthy else LightHealthy
    HealthStatus.DEGRADED -> if (isDark) DarkDegraded else LightDegraded
    HealthStatus.FAILED   -> if (isDark) DarkFailed else LightFailed
    HealthStatus.UNKNOWN  -> if (isDark) DarkHealthUnknown else LightHealthUnknown
}

private fun attestationStyle(s: AttestationStatus, isDark: Boolean): Pair<String, Color> = when (s) {
    is AttestationStatus.Verified    -> "Attested"       to (if (isDark) DarkHealthy else LightHealthy)
    is AttestationStatus.Unverified  -> "Unverified"     to (if (isDark) DarkHealthUnknown else LightHealthUnknown)
    is AttestationStatus.Failed      -> "Attest Failed"  to (if (isDark) DarkFailed else LightFailed)
    is AttestationStatus.Expired     -> "Attest Expired" to (if (isDark) DarkDegraded else LightDegraded)
}

private fun teeTypeLabel(t: TeeType): String = when (t) {
    TeeType.INTEL_TDX      -> "Intel TDX"
    TeeType.NVIDIA_H100_CC -> "NVIDIA H100 CC"
    TeeType.AMD_SEV_SNP    -> "AMD SEV-SNP"
    TeeType.UNKNOWN        -> "Unknown"
}

private fun parseTeeType(s: String): TeeType = when (s) {
    "NvidiaH100Cc" -> TeeType.NVIDIA_H100_CC
    "AmdSevSnp"    -> TeeType.AMD_SEV_SNP
    "Unknown"      -> TeeType.UNKNOWN
    else           -> TeeType.INTEL_TDX
}
