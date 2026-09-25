package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.outlined.Email
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
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.TaskSummary

/**
 * The Tasks screen body: an add field, the open tasks, then the completed ones
 * under a collapsible heading, as in Google Tasks.
 *
 * Reordering is a menu action rather than a drag, because nothing else in this
 * app drags — kanban columns move with the same up/down pair.
 */
@Composable
internal fun TasksScreen(
    tasks: List<TaskSummary>,
    loading: Boolean,
    onAddTask: (String) -> Unit,
    onToggleDone: (TaskSummary, Boolean) -> Unit,
    onEditTask: (TaskSummary) -> Unit,
    onDeleteTask: (TaskSummary) -> Unit,
    onMoveTask: (TaskSummary, Int) -> Unit,
    onOpenMessage: (TaskSummary) -> Unit,
    modifier: Modifier = Modifier,
) {
    var draft by remember { mutableStateOf("") }
    // Collapsed by default: done work is there to be found, not to crowd the
    // open tasks.
    var completedOpen by remember { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val completedListState = rememberLazyListState()
    val open = tasks.filterNot { it.done }
    val completed = tasks.filter { it.done }

    fun submit() {
        if (draft.isBlank()) return
        onAddTask(draft)
        draft = ""
    }

    Column(modifier.fillMaxSize()) {
        OutlinedTextField(
            value = draft,
            onValueChange = { draft = it },
            placeholder = { Text(tr("tasks.addTask")) },
            leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null) },
            trailingIcon = {
                if (draft.isNotBlank()) {
                    TextButton(onClick = ::submit) {
                        Text(tr("buttons.save"))
                    }
                }
            },
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { submit() }),
            shape = MaterialTheme.shapes.extraLarge,
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
                // The Completed heading sits below the scrolling list rather than
                // at its end, so a long list can't push it out of sight. Opened,
                // the completed tasks scroll on their own beneath it, up to half
                // the screen, leaving the open tasks the rest.
                BoxWithConstraints(Modifier.fillMaxSize()) {
                    val completedMaxHeight = maxHeight / 2
                    Column(Modifier.fillMaxSize()) {
                        LazyColumn(
                            Modifier.weight(1f).fillMaxWidth().appScrollbar(listState),
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
                        }

                        if (completed.isNotEmpty()) {
                            HorizontalDivider()
                            CompletedHeader(
                                count = completed.size,
                                open = completedOpen,
                                onToggle = { completedOpen = !completedOpen },
                            )
                            if (completedOpen) {
                                LazyColumn(
                                    Modifier
                                        .fillMaxWidth()
                                        .heightIn(max = completedMaxHeight)
                                        .appScrollbar(completedListState),
                                    state = completedListState,
                                ) {
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
        }
    }
}

/** The "Completed (N)" heading that shows or hides the ticked tasks. */
@Composable
private fun CompletedHeader(
    count: Int,
    open: Boolean,
    onToggle: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onToggle)
            .heightIn(min = 48.dp)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Icon(
            if (open) Icons.Filled.KeyboardArrowDown else Icons.Filled.KeyboardArrowRight,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            "${tr("tasks.completed")} ($count)",
            style = MaterialTheme.typography.titleSmall,
            fontWeight = FontWeight.Medium,
        )
    }
}

/**
 * One task. Tapping the row edits it; a linked message gets its own button so
 * getting back to the mail is one tap, and the rarer actions sit in the menu.
 */
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
    val secondary = MaterialTheme.colorScheme.onSurfaceVariant

    Row(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onEdit)
            .heightIn(min = 56.dp)
            .padding(start = 4.dp, end = 4.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(checked = task.done, onCheckedChange = onToggleDone)

        Column(
            Modifier.weight(1f).padding(vertical = 6.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                task.title,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                textDecoration = if (task.done) TextDecoration.LineThrough else null,
                color = if (task.done) secondary else MaterialTheme.colorScheme.onSurface,
            )
            if (task.notes.isNotBlank()) {
                Text(
                    task.notes,
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    color = secondary,
                )
            }
            if (task.dueAt > 0) {
                val dueColor = if (overdue) MaterialTheme.colorScheme.error else secondary
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    modifier = Modifier.padding(top = 2.dp),
                ) {
                    Icon(Icons.Filled.Event, contentDescription = null, Modifier.size(14.dp), tint = dueColor)
                    Text(
                        formatTaskDueDate(task.dueAt),
                        style = MaterialTheme.typography.labelMedium,
                        color = dueColor,
                    )
                }
            }
        }

        if (task.threadId.isNotBlank()) {
            IconButton(onClick = onOpenMessage) {
                Icon(
                    Icons.Outlined.Email,
                    contentDescription = tr("tasks.openMessage"),
                    Modifier.size(20.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
            }
        }

        Box {
            IconButton(onClick = { menuOpen = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"), Modifier.size(20.dp), tint = secondary)
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
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
