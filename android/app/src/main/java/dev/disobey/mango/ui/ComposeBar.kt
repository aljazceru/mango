package dev.disobey.mango.ui

import android.widget.Toast
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AttachmentInfo

/// Compose bar pinned to the bottom of the chat screen.
/// Includes attachment indicator, text input, attach button, send/stop.
/// Handles IME padding locally so the chat header and message list do not shift under the status bar.
@Composable
fun ComposeBar(
    pendingAttachment: AttachmentInfo?,
    isStreaming: Boolean,
    isInputBlocked: Boolean = isStreaming,
    showStopButton: Boolean = isStreaming,
    onSend: (String) -> Unit,
    onStop: () -> Unit,
    onAttach: () -> Unit,
    onClearAttachment: () -> Unit,
    modifier: Modifier = Modifier,
    routingLabel: String? = null,
    routingDetail: String? = null,
    prefillText: String? = null,
    onPrefillConsumed: () -> Unit = {},
) {
    var inputText by remember { mutableStateOf("") }
    val context = LocalContext.current
    val haptics = LocalHapticFeedback.current
    val isResponseActive = isInputBlocked

    LaunchedEffect(prefillText) {
        val prompt = prefillText
        if (!prompt.isNullOrBlank()) {
            inputText = prompt
            onPrefillConsumed()
        }
    }

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .imePadding()
            .navigationBarsPadding(),
        tonalElevation = 2.dp,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
        ) {
            if (!routingLabel.isNullOrBlank()) {
                Surface(
                    modifier = Modifier
                        .padding(bottom = 6.dp)
                        .clickable(enabled = !routingDetail.isNullOrBlank()) {
                            routingDetail?.let {
                                Toast.makeText(context, it, Toast.LENGTH_SHORT).show()
                            }
                        },
                    shape = MaterialTheme.shapes.small,
                    color = MaterialTheme.colorScheme.secondaryContainer,
                    contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                ) {
                    Text(
                        routingLabel,
                        style = MaterialTheme.typography.labelMedium,
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                    )
                }
            }

            // Pending attachment indicator
            if (pendingAttachment != null) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        if (pendingAttachment.isImage) Icons.Default.Image else Icons.Default.AttachFile,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Text(
                        "${pendingAttachment.filename} (${pendingAttachment.sizeDisplay})",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                        maxLines = 1,
                    )
                    IconButton(
                        onClick = onClearAttachment,
                        enabled = canMutateComposerAttachment(isResponseActive),
                        modifier = Modifier.size(24.dp),
                    ) {
                        Icon(
                            Icons.Default.Close,
                            contentDescription = "Remove attachment",
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }
            }

            // Input row
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                // Attach button
                IconButton(
                    onClick = onAttach,
                    enabled = canMutateComposerAttachment(isResponseActive),
                    modifier = Modifier.size(44.dp),
                ) {
                    Icon(
                        Icons.Default.AttachFile,
                        contentDescription = "Attach file for context",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                // Text input
                OutlinedTextField(
                    value = inputText,
                    onValueChange = { inputText = it },
                    placeholder = { Text("Message") },
                    modifier = Modifier.weight(1f),
                    minLines = 1,
                    maxLines = 6,
                    enabled = canEditComposerText(),
                )

                // Send / Stop button
                if (showStopButton) {
                    IconButton(
                        onClick = {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                            onStop()
                        },
                        modifier = Modifier.size(44.dp),
                    ) {
                        Icon(
                            Icons.Default.Stop,
                            contentDescription = "Stop generating",
                            tint = MaterialTheme.colorScheme.error,
                        )
                    }
                } else {
                    val canSend = canSendComposerText(inputText, isResponseActive)
                    Surface(
                        modifier = Modifier
                            .size(44.dp)
                            .clickable(enabled = canSend) {
                                val trimmed = inputText.trim()
                                if (trimmed.isNotEmpty()) {
                                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                    onSend(trimmed)
                                    inputText = ""
                                }
                            },
                        shape = CircleShape,
                        color = MaterialTheme.colorScheme.surface,
                    ) {
                        Box(contentAlignment = Alignment.Center) {
                            Icon(
                                Icons.Default.ArrowUpward,
                                contentDescription = "Send message",
                                tint = if (canSend)
                                    MaterialTheme.colorScheme.primary
                                else
                                    MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        }
    }
}
