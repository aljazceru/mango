package dev.disobey.mango.ui.ppq

import android.graphics.Bitmap
import android.graphics.Color
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import dev.disobey.mango.rust.PpqFundingSummary
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext

private const val QR_SIZE_PX = 512
private const val QR_QUIET_ZONE = 4

/**
 * Generate a QR bitmap for a BOLT11 invoice using ZXing.
 */
fun bolt11QrBitmap(text: String, size: Int = QR_SIZE_PX): Bitmap? {
    if (text.isBlank()) return null
    return try {
        val hints = mapOf(EncodeHintType.MARGIN to QR_QUIET_ZONE)
        val bitMatrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size, hints)
        val width = bitMatrix.width
        val height = bitMatrix.height
        val bmp = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        for (x in 0 until width) {
            for (y in 0 until height) {
                bmp.setPixel(
                    x,
                    y,
                    if (bitMatrix.get(x, y)) Color.BLACK else Color.WHITE,
                )
            }
        }
        bmp
    } catch (e: Exception) {
        android.util.Log.e("PpqInvoiceCard", "QR generation failed: ${e.message}")
        null
    }
}

/**
 * Card that displays a pending Lightning invoice: amount, expiry countdown, QR code,
 * and actions to open a wallet, copy the invoice, or check payment status.
 */
@Composable
fun PpqInvoiceCard(
    funding: PpqFundingSummary,
    onOpenWallet: (bolt11: String) -> Unit,
    onCopyInvoice: (bolt11: String) -> Unit,
    onCheckStatus: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val bolt11 = funding.bolt11
    val expiresAt = funding.expiresAt
    var now by remember { mutableStateOf(System.currentTimeMillis() / 1000L) }
    var qrBitmap by remember { mutableStateOf<Bitmap?>(null) }

    LaunchedEffect(bolt11) {
        qrBitmap = withContext(Dispatchers.IO) {
            bolt11QrBitmap(bolt11)
        }
    }

    LaunchedEffect(Unit) {
        while (isActive) {
            delay(1000)
            now = System.currentTimeMillis() / 1000L
        }
    }

    Card(
        modifier = modifier.fillMaxWidth(),
        elevation = CardDefaults.cardElevation(defaultElevation = 0.dp),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
        ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "${funding.amountSats} sats",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    text = "Expires in ${formatExpiryCountdown(expiresAt, now)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            qrBitmap?.let { bmp ->
                Image(
                    bitmap = bmp.asImageBitmap(),
                    contentDescription = "Lightning invoice QR code",
                    modifier = Modifier
                        .size(192.dp)
                        .align(Alignment.CenterHorizontally),
                )
            } ?: Text(
                text = "Generating QR code...",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            )

            Text(
                text = bolt11,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Button(
                    onClick = { onOpenWallet(bolt11) },
                    modifier = Modifier.weight(1f),
                ) {
                    Text("Open wallet")
                }
                OutlinedButton(
                    onClick = { onCopyInvoice(bolt11) },
                    modifier = Modifier.weight(1f),
                ) {
                    Text("Copy invoice")
                }
            }

            OutlinedButton(
                onClick = onCheckStatus,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Check status")
            }
        }
    }
}
