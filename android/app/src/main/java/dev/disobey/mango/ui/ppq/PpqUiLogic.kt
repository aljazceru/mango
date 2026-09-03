package dev.disobey.mango.ui.ppq

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.ContextCompat
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Pure PPQ UI helpers. No secret state, no I/O except explicit one-shot helpers.
 */

/**
 * Build an Android `lightning:` URI from a BOLT11 invoice string.
 * Returns null if the invoice does not look like a mainnet/testnet payment request
 * or contains whitespace.
 */
fun buildLightningUri(bolt11: String): String? {
    val trimmed = bolt11.trim()
    if (trimmed.isBlank()) return null
    if (trimmed.contains(Regex("\\s"))) return null
    if (!trimmed.startsWith("lnbc") && !trimmed.startsWith("lntb")) return null
    return "lightning:$trimmed"
}

/**
 * Try to open the given BOLT11 invoice in an external Lightning wallet.
 * Catches [ActivityNotFoundException] and returns false so the caller can show a hint.
 */
fun openLightningWallet(context: Context, bolt11: String): Boolean {
    val uri = buildLightningUri(bolt11) ?: return false
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse(uri))
    return try {
        ContextCompat.startActivity(context, intent, null)
        true
    } catch (_: ActivityNotFoundException) {
        false
    } catch (e: Exception) {
        android.util.Log.e("PpqUiLogic", "openLightningWallet failed: ${e.message}")
        false
    }
}

/**
 * Copy a BOLT11 invoice to the system clipboard. Only called on explicit user tap.
 */
fun copyInvoiceToClipboard(context: Context, bolt11: String) {
    // Threat review (low): mark the clip sensitive so Android 13+ excludes it
    // from clipboard history/preview.
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? android.content.ClipboardManager
    clipboard?.setPrimaryClip(android.content.ClipData.newPlainText("Lightning invoice", bolt11))
}

/**
 * Parse a free-form sats input string. Returns the value only if it is a positive integer
 * and (when limits are supplied) within [minSats, maxSats].
 */
fun parseSatsAmount(
    text: String,
    minSats: ULong? = null,
    maxSats: ULong? = null,
): ULong? {
    val trimmed = text.trim()
    if (trimmed.isEmpty()) return null
    val value = trimmed.toULongOrNull() ?: return null
    if (value == 0uL) return null
    if (minSats != null && value < minSats) return null
    if (maxSats != null && value > maxSats) return null
    return value
}

/**
 * Format remaining seconds until expiry as a concise human-readable string.
 * Returns a fallback when expiry has already passed.
 */
fun formatExpiryCountdown(expiresAt: Long, now: Long): String {
    val remaining = expiresAt - now
    return if (remaining <= 0) {
        "Expired"
    } else if (remaining < 60) {
        "$remaining s"
    } else if (remaining < 3600) {
        "${remaining / 60} m"
    } else {
        val hours = remaining / 3600
        val minutes = (remaining % 3600) / 60
        "${hours}h ${minutes}m"
    }
}

/**
 * Format a Unix timestamp as a short local date/time string.
 */
fun formatPpqTimestamp(epochSeconds: Long): String {
    return try {
        SimpleDateFormat("yyyy-MM-dd HH:mm", Locale.getDefault())
            .format(Date(epochSeconds * 1000L))
    } catch (_: Exception) {
        "—"
    }
}

/**
 * Suggested filename for a PPQ recovery backup.
 */
fun ppqBackupFileName(): String {
    val date = SimpleDateFormat("yyyyMMdd", Locale.getDefault()).format(Date())
    return "mango-ppq-backup-$date.mppq"
}

/**
 * Render a PPQ credit balance for humans: the raw wire text can carry
 * float64 serialization noise ("0.9237520278000001"). The exact text stays
 * authoritative in state; this is display-only rounding — pure decimal-string
 * math, never through Double (plan §6.3).
 *
 * Keeps at most 4 fraction digits (round-half-up with carry), trims trailing
 * zeros: "0.9237520278000001" -> "0.9238", "1.0000" -> "1", "0.1666" -> "0.1666".
 */
fun formatPpqBalance(raw: String?): String {
    if (raw.isNullOrBlank()) return "—"
    val text = raw.trim()
    val dot = text.indexOf('.')
    if (dot < 0) return text
    val whole = text.substring(0, dot)
    var fraction = text.substring(dot + 1).filter { it.isDigit() }
    if (fraction.length <= 4) {
        val trimmed = fraction.trimEnd('0')
        return if (trimmed.isEmpty()) whole else "$whole.$trimmed"
    }
    // Round half-up on the 5th digit, with carry into whole.
    val keep = fraction.substring(0, 4)
    val roundUp = fraction[4] >= '5'
    var frac = if (roundUp) {
        val v = keep.toLong() + 1
        if (v >= 10_000) {
            return (whole.toLong() + v / 10_000).toString()
        }
        String.format("%04d", v)
    } else {
        keep
    }
    frac = frac.trimEnd('0')
    return if (frac.isEmpty()) whole else "$whole.$frac"
}
