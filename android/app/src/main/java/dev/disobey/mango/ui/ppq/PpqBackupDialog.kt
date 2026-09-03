package dev.disobey.mango.ui.ppq

import dev.disobey.mango.PpqBackupCoordinator
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
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

private const val MIN_BACKUP_PASSWORD_LENGTH = 12

/**
 * Password dialog for creating a PPQ recovery backup.
 *
 * Collects a strong backup password and (when biometrics are not chosen) the current
 * app PIN for the step-up [SensitiveActionAuth] the Rust core requires.
 */
@Composable
fun PpqBackupDialog(
    biometricAvailable: Boolean,
    onConfirm: (backupPassword: String, useBiometric: Boolean, pin: String?) -> Unit,
    onDismiss: () -> Unit,
) {
    // Password fields are held in plain remember, never rememberSaveable, and cleared on dispose.
    var backupPassword by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }
    var useBiometric by remember { mutableStateOf(biometricAvailable) }
    var pin by remember { mutableStateOf("") }

    DisposableEffect(Unit) {
        onDispose {
            backupPassword = ""
            confirmPassword = ""
            pin = ""
        }
    }

    val passwordMatch = backupPassword == confirmPassword
    val passwordLongEnough = backupPassword.length >= MIN_BACKUP_PASSWORD_LENGTH
    val authReady = useBiometric || pin.isNotBlank()
    val canConfirm = passwordLongEnough && passwordMatch && authReady

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Back up your PPQ account") },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    text = "Choose a strong password for this backup. You will need it to " +
                           "restore your PPQ balance on another device.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )

                OutlinedTextField(
                    value = backupPassword,
                    onValueChange = { backupPassword = it },
                    label = { Text("Backup password (min $MIN_BACKUP_PASSWORD_LENGTH characters)") },
                    singleLine = true,
                    visualTransformation = if (showPassword) VisualTransformation.None
                                           else PasswordVisualTransformation(),
                    trailingIcon = {
                        TextButton(onClick = { showPassword = !showPassword }) {
                            Text(if (showPassword) "Hide" else "Show")
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )

                OutlinedTextField(
                    value = confirmPassword,
                    onValueChange = { confirmPassword = it },
                    label = { Text("Confirm backup password") },
                    singleLine = true,
                    visualTransformation = if (showPassword) VisualTransformation.None
                                           else PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                )

                if (biometricAvailable) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column {
                            Text("Use biometrics", fontWeight = FontWeight.Medium)
                            Text(
                                "Authenticate with fingerprint or face",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = useBiometric,
                            onCheckedChange = { useBiometric = it; pin = "" },
                        )
                    }
                }

                if (!useBiometric) {
                    OutlinedTextField(
                        value = pin,
                        onValueChange = { pin = it },
                        label = { Text("Current app PIN") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }

                if (!passwordLongEnough && backupPassword.isNotEmpty()) {
                    Text(
                        "Password must be at least $MIN_BACKUP_PASSWORD_LENGTH characters",
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                if (confirmPassword.isNotEmpty() && !passwordMatch) {
                    Text(
                        "Passwords do not match",
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }

                Spacer(modifier = Modifier.height(8.dp))
            }
        },
        confirmButton = {
            Button(
                onClick = {
                    onConfirm(
                        backupPassword,
                        useBiometric,
                        if (useBiometric) null else pin,
                    )
                    onDismiss()
                },
                enabled = canConfirm,
            ) {
                Text("Back up")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
    )
}

/**
 * Restore dialog for a PPQ recovery backup.
 *
 * Collects the backup password, step-up auth, and requests a file pick. The caller's
 * [filePicker] is responsible for launching the OpenDocument flow and eventually invoking
 * [onRestore] with the selected bytes.
 */
@Composable
fun PpqRestoreDialog(
    biometricAvailable: Boolean,
    onRestore: (bytes: ByteArray, backupPassword: String, useBiometric: Boolean, pin: String?, replaceAcknowledged: Boolean) -> Unit,
    filePicker: () -> Unit,
    onDismiss: () -> Unit,
    // Threat review (high): restoring over an existing managed account is a
    // replacement — require an explicit second confirmation.
    existingManaged: Boolean = false,
) {
    var confirmReplace by remember { mutableStateOf(!existingManaged) }
    var backupPassword by remember { mutableStateOf("") }
    var showPassword by remember { mutableStateOf(false) }
    var useBiometric by remember { mutableStateOf(biometricAvailable) }
    var pin by remember { mutableStateOf("") }

    DisposableEffect(Unit) {
        onDispose {
            backupPassword = ""
            pin = ""
        }
    }

    val authReady = useBiometric || pin.isNotBlank()
    val canPick = backupPassword.isNotBlank() && authReady

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Restore PPQ backup") },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(
                    text = "Enter the backup password, then select your .mppq file.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )

                OutlinedTextField(
                    value = backupPassword,
                    onValueChange = { backupPassword = it },
                    label = { Text("Backup password") },
                    singleLine = true,
                    visualTransformation = if (showPassword) VisualTransformation.None
                                           else PasswordVisualTransformation(),
                    trailingIcon = {
                        TextButton(onClick = { showPassword = !showPassword }) {
                            Text(if (showPassword) "Hide" else "Show")
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )

                if (biometricAvailable) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Column {
                            Text("Use biometrics", fontWeight = FontWeight.Medium)
                            Text(
                                "Authenticate with fingerprint or face",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = useBiometric,
                            onCheckedChange = { useBiometric = it; pin = "" },
                        )
                    }
                }

                if (!useBiometric) {
                    OutlinedTextField(
                        value = pin,
                        onValueChange = { pin = it },
                        label = { Text("Current app PIN") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        },
        confirmButton = {
            Button(
                onClick = {
                    if (!confirmReplace) {
                        confirmReplace = true
                        return@Button
                    }
                    PpqBackupCoordinator.onPpqImportResult = { bytes ->
                        PpqBackupCoordinator.onPpqImportResult = null
                        onRestore(bytes, backupPassword, useBiometric, pin, existingManaged)
                    }
                    filePicker()
                },
                enabled = canPick,
            ) {
                Text("Select backup file")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel")
            }
        },
    )
}
