package jp.nonbili.meron.ui

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import jp.nonbili.meron.shared.CalendarEventParams
import jp.nonbili.meron.shared.CalendarItem
import jp.nonbili.meron.shared.CalendarRangeParams
import jp.nonbili.meron.shared.InviteRespondParams
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MessageInvite
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.parseCalendarEventIdResponse
import jp.nonbili.meron.shared.parseCalendarEventsResponse
import jp.nonbili.meron.shared.parseCalendarRestorePayload
import jp.nonbili.meron.shared.requireCoreOk
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private suspend fun <T> MeronMobileState.onCalendarCore(block: suspend (MobileMailCommandClient) -> T): Result<T> = runCatching { withContext(ioDispatcher) { block(MobileMailCommandClient(core)) } }

/** Local midnight of [date], as epoch seconds. */
internal fun localMidnight(date: CivilDate): Long = epochSecondsForLocalDate(date.year, date.month, date.day)

/** The six weeks the calendar screen shows for its month. */
internal fun MeronMobileState.calendarGrid(): List<CivilDate> = monthGrid(calendarYear, calendarMonth, firstDayOfWeek())

/** Open the Calendar screen on the month it was last on. */
internal suspend fun MeronMobileState.openCalendar() {
    screen = Screen.Calendar
    loadCalendar()
}

/** Re-read the month on show. A read that finishes after the month moved on
 *  is dropped, so paging quickly lands on the month the user stopped at. */
internal suspend fun MeronMobileState.loadCalendar() {
    if (!coreLoaded) return
    val month = calendarYear to calendarMonth
    val (from, to) = gridRange(calendarGrid(), ::localMidnight)
    onCalendarCore { requireCoreOk(it.calendarEvents(CalendarRangeParams(from, to, includeTasks = tasksEnabled))) }
        .onSuccess { if (calendarYear to calendarMonth == month) calendarItems = parseCalendarEventsResponse(it) }
        .onFailure { status = trsError("calendar.loadFailed", it) }
}

internal suspend fun MeronMobileState.shiftCalendarMonth(months: Int) {
    val (year, month) = shiftMonth(calendarYear, calendarMonth, months)
    calendarYear = year
    calendarMonth = month
    loadCalendar()
}

internal suspend fun MeronMobileState.calendarToToday() {
    val today = localDateParts(currentTimeMillis())
    calendarYear = today.year
    calendarMonth = today.month
    loadCalendar()
}

/** Create an event, or change one when [CalendarEventParams.eventId] is set.
 *  True when it saved. */
internal suspend fun MeronMobileState.saveCalendarEvent(params: CalendarEventParams): Boolean {
    val result =
        onCalendarCore {
            requireCoreOk(if (params.eventId.isEmpty()) it.createCalendarEvent(params) else it.updateCalendarEvent(params))
        }
    result.onFailure { status = trsError("calendar.saveFailed", it) }
    loadCalendar()
    return result.isSuccess
}

/** Delete an event, with an Undo that hands the core back what it removed:
 *  the calendar lives only on this device, so a delete is otherwise final. */
internal suspend fun MeronMobileState.deleteCalendarEvent(eventId: String) {
    val response =
        onCalendarCore { requireCoreOk(it.deleteCalendarEvent(eventId)) }
            .onFailure { status = trsError("calendar.saveFailed", it) }
            .getOrNull() ?: return
    messages =
        messages.map { message ->
            val invite = message.invite
            if (invite?.eventId == eventId) message.copy(invite = invite.copy(eventId = "")) else message
        }
    loadCalendar()
    val restore = parseCalendarRestorePayload(response) ?: return
    scope.launch {
        val result =
            snackbarHost.showSnackbar(
                message = trs("calendar.eventDeleted"),
                actionLabel = trs("buttons.undo"),
                duration = SnackbarDuration.Long,
            )
        if (result != SnackbarResult.ActionPerformed) return@launch
        onCalendarCore { requireCoreOk(it.restoreCalendarEvent(restore)) }
        loadCalendar()
    }
}

