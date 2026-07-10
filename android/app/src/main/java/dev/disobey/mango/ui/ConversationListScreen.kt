package dev.disobey.mango.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.ConversationSummary
import java.util.Calendar
import java.util.concurrent.TimeUnit

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConversationListScreen(
    state: AppState,
    onSelect: (String) -> Unit,
    onNew: () -> Unit,
    onDelete: (String) -> Unit,
    onRename: (String, String) -> Unit,
    onFork: (String) -> Unit,
    topBarActions: @Composable (RowScope.() -> Unit) = {},
) {
    var renameTarget by remember { mutableStateOf<ConversationSummary?>(null) }
    var renameText by remember { mutableStateOf("") }
    var deleteTarget by remember { mutableStateOf<String?>(null) }
    val haptics = LocalHapticFeedback.current
    var query by remember { mutableStateOf("") }
    val visibleConversations = remember(state.conversations, query) {
        filterConversations(state.conversations, query)
    }
    val groupedConversations = remember(visibleConversations) {
        groupConversationsByDate(visibleConversations)
    }

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("Conversations") }, actions = topBarActions)
        },
        floatingActionButton = {
            FloatingActionButton(onClick = onNew) {
                Icon(Icons.Default.Add, contentDescription = "New Conversation")
            }
        },
    ) { padding ->
        if (state.conversations.isEmpty()) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 32.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    "No conversations yet",
                    style = MaterialTheme.typography.titleMedium,
                )
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    "Start a new conversation to chat with a private AI.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                )
                Spacer(modifier = Modifier.height(16.dp))
                Button(onClick = onNew) { Text("New Conversation") }
                Spacer(modifier = Modifier.height(12.dp))
                StarterPromptList(onPrompt = { onNew() })
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
            ) {
                item(key = "search") {
                    OutlinedTextField(
                        value = query,
                        onValueChange = { query = it },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 16.dp, vertical = 8.dp),
                        leadingIcon = {
                            Icon(Icons.Default.Search, contentDescription = null)
                        },
                        placeholder = { Text("Search conversations") },
                        singleLine = true,
                    )
                }
                if (visibleConversations.isEmpty()) {
                    item(key = "empty_search") {
                        Text(
                            "No matching conversations",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = TextAlign.Center,
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(32.dp),
                        )
                    }
                }
                groupedConversations.forEach { group ->
                    item(key = "header_${group.label}") {
                        Text(
                            group.label,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        )
                    }
                    items(group.conversations, key = { it.id }) { conversation ->
                        val dismissState = rememberSwipeToDismissBoxState(
                            confirmValueChange = { value ->
                                if (value == SwipeToDismissBoxValue.EndToStart) {
                                    deleteTarget = conversation.id
                                    false // let confirmation dialog handle the actual delete
                                } else false
                            }
                        )

                        SwipeToDismissBox(
                            state = dismissState,
                            enableDismissFromStartToEnd = false,
                            backgroundContent = {
                                Box(
                                    modifier = Modifier
                                        .fillMaxSize()
                                        .background(MaterialTheme.colorScheme.errorContainer)
                                        .padding(horizontal = 16.dp),
                                    contentAlignment = Alignment.CenterEnd,
                                ) {
                                    Icon(
                                        Icons.Default.Delete,
                                        contentDescription = "Delete",
                                        tint = MaterialTheme.colorScheme.onErrorContainer,
                                    )
                                }
                            },
                        ) {
                            ConversationRow(
                                conversation = conversation,
                                onClick = { onSelect(conversation.id) },
                                onRename = {
                                    renameTarget = conversation
                                    renameText = conversation.title
                                },
                                onFork = { onFork(conversation.id) },
                                onDelete = { deleteTarget = conversation.id },
                            )
                        }
                    }
                }
            }
        }
    }

    // Rename dialog
    renameTarget?.let { target ->
        AlertDialog(
            onDismissRequest = { renameTarget = null },
            title = { Text("Rename Conversation") },
            text = {
                OutlinedTextField(
                    value = renameText,
                    onValueChange = { renameText = it },
                    placeholder = { Text("Conversation name") },
                    singleLine = true,
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        val trimmed = renameText.trim()
                        if (trimmed.isNotEmpty()) onRename(target.id, trimmed)
                        renameTarget = null
                    },
                ) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { renameTarget = null }) { Text("Cancel") }
            },
        )
    }

    // Delete confirmation dialog
    deleteTarget?.let { targetId ->
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete Conversation") },
            text = { Text("Delete this conversation and all its messages? This cannot be undone.") },
            confirmButton = {
                Button(
                    onClick = {
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        onDelete(targetId)
                        deleteTarget = null
                    },
                    colors = androidx.compose.material3.ButtonDefaults.buttonColors(
                        containerColor = MaterialTheme.colorScheme.error,
                    ),
                ) { Text("Delete") }
            },
            dismissButton = {
                TextButton(onClick = { deleteTarget = null }) { Text("Cancel") }
            },
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ConversationRow(
    conversation: ConversationSummary,
    onClick: () -> Unit,
    onRename: () -> Unit,
    onFork: () -> Unit,
    onDelete: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .combinedClickable(
                onClick = onClick,
                onLongClick = { menuOpen = true },
            ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            DropdownMenu(
                expanded = menuOpen,
                onDismissRequest = { menuOpen = false },
            ) {
                DropdownMenuItem(
                    text = { Text("Rename") },
                    onClick = {
                        menuOpen = false
                        onRename()
                    },
                )
                DropdownMenuItem(
                    text = { Text("Fork") },
                    onClick = {
                        menuOpen = false
                        onFork()
                    },
                )
                DropdownMenuItem(
                    text = {
                        Text(
                            "Delete",
                            color = MaterialTheme.colorScheme.error,
                        )
                    },
                    onClick = {
                        menuOpen = false
                        onDelete()
                    },
                )
            }
            Text(
                conversation.title,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            val metadata = conversationMetadata(
                relativeTime(conversation.updatedAt),
                shortModelName(conversation.modelId),
            )
            if (metadata.isNotEmpty()) {
                Text(
                    metadata,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
internal fun StarterPromptList(onPrompt: (String) -> Unit) {
    val prompts = starterPrompts()
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        prompts.forEach { prompt ->
            TextButton(onClick = { onPrompt(prompt) }) {
                Text(prompt)
            }
        }
    }
}

// MARK: - Helpers

internal data class ConversationDateGroup(
    val label: String,
    val conversations: List<ConversationSummary>,
)

internal fun filterConversations(
    conversations: List<ConversationSummary>,
    query: String,
): List<ConversationSummary> {
    val normalized = query.trim()
    if (normalized.isEmpty()) return conversations
    return conversations.filter { conversation ->
        conversation.title.contains(normalized, ignoreCase = true) ||
            conversation.modelId.contains(normalized, ignoreCase = true) ||
            conversation.backendId.contains(normalized, ignoreCase = true)
    }
}

internal fun groupConversationsByDate(
    conversations: List<ConversationSummary>,
    nowMillis: Long = System.currentTimeMillis(),
): List<ConversationDateGroup> {
    return conversations
        .groupBy { conversationDateBucket(it.updatedAt, nowMillis) }
        .map { (label, items) -> ConversationDateGroup(label, items) }
        .sortedBy { group ->
            when (group.label) {
                "Today" -> 0
                "Yesterday" -> 1
                "Previous 7 days" -> 2
                else -> 3
            }
        }
}

internal fun conversationDateBucket(epochMillis: Long, nowMillis: Long): String {
    val todayStart = startOfDayMillis(nowMillis)
    val itemStart = startOfDayMillis(epochMillis)
    val dayDiff = TimeUnit.MILLISECONDS.toDays(todayStart - itemStart)
    return when {
        dayDiff <= 0 -> "Today"
        dayDiff == 1L -> "Yesterday"
        dayDiff <= 7L -> "Previous 7 days"
        else -> "Earlier"
    }
}

private fun startOfDayMillis(epochMillis: Long): Long {
    val calendar = Calendar.getInstance()
    calendar.timeInMillis = epochMillis
    calendar.set(Calendar.HOUR_OF_DAY, 0)
    calendar.set(Calendar.MINUTE, 0)
    calendar.set(Calendar.SECOND, 0)
    calendar.set(Calendar.MILLISECOND, 0)
    return calendar.timeInMillis
}

internal fun conversationMetadata(vararg segments: String?): String {
    return segments
        .mapNotNull { it?.takeIf { segment -> segment.isNotBlank() } }
        .joinToString(" · ")
}

internal fun starterPrompts(): List<String> = listOf(
    "Summarize a sensitive document",
    "Draft a private decision memo",
    "Help me compare confidential options",
)

internal fun relativeTime(epochMillis: Long): String {
    val now = System.currentTimeMillis()
    val diff = now - epochMillis
    return when {
        diff < TimeUnit.MINUTES.toMillis(1) -> "just now"
        diff < TimeUnit.HOURS.toMillis(1) -> "${TimeUnit.MILLISECONDS.toMinutes(diff)}m ago"
        diff < TimeUnit.DAYS.toMillis(1) -> "${TimeUnit.MILLISECONDS.toHours(diff)}h ago"
        diff < TimeUnit.DAYS.toMillis(7) -> "${TimeUnit.MILLISECONDS.toDays(diff)}d ago"
        else -> {
            val days = TimeUnit.MILLISECONDS.toDays(diff)
            "${days / 7}w ago"
        }
    }
}

internal fun shortModelName(modelId: String): String {
    val lastSlash = modelId.lastIndexOf('/')
    return if (lastSlash >= 0) modelId.substring(lastSlash + 1) else modelId
}
