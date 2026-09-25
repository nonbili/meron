package jp.nonbili.meron.ui

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.RssThreadParams
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
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.parseNotificationThreadId
import jp.nonbili.meron.shared.parseTaskListsResponse
import jp.nonbili.meron.shared.parseTaskRestorePayload
import jp.nonbili.meron.shared.parseTasksResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/*
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
                requireCoreOk(it.taskItems(TaskItemsParams(listId, includeCompleted = true)))
            } ?: return
        // A list the user switched away from while this was in flight must not
        // paint over the one they are now looking at.
        if (activeTaskListId != listId) return
        tasks = parseTasksResponse(response)
    } finally {
        tasksLoading = false
    }
}

internal suspend fun MeronMobileState.addTask(title: String) {
    val listId = activeTaskListId
    if (listId.isBlank() || title.isBlank()) return
    onCore { requireCoreOk(it.createTask(TaskCreateParams(listId = listId, title = title.trim()))) }
    loadTasks(listId)
}

/**
 * File a message in the chosen list, retaining the mail link. Callers without
 * an explicit destination use the remembered list or the core default.
 */
internal suspend fun MeronMobileState.addTaskFromMessage(
    title: String,
    account: String,
    threadId: String,
    messageId: String = "",
    listId: String = activeTaskListId,
) {
    val created =
        onCore {
            requireCoreOk(
                it.createTask(
                    TaskCreateParams(
                        listId = listId,
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
 * The task keeps only the composite thread id, which the core's thread read
 * resolves by itself — across the account's folders, so a mail that has since
 * been archived or trashed still opens. Nothing here loads a mailbox: the
 * conversation is shown over whatever the user was looking at, as a
 * notification tap-through does.
 */
internal fun MeronMobileState.openTaskThread(task: TaskSummary) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val threadId = task.threadId
    if (threadId.isBlank()) return
    // Only for the account to authenticate with and a folder to fall back on:
    // the read itself resolves the thread across the account's folders, so a
    // message that was merely moved (archived, trashed) still opens.
    val parsed = parseNotificationThreadId(threadId)
    val accountId = parsed?.accountId?.takeIf { it.isNotBlank() } ?: task.account
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val response =
                    if (threadIdIsRss(threadId)) {
                        client.readRssThread(RssThreadParams(threadId = threadId))
                    } else {
                        withManagedGoogleAuth(client, accountId) {
                            client.readThread(ThreadReadParams(threadId = threadId))
                        }
                    }
                parseThreadReadPage(response).messages
            }
        }.onSuccess { messages ->
            val newest = messages.maxByOrNull { it.dateEpochSeconds }
            if (newest == null) {
                // The mail is gone, not merely elsewhere. Nothing is remembered
                // about that: a sync can bring the conversation back, and the
                // entry is worth pressing again then.
                status = trs("tasks.mailNotFound")
                return@onSuccess
            }
            readCoreThread(
                ThreadSummary(
                    id = threadId,
                    threadId = threadId,
                    accountId = accountId,
                    folder = newest.folderId.ifBlank { parsed?.folder.orEmpty() },
                    subject = newest.subject,
                    sender = newest.fromAddr.ifBlank { newest.from },
                    dateEpochSeconds = newest.dateEpochSeconds,
                ),
            )
        }.onFailure {
            status = "Could not open message: ${it.message}"
        }
    }
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

/**
 * A due date as the date picker holds it: UTC midnight of the same calendar
 * day, or null for none. The picker works in UTC; tasks store local midnight.
 */
internal fun taskDueToPickerMillis(epochSeconds: Long): Long? {
    val parts = formatTaskDueInput(epochSeconds).split("-").mapNotNull { it.toIntOrNull() }
    if (parts.size != 3) return null
    return daysFromCivil(parts[0], parts[1], parts[2]) * 86_400_000L
}

/** The inverse of [taskDueToPickerMillis]: the picked day at local midnight. */
internal fun pickerMillisToTaskDue(utcMillis: Long): Long {
    val (year, month, day) = civilFromDays(utcMillis.floorDiv(86_400_000L))
    return epochSecondsForLocalDate(year, month, day)
}

// Howard Hinnant's days-from-civil algorithms, proleptic Gregorian.
private fun daysFromCivil(
    year: Int,
    month: Int,
    day: Int,
): Long {
    val y = (if (month <= 2) year - 1 else year).toLong()
    val era = y.floorDiv(400L)
    val yoe = y - era * 400
    val doy = (153 * (if (month > 2) month - 3 else month + 9) + 2) / 5 + day - 1
    val doe = yoe * 365 + yoe / 4 - yoe / 100 + doy
    return era * 146_097 + doe - 719_468
}

private fun civilFromDays(days: Long): Triple<Int, Int, Int> {
    val z = days + 719_468
    val era = z.floorDiv(146_097L)
    val doe = z - era * 146_097
    val yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365
    val doy = doe - (365 * yoe + yoe / 4 - yoe / 100)
    val mp = (5 * doy + 2) / 153
    val day = (doy - (153 * mp + 2) / 5 + 1).toInt()
    val month = (if (mp < 10) mp + 3 else mp - 9).toInt()
    val year = (yoe + era * 400 + if (month <= 2) 1 else 0).toInt()
    return Triple(year, month, day)
}

/** Choose a destination explicitly when more than one list exists. */
internal suspend fun MeronMobileState.requestAddMailToTasks(thread: jp.nonbili.meron.shared.ThreadSummary) {
    val response = onCore { requireCoreOk(it.taskLists()) } ?: return
    taskLists = parseTaskListsResponse(response).lists
    if (taskLists.size == 1) {
        addTaskFromMessage(thread.subject, thread.accountId, thread.id, listId = taskLists.single().id)
    } else if (taskLists.size > 1) {
        pendingTaskThread = thread
    }
}

internal suspend fun MeronMobileState.pickMailTaskList(listId: String) {
    val thread = pendingTaskThread ?: return
    pendingTaskThread = null
    addTaskFromMessage(thread.subject, thread.accountId, thread.id, listId = listId)
}
