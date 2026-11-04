package dev.disobey.mango.ui

import dev.disobey.mango.rust.UiMessage
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

class RouteDividerLogicTest {
    private fun msg(
        role: String,
        decision: String? = null,
        backend: String? = null,
        model: String? = null,
        provider: String? = null,
        tee: String? = null,
        verified: Boolean? = null,
    ) = UiMessage(
        id = java.util.UUID.randomUUID().toString(),
        role = role,
        content = "text",
        createdAt = 0L,
        hasAttachment = false,
        attachmentName = null,
        ragContextCount = null,
        imagePath = null,
        routeBackendId = backend,
        routeModelId = model,
        routeDecision = decision,
        routeReason = null,
        routeProviderName = provider,
        routeTeeLabel = tee,
        routeTeeVerified = verified,
    )

    @Test
    fun `divider shows above the first routed reply`() {
        val messages = listOf(
            msg("user"),
            msg("assistant", decision = "remote", provider = "PPQ.AI",
                model = "private/gpt-oss-120b", tee = "AMD SEV-SNP", verified = true),
        )
        assertNull(routeDividerLabel(messages, 0))
        assertEquals(
            "Sealed in PPQ.AI · AMD SEV-SNP verified",
            routeDividerLabel(messages, 1),
        )
    }

    @Test
    fun `no divider while the route is unchanged`() {
        val messages = listOf(
            msg("user"),
            msg("assistant", decision = "remote", backend = "ppq", provider = "PPQ.AI",
                model = "gpt-oss-120b", tee = "AMD SEV-SNP", verified = true),
            msg("user"),
            msg("assistant", decision = "remote", backend = "ppq", provider = "PPQ.AI",
                model = "gpt-oss-120b", tee = "AMD SEV-SNP", verified = true),
        )
        assertNull(routeDividerLabel(messages, 3))
    }

    @Test
    fun `divider appears when the route changes mid-thread`() {
        val messages = listOf(
            msg("user"),
            msg("assistant", decision = "remote", backend = "ppq", provider = "PPQ.AI",
                model = "gpt-oss-120b", tee = "AMD SEV-SNP", verified = true),
            msg("user"),
            msg("assistant", decision = "local", backend = "qvac-local",
                model = "qwen2.5-0.5b"),
        )
        assertEquals(
            "On this phone · qwen2.5-0.5b",
            routeDividerLabel(messages, 3),
        )
    }

    @Test
    fun `unrouted legacy messages do not break a run`() {
        val messages = listOf(
            msg("user"),
            msg("assistant", decision = "remote", backend = "ppq", provider = "PPQ.AI",
                model = "gpt-oss-120b", tee = "AMD SEV-SNP", verified = true),
            msg("user"),
            msg("assistant"), // legacy reply without provenance
            msg("user"),
            msg("assistant", decision = "remote", backend = "ppq", provider = "PPQ.AI",
                model = "gpt-oss-120b", tee = "AMD SEV-SNP", verified = true),
        )
        assertNull(routeDividerLabel(messages, 3))
        assertNull(routeDividerLabel(messages, 5))
    }
}
