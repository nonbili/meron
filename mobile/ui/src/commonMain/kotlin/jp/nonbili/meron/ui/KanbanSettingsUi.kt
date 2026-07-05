package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Image
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

@Composable
internal fun SettingsKanbanBoardDetailPage(
    board: KanbanBoardSpec,
    active: Boolean,
    onSave: (
        name: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        wallpaperUrl: String,
    ) -> Unit,
    onPickAvatar: () -> Unit,
    onOpenWallpaper: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var name by remember(board.id, board.name) { mutableStateOf(board.name) }
    var avatarUrl by remember(board.id, board.avatarUrl) { mutableStateOf(board.avatarUrl) }
    var wallpaperPresetId by remember(board.id, board.wallpaperPresetId) { mutableStateOf(board.wallpaperPresetId) }
    var wallpaperUrl by remember(board.id, board.wallpaperUrl) { mutableStateOf(board.wallpaperUrl) }
    var confirmDelete by remember(board.id) { mutableStateOf(false) }

    // Autosave on every change, like the desktop board panel.
    val persist: (
        nextName: String,
        nextAvatarUrl: String,
        nextWallpaperPresetId: String,
        nextWallpaperUrl: String,
    ) -> Unit = { nextName, nextAvatarUrl, nextWallpaperPresetId, nextWallpaperUrl ->
        onSave(nextName, nextAvatarUrl, nextWallpaperPresetId, nextWallpaperUrl)
    }

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(tr("kanban.board.deleteTitle")) },
            text = { Text(trf("settings.kanban.deleteBoardText", board.name)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    onDelete()
                }) {
                    Text(tr("kanban.board.delete"), color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = false }) { Text(tr("buttons.cancel")) }
            },
        )
    }
    LazyColumn(modifier) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth().padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                AccountAvatarEditor(
                    name = name.ifBlank { board.name },
                    url = avatarUrl,
                    onPick = onPickAvatar,
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        name.ifBlank { board.name },
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        if (active) {
                            trf("settings.kanban.boardColumnsActive", board.columns.size)
                        } else {
                            trf("settings.kanban.boardColumns", board.columns.size)
                        },
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }

        item { SettingsSectionLabel(tr("settings.account.profile")) }
        item {
            SettingsTextRow(
                value = name,
                label = tr("kanban.board.name"),
                onValueChange = {
                    name = it
                    persist(it, avatarUrl, wallpaperPresetId, wallpaperUrl)
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.account.chatBackground")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Image,
                title = tr("settings.account.chatBackground"),
                subtitle =
                    if (wallpaperUrl.isNotBlank()) {
                        tr("wallpaper.customBackground")
                    } else {
                        wallpaperPresetDisplayName(wallpaperPresetId)
                    },
                onClick = onOpenWallpaper,
                trailing = {
                    WallpaperRowPreview(
                        presetId = wallpaperPresetId,
                        customUrl = wallpaperUrl,
                    )
                },
            )
        }

        item { SettingsSectionLabel(tr("settings.dangerZone")) }
        item {
            SettingsRow(
                icon = Icons.Filled.Delete,
                title = tr("kanban.board.delete"),
                subtitle = tr("settings.kanban.deleteBoardHint"),
                onClick = { confirmDelete = true },
                destructive = true,
            )
        }
        item { Spacer(Modifier.height(24.dp)) }
    }
}
