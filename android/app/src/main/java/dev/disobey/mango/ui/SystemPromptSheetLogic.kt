package dev.disobey.mango.ui

/**
 * Value dispatched on Save: a blank draft clears the conversation's system
 * prompt (null); anything else is stored verbatim. Shared by the sheet and
 * its caller so the clear-on-blank rule lives in exactly one place.
 */
internal fun systemPromptSaveValue(draft: String): String? = draft.ifBlank { null }
