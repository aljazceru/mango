package dev.disobey.mango.ui.ppq

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import dev.disobey.mango.rust.PpqAccountSummary
import dev.disobey.mango.rust.PpqFundingPhase


private val DEFAULT_MAX_SATS = 1_000_000uL

/**
 * Managed PPQ top-up screen.
 *
 * Collects an integer sats amount, displays the generated invoice, and exposes wallet
 * handoff, copy, and status-check actions. Never labels the balance as "Mango balance".
 */
@Composable
fun PpqFundingScreen(
    summary: PpqAccountSummary,
    onCreateInvoice: (amountSats: ULong) -> Unit,
    onCheckStatus: () -> Unit,
    onCancel: () -> Unit,
    onOpenWallet: (bolt11: String) -> Unit,
    onCopyInvoice: (bolt11: String) -> Unit,
    onContinue: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    var amountText by remember { mutableStateOf("") }
    var amountError by remember { mutableStateOf<String?>(null) }
    val funding = summary.funding

    LaunchedEffect(summary.fundingPhase) {
        // Clear the input when an invoice has been confirmed/paid or cancelled server-side.
        if (summary.fundingPhase == PpqFundingPhase.IDLE) {
            amountText = ""
            amountError = null
        }
    }

    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        // Balance line
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "PPQ balance",
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.Medium,
            )
            Text(
                text = summary.balanceDisplay ?: "—",
                style = MaterialTheme.typography.titleSmall,
            )
        }

        // Amount entry
        AnimatedVisibility(visible = funding == null) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "Top up your PPQ balance",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )


                OutlinedTextField(
                    value = amountText,
                    onValueChange = {
                        amountText = it.filter { c -> c.isDigit() }
                        amountError = null
                    },
                    label = { Text("Amount (sats)") },
                    singleLine = true,
                    isError = amountError != null,
                    supportingText = amountError?.let { { Text(it) } },
                    modifier = Modifier.fillMaxWidth(),
                )

                Button(
                    onClick = {
                        // Bounds are enforced by the Rust core against LIVE
                        // server-advertised limits (plan §6.9) — never hardcoded here.
                        val parsed = parseSatsAmount(amountText)
                        if (parsed == null) {
                            amountError = "Enter a whole number of satoshis"
                        } else {
                            onCreateInvoice(parsed)
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Create invoice")
                }
            }
        }

        // Invoice card
        funding?.let {
            PpqInvoiceCard(
                funding = it,
                onOpenWallet = onOpenWallet,
                onCopyInvoice = onCopyInvoice,
                onCheckStatus = onCheckStatus,
            )

            if (summary.fundingPhase == PpqFundingPhase.AWAITING_PAYMENT) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    TextButton(
                        onClick = onCheckStatus,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text("Check status")
                    }
                    TextButton(
                        onClick = onCancel,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text("Cancel")
                    }
                }
            }
        }

        if (summary.fundingPhase == PpqFundingPhase.CREATING_INVOICE) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                Text("Creating invoice…", style = MaterialTheme.typography.bodySmall)
            }
        }

        // Continue when ready
        onContinue?.let { continueAction ->
            if (summary.setupPhase == dev.disobey.mango.rust.PpqSetupPhase.READY) {
                Spacer(modifier = Modifier.height(8.dp))
                Button(
                    onClick = continueAction,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Continue")
                }
            }
        }
    }
}

/**
 * Default wallet-open implementation that tries `lightning:$bolt11` and, if no wallet is
 * installed, shows a non-blocking hint.
 */
fun defaultOnOpenWallet(context: Context, bolt11: String): Boolean {
    return if (openLightningWallet(context, bolt11)) {
        true
    } else {
        Toast.makeText(
            context,
            "No compatible wallet installed — scan the QR with a wallet on another device",
            Toast.LENGTH_LONG,
        ).show()
        false
    }
}
