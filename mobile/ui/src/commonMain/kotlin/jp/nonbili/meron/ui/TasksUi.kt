package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.TaskSummary
import kotlinx.coroutines.launch
import sh.calvin.reorderable.ReorderableItem
import sh.calvin.reorderable.rememberReorderableLazyListState

/** Corner radius of the cards the Tasks and Calendar screens are built from. */
internal val ViewCardRadius = 16.dp

/** Side margin of those cards. */
internal val ViewCardMargin = 12.dp

/** The gap between two rows of a group, and how far their facing corners are
 *  rounded: the settings lists' own. */
internal val GroupedRowGap = 2.dp
internal val GroupedRowInnerRadius = 4.dp

/** The shape of row [index] of [count] stacked into one group: the group's
 *  outer corners are round, the ones between rows barely. */
internal fun groupedRowShape(
    index: Int,
    count: Int,
    radius: Dp = ViewCardRadius,
    inner: Dp = GroupedRowInnerRadius,
): Shape {
    val top = if (index == 0) radius else inner
    val bottom = if (index == count - 1) radius else inner
    return RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom)
}

/**
 * The Tasks screen body: an add field, the open tasks as one card, then the
 * completed ones in a collapsible card docked at the bottom, as in Google
 * Tasks.
 *
 * Open tasks reorder by long-pressing a row and dragging it.
 */
