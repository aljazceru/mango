package dev.disobey.mango.ui

import dev.disobey.mango.rust.ConversationSummary
import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.concurrent.TimeUnit

class ConversationListLogicTest {
    @Test
    fun `search filters title model and backend`() {
        val conversations = listOf(
            conversation("1", title = "Legal memo", modelId = "openai/gpt-4.1", backendId = "remote"),
            conversation("2", title = "Roadmap", modelId = "local/llama", backendId = "device"),
        )

        assertEquals(listOf("1"), filterConversations(conversations, "legal").map { it.id })
        assertEquals(listOf("2"), filterConversations(conversations, "llama").map { it.id })
        assertEquals(listOf("1"), filterConversations(conversations, "remote").map { it.id })
        assertEquals(listOf("1", "2"), filterConversations(conversations, " ").map { it.id })
    }

    @Test
    fun `date buckets use today yesterday previous seven days and earlier`() {
        val now = TimeUnit.DAYS.toMillis(20)

        assertEquals("Today", conversationDateBucket(now, now))
        assertEquals("Yesterday", conversationDateBucket(now - TimeUnit.DAYS.toMillis(1), now))
        assertEquals("Previous 7 days", conversationDateBucket(now - TimeUnit.DAYS.toMillis(4), now))
        assertEquals("Earlier", conversationDateBucket(now - TimeUnit.DAYS.toMillis(10), now))
    }

    @Test
    fun `metadata joins only nonblank segments`() {
        assertEquals("2h ago · gpt-4.1", conversationMetadata("2h ago", "", "gpt-4.1"))
        assertEquals("llama", conversationMetadata(null, "llama"))
        assertEquals("", conversationMetadata("", " "))
    }

    private fun conversation(
        id: String,
        title: String,
        modelId: String,
        backendId: String,
        updatedAt: Long = 0L,
    ): ConversationSummary = ConversationSummary(
        id = id,
        title = title,
        modelId = modelId,
        backendId = backendId,
        updatedAt = updatedAt,
        systemPrompt = null,
        toolsEnabled = false,
    )
}
