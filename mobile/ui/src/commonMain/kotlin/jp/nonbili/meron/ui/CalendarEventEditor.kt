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
            if (item != null && invite) {
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
                PickerRow(
                    icon = Icons.Filled.Repeat,
                    value = tr(repeats.first { it.first == repeat }.second),
                    options = repeats,
                    optionLabel = { tr(it.second) },
                    onPick = { repeat = it.first },
                )
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
                options = listOf<Int?>(null, NO_ALERT) + alertOptions(alertsAllDay),
                optionLabel = { it?.let { lead -> alertLabel(lead, alertsAllDay) } ?: defaultLabel },
                onPick = { alert = it },
            )
            if (first != null && first != NO_ALERT) {
                PickerRow(
                    icon = null,
                    label = tr("calendar.alert.second"),
                    value = alertLabel(secondAlert, alertsAllDay),
                    options = listOf<Int?>(NO_ALERT) + alertOptions(alertsAllDay).filter { it != first },
                    optionLabel = { alertLabel(it ?: NO_ALERT, alertsAllDay) },
                    onPick = { secondAlert = it ?: NO_ALERT },
                )
            }
            FieldRow(
                icon = Icons.AutoMirrored.Filled.Notes,
                value = notes,
                onValueChange = { notes = it },
                placeholder = tr("calendar.fields.notes"),
                minLines = if (invite) 6 else 2,
                maxLines = if (invite) 12 else 6,
            )
            Row(
                modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 20.dp, top = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (item != null) {
                    IconButton(onClick = { onDelete(item.id) }) {
                        Icon(
                            Icons.Filled.Delete,
                            contentDescription = if (invite) tr("calendar.removeFromCalendar") else tr("buttons.delete"),
                            tint = colors.error,
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
                Button(onClick = { onSave(params()) }, enabled = !saving) { Text(tr("buttons.save")) }
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
    content: @Composable RowScope.() -> Unit,
) {
    Row(
        modifier = modifier.fillMaxWidth().heightIn(min = 52.dp).padding(start = 24.dp, end = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.width(EditorIconColumn)) {
            if (icon != null) Icon(icon, contentDescription = null, Modifier.size(22.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
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
            modifier = Modifier.weight(1f),
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
) {
    var menu by remember { mutableStateOf(false) }
    EditorRow(icon, modifier = Modifier.clickable { menu = true }) {
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
