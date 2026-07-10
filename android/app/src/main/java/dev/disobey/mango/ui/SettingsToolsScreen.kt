package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.Screen

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsToolsScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    var braveApiKeyInput by remember { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Tools", fontWeight = FontWeight.Medium) },
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
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            item {
                Spacer(Modifier.height(8.dp))
                SettingsSectionLabel("Tools")
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Web search", fontWeight = FontWeight.Medium)
                        Text(
                            "Required for web search. Keys stay on-device until used for Brave requests.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        RowStatus(appState = appState)
                        OutlinedTextField(
                            value = braveApiKeyInput,
                            onValueChange = { braveApiKeyInput = it },
                            label = {
                                Text(
                                    if (appState.braveApiKeySet) {
                                        "Key configured — enter new key to update"
                                    } else {
                                        "Enter Brave Search API Key"
                                    }
                                )
                            },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            enabled = !appState.braveApiKeyValidating,
                            visualTransformation = PasswordVisualTransformation(),
                        )
                        Button(
                            onClick = {
                                val trimmed = braveApiKeyInput.trim()
                                if (trimmed.isNotEmpty()) {
                                    onDispatch(AppAction.ValidateBraveApiKey(apiKey = trimmed))
                                    braveApiKeyInput = ""
                                }
                            },
                            enabled = braveApiKeyInput.trim().isNotEmpty() && !appState.braveApiKeyValidating,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text(if (appState.braveApiKeyValidating) "Verifying…" else "Save API Key")
                        }
                    }
                }
            }

            item {
                Spacer(Modifier.height(8.dp))
                SettingsSectionLabel("Discover")
                SettingsLinkCard(
                    title = "Discover tools",
                    subtitle = discoverToolsSubtitle(appState.contextvmTools),
                    onClick = { onDispatch(AppAction.PushScreen(screen = Screen.ToolDiscovery)) },
                )
            }
        }
    }
}

@Composable
private fun RowStatus(appState: AppState) {
    androidx.compose.foundation.layout.Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (appState.braveApiKeyValidating) {
            CircularProgressIndicator(modifier = Modifier.height(14.dp), strokeWidth = 2.dp)
            Text("Verifying key", style = MaterialTheme.typography.labelSmall)
        } else if (appState.braveApiKeySet) {
            Icon(
                Icons.Filled.CheckCircle,
                contentDescription = null,
                tint = Color(0xFF2E7D32),
            )
            Text("Configured", style = MaterialTheme.typography.labelSmall)
        } else {
            Text(
                "Not configured",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
