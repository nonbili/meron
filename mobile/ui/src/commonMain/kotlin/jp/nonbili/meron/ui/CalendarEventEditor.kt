package jp.nonbili.meron.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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

/**
 * Create or change an event made here. An accepted invitation opens read-only
 * but for its notes: its time and title are the organizer's to change.
 * Mirrors desktop's EventEditorDialog.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CalendarEventEditor(
    target: CalendarEditorTarget,
    saving: Boolean,
    onDismiss: () -> Unit,
    onSave: (CalendarEventParams) -> Unit,
    onDelete: (eventId: String) -> Unit,
    onOpenMail: (CalendarItem) -> Unit,
) {
    val item = (target as? CalendarEditorTarget.Edit)?.item
    val invite = item?.source == "invite"
    var title by remember { mutableStateOf(item?.title.orEmpty()) }
    var notes by remember { mutableStateOf(item?.notes.orEmpty()) }
    var location by remember { mutableStateOf(item?.location.orEmpty()) }
    var allDay by remember { mutableStateOf(item?.allDay ?: false) }
    var repeat by remember { mutableStateOf(item?.repeat.orEmpty()) }
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
    var repeatMenu by remember { mutableStateOf(false) }

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
        )
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                when {
                    item == null -> tr("calendar.newEvent")
                    invite -> item.title.ifBlank { tr("invite.untitled") }
                    else -> tr("calendar.editEvent")
                },
            )
        },
        text = {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                if (item != null && invite) {
                    Text(
                        formatEventRange(item.start, item.end, item.allDay) + if (item.allDay) " · ${tr("invite.allDay")}" else "",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    if (item.location.isNotBlank()) InviteLocation(item.location)
                } else {
                    OutlinedTextField(
                        value = title,
                        onValueChange = { title = it },
                        placeholder = { Text(tr("calendar.fields.title")) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(tr("invite.allDay"), modifier = Modifier.weight(1f))
                        Switch(checked = allDay, onCheckedChange = { allDay = it })
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
                    Box {
                        AssistChip(
                            onClick = { repeatMenu = true },
                            label = { Text(tr(repeats.first { it.first == repeat }.second)) },
                            leadingIcon = { Icon(Icons.Filled.Repeat, contentDescription = null, Modifier.size(18.dp)) },
                        )
                        DropdownMenu(expanded = repeatMenu, onDismissRequest = { repeatMenu = false }) {
                            repeats.forEach { (value, key) ->
                                DropdownMenuItem(
                                    text = { Text(tr(key)) },
                                    onClick = {
                                        repeat = value
                                        repeatMenu = false
                                    },
                                )
                            }
                        }
                    }
                    OutlinedTextField(
                        value = location,
                        onValueChange = { location = it },
                        placeholder = { Text(tr("calendar.fields.location")) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                OutlinedTextField(
                    value = notes,
                    onValueChange = { notes = it },
                    placeholder = { Text(tr("calendar.fields.notes")) },
                    minLines = if (invite) 8 else 3,
                    maxLines = if (invite) 12 else 6,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (item != null) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        IconButton(onClick = { onDelete(item.id) }) {
                            Icon(
                                Icons.Filled.Delete,
                                contentDescription = if (invite) tr("calendar.removeFromCalendar") else tr("buttons.delete"),
                            )
                        }
                        if (item.threadId.isNotBlank()) {
                            TextButton(onClick = { onOpenMail(item) }, contentPadding = PaddingValues(horizontal = 4.dp)) {
                                Icon(Icons.Outlined.Email, contentDescription = null, Modifier.size(18.dp))
                                Spacer(Modifier.width(8.dp))
                                Text(tr("calendar.openMail"))
                            }
                        }
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = { onSave(params()) }, enabled = !saving) { Text(tr("buttons.save")) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )

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
                                startDay = day
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
            val pickerState = rememberTimePickerState(initialHour = initial / 60, initialMinute = initial % 60, is24Hour = true)
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

@Composable
private fun DateTimeRow(
    label: String,
    day: CivilDate,
    minutes: Int?,
    onPickDate: () -> Unit,
    onPickTime: () -> Unit,
) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, modifier = Modifier.width(56.dp), color = MaterialTheme.colorScheme.onSurfaceVariant)
        // At phone width inside the dialog the full date leaves the time chip
        // no room, wrapping "09:00" a character a line: the time keeps its width
        // and the date gives way, ellipsized.
        AssistChip(
            onClick = onPickDate,
            label = {
                Text(
                    formatDate(localMidnight(day) * 1000, DateStyle.WeekdayDate),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            },
            modifier = Modifier.weight(1f, fill = false),
        )
        if (minutes != null) {
            AssistChip(onClick = onPickTime, label = { Text(formatMinutes(minutes), softWrap = false) })
        }
    }
}