/** Take a cancelled invitation out of the calendar: the occurrence it names,
 *  or the whole event when the series is cancelled. Nothing is sent. */
internal suspend fun MeronMobileState.removeInviteFromCalendar(message: MessageBody) {
    val invite = message.invite ?: return
    val eventId = invite.eventId.takeIf { it.isNotBlank() } ?: return
    onCalendarCore { requireCoreOk(it.removeInviteFromCalendar(message.id)) }
        .onFailure { status = trsError("calendar.saveFailed", it) }
        .onSuccess {
            messages =
                messages.map { loaded ->
                    val other = loaded.invite
                    // The whole event went, or only this occurrence: other
                    // cancelled dates of the series can still be taken out.
                    val removed =
                        other?.eventId == eventId &&
                            (invite.recurrenceId == 0L || (other.uid == invite.uid && other.recurrenceId == invite.recurrenceId))
                    if (removed && other != null) loaded.copy(invite = other.copy(eventId = "")) else loaded
                }
            if (screen == Screen.Calendar) loadCalendar()
        }
}

/** Put an answered invitation back in the calendar — after its event was
 *  deleted — with the answer already given and nothing sent. */
internal suspend fun MeronMobileState.addInviteToCalendar(message: MessageBody) {
    if (message.invite == null) return
    val threadId = selectedCoreThread?.backendThreadId().orEmpty()
    onCalendarCore { requireCoreOk(it.addInviteToCalendar(message.id, threadId)) }
        .onFailure { status = trsError("calendar.saveFailed", it) }
        .onSuccess { response ->
            val eventId = parseCalendarEventIdResponse(response)
            updateInvite(message) { it.copy(eventId = eventId) }
            if (screen == Screen.Calendar) loadCalendar()
        }
}

/** What tapping a task in the calendar does: open it, in its list. */
internal suspend fun MeronMobileState.openCalendarTask(item: CalendarItem) {
    pendingTaskEditId = item.id
    openTasks(item.listId)
}

/** Apply [change] to every loaded copy of [message]'s invitation — the
 *  original, an update to it, the same message in another folder — and only
 *  of the same occurrence: an answer about one date is not one about the
 *  series. */
private fun MeronMobileState.updateInvite(
    message: MessageBody,
    change: (MessageInvite) -> MessageInvite,
) {
    val invite = message.invite ?: return
    messages =
        messages.map { loaded ->
            val other = loaded.invite
            if (other != null && other.uid == invite.uid && other.recurrenceId == invite.recurrenceId) {
                loaded.copy(invite = change(other))
            } else {
                loaded
            }
        }
}

/**
 * Answer an invitation: the core sends the organizer an iMIP reply, records
 * the answer and files the event in the calendar. The card shows it straight
 * away and goes back if the send fails. Mirrors desktop's `respondToInvite`.
 */
internal suspend fun MeronMobileState.respondToInvite(
    message: MessageBody,
    response: String,
) {
    val invite = message.invite ?: return
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val previous = invite.response
    updateInvite(message) { it.copy(response = response) }
    val threadId = selectedCoreThread?.backendThreadId().orEmpty()
    onCalendarCore { requireCoreOk(it.respondToInvite(InviteRespondParams(message.id, response, threadId))) }
        .onSuccess { result ->
            // Filed in the calendar now, or (declined) taken out of it. A
            // declined series takes its event out from under every message
            // that pointed at it; a declined occurrence leaves the series, and
            // the other dates, in place.
            val eventId = parseCalendarEventIdResponse(result)
            if (eventId.isEmpty() && invite.eventId.isNotEmpty() && invite.recurrenceId == 0L) {
                messages =
                    messages.map { loaded ->
                        val other = loaded.invite
                        if (other?.eventId == invite.eventId) loaded.copy(invite = other.copy(eventId = "")) else loaded
                    }
            }
            updateInvite(message) { it.copy(eventId = eventId) }
            if (screen == Screen.Calendar) loadCalendar()
        }.onFailure {
            updateInvite(message) { it.copy(response = previous) }
            status = trsError("invite.respondFailed", it)
        }
}
