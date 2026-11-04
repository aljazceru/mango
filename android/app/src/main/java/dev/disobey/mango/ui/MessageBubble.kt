package dev.disobey.mango.ui

import android.graphics.BitmapFactory
import androidx.compose.animation.core.EaseInOutCubic
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.StartOffset
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.Image
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Replay
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import dev.disobey.mango.rust.UiMessage
import com.mikepenz.markdown.m3.Markdown
import com.mikepenz.markdown.m3.markdownColor
import com.mikepenz.markdown.m3.markdownTypography
import com.mikepenz.markdown.model.markdownDimens
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import dev.disobey.mango.ui.theme.MangoTheme
import dev.disobey.mango.ui.theme.MonoFamily
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/// Renders a single chat message.
/// User messages: right-aligned mango bubble with a tail-side corner, plain Text.
/// Assistant messages: no bubble — a full-width Markdown reading column with a
/// "Mango" header on the first message of a consecutive run.
@Composable
fun MessageBubble(
    message: UiMessage,
    isLastAssistant: Boolean,
    onCopy: () -> Unit,
    onRetry: () -> Unit,
    onEdit: () -> Unit,
    modifier: Modifier = Modifier,
    fontScale: Float = 1f,
    // IMG-07: decrypt-on-read callback; null when DEK unavailable
    onReadEncryptedImage: ((String) -> ByteArray)? = null,
    // "Mango" label header on the first message of a consecutive assistant run.
    showAssistantHeader: Boolean = false,
) {
    val density = LocalDensity.current
    val scaledDensity = if (fontScale != 1f) {
        Density(density.density, density.fontScale * fontScale)
    } else {
        density
    }
    CompositionLocalProvider(LocalDensity provides scaledDensity) {
        when (message.role) {
            "user" -> UserBubble(
                message = message,
                onCopy = onCopy,
                onEdit = onEdit,
                onReadEncryptedImage = onReadEncryptedImage,
                modifier = modifier,
            )
            "assistant" -> AssistantBubble(
                message = message,
                isLastAssistant = isLastAssistant,
                onCopy = onCopy,
                onRetry = onRetry,
                showHeader = showAssistantHeader,
            )
            else -> SystemBubble(message = message, modifier = modifier)
        }
    }
}

