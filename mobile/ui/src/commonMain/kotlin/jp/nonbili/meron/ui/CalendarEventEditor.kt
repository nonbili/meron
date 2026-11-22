package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Notes
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material.icons.outlined.Place
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.CalendarEventParams
import jp.nonbili.meron.shared.CalendarItem
import kotlinx.coroutines.delay

/** What the event editor was opened on: a new event on a day, or an item. */
internal sealed interface CalendarEditorTarget {
    data class New(
        val day: CivilDate,
    ) : CalendarEditorTarget

    data class Edit(
        val item: CalendarItem,
    ) : CalendarEditorTarget
}

/** A synced event's own rule, which none of the editor's choices says: the
 *  core keeps it when this goes back. */
private const val SERVER_REPEAT = "custom"

/** The Repeat choice that opens the custom repeat editor. */
private const val CUSTOMIZE = "\u0000customize"

/** A repeating event made elsewhere whose rule the core cannot say as one. */
private fun hasServerRepeat(item: CalendarItem): Boolean = item.source == "event" && item.recurring && item.repeat.isBlank()

/** A calendar as the picker names it: with its account, since two accounts
 *  can each have a "Personal". A primary calendar already goes by its
 *  account's name. */
private fun calendarLabel(calendar: SyncedCalendar): String = if (calendar.accountName.isNotBlank() && calendar.name != calendar.accountName) "${calendar.name} (${calendar.accountName})" else calendar.name

private val repeats =
    listOf(
        "" to "calendar.repeat.none",
        "daily" to "calendar.repeat.daily",
        "weekly" to "calendar.repeat.weekly",
        "monthly" to "calendar.repeat.monthly",
        "yearly" to "calendar.repeat.yearly",
    )

/** A wall-clock time of day, in minutes since midnight. */
private fun minutesOf(epochSeconds: Long): Int = localDateParts(epochSeconds * 1000).let { it.hour * 60 + it.minute }

private fun localDayOf(epochSeconds: Long): CivilDate = localDateParts(epochSeconds * 1000).let { CivilDate(it.year, it.month, it.day) }

private fun formatMinutes(minutes: Int): String = formatDate(epochSecondsForLocalDateTime(2000, 1, 1, minutes / 60, minutes % 60) * 1000, DateStyle.Time)

private enum class Picking { StartDate, EndDate, StartTime, EndTime }

private const val MINUTES_PER_DAY = 24 * 60L

/** The editor's first alert: null follows the default, [NO_ALERT] is none. */
private fun firstAlert(alerts: List<Int>?): Int? = if (alerts == null) null else alerts.firstOrNull() ?: NO_ALERT

