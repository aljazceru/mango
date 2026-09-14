package dev.disobey.mango.ui.ppq

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Focused tests for the PPQ restore replacement-confirmation flow (finding 8 +
 * coordinator follow-up): explicit consent for EVERY restore; identical generic
 * flow for non-MANAGED modes whether or not a hidden/dormant account exists (decoy
 * sessions must not learn anything); secret-free error messaging.
 */
class PpqRestoreConfirmLogicTest {
    // ── Up-front dialog gating: consent is required for every restore ──────────

    @Test
    fun `consent is mandatory for every restore including fresh setup`() {
        // Fresh setup AND the decoy case (UI mode says None): the submit gate must
        // demand the same explicit consent, so the flow shape never hints that a
        // dormant preserved account exists.
        assertFalse(
            ppqRestoreSubmitAllowed(
                backupPasswordPresent = true,
                authReady = true,
                replaceAcknowledged = false,
            ),
        )
        assertTrue(
            ppqRestoreSubmitAllowed(
                backupPasswordPresent = true,
                authReady = true,
                replaceAcknowledged = true,
            ),
        )
    }

    @Test
    fun `password and auth are always required`() {
        for (ack in listOf(false, true)) {
            assertFalse(ppqRestoreSubmitAllowed(backupPasswordPresent = false, authReady = true, replaceAcknowledged = ack))
            assertFalse(ppqRestoreSubmitAllowed(backupPasswordPresent = true, authReady = false, replaceAcknowledged = ack))
        }
    }

    // ── Identical NONE-mode flow with / without a hidden account ───────────────

    /**
     * Observable UI transcript of a mode-"None" restore. The hidden account (if
     * any) is deliberately NOT an input — that is exactly the property under test:
     * the UI never probes storage and never branches on hidden state. The unused
     * parameter documents the two simulated worlds.
     */
    @Suppress("UNUSED_PARAMETER")
    private fun noneModeTranscript(hiddenAccountPresent: Boolean): List<String> {
        val existingManaged = false // UI-visible mode is None in both worlds
        val transcript = mutableListOf<String>()
        // 1. Consent copy shown before any submit is possible.
        transcript += "body:" + ppqRestoreConsentBody(existingManaged)
        transcript += "label:" + ppqRestoreConsentLabel(existingManaged)
        // 2. Submit gating before consent.
        transcript += "gateNoConsent:" + ppqRestoreSubmitAllowed(true, true, false)
        // 3. Acknowledgement actually sent after consent (checkbox gates submit).
        transcript += "ackSent:" + ppqRestoreSubmitAllowed(true, true, true)
        // 4. Reaction to every Rust outcome the actor can return.
        for ((success, code) in listOf(
            true to null,
            false to null,
            false to "replacement_confirmation_required",
            false to "wrong_password_or_corrupt_file",
            false to "offline",
        )) {
            transcript += "reaction:$success:$code:" + ppqRestoreFailureReaction(success, code)
        }
        // 5. Runtime re-confirmation copy (defensive path) — also generic.
        transcript += "retry:" + ppqReplaceConfirmBody()
        return transcript
    }

    @Test
    fun `none-mode flow is byte-identical with and without a hidden account`() {
        assertEquals(noneModeTranscript(hiddenAccountPresent = false), noneModeTranscript(hiddenAccountPresent = true))
    }

    @Test
    fun `none-mode transcript is non-vacuous — consent gate and ack are enforced`() {
        val t = noneModeTranscript(hiddenAccountPresent = false)
        assertTrue(t.contains("gateNoConsent:false"))
        assertTrue(t.contains("ackSent:true"))
        assertTrue(t.any { it.startsWith("body:") && it.contains("PPQ account for this device") })
        assertTrue(t.any { it.startsWith("reaction:false:replacement_confirmation_required:") &&
            it.endsWith("OfferReplacementConfirmation") })
    }

