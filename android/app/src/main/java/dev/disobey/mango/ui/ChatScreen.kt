package dev.disobey.mango.ui

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.layout.Box
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.AttestationStatusEntry
import dev.disobey.mango.rust.BusyState
import dev.disobey.mango.rust.DocumentSummary
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.UiMessage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/// Full chat screen: message thread + compose bar + top bar with model picker, attestation badge, Instructions.
/// Per CHAT-01 through CHAT-14 and UI-SPEC interaction contract.
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ChatScreen(
    state: AppState,
    onSend: (String) -> Unit,
    onStop: () -> Unit,
    onRetry: () -> Unit,
    onEdit: (String, String) -> Unit,
    onCopy: (String) -> Unit,
    onAttach: (String, String, ULong) -> Unit,
    onClearAttachment: () -> Unit,
    onSelectModel: (String) -> Unit,
    onSetSystemPrompt: (String?) -> Unit,
    onBack: () -> Unit,
    // Phase 8: per-conversation document attachment (D-08)
    onAttachDocument: (String) -> Unit = {},
    onDetachDocument: (String) -> Unit = {},
) {
    val listState = rememberLazyListState()
    val isStreaming = state.busyState is BusyState.Streaming
    var showSystemPromptSheet by remember { mutableStateOf(false) }
    var showDocAttachSheet by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    // The LazyColumn uses reverseLayout = true, so item 0 is at the bottom.
    // "Scroll to bottom" = scrollToItem(0). All effects use this.

    // New content (messages, streaming, thinking, errors): stay pinned to bottom.
    LaunchedEffect(state.messages.size, state.streamingText, state.busyState, state.lastError) {
        listState.scrollToItem(0)
    }

    // Keyboard opens: keep bottom visible.
    val imeVisible = WindowInsets.isImeVisible
    LaunchedEffect(imeVisible) {
        if (imeVisible) listState.scrollToItem(0)
    }

    // File picker launcher
    val fileLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent(),
    ) { uri: Uri? ->
        uri?.let {
            scope.launch(Dispatchers.IO) {
                try {
                    val content = context.contentResolver.openInputStream(it)
                        ?.bufferedReader()?.readText() ?: return@launch
                    val filename = it.lastPathSegment ?: "attachment"
                    val sizeBytes = content.length.toLong()
                    withContext(Dispatchers.Main) {
                        onAttach(filename, content, sizeBytes.toULong())
                    }
                } catch (_: Exception) {
                    // Non-readable file -- ignore
                }
            }
        }
    }

    Scaffold(
        topBar = {
            ChatTopBar(
                state = state,
                onBack = onBack,
                onSelectModel = onSelectModel,
                onShowSystemPrompt = { showSystemPromptSheet = true },
                onShowDocAttach = { showDocAttachSheet = true },
            )
        },
        bottomBar = {
            ComposeBar(
                pendingAttachment = state.pendingAttachment,
                isStreaming = isStreaming,
                onSend = onSend,
                onStop = onStop,
                onAttach = { fileLauncher.launch("*/*") },
                onClearAttachment = onClearAttachment,
            )
        },
        modifier = Modifier.imePadding(),
    ) { innerPadding ->
        // D-17: welcome placeholder when showFirstChatPlaceholder is true and messages empty
        if (state.showFirstChatPlaceholder && state.messages.isEmpty()) {
            androidx.compose.foundation.layout.Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = "You're all set! Send your first message to start a confidential conversation.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    modifier = Modifier.padding(horizontal = 32.dp)
                )
            }
        } else {
        // reverseLayout = true: item 0 renders at the bottom, older messages scroll up.
        // This eliminates all "scroll to bottom on load" timing problems — the list
        // naturally starts at the bottom with no scrolling required.
        LazyColumn(
            state = listState,
            reverseLayout = true,
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // Dynamic items at index 0 (bottom). Order here = bottom-to-top visually.

            // Error bubble (bottommost)
            state.lastError?.let { error ->
                item(key = "error") {
                    ErrorBubble(error = error, onRetry = onRetry)
                }
            }

            // Streaming message
            state.streamingText?.let { text ->
                if (text.isNotEmpty()) {
                    item(key = "streaming") {
                        StreamingMessageBubble(text = text)
                    }
                }
            }

            // Thinking indicator
            val isThinking = (state.busyState is BusyState.Streaming || state.busyState is BusyState.Loading)
                && state.streamingText.isNullOrEmpty()
            if (isThinking) {
                item(key = "thinking") {
                    ThinkingIndicatorBubble()
                }
            }

            // Messages newest-first (reversed) so the most recent sits just above the dynamic items.
            items(state.messages.reversed(), key = { it.id }) { message ->
                MessageBubble(
                    message = message,
                    isLastAssistant = isLastAssistantMessage(state.messages, message),
                    isStreaming = false,
                    onCopy = { onCopy(message.content) },
                    onRetry = onRetry,
                    onEdit = { onEdit(message.id, message.content) },
                )
            }
        }
        } // end else (not showFirstChatPlaceholder)
    }

    // System prompt bottom sheet (per CHAT-11 / D-09)
    if (showSystemPromptSheet) {
        SystemPromptSheet(
            initialPrompt = "",
            onSave = { prompt ->
                onSetSystemPrompt(if (prompt.isBlank()) null else prompt)
                showSystemPromptSheet = false
            },
            onDismiss = { showSystemPromptSheet = false },
        )
    }

    // Document attachment bottom sheet (D-08)
    if (showDocAttachSheet) {
        DocAttachSheet(
            documents = state.documents,
            attachedDocIds = state.currentConversationAttachedDocs,
            onToggle = { docId ->
                if (state.currentConversationAttachedDocs.contains(docId)) {
                    onDetachDocument(docId)
                } else {
                    onAttachDocument(docId)
                }
                showDocAttachSheet = false
            },
            onDismiss = { showDocAttachSheet = false },
        )
    }
}

