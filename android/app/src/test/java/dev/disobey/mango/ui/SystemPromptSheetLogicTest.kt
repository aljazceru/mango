package dev.disobey.mango.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Unit tests for the production [systemPromptSaveValue] helper used by the
 * Instructions (system prompt) sheet and its caller. The actual draft-lifecycle
 * behavior (prefill, edit, cancel, conversation switch, reopen) is covered by
 * the Compose instrumented tests in [SystemPromptSheetInstrumentedTest].
 */
class SystemPromptSheetLogicTest {

    @Test
    fun `non-blank draft is stored verbatim`() {
        assertEquals("You are concise.", systemPromptSaveValue("You are concise."))
    }

    @Test
    fun `blank or whitespace draft clears the system prompt`() {
        assertNull(systemPromptSaveValue(""))
        assertNull(systemPromptSaveValue("   "))
    }
}
