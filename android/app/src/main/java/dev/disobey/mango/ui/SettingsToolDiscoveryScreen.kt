package dev.disobey.mango.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.Text
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.SearchOff
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.ContextvmDiscoveryState
import dev.disobey.mango.rust.DiscoverableTool
import dev.disobey.mango.rust.Screen

/**
 * Phase 35 + Phase 36 — Tool Discovery sub-screen.
 *
 * Phase 35 baseline: 5 UI-SPEC states (Idle, Loading, Empty, Error, Loaded)
 * with locked copy strings.
 *
 * Phase 36 additions (UI-SPEC §Layout / §States J–M):
 * - Always-visible search field below the TopAppBar (placeholder "Search tools").
 * - Cache-first render: while a refresh is in flight, the cached list is
 *   shown immediately; the spinner only renders when the cache is empty.
 * - "Used N×" muted pill on rows where `tool.usageCount > 0`.
 * - Trailing chevron on every row.
 * - Whole-row tap (excluding the Switch) navigates to the new
 *   `Screen.ContextvmToolDetail(toolId)` detail screen.
 *
 * Threat model:
 * - Untrusted tool descriptions (sourced from Nostr announcements) are
 *   length-capped at 500 chars by the Rust core before reaching this UI.
 *   They render as plain `Text` (no Markdown) to avoid injection-rendering
 *   surfaces.
 * - "Try again" / Refresh actions dispatch `RetryContextvmDiscovery`; the
 *   Rust core dedups concurrent discovery requests so spam-tapping is a
 *   no-op while a query is in flight.
 * - Search filter is a pure in-memory `String.contains` over the
 *   already-loaded `AppState.contextvmTools` vector — no DB query, no
 *   substitution surface for SQL meta-chars (T-36-02-V1).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsToolDiscoveryScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    // Phase 36: search query state, hoisted at the top so it survives
    // recomposition across discovery state transitions.
    var query by remember { mutableStateOf("") }

    // Provider filter state (null = all providers)
    var selectedProvider by remember { mutableStateOf<String?>(null) }
    var providerMenuExpanded by remember { mutableStateOf(false) }

    // Auto-fire discovery on first composition (UI-SPEC §C "pull on open").
    // The Rust core preserves the cached `contextvm_tools` list across the
    // Loading transition, so the UI can render cached rows immediately.
    LaunchedEffect(Unit) {
        onDispatch(AppAction.DiscoverContextvmTools)
    }

    val isLoading = appState.contextvmDiscoveryState is ContextvmDiscoveryState.Loading

    // Phase 36: live filter, no debounce. Cardinality is bounded by Nostr
    // discovery (tens, not thousands) — O(N) substring scan per keystroke
    // is trivially fast (T-36-02-D1 accepted).
    val filteredTools by remember(appState.contextvmTools, query, selectedProvider) {
        derivedStateOf {
            val q = query.trim().lowercase()
            appState.contextvmTools.filter { tool ->
                // Text search filter
                val textMatch = if (q.isEmpty()) {
                    true
                } else {
                    tool.name.lowercase().contains(q) ||
                        tool.description.lowercase().contains(q) ||
                        (tool.providerDisplayName ?: "").lowercase().contains(q) ||
                        (tool.providerName ?: "").lowercase().contains(q)
                }

                // Provider filter (Phase 37: filter by provider pubkey)
                val providerMatch = if (selectedProvider == null) {
                    true
                } else {
                    tool.providerPubkey == selectedProvider
                }

                textMatch && providerMatch
            }
        }
    }

    // Extract unique providers from the tool list (Phase 37: use provider_name from Nostr profile)
    val uniqueProviders by remember(appState.contextvmTools) {
        derivedStateOf {
            appState.contextvmTools
                .map { tool ->
                    // Use provider_name (from Nostr profile) if available, otherwise fall back to
                    // provider_display_name, and finally to npub if neither is available
                    val displayName = tool.providerName
                        ?: tool.providerDisplayName
                        ?: if (tool.npub.length > 8) "${tool.npub.substring(0, 8)}…" else tool.npub
                    Pair(displayName, tool.providerPubkey)
                }
                .distinctBy { it.second } // Dedup by pubkey
                .sortedBy { it.first }
        }
    }

    // Get the display name for the selected provider
    val selectedProviderDisplayName = selectedProvider?.let { pubkey ->
        uniqueProviders.find { it.second == pubkey }?.first
    }

    // Phase 37: group filtered tools by provider for display
    val groupedTools by remember(filteredTools) {
        derivedStateOf {
            filteredTools
                .groupBy { it.providerPubkey }
                .toSortedMap { pk1, pk2 ->
                    val name1 = filteredTools.find { it.providerPubkey == pk1 }?.let { tool ->
                        tool.providerName ?: tool.providerDisplayName ?: tool.npub
                    } ?: pk1
                    val name2 = filteredTools.find { it.providerPubkey == pk2 }?.let { tool ->
                        tool.providerName ?: tool.providerDisplayName ?: tool.npub
                    } ?: pk2
                    name1.compareTo(name2)
                }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Discover Tools", fontWeight = FontWeight.Medium) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(
                        onClick = { onDispatch(AppAction.RetryContextvmDiscovery) },
                        enabled = !isLoading,
                    ) {
                        Icon(Icons.Filled.Refresh, contentDescription = "Refresh")
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            // Phase 36 §L — search field rendered in EVERY discovery state.
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                placeholder = { Text("Search tools") },
                leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null) },
                singleLine = true,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            )

            // Provider filter dropdown (Phase 37: use provider_name from Nostr profile)
            ExposedDropdownMenuBox(
                expanded = providerMenuExpanded,
                onExpandedChange = { providerMenuExpanded = it },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 4.dp),
            ) {
                OutlinedTextField(
                    value = selectedProviderDisplayName ?: "All Providers",
                    onValueChange = {},
                    readOnly = true,
                    label = { Text("Filter by provider") },
                    trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = providerMenuExpanded) },
                    modifier = Modifier
                        .menuAnchor()
                        .fillMaxWidth(),
                    colors = OutlinedTextFieldDefaults.colors(
                        focusedContainerColor = MaterialTheme.colorScheme.surface,
                        unfocusedContainerColor = MaterialTheme.colorScheme.surface,
                    ),
                )
                ExposedDropdownMenu(
                    expanded = providerMenuExpanded,
                    onDismissRequest = { providerMenuExpanded = false },
                ) {
                    // "All Providers" option
                    DropdownMenuItem(
                        text = { Text("All Providers") },
                        onClick = {
                            selectedProvider = null
                            providerMenuExpanded = false
                        },
                    )
                    // Provider options
                    uniqueProviders.forEach { (displayName, pubkey) ->
                        DropdownMenuItem(
                            text = { Text(displayName) },
                            onClick = {
                                selectedProvider = pubkey
                                providerMenuExpanded = false
                            },
                        )
                    }
                }
            }
            Box(
                modifier = Modifier.fillMaxSize(),
                contentAlignment = Alignment.Center,
            ) {
                when (appState.contextvmDiscoveryState) {
                    is ContextvmDiscoveryState.Idle,
                    is ContextvmDiscoveryState.Loading -> {
                        // Phase 36 cache-first: render cached rows during in-flight
                        // refresh. Only show the spinner when the cache is empty.
                        if (appState.contextvmTools.isEmpty()) {
                            LoadingState()
                        } else {
                            ToolListGroupedOrEmptySearch(
                                groupedTools = groupedTools,
                                query = query,
                                onToggle = { tool, enabled ->
                                    onDispatch(
                                        AppAction.SetContextvmToolEnabled(
                                            toolId = tool.id,
                                            enabled = enabled,
                                        )
                                    )
                                },
                                onRowTap = { tool ->
                                    onDispatch(
                                        AppAction.PushScreen(
                                            screen = Screen.ContextvmToolDetail(toolId = tool.id),
                                        )
                                    )
                                },
                            )
                        }
                    }
                    is ContextvmDiscoveryState.Error -> ErrorState(
                        onRetry = { onDispatch(AppAction.RetryContextvmDiscovery) },
                    )
                    is ContextvmDiscoveryState.Loaded -> {
                        if (appState.contextvmTools.isEmpty()) {
                            EmptyState(onRetry = { onDispatch(AppAction.RetryContextvmDiscovery) })
                        } else {
                            ToolListGroupedOrEmptySearch(
                                groupedTools = groupedTools,
                                query = query,
                                onToggle = { tool, enabled ->
                                    onDispatch(
                                        AppAction.SetContextvmToolEnabled(
                                            toolId = tool.id,
                                            enabled = enabled,
                                        )
                                    )
                                },
                                onRowTap = { tool ->
                                    onDispatch(
                                        AppAction.PushScreen(
                                            screen = Screen.ContextvmToolDetail(toolId = tool.id),
                                        )
                                    )
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LoadingState() {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        CircularProgressIndicator(strokeWidth = 2.dp, modifier = Modifier.size(24.dp))
        Text(
            "Searching Nostr relays…",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun EmptyState(onRetry: () -> Unit) {
    Column(
        modifier = Modifier.padding(48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(
            Icons.Outlined.SearchOff,
            contentDescription = null,
            modifier = Modifier.size(48.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text("No tools found", style = MaterialTheme.typography.bodyLarge)
        Text(
            "Tools advertised on Nostr will appear here.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Button(onClick = onRetry) { Text("Try again") }
    }
}

@Composable
private fun ErrorState(onRetry: () -> Unit) {
    Column(
        modifier = Modifier.padding(48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            "Couldn't reach relays",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.error,
        )
        Text(
            "Check your connection and try again.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Button(onClick = onRetry) { Text("Try again") }
    }
}

/**
 * Phase 37 — renders either the grouped tool list with provider headers or the centered
 * "No tools match \"{query}\"" caption when a non-empty query yields no results.
 */