// MARK: - User Bubble

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun UserBubble(
    message: UiMessage,
    onCopy: () -> Unit,
    onEdit: () -> Unit,
    modifier: Modifier = Modifier,
    onReadEncryptedImage: ((String) -> ByteArray)? = null,
) {
    val bubbleColor = MangoTheme.colors.userBubble

    // IMG-07: load decrypted thumbnail on demand
    var thumbnail by remember(message.id) { mutableStateOf<android.graphics.Bitmap?>(null) }
    var actionsOpen by remember { mutableStateOf(false) }
    if (message.imagePath != null && onReadEncryptedImage != null) {
        LaunchedEffect(message.id) {
            runCatching {
                val bytes = withContext(Dispatchers.IO) { onReadEncryptedImage(message.id) }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            }.onSuccess { bmp ->
                thumbnail = bmp
            }
        }
    }

    Column(modifier = modifier.fillMaxWidth(), horizontalAlignment = Alignment.End) {
        // Attachment indicator
        if (message.hasAttachment) {
            message.attachmentName?.let { name ->
                AttachmentIndicator(name = name)
            }
        }
        // IMG-07: show decrypted thumbnail when available
        thumbnail?.let { bmp ->
            Image(
                bitmap = bmp.asImageBitmap(),
                contentDescription = "Sent image",
                contentScale = ContentScale.Fit,
                modifier = Modifier
                    .widthIn(max = 240.dp)
                    .clip(MaterialTheme.shapes.extraSmall),
            )
        }
        if (thumbnail == null || message.content.isNotEmpty()) {
            Box {
                Surface(
                    // Tail-side corner: sharp at bottom-end (mirrored in RTL).
                    shape = RoundedCornerShape(topStart = 20.dp, topEnd = 20.dp, bottomEnd = 6.dp, bottomStart = 20.dp),
                    color = bubbleColor,
                    contentColor = MangoTheme.colors.onUserBubble,
                    modifier = Modifier
                        .combinedClickable(
                            onClick = {},
                            onLongClick = { actionsOpen = true },
                        )
                        .semantics {
                            customActions = listOf(
                                CustomAccessibilityAction("Edit message") {
                                    onEdit()
                                    true
                                },
                                CustomAccessibilityAction("Copy message") {
                                    onCopy()
                                    true
                                },
                            )
                        },
                ) {
                    Text(
                        text = message.content,
                        style = MaterialTheme.typography.bodyLarge,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
                MessageActionsMenu(
                    expanded = actionsOpen,
                    onDismiss = { actionsOpen = false },
                    actions = listOf(
                        MessageAction("Edit", onEdit),
                        MessageAction("Copy", onCopy),
                    ),
                )
            }
        }
        MessageTimestamp(message.createdAt)
    }
}

// MARK: - Assistant Bubble

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun AssistantBubble(
    message: UiMessage,
    isLastAssistant: Boolean,
    onCopy: () -> Unit,
    onRetry: () -> Unit,
    showHeader: Boolean = false,
    modifier: Modifier = Modifier,
) {
    var actionsOpen by remember { mutableStateOf(false) }
    val actions = buildList {
        add(MessageAction("Copy", onCopy))
        if (isLastAssistant) add(MessageAction("Retry", onRetry))
    }
    // Per-message provenance (route · provider/model · TEE) is no longer a
    // footer; route changes render as thread dividers and the full metadata
    // moves to the long-press menu.
    val metadata = routeMetadataLabel(
        providerName = message.routeProviderName,
        modelId = message.routeModelId,
        decision = message.routeDecision,
        teeLabel = message.routeTeeLabel,
        teeVerified = message.routeTeeVerified,
    )
    Column(modifier = modifier.fillMaxWidth(), horizontalAlignment = Alignment.Start) {
        if (showHeader) {
            AssistantHeader()
        }
        // Attachment indicator
        if (message.hasAttachment) {
            message.attachmentName?.let { name ->
                AttachmentIndicator(name = name)
            }
        }
        Box {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(MaterialTheme.shapes.small)
                    .combinedClickable(
                        onClick = {},
                        onLongClick = { actionsOpen = true },
                    )
                    .semantics {
                        customActions = actions.map { action ->
                            CustomAccessibilityAction("${action.label} message") {
                                action.onClick()
                                true
                            }
                        }
                    },
            ) {
                Markdown(
                    content = message.content,
                    colors = markdownColor(
                        codeBackground = MaterialTheme.colorScheme.surfaceContainerHigh,
                        inlineCodeBackground = MaterialTheme.colorScheme.surfaceContainerHigh,
                        dividerColor = MaterialTheme.colorScheme.outlineVariant,
                    ),
                    typography = markdownTypography(
                        h1 = MaterialTheme.typography.titleMedium,
                        h2 = MaterialTheme.typography.titleSmall,
                        h3 = MaterialTheme.typography.bodyLarge.copy(
                            fontWeight = FontWeight.Bold,
                        ),
                        h4 = MaterialTheme.typography.bodyMedium.copy(
                            fontWeight = FontWeight.Bold,
                        ),
                        h5 = MaterialTheme.typography.bodySmall.copy(
                            fontWeight = FontWeight.Bold,
                        ),
                        h6 = MaterialTheme.typography.bodySmall.copy(
                            fontWeight = FontWeight.Bold,
                        ),
                        text = MaterialTheme.typography.bodyLarge,
                        paragraph = MaterialTheme.typography.bodyLarge,
                        bullet = MaterialTheme.typography.bodyLarge,
                        ordered = MaterialTheme.typography.bodyLarge,
                        list = MaterialTheme.typography.bodyLarge,
                        table = MaterialTheme.typography.bodySmall,
                        code = MaterialTheme.typography.bodyMedium.copy(
                            fontFamily = MonoFamily,
                        ),
                        inlineCode = MaterialTheme.typography.bodyMedium.copy(
                            fontFamily = MonoFamily,
                        ),
                    ),
                    dimens = markdownDimens(codeBackgroundCornerSize = 14.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 4.dp),
                )
            }
            MessageActionsMenu(
                expanded = actionsOpen,
                onDismiss = { actionsOpen = false },
                actions = actions,
                header = metadata,
            )
        }
        // Document context indicator (D-07): shown when documents contributed context
        val ragCount = message.ragContextCount
        if (ragCount != null && ragCount > 0u) {
            Text(
                text = "[context from $ragCount doc(s)]",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 4.dp, top = 2.dp),
            )
        }
        // Actions row: copy + regenerate as muted icon buttons, then the time.
        Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(
                onClick = onCopy,
                modifier = Modifier.size(48.dp),
            ) {
                Icon(
                    Icons.Default.ContentCopy,
                    contentDescription = "Copy message",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(16.dp),
                )
            }
            if (isLastAssistant) {
                IconButton(
                    onClick = onRetry,
                    modifier = Modifier.size(48.dp),
                ) {
                    Icon(
                        Icons.Default.Replay,
                        contentDescription = "Regenerate response",
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
            Spacer(modifier = Modifier.weight(1f))
            MessageTimestamp(message.createdAt)
        }
    }
}

/// Small "Mango" run header shown above the first assistant message of a run.
@Composable
private fun AssistantHeader() {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        modifier = Modifier.padding(bottom = 2.dp),
    ) {
        MangoMark(16.dp)
        Text(
            text = "Mango",
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun MessageTimestamp(createdAt: Long) {
    Text(
        text = formatMessageTimestamp(createdAt),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 4.dp, vertical = 2.dp),
    )
}

private data class MessageAction(
    val label: String,
    val onClick: () -> Unit,
)

@Composable
private fun MessageActionsMenu(
    expanded: Boolean,
    onDismiss: () -> Unit,
    actions: List<MessageAction>,
    /// Read-only provenance line ("Remote · PPQ.AI / gpt-oss · SEV-SNP verified")
    /// shown as a disabled header item, matching the removed per-message footer.
    header: String? = null,
) {
    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        if (header != null) {
            DropdownMenuItem(
                text = {
                    Text(
                        header,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                enabled = false,
                onClick = {},
            )
        }
        actions.forEach { action ->
            DropdownMenuItem(
                text = { Text(action.label) },
                onClick = {
                    onDismiss()
                    action.onClick()
                },
            )
        }
    }
}

internal fun formatMessageTimestamp(epochMillis: Long): String {
    return SimpleDateFormat("h:mm a", Locale.getDefault()).format(Date(epochMillis))
}

internal fun routeMetadataLabel(
    providerName: String?,
    modelId: String?,
    decision: String?,
    teeLabel: String?,
    teeVerified: Boolean?,
): String? {
    val route = decision.cleanRouteSegment()?.replaceFirstChar { char ->
        if (char.isLowerCase()) char.titlecase(Locale.getDefault()) else char.toString()
    }
    val providerModel = listOfNotNull(
        providerName.cleanRouteSegment(),
        modelId.cleanRouteSegment(),
    ).joinToString(" / ").ifBlank { null }
    val tee = when {
        teeLabel.cleanRouteSegment() != null && teeVerified == true -> "${teeLabel.cleanRouteSegment()} verified"
        teeLabel.cleanRouteSegment() != null && teeVerified == false -> "${teeLabel.cleanRouteSegment()} unverified"
        teeLabel.cleanRouteSegment() != null -> teeLabel.cleanRouteSegment()
        teeVerified == true -> "TEE verified"
        teeVerified == false -> "TEE unverified"
        else -> null
    }

    return listOfNotNull(route, providerModel, tee)
        .distinct()
        .joinToString(" · ")
        .ifBlank { null }
}

private fun String?.cleanRouteSegment(): String? = this?.trim()?.takeIf { it.isNotEmpty() }

/// Route identity for provenance: shown as a centered divider above an
/// assistant message whenever it differs from the previous routed reply.
internal data class RouteSignature(
    val decision: String,
    val backendId: String,
    val modelId: String,
    val teeLabel: String,
)

private fun UiMessage.routeSignature(): RouteSignature? {
    if (role != "assistant") return null
    val decision = routeDecision.cleanRouteSegment()
    val backend = routeBackendId.cleanRouteSegment()
    val model = routeModelId.cleanRouteSegment()
    val tee = routeTeeLabel.cleanRouteSegment()
    if (decision == null && backend == null && model == null && tee == null) return null
    return RouteSignature(decision ?: "", backend ?: "", model ?: "", tee ?: "")
}

/// Label for the centered route-change divider rendered ABOVE the message at
/// [index] in the chronological [messages] list, or null when the route is
/// unchanged (or unknown). Messages without provenance don't change the route,
/// so legacy replies never get a divider and don't break a run.
internal fun routeDividerLabel(messages: List<UiMessage>, index: Int): String? {
    val sig = messages.getOrNull(index)?.routeSignature() ?: return null
    val prev = messages.subList(0, index).lastOrNull { it.routeSignature() != null }
        ?.routeSignature()
    return if (prev == sig) null else routeDividerText(messages[index])
}

private fun routeDividerText(message: UiMessage): String {
    val decision = message.routeDecision.cleanRouteSegment()?.lowercase(Locale.US)
    val provider = message.routeProviderName.cleanRouteSegment()
    val model = shortModelName(message.routeModelId.cleanRouteSegment() ?: "")
        .takeIf { it.isNotEmpty() }
    val tee = message.routeTeeLabel.cleanRouteSegment()
    return when (decision) {
        "local" -> listOfNotNull("On this phone", model).joinToString(" · ")
        "remote" -> {
            val dest = provider ?: model ?: "Remote enclave"
            val teePart = when {
                tee != null && message.routeTeeVerified == true -> "$tee verified"
                tee != null -> tee
                message.routeTeeVerified == true -> "TEE verified"
                else -> null
            }
            if (teePart != null) "Sealed in $dest · $teePart" else "Sealed in $dest"
        }
        else -> routeMetadataLabel(
            providerName = message.routeProviderName,
            modelId = message.routeModelId,
            decision = message.routeDecision,
            teeLabel = message.routeTeeLabel,
            teeVerified = message.routeTeeVerified,
        ) ?: "Route changed"
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
    fontScale: Float = 1f,
) {
    val density = LocalDensity.current
    val scaledDensity = if (fontScale != 1f) {
        Density(density.density, density.fontScale * fontScale)
    } else {
        density
    }
    CompositionLocalProvider(LocalDensity provides scaledDensity) {
    Column(
        modifier = modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.Start,
    ) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier
                .semantics { liveRegion = LiveRegionMode.Polite },
        )
        // Streaming cursor
        Text(
            text = "▋",
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(start = 4.dp),
        )
    }
    } // end CompositionLocalProvider
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

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
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

// MARK: - Error Bubble

@Composable
fun ErrorBubble(
    error: String,
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
    onCompact: (() -> Unit)? = null,
) {
    MangoCard(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.errorContainer,
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
                onCompact?.let { compact ->
                    TextButton(
                        onClick = compact,
                        contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp),
                    ) {
                        Text(
                            "Compact & retry",
                            style = MaterialTheme.typography.labelSmall,
                        )
                    }
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
