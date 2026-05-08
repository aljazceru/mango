package dev.disobey.mango.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
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
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState
import kotlinx.coroutines.launch

/**
 * Phase 36 — Tool Detail sub-screen.
 *
 * Renders a single contextvm-discovered tool's detail view (UI-SPEC §Layout
 * "Tool Detail sub-screen", §States N).
 *
 * Sections, top-to-bottom:
 *   1. Heading block: tool name (titleLarge), optional "Used N× — last used …"
 *      caption when usage_count > 0, then description body.
 *   2. ADVERTISED BY: provider display name (or "Unnamed provider"), npub row
 *      with Copy, Hex row with Copy.
 *   3. USAGE: "Never used" or "Used N times" + "Last used {relative}".
 *   4. SCHEMA expander: label + Show/Hide toggle, monospace pretty-printed JSON
 *      when expanded; "No schema published" when schema is empty.
 *   5. Tool ID: row with truncated id + Copy.
 *
 * Copy confirmation:
 *   - SnackbarHostState scoped to this screen's Scaffold.
 *   - Locked copy: "npub copied", "Pubkey copied", "Tool ID copied".
 *   - Failure variant: "Couldn't copy — try again".
 *
 * Threat model:
 *   - description / schema_pretty render as plain Text — no Markdown, no
 *     link auto-detect, no syntax highlighting (T-36-02-T1 mitigated).
 *   - Clipboard payloads (npub, pubkey hex, tool_id) are public identifiers
 *     already exposed in the announcement (T-36-02-I1 accepted).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsToolDetailScreen(
    appState: AppState,
    toolId: String,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit = { onDispatch(AppAction.PopScreen) },
) {
    val tool = appState.contextvmTools.firstOrNull { it.id == toolId }
    val snackbarHostState = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    Scaffold(
        topBar = {
            // Locked copy: "Tool details"
            TopAppBar(
                title = { Text("Tool details", fontWeight = FontWeight.Medium) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { padding ->
        if (tool == null) {
            // Edge case — should never happen via row tap because every row in
            // contextvmTools is reachable via its own id. Guard exists per
            // plan must-have "Detail screen gracefully handles the edge case".
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(32.dp),
                contentAlignment = Alignment.Center,
            ) {
                Text("Tool not found", style = MaterialTheme.typography.bodyMedium)
            }
            return@Scaffold
        }

        // Local copy helper. On clipboard write failure, surfaces locked
        // failure copy "Couldn't copy — try again" (UI-SPEC §States O failure).
        fun copy(label: String, text: String, snackText: String) {
            val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager
            if (cm != null) {
                cm.setPrimaryClip(ClipData.newPlainText(label, text))
                scope.launch { snackbarHostState.showSnackbar(snackText) }
            } else {
                scope.launch { snackbarHostState.showSnackbar("Couldn't copy — try again") }
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            // ── 1. Heading block ────────────────────────────────────────────
            Spacer(modifier = Modifier.heightIn(min = 8.dp))
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    tool.name,
                    style = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.Medium),
                )
                if (tool.usageCount > 0u && tool.lastUsedLabel != null) {
                    // Locked: "Used 1× — last used {relative}" / "Used {N}× — last used {relative}"
                    val sub = if (tool.usageCount == 1u) {
                        "Used 1× — last used ${tool.lastUsedLabel}"
                    } else {
                        "Used ${tool.usageCount}× — last used ${tool.lastUsedLabel}"
                    }
                    Text(
                        sub,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                if (tool.description.isNotBlank()) {
                    Text(tool.description, style = MaterialTheme.typography.bodyMedium)
                }
            }

            // ── 2. ADVERTISED BY ────────────────────────────────────────────
            Section(label = "ADVERTISED BY") {
                Text(
                    // Locked fallback when display name is null: "Unnamed provider"
                    tool.providerDisplayName ?: "Unnamed provider",
                    style = MaterialTheme.typography.bodyMedium,
                )
                CopyRow(
                    text = tool.npub,
                    prefix = null,
                    onClick = { copy("npub", tool.npub, "npub copied") },
                )
                CopyRow(
                    text = tool.providerPubkey.take(8) + "…",
                    prefix = "Hex:",
                    onClick = { copy("pubkey", tool.providerPubkey, "Pubkey copied") },
                )
            }

            // ── 3. USAGE ────────────────────────────────────────────────────
            Section(label = "USAGE") {
                if (tool.usageCount == 0u) {
                    // Locked: "Never used"
                    Text(
                        "Never used",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else {
                    // Locked: "Used 1 time" / "Used {N} times"
                    val timesLine = if (tool.usageCount == 1u) {
                        "Used 1 time"
                    } else {
                        "Used ${tool.usageCount} times"
                    }
                    Text(timesLine, style = MaterialTheme.typography.bodySmall)
                    if (tool.lastUsedLabel != null) {
                        // Locked: "Last used {relative}"
                        Text(
                            "Last used ${tool.lastUsedLabel}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            // ── 4. SCHEMA expander ─────────────────────────────────────────
            var schemaExpanded by remember { mutableStateOf(false) }
            Section(
                label = "SCHEMA",
                trailing = {
                    if (tool.schemaPretty.isNotBlank()) {
                        TextButton(onClick = { schemaExpanded = !schemaExpanded }) {
                            // Locked: "Show" (collapsed) / "Hide" (expanded)
                            Text(if (schemaExpanded) "Hide" else "Show")
                        }
                    }
                },
            ) {
                if (tool.schemaPretty.isBlank()) {
                    // Locked: "No schema published"
                    Text(
                        "No schema published",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                } else if (schemaExpanded) {
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        colors = CardDefaults.cardColors(
                            containerColor = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f),
                        ),
                    ) {
                        SelectionContainer {
                            Column(
                                modifier = Modifier
                                    .padding(12.dp)
                                    .heightIn(max = 320.dp)
                                    .verticalScroll(rememberScrollState()),
                            ) {
                                Text(
                                    tool.schemaPretty,
                                    style = MaterialTheme.typography.bodySmall.copy(
                                        fontFamily = FontFamily.Monospace,
                                    ),
                                )
                            }
                        }
                    }
                }
            }

            // ── 5. Tool ID row ─────────────────────────────────────────────
            CopyRow(
                text = tool.id.take(8) + "…",
                // Locked label prefix: "Tool ID:"
                prefix = "Tool ID:",
                onClick = { copy("tool_id", tool.id, "Tool ID copied") },
            )

            Spacer(modifier = Modifier.heightIn(min = 24.dp))
        }
    }
}

@Composable
private fun Section(
    label: String,
    trailing: @Composable () -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier
                    .weight(1f)
                    .padding(vertical = 4.dp),
            )
            trailing()
        }
        content()
    }
}

@Composable
private fun CopyRow(
    text: String,
    prefix: String?,
    onClick: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (prefix != null) {
            Text(
                "$prefix ",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            text,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            modifier = Modifier
                .weight(1f)
                .clickable(onClick = onClick)
                .padding(vertical = 4.dp),
        )
        IconButton(onClick = onClick) {
            // a11y label: locked "Copy"
            Icon(Icons.Outlined.ContentCopy, contentDescription = "Copy")
        }
    }
}