@Composable
private fun ToolListGroupedOrEmptySearch(
    groupedTools: Map<String, List<DiscoverableTool>>,
    query: String,
    onToggle: (DiscoverableTool, Boolean) -> Unit,
    onRowTap: (DiscoverableTool) -> Unit,
) {
    val totalTools = groupedTools.values.sumOf { it.size }

    if (totalTools == 0 && query.trim().isNotEmpty()) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(32.dp),
            contentAlignment = Alignment.Center,
        ) {
            // Locked copy per UI-SPEC §States M.
            // Straight ASCII quotes (not curly) — UI-SPEC §Copywriting Contract.
            Text(
                "No tools match \"${query}\"",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    } else {
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(0.dp),
            contentPadding = PaddingValues(vertical = 8.dp),
        ) {
            groupedTools.forEach { (providerPubkey, tools) ->
                // Get provider name for header
                val providerName = tools.firstOrNull()?.let { tool ->
                    tool.providerName ?: tool.providerDisplayName ?: if (tool.npub.length > 8) "${tool.npub.substring(0, 8)}…" else tool.npub
                } ?: providerPubkey

                // Provider header
                item {
                    Text(
                        text = providerName,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 8.dp)
                    )
                }

                // Tool rows for this provider
                items(tools, key = { it.id }) { tool ->
                    ToolRow(tool = tool, onToggle = onToggle, onRowTap = onRowTap)
                }
            }
        }
    }
}