/**
 * Create or change an event made here. An accepted invitation opens read-only
 * but for its notes and alerts: its time and title are the organizer's to change.
 * Mirrors desktop's EventEditorDialog.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CalendarEventEditor(
    target: CalendarEditorTarget,
    defaultAlertMinutes: Int,
    defaultAllDayAlertDays: Int,
    saving: Boolean,
    onDismiss: () -> Unit,
    onSave: (CalendarEventParams) -> Unit,
    onDelete: (eventId: String) -> Unit,
    onOpenMail: (CalendarItem) -> Unit,
    /** Synced calendars, for the picker; the writable ones take events. */
    calendars: List<SyncedCalendar> = emptyList(),
    /** The calendar the last new event went in, offered first for the next. */
    lastCalendar: String = "",
    onExport: (eventId: String) -> Unit = {},
) {
    val item = (target as? CalendarEditorTarget.Edit)?.item
    val invite = item?.source == "invite"
    val synced = item?.calendar?.takeIf { it.isNotBlank() }?.let { id -> calendars.firstOrNull { it.id == id } }
    // A subscription or a calendar shared read-only: shown, not changed.
    val readOnly = synced != null && !synced.writable
    val writableCalendars = calendars.filter { it.writable }
    var calendar by remember {
        mutableStateOf(
            item?.calendar ?: lastCalendar.takeIf { last -> calendars.any { it.id == last && it.writable } }.orEmpty(),
        )
    }
    var customizing by remember { mutableStateOf(false) }
    var title by remember { mutableStateOf(item?.title.orEmpty()) }
    var notes by remember { mutableStateOf(item?.notes.orEmpty()) }
    var location by remember { mutableStateOf(item?.location.orEmpty()) }
    var allDay by remember { mutableStateOf(item?.allDay ?: false) }
    var repeat by remember { mutableStateOf(if (item != null && hasServerRepeat(item)) SERVER_REPEAT else item?.repeat.orEmpty()) }
    // An all-day item's bounds are UTC midnights with an exclusive end; here
    // the end is the last day, as the user names it.
    var startDay by remember {
        mutableStateOf(
            when {
                item == null -> (target as CalendarEditorTarget.New).day
                item.allDay -> CivilDate.ofUtcSeconds(item.start)
                else -> localDayOf(item.start)
            },
        )
    }
    var endDay by remember {
        mutableStateOf(
            when {
                item == null -> startDay
                item.allDay -> CivilDate.ofUtcSeconds(maxOf(item.start, item.end - DAY_SECONDS))
                else -> localDayOf(maxOf(item.start, item.end))
            },
        )
    }
    var startMinutes by remember { mutableStateOf(item?.takeUnless { it.allDay }?.let { minutesOf(it.start) } ?: (9 * 60)) }
    var endMinutes by remember { mutableStateOf(item?.takeUnless { it.allDay }?.let { minutesOf(maxOf(it.start, it.end)) } ?: (10 * 60)) }
    var picking by remember { mutableStateOf<Picking?>(null) }
    // What a custom repeat's days and end are counted from: the series' first
    // day, moved by as many days as the start has been moved here.
    val openedStartDay = remember { startDay }
    val openedSeriesDay = remember { item?.seriesDate?.let(::parseIsoDate) }

    fun seriesDayOf(day: CivilDate): CivilDate = openedSeriesDay?.plusDays(day.epochDay - openedStartDay.epochDay) ?: day

    // A rule chosen in this edit, and the first day it was chosen for: each
    // move of the start moves it, so moving back restores it exactly. The
    // event's own rule is left as it was for the core to move: it knows the
    // series' zone, where a time change can cross midnight.
    var anchor by remember { mutableStateOf<Pair<String, CivilDate>?>(null) }

    /** Move the start's day, a custom repeat chosen here moving with it. */
    fun moveStartDay(day: CivilDate) {
        startDay = day
        anchor?.let { (rule, from) -> repeat = alignRepeat(rule, from, seriesDayOf(day)) }
    }

    fun chooseRepeat(value: String) {
        // The event's own rule picked again is the one left as it was: the
        // core moves it with the series, wherever the start now is.
        anchor = if (item != null && value == item.repeat) null else value to seriesDayOf(startDay)
        repeat = value
    }
    var alert by remember { mutableStateOf(firstAlert(item?.alerts)) }
    var secondAlert by remember { mutableStateOf(item?.alerts?.getOrNull(1) ?: NO_ALERT) }

    fun alerts(): List<Int>? =
        when (val first = alert) {
            null -> null
            NO_ALERT -> emptyList()
            else -> listOfNotNull(first, secondAlert.takeIf { it != NO_ALERT })
        }

    fun params(): CalendarEventParams {
        val (start, end) =
            if (allDay) {
                startDay.utcSeconds to maxOf(startDay, endDay).utcSeconds + DAY_SECONDS
            } else {
                val start = epochSecondsForLocalDateTime(startDay.year, startDay.month, startDay.day, startMinutes / 60, startMinutes % 60)
                val end = epochSecondsForLocalDateTime(endDay.year, endDay.month, endDay.day, endMinutes / 60, endMinutes % 60)
                start to maxOf(start, end)
            }
        return CalendarEventParams(
            eventId = item?.id.orEmpty(),
            title = title,
            notes = notes,
            location = location,
            start = start,
            end = end,
            allDay = allDay,
            repeat = repeat,
            occurrenceStart = item?.takeIf { it.recurring }?.start ?: 0,
            alerts = alerts(),
            calendar = calendar,
        )
    }

    val colors = MaterialTheme.colorScheme
    val busy by rememberUpdatedState(saving)
    // Swiping the sheet away with changes in it asks first: there is no
    // Cancel, and an event half written is not one to save unasked.
    val opened = remember { params() }
    val dirty by rememberUpdatedState(params() != opened)
    var confirmDiscard by remember { mutableStateOf(false) }
    ModalBottomSheet(
        // Back gets here even when the sheet refused to hide, so the question
        // is asked here too.
        onDismissRequest = { if (dirty && !busy) confirmDiscard = true else onDismiss() },
        // A save in flight keeps the sheet up, as it keeps the editor.
        sheetState =
            rememberModalBottomSheetState(
                skipPartiallyExpanded = true,
                confirmValueChange = {
                    if (it == SheetValue.Hidden && dirty && !busy) confirmDiscard = true
                    it != SheetValue.Hidden || !(busy || dirty)
                },
            ),
        containerColor = colors.surface,
    ) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(bottom = 12.dp)) {
            // An invitation, or an event on a calendar this device may not
            // change: what it is, not fields to edit.
            if (item != null && (invite || readOnly)) {
                Column(
                    modifier = Modifier.padding(horizontal = 24.dp).padding(bottom = 12.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    Text(item.title.ifBlank { tr("invite.untitled") }, style = MaterialTheme.typography.titleLarge)
                    Text(
                        formatEventRange(item.start, item.end, item.allDay) + if (item.allDay) " · ${tr("invite.allDay")}" else "",
                        color = colors.onSurfaceVariant,
                    )
                    if (item.location.isNotBlank()) InviteLocation(item.location)
                    if (readOnly && synced != null) {
                        Text("${calendarLabel(synced)} · ${tr("calendar.readOnly")}", color = colors.onSurfaceVariant)
                    }
                }
            } else {
                TextField(
                    value = title,
                    onValueChange = { title = it },
                    placeholder = {
                        Text(tr("calendar.fields.title"), style = MaterialTheme.typography.titleLarge, color = colors.onSurfaceVariant)
                    },
                    textStyle = MaterialTheme.typography.titleLarge,
                    singleLine = true,
                    colors = bareFieldColors(),
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                )
                EditorRow(Icons.Filled.Schedule) {
                    Text(tr("invite.allDay"), modifier = Modifier.weight(1f))
                    Switch(
                        checked = allDay,
                        onCheckedChange = {
                            allDay = it
                            // Leads mean minutes for a timed event and days
                            // for an all-day one: start over from the default.
                            alert = null
                            secondAlert = NO_ALERT
                        },
                    )
                }
                DateTimeRow(
                    label = tr("calendar.fields.starts"),
                    day = startDay,
                    minutes = startMinutes.takeUnless { allDay },
                    onPickDate = { picking = Picking.StartDate },
                    onPickTime = { picking = Picking.StartTime },
                )
                DateTimeRow(
                    label = tr("calendar.fields.ends"),
                    day = endDay,
                    minutes = endMinutes.takeUnless { allDay },
                    onPickDate = { picking = Picking.EndDate },
                    onPickTime = { picking = Picking.EndTime },
                )
                // The plain choices, then the event's own rule and the one
                // being made, and last the way to make one.
                val repeatLabels =
                    buildList {
                        repeats.forEach { (value, key) -> add(value to tr(key)) }
                        if (item != null && hasServerRepeat(item)) {
                            add(SERVER_REPEAT to tr(if (item.calendar.isNotBlank()) "calendar.repeat.serverRule" else "calendar.repeat.custom"))
                        }
                        // The event's own rule as it opened only while its first
                        // day has not moved; moved, the rule shown is it moved along.
                        // The event's own rule, which the core moves with the series.
                        val ownRule = item?.repeat.orEmpty()
                        val locale = LocalAppLocale.current
                        for (value in setOf(ownRule, repeat)) {
                            // The event's own rule is named as the core will save
                            // it, moved with the start; its value stays the rule
                            // as it opened, for the core to move.
                            val shown =
                                if (value == ownRule && anchor == null && openedSeriesDay != null) {
                                    alignRepeat(value, openedSeriesDay, seriesDayOf(startDay))
                                } else {
                                    value
                                }
                            val rule = if (isPlainRepeat(shown)) null else parseRepeat(shown)
                            if (rule != null) add(value to describeRepeat(rule) { key, args -> localizedString(locale, key, args) })
                        }
                        add(CUSTOMIZE to tr("calendar.repeat.customize"))
                    }.distinctBy { it.first }
                PickerRow(
                    icon = Icons.Filled.Repeat,
                    value = repeatLabels.firstOrNull { it.first == repeat }?.second.orEmpty(),
                    options = repeatLabels,
                    optionLabel = { it.second },
                    onPick = { if (it.first == CUSTOMIZE) customizing = true else chooseRepeat(it.first) },
                )
                if (writableCalendars.isNotEmpty() || synced != null) {
                    val picked = calendars.firstOrNull { it.id == calendar }
                    val calendarOptions =
                        if (readOnly && synced != null) {
                            listOf(synced.id to "${calendarLabel(synced)} · ${tr("calendar.readOnly")}")
                        } else {
                            // A server's own rule cannot be kept on this device.
                            (if (item?.calendar.isNullOrBlank() || repeat != SERVER_REPEAT) listOf("" to tr("calendar.onThisDevice")) else emptyList()) +
                                writableCalendars.map { it.id to calendarLabel(it) }
                        }
                    PickerRow(
                        icon = Icons.Filled.Event,
                        iconTint = picked?.let { calendarColor(it.id, it.color) },
                        value = calendarOptions.firstOrNull { it.first == calendar }?.second ?: tr("calendar.onThisDevice"),
                        options = if (readOnly) emptyList() else calendarOptions,
                        optionLabel = { it.second },
                        // Choosing another calendar moves the event there on save.
                        onPick = { calendar = it.first },
                    )
                }
                FieldRow(
                    icon = Icons.Outlined.Place,
                    value = location,
                    onValueChange = { location = it },
                    placeholder = tr("calendar.fields.location"),
                    singleLine = true,
                )
            }
            // An invitation's alerts belong to its series, whose kind one
            // occurrence can differ from.
            val alertsAllDay = if (invite) item?.alertsAllDay ?: item?.allDay ?: false else allDay
            val defaultLabel =
                tr(
                    "calendar.alert.default",
                    mapOf("value" to alertLabel(if (alertsAllDay) defaultAllDayAlertDays else defaultAlertMinutes, alertsAllDay)),
                )
            val first = alert
            PickerRow(
                icon = Icons.Filled.Notifications,
                value = first?.let { alertLabel(it, alertsAllDay) } ?: defaultLabel,
                options = if (readOnly) emptyList() else listOf<Int?>(null, NO_ALERT) + alertOptions(alertsAllDay),
                optionLabel = { it?.let { lead -> alertLabel(lead, alertsAllDay) } ?: defaultLabel },
                onPick = { alert = it },
            )
            if (first != null && first != NO_ALERT) {
                PickerRow(
                    icon = null,
                    label = tr("calendar.alert.second"),
                    value = alertLabel(secondAlert, alertsAllDay),
                    options = if (readOnly) emptyList() else listOf<Int?>(NO_ALERT) + alertOptions(alertsAllDay).filter { it != first },
                    optionLabel = { alertLabel(it ?: NO_ALERT, alertsAllDay) },
                    onPick = { secondAlert = it ?: NO_ALERT },
                )
            }
            NotesRow(
                notes = notes,
                onNotesChange = { notes = it },
                minLines = if (invite) 6 else 2,
                maxLines = if (invite) 12 else 6,
                editable = !readOnly,
            )
            Row(
                modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 20.dp, top = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (item != null) {
                    if (!readOnly) {
                        IconButton(onClick = { onDelete(item.id) }) {
                            Icon(
                                Icons.Filled.Delete,
                                contentDescription = if (invite) tr("calendar.removeFromCalendar") else tr("buttons.delete"),
                                tint = colors.error,
                            )
                        }
                    }
                    IconButton(onClick = { onExport(item.id) }) {
                        Icon(
                            Icons.Filled.Download,
                            contentDescription = tr(if (item.recurring) "calendar.file.exportSeries" else "calendar.file.exportEvent"),
                        )
                    }
                    if (item.threadId.isNotBlank()) {
                        TextButton(onClick = { onOpenMail(item) }) {
                            Icon(Icons.Outlined.Email, contentDescription = null, Modifier.size(18.dp))
                            Spacer(Modifier.width(8.dp))
                            Text(tr("calendar.openMail"))
                        }
                    }
                }
                Spacer(Modifier.weight(1f))
                Button(
                    onClick = {
                        onSave(
                            params().copy(
                                original = if (item != null) opened else null,
                                // Only a rule chosen here was moved with the start.
                                repeatAsShown = if (item != null) anchor != null else null,
                                // The series' first day a chosen rule was placed
                                // for: the core moves it on in the series' zone.
                                repeatFrom = if (item != null && anchor != null) seriesDayOf(startDay).isoString() else null,
                            ),
                        )
                    },
                    enabled = !saving && !readOnly,
                ) { Text(tr("buttons.save")) }
            }
        }
    }

    if (confirmDiscard) {
        AlertDialog(
            onDismissRequest = { confirmDiscard = false },
            title = { Text(tr("contacts.notesDiscard")) },
            confirmButton = {
                TextButton(onClick = {
                    confirmDiscard = false
                    onDismiss()
                }) { Text(tr("buttons.discard")) }
            },
            dismissButton = { TextButton(onClick = { confirmDiscard = false }) { Text(tr("mobile.compose.keepEditing")) } },
        )
    }

    when (val current = picking) {
        Picking.StartDate, Picking.EndDate -> {
            // The Material picker works in UTC midnights, which a CivilDate is.
            val pickerState =
                rememberDatePickerState(
                    initialSelectedDateMillis = (if (current == Picking.StartDate) startDay else endDay).utcSeconds * 1000,
                )
            DatePickerDialog(
                onDismissRequest = { picking = null },
                confirmButton = {
                    TextButton(onClick = {
                        pickerState.selectedDateMillis?.let { millis ->
                            val day = CivilDate.ofUtcSeconds(millis / 1000)
                            if (current == Picking.StartDate) {
                                // Moving the start moves the end with it, keeping the length.
                                endDay = endDay.plusDays(day.epochDay - startDay.epochDay)
                                moveStartDay(day)
                            } else {
                                endDay = maxOf(day, startDay)
                            }
                        }
                        picking = null
                    }) { Text(tr("buttons.save")) }
                },
                dismissButton = { TextButton(onClick = { picking = null }) { Text(tr("buttons.cancel")) } },
            ) {
                DatePicker(state = pickerState)
            }
        }

        Picking.StartTime, Picking.EndTime -> {
            val initial = if (current == Picking.StartTime) startMinutes else endMinutes
            val pickerState = rememberTimePickerState(initialHour = initial / 60, initialMinute = initial % 60)
            AlertDialog(
                onDismissRequest = { picking = null },
                text = { TimePicker(state = pickerState) },
                confirmButton = {
                    TextButton(onClick = {
                        val minutes = pickerState.hour * 60 + pickerState.minute
                        if (current == Picking.StartTime) {
                            // Keep the length when the start moves, carrying
                            // the end into the next (or previous) day.
                            val end = endDay.epochDay * MINUTES_PER_DAY + endMinutes + (minutes - startMinutes)
                            endDay = CivilDate.fromEpochDay(end.floorDiv(MINUTES_PER_DAY))
                            endMinutes = end.mod(MINUTES_PER_DAY).toInt()
                            startMinutes = minutes
                        } else {
                            endMinutes = minutes
                        }
                        picking = null
                    }) { Text(tr("buttons.save")) }
                },
                dismissButton = { TextButton(onClick = { picking = null }) { Text(tr("buttons.cancel")) } },
            )
        }

        null -> {}
    }

    if (customizing) {
        CustomRepeatDialog(
            // A server's rule cannot be opened; the editor starts afresh. The
            // event's own rule opens moved with the start, as the core would
            // move it; Done makes it one chosen here.
            value =
                when {
                    repeat == SERVER_REPEAT -> ""
                    anchor == null && openedSeriesDay != null -> alignRepeat(repeat, openedSeriesDay, seriesDayOf(startDay))
                    else -> repeat
                },
            first = seriesDayOf(startDay),
            onDone = {
                chooseRepeat(it)
                customizing = false
            },
            onDismiss = { customizing = false },
        )
    }
}

