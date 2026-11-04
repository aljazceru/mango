package dev.disobey.mango.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.AltRoute
import androidx.compose.material.icons.outlined.Cloud
import androidx.compose.material.icons.outlined.PhoneAndroid
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.mango.R
import dev.disobey.mango.ui.theme.MangoTheme
import dev.disobey.mango.ui.theme.TintPair

/** The mango app icon, used as the brand mark. Decorative: no content description. */
@Composable
fun MangoMark(size: Dp, modifier: Modifier = Modifier) {
    Image(painterResource(R.mipmap.ic_launcher), contentDescription = null, modifier = modifier.size(size))
}

/** Where a chat's inference runs. */
enum class RouteKind { TEE, LOCAL, HYBRID }

val RouteKind.icon: ImageVector
    get() = when (this) {
        RouteKind.TEE -> Icons.Outlined.Cloud
        RouteKind.LOCAL -> Icons.Outlined.PhoneAndroid
        RouteKind.HYBRID -> Icons.AutoMirrored.Outlined.AltRoute
    }

val RouteKind.label: String
    get() = when (this) {
        RouteKind.TEE -> "Confidential cloud"
        RouteKind.LOCAL -> "On this phone"
        RouteKind.HYBRID -> "Hybrid"
    }

val RouteKind.tint: TintPair
    @Composable @ReadOnlyComposable get() = when (this) {
        RouteKind.TEE -> MangoTheme.colors.tee
        RouteKind.LOCAL -> MangoTheme.colors.local
        RouteKind.HYBRID -> MangoTheme.colors.hybrid
    }

/** Rounded-square tinted icon tile (route avatars, settings rows, suggestion cards). */
@Composable
fun TintedIcon(icon: ImageVector, tint: TintPair, modifier: Modifier = Modifier, size: Dp = 40.dp, contentDescription: String? = null) {
    Box(
        modifier = modifier
            .size(size)
            .clip(RoundedCornerShape(size * 0.35f))
            .background(tint.container),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = contentDescription, tint = tint.content, modifier = Modifier.size(size * 0.5f))
    }
}

@Composable
fun RouteAvatar(kind: RouteKind, modifier: Modifier = Modifier, size: Dp = 40.dp) {
    TintedIcon(kind.icon, kind.tint, modifier, size, contentDescription = kind.label)
}

enum class PillTone { OK, WARN, BAD, NEUTRAL }

/** Status chip: always icon (optional) + label, so status never relies on color alone. */
@Composable
fun StatusPill(text: String, tone: PillTone, modifier: Modifier = Modifier, icon: ImageVector? = null) {
    val c = MangoTheme.colors
    val (bg, fg) = when (tone) {
        PillTone.OK -> c.okContainer to c.ok
        PillTone.WARN -> c.warnContainer to c.warn
        PillTone.BAD -> MaterialTheme.colorScheme.errorContainer to MaterialTheme.colorScheme.error
        PillTone.NEUTRAL -> MaterialTheme.colorScheme.surfaceContainerHigh to MaterialTheme.colorScheme.onSurface
    }
    Row(
        modifier = modifier
            .clip(CircleShape)
            .background(bg)
            .padding(horizontal = 9.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (icon != null) Icon(icon, contentDescription = null, tint = fg, modifier = Modifier.size(14.dp))
        Text(text, style = MaterialTheme.typography.labelMedium, color = fg, maxLines = 1)
    }
}

/** Circular mango-gradient action (send, ask). Disabled state drops the gradient. */
@Composable
fun GradientIconButton(
    icon: ImageVector,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    size: Dp = 44.dp,
) {
    val c = MangoTheme.colors
    Box(
        modifier = modifier
            .size(size)
            .clip(CircleShape)
            .then(
                if (enabled) Modifier.background(c.accentGradient)
                else Modifier.background(MaterialTheme.colorScheme.surfaceContainerHigh)
            )
            .clickable(enabled = enabled, role = Role.Button, onClickLabel = contentDescription, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            icon,
            contentDescription = contentDescription,
            tint = if (enabled) c.onAccent else MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(size * 0.5f),
        )
    }
}

/** Card sitting on the background: white on cream, raised + hairline on near-black. */
@Composable
fun MangoCard(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    color: Color = MangoTheme.colors.card,
    content: @Composable ColumnScope.() -> Unit,
) {
    val border = if (MangoTheme.colors.isDark) BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant) else null
    if (onClick != null) {
        Surface(onClick = onClick, modifier = modifier, shape = MaterialTheme.shapes.medium, color = color, border = border) {
            Column(content = content)
        }
    } else {
        Surface(modifier = modifier, shape = MaterialTheme.shapes.medium, color = color, border = border) {
            Column(content = content)
        }
    }
}

/** Small uppercase section label ("TODAY", "MODELS"). */
@Composable
fun SectionLabel(text: String, modifier: Modifier = Modifier) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall.copy(letterSpacing = 1.sp),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}
