package dev.disobey.mango.ui

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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.outlined.SearchOff
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import dev.disobey.mango.rust.ContextvmDiscoveryState
import dev.disobey.mango.rust.DiscoverableTool

/**
 * Phase 35 — Tool Discovery sub-screen.
 *
 * Renders all 5 UI-SPEC states (Idle, Loading, Empty, Error, Loaded) per
 * 35-UI-SPEC §C–§G. All copy strings are locked verbatim from UI-SPEC.
 *
 * Threat model:
 * - Untrusted tool descriptions (sourced from Nostr announcements) are
 *   length-capped at 500 chars by the Rust core before reaching this UI.
 *   They render as plain `Text` (no Markdown) to avoid injection-rendering
 *   surfaces.
 * - "Try again" / Refresh actions dispatch `RetryContextvmDiscovery`; the
 *   Rust core dedups concurrent discovery requests so spam-tapping is a
 *   no-op while a query is in flight.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsToolDiscoveryScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    // Auto-fire discovery on first composition (UI-SPEC §C "pull on open").
    LaunchedEffect(Unit) {
        onDispatch(AppAction.DiscoverContextvmTools)
    }
    val isLoading = appState.contextvmDiscoveryState is ContextvmDiscoveryState.Loading

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
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            contentAlignment = Alignment.Center,
        ) {
            when (appState.contextvmDiscoveryState) {
                is ContextvmDiscoveryState.Idle,
                is ContextvmDiscoveryState.Loading -> LoadingState()
                is ContextvmDiscoveryState.Error -> ErrorState(
                    onRetry = { onDispatch(AppAction.RetryContextvmDiscovery) },
                )
                is ContextvmDiscoveryState.Loaded -> {
                    if (appState.contextvmTools.isEmpty()) {
                        EmptyState(onRetry = { onDispatch(AppAction.RetryContextvmDiscovery) })
                    } else {
                        ToolList(
                            tools = appState.contextvmTools,
                            onToggle = { tool, enabled ->
                                onDispatch(
                                    AppAction.SetContextvmToolEnabled(
                                        toolId = tool.id,
                                        enabled = enabled,
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

@Composable
private fun ToolList(
    tools: List<DiscoverableTool>,
    onToggle: (DiscoverableTool, Boolean) -> Unit,
) {
    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
        contentPadding = PaddingValues(vertical = 8.dp),
    ) {
        items(tools, key = { it.id }) { tool ->
            ToolRow(tool = tool, onToggle = onToggle)
        }
    }
}

@Composable
private fun ToolRow(
    tool: DiscoverableTool,
    onToggle: (DiscoverableTool, Boolean) -> Unit,
) {
    val providerLabel =
        tool.providerDisplayName ?: (tool.providerPubkey.take(8) + "…")
    Card(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    tool.name,
                    style = MaterialTheme.typography.bodyMedium,
                    fontWeight = FontWeight.Medium,
                )
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
            Spacer(Modifier.width(16.dp))
            Switch(
                checked = tool.enabled,
                onCheckedChange = { checked -> onToggle(tool, checked) },
            )
        }
    }
}
