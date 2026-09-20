package jp.nonbili.meron.ui

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.TaskClearCompletedParams
import jp.nonbili.meron.shared.TaskCreateParams
import jp.nonbili.meron.shared.TaskDeleteParams
import jp.nonbili.meron.shared.TaskItemsParams
import jp.nonbili.meron.shared.TaskListCreateParams
import jp.nonbili.meron.shared.TaskListDeleteParams
import jp.nonbili.meron.shared.TaskListRenameParams
import jp.nonbili.meron.shared.TaskReorderParams
import jp.nonbili.meron.shared.TaskRestoreParams
import jp.nonbili.meron.shared.TaskSetDoneParams
import jp.nonbili.meron.shared.TaskSummary
import jp.nonbili.meron.shared.TaskUpdateParams
import jp.nonbili.meron.shared.parseNotificationThreadId
import jp.nonbili.meron.shared.parseTaskListsResponse
import jp.nonbili.meron.shared.parseTaskRestorePayload
import jp.nonbili.meron.shared.parseTasksResponse
import jp.nonbili.meron.shared.requireCoreOk
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The Tasks screen's state transitions.
 *
 * Every question about what a task *is* — ordering, which list a new one lands
 * in, when a completed task disappears — is answered by meron-core, the same
 * way it answers them for desktop. This file only asks and repaints.
 */

/**
 * Run one core call off the main thread, returning null when it fails.
 *
 * A failed task read or write is not worth a dialog: the list simply doesn't
 * change, and the next action tries again. Callers that do have something to
 * say — "added to Tasks" — check the result themselves.
 */
private suspend fun <T> MeronMobileState.onCore(block: suspend (MobileMailCommandClient) -> T): T? = runCatching { withContext(ioDispatcher) { block(MobileMailCommandClient(core)) } }.getOrNull()

/** Open the Tasks screen, on [listId] or the list the core defaults to. */
internal suspend fun MeronMobileState.openTasks(listId: String = "") {
    val response = onCore { requireCoreOk(it.taskLists()) } ?: return
    val parsed = parseTaskListsResponse(response)
    taskLists = parsed.lists
    val target =
        parsed.lists.firstOrNull { it.id == listId }?.id
            ?: parsed.lists.firstOrNull { it.id == activeTaskListId }?.id
            ?: parsed.defaultListId
    if (target.isBlank()) return
    activeTaskListId = target
    screen = Screen.Tasks
    loadTasks(target)
}

internal suspend fun MeronMobileState.selectTaskList(listId: String) {
    if (listId == activeTaskListId) return
    activeTaskListId = listId
    loadTasks(listId)
}

/** Re-read one list. The core owns the order, so the result is never re-sorted. */
internal suspend fun MeronMobileState.loadTasks(listId: String = activeTaskListId) {
    if (listId.isBlank()) return
    tasksLoading = true
    try {
        val response =
            onCore {
                requireCoreOk(it.taskItems(TaskItemsParams(listId, includeCompleted = showCompletedTasks)))
            } ?: return
        // A list the user switched away from while this was in flight must not
        // paint over the one they are now looking at.
        if (activeTaskListId != listId) return
        tasks = parseTasksResponse(response)
    } finally {
        tasksLoading = false
    }
}

internal suspend fun MeronMobileState.setShowCompletedTasks(show: Boolean) {
    showCompletedTasks = show
    loadTasks()
}

internal suspend fun MeronMobileState.addTask(title: String) {
    val listId = activeTaskListId
    if (listId.isBlank() || title.isBlank()) return
    onCore { requireCoreOk(it.createTask(TaskCreateParams(listId = listId, title = title.trim()))) }
    loadTasks(listId)
}

/**
 * File a message as a task. There is no list picker on a thread screen, so the
 * core puts it in the default list.
 */
internal suspend fun MeronMobileState.addTaskFromMessage(
    title: String,
    account: String,
    threadId: String,
    messageId: String = "",
) {
    val created =
        onCore {
            requireCoreOk(
                it.createTask(
                    TaskCreateParams(
                        title = title,
                        account = account,
                        threadId = threadId,
                        messageId = messageId,
                    ),
                ),
            )
        }
    if (created != null) status = trs("tasks.addedFromMessage")
    if (screen == Screen.Tasks) loadTasks()
}

internal suspend fun MeronMobileState.setTaskDone(
    taskId: String,
    done: Boolean,
) {
    onCore { requireCoreOk(it.setTaskDone(TaskSetDoneParams(taskId, done))) }
    loadTasks()
}

internal suspend fun MeronMobileState.updateTask(
    taskId: String,
    title: String? = null,
    notes: String? = null,
    dueAt: Long? = null,
) {
    onCore { requireCoreOk(it.updateTask(TaskUpdateParams(taskId, title, notes, dueAt))) }
    loadTasks()
}

internal suspend fun MeronMobileState.deleteTask(taskId: String) {
    val response = onCore { requireCoreOk(it.deleteTask(TaskDeleteParams(taskId))) }
    loadTasks()
    offerTaskUndo(trs("tasks.taskDeleted"), response)
}