/** The editor's text fields sit bare on the sheet, as the task editor's do. */
@Composable
private fun bareFieldColors() =
    TextFieldDefaults.colors(
        focusedContainerColor = Color.Transparent,
        unfocusedContainerColor = Color.Transparent,
        focusedIndicatorColor = Color.Transparent,
        unfocusedIndicatorColor = Color.Transparent,
    )

/** Where a row's content starts: the icon column, then the gap a bare text
 *  field pads its own text by, so rows and fields share a left edge. */
private val EditorIconColumn = 38.dp

/** One line of the editor: an icon naming it (or the gap of one), then what
 *  it holds. */
@Composable
private fun EditorRow(
    icon: ImageVector?,
    modifier: Modifier = Modifier,
    iconTint: Color? = null,
    content: @Composable RowScope.() -> Unit,
) {
    Row(
        modifier = modifier.fillMaxWidth().heightIn(min = 52.dp).padding(start = 24.dp, end = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(EditorIconColumn)) {
            if (icon != null) Icon(icon, contentDescription = null, Modifier.size(22.dp), tint = iconTint ?: MaterialTheme.colorScheme.onSurfaceVariant)
        }
        content()
    }
}

/** A row that is a text field. The icon stays by the first line as the field
 *  grows. */
@Composable
private fun FieldRow(
    icon: ImageVector,
    value: String,
    onValueChange: (String) -> Unit,
    placeholder: String,
    singleLine: Boolean = false,
    minLines: Int = 1,
    maxLines: Int = if (singleLine) 1 else Int.MAX_VALUE,
    fieldModifier: Modifier = Modifier,
) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Row(modifier = Modifier.fillMaxWidth().padding(start = 24.dp, end = 4.dp)) {
        Icon(icon, contentDescription = null, Modifier.padding(top = 17.dp).size(22.dp), tint = muted)
        TextField(
            value = value,
            onValueChange = onValueChange,
            placeholder = { Text(placeholder, color = muted) },
            singleLine = singleLine,
            minLines = minLines,
            maxLines = maxLines,
            colors = bareFieldColors(),
            modifier = Modifier.weight(1f).then(fieldModifier),
        )
    }
}

