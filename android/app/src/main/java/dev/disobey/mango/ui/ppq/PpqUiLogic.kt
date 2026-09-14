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
 * Rust error code (PpqRecoveryResult.errorCode) returned when the backup belongs
 * to a different PPQ account than the one stored on this device and the restore
 * was not sent with an explicit replacement acknowledgement.
 */
const val PPQ_ERROR_REPLACEMENT_CONFIRMATION_REQUIRED: String = "replacement_confirmation_required"

/**
 * A restore attempt captured so it can be re-sent with `replaceAcknowledged = true`
 * after the user explicitly confirms replacement (Rust answered
 * [PPQ_ERROR_REPLACEMENT_CONFIRMATION_REQUIRED]). Holds secrets in composition
 * memory only — never logged, never persisted, cleared once consumed.
 */
class PpqRestoreAttempt(
    val bytes: ByteArray,
    val backupPassword: String,
    val useBiometric: Boolean,
    val pin: String?,
)

/**
 * True when Rust rejected a restore because it targets a different stored account.
 * Used only to route to the (generic, non-disclosing) re-confirmation dialog — the
 * error itself is never shown as proof that any particular account exists.
 */
fun isPpqReplacementConfirmationRequired(errorCode: String?): Boolean =
    errorCode == PPQ_ERROR_REPLACEMENT_CONFIRMATION_REQUIRED

/**
 * Gate for the restore dialog's submit action. EXPLICIT CONSENT IS REQUIRED FOR
 * EVERY restore — including flows where the UI shows no managed account — so the
 * requirement is identical whether or not a dormant/preserved account exists on
 * the device (a decoy session must not learn anything from the flow shape).
 */
fun ppqRestoreSubmitAllowed(
    backupPasswordPresent: Boolean,
    authReady: Boolean,
    replaceAcknowledged: Boolean,
): Boolean = backupPasswordPresent && authReady && replaceAcknowledged

/**
 * Consent body copy for the restore dialog.
 *
 * When a managed account is VISIBLE ([existingManaged]) the text names it — the
 * user already sees that account in the UI. Otherwise the text is GENERIC and
 * identical whether or not any credentials are present on the device: it never
 * asserts that a stored/different/hidden account exists, and never claims that
 * the backup file or any server response proves anything about device
 * credentials (only possession of the backup is ever known to the UI).
 */
fun ppqRestoreConsentBody(existingManaged: Boolean): String = if (existingManaged) {
    "This device already has a managed PPQ account. If this backup belongs to a " +
        "different account, restoring it will permanently replace the account on " +
        "this device: its credentials will be overwritten and any balance you have " +
        "not backed up may become unrecoverable."
} else {
    "Restoring makes this backup the PPQ account for this device. If this device " +
        "already holds PPQ credentials, they will be permanently replaced, and any " +
        "balance that was not backed up may become unrecoverable."
}

/** Checkbox label for the restore consent. Same visibility rules as [ppqRestoreConsentBody]. */
fun ppqRestoreConsentLabel(existingManaged: Boolean): String = if (existingManaged) {
    "I understand the PPQ account on this device may be replaced and I may lose " +
        "access to its remaining balance"
} else {
    "I understand restoring may replace PPQ credentials currently on this device"
}

/**
 * Generic, non-disclosing copy for the runtime re-confirmation (Rust answered
 * `replacement_confirmation_required`). Identical whether or not any credentials
 * exist on the device; asserts nothing about what is stored and attributes nothing
 * to the server or the backup file.
 */
fun ppqReplaceConfirmBody(): String =
    "To restore this backup, it becomes the PPQ account for this device. Any PPQ " +
        "credentials currently stored here would be permanently replaced, and any " +
        "balance that was not backed up may become unrecoverable. The backup file " +
        "itself is not changed."

/**
 * What the UI should do after a PPQ restore attempt completes (pure, unit-tested).
 */
sealed interface PpqRestoreFailureReaction {
    /** Restore succeeded — nothing to surface. */
    data object None : PpqRestoreFailureReaction
    /** Rust answered replacement-required — offer the explicit replacement
     * confirmation and retry with acknowledgement on consent. */
    data object OfferReplacementConfirmation : PpqRestoreFailureReaction
    /** Any other failure — show the mapped, secret-free error message. */
    data class ShowError(val message: String) : PpqRestoreFailureReaction
}

fun ppqRestoreFailureReaction(success: Boolean, errorCode: String?): PpqRestoreFailureReaction = when {
    success -> PpqRestoreFailureReaction.None
    isPpqReplacementConfirmationRequired(errorCode) -> PpqRestoreFailureReaction.OfferReplacementConfirmation
    else -> PpqRestoreFailureReaction.ShowError(ppqRestoreErrorMessage(errorCode))
}

/**
 * Human-readable, secret-free message for a failed PPQ restore. Never echoes the
 * backup password, PIN, or byte content.
 */
fun ppqRestoreErrorMessage(errorCode: String?): String = when (errorCode) {
    "wrong_password_or_corrupt_file" -> "Wrong backup password or unreadable backup file"
    "unsupported_version" -> "This backup format is not supported by this app version"
    "invalid_file" -> "Not a valid PPQ backup file"
    "offline" -> "Restore needs a network connection to verify the account"
    // Generic: never worded as "a different account was found on this device" —
    // the code is only a signal to re-ask consent, not proof of hidden state.
    PPQ_ERROR_REPLACEMENT_CONFIRMATION_REQUIRED -> "Restore needs explicit confirmation before replacing device PPQ credentials"
    else -> "Restore failed"
}

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
    val clip = android.content.ClipData.newPlainText("Lightning invoice", bolt11)
    clip.description.extras = android.os.PersistableBundle().apply {
        putBoolean(android.content.ClipDescription.EXTRA_IS_SENSITIVE, true)
    }
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? android.content.ClipboardManager
    clipboard?.setPrimaryClip(clip)
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
