package dev.disobey.mango.ui

import androidx.compose.animation.core.EaseInOutCubic
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.StartOffset
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.UiMessage
import com.mikepenz.markdown.m3.Markdown
import com.mikepenz.markdown.m3.markdownColor
import com.mikepenz.markdown.m3.markdownTypography

/// Renders a single chat message bubble.
/// User messages: right-aligned, blue tint, plain Text.
/// Assistant messages: left-aligned, surface variant background, Markdown rendered.
@Composable
fun MessageBubble(
    message: UiMessage,
    isLastAssistant: Boolean,
    isStreaming: Boolean,
    onCopy: () -> Unit,
    onRetry: () -> Unit,
    onEdit: () -> Unit,
    modifier: Modifier = Modifier,
) {
    when (message.role) {
        "user" -> UserBubble(message = message, onCopy = onCopy, onEdit = onEdit, modifier = modifier)
        "assistant" -> AssistantBubble(
            message = message,
            isLastAssistant = isLastAssistant,
            isStreaming = isStreaming,
            onCopy = onCopy,
            onRetry = onRetry,
            modifier = modifier,
        )
        else -> SystemBubble(message = message, modifier = modifier)
    }
}

// MARK: - User Bubble

@Composable
private fun UserBubble(
    message: UiMessage,
    onCopy: () -> Unit,
    onEdit: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val bubbleColor = MaterialTheme.colorScheme.primaryContainer

    Column(modifier = modifier.fillMaxWidth(), horizontalAlignment = Alignment.End) {
        // Attachment indicator
        if (message.hasAttachment) {
            message.attachmentName?.let { name ->
                AttachmentIndicator(name = name)
            }
        }
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = bubbleColor,
        ) {
            Text(
                text = message.content,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        // Action row
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            TextButton(onClick = onEdit) {
                Text(
                    "Edit",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = onCopy) {
                Text(
                    "Copy",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

// MARK: - Assistant Bubble

@Composable
private fun AssistantBubble(
    message: UiMessage,
    isLastAssistant: Boolean,
    isStreaming: Boolean,
    onCopy: () -> Unit,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier.fillMaxWidth(), horizontalAlignment = Alignment.Start) {
        // Attachment indicator
        if (message.hasAttachment) {
            message.attachmentName?.let { name ->
                AttachmentIndicator(name = name)
            }
        }
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceVariant,
        ) {
            Markdown(
                content = message.content,
                colors = markdownColor(),
                typography = markdownTypography(),
                modifier = Modifier
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        // RAG context indicator (D-07): shown when documents contributed context
        val ragCount = message.ragContextCount
        if (ragCount != null && ragCount > 0u) {
            Text(
                text = "[context from $ragCount doc(s)]",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, top = 2.dp),
            )
        }
        // Action row
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            TextButton(onClick = onCopy) {
                Text(
                    "Copy",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (isLastAssistant) {
                TextButton(onClick = onRetry) {
                    Text(
                        "Retry",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

// MARK: - System Bubble

@Composable
private fun SystemBubble(
    message: UiMessage,
    modifier: Modifier = Modifier,
) {
    Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        Text(
            text = message.content,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(vertical = 4.dp),
        )
    }
}

// MARK: - Streaming Message Bubble

/// Left-aligned bubble for the in-progress streaming response.
/// retainState = true prevents loading flash between tokens.
@Composable
fun StreamingMessageBubble(
    text: String,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.Start,
    ) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceVariant,
        ) {
            Markdown(
                content = text,
                colors = markdownColor(),
                typography = markdownTypography(),
                modifier = Modifier
                    .padding(horizontal = 16.dp, vertical = 8.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
        // Streaming cursor
        Text(
            text = "▋",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 16.dp),
        )
    }
}

// MARK: - Thinking Indicator Bubble

/// Left-aligned bubble showing three staggered pulsing dots while the LLM is processing
/// before the first streaming token arrives.
@Composable
fun ThinkingIndicatorBubble(modifier: Modifier = Modifier) {
    val infiniteTransition = rememberInfiniteTransition(label = "thinking")

    val offsets = (0..2).map { index ->
        infiniteTransition.animateFloat(
            initialValue = 0f,
            targetValue = -8f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 600, easing = EaseInOutCubic),
                repeatMode = RepeatMode.Reverse,
                initialStartOffset = StartOffset(index * 150),
            ),
            label = "dot_$index",
        )
    }

    Column(modifier = modifier.fillMaxWidth(), horizontalAlignment = Alignment.Start) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceVariant,
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                offsets.forEach { offset ->
                    Box(
                        modifier = Modifier
                            .size(8.dp)
                            .offset(y = offset.value.dp)
                            .background(
                                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                                shape = CircleShape,
                            ),
                    )
                }
            }
        }
    }
}

// MARK: - Error Bubble

@Composable
fun ErrorBubble(
    error: String,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer,
        ),
        shape = RoundedCornerShape(12.dp),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.Top,
        ) {
            Icon(
                Icons.Default.Warning,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onErrorContainer,
                modifier = Modifier.size(16.dp).padding(top = 2.dp),
            )
            Column {
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onErrorContainer,
                )
                Spacer(modifier = Modifier.height(4.dp))
                TextButton(
                    onClick = onRetry,
                    contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp),
                ) {
                    Text(
                        "Retry",
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
            }
        }
    }
}

// MARK: - Attachment Indicator

@Composable
private fun AttachmentIndicator(name: String) {
    Row(
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(vertical = 2.dp),
    ) {
        Icon(
            Icons.Default.AttachFile,
            contentDescription = null,
            modifier = Modifier.size(12.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = name,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
