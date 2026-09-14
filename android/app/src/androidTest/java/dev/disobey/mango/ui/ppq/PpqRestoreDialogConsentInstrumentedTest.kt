package dev.disobey.mango.ui.ppq

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import dev.disobey.mango.PpqBackupCoordinator
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Instrumented Compose coverage for the PPQ restore consent flow (finding 8 +
 * follow-up): the dialog must show EXPLICIT consent — the visible managed-account
 * risk text, or the generic text for mode "None" — and only send
 * `replaceAcknowledged = true` after the user checks the acknowledgement box.
 * Runs against the isolated debug applicationId; no real account or file access.
 */
class PpqRestoreDialogConsentInstrumentedTest {

    @get:Rule
    val compose = createComposeRule()

    @After
    fun clearCoordinator() {
        PpqBackupCoordinator.onPpqImportResult = null
    }

    /** Fill password + PIN so only the consent gate can keep submit disabled. */
    private fun fillAuthFields() {
        compose.waitUntil(5_000) {
            compose.onAllNodes(hasSetTextAction()).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Backup password").performTextReplacement("hunter2hunter2")
        compose.onNodeWithText("Current app PIN").performTextReplacement("123456")
        compose.waitForIdle()
    }

    @Test
    fun visibleManagedAccount_showsRiskText_and_blocksSubmitUntilAcknowledged() {
        var acknowledged: Boolean? = null
        compose.setContent {
            PpqRestoreDialog(
                biometricAvailable = false,
                existingManaged = true,
                onRestore = { _, _, _, _, replaceAcknowledged -> acknowledged = replaceAcknowledged },
                filePicker = {},
                onDismiss = {},
            )
        }

        // The visible-account risk text is displayed before any submit is possible.
        compose.onNodeWithText("This device already has a managed PPQ account.", substring = true)
            .assertIsDisplayed()
        compose.onNodeWithText("I understand the PPQ account on this device may be replaced", substring = true)
            .assertIsDisplayed()

        fillAuthFields()
        // Password + auth alone are NOT enough: consent still gates the submit.
        compose.onNodeWithText("Select backup file").assertIsNotEnabled()

        compose.onNodeWithText("I understand the PPQ account on this device may be replaced", substring = true)
            .performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Select backup file").assertIsEnabled()
    }

    @Test
    fun visibleManagedAccount_sendsAcknowledgementOnlyAfterConsent() {
        var acknowledged: Boolean? = null
        var delivered = false
        compose.setContent {
            PpqRestoreDialog(
                biometricAvailable = false,
                existingManaged = true,
                onRestore = { _, _, _, _, replaceAcknowledged ->
                    acknowledged = replaceAcknowledged
                    delivered = true
                },
                filePicker = {
                    // The dialog registers the import closure before the picker runs;
                    // simulate MainActivity delivering the picked bytes.
                    PpqBackupCoordinator.onPpqImportResult?.invoke(byteArrayOf(9))
                },
                onDismiss = {},
            )
        }

        fillAuthFields()
        compose.onNodeWithText("I understand the PPQ account on this device may be replaced", substring = true)
            .performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Select backup file").performClick()
        compose.waitForIdle()

        assertTrue(delivered)
        assertEquals(true, acknowledged)
        // The one-shot import closure was consumed by the delivery.
        assertNull(PpqBackupCoordinator.onPpqImportResult)
    }

    @Test
    fun modeNone_showsGenericConsent_and_requiresTheSameAcknowledgement() {
        var acknowledged: Boolean? = null
        compose.setContent {
            PpqRestoreDialog(
                biometricAvailable = false,
                existingManaged = false,
                onRestore = { _, _, _, _, replaceAcknowledged -> acknowledged = replaceAcknowledged },
                filePicker = {},
                onDismiss = {},
            )
        }

        // Generic copy — identical whether or not anything is stored on the device;
        // must not assert that an account exists here.
        compose.onNodeWithText("Restoring makes this backup the PPQ account for this device.", substring = true)
            .assertIsDisplayed()
        compose.onNodeWithText("I understand restoring may replace PPQ credentials", substring = true)
            .assertIsDisplayed()

        fillAuthFields()
        // Identical gating: fresh setup and decoy sessions demand the same consent.
        compose.onNodeWithText("Select backup file").assertIsNotEnabled()
        compose.onNodeWithText("I understand restoring may replace PPQ credentials", substring = true)
            .performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Select backup file").assertIsEnabled()
    }

    @Test
    fun replaceConfirmDialog_showsGenericCopy_andReportsConsentChoice() {
        var confirmed = false
        var cancelled = false
        var showing = true
        compose.setContent {
            if (showing) {
                PpqReplaceConfirmDialog(
                    onConfirmReplace = {
                        confirmed = true
                        showing = false
                    },
                    onDismiss = {
                        cancelled = true
                        showing = false
                    },
                )
            }
        }

        compose.onNodeWithText("Restore and replace?").assertIsDisplayed()
        // Generic, non-disclosing body: no claim that a stored/different account exists.
        compose.onNodeWithText("Any PPQ credentials currently stored here would be permanently replaced", substring = true)
            .assertIsDisplayed()
        compose.onNodeWithText("Cancel").performClick()
        compose.waitForIdle()

        assertTrue(cancelled)
        assertFalse(confirmed)
    }
}
