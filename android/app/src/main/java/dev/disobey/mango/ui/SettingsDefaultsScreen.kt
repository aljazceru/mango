package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.HealthStatus

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsDefaultsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    val isDark = isSystemInDarkTheme()
    var defaultModelExp by remember { mutableStateOf(false) }
    var defaultModel by remember { mutableStateOf("") }
    var defaultInstructions by remember { mutableStateOf(appState.globalSystemPrompt ?: "") }

    // Aggregate (modelId, backendName) pairs from all non-failed backends
    val allModelEntries: List<Pair<String, String>> = appState.backends
        .filter { it.healthStatus != HealthStatus.FAILED && it.models.isNotEmpty() }
        .flatMap { backend -> backend.models.map { modelId -> Pair(modelId, backend.name) } }
        .sortedBy { (modelId, _) -> modelId }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Defaults", fontWeight = FontWeight.Medium) },
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
                .padding(horizontal = 16.dp)
        ) {
            item {
                Spacer(Modifier.height(8.dp))
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

                Spacer(Modifier.height(32.dp))
            }
        }
    }
}
