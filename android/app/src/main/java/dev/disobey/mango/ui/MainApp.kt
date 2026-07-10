package dev.disobey.mango.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.disobey.mango.AppManager
import dev.disobey.mango.FeatureFlags
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.Screen
import kotlinx.coroutines.channels.Channel

/// Root composable: routes to Settings, Chat, or Home based on router state.
@Composable
fun MainApp(
    manager: AppManager,
    themeMode: String = "system",
    onThemeModeChanged: (String) -> Unit = {},
    fontSize: String = "normal",
    onFontSizeChanged: (String) -> Unit = {},
) {
    // Wait for the Rust actor's first state emission before rendering any screen.
    // Without this guard, Compose renders the hardcoded Screen.Home default state
    // for one or more frames before the actor finishes DB init and sends the real
    // initial screen (e.g. Screen.Onboarding on first install), causing a visible flash.
    if (!manager.isReady) {
        Box(modifier = Modifier) // blank frame — invisible to user, resolves in <100ms
        return
    }

    val state by manager.stateFlow.collectAsState(initial = manager.state)
    val context = LocalContext.current
    val snackbarHostState = remember { SnackbarHostState() }
    val snackbarQueue = remember { Channel<String>(Channel.UNLIMITED) }

    LaunchedEffect(state.toast) {
        val toast = state.toast
        if (toast != null && state.router.currentScreen !is Screen.Locked) {
            snackbarQueue.trySend(toast)
            manager.dispatch(AppAction.ClearToast)
        }
    }

    LaunchedEffect(Unit) {
        for (message in snackbarQueue) {
            snackbarHostState.showSnackbar(message)
        }
    }

    // Intercept the Android system back gesture / button at the Compose root so it
    // dispatches PopScreen to the Rust router instead of falling through to the
    // Activity's default handler (which calls finish() and exits the app).
    //
    // The Rust router's screen_stack is the single source of truth for "is there
    // somewhere to go back to": PushScreen pushes onto the stack, LoadConversation
    // / NewConversation / SendMessage-auto-create also push the previous screen
    // (see push_nav_history in rust/src/lib.rs), and PopScreen pops and restores.
    // When the stack is empty we DO NOT intercept, so back on Home / Onboarding /
    // Locked falls through to Activity.finish() and exits the app -- the expected
    // Android behavior on root screens.
    BackHandler(enabled = state.router.screenStack.isNotEmpty()) {
        manager.dispatch(AppAction.PopScreen)
    }

    Box(modifier = Modifier.fillMaxSize()) {
        when (val screen = state.router.currentScreen) {
            is Screen.Onboarding -> {
                OnboardingScreen(
                    state = state,
                    onDispatch = { action -> manager.dispatch(action) }
                )
            }
        is Screen.Settings -> {
            SettingsScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) },
                themeMode = themeMode,
                onThemeModeChanged = onThemeModeChanged,
                fontSize = fontSize,
            )
        }
        is Screen.Chat -> {
            ChatScreen(
                state = state,
                onSend = { text, forceRole ->
                    manager.dispatch(AppAction.SendMessage(text = text, forceRole = forceRole))
                },
                onStop = { manager.dispatch(AppAction.StopGeneration) },
                onRetry = { manager.dispatch(AppAction.RetryLastMessage) },
                onEdit = { id, text -> manager.dispatch(AppAction.EditMessage(messageId = id, newText = text)) },
                onCopy = { text ->
                    // Copy message text to the system clipboard.
                    // On Android 13+ (API 33+) the OS displays its own "copied" confirmation
                    // UI automatically, so we suppress the Toast to avoid double-feedback.
                    // On Android 9-12 (API 28-32) the OS shows nothing, so we Toast manually
                    // to confirm the action succeeded.
                    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
                    if (clipboard != null) {
                        clipboard.setPrimaryClip(ClipData.newPlainText("message", text))
                        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
                            Toast.makeText(context, "Copied", Toast.LENGTH_SHORT).show()
                        }
                    }
                },
                onAttach = { filename, content, size -> manager.dispatch(AppAction.AttachFile(filename = filename, content = content, sizeBytes = size)) },
                onAttachImage = { filename, filePath, mimeType -> manager.dispatch(AppAction.AttachImage(filename = filename, filePath = filePath, mimeType = mimeType)) },
                onClearAttachment = { manager.dispatch(AppAction.ClearAttachment) },
                onSelectModel = { model -> manager.dispatch(AppAction.SelectModel(modelId = model)) },
                onSetSystemPrompt = { prompt -> manager.dispatch(AppAction.SetSystemPrompt(prompt = prompt)) },
                onBack = { manager.dispatch(AppAction.PopScreen) },
                onAttachDocument = { docId -> manager.dispatch(AppAction.AttachDocumentToConversation(documentId = docId)) },
                onDetachDocument = { docId -> manager.dispatch(AppAction.DetachDocumentFromConversation(documentId = docId)) },
                onShareConversation = { conversationId ->
                    try {
                        val markdown = manager.exportConversationMarkdown(conversationId)
                        val shareIntent = Intent(Intent.ACTION_SEND).apply {
                            type = "text/plain"
                            putExtra(Intent.EXTRA_SUBJECT, "Mango conversation")
                            putExtra(Intent.EXTRA_TEXT, markdown)
                        }
                        context.startActivity(
                            Intent.createChooser(shareIntent, "Share conversation")
                                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        )
                    } catch (e: Exception) {
                        manager.dispatch(
                            AppAction.ShowToast(
                                message = e.message ?: "Could not export conversation",
                            )
                        )
                    }
                },
                onDispatchAction = { action -> manager.dispatch(action) },
                onReadEncryptedImage = { messageId -> manager.readEncryptedImage(messageId) },
                fontScale = fontSizeToScale(fontSize),
            )
        }
        is Screen.Home -> {
            ConversationListScreen(
                state = state,
                onSelect = { id -> manager.dispatch(AppAction.LoadConversation(conversationId = id)) },
                onNew = { manager.dispatch(AppAction.NewConversation) },
                onDelete = { id -> manager.dispatch(AppAction.DeleteConversation(id = id)) },
                onRename = { id, title -> manager.dispatch(AppAction.RenameConversation(id = id, title = title)) },
                onFork = { id -> manager.dispatch(AppAction.ForkConversation(id = id)) },
                topBarActions = {
                    if (FeatureFlags.AGENTS_ENABLED) {
                        TextButton(onClick = { manager.dispatch(AppAction.PushScreen(screen = Screen.Agents)) }) {
                            Text("Agents")
                        }
                    }
                    TextButton(onClick = { manager.dispatch(AppAction.PushScreen(screen = Screen.Documents)) }) {
                        Text("Library")
                    }
                    TextButton(onClick = { manager.dispatch(AppAction.PushScreen(screen = Screen.Settings)) }) {
                        Text("Settings")
                    }
                },
            )
        }
        is Screen.Documents -> {
            DocumentLibraryScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.DirectorySources -> {
            DirectorySourcesScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.Memories -> {
            MemoryScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.Agents -> {
            if (FeatureFlags.AGENTS_ENABLED) {
                AgentScreen(
                    appState = state,
                    onDispatch = { action -> manager.dispatch(action) },
                    onBack = { manager.dispatch(AppAction.PopScreen) }
                )
            } else {
                LaunchedEffect(Unit) {
                    manager.dispatch(AppAction.PopScreen)
                }
                Box(modifier = Modifier.fillMaxSize())
            }
        }
        is Screen.SettingsProviders -> {
            SettingsProvidersScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.SettingsDefaults -> {
            SettingsDefaultsScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.SettingsMemory -> {
            SettingsMemoryScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.SettingsAppearance -> {
            SettingsAppearanceScreen(
                themeMode = themeMode,
                onBack = { manager.dispatch(AppAction.PopScreen) },
                onThemeModeChanged = onThemeModeChanged,
                fontSize = fontSize,
                onFontSizeChanged = onFontSizeChanged,
            )
        }
        is Screen.SettingsSecurity -> {
            SettingsSecurityScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.SettingsTools -> {
            SettingsToolsScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.SettingsLocalModels -> {
            SettingsLocalModelsScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.SettingsHybridRouting -> {
            SettingsHybridRoutingScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.ToolDiscovery -> {
            SettingsToolDiscoveryScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        // Phase 36: tool detail sub-screen reached via row tap.
        is Screen.ContextvmToolDetail -> {
            SettingsToolDetailScreen(
                appState = state,
                toolId = screen.toolId,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        // Phase 38: trusted providers management screen.
        is Screen.TrustedProviders -> {
            TrustedProvidersScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        // Phase 28: lock gate and PIN setup screens
        is Screen.Locked -> {
            LockScreen(
                appState = state,
                onDispatchAction = { action -> manager.dispatch(action) }
            )
        }
            is Screen.PinSetup -> {
                PinSetupScreen(
                    appState = state,
                    onDispatchAction = { action -> manager.dispatch(action) }
                )
            }
        }
        SnackbarHost(
            hostState = snackbarHostState,
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 24.dp),
        )
    }
}

internal fun fontSizeToScale(fontSize: String): Float = when (fontSize) {
    "small" -> 0.85f
    "large" -> 1.15f
    "xlarge" -> 1.3f
    else -> 1f
}
