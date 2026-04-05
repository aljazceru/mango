package dev.disobey.mango.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import dev.disobey.mango.AppManager
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.Screen

/// Root composable: routes to Settings, Chat, or Home based on router state.
@Composable
fun MainApp(
    manager: AppManager,
    themeMode: String = "system",
    onThemeModeChanged: (String) -> Unit = {},
) {
    // Wait for the Rust actor's first state emission before rendering any screen.
    // Without this guard, Compose renders the hardcoded Screen.Home default state
    // for one or more frames before the actor finishes DB init and sends the real
    // initial screen (e.g. Screen.Onboarding on first install), causing a visible flash.
    if (!manager.isReady) {
        Box(modifier = Modifier) // blank frame — invisible to user, resolves in <100ms
        return
    }

    val state = manager.state

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
            )
        }
        is Screen.Chat -> {
            ChatScreen(
                state = state,
                onSend = { text -> manager.dispatch(AppAction.SendMessage(text = text)) },
                onStop = { manager.dispatch(AppAction.StopGeneration) },
                onRetry = { manager.dispatch(AppAction.RetryLastMessage) },
                onEdit = { id, text -> manager.dispatch(AppAction.EditMessage(messageId = id, newText = text)) },
                onCopy = { _ -> },
                onAttach = { filename, content, size -> manager.dispatch(AppAction.AttachFile(filename = filename, content = content, sizeBytes = size)) },
                onClearAttachment = { manager.dispatch(AppAction.ClearAttachment) },
                onSelectModel = { model -> manager.dispatch(AppAction.SelectModel(modelId = model)) },
                onSetSystemPrompt = { prompt -> manager.dispatch(AppAction.SetSystemPrompt(prompt = prompt)) },
                onBack = { manager.dispatch(AppAction.PopScreen) },
                onAttachDocument = { docId -> manager.dispatch(AppAction.AttachDocumentToConversation(documentId = docId)) },
                onDetachDocument = { docId -> manager.dispatch(AppAction.DetachDocumentFromConversation(documentId = docId)) }
            )
        }
        is Screen.Home -> {
            ConversationListScreen(
                state = state,
                onSelect = { id -> manager.dispatch(AppAction.LoadConversation(conversationId = id)) },
                onNew = { manager.dispatch(AppAction.NewConversation) },
                onDelete = { id -> manager.dispatch(AppAction.DeleteConversation(id = id)) },
                onRename = { id, title -> manager.dispatch(AppAction.RenameConversation(id = id, title = title)) },
                topBarActions = {
                    TextButton(onClick = { manager.dispatch(AppAction.PushScreen(screen = Screen.Agents)) }) {
                        Text("Agents")
                    }
                    TextButton(onClick = { manager.dispatch(AppAction.PushScreen(screen = Screen.Documents)) }) {
                        Text("RAG")
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
        is Screen.Memories -> {
            MemoryScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        is Screen.Agents -> {
            AgentScreen(
                appState = state,
                onDispatch = { action -> manager.dispatch(action) },
                onBack = { manager.dispatch(AppAction.PopScreen) }
            )
        }
        else -> {}
    }
}
