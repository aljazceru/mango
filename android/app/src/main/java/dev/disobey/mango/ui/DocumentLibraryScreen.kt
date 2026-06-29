package dev.disobey.mango.ui

import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.material.icons.filled.Article
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.DirectorySourceSummary
import dev.disobey.mango.rust.DirectorySyncStatus
import dev.disobey.mango.rust.DocumentSummary
import dev.disobey.mango.rust.Screen
import kotlinx.coroutines.launch

/// Unified RAG screen: lists documents + directory sources under one entry
/// (LRAG-06, DIR-05). The Home toolbar routes here via Screen.Documents; the
/// legacy DirectorySources screen remains reachable by tapping a folder row.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DocumentLibraryScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    var showAddMenu by remember { mutableStateOf(false) }

    // Cache URIs by display name so the first Sync Now can resolve even before
    // the source row re-appears in AppState. Mirrors DirectorySourcesScreen.
    val pickedUrisByName = remember { mutableStateOf<Map<String, Uri>>(emptyMap()) }

    val openDocumentLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri: Uri? ->
        uri?.let {
            val contentResolver = context.contentResolver
            val filename = contentResolver.query(it, null, null, null, null)?.use { cursor ->
                val nameIndex = cursor.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                cursor.moveToFirst()
                if (nameIndex >= 0) cursor.getString(nameIndex) else "document"
            } ?: it.lastPathSegment ?: "document"

            try {
                val bytes = contentResolver.openInputStream(it)?.use { stream ->
                    stream.readBytes()
                } ?: return@let
                onDispatch(
                    AppAction.IngestDocument(
                        filename = filename,
                        content = bytes,
                    )
                )
            } catch (e: Exception) {
                // File read error -- future plan adds toast
            }
        }
    }

    // Folder picker — same launcher pattern as DirectorySourcesScreen.kt. The
    // persisted URI permission is taken inside rememberDirectoryPicker, so
    // bookmark rehydration remains intact.
    val openFolderPicker = rememberDirectoryPicker { uri, displayName ->
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

    // Kick an initial sync for any newly-added folder whose URI is cached but
    // whose file_count is still 0 — matches DirectorySourcesScreen behavior.
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
                title = { Text("RAG") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        },
        floatingActionButton = {
            Box {
                FloatingActionButton(onClick = { showAddMenu = true }) {
                    Icon(Icons.Filled.Add, contentDescription = "Add RAG source")
                }
                DropdownMenu(
                    expanded = showAddMenu,
                    onDismissRequest = { showAddMenu = false },
                ) {
                    DropdownMenuItem(
                        text = { Text("Document") },
                        onClick = {
                            showAddMenu = false
                            openDocumentLauncher.launch(
                                arrayOf("application/pdf", "text/plain", "text/markdown")
                            )
                        },
                    )
                    DropdownMenuItem(
                        text = { Text("Folder") },
                        onClick = {
                            showAddMenu = false
                            openFolderPicker()
                        },
                    )
                }
            }
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            // Ingestion progress indicator (unchanged)
            appState.ingestionProgress?.let { progress ->
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp)
                ) {
                    Text(
                        text = "${progress.documentName}: ${progress.stage}...",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(bottom = 4.dp)
                    )
                    LinearProgressIndicator(
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            }

            val bothEmpty = appState.documents.isEmpty() &&
                appState.directorySources.isEmpty() &&
                appState.ingestionProgress == null

            if (bothEmpty) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Icon(
                            Icons.Filled.Description,
                            contentDescription = null,
                            modifier = Modifier.size(48.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            "No RAG sources yet",
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            "Tap + to add a document or a folder.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    if (appState.directorySources.isNotEmpty()) {
                        item(key = "folders_header") {
                            Text(
                                text = "FOLDERS",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(vertical = 2.dp)
                            )
                        }
                        items(appState.directorySources, key = { "dir_${it.id}" }) { src ->
                            DirectorySourceCompactRow(
                                source = src,
                                onClick = {
                                    onDispatch(
                                        AppAction.PushScreen(screen = Screen.DirectorySources)
                                    )
                                },
                            )
                        }
                    }

                    if (appState.documents.isNotEmpty()) {
                        item(key = "documents_header") {
                            Text(
                                text = "DOCUMENTS",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(vertical = 2.dp)
                            )
                        }
                        items(appState.documents, key = { "doc_${it.id}" }) { doc ->
                            DocumentRow(
                                doc = doc,
                                onDelete = {
                                    onDispatch(AppAction.DeleteDocument(documentId = doc.id))
                                }
                            )
                        }
                    }
                }
            }
        }
    }
}

