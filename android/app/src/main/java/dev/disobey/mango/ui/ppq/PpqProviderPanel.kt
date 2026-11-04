package dev.disobey.mango.ui.ppq

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Warning
import dev.disobey.mango.ui.PillTone
import dev.disobey.mango.ui.StatusPill
import dev.disobey.mango.rust.PpqAccountMode
import dev.disobey.mango.rust.PpqAccountSummary

/**
 * Expanded PPQ card content for Settings → Providers.
 *
 * Shows the managed/external mode, balance, backup state, and pending invoice.
 * Action buttons: Top up / Back up again / Refresh / Remove from this device.
 */
@Composable
fun PpqProviderPanel(
    summary: PpqAccountSummary,
    onTopUp: () -> Unit,
    onBackUp: () -> Unit,
    onRefresh: () -> Unit,
    onRemove: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // Mode label
        Text(
            text = when (summary.mode) {
                PpqAccountMode.MANAGED -> "Managed by Mango"
                PpqAccountMode.EXTERNAL_KEY -> "External API key"
                PpqAccountMode.NONE -> "Not set up"
            },
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.SemiBold,
        )

        // Balance + last refresh (stacked: the Fraunces headline must not collide
        // with the timestamp at large font scales)
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                text = "PPQ balance (USD)",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = formatPpqBalance(summary.balanceDisplay),
                style = MaterialTheme.typography.headlineSmall,
            )
            summary.balanceUpdatedAt?.let { at ->
                Text(
                    text = "Updated ${formatPpqTimestamp(at)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        // Backup status
        if (summary.backupConfirmed) {
            StatusPill(text = "Backup: Saved", tone = PillTone.OK, icon = Icons.Filled.CheckCircle)
        } else {
            StatusPill(text = "Backup: Needs backup", tone = PillTone.WARN, icon = Icons.Filled.Warning)
        }

        // Pending invoice line
        summary.funding?.let { funding ->
            HorizontalDivider()
            Text(
                text = "Pending invoice: ${funding.amountSats} sats",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        // Actions
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(
                onClick = onTopUp,
                modifier = Modifier.weight(1f),
                enabled = summary.mode == PpqAccountMode.MANAGED,
            ) {
                Text("Top up")
            }
            OutlinedButton(
                onClick = onBackUp,
                modifier = Modifier.weight(1f),
                enabled = summary.mode == PpqAccountMode.MANAGED,
            ) {
                Text("Back up again")
            }
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedButton(
                onClick = onRefresh,
                modifier = Modifier.weight(1f),
            ) {
                Text("Refresh")
            }
            OutlinedButton(
                onClick = onRemove,
                modifier = Modifier.weight(1f),
                colors = ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
            ) {
                Text("Remove from this device", color = MaterialTheme.colorScheme.error)
            }
        }
    }
}
