package dev.disobey.mango.ui

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Instrumented behavioral tests for the Instructions (system prompt) sheet.
 * Covers prefill, edit-save, reopen, cancel, and conversation-switch reset.
 * Runs against the isolated debug applicationId (dev.disobey.mango.dev) so it
 * never touches an installed personal build or its data.
 */
class SystemPromptSheetInstrumentedTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun sheetPrefillsSavedPrompt() {
        compose.setContent {
            SystemPromptSheet(
                initialPrompt = "Be concise.",
                onSave = {},
                onDismiss = {},
            )
        }
        compose.onNodeWithText("Be concise.").assertIsDisplayed()
    }

    @Test
    fun editThenSaveDispatchesEditedText() {
        var saved: String? = null
        compose.setContent {
            SystemPromptSheet(
                initialPrompt = "old",
                onSave = { saved = it },
                onDismiss = {},
            )
        }
        compose.onNode(hasSetTextAction()).performTextReplacement("You are terse.")
        compose.onNodeWithText("Save").performClick()
        compose.waitForIdle()
        assertEquals("You are terse.", saved)
    }

    @Test
    fun reopenShowsSavedPrompt() {
        var saved: String? = null
        compose.setContent {
            var initial by remember { mutableStateOf("v1") }
            var show by remember { mutableStateOf(true) }
            if (show) {
                SystemPromptSheet(
                    initialPrompt = initial,
                    onSave = {
                        saved = it
                        initial = it
                        show = false
                    },
                    onDismiss = { show = false },
                )
            } else {
                androidx.compose.material3.Button(onClick = { show = true }) {
                    androidx.compose.material3.Text("reopen-sheet")
                }
            }
        }
        compose.waitUntil(5_000) {
            compose.onAllNodes(hasSetTextAction()).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNode(hasSetTextAction()).performTextReplacement("v2")
        compose.waitForIdle()
        compose.onNodeWithText("Save").performClick()
        compose.waitForIdle()
        assertEquals("v2", saved)

        // Reopen: the fresh draft must show the persisted value, not the
        // pre-edit one.
        compose.onNodeWithText("reopen-sheet").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodes(hasSetTextAction()).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("v2").assertIsDisplayed()
    }

    @Test
    fun cancelDiscardsDraftWithoutSaving() {
        var saved: String? = null
        var dismissed = false
        compose.setContent {
            SystemPromptSheet(
                initialPrompt = "keep me",
                onSave = { saved = it },
                onDismiss = { dismissed = true },
            )
        }
        compose.onNode(hasSetTextAction()).performTextReplacement("discarded")
        compose.onNodeWithText("Cancel").performClick()
        compose.waitForIdle()
        assertTrue(dismissed)
        assertEquals(null, saved)
    }

    @Test
    fun newInitialPromptResetsDraftOnConversationSwitch() {
        var saved = false
        compose.setContent {
            var initial by remember { mutableStateOf("prompt for A") }
            SystemPromptSheet(
                initialPrompt = initial,
                onSave = { saved = true },
                onDismiss = {},
            )
            // Drive a "conversation switch" from outside the sheet.
            androidx.compose.material3.Button(onClick = { initial = "prompt for B" }) {
                androidx.compose.material3.Text("switch-conversation")
            }
        }
        compose.onNode(hasSetTextAction()).performTextReplacement("half-typed edit")
        compose.onNodeWithText("switch-conversation").performClick()
        compose.waitForIdle()
        // LaunchedEffect(initialPrompt) must replace the stale draft.
        compose.onNodeWithText("prompt for B").assertIsDisplayed()
        assertFalse(saved)
    }
}
