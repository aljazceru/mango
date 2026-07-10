package dev.disobey.mango.ui

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ChatInteractionLogicTest {
    @Test
    fun `composer text remains editable while response is active`() {
        assertTrue(canEditComposerText())
    }

    @Test
    fun `send is disabled while response is active`() {
        assertFalse(canSendComposerText(inputText = "next prompt", isResponseActive = true))
    }

    @Test
    fun `send is enabled for nonblank draft after response finishes`() {
        assertTrue(canSendComposerText(inputText = "next prompt", isResponseActive = false))
    }

    @Test
    fun `blank or whitespace draft cannot be sent`() {
        assertFalse(canSendComposerText(inputText = "", isResponseActive = false))
        assertFalse(canSendComposerText(inputText = "   ", isResponseActive = false))
    }

    @Test
    fun `attachment mutation is disabled while response is active`() {
        assertFalse(canMutateComposerAttachment(isResponseActive = true))
        assertTrue(canMutateComposerAttachment(isResponseActive = false))
    }

    @Test
    fun `reversed chat list is at bottom only at item zero with no offset`() {
        assertTrue(isChatListAtBottom(firstVisibleItemIndex = 0, firstVisibleItemScrollOffset = 0))
        assertFalse(isChatListAtBottom(firstVisibleItemIndex = 0, firstVisibleItemScrollOffset = 1))
        assertFalse(isChatListAtBottom(firstVisibleItemIndex = 1, firstVisibleItemScrollOffset = 0))
    }

    @Test
    fun `auto pin only when already bottom new conversation or explicit request`() {
        assertTrue(
            shouldAutoPinChat(
                wasAtBottomBeforeUpdate = true,
                isNewConversation = false,
                userRequestedBottom = false,
            ),
        )
        assertTrue(
            shouldAutoPinChat(
                wasAtBottomBeforeUpdate = false,
                isNewConversation = true,
                userRequestedBottom = false,
            ),
        )
        assertTrue(
            shouldAutoPinChat(
                wasAtBottomBeforeUpdate = false,
                isNewConversation = false,
                userRequestedBottom = true,
            ),
        )
        assertFalse(
            shouldAutoPinChat(
                wasAtBottomBeforeUpdate = false,
                isNewConversation = false,
                userRequestedBottom = false,
            ),
        )
    }
}