/// Compact directory-source row shown inside the unified RAG screen. Tapping
/// pushes Screen.DirectorySources so the full management UI (exclusions, sync,
/// remove) stays reachable without being a top-level Home entry.
@Composable
private fun DirectorySourceCompactRow(
    source: DirectorySourceSummary,
    onClick: () -> Unit,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() },
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Filled.Folder,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(28.dp),
            )
            Spacer(modifier = Modifier.size(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = source.displayName,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                val statusText = when (val st = source.syncStatus) {
                    is DirectorySyncStatus.Idle ->
                        "${source.fileCount} files · ${source.lastSyncedLabel}"
                    is DirectorySyncStatus.Syncing -> "Syncing…"
                    is DirectorySyncStatus.Error -> "Error: ${st.message}"
                }
                val statusColor = when (source.syncStatus) {
                    is DirectorySyncStatus.Error -> MaterialTheme.colorScheme.error
                    else -> MaterialTheme.colorScheme.onSurfaceVariant
                }
                Text(
                    text = statusText,
                    style = MaterialTheme.typography.labelSmall,
                    color = statusColor,
                )
            }
        }
    }
}

@Composable
private fun DocumentRow(
    doc: DocumentSummary,
    onDelete: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Format icon
            Icon(
                imageVector = if (doc.format == "pdf") Icons.Filled.Description else Icons.Default.Article,
                contentDescription = doc.format,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .size(32.dp)
                    .padding(end = 4.dp)
            )

            Spacer(modifier = Modifier.padding(horizontal = 4.dp))

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = doc.name,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 1
                )
                Spacer(modifier = Modifier.height(2.dp))
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    // Format badge
                    SuggestionChip(
                        onClick = {},
                        label = {
                            Text(
                                text = doc.format.uppercase(),
                                style = MaterialTheme.typography.labelSmall
                            )
                        },
                        modifier = Modifier.height(24.dp)
                    )
                    Text(
                        text = formatSize(doc.sizeBytes),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = formatDate(doc.ingestionDate),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        text = "${doc.chunkCount} chunks",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }

            // Delete button
            IconButton(onClick = onDelete) {
                Icon(
                    Icons.Filled.Delete,
                    contentDescription = "Delete document",
                    tint = MaterialTheme.colorScheme.error
                )
            }
        }
    }
}

private fun formatSize(sizeBytes: ULong): String {
    val bytes = sizeBytes.toLong()
    return when {
        bytes < 1024 -> "$bytes B"
        bytes < 1024 * 1024 -> "%.1f KB".format(bytes / 1024.0)
        else -> "%.1f MB".format(bytes / (1024.0 * 1024.0))
    }
}

private fun formatDate(unixTimestamp: Long): String {
    val now = System.currentTimeMillis() / 1000L
    val diff = now - unixTimestamp
    return when {
        diff < 60 -> "just now"
        diff < 3600 -> "${diff / 60}m ago"
        diff < 86400 -> "${diff / 3600}h ago"
        diff / 86400 == 1L -> "yesterday"
        else -> "${diff / 86400}d ago"
    }
}

// Suppressed import — DocumentsContract kept for future use when we want to
// display the SAF path under the folder name without touching DirectorySourcesScreen.
@Suppress("unused")
private val _documentsContractMarker: Class<*> = DocumentsContract::class.java
