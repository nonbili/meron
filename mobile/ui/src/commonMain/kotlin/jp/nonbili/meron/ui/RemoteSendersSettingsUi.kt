package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

/**
 * The app-wide remote-content allowlist: every sender trusted with "Always allow
 * from …" on a message whose remote content was blocked. It exists mainly so a
 * decision made in a hurry can be taken back — the desktop RemoteSendersDialog
 * is the same list behind the same `remote_image_senders` row.
 */
@Composable
internal fun RemoteSendersPage(
    senders: List<String>,
    onRemoveSender: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var filter by remember { mutableStateOf("") }
    val query = filter.trim().lowercase()
    val visible = if (query.isEmpty()) senders else senders.filter { it.contains(query) }
    val listState = rememberLazyListState()
    Column(modifier) {
        Text(
            tr("settings.privacy.remoteSendersHint"),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
        )
        // A filter earns its place only once the list is long enough to scroll
        // past; below that it is one more thing between the user and the row
        // they came to delete.
        if (senders.size > FILTER_THRESHOLD) {
            SettingsTextRow(
                value = filter,
                label = tr("settings.privacy.filterSenders"),
                onValueChange = { filter = it },
            )
        }
        HorizontalDivider()
        if (visible.isEmpty()) {
            SettingsEmptyLabel(
                if (senders.isEmpty()) {
                    tr("settings.privacy.noRemoteSenders")
                } else {
                    tr("settings.privacy.noMatchingSenders")
                },
            )
        } else {
            LazyColumn(Modifier.fillMaxWidth().appScrollbar(listState), state = listState) {
                items(visible, key = { it }) { sender ->
                    ListItem(
                        headlineContent = { Text(sender, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        trailingContent = {
                            IconButton(onClick = { onRemoveSender(sender) }) {
                                Icon(
                                    Icons.Filled.Delete,
                                    contentDescription = tr("settings.privacy.removeRemoteSender"),
                                    tint = MaterialTheme.colorScheme.error,
                                )
                            }
                        },
                    )
                }
            }
        }
    }
}

private const val FILTER_THRESHOLD = 8