/**
 * Phase 36 §M — renders either the filtered tool list or the centered
 * "No tools match \"{query}\"" caption when a non-empty query yields no
 * results.
 */
@Composable
private fun ToolListOrEmptySearch(
    tools: List<DiscoverableTool>,
    query: String,
    onToggle: (DiscoverableTool, Boolean) -> Unit,
    onRowTap: (DiscoverableTool) -> Unit,
) {
    if (tools.isEmpty() && query.trim().isNotEmpty()) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(32.dp),
            contentAlignment = Alignment.Center,
        ) {
            // Locked copy per UI-SPEC §States M.
            // Straight ASCII quotes (not curly) — UI-SPEC §Copywriting Contract.
            Text(
                "No tools match \"${query}\"",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    } else {
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
            contentPadding = PaddingValues(vertical = 8.dp),
        ) {
            items(tools, key = { it.id }) { tool ->
                ToolRow(tool = tool, onToggle = onToggle, onRowTap = onRowTap)
            }
        }
    }
}

@Composable
private fun ToolRow(
    tool: DiscoverableTool,
    onToggle: (DiscoverableTool, Boolean) -> Unit,
    onRowTap: (DiscoverableTool) -> Unit,
) {
    val providerLabel =
        tool.providerDisplayName ?: (tool.providerPubkey.take(8) + "…")
    Card(
        modifier = Modifier
            .fillMaxWidth()
            // Phase 36: whole-row tap navigates to the detail screen.
            // The Switch absorbs its own click via Compose default — toggling
            // enabled does NOT navigate.
            .clickable { onRowTap(tool) },
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 14.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Text(
                    tool.name,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (tool.usageCount > 0u) {
                    UsedBadge(tool.usageCount)
                }
                // Phase 36 §K — trailing chevron on every row.
                Icon(
                    Icons.AutoMirrored.Filled.KeyboardArrowRight,
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Switch(
                    checked = tool.enabled,
                    onCheckedChange = { checked -> onToggle(tool, checked) },
                )
            }
            Spacer(Modifier.size(2.dp))
            Text(
                providerLabel,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (tool.description.isNotBlank()) {
                Text(
                    tool.description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

/**
 * Phase 36 §J — "Used N×" muted pill. Singular "Used 1×", plural
 * "Used {N}×" with U+00D7 multiplication sign. Visual matches the
 * Phase 35 `Remote` provenance badge.
 */
@Composable
private fun UsedBadge(count: UInt) {
    val label = if (count == 1u) "Used 1×" else "Used ${count}×"
    Surface(
        shape = RoundedCornerShape(8.dp),
        color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
        )
    }
}
