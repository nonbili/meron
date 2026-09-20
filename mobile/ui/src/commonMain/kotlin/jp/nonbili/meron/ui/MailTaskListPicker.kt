package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ListItem
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
    ModalBottomSheet(onDismissRequest = { state.pendingTaskThread = null }) {
        ListItem(headlineContent = { Text(tr("tasks.addFromMessage")) })
        LazyColumn(Modifier.heightIn(max = 400.dp)) {
            items(state.taskLists, key = { it.id }) { list ->
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
