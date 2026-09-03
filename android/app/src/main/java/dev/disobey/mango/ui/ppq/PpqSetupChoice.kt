package dev.disobey.mango.ui.ppq

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * Initial PPQ setup fork: managed account vs existing API key.
 */
@Composable
fun PpqSetupChoice(
    onAutomatic: () -> Unit,
    onExistingKey: () -> Unit,
    onRestore: () -> Unit = {},
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            text = "PPQ holds the prepaid balance. Mango stores the PPQ access credentials " +
                   "on this device. Top-ups open an external Lightning wallet.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )

        Spacer(modifier = Modifier.height(8.dp))

        Button(
            onClick = onAutomatic,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("Set up PPQ automatically")
        }

        OutlinedButton(
            onClick = onExistingKey,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text("I already have a PPQ API key")
        }
        TextButton(onClick = onRestore, modifier = Modifier.fillMaxWidth()) {
            Text("Restore from backup", color = MaterialTheme.colorScheme.onSurfaceVariant)
        }

    }
}