/**
 * Notes read with their links tappable, as an invitation's usually carry a
 * meeting link; a tap elsewhere in them edits them. Empty notes start as the
 * field. Mirrors desktop's NotesField.
 */
@Composable
private fun NotesRow(
    notes: String,
    onNotesChange: (String) -> Unit,
    minLines: Int,
    maxLines: Int,
    editable: Boolean = true,
) {
    var editing by remember { mutableStateOf(editable && notes.isBlank()) }
    // Only a tap on the read view moves focus into the field.
    var focusOnEdit by remember { mutableStateOf(false) }
    if (editing) {
        val focusRequester = remember { FocusRequester() }
        var focused by remember { mutableStateOf(false) }
        FieldRow(
            icon = Icons.AutoMirrored.Filled.Notes,
            value = notes,
            onValueChange = onNotesChange,
            placeholder = tr("calendar.fields.notes"),
            minLines = minLines,
            maxLines = maxLines,
            fieldModifier =
                Modifier
                    .focusRequester(focusRequester)
                    .onFocusChanged {
                        if (focused && !it.isFocused && notes.isNotBlank()) editing = false
                        focused = it.isFocused
                    },
        )
        LaunchedEffect(Unit) {
            if (focusOnEdit) focusRequester.requestFocus()
        }
        return
    }
    val colors = MaterialTheme.colorScheme
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .clickable(enabled = editable) {
                    focusOnEdit = true
                    editing = true
                }.padding(start = 24.dp, end = 4.dp),
    ) {
        Icon(Icons.AutoMirrored.Filled.Notes, contentDescription = null, Modifier.padding(top = 17.dp).size(22.dp), tint = colors.onSurfaceVariant)
        LinkifiedText(
            notes,
            minLines = minLines,
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.weight(1f).padding(horizontal = 16.dp, vertical = 16.dp),
        )
    }
}

