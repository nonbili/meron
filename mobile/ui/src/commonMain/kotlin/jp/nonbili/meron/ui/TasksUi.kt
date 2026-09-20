package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.Mail
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.TaskSummary

/**
 * The Tasks screen body: an add field, the open tasks, then the completed ones.
 *
 * Reordering is a menu action rather than a drag, because nothing else in this
 * app drags — kanban columns move with the same up/down pair.
 */
@Composable
internal fun TasksScreen(
    tasks: List<TaskSummary>,
    loading: Boolean,
    showCompleted: Boolean,
    onAddTask: (String) -> Unit,
    onToggleDone: (TaskSummary, Boolean) -> Unit,
    onEditTask: (TaskSummary) -> Unit,
    onDeleteTask: (TaskSummary) -> Unit,
    onMoveTask: (TaskSummary, Int) -> Unit,
    onOpenMessage: (TaskSummary) -> Unit,
    modifier: Modifier = Modifier,
) {
    var draft by remember { mutableStateOf("") }
    val listState = rememberLazyListState()
    val open = tasks.filterNot { it.done }
    val completed = tasks.filter { it.done }

    Column(modifier.fillMaxSize()) {
        OutlinedTextField(
            value = draft,
            onValueChange = { draft = it },
            placeholder = { Text(tr("tasks.addTask")) },
            leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null) },
            trailingIcon = {
                if (draft.isNotBlank()) {
                    TextButton(onClick = {
                        onAddTask(draft)
                        draft = ""
                    }) {
                        Text(tr("buttons.save"))
                    }
                }
            },
            singleLine = true,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
        )

        when {
            loading && tasks.isEmpty() -> {
                LoadingState()
            }

            tasks.isEmpty() -> {
                EmptyState(
                    icon = Icons.Filled.Checklist,
                    title = tr("tasks.empty"),
                    text = tr("tasks.emptyHint"),
                )
            }

            else -> {
                LazyColumn(
                    Modifier.fillMaxSize().appScrollbar(listState),
                    state = listState,
                ) {
                    items(open, key = { it.id }) { task ->
                        TaskRow(
                            task = task,
                            canMoveUp = open.firstOrNull()?.id != task.id,
                            canMoveDown = open.lastOrNull()?.id != task.id,
                            onToggleDone = { onToggleDone(task, it) },
                            onEdit = { onEditTask(task) },
                            onDelete = { onDeleteTask(task) },
                            onMove = { onMoveTask(task, it) },
                            onOpenMessage = { onOpenMessage(task) },
                        )
                    }

                    if (showCompleted && completed.isNotEmpty()) {
                        item {
                            HorizontalDivider(Modifier.padding(vertical = 8.dp))
                            Text(
                                tr("tasks.completed"),
                                style = MaterialTheme.typography.labelMedium,
                                fontWeight = FontWeight.SemiBold,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
                            )
                        }
                        items(completed, key = { it.id }) { task ->
                            TaskRow(
                                task = task,
                                // Completed tasks are ordered by when they were
                                // ticked, so hand-ordering them means nothing.
                                canMoveUp = false,
                                canMoveDown = false,
                                onToggleDone = { onToggleDone(task, it) },
                                onEdit = { onEditTask(task) },
                                onDelete = { onDeleteTask(task) },
                                onMove = {},
                                onOpenMessage = { onOpenMessage(task) },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun TaskRow(
    task: TaskSummary,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onToggleDone: (Boolean) -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    onMove: (Int) -> Unit,
    onOpenMessage: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    val overdue = taskIsOverdue(task.dueAt) && !task.done

    Row(
        Modifier.fillMaxWidth().padding(start = 8.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = task.done, onCheckedChange = onToggleDone)

        Column(
            Modifier.weight(1f).padding(vertical = 4.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                task.title,
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                textDecoration = if (task.done) TextDecoration.LineThrough else null,
                color =
                    if (task.done) {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    } else {
                        MaterialTheme.colorScheme.onSurface
                    },
            )
            if (task.notes.isNotBlank()) {
                Text(
                    task.notes,
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (task.dueAt > 0) {
                Text(
                    formatTaskDueDate(task.dueAt),
                    style = MaterialTheme.typography.labelSmall,
                    color =
                        if (overdue) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }

        Box {
            IconButton(onClick = { menuOpen = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"), Modifier.size(20.dp))
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                DropdownMenuItem(
                    text = { Text(tr("buttons.edit")) },
                    leadingIcon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                    onClick = {
                        menuOpen = false
                        onEdit()
                    },
                )
                if (task.threadId.isNotBlank()) {
                    DropdownMenuItem(
                        text = { Text(tr("tasks.openMessage")) },
                        leadingIcon = { Icon(Icons.Filled.Mail, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            onOpenMessage()
                        },
                    )
                }
                if (canMoveUp) {
                    DropdownMenuItem(
                        text = { Text(tr("tasks.moveUp")) },
                        leadingIcon = { Icon(Icons.Filled.KeyboardArrowUp, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            onMove(-1)
                        },
                    )
                }
                if (canMoveDown) {
                    DropdownMenuItem(
                        text = { Text(tr("tasks.moveDown")) },
                        leadingIcon = { Icon(Icons.Filled.KeyboardArrowDown, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            onMove(1)
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(tr("tasks.deleteTask")) },
                    leadingIcon = { Icon(Icons.Filled.Delete, contentDescription = null) },
                    onClick = {
                        menuOpen = false
                        onDelete()
                    },
                )
            }
        }
    }
}
