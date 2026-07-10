package dev.disobey.mango.ui

internal fun canEditComposerText(): Boolean = true

internal fun canMutateComposerAttachment(isResponseActive: Boolean): Boolean = !isResponseActive

internal fun canSendComposerText(
    inputText: String,
    isResponseActive: Boolean,
): Boolean = inputText.isNotBlank() && !isResponseActive

internal fun isChatListAtBottom(
    firstVisibleItemIndex: Int,
    firstVisibleItemScrollOffset: Int,
): Boolean = firstVisibleItemIndex == 0 && firstVisibleItemScrollOffset == 0

internal fun shouldAutoPinChat(
    wasAtBottomBeforeUpdate: Boolean,
    isNewConversation: Boolean,
    userRequestedBottom: Boolean,
): Boolean = wasAtBottomBeforeUpdate || isNewConversation || userRequestedBottom
