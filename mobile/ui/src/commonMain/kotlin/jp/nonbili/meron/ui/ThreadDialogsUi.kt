package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ThreadSummary

@Composable
internal fun MoveThreadDialog(
    thread: ThreadSummary,
    folders: List<FolderSummary>,
    onMove: (FolderSummary) -> Unit,
    onCreateAndMove: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val unusedCreateAndMove = onCreateAndMove
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("threads.moveConversation")) },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item {
                    Text(
                        thread.subject.ifBlank { tr("threads.noSubject") },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (folders.isEmpty()) {
                    item {
                        Text(
                            tr("folders.noOtherAvailable"),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                } else {
                    items(folders, key = { "${it.accountId}\n${it.name}" }) { folder ->
                        FolderDestinationAction(folder) {
                            onMove(folder)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) }
        },
    )
    unusedCreateAndMove
}

@Composable
internal fun CopyThreadDialog(
    thread: ThreadSummary,
    folders: List<FolderSummary>,
    accounts: List<AccountSummary> = emptyList(),
    onCopy: (FolderSummary) -> Unit,
    onDismiss: () -> Unit,
) {
    val foldersByAccount = folders.groupBy { it.accountId.ifBlank { thread.accountId } }
    val accountGroups =
        if (accounts.isNotEmpty()) {
            accounts.map { account -> account.id to account.displayName.ifBlank { account.email.ifBlank { account.id } } }
        } else {
            foldersByAccount.keys.map { accountId -> accountId to accountId }
        }
    val copyTargetCount =
        accountGroups.sumOf { (accountId, _) ->
            foldersByAccount[accountId]
                .orEmpty()
                .count { folder ->
                    accountId != thread.accountId ||
                        !folder.name.equals(thread.folder, ignoreCase = true)
                }
        }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("threads.copyConversation")) },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item {
                    Text(
                        thread.subject.ifBlank { tr("threads.noSubject") },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (copyTargetCount == 0) {
                    item {
                        Text(
                            tr("folders.noCopyTargets"),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                } else {
                    accountGroups.forEach { (accountId, accountLabel) ->
                        val accountFolders =
                            foldersByAccount[accountId]
                                .orEmpty()
                                .filterNot { folder ->
                                    accountId == thread.accountId &&
                                        folder.name.equals(thread.folder, ignoreCase = true)
                                }
                        item(key = "account:$accountId") {
                            Text(
                                accountLabel,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                fontWeight = FontWeight.SemiBold,
                                fontSize = 12.sp,
                                modifier = Modifier.padding(start = 8.dp, top = 12.dp, end = 8.dp, bottom = 2.dp),
                            )
                        }
                        if (accountFolders.isEmpty()) {
                            item(key = "empty:$accountId") {
                                Text(
                                    tr("folders.noneAvailable"),
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    style = MaterialTheme.typography.bodySmall,
                                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
                                )
                            }
                        } else {
                            items(accountFolders, key = { "$accountId\n${it.name}" }) { folder ->
                                FolderDestinationAction(folder) {
                                    onCopy(folder)
                                }
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) }
        },
    )
}

@Composable
internal fun FolderDestinationAction(
    folder: FolderSummary,
    onClick: () -> Unit,
) {
    val icon = folderIcon(folder.name)
    TextButton(onClick = onClick, modifier = Modifier.fillMaxWidth()) {
        Icon(
            icon,
            contentDescription = null,
            modifier = Modifier.size(18.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.width(10.dp))
        Text(
            folder.name.replaceFirstChar { it.uppercase() },
            modifier = Modifier.weight(1f),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
