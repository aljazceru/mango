package dev.disobey.mango.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import dev.disobey.mango.ui.theme.*
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.mango.rust.AgentSessionSummary
import dev.disobey.mango.rust.AgentStepSummary
import dev.disobey.mango.rust.AppAction
import dev.disobey.mango.rust.AppState

// Status color helper
private fun statusColor(status: String, isDark: Boolean): Color = when (status) {
    "running"              -> if (isDark) DarkAgentRunning else LightAgentRunning
    "paused"               -> if (isDark) DarkAgentPaused else LightAgentPaused
    "completed"            -> if (isDark) DarkAgentCompleted else LightAgentCompleted
    "failed", "cancelled"  -> if (isDark) DarkAgentFailed else LightAgentFailed
    else                   -> Color.Gray
}

private fun formatElapsed(secs: Long): String {
    return if (secs < 60) "${secs}s"
    else "${secs / 60}m ${secs % 60}s"
}

private fun actionTypeLabel(actionType: String): String = when (actionType) {
    "tool_call" -> "[Tool]"
    "final_answer" -> "[Answer]"
    "error" -> "[Error]"
    else -> "[Step]"
}

/**
 * Agent session list screen with task launch input.
 * If currentAgentSessionId is set, shows the step detail view instead.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentScreen(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit
) {
    val isDark = isSystemInDarkTheme()

    if (appState.currentAgentSessionId != null) {
        AgentDetailSection(appState = appState, onDispatch = onDispatch, onBack = onBack, isDark = isDark)
        return
    }

    var taskInput by remember { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Agent Sessions") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            // Task launch input bar
            Surface(
                tonalElevation = 2.dp,
                modifier = Modifier.fillMaxWidth()
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    OutlinedTextField(
                        value = taskInput,
                        onValueChange = { taskInput = it },
                        placeholder = { Text("Describe a task for the agent...") },
                        modifier = Modifier.weight(1f),
                        singleLine = true,
                        textStyle = LocalTextStyle.current.copy(fontSize = 14.sp)
                    )
                    Button(
                        onClick = {
                            val description = taskInput.trim()
                            if (description.isNotEmpty()) {
                                onDispatch(AppAction.LaunchAgentSession(taskDescription = description))
                                taskInput = ""
                            }
                        },
                        enabled = taskInput.trim().isNotEmpty()
                    ) {
                        Text("Launch")
                    }
                }
            }

            // Session list
            if (appState.agentSessions.isEmpty()) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Text(
                            "No agent sessions yet.",
                            style = MaterialTheme.typography.titleMedium
                        )
                        Text(
                            "Launch an agent above to get started.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    items(appState.agentSessions) { session ->
                        AgentSessionItem(
                            session = session,
                            isDark = isDark,
                            onClick = {
                                onDispatch(AppAction.LoadAgentSession(sessionId = session.id))
                            }
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AgentSessionItem(
    session: AgentSessionSummary,
    isDark: Boolean,
    onClick: () -> Unit
) {
    val color = statusColor(session.status, isDark)

    Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Text(
                text = session.title,
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                // Status chip
                Surface(
                    color = color.copy(alpha = 0.15f),
                    shape = MaterialTheme.shapes.small
                ) {
                    Text(
                        text = session.status,
                        color = color,
                        fontSize = 11.sp,
                        modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                    )
                }
                Text(
                    text = "${session.stepCount} steps",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = formatElapsed(session.elapsedSecs),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

/**
 * Agent session detail view: shows step list and pause/resume/cancel controls.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AgentDetailSection(
    appState: AppState,
    onDispatch: (AppAction) -> Unit,
    onBack: () -> Unit,
    isDark: Boolean = isSystemInDarkTheme()
) {
    val sessionId = appState.currentAgentSessionId ?: return
    val session = appState.agentSessions.find { it.id == sessionId }
    val statusCol = statusColor(session?.status ?: "", isDark)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(session?.title ?: "Session Detail") },
                navigationIcon = {
                    // Uses ClearAgentDetail action (not empty-string LoadAgentSession)
                    IconButton(onClick = { onDispatch(AppAction.ClearAgentDetail) }) {
                        Icon(Icons.Default.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    if (session != null) {
                        Surface(
                            color = statusCol.copy(alpha = 0.15f),
                            shape = MaterialTheme.shapes.small
                        ) {
                            Text(
                                text = session.status,
                                color = statusCol,
                                fontSize = 11.sp,
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
                            )
                        }
                    }
                }
            )
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
        ) {
            // Action buttons row
            if (session != null &&
                (session.status == "running" || session.status == "paused")
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    if (session.status == "running") {
                        OutlinedButton(
                            onClick = {
                                onDispatch(AppAction.PauseAgentSession(sessionId = sessionId))
                            }
                        ) {
                            Text("Pause", color = if (isDark) DarkAgentPaused else LightAgentPaused)
                        }
                    }
                    if (session.status == "paused") {
                        OutlinedButton(
                            onClick = {
                                onDispatch(AppAction.ResumeAgentSession(sessionId = sessionId))
                            }
                        ) {
                            Text("Resume", color = if (isDark) DarkAgentRunning else LightAgentRunning)
                        }
                    }
                    OutlinedButton(
                        onClick = {
                            onDispatch(AppAction.CancelAgentSession(sessionId = sessionId))
                        }
                    ) {
                        Text("Cancel", color = if (isDark) DarkAgentFailed else LightAgentFailed)
                    }
                }
                Divider()
            }

            // Step list
            val steps = appState.currentAgentSteps.sortedBy { it.stepNumber }
            if (steps.isEmpty()) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Text(
                            "No steps yet.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Text(
                            "Steps will appear here as the agent works.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            } else {
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    items(steps) { step ->
                        AgentStepItem(step = step)
                    }
                }
            }
        }
    }
}

@Composable
private fun AgentStepItem(step: AgentStepSummary) {
    val isDark = isSystemInDarkTheme()
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(10.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                // Step number badge
                Surface(
                    color = MaterialTheme.colorScheme.surfaceVariant,
                    shape = MaterialTheme.shapes.small
                ) {
                    Text(
                        text = "#${step.stepNumber}",
                        fontSize = 10.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp)
                    )
                }
                // Action type badge
                Surface(
                    color = (if (isDark) DarkAgentCompleted else LightAgentCompleted).copy(alpha = 0.1f),
                    shape = MaterialTheme.shapes.small
                ) {
                    Text(
                        text = actionTypeLabel(step.actionType),
                        fontSize = 10.sp,
                        color = if (isDark) DarkAgentCompleted else LightAgentCompleted,
                        modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp)
                    )
                }
                // Tool name
                step.toolName?.let { toolName ->
                    Text(
                        text = toolName,
                        style = MaterialTheme.typography.bodySmall,
                        fontWeight = FontWeight.Medium
                    )
                }
                Spacer(modifier = Modifier.weight(1f))
                if (step.status == "failed") {
                    Text(
                        text = "FAILED",
                        fontSize = 10.sp,
                        color = if (isDark) DarkAgentFailed else LightAgentFailed
                    )
                }
            }
            step.resultSnippet?.let { snippet ->
                if (snippet.isNotEmpty()) {
                    Text(
                        text = snippet,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2
                    )
                }
            }
        }
    }
}
