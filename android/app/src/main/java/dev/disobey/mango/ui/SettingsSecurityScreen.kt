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
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsSecurityScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    var message by remember { mutableStateOf<String?>(null) }
    var duressPin by remember { mutableStateOf("") }
    var confirmPin by remember { mutableStateOf("") }
    var lockExpanded by remember { mutableStateOf(false) }
    var showDeleteChatsConfirm by remember { mutableStateOf(false) }
    var showDeleteDataConfirm by remember { mutableStateOf(false) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Security", fontWeight = FontWeight.Medium) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { pad ->
        if (showDeleteChatsConfirm) {
            AlertDialog(
                onDismissRequest = { showDeleteChatsConfirm = false },
                title = { Text("Delete All Chats") },
                text = {
                    Text("This will permanently delete every conversation and message on this device.")
                },
                confirmButton = {
                    TextButton(
                        onClick = {
                            showDeleteChatsConfirm = false
                            onDispatch(AppAction.DeleteAllConversations)
                        }
                    ) {
                        Text("Delete", color = MaterialTheme.colorScheme.error)
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showDeleteChatsConfirm = false }) {
                        Text("Cancel")
                    }
                },
            )
        }

        if (showDeleteDataConfirm) {
            AlertDialog(
                onDismissRequest = { showDeleteDataConfirm = false },
                title = { Text("Delete All Data") },
                text = {
                    Text("This will permanently delete chats, documents, memories, API keys, auth data, and local files, then return the app to clean-install state.")
                },
                confirmButton = {
                    TextButton(
                        onClick = {
                            showDeleteDataConfirm = false
                            onDispatch(AppAction.DeleteAllData)
                        }
                    ) {
                        Text("Delete Everything", color = MaterialTheme.colorScheme.error)
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showDeleteDataConfirm = false }) {
                        Text("Cancel")
                    }
                },
            )
        }

        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(pad)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            item {
                Spacer(Modifier.height(8.dp))
                SettingsSectionLabel("Security")
                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Lock timeout", fontWeight = FontWeight.Medium)
                        Text(
                            "How long the app can stay in the background before it locks.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )

                        ExposedDropdownMenuBox(
                            expanded = lockExpanded,
                            onExpandedChange = { lockExpanded = it },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            OutlinedTextField(
                                value = lockTimeoutLabel(appState.lockTimeoutSeconds),
                                onValueChange = {},
                                readOnly = true,
                                label = { Text("Lock after") },
                                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = lockExpanded) },
                                modifier = Modifier.menuAnchor().fillMaxWidth()
                            )
                            DropdownMenu(
                                expanded = lockExpanded,
                                onDismissRequest = { lockExpanded = false }
                            ) {
                                lockTimeoutOptions.forEach { option ->
                                    DropdownMenuItem(
                                        text = { Text(option.label) },
                                        onClick = {
                                            onDispatch(AppAction.SetLockTimeout(seconds = option.seconds))
                                            lockExpanded = false
                                        }
                                    )
                                }
                            }
                        }

                        if (appState.lockTimeoutSeconds == -1L) {
                            Text(
                                "Auto-lock disabled. The app will open without your PIN — it is protected only by your device unlock. If your device is unlocked, anyone with access can open the app.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }

                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Biometric login", fontWeight = FontWeight.Medium)
                        Text(
                            if (appState.biometricAvailable) {
                                "Use Face ID, Touch ID, or device biometrics to unlock."
                            } else {
                                "Biometrics are not available or not enrolled on this device."
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Switch(
                            checked = appState.biometricLoginEnabled,
                            onCheckedChange = {
                                onDispatch(AppAction.SetBiometricLoginEnabled(enabled = it))
                            },
                            enabled = appState.biometricAvailable,
                        )
                    }

                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Duress PIN", fontWeight = FontWeight.Medium)
                        Text(
                            "Entering this PIN on the lock screen silently erases all local data.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        if (appState.duressPinConfigured) {
                            Text(
                                "A duress PIN is currently configured.",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        OutlinedTextField(
                            value = duressPin,
                            onValueChange = { duressPin = it },
                            label = { Text("New duress PIN") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        OutlinedTextField(
                            value = confirmPin,
                            onValueChange = { confirmPin = it },
                            label = { Text("Confirm duress PIN") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        message?.let { note ->
                            Text(
                                note,
                                style = MaterialTheme.typography.labelSmall,
                                color = if (note.contains("failed", ignoreCase = true) || note.contains("must", ignoreCase = true))
                                    MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Button(
                            onClick = {
                                val trimmed = duressPin.trim()
                                when {
                                    trimmed.isEmpty() -> message = "Enter a duress PIN or use Remove."
                                    trimmed != confirmPin.trim() -> message = "Duress PIN confirmation does not match."
                                    else -> {
                                        message = null
                                        onDispatch(AppAction.SetDuressPin(pin = trimmed))
                                        duressPin = ""
                                        confirmPin = ""
                                    }
                                }
                            },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text(if (appState.duressPinConfigured) "Update Duress PIN" else "Save Duress PIN")
                        }
                        if (appState.duressPinConfigured) {
                            TextButton(
                                onClick = {
                                    message = null
                                    onDispatch(AppAction.SetDuressPin(pin = null))
                                    duressPin = ""
                                    confirmPin = ""
                                },
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Remove Duress PIN")
                            }
                        }
                    }

                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Delete all chats", fontWeight = FontWeight.Medium)
                        Text(
                            "Remove every conversation and message stored on this device.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { showDeleteChatsConfirm = true },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Delete All Chats", color = MaterialTheme.colorScheme.error)
                        }
                    }

                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Delete all data", fontWeight = FontWeight.Medium)
                        Text(
                            "Erase chats, documents, memories, API keys, auth data, and local files, then return to the first-launch app state.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { showDeleteDataConfirm = true },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Delete All Data", color = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }
        }
    }
}