/**
 * Pair a message with an Undo that hands the core back what it just removed.
 *
 * Tasks live only on this device — no server holds a copy — so a delete with no
 * way back is the one action here that can lose work outright. The restore blob
 * is opaque: whatever the delete returned goes back untouched.
 */
private fun MeronMobileState.offerTaskUndo(
    message: String,
    deleteResponse: String?,
) {
    val restore = deleteResponse?.let(::parseTaskRestorePayload)
    if (restore == null) {
        status = message
        return
    }
    scope.launch {
        val result =
            snackbarHost.showSnackbar(
                message = message,
                actionLabel = trs("buttons.undo"),
                duration = SnackbarDuration.Long,
            )
        if (result != SnackbarResult.ActionPerformed) return@launch
        onCore { requireCoreOk(it.restoreTasks(TaskRestoreParams(restore))) }
        // A restored list has to reappear in the picker, not just its tasks.
        refreshTaskLists()
        if (activeTaskListId.isBlank()) {
            taskLists.firstOrNull()?.let { activeTaskListId = it.id }
        }
        loadTasks()
    }
}

/**
 * Nudge one task up or down. Mobile has no drag-and-drop anywhere in the app,
 * so reordering is a menu action, as it already is for kanban columns.
 */
internal suspend fun MeronMobileState.moveTask(
    taskId: String,
    delta: Int,
) {
    val listId = activeTaskListId
    if (listId.isBlank()) return
    // Completed tasks are ordered by when they were ticked, not by hand, so only
    // the open ones take part.
    val ids = tasks.filterNot { it.done }.map { it.id }.toMutableList()
    val from = ids.indexOf(taskId)
    val to = from + delta
    if (from < 0 || to < 0 || to >= ids.size) return
    ids.add(to, ids.removeAt(from))
    onCore { requireCoreOk(it.reorderTasks(TaskReorderParams(listId, ids))) }
    loadTasks(listId)
}

internal suspend fun MeronMobileState.clearCompletedTasks() {
    val listId = activeTaskListId
    if (listId.isBlank()) return
    val cleared = onCore { requireCoreOk(it.clearCompletedTasks(TaskClearCompletedParams(listId))) }
    loadTasks(listId)
    offerTaskUndo(trs("tasks.completedCleared"), cleared)
}

internal suspend fun MeronMobileState.createTaskList(title: String) {
    onCore { requireCoreOk(it.createTaskList(TaskListCreateParams(title))) }
    val response = onCore { requireCoreOk(it.taskLists()) } ?: return
    val parsed = parseTaskListsResponse(response)
    taskLists = parsed.lists
    // The new list sorts last, so that is the one to open.
    parsed.lists.lastOrNull()?.let { selectTaskList(it.id) }
}

internal suspend fun MeronMobileState.renameTaskList(
    listId: String,
    title: String,
) {
    if (title.isBlank()) return
    onCore { requireCoreOk(it.renameTaskList(TaskListRenameParams(listId, title.trim()))) }
    refreshTaskLists()
}

internal suspend fun MeronMobileState.deleteTaskList(listId: String) {
    val name = taskLists.firstOrNull { it.id == listId }?.title.orEmpty()
    val response = onCore { requireCoreOk(it.deleteTaskList(TaskListDeleteParams(listId))) }
    refreshTaskLists()
    val next = taskLists.firstOrNull()?.id.orEmpty()
    activeTaskListId = next
    if (next.isBlank()) tasks = emptyList() else loadTasks(next)
    offerTaskUndo(trs("tasks.listDeleted", mapOf("name" to name)), response)
}

private suspend fun MeronMobileState.refreshTaskLists() {
    val response = onCore { requireCoreOk(it.taskLists()) } ?: return
    taskLists = parseTaskListsResponse(response).lists
}

/**
 * Follow a task back to the message it was made from.
 *
 * The task keeps only the composite thread id, so it is split back into the
 * account, folder and key a thread-open needs. This reuses the notification
 * tap-through, which already knows how to find one thread in a mailbox it has
 * not loaded yet.
 */
internal fun MeronMobileState.openTaskThread(task: TaskSummary) {
    val parsed = parseNotificationThreadId(task.threadId) ?: return
    openNotificationThread(
        NotificationThreadTarget(
            accountId = parsed.accountId.ifBlank { task.account },
            folder = parsed.folder,
            threadKey = parsed.threadKey,
        ),
    )
}

/** A due date as `YYYY-MM-DD` for the edit field, blank when there is none. */
internal fun formatTaskDueInput(epochSeconds: Long): String = if (epochSeconds <= 0) "" else formatDate(epochSeconds * 1000, DateStyle.IsoDate)

/** The inverse: a typed `YYYY-MM-DD` as epoch seconds, 0 for blank or invalid. */
internal fun parseTaskDueInput(value: String): Long {
    val parts = value.trim().split("-")
    if (parts.size != 3) return 0
    val year = parts[0].toIntOrNull() ?: return 0
    val month = parts[1].toIntOrNull() ?: return 0
    val day = parts[2].toIntOrNull() ?: return 0
    if (month !in 1..12 || day !in 1..31) return 0
    return epochSecondsForLocalDate(year, month, day)
}
