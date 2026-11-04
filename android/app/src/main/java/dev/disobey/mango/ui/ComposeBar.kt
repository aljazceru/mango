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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Stop
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.AttachmentInfo
import dev.disobey.mango.ui.theme.MangoTheme
/// Compose bar pinned to the bottom of the chat screen: a floating pill with
/// attachment strip, a route chip row, transparent text field and a
/// gradient send / stop action.
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
    routingKind: RouteKind? = null,
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

    // Floating pill card, hairlined on dark via MangoCard-style border logic.
    val border = if (MangoTheme.colors.isDark)
        androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
    else null
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .imePadding()
            .navigationBarsPadding()
            .padding(horizontal = 12.dp, vertical = 8.dp),
        shape = MaterialTheme.shapes.extraLarge,
        color = MangoTheme.colors.card,
        border = border,
        shadowElevation = 6.dp,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 6.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
        ) {
            // Route chip on its own row (tap shows the routing reason); the full
            // label ellipsizes so long provider names can't starve the input row.
            if (!routingLabel.isNullOrBlank()) {
                Surface(
                    modifier = Modifier
                        .padding(start = 4.dp, bottom = 4.dp)
                        .clip(CircleShape)
                        .clickable(enabled = !routingDetail.isNullOrBlank()) {
                            routingDetail?.let {
                                Toast.makeText(context, it, Toast.LENGTH_SHORT).show()
                            }
                        },
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    contentColor = MaterialTheme.colorScheme.onSurface,
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 9.dp, vertical = 5.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        routingKind?.let { kind ->
                            Icon(
                                kind.icon,
                                contentDescription = null,
                                tint = kind.tint.content,
                                modifier = Modifier.size(13.dp),
                            )
                        }
                        Text(
                            routingLabel,
                            style = MaterialTheme.typography.labelSmall,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }
            // Pending attachment indicator
            if (pendingAttachment != null) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(start = 10.dp, bottom = 2.dp),
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
                        modifier = Modifier.size(48.dp),
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
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                // Attach button
                IconButton(
                    onClick = onAttach,
                    enabled = canMutateComposerAttachment(isResponseActive),
                    modifier = Modifier.size(48.dp),
                ) {
                    Icon(
                        Icons.Default.AttachFile,
                        contentDescription = "Attach file for context",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                // Text input
                TextField(
                    value = inputText,
                    onValueChange = { inputText = it },
                    placeholder = {
                        Text("Message", maxLines = 1, overflow = TextOverflow.Ellipsis)
                    },
                    modifier = Modifier.weight(1f),
                    minLines = 1,
                    maxLines = 6,
                    enabled = canEditComposerText(),
                    colors = TextFieldDefaults.colors(
                        focusedContainerColor = Color.Transparent,
                        unfocusedContainerColor = Color.Transparent,
                        disabledContainerColor = Color.Transparent,
                        focusedIndicatorColor = Color.Transparent,
                        unfocusedIndicatorColor = Color.Transparent,
                        disabledIndicatorColor = Color.Transparent,
                    ),
                )

                // Send / Stop button
                if (showStopButton) {
                    Surface(
                        modifier = Modifier
                            .size(48.dp)
                            .clip(CircleShape)
                            .clickable {
                                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                onStop()
                            },
                        shape = CircleShape,
                        color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    ) {
                        Box(contentAlignment = Alignment.Center) {
                            Icon(
                                Icons.Default.Stop,
                                contentDescription = "Stop generating",
                                tint = MaterialTheme.colorScheme.error,
                            )
                        }
                    }
                } else {
                    val canSend = canSendComposerText(inputText, isResponseActive)
                    GradientIconButton(
                        icon = Icons.Default.ArrowUpward,
                        contentDescription = "Send message",
                        enabled = canSend,
                        onClick = {
                            val trimmed = inputText.trim()
                            if (trimmed.isNotEmpty()) {
                                haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                                onSend(trimmed)
                                inputText = ""
                            }
                        },
                    )
                }
            }
        }
    }
}
