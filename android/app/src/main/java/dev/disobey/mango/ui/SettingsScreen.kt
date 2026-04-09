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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Switch
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.graphics.Color
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.TeeType
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
    var showAdvanced        by remember { mutableStateOf(false) }
    var addName             by remember { mutableStateOf("") }
    var addUrl              by remember { mutableStateOf("") }
    var addApiKey           by remember { mutableStateOf("") }
    var showApiKey          by remember { mutableStateOf(false) }
    var addTeeType          by remember { mutableStateOf("IntelTdx") }
    var teeExpanded         by remember { mutableStateOf(false) }
    var attestationInterval by remember { mutableStateOf("") }
    var braveApiKeyInput   by remember { mutableStateOf("") }
    var braveApiKeyMessage by remember { mutableStateOf<String?>(null) }
    var themeExpanded by remember { mutableStateOf(false) }

    // Mirror toast into inline message when validation completes, then clear toast.
    LaunchedEffect(Unit) {
        snapshotFlow { appState.toast }
            .collect { toast ->
                if (toast != null) {
                    braveApiKeyMessage = toast
                    onDispatch(AppAction.ClearToast)
                }
            }
    }
    val themeOptions = listOf("system" to "Follow System", "light" to "Force Light", "dark" to "Force Dark")
    val themeLabel = themeOptions.firstOrNull { it.first == themeMode }?.second ?: "Follow System"

    val teeOptions  = listOf("IntelTdx", "NvidiaH100Cc", "AmdSevSnp", "Unknown")
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
            item {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(10.dp),
                    elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                ) {
                    Row(
                        modifier = Modifier
                            .clickable { onDispatch(AppAction.PushScreen(screen = Screen.SettingsProviders)) }
                            .padding(16.dp)
                            .fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Providers", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.weight(1f))
                        val enabledCount = appState.backends.count { it.hasApiKey }
                        if (enabledCount > 0) {
                            Text(
                                "$enabledCount enabled",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                            Spacer(Modifier.width(8.dp))
                        }
                        Icon(
                            Icons.AutoMirrored.Filled.KeyboardArrowRight,
                            contentDescription = "Open",
                            modifier = Modifier.size(16.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
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
            }
            item {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(10.dp),
                    elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
                    colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                ) {
                    Row(
                        modifier = Modifier
                            .clickable { onDispatch(AppAction.PushScreen(screen = Screen.SettingsDefaults)) }
                            .padding(16.dp)
                            .fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Defaults", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
                        Spacer(Modifier.weight(1f))
                        val activeModel = appState.backends.firstOrNull { it.isActive }?.models?.firstOrNull()
                        if (activeModel != null) {
                            Text(
                                activeModel,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1
                            )
                            Spacer(Modifier.width(8.dp))
                        }
                        Icon(
                            Icons.AutoMirrored.Filled.KeyboardArrowRight,
                            contentDescription = "Open",
                            modifier = Modifier.size(16.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
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

            // ── Security (Lock Timeout) ───────────────────────────────────────
            item {
                Spacer(Modifier.height(16.dp))
                Text(
                    "SECURITY",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 4.dp, bottom = 4.dp)
                )
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(10.dp),
                ) {
                    LockTimeoutPicker(
                        currentSeconds = appState.lockTimeoutSeconds,
                        onDispatch = onDispatch,
                    )
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
                            if (appState.braveApiKeyValidating) {
                                CircularProgressIndicator(
                                    modifier = Modifier.size(16.dp),
                                    strokeWidth = 2.dp
                                )
                            } else if (appState.braveApiKeySet) {
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                                ) {
                                    Icon(
                                        Icons.Filled.CheckCircle,
                                        contentDescription = null,
                                        modifier = Modifier.size(14.dp),
                                        tint = Color(0xFF4CAF50)
                                    )
                                    Text(
                                        "Configured",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant
                                    )
                                }
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
                            enabled = !appState.braveApiKeyValidating,
                            visualTransformation = PasswordVisualTransformation()
                        )
                        braveApiKeyMessage?.let { msg ->
                            Spacer(Modifier.height(4.dp))
                            Text(
                                msg,
                                style = MaterialTheme.typography.labelSmall,
                                color = if (msg.contains("saved")) Color(0xFF4CAF50)
                                        else MaterialTheme.colorScheme.error
                            )
                        }
                        Spacer(Modifier.height(8.dp))
                        Button(
                            onClick = {
                                val trimmed = braveApiKeyInput.trim()
                                if (trimmed.isNotEmpty()) {
                                    braveApiKeyMessage = null
                                    onDispatch(AppAction.ValidateBraveApiKey(apiKey = trimmed))
                                    braveApiKeyInput = ""
                                }
                            },
                            modifier = Modifier.fillMaxWidth(),
                            shape = RoundedCornerShape(8.dp),
                            enabled = braveApiKeyInput.trim().isNotEmpty() && !appState.braveApiKeyValidating
                        ) { Text(if (appState.braveApiKeyValidating) "Verifying…" else "Save API Key") }
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

// ── Lock Timeout Picker ───────────────────────────────────────────────────────

private data class LockTimeoutOption(val label: String, val seconds: Long)

private val lockTimeoutOptions = listOf(
    LockTimeoutOption("Immediately", 0L),
    LockTimeoutOption("1 minute", 60L),
    LockTimeoutOption("5 minutes", 300L),
    LockTimeoutOption("15 minutes", 900L),
    LockTimeoutOption("Never", -1L),
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun LockTimeoutPicker(
    currentSeconds: Long,
    onDispatch: (AppAction) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val currentLabel = lockTimeoutOptions.firstOrNull { it.seconds == currentSeconds }?.label ?: "5 minutes"

    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Lock Timeout", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
        Text(
            "How long the app can be in the background before it locks.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        ExposedDropdownMenuBox(
            expanded = expanded,
            onExpandedChange = { expanded = it },
            modifier = Modifier.fillMaxWidth()
        ) {
            OutlinedTextField(
                value = currentLabel,
                onValueChange = {},
                readOnly = true,
                label = { Text("Lock after") },
                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
                shape = RoundedCornerShape(8.dp),
                modifier = Modifier.menuAnchor().fillMaxWidth()
            )
            ExposedDropdownMenu(
                expanded = expanded,
                onDismissRequest = { expanded = false }
            ) {
                lockTimeoutOptions.forEach { option ->
                    DropdownMenuItem(
                        text = {
                            Text(
                                text = option.label,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = if (option.seconds == currentSeconds) FontWeight.Bold else FontWeight.Normal
                            )
                        },
                        onClick = {
                            onDispatch(AppAction.SetLockTimeout(seconds = option.seconds))
                            expanded = false
                        }
                    )
                }
            }
        }
        if (currentSeconds == -1L) {
            Text(
                "Not recommended. App will only lock on restart.",
                style = MaterialTheme.typography.labelSmall,
                color = androidx.compose.ui.graphics.Color(0xFFE65100)
            )
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
