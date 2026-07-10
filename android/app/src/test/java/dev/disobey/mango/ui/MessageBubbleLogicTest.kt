package dev.disobey.mango.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class MessageBubbleLogicTest {
    @Test
    fun `route metadata label includes route provider model and verified tee`() {
        assertEquals(
            "Remote · OpenAI / gpt-4.1 · TEE verified",
            routeMetadataLabel(
                providerName = "OpenAI",
                modelId = "gpt-4.1",
                decision = "remote",
                teeLabel = "TEE",
                teeVerified = true,
            ),
        )
    }

    @Test
    fun `route metadata label omits missing segments`() {
        assertEquals(
            "Local · llama-3.2",
            routeMetadataLabel(
                providerName = null,
                modelId = "llama-3.2",
                decision = "local",
                teeLabel = null,
                teeVerified = null,
            ),
        )
    }

    @Test
    fun `route metadata label is absent for legacy messages`() {
        assertNull(
            routeMetadataLabel(
                providerName = null,
                modelId = null,
                decision = null,
                teeLabel = null,
                teeVerified = null,
            ),
        )
    }
}
