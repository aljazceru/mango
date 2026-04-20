package dev.disobey.mango.ui

import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.AppManager
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.DirectorySourceSummary
import dev.disobey.mango.rust.DirectorySyncStatus
import kotlinx.coroutines.launch

/**
 * Phase 32 Plan 06 — Android directory-source management UI.
 *
 * Mirrors the iOS `DirectorySourcesView.swift` (plan 32-05) and the desktop
 * `directory_sources.rs` (plan 32-04): list rows + add folder + per-source
 * sync-now + exclusion editor + remove-with-confirmation.
 *
 * All platform-specific bits (SAF picker, tree URI resolution, tree traversal)
 * live in `DirectorySourcePicker.kt`; this file only composes the UI and routes
 * actions.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DirectorySourcesScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    // Per-row transient UI state — "show edit dialog for source X", "confirm remove".
    var editingSource by remember { mutableStateOf<DirectorySourceSummary?>(null) }
    var confirmRemoveSource by remember { mutableStateOf<DirectorySourceSummary?>(null) }

    // Newly-picked URIs cached by display name so the first Sync Now can resolve
    // the URI even before the source re-appears in AppState.
    val pickedUrisByName = remember { mutableStateOf<Map<String, Uri>>(emptyMap()) }

    val openPicker = rememberDirectoryPicker { uri, displayName ->
        pickedUrisByName.value = pickedUrisByName.value + (displayName to uri)
        onDispatch(
            AppAction.AddDirectorySource(
                displayName = displayName,
                path = null,
                bookmarkData = null,
                treeUri = uri.toString(),
                exclusionGlobs = DEFAULT_EXCLUSION_PRESETS,
            )
        )
    }

    // Kick a sync for any newly-added source whose URI is cached but whose
    // file_count is still 0 (the actor just inserted the row, no sync yet — D-02).
    LaunchedEffect(appState.directorySources.map { it.id to it.fileCount }) {
        for (source in appState.directorySources) {
            if (source.fileCount > 0L) continue
            val uri = pickedUrisByName.value[source.displayName] ?: continue
            scope.launch {
                syncDirectory(context, source, uri) { action -> onDispatch(action) }
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Directory Sources") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
            )
        },
        floatingActionButton = {
            FloatingActionButton(onClick = openPicker) {
                Icon(Icons.Filled.Add, contentDescription = "Add folder")
            }
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            if (appState.directorySources.isEmpty()) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(24.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        "No directory sources yet. Add a folder to sync your notes.",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    items(appState.directorySources, key = { it.id }) { source ->
                        // Resolve the tree URI string once per row so we can derive
                        // both the human-readable path and the URI for the open-in-files intent.
                        val treeUriString = resolveTreeUri(context, source)
                            ?: pickedUrisByName.value[source.displayName]?.toString()
                        // Derive a human-readable relative path from the tree URI document ID
                        // (e.g. "primary:Download/test" → "Download/test"). Falls back to
                        // displayName when the URI is not yet resolved.
                        val displayPath: String = treeUriString?.let { uriStr ->
                            try {
                                DocumentsContract.getTreeDocumentId(Uri.parse(uriStr))
                                    .substringAfterLast(':')
                                    .trimStart('/')
                                    .ifEmpty { null }
                            } catch (_: Exception) { null }
                        } ?: source.displayName

                        DirectorySourceRow(
                            source = source,
                            displayPath = displayPath,
                            onSyncNow = {
                                val uri = treeUriString?.let(Uri::parse)
                                if (uri != null) {
                                    onDispatch(AppAction.TriggerDirectorySync(sourceId = source.id))
                                    scope.launch {
                                        syncDirectory(context, source, uri) { action ->
                                            onDispatch(action)
                                        }
                                    }
                                }
                            },
                            onEdit = { editingSource = source },
                            onRemove = { confirmRemoveSource = source },
                            onOpenFolder = {
                                treeUriString?.let { uriStr ->
                                    try {
                                        val intent = Intent(Intent.ACTION_VIEW).apply {
                                            setDataAndType(Uri.parse(uriStr), DocumentsContract.Document.MIME_TYPE_DIR)
                                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                        }
                                        context.startActivity(intent)
                                    } catch (_: Exception) {
                                        // DocumentsUI not present on this device — silently ignore.
                                    }
                                }
                            },
                        )
                    }
                }
            }
        }
    }

    // Exclusion editor sheet
    editingSource?.let { source ->
        ExclusionEditorDialog(
            source = source,
            onDismiss = { editingSource = null },
            onSave = { newGlobs ->
                onDispatch(
                    AppAction.SetDirectoryExclusions(
                        sourceId = source.id,
                        globs = newGlobs,
                    )
                )
                editingSource = null
            },
        )
    }

    // Remove-confirm dialog (DIR-06)
    confirmRemoveSource?.let { source ->
        AlertDialog(
            onDismissRequest = { confirmRemoveSource = null },
            title = { Text("Remove ${source.displayName}?") },
            text = {
                Text(
                    "Remove source and delete ${formatFileCount(source.fileCount)} indexed chunks? " +
                        "The folder itself is not deleted.",
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        onDispatch(AppAction.RemoveDirectorySource(sourceId = source.id))
                        confirmRemoveSource = null
                    },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) { Text("Remove") }
            },
            dismissButton = {
                TextButton(onClick = { confirmRemoveSource = null }) { Text("Cancel") }
            },
        )
    }
}

@Composable
private fun DirectorySourceRow(
    source: DirectorySourceSummary,
    displayPath: String,
    onSyncNow: () -> Unit,
    onEdit: () -> Unit,
    onRemove: () -> Unit,
    onOpenFolder: () -> Unit,
) {
    // Compact content padding so four buttons fit on one line on a 360dp-wide screen.
    val compactPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp)

    Card(
        elevation = CardDefaults.cardElevation(defaultElevation = 1.dp),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    Icons.Filled.Folder,
                    contentDescription = null,
                    modifier = Modifier.size(24.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
                Spacer(modifier = Modifier.size(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(source.displayName, style = MaterialTheme.typography.titleMedium)
                    // Full path under the display name — shows the storage location.
                    if (displayPath != source.displayName) {
                        Text(
                            displayPath,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    val statusText = when (val st = source.syncStatus) {
                        is DirectorySyncStatus.Idle ->
                            "${formatFileCount(source.fileCount)} files · " +
                                "Last synced: ${source.lastSyncedLabel}"
                        is DirectorySyncStatus.Syncing -> "Syncing…"
                        is DirectorySyncStatus.Error -> "Error: ${st.message}"
                    }
                    val statusColor = when (source.syncStatus) {
                        is DirectorySyncStatus.Error -> MaterialTheme.colorScheme.error
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    }
                    Text(statusText, style = MaterialTheme.typography.bodySmall, color = statusColor)
                }
            }
            Spacer(modifier = Modifier.height(8.dp))
            // Use compact contentPadding so all four buttons fit on one row without wrapping.
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                OutlinedButton(
                    onClick = onSyncNow,
                    contentPadding = compactPadding,
                ) {
                    Icon(Icons.Filled.Refresh, contentDescription = null, modifier = Modifier.size(14.dp))
                    Spacer(modifier = Modifier.size(4.dp))
                    Text("Sync", style = MaterialTheme.typography.labelSmall)
                }
                OutlinedButton(
                    onClick = onEdit,
                    contentPadding = compactPadding,
                ) {
                    Icon(Icons.Filled.Edit, contentDescription = null, modifier = Modifier.size(14.dp))
                    Spacer(modifier = Modifier.size(4.dp))
                    Text("Edit", style = MaterialTheme.typography.labelSmall)
                }
                OutlinedButton(
                    onClick = onOpenFolder,
                    contentPadding = compactPadding,
                ) {
                    Icon(Icons.Filled.FolderOpen, contentDescription = null, modifier = Modifier.size(14.dp))
                    Spacer(modifier = Modifier.size(4.dp))
                    Text("Open", style = MaterialTheme.typography.labelSmall)
                }
                OutlinedButton(
                    onClick = onRemove,
                    contentPadding = compactPadding,
                    colors = ButtonDefaults.outlinedButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) {
                    Icon(Icons.Filled.Delete, contentDescription = null, modifier = Modifier.size(14.dp))
                    Spacer(modifier = Modifier.size(4.dp))
                    Text("Remove", style = MaterialTheme.typography.labelSmall)
                }
            }
        }
    }
}

/**
 * Inline exclusion editor — multi-line TextField, one glob per line.
 *
 * Validation is client-side lightweight (D-29 explicitly allows local
 * validation; the Rust side re-validates every glob via `validate_glob_pattern`
 * on `SetDirectoryExclusions` so anything genuinely invalid will be rejected
 * server-side anyway — T-32-V5).
 */
