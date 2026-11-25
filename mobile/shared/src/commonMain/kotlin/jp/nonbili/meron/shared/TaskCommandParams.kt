package jp.nonbili.meron.shared

// ---- Tasks ------------------------------------------------------------------

data class TaskItemsParams(
    val listId: String,
    val includeCompleted: Boolean = false,
) {
    fun toJson(): String =
        jsonObject(
            "list_id" to listId.jsonString(),
            "include_completed" to includeCompleted.toString(),
        )
}

data class TaskListCreateParams(
    val title: String,
) {
    fun toJson(): String = jsonObject("title" to title.jsonString())
}

data class TaskListRenameParams(
    val listId: String,
    val title: String,
) {
    fun toJson(): String =
        jsonObject(
            "list_id" to listId.jsonString(),
            "title" to title.jsonString(),
        )
}

data class TaskListDeleteParams(
    val listId: String,
) {
    fun toJson(): String = jsonObject("list_id" to listId.jsonString())
}

/** Every task filed from a thread, done or not. */
data class TaskForThreadParams(
    val threadId: String,
) {
    fun toJson(): String = jsonObject("thread_id" to threadId.jsonString())
}

/**
 * A new task. [listId] may be blank — the core then files it under the default
 * list, which is what "add this message to Tasks" wants, since that action has
 * no list picker to offer.
 */
data class TaskCreateParams(
    val listId: String = "",
    val title: String,
    val notes: String = "",
    val dueAt: Long = 0,
    val account: String = "",
    val threadId: String = "",
    val messageId: String = "",
    val dueHasTime: Boolean = false,
) {
    fun toJson(): String =
        jsonObject(
            "list_id" to listId.jsonString(),
            "title" to title.jsonString(),
            "notes" to notes.jsonString(),
            "due_at" to dueAt.toString(),
            "due_has_time" to dueHasTime.takeIf { it }?.toString(),
            "account" to account.jsonString(),
            "thread_id" to threadId.jsonString(),
            "message_id" to messageId.jsonString(),
        )
}

/** A patch: a null field is left alone rather than cleared. */
data class TaskUpdateParams(
    val taskId: String,
    val title: String? = null,
    val notes: String? = null,
    val dueAt: Long? = null,
    val listId: String? = null,
    val dueHasTime: Boolean? = null,
) {
    fun toJson(): String =
        jsonObject(
            "task_id" to taskId.jsonString(),
            "title" to title?.jsonString(),
            "notes" to notes?.jsonString(),
            "due_at" to dueAt?.toString(),
            "due_has_time" to dueHasTime?.toString(),
            "list_id" to listId?.jsonString(),
        )
}

/** [message] is the thread message's `id` and [threadId] its conversation's,
 * which the calendar event links back to; [response] is `accepted`,
 * `tentative` or `declined`. */
data class InviteRespondParams(
    val message: String,
    val response: String,
    val threadId: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "message" to message.jsonString(),
            "thread_id" to threadId.jsonString(),
            "response" to response.jsonString(),
        )
}

/** A calendar range in epoch seconds, `[from, to)`. */
data class CalendarRangeParams(
    val from: Long,
    val to: Long,
    val includeTasks: Boolean,
) {
    fun toJson(): String =
        jsonObject(
            "from" to from.toString(),
            "to" to to.toString(),
            "include_tasks" to includeTasks.toString(),
        )
}

/**
 * An event to create, or (with [eventId]) to change. Times are epoch seconds;
 * an all-day event's are UTC midnights with [end] exclusive. [repeat] is
 * `daily`, `weekly`, `monthly`, `yearly`, or empty for once.
 */
data class CalendarEventParams(
    val eventId: String = "",
    val title: String,
    val notes: String = "",
    val location: String = "",
    val start: Long,
    val end: Long,
    val allDay: Boolean = false,
    val repeat: String = "",
    /** Editing a repeating event: the start of the occurrence the editor was
     *  opened on, so the core moves the series by the change rather than
     *  restarting it there. 0 for none. */
    val occurrenceStart: Long = 0,
    /** Minutes before start, at most two; null follows the app-wide default
     *  and an empty list means none. */
    val alerts: List<Int>? = null,
    /** The synced calendar it goes in; empty for this device. On update, a
     *  different calendar moves it there; null leaves it where it is. */
    val calendar: String? = null,
    /** On update, the event as the editor first showed it: fields left as
     *  they were keep what a sync brought in meanwhile. */
    val original: CalendarEventParams? = null,
    /** Whether [repeat] was moved with the start here. Mobile cannot place a
     *  series' days in the series' own zone, so it says false and leaves
     *  moving a rule left as it was to the core. Null when not editing. */
    val repeatAsShown: Boolean? = null,
    /** The series' first day (`YYYY-MM-DD`) a rule chosen here was placed
     *  for; the core moves it to the first day in the series' own zone. */
    val repeatFrom: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "event_id" to eventId.takeIf { it.isNotEmpty() }?.jsonString(),
            "calendar" to calendar?.jsonString(),
            "original" to original?.copy(eventId = "", original = null, repeatAsShown = null, repeatFrom = null)?.toJson(),
            "repeat_from" to repeatFrom?.jsonString(),
            "repeat_as_shown" to repeatAsShown?.toString(),
            "alerts" to (alerts?.joinToString(separator = ",", prefix = "[", postfix = "]") ?: "null"),
            "occurrence_start" to occurrenceStart.takeIf { it > 0 }?.toString(),
            "title" to title.jsonString(),
            "notes" to notes.jsonString(),
            "location" to location.jsonString(),
            "start" to start.toString(),
            "end" to end.toString(),
            "all_day" to allDay.toString(),
            "repeat" to repeat.jsonString(),
        )
}

data class TaskSetDoneParams(
    val taskId: String,
    val done: Boolean,
) {
    fun toJson(): String =
        jsonObject(
            "task_id" to taskId.jsonString(),
            "done" to done.toString(),
        )
}

data class TaskDeleteParams(
    val taskId: String,
) {
    fun toJson(): String = jsonObject("task_id" to taskId.jsonString())
}

data class TaskReorderParams(
    val listId: String,
    val taskIds: List<String>,
) {
    fun toJson(): String =
        jsonObject(
            "list_id" to listId.jsonString(),
            "task_ids" to taskIds.jsonStringArray(),
        )
}

/**
 * Undo a delete, by handing back the `restore` blob the delete returned.
 *
 * [restore] is raw JSON, not a string value: the client never looks inside it,
 * so it travels from the delete response to here untouched rather than being
 * parsed into fields and rebuilt.
 */
data class TaskRestoreParams(
    val restore: String,
) {
    fun toJson(): String = jsonObject("restore" to restore)
}

data class TaskClearCompletedParams(
    val listId: String,
) {
    fun toJson(): String = jsonObject("list_id" to listId.jsonString())
}
