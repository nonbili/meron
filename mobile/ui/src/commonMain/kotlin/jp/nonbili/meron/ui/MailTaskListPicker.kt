package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MailTaskListPicker(state: MeronMobileState) {
    if (state.pendingTaskThread == null) return
    ModalBottomSheet(
        onDismissRequest = { state.pendingTaskThread = null },
        containerColor = MaterialTheme.colorScheme.surface,
    ) {
        ListItem(headlineContent = { Text(tr("tasks.addFromMessage")) })
        // Only lists this device may change; synced ones under their account.
        val groups = taskListGroups(state.taskLists.filterNot { it.readOnly })
        LazyColumn(Modifier.heightIn(max = 400.dp)) {
            groups.forEach { group ->
                if (group.label.isNotBlank()) {
                    item(key = "group:${group.key}") {
                        Text(
                            group.label,
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 4.dp),
                        )
                    }
                }
                items(group.lists, key = { it.id }) { list ->
                    ListItem(
                        headlineContent = { Text(list.title) },
                        trailingContent = {
                            RadioButton(selected = list.id == state.activeTaskListId, onClick = null)
                        },
                        modifier =
                            Modifier.clickable {
                                state.scope.launch { state.pickMailTaskList(list.id) }
                            },
                    )
                }
            }
        }
    }
}
