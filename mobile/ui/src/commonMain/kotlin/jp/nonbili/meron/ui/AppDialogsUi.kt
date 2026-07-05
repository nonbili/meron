package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

@Composable
internal fun AddFeedDialog(
    url: String,
    onUrlChange: (String) -> Unit,
    onAdd: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("feeds.actions.addFeed")) },
        text = {
            OutlinedTextField(
                value = url,
                onValueChange = onUrlChange,
                label = { Text(tr("feeds.url")) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(onClick = onAdd) {
                Text(tr("common.add"))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}

@Composable
internal fun KanbanCreateFolderDialog(
    name: String,
    onNameChange: (String) -> Unit,
    onCreate: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("folders.create")) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = onNameChange,
                label = { Text(tr("folders.namePlaceholder")) },
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(onClick = onCreate) {
                Text(tr("folders.create"))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(tr("buttons.cancel"))
            }
        },
    )
}
