package dev.disobey.mango.ui

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performTextInput
import org.junit.Rule
import org.junit.Test

/**
 * Instrumented behavioral tests for the chat composer send button.
 * Covers the enabled/disabled gate for empty, blocked, and drafted input.
 * Runs against the isolated debug applicationId (dev.disobey.mango.dev) so it
 * never touches an installed personal build or its data.
 */
class ComposeBarInstrumentedTest {

    @get:Rule
    val compose = createComposeRule()

    private fun mount(isStreaming: () -> Boolean = { false }, showStopButton: Boolean = false) {
        compose.setContent {
            ComposeBar(
                pendingAttachment = null,
                isStreaming = isStreaming(),
                showStopButton = showStopButton,
                onSend = {},
                onStop = {},
                onAttach = {},
                onClearAttachment = {},
            )
        }
    }

    @Test
    fun sendDisabledWhenComposerEmpty() {
        mount()
        compose.onNodeWithContentDescription("Send message").assertIsNotEnabled()
    }

    @Test
    fun sendEnabledWithDraftText() {
        mount()
        compose.onNode(hasSetTextAction()).performTextInput("hello")
        compose.onNodeWithContentDescription("Send message").assertIsEnabled()
    }

    @Test
    fun sendStaysDisabledWhileInputBlocked() {
        val streaming = mutableStateOf(false)
        mount(isStreaming = { streaming.value })
        compose.onNode(hasSetTextAction()).performTextInput("draft")
        compose.runOnIdle { streaming.value = true }
        compose.onNodeWithContentDescription("Send message").assertIsNotEnabled()
    }

    @Test
    fun stopReplacesSendDuringGeneration() {
        mount(isStreaming = { true }, showStopButton = true)
        compose.onNodeWithContentDescription("Stop generating").assertIsEnabled()
        compose.onNodeWithContentDescription("Send message").assertDoesNotExist()
    }
}
