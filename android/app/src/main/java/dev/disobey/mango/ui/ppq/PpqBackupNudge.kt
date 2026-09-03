package dev.disobey.mango.ui.ppq

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * Backup nudge shown immediately after a managed PPQ account is created.
 */
@Composable
fun PpqBackupNudge(
    onBackup: () -> Unit,
    onDeferConfirmed: () -> Unit,
) {
    var showWarning by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            text = "Your PPQ account is ready. Back it up now.",
            style = MaterialTheme.typography.titleMedium,
            fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold,
            textAlign = TextAlign.Center,
        )

        Text(
            text = "If this phone is lost or Mango is deleted, your PPQ balance may be " +
                   "unrecoverable without a backup.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Button(
            onClick = onBackup,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Back up now")
        }

        OutlinedButton(
            onClick = { showWarning = true },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Do this later")
        }
    }

    if (showWarning) {
        AlertDialog(
            onDismissRequest = { showWarning = false },
            title = { Text("Skip backup?") },
            text = {
                Text(
                    "If this phone is lost or Mango is deleted, your PPQ balance may be " +
                    "unrecoverable."
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        showWarning = false
                        onDeferConfirmed()
                    }
                ) {
                    Text("I understand", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { showWarning = false }) {
                    Text("Cancel")
                }
            },
        )
    }
}