@Composable
private fun ExclusionEditorDialog(
    source: DirectorySourceSummary,
    onDismiss: () -> Unit,
    onSave: (List<String>) -> Unit,
) {
    var text by remember { mutableStateOf(source.exclusionGlobs.joinToString("\n")) }
    val lines = text.split("\n").map { it.trim() }.filter { it.isNotEmpty() }
    val invalid = lines.filterNot { it.looksLikeValidGlob() }
    val canSave = invalid.isEmpty()

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Exclusions for ${source.displayName}") },
        text = {
            Column {
                Text(
                    "One glob per line. Examples: .obsidian/, *.tmp, .DS_Store",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(8.dp))
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(200.dp),
                    textStyle = MaterialTheme.typography.bodyMedium.copy(
                        fontFamily = FontFamily.Monospace,
                    ),
                )
                if (invalid.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        "Invalid patterns: ${invalid.joinToString(", ")}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = {
            Button(onClick = { onSave(lines) }, enabled = canSave) { Text("Save") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}

/**
 * Lightweight local glob validation — rejects obviously-malformed patterns
 * (empty, pure whitespace, unbalanced brackets). The Rust side runs the
 * authoritative `validate_glob_pattern` on save (T-32-V5).
 */
private fun String.looksLikeValidGlob(): Boolean {
    if (isBlank()) return false
    // Crude bracket balance check — real glob grammar is complex; full validation
    // happens on the Rust side via globset.
    val opens = count { it == '[' }
    val closes = count { it == ']' }
    if (opens != closes) return false
    return true
}

/**
 * Locale-aware thousands-grouping formatter for file counts (e.g. 1234 → "1,234" in en-US).
 *
 * Relative-time labels are now produced by the Rust core as `DirectorySourceSummary.lastSyncedLabel`
 * so all three platforms render identical strings (Plan 32-07).
 */
private fun formatFileCount(n: Long): String =
    java.text.NumberFormat.getIntegerInstance().format(n)

@Suppress("unused") // referenced via symbol name by Color import deduplication
private val Unused: Color = Color.Transparent
