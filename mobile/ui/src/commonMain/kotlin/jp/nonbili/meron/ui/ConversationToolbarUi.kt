package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.PlaylistAdd
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.DragHandle
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.Report
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Sell
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import sh.calvin.reorderable.ReorderableColumn

/**
 * The conversation actions that can be pinned to the thread's top bar, in the
 * order Settings lists the unpinned ones. Ids match the desktop's
 * `CONVERSATION_TOOLBAR_ACTIONS` where the two share an action.
 */
internal enum class ConversationToolbarAction(
    val id: String,
    val icon: ImageVector,
    val labelKey: String,
) {
    Archive("archive", Icons.Filled.Archive, "threads.actions.archiveThread"),
    Spam("spam", Icons.Filled.Report, "threads.actions.markAsSpam"),
    Delete("delete", Icons.Filled.Delete, "buttons.delete"),
    Tags("tags", Icons.Filled.Sell, "tags.title"),
    Tasks("tasks", Icons.AutoMirrored.Filled.PlaylistAdd, "tasks.addFromMessage"),
    Star("star", Icons.Filled.Star, "chat.star"),
    Search("search", Icons.Filled.Search, "chat.searchThread"),
    MoveTo("moveTo", Icons.Outlined.FolderOpen, "threads.actions.moveTo"),
    CopyTo("copyTo", Icons.Filled.ContentCopy, "threads.actions.copyTo"),
    Print("print", Icons.Filled.Print, "chat.actions.printThread"),
}

/** What the bar held before it could be changed, so an untouched install keeps it. */
internal val DEFAULT_CONVERSATION_TOOLBAR =
    listOf(ConversationToolbarAction.Tags, ConversationToolbarAction.Tasks, ConversationToolbarAction.Star)

/** The pinned actions, in order, as comma-separated ids: a set would lose the
 *  order. Unknown ids (a newer build's) are dropped; empty means none pinned. */
internal fun loadConversationToolbar(prefs: AppPreferences): List<ConversationToolbarAction> {
    val raw = prefs.getString(CONVERSATION_TOOLBAR_PREF, "\u0000")
    if (raw == "\u0000") return DEFAULT_CONVERSATION_TOOLBAR
    val byId = ConversationToolbarAction.entries.associateBy { it.id }
    return raw.split(',').mapNotNull { byId[it.trim()] }.distinct()
}

internal fun saveConversationToolbar(
    prefs: AppPreferences,
    actions: List<ConversationToolbarAction>,
) = prefs.putString(CONVERSATION_TOOLBAR_PREF, actions.joinToString(",") { it.id })

/**
 * Which actions the conversation's top bar shows, and in what order. The pinned
 * ones come first, dragged by their handles; the rest follow in the app's own
 * order, and a switch pins one to the end. Whatever is not pinned stays in the
 * bar's ⋯ menu.
 */
@Composable
internal fun ConversationToolbarPage(
    pinned: List<ConversationToolbarAction>,
    onChange: (List<ConversationToolbarAction>) -> Unit,
    modifier: Modifier = Modifier,
) {
    val haptics = LocalHapticFeedback.current
    val unpinned = ConversationToolbarAction.entries.filter { it !in pinned }
    // Read here: accessibility actions run outside composition.
    val moveUp = tr("rules.moveUp")
    val moveDown = tr("rules.moveDown")
    val reorder = tr("common.dragToReorder")

    fun move(
        from: Int,
        to: Int,
    ) {
        if (from == to || from !in pinned.indices || to !in pinned.indices) return
        onChange(pinned.toMutableList().apply { add(to, removeAt(from)) })
    }

    fun toggle(action: ConversationToolbarAction) = onChange(if (action in pinned) pinned - action else pinned + action)

    Column(
        modifier.verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(top = 8.dp, bottom = 24.dp + navigationBarInset()),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            tr("settings.appearance.conversationToolbarHint"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
        if (pinned.isNotEmpty()) {
            ReorderableColumn(
                list = pinned,
                onSettle = ::move,
                onMove = { haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick) },
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) { at, action, dragging ->
                key(action.id) {
                    ReorderableItem {
                        Surface(
                            shape = groupShape(at, pinned.size),
                            color = if (dragging) MaterialTheme.colorScheme.surfaceContainerHigh else MaterialTheme.colorScheme.surface,
                            tonalElevation = if (dragging) 6.dp else 0.dp,
                            shadowElevation = if (dragging) 4.dp else 0.dp,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            SettingsToggleRow(
                                icon = action.icon,
                                title = tr(action.labelKey),
                                checked = true,
                                onToggle = { toggle(action) },
                                leading = {
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        Icon(
                                            Icons.Filled.DragHandle,
                                            contentDescription = null,
                                            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                                            modifier =
                                                Modifier
                                                    .draggableHandle(
                                                        onDragStarted = {
                                                            haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
                                                        },
                                                    ).semantics {
                                                        contentDescription = reorder
                                                        // A drag can't be performed from a screen reader.
                                                        customActions =
                                                            listOfNotNull(
                                                                CustomAccessibilityAction(moveUp) {
                                                                    move(at, at - 1)
                                                                    true
                                                                }.takeIf { at > 0 },
                                                                CustomAccessibilityAction(moveDown) {
                                                                    move(at, at + 1)
                                                                    true
                                                                }.takeIf { at < pinned.lastIndex },
                                                            )
                                                    }.padding(end = 12.dp),
                                        )
                                        Icon(action.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                },
                            )
                        }
                    }
                }
            }
        }
        if (unpinned.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                unpinned.forEachIndexed { at, action ->
                    key(action.id) {
                        Surface(
                            shape = groupShape(at, unpinned.size),
                            color = MaterialTheme.colorScheme.surface,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            SettingsToggleRow(
                                icon = action.icon,
                                title = tr(action.labelKey),
                                checked = false,
                                onToggle = { toggle(action) },
                                leading = {
                                    // Lines the icon up with the pinned rows', past their handles.
                                    Row(verticalAlignment = Alignment.CenterVertically) {
                                        Icon(
                                            Icons.Filled.DragHandle,
                                            contentDescription = null,
                                            tint = androidx.compose.ui.graphics.Color.Transparent,
                                            modifier = Modifier.padding(end = 12.dp),
                                        )
                                        Icon(action.icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

/** A row's corners in a connected group, as SettingsList draws them. */
private fun groupShape(
    at: Int,
    size: Int,
): RoundedCornerShape {
    val top = if (at == 0) 16.dp else 4.dp
    val bottom = if (at == size - 1) 16.dp else 4.dp
    return RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom)
}
