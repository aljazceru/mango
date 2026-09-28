package dev.disobey.mango.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState

@Stable
class AppLockUiState {
    var message by mutableStateOf<String?>(null)
    var duressPin by mutableStateOf("")
    var confirmPin by mutableStateOf("")
    var currentPin by mutableStateOf("")
    var newPin by mutableStateOf("")
    var confirmNewPin by mutableStateOf("")
    var pinMessage by mutableStateOf<String?>(null)
    var enableLockPin by mutableStateOf("")
    var enableLockConfirm by mutableStateOf("")
    var enableLockBiometric by mutableStateOf(false)
    var lockExpanded by mutableStateOf(false)
}

@Composable
fun rememberAppLockUiState() = remember { AppLockUiState() }

@OptIn(ExperimentalMaterial3Api::class)
fun LazyListScope.appLockItems(appState: AppState, onDispatch: (AppAction) -> Unit, ui: AppLockUiState) {
            item {
                Spacer(Modifier.height(8.dp))
                SettingsSectionLabel("Security")
                Card(modifier = Modifier.fillMaxWidth()) {
                    if (appState.noLockMode) {
                        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Text("App lock", fontWeight = FontWeight.Medium)
                            Text(
                                "No PIN is set. The app opens directly and your data is " +
                                    "protected by your device unlock only.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            OutlinedTextField(
                                value = ui.enableLockPin,
                                onValueChange = { ui.enableLockPin = it },
                                label = { Text("New PIN (min 4 characters)") },
                                visualTransformation = PasswordVisualTransformation(),
                                singleLine = true,
                                modifier = Modifier.fillMaxWidth(),
                            )
                            OutlinedTextField(
                                value = ui.enableLockConfirm,
                                onValueChange = { ui.enableLockConfirm = it },
                                label = { Text("Confirm PIN") },
                                visualTransformation = PasswordVisualTransformation(),
                                singleLine = true,
                                modifier = Modifier.fillMaxWidth(),
                            )
                            if (appState.biometricAvailable) {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text("Enable biometric unlock")
                                    Switch(
                                        checked = ui.enableLockBiometric,
                                        onCheckedChange = { ui.enableLockBiometric = it },
                                    )
                                }
                            }
                            ui.pinMessage?.let { note ->
                                Text(
                                    note,
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.error,
                                )
                            }
                            Button(
                                onClick = {
                                    val nw = ui.enableLockPin.trim()
                                    when {
                                        nw.length < 4 -> ui.pinMessage = "PIN must be at least 4 characters."
                                        nw != ui.enableLockConfirm.trim() -> ui.pinMessage = "PIN confirmation does not match."
                                        else -> {
                                            ui.pinMessage = null
                                            onDispatch(
                                                AppAction.EnablePinLock(
                                                    pin = nw,
                                                    duressPin = null,
                                                    enableBiometric = ui.enableLockBiometric,
                                                )
                                            )
                                            ui.enableLockPin = ""
                                            ui.enableLockConfirm = ""
                                        }
                                    }
                                },
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Enable app lock")
                            }
                        }
                    } else {
                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Lock timeout", fontWeight = FontWeight.Medium)
                        Text(
                            "How long the app can stay in the background before it locks.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )

                        ExposedDropdownMenuBox(
                            expanded = ui.lockExpanded,
                            onExpandedChange = { ui.lockExpanded = it },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            OutlinedTextField(
                                value = lockTimeoutLabel(appState.lockTimeoutSeconds),
                                onValueChange = {},
                                readOnly = true,
                                label = { Text("Lock after") },
                                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = ui.lockExpanded) },
                                modifier = Modifier.menuAnchor().fillMaxWidth()
                            )
                            DropdownMenu(
                                expanded = ui.lockExpanded,
                                onDismissRequest = { ui.lockExpanded = false }
                            ) {
                                lockTimeoutOptions.forEach { option ->
                                    DropdownMenuItem(
                                        text = { Text(option.label) },
                                        onClick = {
                                            onDispatch(AppAction.SetLockTimeout(seconds = option.seconds))
                                            ui.lockExpanded = false
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
                        Text("Change PIN", fontWeight = FontWeight.Medium)
                        Text(
                            "Changing your PIN re-wraps your encryption key; your data, biometric " +
                                "unlock, and emergency PIN are unaffected.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedTextField(
                            value = ui.currentPin,
                            onValueChange = { ui.currentPin = it },
                            label = { Text("Current PIN") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        OutlinedTextField(
                            value = ui.newPin,
                            onValueChange = { ui.newPin = it },
                            label = { Text("New PIN (min 4 characters)") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        OutlinedTextField(
                            value = ui.confirmNewPin,
                            onValueChange = { ui.confirmNewPin = it },
                            label = { Text("Confirm new PIN") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        ui.pinMessage?.let { note ->
                            Text(
                                note,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.error,
                            )
                        }
                        Button(
                            onClick = {
                                val cur = ui.currentPin.trim()
                                val nw = ui.newPin.trim()
                                when {
                                    cur.isEmpty() || nw.isEmpty() -> ui.pinMessage = "Fill in all PIN fields."
                                    nw.length < 4 -> ui.pinMessage = "New PIN must be at least 4 characters."
                                    nw != ui.confirmNewPin.trim() -> ui.pinMessage = "New PIN confirmation does not match."
                                    nw == cur -> ui.pinMessage = "New PIN must be different from the current PIN."
                                    else -> {
                                        ui.pinMessage = null
                                        onDispatch(AppAction.ChangePin(currentPin = cur, newPin = nw))
                                        ui.currentPin = ""
                                        ui.newPin = ""
                                        ui.confirmNewPin = ""
                                    }
                                }
                            },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Change PIN")
                        }
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
                            value = ui.duressPin,
                            onValueChange = { ui.duressPin = it },
                            label = { Text("New duress PIN") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        OutlinedTextField(
                            value = ui.confirmPin,
                            onValueChange = { ui.confirmPin = it },
                            label = { Text("Confirm duress PIN") },
                            visualTransformation = PasswordVisualTransformation(),
                            singleLine = true,
                            modifier = Modifier.fillMaxWidth(),
                        )
                        ui.message?.let { note ->
                            Text(
                                note,
                                style = MaterialTheme.typography.labelSmall,
                                color = if (note.contains("failed", ignoreCase = true) || note.contains("must", ignoreCase = true))
                                    MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Button(
                            onClick = {
                                val trimmed = ui.duressPin.trim()
                                when {
                                    trimmed.isEmpty() -> ui.message = "Enter a duress PIN or use Remove."
                                    trimmed != ui.confirmPin.trim() -> ui.message = "Duress PIN confirmation does not match."
                                    else -> {
                                        ui.message = null
                                        onDispatch(AppAction.SetDuressPin(pin = trimmed))
                                        ui.duressPin = ""
                                        ui.confirmPin = ""
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
                                    ui.message = null
                                    onDispatch(AppAction.SetDuressPin(pin = null))
                                    ui.duressPin = ""
                                    ui.confirmPin = ""
                                },
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Remove Duress PIN")
                            }
                        }
                    }
                    }
                }
            }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsAppLockScreen(appState: AppState, onDispatch: (AppAction) -> Unit, onBack: () -> Unit = { onDispatch(AppAction.PopScreen) }) {
    val ui = rememberAppLockUiState()
    Scaffold(topBar = {
        TopAppBar(
            title = { Text("App lock", fontWeight = FontWeight.Medium) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
        )
    }) { pad ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) { appLockItems(appState, onDispatch, ui) }
    }
}
