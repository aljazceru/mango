package dev.disobey.mango.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.Schedule
import androidx.compose.material.icons.outlined.Shield
import androidx.compose.material.icons.outlined.Warning
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatus
import dev.disobey.mango.rust.ConversationSummary
import dev.disobey.mango.rust.InferenceRoute
import dev.disobey.mango.ui.theme.MangoTheme
import java.util.Calendar
import java.util.concurrent.TimeUnit

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConversationListScreen(
    state: AppState,
    onSelect: (String) -> Unit,
    onNew: () -> Unit,
    // Text + send from the bottom ask bar: the caller dispatches
    // NewConversation then SendMessage, in the order the core expects.
    onAsk: (String) -> Unit,
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
    var askText by remember { mutableStateOf("") }
    val visibleConversations = remember(state.conversations, query) {
        filterConversations(state.conversations, query)
    }
    val groupedConversations = remember(visibleConversations) {
        groupConversationsByDate(visibleConversations)
    }
    val trust = trustStripInfo(state)

    Column(
        modifier = Modifier
            .fillMaxSize()
            .statusBarsPadding(),
    ) {
        // Brand row: mark + wordmark, nav icon buttons on the trailing side.
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            MangoMark(28.dp)
            Spacer(modifier = Modifier.width(10.dp))
            Text("mango", style = MaterialTheme.typography.titleLarge)
            Spacer(modifier = Modifier.weight(1f))
            topBarActions()
        }

        Text(
            "Chats",
            style = MaterialTheme.typography.headlineLarge,
            modifier = Modifier.padding(horizontal = 20.dp),
        )
        Spacer(modifier = Modifier.height(8.dp))

        if (state.conversations.isNotEmpty()) {
            TextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp),
                leadingIcon = {
                    Icon(
                        Icons.Default.Search,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                placeholder = { Text("Search chats") },
                singleLine = true,
                shape = CircleShape,
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    unfocusedContainerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    disabledIndicatorColor = Color.Transparent,
                ),
            )
            TrustStrip(trust)
        }

        Box(modifier = Modifier.weight(1f)) {
            if (state.conversations.isEmpty()) {
                // Bounded scroll: the hero stays centered when it fits, and the
                // prompts remain reachable under the keyboard / large text.
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(horizontal = 32.dp),
                    verticalArrangement = Arrangement.Center,
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    item(key = "empty_hero") {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            MangoMark(64.dp)
                            Spacer(modifier = Modifier.height(16.dp))
                            Text(
                                "No chats yet",
                                style = MaterialTheme.typography.titleLarge,
                            )
                            Spacer(modifier = Modifier.height(8.dp))
                            Text(
                                "Ask privately below, or start with a prompt.",
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                textAlign = TextAlign.Center,
                            )
                            Spacer(modifier = Modifier.height(16.dp))
                            StarterPromptList(onPrompt = { prompt -> askText = prompt })
                        }
                    }
                }
            } else {
                LazyColumn(modifier = Modifier.fillMaxSize()) {
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
                            SectionLabel(
                                group.label,
                                modifier = Modifier.padding(
                                    horizontal = 20.dp,
                                    vertical = 8.dp,
                                ),
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

        AskBar(
            text = askText,
            onTextChange = { askText = it },
            onSubmit = {
                val trimmed = askText.trim()
                if (trimmed.isEmpty()) onNew() else onAsk(trimmed)
                askText = ""
            },
        )
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

/** Bottom "Ask privately…" bar: a new conversation is one send away. */
@Composable
private fun AskBar(
    text: String,
    onTextChange: (String) -> Unit,
    onSubmit: () -> Unit,
) {
    val border = if (MangoTheme.colors.isDark) {
        BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
    } else null
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .imePadding()
            .navigationBarsPadding()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        shape = CircleShape,
        color = MangoTheme.colors.card,
        border = border,
    ) {
        Row(
            modifier = Modifier.padding(start = 16.dp, end = 6.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Default.Lock,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
            TextField(
                value = text,
                onValueChange = onTextChange,
                modifier = Modifier.weight(1f),
                placeholder = { Text("Ask privately…") },
                singleLine = true,
                colors = TextFieldDefaults.colors(
                    focusedContainerColor = Color.Transparent,
                    unfocusedContainerColor = Color.Transparent,
                    disabledContainerColor = Color.Transparent,
                    focusedIndicatorColor = Color.Transparent,
                    unfocusedIndicatorColor = Color.Transparent,
                    disabledIndicatorColor = Color.Transparent,
                ),
            )
            GradientIconButton(
                icon = Icons.Default.ArrowUpward,
                // Empty send acts as the new-chat affordance; keeps the old
                // FAB's contentDescription so desc selectors still resolve.
                contentDescription = if (text.isBlank()) "New Conversation" else "Send message",
                onClick = onSubmit,
            )
        }
    }
}

private enum class TrustTone { OK, WARN, BAD }

private data class TrustInfo(val tone: TrustTone, val text: String)

/** Honest one-line trust state for the default chat route. */
private fun trustStripInfo(state: AppState): TrustInfo {
    val activeId = state.activeBackendId
    val remoteBackendId = activeId
        ?.takeIf { it.startsWith("hybrid:") }
        ?.removePrefix("hybrid:")
        ?.let { profileId ->
            state.hybridProfiles.firstOrNull { it.id == profileId }?.remoteBackendId
        }
        ?: activeId
    val backend = state.backends.firstOrNull { it.id == remoteBackendId }
    if (backend != null && (backend.id.startsWith("local-") || backend.id == "qvac-local")) {
        return TrustInfo(TrustTone.OK, "Private by default · runs on this phone")
    }
    val name = backend?.name ?: remoteBackendId ?: "backend"
    return when (val status = state.attestationStatuses
        .firstOrNull { it.backendId == remoteBackendId }?.status) {
        is AttestationStatus.Verified ->
            TrustInfo(TrustTone.OK, "Private by default · $name verified")
        is AttestationStatus.Expired ->
            TrustInfo(TrustTone.WARN, "Private by default · $name attestation expired")
        is AttestationStatus.Failed ->
            TrustInfo(TrustTone.BAD, "Private by default · $name attestation failed")
        else ->
            TrustInfo(TrustTone.WARN, "Private by default · $name attestation not verified")
    }
}

@Composable
private fun TrustStrip(trust: TrustInfo) {
    val c = MangoTheme.colors
    val (ring, dot) = when (trust.tone) {
        TrustTone.OK -> c.okContainer to c.ok
        TrustTone.WARN -> c.warnContainer to c.warn
        TrustTone.BAD -> MaterialTheme.colorScheme.errorContainer to MaterialTheme.colorScheme.error
    }
    val icon: ImageVector = when (trust.tone) {
        TrustTone.OK -> Icons.Outlined.Shield
        TrustTone.WARN -> Icons.Outlined.Schedule
        TrustTone.BAD -> Icons.Outlined.Warning
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .size(18.dp)
                .background(ring, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                icon,
                contentDescription = null,
                tint = dot,
                modifier = Modifier.size(11.dp),
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            trust.text,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
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
        Box {
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
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalAlignment = Alignment.Top,
            ) {
                RouteAvatar(
                    kind = when (conversation.route) {
                        InferenceRoute.ON_DEVICE -> RouteKind.LOCAL
                        InferenceRoute.HYBRID -> RouteKind.HYBRID
                        InferenceRoute.CLOUD -> RouteKind.TEE
                    },
                )
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(horizontal = 12.dp),
                ) {
                    Text(
                        conversation.title,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        conversation.lastMessagePreview ?: shortModelName(conversation.modelId),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                Text(
                    relativeTime(conversation.updatedAt),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
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