    @Test
    fun `generic copy never discloses or claims proof about device accounts`() {
        for (text in listOf(
            ppqRestoreConsentBody(existingManaged = false),
            ppqRestoreConsentLabel(existingManaged = false),
            ppqReplaceConfirmBody(),
            ppqRestoreErrorMessage("replacement_confirmation_required"),
        )) {
            // No assertion that an account exists on this device…
            assertFalse(text.contains("already has a managed PPQ account"))
            assertFalse(text.contains("already has a PPQ account"))
            // …no claim that a different/stored account was found…
            assertFalse(text.contains("different PPQ account"))
            assertFalse(text.contains("belongs to a different account"))
            assertFalse(text.contains("stored on this device"))
            // …and no claim that the backup/server proves anything about device state.
            assertFalse(text.contains("prov"))
            assertFalse(text.contains("server"))
            assertFalse(text.contains("confirmed that"))
        }
    }

    @Test
    fun `visible managed account keeps its explicit specific consent`() {
        val body = ppqRestoreConsentBody(existingManaged = true)
        assertTrue(body.contains("managed PPQ account"))
        assertTrue(body.contains("permanently replace"))
        assertTrue(ppqRestoreConsentLabel(existingManaged = true).startsWith("I understand"))
        // The generic variant must differ from the visible-account variant.
        assertFalse(body == ppqRestoreConsentBody(existingManaged = false))
    }

    // ── Runtime routing of Rust responses ─────────────────────────────────────

    @Test
    fun `replacement-required error is recognized regardless of a visible account`() {
        assertTrue(isPpqReplacementConfirmationRequired("replacement_confirmation_required"))
        assertFalse(isPpqReplacementConfirmationRequired(null))
        assertFalse(isPpqReplacementConfirmationRequired("wrong_password_or_corrupt_file"))
        assertFalse(isPpqReplacementConfirmationRequired(""))
    }

    @Test
    fun `successful restore never triggers confirmation or error`() {
        assertEquals(PpqRestoreFailureReaction.None, ppqRestoreFailureReaction(success = true, errorCode = null))
        assertEquals(
            PpqRestoreFailureReaction.None,
            ppqRestoreFailureReaction(success = true, errorCode = "replacement_confirmation_required"),
        )
    }

    @Test
    fun `replacement-required failure offers generic re-confirmation`() {
        assertEquals(
            PpqRestoreFailureReaction.OfferReplacementConfirmation,
            ppqRestoreFailureReaction(success = false, errorCode = "replacement_confirmation_required"),
        )
    }

    @Test
    fun `other failures map to secret-free user messages`() {
        val wrongPassword = ppqRestoreFailureReaction(success = false, errorCode = "wrong_password_or_corrupt_file")
        assertTrue(wrongPassword is PpqRestoreFailureReaction.ShowError)
        assertEquals("Wrong backup password or unreadable backup file", (wrongPassword as PpqRestoreFailureReaction.ShowError).message)

        val offline = ppqRestoreFailureReaction(success = false, errorCode = "offline")
        assertEquals("Restore needs a network connection to verify the account", (offline as PpqRestoreFailureReaction.ShowError).message)

        val unknown = ppqRestoreFailureReaction(success = false, errorCode = "something_new_from_rust")
        assertEquals("Restore failed", (unknown as PpqRestoreFailureReaction.ShowError).message)

        val exception = ppqRestoreFailureReaction(success = false, errorCode = null)
        assertEquals("Restore failed", (exception as PpqRestoreFailureReaction.ShowError).message)
    }

    @Test
    fun `error messages never echo credentials`() {
        val messages = listOf(
            "wrong_password_or_corrupt_file",
            "unsupported_version",
            "invalid_file",
            "offline",
            "replacement_confirmation_required",
            "unheard_of_code",
            null,
        ).map { ppqRestoreErrorMessage(it) }
        for (message in messages) {
            assertTrue(message.isNotEmpty())
            assertFalse(message.contains("password=", ignoreCase = true))
            assertFalse(message.contains("pin=", ignoreCase = true))
        }
    }
}
