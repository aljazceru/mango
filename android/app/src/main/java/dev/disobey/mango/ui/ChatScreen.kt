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
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.outlined.Balance
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Edit
import androidx.compose.material.icons.outlined.FolderOpen
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
import androidx.compose.runtime.key
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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
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
import dev.disobey.mango.rust.BusyState
import dev.disobey.mango.rust.DocumentSummary
import dev.disobey.mango.rust.HealthStatus
import dev.disobey.mango.rust.UiMessage
import dev.disobey.mango.ui.theme.MangoTheme
import dev.disobey.mango.ui.theme.MonoFamily
import dev.disobey.mango.ui.theme.TintPair
import dev.disobey.mango.rust.modelSupportsVision
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/// Matches the Rust AttachImage limit (lib.rs rejects > 50 MB raw input).
private const val MAX_IMAGE_ATTACH_BYTES: Long = 50L * 1024L * 1024L

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
                var staged: File? = null
                try {
                    // Stage to an app cache file with STRICT streaming UTF-8
                    // validation (fresh CharsetDecoder defaults to REPORT, set
                    // again for clarity): binary files abort at the first
                    // malformed byte — that is not a size restriction, it is
                    // correctness for text attachments — and no matter how
                    // large the file is, only an 8 KB buffer lives in Kotlin
                    // memory. The content never crosses the FFI as a string;
                    // there is NO client-side size cap. Only backend limits
                    // apply (the backend's error surfaces if it rejects).
                    val decoder: java.nio.charset.CharsetDecoder =
                        Charsets.UTF_8.newDecoder()
                    decoder.onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
                    staged = File.createTempFile("attach_", ".txt", context.cacheDir)
                    context.contentResolver.openInputStream(it)?.use { input ->
                        java.io.InputStreamReader(input, decoder).use { reader ->
                            staged!!.outputStream().use { raw ->
                                java.io.OutputStreamWriter(raw, Charsets.UTF_8).use { writer ->
                                    val buffer = CharArray(8192)
                                    while (true) {
                                        val read = reader.read(buffer)
                                        if (read < 0) break
                                        writer.write(buffer, 0, read)
                                    }
                                }
                            }
                        }
                    } ?: run {
                        staged!!.delete()
                        return@launch
                    }
                    val filename = it.lastPathSegment ?: "attachment"
                    withContext(Dispatchers.Main) {
                        onDispatchAction(
                            AppAction.AttachFileFromPath(
                                filename = filename,
                                filePath = staged!!.absolutePath,
                            ),
                        )
                    }
                } catch (_: java.nio.charset.CharacterCodingException) {
                    staged?.delete()
                    withContext(Dispatchers.Main) {
                        onDispatchAction(AppAction.ShowToast(
                            message = "Only text files can be attached. Use the document library for documents.",
                        ))
                    }
                } catch (_: Exception) {
                    staged?.delete()
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
                var staged: File? = null
                try {
                    // Stream-capped read: the picker can hand us an image larger
                    // than Rust's 50 MB attach limit — bound allocation and
                    // surface a visible error instead of exhausting the heap.
                    val bytes = context.contentResolver.openInputStream(uri)?.use {
                        readBytesCapped(it, MAX_IMAGE_ATTACH_BYTES)
                    }
                    if (bytes == null) {
                        withContext(Dispatchers.Main) {
                            onDispatchAction(
                                AppAction.ShowToast(
                                    message = "Image exceeds the 50 MB limit or could not be read.",
                                )
                            )
                        }
                        return@launch
                    }
                    val mime = context.contentResolver.getType(uri) ?: "image/jpeg"
                    val normalizedMime = if (mime == "image/png") "image/png" else "image/jpeg"
                    val ext = if (normalizedMime == "image/png") "png" else "jpg"
                    staged = File(context.cacheDir, "img_${System.currentTimeMillis()}.$ext")
                    staged!!.writeBytes(bytes)
                    val name = uri.lastPathSegment?.substringAfterLast('/') ?: "image.$ext"
                    withContext(Dispatchers.Main) {
                        onAttachImage(name, staged!!.absolutePath, normalizedMime)
                    }
                } catch (_: Exception) {
                    staged?.delete()
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
            Column {
                ChatTopBar(
                    state = state,
                    onBack = onBack,
                    onShowSystemPrompt = { showSystemPromptSheet = true },
                    onShowDocAttach = { showDocAttachSheet = true },
                    onShareConversation = onShareConversation,
                    onDispatchAction = onDispatchAction,
                    forceRemoteNext = forceRemoteNext,
                    onToggleForceRemoteNext = { forceRemoteNext = !forceRemoteNext },
                )
                ChatStatusChips(
                    state = state,
                    onSelectModel = onSelectModel,
                    onDispatchAction = onDispatchAction,
                )
            }
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
                routingKind = if (activeHybridProfile != null) RouteKind.HYBRID else null,
                prefillText = composerPrefill,
                onPrefillConsumed = { composerPrefill = null },
            )
        },
    ) { innerPadding ->
        // D-17: empty-chat hero — mark, route card and suggestion grid.
        if (isEmptyIdleChat) {
            EmptyChatHero(
                state = state,
                activeHybridProfile = activeHybridProfile,
                composerPrefill = { prompt -> composerPrefill = prompt },
                onDispatchAction = onDispatchAction,
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding),
            )
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
                            ErrorBubble(
                                error = error,
                                onRetry = onRetry,
                                onCompact = if (state.compactionOffered) {
                                    {
                                        state.currentConversationId?.let { cid ->
                                            onDispatchAction(
                                                AppAction.CompactConversation(
                                                    conversationId = cid,
                                                    retryAfter = true,
                                                ),
                                            )
                                        }
                                    }
                                } else {
                                    null
                                },
                            )
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
                    // Provenance dividers: one centered line above an assistant
                    // message whenever the route changes.
                    val dividerLabels = state.messages.mapIndexed { i, _ ->
                        routeDividerLabel(state.messages, i)
                    }
                    itemsIndexed(state.messages.reversed(), key = { _, m -> m.id }) { revIndex, message ->
                        val index = state.messages.size - 1 - revIndex
                        Column {
                            dividerLabels[index]?.let { label ->
                                RouteDivider(
                                    label = label,
                                    icon = if (message.routeDecision.equals("local", ignoreCase = true))
                                        RouteKind.LOCAL.icon else RouteKind.TEE.icon,
                                )
                            }
                            MessageBubble(
                                message = message,
                                isLastAssistant = isLastAssistantMessage(state.messages, message),
                                onCopy = { onCopy(message.content) },
                                onRetry = onRetry,
                                onEdit = { onEdit(message.id, message.content) },
                                onReadEncryptedImage = onReadEncryptedImage,
                                fontScale = fontScale,
                                showAssistantHeader = isFirstOfRun(state.messages, index),
                            )
                        }
                    }

                    // Compaction badge (topmost item in the reversed list)
                    state.compaction?.let { info ->
                        item(key = "compaction") {
                            Text(
                                text = "Compacted · ${info.coveredMessageCount} of ${info.totalMessageCount} messages summarized",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
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
    // Key the sheet on conversation id so reopening after a switch resets the
    // draft to the *current* conversation's saved system prompt; the sheet
    // re-initializes its text each time it is shown.
    val currentSystemPrompt = currentConversation?.systemPrompt ?: ""
    if (showSystemPromptSheet) {
        key(currentConversation?.id) {
            SystemPromptSheet(
                initialPrompt = currentSystemPrompt,
                onSave = { prompt ->
                    onSetSystemPrompt(systemPromptSaveValue(prompt))
                    showSystemPromptSheet = false
                },
                onDismiss = { showSystemPromptSheet = false },
            )
        }
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
    val activeHybridProfile = currentConversation?.backendId
        ?.takeIf { it.startsWith("hybrid:") }
        ?.removePrefix("hybrid:")
        ?.let { profileId -> state.hybridProfiles.firstOrNull { it.id == profileId } }

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
                overflow = TextOverflow.Ellipsis,
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
                    // Compact chat: summarize older turns for the model; the full
                    // history stays on device. The core guards busy state (toast).
                    val canCompact = state.currentConversationId != null
                    DropdownMenuItem(
                        text = {
                            Text(
                                "Compact chat",
                                style = MaterialTheme.typography.bodyMedium,
                                color = if (canCompact)
                                    MaterialTheme.colorScheme.onSurface
                                else
                                    MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        },
                        enabled = canCompact,
                        onClick = {
                            showConvMenu = false
                            state.currentConversationId?.let { cid ->
                                onDispatchAction(
                                    AppAction.CompactConversation(
                                        conversationId = cid,
                                        retryAfter = false,
                                    ),
                                )
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

// MARK: - Chat Status Chips

/// Chip row under the chat top bar: attestation badge + model picker. It lives
/// below the app bar so the header can grow at large font scales instead of
/// clipping inside the TopAppBar's fixed-height title slot.
@Composable
private fun ChatStatusChips(
    state: AppState,
    onSelectModel: (String) -> Unit,
    onDispatchAction: (AppAction) -> Unit,
) {
    val currentConversation = state.currentConversationId?.let { id ->
        state.conversations.firstOrNull { it.id == id }
    }
    val selectedModelId = currentConversation?.modelId
    val activeHybridProfile = currentConversation?.backendId
        ?.takeIf { it.startsWith("hybrid:") }
        ?.removePrefix("hybrid:")
        ?.let { profileId -> state.hybridProfiles.firstOrNull { it.id == profileId } }
    // Aggregate models from ALL healthy (or degraded) *configured* providers
    // (API key stored, or local on-device backends that need none) so the picker
    // shows every available model across providers, not just the active one.
    val availableModelEntries: List<Pair<String, String>> = state.backends
        .filter {
            it.healthStatus != HealthStatus.FAILED && it.models.isNotEmpty()
                && (it.hasApiKey || it.id.startsWith("local-") || it.id == "qvac-local")
        }
        .flatMap { backend -> backend.models.map { modelId -> Pair(modelId, backend.name) } }
    var showModelMenu by remember { mutableStateOf(false) }
    val attestationBackendId = activeHybridProfile?.remoteBackendId ?: state.activeBackendId
    val activeAttestation = attestationBackendId?.let { backendId ->
        state.attestationStatuses.firstOrNull { it.backendId == backendId }?.status
    }
    val hasLocalBackend = state.backends.any { it.id.startsWith("local-") && it.models.isNotEmpty() }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 56.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        activeAttestation?.let { status ->
            AttestationBadge(
                status = status,
                verifiedLabel = attestationBackendId
                    ?.let { id -> state.backends.firstOrNull { it.id == id } }
                    ?.let { attestationTeeLabel(it.teeType) },
            )
        }
        // Model chip (mono + chevron) opens the model picker.
        Box(modifier = Modifier.weight(1f, fill = false)) {
            Surface(
                onClick = { showModelMenu = true },
                shape = CircleShape,
                color = MaterialTheme.colorScheme.surfaceContainerHigh,
            ) {
                Row(
                    modifier = Modifier.padding(start = 9.dp, end = 6.dp, top = 5.dp, bottom = 5.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    Text(
                        text = activeHybridProfile?.name
                            ?: selectedModelId?.let { shortModelName(it) }
                            ?: "Model",
                        style = MaterialTheme.typography.labelSmall.copy(
                            fontFamily = MonoFamily,
                        ),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Icon(
                        Icons.Default.KeyboardArrowDown,
                        contentDescription = null,
                        modifier = Modifier.size(13.dp),
                    )
                }
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

// MARK: - Empty Chat Hero

/// Empty-conversation hero: brand mark + prompt, a route card describing where
/// this chat runs, and a 2x2 grid of suggestion cards.
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EmptyChatHero(
    state: AppState,
    activeHybridProfile: HybridProfile?,
    composerPrefill: (String) -> Unit,
    onDispatchAction: (AppAction) -> Unit,
    modifier: Modifier = Modifier,
) {
    val currentConversation = state.currentConversationId?.let { id ->
        state.conversations.firstOrNull { it.id == id }
    }
    val backendId = currentConversation?.backendId ?: state.activeBackendId
    val routeKind = chatRouteKind(backendId, activeHybridProfile)
    val backendName = when {
        activeHybridProfile != null -> activeHybridProfile.name
        routeKind != RouteKind.TEE -> routeKind.label
        else -> backendId
            ?.let { id -> state.backends.firstOrNull { it.id == id }?.name }
            ?: RouteKind.TEE.label
    }
    val modelId = activeHybridProfile?.remoteModelId
        ?: currentConversation?.modelId
        ?: state.defaultModelId
    val attestationBackendId = activeHybridProfile?.remoteBackendId ?: backendId
    val attestation = attestationBackendId?.let { id ->
        state.attestationStatuses.firstOrNull { it.backendId == id }?.status
    }

    Column(
        modifier = modifier.verticalScroll(rememberScrollState()),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(28.dp))
        MangoMark(64.dp)
        Spacer(Modifier.height(14.dp))
        Text(
            text = "What's on your mind?",
            style = MaterialTheme.typography.headlineMedium,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.height(18.dp))

        // Route card: where this chat runs + trust status.
        MangoCard(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp),
        ) {
            FlowRow(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(14.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                RouteAvatar(routeKind)
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "This chat runs in",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        text = backendName,
                        style = MaterialTheme.typography.titleSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    modelId?.let { id ->
                        Text(
                            text = shortModelName(id),
                            style = MaterialTheme.typography.labelSmall.copy(
                                fontFamily = MonoFamily,
                            ),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                attestation?.let { status ->
                    StatusPill(
                        text = badgeLabel(status),
                        tone = attestationPillTone(status),
                        icon = badgeIcon(status),
                    )
                }
            }
        }

        // Suggestion grid (2x2).
        val docFoldersLabel = "Ask my document folders"
        val suggestions: List<SuggestionSpec> = starterPrompts().map { text ->
            SuggestionSpec(
                text = text,
                icon = when {
                    text.startsWith("Summarize") -> Icons.Outlined.Description
                    text.startsWith("Draft") -> Icons.Outlined.Edit
                    else -> Icons.Outlined.Balance
                },
                tint = when {
                    text.startsWith("Summarize") -> MangoTheme.colors.tee
                    text.startsWith("Draft") -> MangoTheme.colors.local
                    else -> MangoTheme.colors.hybrid
                },
                onClick = { composerPrefill(text) },
            )
        } + SuggestionSpec(
            text = docFoldersLabel,
            icon = Icons.Outlined.FolderOpen,
            tint = MangoTheme.colors.tintPrivacy,
            onClick = {
                if (state.directorySources.isEmpty()) {
                    onDispatchAction(AppAction.PushScreen(screen = Screen.DirectorySources))
                } else {
                    // Directory sources feed retrieval automatically; prefill a
                    // prompt that exercises it.
                    composerPrefill("Summarize what's in my document folders")
                }
            },
        )
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            suggestions.chunked(2).forEach { rowItems ->
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    rowItems.forEach { spec ->
                        SuggestionCard(spec, modifier = Modifier.weight(1f))
                    }
                    if (rowItems.size == 1) Spacer(Modifier.weight(1f))
                }
            }
        }
    }
}

private data class SuggestionSpec(
    val text: String,
    val icon: ImageVector,
    val tint: TintPair,
    val onClick: () -> Unit,
)

@Composable
private fun SuggestionCard(spec: SuggestionSpec, modifier: Modifier = Modifier) {
    MangoCard(modifier = modifier, onClick = spec.onClick) {
        Column(modifier = Modifier.padding(12.dp)) {
            TintedIcon(icon = spec.icon, tint = spec.tint, size = 30.dp)
            Spacer(Modifier.height(8.dp))
            Text(
                text = spec.text,
                style = MaterialTheme.typography.labelLarge,
            )
        }
    }
}

/// Where this chat's inference runs, derived from the conversation's backend.
internal fun chatRouteKind(backendId: String?, profile: HybridProfile?): RouteKind = when {
    profile != null || backendId?.startsWith("hybrid:") == true -> RouteKind.HYBRID
    backendId?.startsWith("local-") == true || backendId == "qvac-local" -> RouteKind.LOCAL
    else -> RouteKind.TEE
}

/// True when this message starts a consecutive run of assistant messages
/// (the "Mango" label header goes above the first of a run).
internal fun isFirstOfRun(messages: List<UiMessage>, index: Int): Boolean {
    val message = messages.getOrNull(index) ?: return false
    if (message.role != "assistant") return false
    return messages.getOrNull(index - 1)?.role != "assistant"
}

/// Centered provenance divider rendered when the route changes mid-thread.
@Composable
private fun RouteDivider(label: String, icon: ImageVector) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        HorizontalDivider(modifier = Modifier.width(16.dp))
        Icon(
            icon,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(12.dp),
        )
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        HorizontalDivider(modifier = Modifier.width(16.dp))
    }
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
