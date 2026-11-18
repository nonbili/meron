package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Block
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Palette
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * Account colours offered in account settings, the same presets desktop offers
 * (lib/colorPresets.ts). Desktop can also pick any other `#rrggbb` through the
 * system colour picker; such a colour shows here as its own selected swatch.
 */
internal val ACCOUNT_COLORS: List<Pair<String, String>> =
    listOf(
        "red" to "#ef4444",
        "orange" to "#f97316",
        "amber" to "#f59e0b",
        "green" to "#22c55e",
        "teal" to "#14b8a6",
        "blue" to "#3b82f6",
        "violet" to "#8b5cf6",
        "pink" to "#ec4899",
    )

/** Parse an account's `#rrggbb` colour; null when unset or malformed. */
internal fun accountColorOrNull(hex: String): Color? {
    val digits = hex.trim().removePrefix("#")
    if (digits.length != 6) return null
    val rgb = digits.toLongOrNull(16) ?: return null
    return Color(0xFF000000 or rgb)
}

/** Pick the colour that marks this account's rows in mixed-account lists. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SettingsAccountColorRow(
    color: String,
    onSelect: (String) -> Unit,
) {
    val current = color.trim().lowercase()
    ListItem(
        headlineContent = { SettingsHeadline(title = tr("settings.account.color"), hint = tr("settings.account.colorHint")) },
        supportingContent = {
            // Wraps on narrow screens rather than clipping the last swatches.
            FlowRow(
                Modifier.padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                ColorSwatch(
                    label = tr("settings.account.colorNone"),
                    fill = null,
                    selected = current.isEmpty(),
                    onClick = { if (current.isNotEmpty()) onSelect("") },
                )
                ACCOUNT_COLORS.forEach { (id, hex) ->
                    ColorSwatch(
                        label = tr("colors.$id"),
                        fill = accountColorOrNull(hex),
                        selected = current == hex,
                        onClick = { if (current != hex) onSelect(hex) },
                    )
                }
                // A colour picked on desktop outside the presets.
                val custom = accountColorOrNull(current).takeIf { ACCOUNT_COLORS.none { it.second == current } }
                if (custom != null) {
                    ColorSwatch(label = tr("colors.custom"), fill = custom, selected = true, onClick = {})
                }
            }
        },
        leadingContent = {
            Icon(Icons.Filled.Palette, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        },
    )
}

@Composable
private fun ColorSwatch(
    label: String,
    fill: Color?,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val ring = if (selected) MaterialTheme.colorScheme.primary else Color.Transparent
    Box(
        Modifier
            .size(28.dp)
            .border(2.dp, ring, CircleShape)
            .padding(3.dp)
            .clip(CircleShape)
            .background(fill ?: Color.Transparent)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick)
            .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        when {
            fill == null -> {
                Icon(
                    Icons.Filled.Block,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp),
                )
            }

            selected -> {
                Icon(Icons.Filled.Check, contentDescription = null, tint = Color.White, modifier = Modifier.size(14.dp))
            }
        }
    }
}

/**
 * Draw the bar along a row's start edge that marks its account in lists that
 * mix accounts; no-op for a null colour. Apply after the row's background.
 */
internal fun Modifier.accountColorEdge(color: Color?): Modifier =
    if (color == null) {
        this
    } else {
        drawBehind {
            val width = 3.dp.toPx()
            val inset = 8.dp.toPx()
            drawRoundRect(
                color = color,
                topLeft = Offset(width, inset),
                size = Size(width, (size.height - 2 * inset).coerceAtLeast(0f)),
                cornerRadius = CornerRadius(width / 2),
            )
        }
    }
