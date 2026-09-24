package dev.disobey.mango.ui.ppq

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.PpqDestructivePreflight

/**
 * Confirmation dialog for destructive PPQ actions. Rust sets
 * `appState.ppq.destructivePreflight` ("forget_managed" or "delete_all_data")
 * and waits for the matching Confirm call; without this dialog those
 * preflights were invisible dead ends (Remove did nothing).
 *
 * Mirrors PpqRestoreDialog's step-up auth: biometrics or the app PIN.
 */
@Composable
fun PpqDestructivePreflightDialog(
    preflight: PpqDestructivePreflight,
    biometricAvailable: Boolean,
    onConfirm: (useBiometric: Boolean, pin: String?, backupRiskAcknowledged: Boolean) -> Unit,
    onDismiss: () -> Unit,
) {
    var useBiometric by remember { mutableStateOf(biometricAvailable) }
    var pin by remember { mutableStateOf("") }
    var acknowledged by remember { mutableStateOf(preflight.backupConfirmed) }

    val isForget = preflight.kind == "forget_managed"
    val balanceLine = preflight.balanceDisplay
        ?.let { "PPQ balance: $it USD" }
        ?: "PPQ balance: unknown"

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                if (isForget) "Remove PPQ from this device?"
                else "Delete all data?"
            )
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(balanceLine, fontWeight = FontWeight.Medium)
                Text(
                    if (isForget) {
                        "This removes the managed PPQ account from this device. " +
                            "Unless you have a .mppq backup, the remaining balance " +
                            "becomes unrecoverable."
                    } else {
                        "This erases ALL app data: conversations, documents, keys, " +
                            "and the managed PPQ account. Unless you have a .mppq " +
                            "backup, the remaining balance becomes unrecoverable."
                    },
                    color = MaterialTheme.colorScheme.error,
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = acknowledged, onCheckedChange = { acknowledged = it })
                    Text(
                        "I understand this may permanently lose my PPQ balance",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
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
                        Switch(checked = useBiometric, onCheckedChange = { useBiometric = it; pin = "" })
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
                enabled = acknowledged && (useBiometric || pin.isNotBlank()),
                onClick = { onConfirm(useBiometric, pin.takeIf { !useBiometric }, acknowledged) },
            ) {
                Text(if (isForget) "Remove account" else "Delete everything")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}
