package dev.disobey.mango.ui

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.ConversationRetentionMode

@Stable
class DataRetentionUiState {
    var retentionExpanded by mutableStateOf(false)
    var retentionDaysText by mutableStateOf("")
    var showDeleteChatsConfirm by mutableStateOf(false)
    var showDeleteDataConfirm by mutableStateOf(false)
}

@Composable
fun rememberDataRetentionUiState() = remember { DataRetentionUiState() }

@Composable
fun DataRetentionDialogs(appState: AppState, onDispatch: (AppAction) -> Unit, ui: DataRetentionUiState) {
        if (ui.showDeleteChatsConfirm) {
            AlertDialog(
                onDismissRequest = { ui.showDeleteChatsConfirm = false },
                title = { Text("Delete All Chats") },
                text = {
                    Text("This will permanently delete every conversation and message on this device.")
                },
                confirmButton = {
                    TextButton(
                        onClick = {
                            ui.showDeleteChatsConfirm = false
                            onDispatch(AppAction.DeleteAllConversations)
                        }
                    ) {
                        Text("Delete", color = MaterialTheme.colorScheme.error)
                    }
                },
                dismissButton = {
                    TextButton(onClick = { ui.showDeleteChatsConfirm = false }) {
                        Text("Cancel")
                    }
                },
            )
        }

        if (ui.showDeleteDataConfirm) {
            AlertDialog(
                onDismissRequest = { ui.showDeleteDataConfirm = false },
                title = { Text("Delete All Data") },
                text = {
                    Text("This will permanently delete chats, documents, memories, API keys, auth data, and local files, then return the app to clean-install state.")
                },
                confirmButton = {
                    TextButton(
                        onClick = {
                            ui.showDeleteDataConfirm = false
                            onDispatch(AppAction.DeleteAllData)
                        }
                    ) {
                        Text("Delete Everything", color = MaterialTheme.colorScheme.error)
                    }
                },
                dismissButton = {
                    TextButton(onClick = { ui.showDeleteDataConfirm = false }) {
                        Text("Cancel")
                    }
                },
            )
        }
}

@OptIn(ExperimentalMaterial3Api::class)
fun LazyListScope.dataRetentionItems(appState: AppState, onDispatch: (AppAction) -> Unit, ui: DataRetentionUiState) {
    item {
        Card(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Conversation retention", fontWeight = FontWeight.Medium)
                        Text(
                            "Automatically archive or delete conversations older than the given number of days. Archived conversations are hidden from the chat list and can be restored below. Off by default.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )

                        val retentionOptions = listOf(
                            "Off" to ConversationRetentionMode.OFF,
                            "Archive" to ConversationRetentionMode.ARCHIVE,
                            "Delete" to ConversationRetentionMode.DELETE,
                        )
                        val retentionLabel = retentionOptions
                            .firstOrNull { it.second == appState.conversationRetentionMode }?.first ?: "Off"
                        ExposedDropdownMenuBox(
                            expanded = ui.retentionExpanded,
                            onExpandedChange = { ui.retentionExpanded = it },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            OutlinedTextField(
                                value = retentionLabel,
                                onValueChange = {},
                                readOnly = true,
                                label = { Text("Mode") },
                                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = ui.retentionExpanded) },
                                modifier = Modifier.menuAnchor().fillMaxWidth()
                            )
                            DropdownMenu(
                                expanded = ui.retentionExpanded,
                                onDismissRequest = { ui.retentionExpanded = false }
                            ) {
                                retentionOptions.forEach { (label, mode) ->
                                    DropdownMenuItem(
                                        text = { Text(label) },
                                        onClick = {
                                            onDispatch(AppAction.SetConversationRetention(
                                                mode = mode,
                                                days = appState.conversationRetentionDays,
                                            ))
                                            ui.retentionExpanded = false
                                        }
                                    )
                                }
                            }
                        }

                        Row(modifier = Modifier.fillMaxWidth()) {
                            OutlinedTextField(
                                value = ui.retentionDaysText.ifEmpty { appState.conversationRetentionDays.toString() },
                                onValueChange = { ui.retentionDaysText = it.filter { c -> c.isDigit() } },
                                label = { Text("Days") },
                                singleLine = true,
                                modifier = Modifier.weight(1f),
                            )
                            OutlinedButton(
                                onClick = {
                                    ui.retentionDaysText.toUIntOrNull()?.let { days ->
                                        if (days > 0u) {
                                            onDispatch(AppAction.SetConversationRetention(
                                                mode = appState.conversationRetentionMode,
                                                days = days,
                                            ))
                                        }
                                    }
                                    ui.retentionDaysText = ""
                                },
                                modifier = Modifier.padding(start = 8.dp),
                            ) {
                                Text("Apply")
                            }
                        }

                        if (appState.conversationRetentionMode == ConversationRetentionMode.DELETE) {
                            Text(
                                "Delete permanently removes conversations and their messages once they pass the threshold. This cannot be undone.",
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.error,
                            )
                        }

                        if (appState.archivedConversations.isNotEmpty()) {
                            Text(
                                "Archived conversations",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            appState.archivedConversations.forEach { conv ->
                                Row(modifier = Modifier.fillMaxWidth()) {
                                    Text(
                                        conv.title,
                                        modifier = Modifier.weight(1f),
                                        style = MaterialTheme.typography.bodyMedium,
                                        maxLines = 1,
                                    )
                                    TextButton(onClick = { onDispatch(AppAction.UnarchiveConversation(id = conv.id)) }) {
                                        Text("Unarchive")
                                    }
                                }
                            }
                        }
                    }

                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        SettingsSectionLabel("Danger zone")
                        Text("Delete all chats", fontWeight = FontWeight.Medium)
                        Text(
                            "Remove every conversation and message stored on this device.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { ui.showDeleteChatsConfirm = true },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Delete All Chats", color = MaterialTheme.colorScheme.error)
                        }
                    }

                    HorizontalDivider(modifier = Modifier.padding(horizontal = 16.dp))

                    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                        Text("Delete all data", fontWeight = FontWeight.Medium)
                        Text(
                            "Erase chats, documents, memories, API keys, auth data, and local files, then return to the first-launch app state.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { ui.showDeleteDataConfirm = true },
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Delete All Data", color = MaterialTheme.colorScheme.error)
                        }
                    }
                }
            }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsDataRetentionScreen(appState: AppState, onDispatch: (AppAction) -> Unit, onBack: () -> Unit = { onDispatch(AppAction.PopScreen) }) {
    val ui = rememberDataRetentionUiState()
    DataRetentionDialogs(appState, onDispatch, ui)
    Scaffold(topBar = {
        TopAppBar(
            title = { Text("Data & retention", fontWeight = FontWeight.Medium) },
            navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
        )
    }) { pad ->
        LazyColumn(
            modifier = Modifier.fillMaxSize().padding(pad).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) { dataRetentionItems(appState, onDispatch, ui) }
    }
}