/** A row naming its choice, opening a menu of the others when tapped. */
@Composable
private fun <T> PickerRow(
    icon: ImageVector?,
    value: String,
    options: List<T>,
    optionLabel: @Composable (T) -> String,
    onPick: (T) -> Unit,
    label: String? = null,
    iconTint: Color? = null,
) {
    var menu by remember { mutableStateOf(false) }
    EditorRow(icon, iconTint = iconTint, modifier = Modifier.clickable(enabled = options.isNotEmpty()) { menu = true }) {
        Box(Modifier.weight(1f)) {
            Column {
                if (label != null) {
                    Text(label, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Text(value, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                options.forEach { option ->
                    DropdownMenuItem(
                        text = { Text(optionLabel(option)) },
                        onClick = {
                            onPick(option)
                            menu = false
                        },
                    )
                }
            }
        }
    }
}

private val urlPattern = Regex("^https?://\\S+$")

/** An invitation's location is often its meeting link: open it, or copy it. */
@Composable
private fun InviteLocation(location: String) {
    val url = location.trim()
    if (!urlPattern.matches(url)) {
        Text(location, color = MaterialTheme.colorScheme.onSurfaceVariant)
        return
    }
    val platform = LocalPlatformServices.current
    val copyLabel = tr("chat.actions.copyLinkAddress")
    var copied by remember { mutableStateOf(false) }
    LaunchedEffect(copied) {
        if (copied) {
            delay(1500)
            copied = false
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            url,
            color = MaterialTheme.colorScheme.primary,
            textDecoration = TextDecoration.Underline,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false).clickable { platform.openUrl(url) },
        )
        IconButton(onClick = {
            platform.copyText(copyLabel, url)
            copied = true
        }) {
            Icon(
                if (copied) Icons.Filled.Check else Icons.Filled.ContentCopy,
                contentDescription = if (copied) tr("common.copied") else copyLabel,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

/** A start or an end: its day and, unless the event lasts all day, its time,
 *  each opening its picker. */
@Composable
private fun DateTimeRow(
    label: String,
    day: CivilDate,
    minutes: Int?,
    onPickDate: () -> Unit,
    onPickTime: () -> Unit,
) {
    EditorRow(icon = null) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
        // On a narrow screen or with large text the time keeps its width and
        // the date gives way, ellipsized, rather than pushing it off the row.
        Row(
            modifier = Modifier.weight(1f).padding(start = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
        ) {
            PickerValue(
                formatDate(localMidnight(day) * 1000, DateStyle.WeekdayDate),
                onPickDate,
                Modifier.weight(1f, fill = false),
            )
            if (minutes != null) PickerValue(formatMinutes(minutes), onPickTime)
        }
    }
}

@Composable
private fun PickerValue(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Text(
        text,
        style = MaterialTheme.typography.labelLarge,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier =
            modifier
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .clickable(onClick = onClick)
                .padding(horizontal = 12.dp, vertical = 8.dp),
    )
}
