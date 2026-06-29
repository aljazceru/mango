package dev.disobey.mango.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.DiscoverableTool
import dev.disobey.mango.rust.TrustedProvider
import kotlinx.coroutines.launch

/** Resolved display name for a provider pubkey, preferring Nostr profile name. */
private fun providerDisplayName(
    pubkey: String,
    tools: List<DiscoverableTool>,
    trusted: TrustedProvider? = null,
): String {
    val fromTools = tools.firstOrNull { it.providerPubkey == pubkey }
    return fromTools?.providerName
        ?: fromTools?.providerDisplayName
        ?: trusted?.label
        ?: ""
}

/**
 * Phase 38 — Trusted Providers management screen.
 *
 * Shows two sections:
 * 1. "Trusted" — providers the user has explicitly trusted, showing profile
 *    name resolved from the tools cache. Tap row → provider detail sheet.
 * 2. "Discovered" — providers found via Nostr discovery not yet trusted.
 *    Search box filters by name. Tap row body → detail sheet. Trust button
 *    moves provider to Trusted.
 *
 * Provider detail sheet: name, npub (copyable), about, list of tools.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrustedProvidersScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    var showAddDialog by remember { mutableStateOf(false) }
    var confirmRemovePubkey by remember { mutableStateOf<String?>(null) }
    // pubkey of the provider whose detail sheet is open, null = closed
    var detailPubkey by remember { mutableStateOf<String?>(null) }
    var searchQuery by remember { mutableStateOf("") }

    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    val trustedPubkeys = appState.trustedProviders.map { it.pubkey }.toSet()
    // One representative DiscoverableTool per discovered (not-trusted) provider.
    val allDiscoveredProviders = appState.contextvmTools
        .filter { it.providerPubkey !in trustedPubkeys }
        .distinctBy { it.providerPubkey }
        .sortedBy { it.providerName ?: it.providerDisplayName ?: it.npub }
    val q = searchQuery.trim().lowercase()
    val filteredDiscoveredProviders = if (q.isEmpty()) {
        allDiscoveredProviders
    } else {
        allDiscoveredProviders.filter { tool ->
            val name = (tool.providerName ?: tool.providerDisplayName ?: "").lowercase()
            val about = (tool.providerAbout ?: "").lowercase()
            val npub = tool.npub.lowercase()
            val hex = tool.providerPubkey.lowercase()
            name.contains(q) || about.contains(q) || npub.contains(q) || hex.contains(q)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Trusted Providers", fontWeight = FontWeight.Medium) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { showAddDialog = true }) {
                        Icon(Icons.Filled.Add, contentDescription = "Add provider manually")
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            item {
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Only tools from trusted providers are offered to the assistant " +
                        "when auto-discovery is on.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(16.dp))
            }

            // ── Trusted section ──────────────────────────────────────────────
            item { SettingsSectionLabel("Trusted") }

            if (appState.trustedProviders.isEmpty()) {
                item {
                    Text(
                        text = "No trusted providers yet.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(start = 4.dp, bottom = 4.dp),
                    )
                }
            } else {
                items(appState.trustedProviders, key = { "trusted:${it.pubkey}" }) { provider ->
                    val resolvedName = providerDisplayName(
                        provider.pubkey, appState.contextvmTools, provider
                    ).ifBlank { null }
                    TrustedProviderRow(
                        name = resolvedName,
                        npub = provider.npub,
                        onTap = { detailPubkey = provider.pubkey },
                        onRemove = { confirmRemovePubkey = provider.pubkey },
                    )
                }
            }

            // ── Discovered section ───────────────────────────────────────────
            if (allDiscoveredProviders.isNotEmpty()) {
                item {
                    Spacer(Modifier.height(8.dp))
                    SettingsSectionLabel("Discovered")
                    Spacer(Modifier.height(4.dp))
                    OutlinedTextField(
                        value = searchQuery,
                        onValueChange = { searchQuery = it },
                        placeholder = { Text("Search providers") },
                        leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }

                if (filteredDiscoveredProviders.isEmpty()) {
                    item {
                        Text(
                            text = "No providers match \"$searchQuery\".",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(start = 4.dp, top = 4.dp),
                        )
                    }
                } else {
                    items(filteredDiscoveredProviders, key = { "disc:${it.providerPubkey}" }) { tool ->
                        val name = tool.providerName
                            ?: tool.providerDisplayName
                            ?: if (tool.npub.length > 12) "${tool.npub.take(12)}…" else tool.npub
                        val toolCount = appState.contextvmTools.count { it.providerPubkey == tool.providerPubkey }
                        DiscoveredProviderRow(
                            name = name,
                            toolCount = toolCount,
                            onTap = { detailPubkey = tool.providerPubkey },
                            onTrust = {
                                onDispatch(
                                    AppAction.AddTrustedProvider(
                                        pubkey = tool.providerPubkey,
                                        label = null,
                                    )
                                )
                            },
                        )
                    }
                }
            }

            item { Spacer(Modifier.height(32.dp)) }
        }
    }

    // ── Provider detail bottom sheet ─────────────────────────────────────────
    detailPubkey?.let { pubkey ->
        val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
        val isTrusted = pubkey in trustedPubkeys
        // Pick the representative tool for provider metadata.
        val repTool = appState.contextvmTools.firstOrNull { it.providerPubkey == pubkey }
        val providerTools = appState.contextvmTools.filter { it.providerPubkey == pubkey }
        val name = repTool?.providerName ?: repTool?.providerDisplayName
            ?: appState.trustedProviders.find { it.pubkey == pubkey }?.label
            ?: pubkey.take(16)
        val npub = repTool?.npub
            ?: appState.trustedProviders.find { it.pubkey == pubkey }?.npub
            ?: pubkey

        fun copyToClipboard(label: String, text: String, snackText: String) {
            val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            if (cm != null) {
                cm.setPrimaryClip(ClipData.newPlainText(label, text))
                scope.launch { snackbarHostState.showSnackbar(snackText) }
            }
        }

        ModalBottomSheet(
            onDismissRequest = { detailPubkey = null },
            sheetState = sheetState,
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(max = 600.dp)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 20.dp)
                    .padding(bottom = 32.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                // Name + trust/remove action
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = name,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.weight(1f),
                    )
                    if (isTrusted) {
                        TextButton(
                            onClick = {
                                onDispatch(AppAction.RemoveTrustedProvider(pubkey = pubkey))
                                detailPubkey = null
                            },
                        ) {
                            Text("Remove", color = MaterialTheme.colorScheme.error)
                        }
                    } else {
                        OutlinedButton(
                            onClick = {
                                onDispatch(AppAction.AddTrustedProvider(pubkey = pubkey, label = null))
                                detailPubkey = null
                            },
                        ) {
                            Text("Trust")
                        }
                    }
                }

                // About
                val about = repTool?.providerAbout
                if (!about.isNullOrBlank()) {
                    Text(
                        text = about,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                // npub row
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = npub,
                        style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    IconButton(onClick = { copyToClipboard("npub", npub, "npub copied") }) {
                        Icon(Icons.Outlined.ContentCopy, contentDescription = "Copy npub")
                    }
                }

                HorizontalDivider()

                // Tools list
                Text(
                    text = "TOOLS",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (providerTools.isEmpty()) {
                    Text(
                        text = "No tools cached. Open Discover Tools to refresh.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else {
                    providerTools.forEach { tool ->
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 4.dp),
                            verticalArrangement = Arrangement.spacedBy(2.dp),
                        ) {
                            Text(
                                text = tool.name,
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = FontWeight.Medium,
                            )
                            if (tool.description.isNotBlank()) {
                                Text(
                                    text = tool.description,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 2,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Add provider manually dialog ─────────────────────────────────────────
    if (showAddDialog) {
        AddTrustedProviderDialog(
            onConfirm = { pubkey, label ->
                onDispatch(AppAction.AddTrustedProvider(pubkey = pubkey.trim(), label = label?.trim()?.ifEmpty { null }))
                showAddDialog = false
            },
            onDismiss = { showAddDialog = false },
        )
    }

    // ── Remove confirmation dialog ───────────────────────────────────────────
    confirmRemovePubkey?.let { pubkey ->
        val provider = appState.trustedProviders.find { it.pubkey == pubkey }
        val resolved = providerDisplayName(pubkey, appState.contextvmTools, provider).ifBlank { null }
        val displayName = resolved ?: provider?.npub?.take(20)?.let { "$it…" } ?: pubkey.take(16)
        AlertDialog(
            onDismissRequest = { confirmRemovePubkey = null },
            title = { Text("Remove trusted provider?") },
            text = { Text("\"$displayName\" will no longer be trusted. Tools from this provider won't be auto-discovered.") },
            confirmButton = {
                TextButton(
                    onClick = {
                        onDispatch(AppAction.RemoveTrustedProvider(pubkey = pubkey))
                        confirmRemovePubkey = null
                    },
                ) {
                    Text("Remove", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmRemovePubkey = null }) { Text("Cancel") }
            },
        )
    }
}

@Composable
private fun TrustedProviderRow(
    name: String?,
    npub: String,
    onTap: () -> Unit,
    onRemove: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onTap)
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                if (!name.isNullOrBlank()) {
                    Text(
                        text = name,
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Spacer(Modifier.height(2.dp))
                }
                Text(
                    text = npub,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Spacer(Modifier.width(8.dp))
            IconButton(onClick = onRemove) {
                Icon(
                    Icons.Filled.Delete,
                    contentDescription = "Remove",
                    tint = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

@Composable
private fun DiscoveredProviderRow(
    name: String,
    toolCount: Int,
    onTap: () -> Unit,
    onTrust: () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(onClick = onTap)
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = name,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(2.dp))
                Text(
                    text = when (toolCount) {
                        1 -> "1 tool"
                        else -> "$toolCount tools"
                    },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(Modifier.width(8.dp))
            OutlinedButton(onClick = onTrust) {
                Text("Trust")
            }
        }
    }
}

@Composable
private fun AddTrustedProviderDialog(
    onConfirm: (pubkey: String, label: String?) -> Unit,
    onDismiss: () -> Unit,
) {
    var pubkey by remember { mutableStateOf("") }
    var label by remember { mutableStateOf("") }
    val isValid = pubkey.trim().length == 64 && pubkey.trim().all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Add trusted provider") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = "Enter the provider's Nostr hex pubkey (64 hex characters).",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                OutlinedTextField(
                    value = pubkey,
                    onValueChange = { input ->
                        pubkey = input
                            .asSequence()
                            .map { it.lowercaseChar() }
                            .filter { it in '0'..'9' || it in 'a'..'f' }
                            .take(64)
                            .joinToString("")
                    },
                    label = { Text("Hex pubkey") },
                    placeholder = { Text("0000...") },
                    singleLine = true,
                    isError = pubkey.isNotEmpty() && !isValid,
                    supportingText = if (pubkey.isNotEmpty() && !isValid) {
                        { Text("Must be a 64-character hex string") }
                    } else null,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = label,
                    onValueChange = { label = it },
                    label = { Text("Label (optional)") },
                    placeholder = { Text("e.g. Tinfoil weather service") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(
                        onDone = { if (isValid) onConfirm(pubkey, label.ifEmpty { null }) },
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            Button(
                onClick = { onConfirm(pubkey, label.ifEmpty { null }) },
                enabled = isValid,
            ) {
                Text("Add")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}
