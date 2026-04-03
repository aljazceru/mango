package dev.disobey.mango.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Help
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.ui.theme.AttestExpiredBgDark
import dev.disobey.mango.ui.theme.AttestExpiredBgLight
import dev.disobey.mango.ui.theme.AttestExpiredBorderDark
import dev.disobey.mango.ui.theme.AttestExpiredBorderLight
import dev.disobey.mango.ui.theme.AttestExpiredTextDark
import dev.disobey.mango.ui.theme.AttestExpiredTextLight
import dev.disobey.mango.ui.theme.AttestFailedBgDark
import dev.disobey.mango.ui.theme.AttestFailedBgLight
import dev.disobey.mango.ui.theme.AttestFailedBorderDark
import dev.disobey.mango.ui.theme.AttestFailedBorderLight
import dev.disobey.mango.ui.theme.AttestFailedTextDark
import dev.disobey.mango.ui.theme.AttestFailedTextLight
import dev.disobey.mango.ui.theme.AttestUnverifiedBgDark
import dev.disobey.mango.ui.theme.AttestUnverifiedBgLight
import dev.disobey.mango.ui.theme.AttestUnverifiedBorderDark
import dev.disobey.mango.ui.theme.AttestUnverifiedBorderLight
import dev.disobey.mango.ui.theme.AttestUnverifiedTextDark
import dev.disobey.mango.ui.theme.AttestUnverifiedTextLight
import dev.disobey.mango.ui.theme.AttestVerifiedBgDark
import dev.disobey.mango.ui.theme.AttestVerifiedBgLight
import dev.disobey.mango.ui.theme.AttestVerifiedBorderDark
import dev.disobey.mango.ui.theme.AttestVerifiedBorderLight
import dev.disobey.mango.ui.theme.AttestVerifiedTextDark
import dev.disobey.mango.ui.theme.AttestVerifiedTextLight

/// Capsule-shaped attestation trust badge for the chat top bar.
/// Color and icon reflect the attestation status per UI-SPEC.
@Composable
fun AttestationBadge(
    status: AttestationStatus,
    modifier: Modifier = Modifier,
) {
    var showDetail by remember { mutableStateOf(false) }

    Surface(
        onClick = { showDetail = true },
        modifier = modifier,
        shape = androidx.compose.foundation.shape.RoundedCornerShape(50),
        color = badgeBackground(status),
        border = BorderStroke(1.dp, badgeStroke(status)),
        tonalElevation = 0.dp,
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = badgeIcon(status),
                contentDescription = null,
                tint = badgeForeground(status),
                modifier = Modifier.size(12.dp),
            )
            Text(
                text = badgeLabel(status),
                style = MaterialTheme.typography.labelSmall,
                color = badgeForeground(status),
            )
        }
    }

    if (showDetail) {
        AttestationDetailDialog(status = status, onDismiss = { showDetail = false })
    }
}

// MARK: - Detail Dialog

@Composable
private fun AttestationDetailDialog(status: AttestationStatus, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Trust Status") },
        text = { Text(detailText(status)) },
        confirmButton = {
            Button(onClick = onDismiss) { Text("OK") }
        },
    )
}

// MARK: - Status Helpers

private fun badgeLabel(status: AttestationStatus): String = when (status) {
    is AttestationStatus.Verified -> "Verified"
    is AttestationStatus.Unverified -> "Not Verified"
    is AttestationStatus.Expired -> "Expired"
    is AttestationStatus.Failed -> "Failed"
}

private fun badgeIcon(status: AttestationStatus): ImageVector = when (status) {
    is AttestationStatus.Verified -> Icons.Default.CheckCircle
    is AttestationStatus.Unverified -> Icons.Default.Help
    is AttestationStatus.Expired -> Icons.Default.Schedule
    is AttestationStatus.Failed -> Icons.Default.Warning
}

@Composable
private fun badgeBackground(status: AttestationStatus): Color {
    val dark = isSystemInDarkTheme()
    return when (status) {
        is AttestationStatus.Verified ->   if (dark) AttestVerifiedBgDark else AttestVerifiedBgLight
        is AttestationStatus.Expired ->    if (dark) AttestExpiredBgDark else AttestExpiredBgLight
        is AttestationStatus.Unverified -> if (dark) AttestUnverifiedBgDark else AttestUnverifiedBgLight
        is AttestationStatus.Failed ->     if (dark) AttestFailedBgDark else AttestFailedBgLight
    }
}

@Composable
private fun badgeForeground(status: AttestationStatus): Color {
    val dark = isSystemInDarkTheme()
    return when (status) {
        is AttestationStatus.Verified ->   if (dark) AttestVerifiedTextDark else AttestVerifiedTextLight
        is AttestationStatus.Expired ->    if (dark) AttestExpiredTextDark else AttestExpiredTextLight
        is AttestationStatus.Unverified -> if (dark) AttestUnverifiedTextDark else AttestUnverifiedTextLight
        is AttestationStatus.Failed ->     if (dark) AttestFailedTextDark else AttestFailedTextLight
    }
}

@Composable
private fun badgeStroke(status: AttestationStatus): Color {
    val dark = isSystemInDarkTheme()
    return when (status) {
        is AttestationStatus.Verified ->   if (dark) AttestVerifiedBorderDark else AttestVerifiedBorderLight
        is AttestationStatus.Expired ->    if (dark) AttestExpiredBorderDark else AttestExpiredBorderLight
        is AttestationStatus.Unverified -> if (dark) AttestUnverifiedBorderDark else AttestUnverifiedBorderLight
        is AttestationStatus.Failed ->     if (dark) AttestFailedBorderDark else AttestFailedBorderLight
    }
}

private fun detailText(status: AttestationStatus): String = when (status) {
    is AttestationStatus.Verified ->
        "This conversation is routed to a Trusted Execution Environment. The TEE attestation report has been independently verified by this app."
    is AttestationStatus.Unverified ->
        "Attestation has not been checked for this backend yet."
    is AttestationStatus.Expired ->
        "The cached attestation result has expired and re-verification is needed."
    is AttestationStatus.Failed ->
        "Attestation verification failed. The backend could not prove it is running in a trusted environment.\n\nReason: ${status.reason}"
}