@Composable
internal fun TasksScreen(
    tasks: List<TaskSummary>,
    loading: Boolean,
    onAddTask: (String) -> Unit,
    onToggleDone: (TaskSummary, Boolean) -> Unit,
    onEditTask: (TaskSummary) -> Unit,
    onDeleteTask: (TaskSummary) -> Unit,
    onReorderTasks: (List<String>) -> Unit,
    onOpenMessage: (TaskSummary) -> Unit,
    modifier: Modifier = Modifier,
    listState: LazyListState = rememberLazyListState(),
) {
    var draft by remember { mutableStateOf("") }
    // Collapsed by default: done work is there to be found, not to crowd the
    // open tasks.
    var completedOpen by remember { mutableStateOf(false) }
    val completedListState = rememberLazyListState()
    val open = tasks.filterNot { it.done }
    val completed = tasks.filter { it.done }
    // The screen runs under the navigation bar: the open tasks scroll beneath
    // it, and the Completed card, when there is one, stands clear of it.
    val navigationBar = navigationBarInset()

    // A drag rearranges a local copy of the open task IDs and saves it on drop;
    // a reload that brings a different order replaces the copy. Only IDs are
    // kept so an edited task still shows its current content.
    val openIds = open.map { it.id }
    var orderIds by remember(openIds) { mutableStateOf(openIds) }
    val openById = open.associateBy { it.id }
    val order = orderIds.mapNotNull { openById[it] }
    val currentOpenIds by rememberUpdatedState(openIds)
    val haptics = LocalHapticFeedback.current
    val reorderState =
        rememberReorderableLazyListState(listState) { from, to ->
            orderIds = orderIds.toMutableList().apply { add(to.index, removeAt(from.index)) }
            haptics.performHapticFeedback(HapticFeedbackType.SegmentFrequentTick)
        }

    fun submit() {
        if (draft.isBlank()) return
        onAddTask(draft)
        draft = ""
    }

    Column(modifier.fillMaxSize()) {
        AddTaskField(
            draft = draft,
            onDraftChange = { draft = it },
            onSubmit = ::submit,
            modifier = Modifier.fillMaxWidth().padding(horizontal = ViewCardMargin).padding(vertical = 12.dp),
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
                // The Completed card sits below the scrolling list rather than
                // at its end, so a long list can't push it out of sight. Opened,
                // the completed tasks scroll on their own inside it, up to half
                // the screen, leaving the open tasks the rest.
                BoxWithConstraints(Modifier.fillMaxSize()) {
                    val completedMaxHeight = maxHeight / 2
                    Column(Modifier.fillMaxSize()) {
                        LazyColumn(
                            Modifier.weight(1f).fillMaxWidth().appScrollbar(listState),
                            state = listState,
                            verticalArrangement = Arrangement.spacedBy(GroupedRowGap),
                            contentPadding =
                                PaddingValues(
                                    start = ViewCardMargin,
                                    end = ViewCardMargin,
                                    bottom = 12.dp + if (completed.isEmpty()) navigationBar else 0.dp,
                                ),
                        ) {
                            itemsIndexed(order, key = { _, task -> task.id }) { index, task ->
                                ReorderableItem(reorderState, key = task.id) { dragging ->
                                    Surface(
                                        color = if (dragging) MaterialTheme.colorScheme.surfaceContainerHigh else MaterialTheme.colorScheme.surface,
                                        shape = if (dragging) RoundedCornerShape(ViewCardRadius) else groupedRowShape(index, order.size),
                                        shadowElevation = if (dragging) 6.dp else 0.dp,
                                        modifier =
                                            Modifier.longPressDraggableHandle(
                                                onDragStarted = {
                                                    haptics.performHapticFeedback(HapticFeedbackType.GestureThresholdActivate)
                                                },
                                                onDragStopped = {
                                                    if (orderIds != currentOpenIds) onReorderTasks(orderIds)
                                                },
                                            ),
                                    ) {
                                        Column {
                                            TaskRow(
                                                task = task,
                                                color = if (dragging) MaterialTheme.colorScheme.surfaceContainerHigh else MaterialTheme.colorScheme.surface,
                                                onToggleDone = { onToggleDone(task, it) },
                                                onEdit = { onEditTask(task) },
                                                onDelete = { onDeleteTask(task) },
                                                onOpenMessage = { onOpenMessage(task) },
                                            )
                                        }
                                    }
                                }
                            }
                        }

                        if (completed.isNotEmpty()) {
                            Surface(
                                color = MaterialTheme.colorScheme.surface,
                                shape = RoundedCornerShape(ViewCardRadius),
                                modifier = Modifier.fillMaxWidth().padding(horizontal = ViewCardMargin).padding(bottom = 8.dp + navigationBar),
                            ) {
                                Column {
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
                                            itemsIndexed(completed, key = { _, task -> task.id }) { _, task ->
                                                TaskDivider()
                                                TaskRow(
                                                    task = task,
                                                    color = MaterialTheme.colorScheme.surface,
                                                    onToggleDone = { onToggleDone(task, it) },
                                                    onEdit = { onEditTask(task) },
                                                    onDelete = { onDeleteTask(task) },
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
    }
}

/** The quick-add field: a card of its own, with a send button once there is
 *  something to add. */
@Composable
private fun AddTaskField(
    draft: String,
    onDraftChange: (String) -> Unit,
    onSubmit: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    TextField(
        value = draft,
        onValueChange = onDraftChange,
        placeholder = { Text(tr("tasks.addTask"), color = colors.onSurfaceVariant) },
        leadingIcon = { Icon(Icons.Filled.Add, contentDescription = null, tint = colors.primary) },
        trailingIcon = {
            if (draft.isNotBlank()) {
                FilledIconButton(onClick = onSubmit, modifier = Modifier.padding(end = 6.dp).size(36.dp)) {
                    Icon(Icons.Filled.ArrowUpward, contentDescription = tr("buttons.save"), Modifier.size(20.dp))
                }
            }
        },
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onSubmit() }),
        shape = RoundedCornerShape(ViewCardRadius),
        colors =
            TextFieldDefaults.colors(
                focusedContainerColor = colors.surface,
                unfocusedContainerColor = colors.surface,
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent,
            ),
        modifier = modifier,
    )
}

/** The hairline between two tasks of a card, starting under the titles. */
@Composable
private fun TaskDivider() {
    HorizontalDivider(Modifier.padding(start = 52.dp), color = MaterialTheme.colorScheme.outlineVariant)
}

/** The "Completed" heading, with its count, that shows or hides the ticked
 *  tasks. */
@Composable
private fun CompletedHeader(
    count: Int,
    open: Boolean,
    onToggle: () -> Unit,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onToggle)
            .heightIn(min = 52.dp)
            .padding(start = 16.dp, end = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(tr("tasks.completed"), style = MaterialTheme.typography.titleSmall)
        Text(count.toString(), style = MaterialTheme.typography.titleSmall, color = muted, modifier = Modifier.weight(1f))
        Icon(
            if (open) Icons.Filled.KeyboardArrowDown else Icons.Filled.KeyboardArrowUp,
            contentDescription = null,
            tint = muted,
        )
    }
}

/** A task's tick: a ring that fills once the task is done. */
@Composable
private fun TaskCheck(
    done: Boolean,
    onToggle: (Boolean) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Box(
        Modifier.size(48.dp).clip(CircleShape).toggleable(value = done, role = Role.Checkbox, onValueChange = onToggle),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .size(22.dp)
                .clip(CircleShape)
                .then(
                    if (done) {
                        Modifier.background(colors.primary.copy(alpha = 0.7f))
                    } else {
                        Modifier.border(1.5.dp, colors.outline, CircleShape)
                    },
                ),
            contentAlignment = Alignment.Center,
        ) {
            if (done) Icon(Icons.Filled.Check, contentDescription = null, Modifier.size(15.dp), tint = colors.onPrimary)
        }
    }
}

/**
 * One task, swiped like a mail row: towards the end ticks it (or unticks a
 * done one), towards the start deletes it. [color] is what the row is drawn
 * on, kept under it as it slides.
 */
@Composable
private fun TaskRow(
    task: TaskSummary,
    color: Color,
    onToggleDone: (Boolean) -> Unit,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
    onOpenMessage: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val dismissState = rememberSwipeToDismissBoxState()
    val scope = rememberCoroutineScope()
    SwipeToDismissBox(
        state = dismissState,
        // The row springs back once the swipe has done its work, and leaves by
        // the list reloading rather than by sliding off.
        onDismiss = { value ->
            when (value) {
                SwipeToDismissBoxValue.StartToEnd -> {
                    onToggleDone(!task.done)
                }

                SwipeToDismissBoxValue.EndToStart -> {
                    onDelete()
                }

                SwipeToDismissBoxValue.Settled -> {}
            }
            scope.launch { dismissState.reset() }
        },
        backgroundContent = {
            val deleting = dismissState.dismissDirection == SwipeToDismissBoxValue.EndToStart
            Box(
                Modifier
                    .fillMaxSize()
                    .background(if (deleting) colors.errorContainer else colors.primaryContainer)
                    .padding(horizontal = 24.dp),
                contentAlignment = if (deleting) Alignment.CenterEnd else Alignment.CenterStart,
            ) {
                Icon(
                    when {
                        deleting -> Icons.Filled.Delete
                        task.done -> Icons.AutoMirrored.Filled.Undo
                        else -> Icons.Filled.Check
                    },
                    contentDescription = if (deleting) tr("tasks.deleteTask") else null,
                    tint = if (deleting) colors.onErrorContainer else colors.onPrimaryContainer,
                )
            }
        },
    ) {
        Box(Modifier.background(color)) {
            TaskRowContent(task = task, onToggleDone = onToggleDone, onEdit = onEdit, onOpenMessage = onOpenMessage)
        }
    }
}

/**
 * A task's row itself. Tapping it opens the task for editing; a linked
 * message gets its own button so getting back to the mail is one tap.
 */
@Composable
private fun TaskRowContent(
    task: TaskSummary,
    onToggleDone: (Boolean) -> Unit,
    onEdit: () -> Unit,
    onOpenMessage: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val overdue = taskIsOverdue(task.dueAt, hasTime = task.dueHasTime) && !task.done
    val secondary = colors.onSurfaceVariant

    Row(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onEdit)
            .heightIn(min = 56.dp)
            .padding(start = 4.dp, end = if (task.threadId.isNotBlank()) 4.dp else 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TaskCheck(done = task.done, onToggle = onToggleDone)

        Column(
            Modifier.weight(1f).padding(vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                task.title,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                textDecoration = if (task.done) TextDecoration.LineThrough else null,
                color = if (task.done) secondary else colors.onSurface,
            )
            if (task.notes.isNotBlank()) {
                Text(
                    task.notes,
                    style = MaterialTheme.typography.bodyMedium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    color = secondary,
                )
            }
            if (task.dueAt > 0) {
                val dueColor = if (overdue) colors.error else secondary
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    modifier =
                        Modifier
                            .padding(top = 4.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .background(if (overdue) colors.error.copy(alpha = 0.1f) else colors.surfaceContainerHigh)
                            .padding(horizontal = 8.dp, vertical = 3.dp),
                ) {
                    Icon(Icons.Filled.Event, contentDescription = null, Modifier.size(14.dp), tint = dueColor)
                    Text(
                        formatTaskDueDate(task.dueAt, hasTime = task.dueHasTime),
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
                    tint = colors.primary,
                )
            }
        }
    }
}
