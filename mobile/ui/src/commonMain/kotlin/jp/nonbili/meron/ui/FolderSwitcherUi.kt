package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.sizeIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.NotificationsOff
import androidx.compose.material.icons.outlined.NotificationsNone
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.FolderSummary

/**
 * A folder name that doubles as a picker: tapping it lists the other folders of
 * the same account, so the surface showing it (a kanban column, the mail list)
 * can be pointed elsewhere without being torn down and rebuilt.
 */
@Composable
internal fun FolderSwitcher(
    label: String,
    folders: List<FolderSummary>,
    currentFolderId: String,
    onRequestFolders: () -> Unit,
    onSelectFolder: (String) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    /** Folders already shown elsewhere (e.g. another column) and so not offered. */
    takenFolderIds: Set<String> = emptySet(),
    fontSize: TextUnit = 12.sp,
    fontWeight: FontWeight = FontWeight.SemiBold,
    /** Long-pressing a folder that can notify offers this; absent when the list has no such action. */
    onSetFolderNotify: ((FolderSummary, Boolean) -> Unit)? = null,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Box(modifier) {
        Row(
            Modifier
                .clip(RoundedCornerShape(6.dp))
                .clickable(enabled = enabled) {
                    onRequestFolders()
                    menuOpen = true
                }.padding(horizontal = 2.dp, vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                label,
                fontWeight = fontWeight,
                fontSize = fontSize,
                modifier = Modifier.weight(1f, fill = false),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (enabled) {
                Icon(
                    Icons.Filled.KeyboardArrowDown,
                    contentDescription = tr("kanban.actions.switchFolder"),
                    modifier = Modifier.size(15.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            if (folders.isEmpty()) {
                DropdownMenuItem(text = { Text(tr("folders.loading")) }, onClick = {}, enabled = false)
            }
            // Nested folders are indented under their parent, the same hierarchy
            // the add-column dialog shows.
            val folderRows = remember(folders) { flattenFolderTree(buildFolderTree(folders)) }
            folderRows.forEach { row ->
                val folder = row.node.folder
                val current = folder != null && kanbanFolderIdsEqual(folder.name, currentFolderId)
                val taken =
                    folder != null &&
                        !current &&
                        takenFolderIds.any { kanbanFolderIdsEqual(folder.name, it) }
                if (folder != null && !taken && !current && onSetFolderNotify != null && folder.isNotifiable()) {
                    NotifiableFolderRow(
                        folder = folder,
                        label = row.node.name.replaceFirstChar { it.uppercase() },
                        modifier = Modifier.padding(start = (row.depth * 14).dp),
                        onSelect = {
                            menuOpen = false
                            onSelectFolder(folder.name)
                        },
                        onSetNotify = { enabled ->
                            menuOpen = false
                            onSetFolderNotify(folder, enabled)
                        },
                    )
                    return@forEach
                }
                DropdownMenuItem(
                    modifier = Modifier.padding(start = (row.depth * 14).dp),
                    text = {
                        Text(
                            row.node.name.replaceFirstChar { it.uppercase() },
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            fontWeight = if (current) FontWeight.SemiBold else FontWeight.Normal,
                        )
                    },
                    leadingIcon = {
                        Icon(
                            when {
                                current -> Icons.Filled.Check
                                folder != null -> folderIcon(folder)
                                else -> folderIcon(row.node.name)
                            },
                            contentDescription = null,
                            tint =
                                if (current) {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.onSurfaceVariant
                                },
                        )
                    },
                    // Structural nodes (no folder of their own) are labels only.
                    enabled = folder != null && !taken && !current,
                    onClick = {
                        menuOpen = false
                        folder?.let { onSelectFolder(it.name) }
                    },
                )
            }
        }
    }
}

/**
 * A picker row for a folder that can be opted in to notifications: a tap picks
 * it like any other row, a long press offers the switch. Laid out to match
 * [DropdownMenuItem], which has no long-press of its own.
 */
@Composable
private fun NotifiableFolderRow(
    folder: FolderSummary,
    label: String,
    onSelect: () -> Unit,
    onSetNotify: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    var actionsOpen by remember { mutableStateOf(false) }
    Box(modifier) {
        Row(
            Modifier
                .fillMaxWidth()
                .combinedClickable(onClick = onSelect, onLongClick = { actionsOpen = true })
                .sizeIn(minWidth = 112.dp, maxWidth = 280.dp, minHeight = 48.dp)
                .padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                folderIcon(folder),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.width(12.dp))
            Text(
                label,
                style = MaterialTheme.typography.labelLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        DropdownMenu(expanded = actionsOpen, onDismissRequest = { actionsOpen = false }) {
            DropdownMenuItem(
                text = { Text(tr(if (folder.notify) "folders.notify.disable" else "folders.notify.enable")) },
                leadingIcon = {
                    Icon(
                        if (folder.notify) Icons.Filled.NotificationsOff else Icons.Outlined.NotificationsNone,
                        contentDescription = null,
                    )
                },
                onClick = {
                    actionsOpen = false
                    onSetNotify(!folder.notify)
                },
            )
        }
    }
}