// MARK: - Top Bar

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ChatTopBar(
    state: AppState,
    onBack: () -> Unit,
    onSelectModel: (String) -> Unit,
    onShowSystemPrompt: () -> Unit,
    onShowDocAttach: () -> Unit = {},
) {
    val currentConversation = state.currentConversationId?.let { id ->
        state.conversations.firstOrNull { it.id == id }
    }
    val selectedModelId = currentConversation?.modelId
    // Aggregate models from ALL healthy (or degraded) backends so the picker
    // shows every TEE-capable model across providers, not just the active one.
    val availableModelEntries: List<Pair<String, String>> = state.backends
        .filter { it.healthStatus != HealthStatus.FAILED && it.models.isNotEmpty() }
        .flatMap { backend -> backend.models.map { modelId -> Pair(modelId, backend.name) } }
    var showModelMenu by remember { mutableStateOf(false) }
    val activeAttestation = state.activeBackendId?.let { backendId ->
        state.attestationStatuses.firstOrNull { it.backendId == backendId }?.status
    }

    TopAppBar(
        title = {
            Text(
                text = currentConversation?.title ?: "New Conversation",
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
            )
        },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
            }
        },
        actions = {
            // Model picker
            Box {
                TextButton(onClick = { showModelMenu = true }) {
                    Text(
                        text = selectedModelId?.let { shortModelName(it) } ?: "Model",
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
                DropdownMenu(
                    expanded = showModelMenu,
                    onDismissRequest = { showModelMenu = false },
                ) {
                    availableModelEntries.forEach { (modelId, backendName) ->
                        DropdownMenuItem(
                            text = {
                                Column {
                                    Text(
                                        text = shortModelName(modelId),
                                        fontWeight = if (modelId == selectedModelId)
                                            FontWeight.Bold else FontWeight.Normal,
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                    Text(
                                        text = backendName,
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            },
                            leadingIcon = if (modelId == selectedModelId) {
                                { Icon(Icons.Default.Check, contentDescription = "Selected") }
                            } else null,
                            onClick = {
                                onSelectModel(modelId)
                                showModelMenu = false
                            },
                        )
                    }
                }
            }
            // Attestation badge
            activeAttestation?.let { status ->
                AttestationBadge(status = status)
            }
            // Per-conversation document attachment (D-08)
            val attachedCount = state.currentConversationAttachedDocs.size
            TextButton(onClick = onShowDocAttach) {
                Text(
                    if (attachedCount > 0) "RAG ($attachedCount)" else "RAG",
                    style = MaterialTheme.typography.labelMedium,
                )
            }
            // Instructions button (system prompt per CHAT-11)
            TextButton(onClick = onShowSystemPrompt) {
                Text(
                    "Instructions",
                    style = MaterialTheme.typography.labelMedium,
                )
            }
        },
    )
}

// MARK: - Helper

private fun isLastAssistantMessage(messages: List<UiMessage>, message: UiMessage): Boolean {
    if (message.role != "assistant") return false
    return messages.lastOrNull { it.role == "assistant" }?.id == message.id
}

// MARK: - Document Attachment Sheet

/// ModalBottomSheet for toggling document attachment to the current conversation (D-08).
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DocAttachSheet(
    documents: List<DocumentSummary>,
    attachedDocIds: List<String>,
    onToggle: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Text(
            text = "Attach RAG Documents",
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)
        )
        HorizontalDivider()
        if (documents.isEmpty()) {
            Text(
                text = "No documents in library",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(16.dp)
            )
        } else {
            LazyColumn {
                items(documents, key = { it.id }) { doc ->
                    val isAttached = attachedDocIds.contains(doc.id)
                    ListItem(
                        headlineContent = {
                            Text(
                                text = doc.name,
                                style = MaterialTheme.typography.bodyMedium
                            )
                        },
                        supportingContent = {
                            Text(
                                text = doc.format.uppercase(),
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        },
                        leadingContent = {
                            Checkbox(
                                checked = isAttached,
                                onCheckedChange = { onToggle(doc.id) }
                            )
                        },
                        modifier = Modifier.padding(vertical = 2.dp)
                    )
                }
            }
        }
        Spacer(Modifier.height(16.dp))
    }
}
