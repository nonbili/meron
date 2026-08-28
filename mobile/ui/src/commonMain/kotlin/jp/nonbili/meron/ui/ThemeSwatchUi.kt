package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

// Mini app mock (side navigation / mail list / bubbles) painted with a theme's
// OWN colors, so every swatch previews its theme regardless of the active one.
// Mirrors desktop/frontend/src/components/dialog/ThemeSwatch.tsx.

@Composable
internal fun ThemePreviewMock(
    colors: ThemePreviewColors,
    modifier: Modifier = Modifier,
    height: Dp = 72.dp,
) {
    val large = height >= 48.dp
    Row(modifier.fillMaxWidth().height(height).background(colors.bgApp)) {
        Box(Modifier.fillMaxHeight().width(if (large) 12.dp else 5.dp).background(colors.bgSideNav))
        Box(Modifier.fillMaxHeight().width(if (large) 30.dp else 13.dp).background(colors.bgChats))
        Box(Modifier.fillMaxHeight().width(1.dp).background(colors.border))
        Column(
            Modifier.weight(1f).fillMaxHeight().padding(horizontal = if (large) 8.dp else 4.dp),
            verticalArrangement = Arrangement.spacedBy(if (large) 5.dp else 2.dp, Alignment.CenterVertically),
        ) {
            Pill(colors.bubbleIn, width = if (large) 44.dp else 16.dp, large = large, borderColor = colors.border)
            Pill(colors.bubbleOut, width = if (large) 44.dp else 16.dp, large = large, alignment = Alignment.End)
            Pill(colors.accent, width = if (large) 28.dp else 11.dp, large = large)
        }
    }
}

@Composable
private fun ColumnScope.Pill(
    color: Color,
    width: Dp,
    large: Boolean,
    alignment: Alignment.Horizontal = Alignment.Start,
    borderColor: Color? = null,
) {
    Box(
        Modifier
            .align(alignment)
            .width(width)
            .height(if (large) 9.dp else 4.dp)
            .clip(CircleShape)
            .background(color)
            .then(if (borderColor != null) Modifier.border(1.dp, borderColor, CircleShape) else Modifier),
    )
}

/** One selectable theme tile: the mock plus the theme name and a check when active. */
@Composable
internal fun ThemeSwatch(
    mode: AppAppearanceMode,
    selected: Boolean,
    onSelect: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = themePreviewColors(mode)
    Column(
        modifier
            .clip(RoundedCornerShape(12.dp))
            .border(
                width = if (selected) 2.dp else 1.dp,
                color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant,
                shape = RoundedCornerShape(12.dp),
            ).clickable(onClick = onSelect),
    ) {
        ThemePreviewMock(colors)
        Row(
            Modifier
                .fillMaxWidth()
                .background(MaterialTheme.colorScheme.surface)
                .padding(horizontal = 8.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                mode.label,
                modifier = Modifier.weight(1f),
                style = MaterialTheme.typography.labelMedium,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            )
            if (selected) {
                Icon(
                    Icons.Filled.Check,
                    contentDescription = null,
                    modifier = Modifier.size(14.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
            }
        }
    }
}
