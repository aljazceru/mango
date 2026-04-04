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
import androidx.compose.ui.text.style.TextOverflow
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
            if (step.actionType == "final_answer") {
                Column {
                    Text("Final Answer", style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
                    Text(step.resultSnippet ?: "", style = MaterialTheme.typography.bodyMedium)
                }
            } else {
                Column(modifier = Modifier.padding(vertical = 4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("Step ${step.stepNumber}", style = MaterialTheme.typography.labelSmall, fontWeight = FontWeight.Bold)
                        Spacer(modifier = Modifier.width(8.dp))
                        step.toolName?.let { Text(it, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary) }
                        Spacer(modifier = Modifier.weight(1f))
                        Text(step.status, style = MaterialTheme.typography.labelSmall,
                            color = when(step.status) { "completed" -> Color.Green; "failed" -> Color.Red; else -> Color(0xFFFFA500) })
                    }
                    step.toolInput?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 3, overflow = TextOverflow.Ellipsis)
                    }
                    step.resultSnippet?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 3, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}
