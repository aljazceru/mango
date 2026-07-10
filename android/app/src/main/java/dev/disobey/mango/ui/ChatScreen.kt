package dev.disobey.mango.ui

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import java.io.File
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
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
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
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
import androidx.compose.foundation.layout.Row
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.BackendRole
import dev.disobey.mango.rust.DiscoverableTool
import dev.disobey.mango.rust.HybridProfile
import dev.disobey.mango.rust.Screen
import dev.disobey.mango.rust.AttestationStatusEntry
import dev.disobey.mango.rust.BusyState
import dev.disobey.mango.rust.DocumentSummary
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.UiMessage
import dev.disobey.mango.rust.modelSupportsVision
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/// Full chat screen: message thread + compose bar + top bar with model picker, attestation badge, Instructions.
/// Per CHAT-01 through CHAT-14 and UI-SPEC interaction contract.
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ChatScreen(
    state: AppState,
    onSend: (String, BackendRole?) -> Unit,
    onStop: () -> Unit,
    onRetry: () -> Unit,
    onEdit: (String, String) -> Unit,
    onCopy: (String) -> Unit,
    onAttach: (String, String, ULong) -> Unit,
    onAttachImage: (String, String, String) -> Unit,
    onClearAttachment: () -> Unit,
    onSelectModel: (String) -> Unit,
    onSetSystemPrompt: (String?) -> Unit,
    onBack: () -> Unit,
    // Phase 8: per-conversation document attachment (D-08)
    onAttachDocument: (String) -> Unit = {},
    onDetachDocument: (String) -> Unit = {},
    onShareConversation: (String) -> Unit = {},
    // Phase 27: tools toggle (CHAT-TOOL-07)
    onDispatchAction: (AppAction) -> Unit = {},
    // IMG-07: decrypt-on-read for encrypted image thumbnails
    onReadEncryptedImage: ((String) -> ByteArray)? = null,
    fontScale: Float = 1f,
) {
    val listState = rememberLazyListState()
    var wasAtBottomBeforeUpdate by remember { mutableStateOf(true) }
    var userRequestedBottom by remember { mutableStateOf(false) }
    var lastConversationId by remember { mutableStateOf(state.currentConversationId) }
    val isAtBottom by remember {
        derivedStateOf {
            isChatListAtBottom(
                firstVisibleItemIndex = listState.firstVisibleItemIndex,
                firstVisibleItemScrollOffset = listState.firstVisibleItemScrollOffset,
            )
        }
    }
    val showScrollToBottom by remember { derivedStateOf { !isAtBottom } }
    val isStreaming = state.busyState is BusyState.Streaming
    val isChatBusy = isStreaming || state.busyState is BusyState.Loading
    val isEmptyIdleChat = state.messages.isEmpty() &&
        state.streamingText.isNullOrEmpty() &&
        state.busyState is BusyState.Idle
    val loadingMessage = (state.busyState as? BusyState.Loading)?.message
    val isAttestationLoading =
        loadingMessage?.contains("attestation", ignoreCase = true) == true
    var showSystemPromptSheet by remember { mutableStateOf(false) }
    var showDocAttachSheet by remember { mutableStateOf(false) }
    var composerPrefill by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    var wasResponseActive by remember { mutableStateOf(false) }
    val currentConversation = state.currentConversationId?.let { id ->
        state.conversations.firstOrNull { it.id == id }
    }
    val activeHybridProfile = currentConversation?.backendId
        ?.takeIf { it.startsWith("hybrid:") }
        ?.removePrefix("hybrid:")
        ?.let { profileId -> state.hybridProfiles.firstOrNull { it.id == profileId } }
    var forceRemoteNext by remember(currentConversation?.id, activeHybridProfile?.id) {
        mutableStateOf(false)
    }
    val routeChip = hybridRouteChip(state, activeHybridProfile, forceRemoteNext)

    LaunchedEffect(isChatBusy) {
        if (wasResponseActive && !isChatBusy) {
            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
        }
        wasResponseActive = isChatBusy
    }

    // The LazyColumn uses reverseLayout = true, so item 0 is at the bottom.
    // "Scroll to bottom" = scrollToItem(0). All effects use this.

    LaunchedEffect(isAtBottom) {
        if (isAtBottom) {
            wasAtBottomBeforeUpdate = true
            userRequestedBottom = false
        } else {
            wasAtBottomBeforeUpdate = false
        }
    }

    // New content: stay pinned only when the user was already at bottom, when a
    // different conversation loads, or after the user taps the bottom affordance.
    LaunchedEffect(state.messages.size, state.streamingText, state.busyState, state.lastError) {
        val isNewConversation = lastConversationId != state.currentConversationId
        val shouldPin = shouldAutoPinChat(
            wasAtBottomBeforeUpdate = wasAtBottomBeforeUpdate,
            isNewConversation = isNewConversation,
            userRequestedBottom = userRequestedBottom,
        )
        lastConversationId = state.currentConversationId
        if (shouldPin) {
            listState.scrollToItem(0)
            wasAtBottomBeforeUpdate = true
            userRequestedBottom = false
        } else {
            wasAtBottomBeforeUpdate = false
        }
    }

    // Keyboard opens: keep bottom visible.
    val imeVisible = WindowInsets.isImeVisible
    LaunchedEffect(imeVisible) {
        if (imeVisible && isAtBottom) listState.scrollToItem(0)
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
                    withContext(Dispatchers.Main) {
                        onDispatchAction(AppAction.ShowToast(message = "Could not read attachment"))
                    }
                }
            }
        }
    }

    // Phase 31 (IMG-05/06): camera + gallery action sheet state and launchers.
    // Paperclip opens a bottom sheet with Take Photo / Choose Photo / Attach File (D-6).
    var showAttachSheet by remember { mutableStateOf(false) }
    var pendingCameraFile by remember { mutableStateOf<File?>(null) }

    val galleryLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.PickVisualMedia()
    ) { uri: Uri? ->
        if (uri != null) {
            scope.launch(Dispatchers.IO) {
                try {
                    val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
                        ?: return@launch
                    val mime = context.contentResolver.getType(uri) ?: "image/jpeg"
                    val normalizedMime = if (mime == "image/png") "image/png" else "image/jpeg"
                    val ext = if (normalizedMime == "image/png") "png" else "jpg"
                    val tmp = File(context.cacheDir, "img_${System.currentTimeMillis()}.$ext")
                    tmp.writeBytes(bytes)
                    val name = uri.lastPathSegment?.substringAfterLast('/') ?: "image.$ext"
                    withContext(Dispatchers.Main) {
                        onAttachImage(name, tmp.absolutePath, normalizedMime)
                    }
                } catch (_: Exception) {
                    withContext(Dispatchers.Main) {
                        onDispatchAction(AppAction.ShowToast(message = "Could not read image"))
                    }
                }
            }
        }
    }

    val cameraLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.TakePicture()
    ) { success: Boolean ->
        if (success) {
            pendingCameraFile?.let { file ->
                onAttachImage(file.name, file.absolutePath, "image/jpeg")
            }
        } else {
            pendingCameraFile?.delete()
            onDispatchAction(AppAction.ShowToast(message = "Photo was not attached"))
        }
        pendingCameraFile = null
    }

    fun launchCameraInternal() {
        val file = File(context.cacheDir, "camera_${System.currentTimeMillis()}.jpg")
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        pendingCameraFile = file
        cameraLauncher.launch(uri)
    }

    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted: Boolean ->
        if (granted) {
            launchCameraInternal()
        } else {
            onDispatchAction(AppAction.ShowToast(message = "Camera permission denied"))
        }
    }

    fun launchCamera() {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA)
            == PackageManager.PERMISSION_GRANTED
        ) {
            launchCameraInternal()
        } else {
            cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
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
                onShareConversation = onShareConversation,
                onDispatchAction = onDispatchAction,
                forceRemoteNext = forceRemoteNext,
                onToggleForceRemoteNext = { forceRemoteNext = !forceRemoteNext },
            )
        },
        bottomBar = {
            ComposeBar(
                pendingAttachment = state.pendingAttachment,
                isStreaming = isStreaming,
                isInputBlocked = isChatBusy,
                showStopButton = isStreaming || isAttestationLoading,
                onSend = { text ->
                    onSend(text, if (forceRemoteNext) BackendRole.REMOTE else null)
                    forceRemoteNext = false
                },
                onStop = onStop,
                onAttach = { showAttachSheet = true },
                onClearAttachment = onClearAttachment,
                routingLabel = routeChip?.label,
                routingDetail = routeChip?.detail,
                prefillText = composerPrefill,
                onPrefillConsumed = { composerPrefill = null },
            )
        },
    ) { innerPadding ->
        // D-17: welcome placeholder when the current chat is empty and idle.
        if (isEmptyIdleChat) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = "You're all set! Send your first message to start a confidential conversation.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    modifier = Modifier.padding(horizontal = 32.dp)
                )
                Spacer(Modifier.height(16.dp))
                StarterPromptList(onPrompt = { prompt -> composerPrefill = prompt })
            }
        } else {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
            ) {
                // reverseLayout = true: item 0 renders at the bottom, older messages scroll up.
                LazyColumn(
                    state = listState,
                    reverseLayout = true,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    // Dynamic items at index 0 (bottom). Order here = bottom-to-top visually.

                    // Error bubble (bottommost)
                    state.lastError?.takeUnless { isEmptyIdleChat }?.let { error ->
                        item(key = "error") {
                            ErrorBubble(error = error, onRetry = onRetry)
                        }
                    }

                    // Streaming message
                    state.streamingText?.let { text ->
                        if (text.isNotEmpty()) {
                            item(key = "streaming") {
                                StreamingMessageBubble(text = text, fontScale = fontScale)
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
                            onReadEncryptedImage = onReadEncryptedImage,
                            fontScale = fontScale,
                        )
                    }
                }
                if (showScrollToBottom) {
                    IconButton(
                        onClick = {
                            userRequestedBottom = true
                            scope.launch { listState.animateScrollToItem(0) }
                        },
                        modifier = Modifier
                            .align(Alignment.BottomCenter)
                            .padding(bottom = 12.dp)
                            .wrapContentSize(),
                    ) {
                        Icon(
                            Icons.Default.KeyboardArrowDown,
                            contentDescription = "Scroll to latest message",
                        )
                    }
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

    // Phase 31 (IMG-05/06, D-6): paperclip action sheet — Take Photo / Choose Photo / Attach File.
    // Vision capability gating (follow-up to image-upload-still-broken-after-fix):
    // image entries are hidden when the selected model is not vision-capable so
    // the user never gets into the silent-failure path of sending a photo to a
    // text-only model.
    val currentModelId = activeHybridProfile?.remoteModelId
        ?: state.currentConversationId
            ?.let { id -> state.conversations.firstOrNull { it.id == id }?.modelId }
            .orEmpty()
    val showImageOptions = currentModelId.isNotEmpty() && modelSupportsVision(currentModelId)
    if (showAttachSheet) {
        val attachSheetState = rememberModalBottomSheetState()
        ModalBottomSheet(
            onDismissRequest = { showAttachSheet = false },
            sheetState = attachSheetState,
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 8.dp),
            ) {
                if (showImageOptions) {
                    TextButton(
                        onClick = {
                            showAttachSheet = false
                            launchCamera()
                        },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text("Take Photo", style = MaterialTheme.typography.bodyLarge) }
                    TextButton(
                        onClick = {
                            showAttachSheet = false
                            galleryLauncher.launch(
                                PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)
                            )
                        },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text("Choose Photo", style = MaterialTheme.typography.bodyLarge) }
                }
                TextButton(
                    onClick = {
                        showAttachSheet = false
                        fileLauncher.launch("*/*")
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) { Text("Attach File", style = MaterialTheme.typography.bodyLarge) }
                Spacer(Modifier.height(8.dp))
            }
        }
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
    onShareConversation: (String) -> Unit = {},
    onDispatchAction: (AppAction) -> Unit = {},
    forceRemoteNext: Boolean = false,
    onToggleForceRemoteNext: () -> Unit = {},
) {
    val currentConversation = state.currentConversationId?.let { id ->
        state.conversations.firstOrNull { it.id == id }
    }
    val selectedModelId = currentConversation?.modelId
    val activeHybridProfile = currentConversation?.backendId
        ?.takeIf { it.startsWith("hybrid:") }
        ?.removePrefix("hybrid:")
        ?.let { profileId -> state.hybridProfiles.firstOrNull { it.id == profileId } }
    // Aggregate models from ALL healthy (or degraded) backends so the picker
    // shows every TEE-capable model across providers, not just the active one.
    val availableModelEntries: List<Pair<String, String>> = state.backends
        .filter { it.healthStatus != HealthStatus.FAILED && it.models.isNotEmpty() }
        .flatMap { backend -> backend.models.map { modelId -> Pair(modelId, backend.name) } }
    var showModelMenu by remember { mutableStateOf(false) }
    val attestationBackendId = activeHybridProfile?.remoteBackendId ?: state.activeBackendId
    val activeAttestation = attestationBackendId?.let { backendId ->
        state.attestationStatuses.firstOrNull { it.backendId == backendId }?.status
    }
    val hasLocalBackend = state.backends.any { it.id.startsWith("local-") && it.models.isNotEmpty() }

    var showConvMenu by remember { mutableStateOf(false) }
    var showToolsSheet by remember { mutableStateOf(false) }
    var showRenameDialog by remember { mutableStateOf(false) }
    var renameText by remember { mutableStateOf("") }
    val toolsEnabled = state.conversations
        .firstOrNull { it.id == state.currentConversationId }
        ?.toolsEnabled ?: false
    val attachedCount = state.currentConversationAttachedDocs.size

    TopAppBar(
        title = {
            Text(
                text = currentConversation?.title ?: "New Conversation",
                style = MaterialTheme.typography.titleMedium,
                maxLines = 1,
                modifier = if (currentConversation != null) {
                    Modifier.clickable {
                        renameText = currentConversation.title
                        showRenameDialog = true
                    }
                } else {
                    Modifier
                },
            )
        },
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
            }
        },
        actions = {
            activeAttestation?.let { status ->
                AttestationBadge(
                    status = status,
                    modifier = Modifier.padding(end = 4.dp),
                )
            }
            // Model picker
            Box {
                TextButton(onClick = { showModelMenu = true }) {
                    Text(
                        text = activeHybridProfile?.name
                            ?: selectedModelId?.let { shortModelName(it) }
                            ?: "Model",
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
                    if (hasLocalBackend && state.hybridProfiles.isNotEmpty()) {
                        HorizontalDivider()
                        state.hybridProfiles.forEach { profile ->
                            val selected = activeHybridProfile?.id == profile.id
                            DropdownMenuItem(
                                text = {
                                    Column {
                                        Text(
                                            text = profile.name,
                                            fontWeight = if (selected)
                                                FontWeight.Bold else FontWeight.Normal,
                                            style = MaterialTheme.typography.bodyMedium,
                                        )
                                        Text(
                                            text = "${shortModelName(profile.localModelId)} -> ${shortModelName(profile.remoteModelId)}",
                                            style = MaterialTheme.typography.labelSmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        )
                                    }
                                },
                                leadingIcon = if (selected) {
                                    { Icon(Icons.Default.Check, contentDescription = "Selected") }
                                } else null,
                                onClick = {
                                    currentConversation?.let { conversation ->
                                        onDispatchAction(
                                            AppAction.OverrideConversationBackend(
                                                conversationId = conversation.id,
                                                backendId = "hybrid:${profile.id}",
                                            ),
                                        )
                                    }
                                    onDispatchAction(
                                        AppAction.SetActiveHybridProfile(profileId = profile.id),
                                    )
                                    showModelMenu = false
                                },
                            )
                        }
                    }
                }
            }
            // Collapsed "..." menu: Documents, Instructions, Tools
            Box {
                IconButton(onClick = { showConvMenu = true }) {
                    Icon(
                        Icons.Default.MoreVert,
                        contentDescription = "Conversation options",
                    )
                }
                DropdownMenu(
                    expanded = showConvMenu,
                    onDismissRequest = { showConvMenu = false },
                ) {
                    DropdownMenuItem(
                        text = {
                            Text(
                                if (attachedCount > 0) "Documents ($attachedCount)" else "Documents",
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        },
                        onClick = {
                            showConvMenu = false
                            onShowDocAttach()
                        },
                    )
                    DropdownMenuItem(
                        text = {
                            Text(
                                "Instructions",
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        },
                        onClick = {
                            showConvMenu = false
                            onShowSystemPrompt()
                        },
                    )
                    DropdownMenuItem(
                        text = {
                            Text(
                                if (toolsEnabled) "Tools: On" else "Tools",
                                style = MaterialTheme.typography.bodyMedium,
                                color = if (toolsEnabled)
                                    MaterialTheme.colorScheme.primary
                                else
                                    MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        },
                        onClick = {
                            showConvMenu = false
                            showToolsSheet = true
                        },
                    )
                    DropdownMenuItem(
                        text = {
                            Text(
                                "Share conversation",
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        },
                        leadingIcon = {
                            Icon(Icons.Default.Share, contentDescription = null)
                        },
                        enabled = state.currentConversationId != null,
                        onClick = {
                            showConvMenu = false
                            state.currentConversationId?.let(onShareConversation)
                        },
                    )
                    if (activeHybridProfile != null) {
                        DropdownMenuItem(
                            text = {
                                Text(
                                    if (forceRemoteNext) "Remote next: On" else "Use remote this turn",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = if (forceRemoteNext)
                                        MaterialTheme.colorScheme.primary
                                    else
                                        MaterialTheme.colorScheme.onSurface,
                                )
                            },
                            onClick = {
                                showConvMenu = false
                                onToggleForceRemoteNext()
                            },
                        )
                    }
                    // Fork chat: duplicate the current conversation into a new
                    // independent one. Only enabled when a conversation is
                    // active AND has at least one message (mirrors Desktop).
                    val canFork = state.currentConversationId != null &&
                        state.messages.isNotEmpty()
                    DropdownMenuItem(
                        text = {
                            Text(
                                "Fork chat",
                                style = MaterialTheme.typography.bodyMedium,
                                color = if (canFork)
                                    MaterialTheme.colorScheme.onSurface
                                else
                                    MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        },
                        enabled = canFork,
                        onClick = {
                            showConvMenu = false
                            state.currentConversationId?.let { cid ->
                                onDispatchAction(AppAction.ForkConversation(id = cid))
                            }
                        },
                    )
                }
            }
        },
    )

    // Rename dialog: tap-on-title affordance opens this.
    if (showRenameDialog && currentConversation != null) {
        AlertDialog(
            onDismissRequest = { showRenameDialog = false },
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
                        if (trimmed.isNotEmpty()) {
                            onDispatchAction(
                                AppAction.RenameConversation(
                                    id = currentConversation.id,
                                    title = trimmed,
                                )
                            )
                        }
                        showRenameDialog = false
                    },
                ) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { showRenameDialog = false }) { Text("Cancel") }
            },
        )
    }

    // Tools bottom sheet: individual tool toggles
    if (showToolsSheet) {
        val convId = state.currentConversationId
        ToolsSheet(
            toolsEnabled = toolsEnabled,
            braveApiKeySet = state.braveApiKeySet,
            contextvmTools = state.contextvmTools,
            onSetToolsEnabled = { enabled ->
                if (convId != null) {
                    onDispatchAction(
                        AppAction.SetConversationToolsEnabled(
                            conversationId = convId,
                            enabled = enabled,
                        )
                    )
                }
            },
            onToggleContextvmTool = { tool, enabled ->
                onDispatchAction(
                    AppAction.SetContextvmToolEnabled(
                        toolId = tool.id,
                        enabled = enabled,
                    )
                )
            },
            onOpenToolSettings = {
                showToolsSheet = false
                onDispatchAction(AppAction.PushScreen(screen = Screen.SettingsTools))
            },
            onDismiss = { showToolsSheet = false },
        )
    }
}

// MARK: - Helper

private fun isLastAssistantMessage(messages: List<UiMessage>, message: UiMessage): Boolean {
    if (message.role != "assistant") return false
    return messages.lastOrNull { it.role == "assistant" }?.id == message.id
}

private data class RouteChip(val label: String, val detail: String)

private fun hybridRouteChip(
    state: AppState,
    profile: HybridProfile?,
    forceRemoteNext: Boolean,
): RouteChip? {
    if (profile == null) return null
    if (forceRemoteNext) {
        return RouteChip(
            label = "Remote next turn · ${shortModelName(profile.remoteModelId)}",
            detail = "Routing reason: user override",
        )
    }

    val lastRoute = state.lastTurnRouting
    if (
        lastRoute?.profileId == profile.id &&
            lastRoute.conversationId == state.currentConversationId
    ) {
        val label = when (lastRoute.decision) {
            BackendRole.LOCAL -> "Answered locally · on-device"
            BackendRole.REMOTE -> {
                if (lastRoute.teeVerified) {
                    "Escalated to ${lastRoute.providerName} · ${lastRoute.teeLabel} verified"
                } else {
                    "Escalated to ${lastRoute.providerName} · verifying"
                }
            }
        }
        return RouteChip(
            label = label,
            detail = "Routing reason: ${lastRoute.reason}",
        )
    }

    return RouteChip(
        label = "Hybrid ready · local by default",
        detail = "Routing reason: local default",
    )
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
            text = "Attach Documents",
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

// MARK: - Tools Sheet

/// ModalBottomSheet for configuring per-conversation tool toggles (Phase 27, CHAT-TOOL-07).
/// Shows individual tool toggles so more tools can be added without layout changes.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ToolsSheet(
    toolsEnabled: Boolean,
    braveApiKeySet: Boolean,
    contextvmTools: List<DiscoverableTool>,
    onSetToolsEnabled: (Boolean) -> Unit,
    onToggleContextvmTool: (DiscoverableTool, Boolean) -> Unit,
    onOpenToolSettings: () -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState()
    val enabledContextvmTools = contextvmTools.filter { it.enabled }
    val hasAnyTool = braveApiKeySet || enabledContextvmTools.isNotEmpty()

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "Tools",
                style = MaterialTheme.typography.titleMedium,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onOpenToolSettings) {
                Text("Configure")
            }
        }
        HorizontalDivider()

        if (!hasAnyTool) {
            ListItem(
                headlineContent = {
                    Text(
                        text = "No tools configured",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                supportingContent = {
                    Text(
                        text = "Add a Brave Search key or enable discovered tools to get started.",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
            )
        } else {
            // Master toggle — controls whether tools run in this conversation.
            ListItem(
                headlineContent = {
                    Text(
                        text = "Use tools in this conversation",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                },
                trailingContent = {
                    androidx.compose.material3.Switch(
                        checked = toolsEnabled,
                        onCheckedChange = { onSetToolsEnabled(it) },
                    )
                },
            )
            HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

            // Brave Search row — only shown when key is configured.
            if (braveApiKeySet) {
                ListItem(
                    headlineContent = {
                        Text(
                            text = "Brave Search",
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    },
                    supportingContent = {
                        Text(
                            text = "Web search",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    },
                )
            }

            // One row per enabled contextvm tool.
            enabledContextvmTools.forEach { tool ->
                val providerName = tool.providerName
                    ?: tool.providerDisplayName
                    ?: tool.npub.take(12) + "…"
                ListItem(
                    headlineContent = {
                        Text(
                            text = tool.name,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    },
                    supportingContent = {
                        Text(
                            text = providerName,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    },
                )
            }
        }

        Spacer(Modifier.height(16.dp))
    }
}
