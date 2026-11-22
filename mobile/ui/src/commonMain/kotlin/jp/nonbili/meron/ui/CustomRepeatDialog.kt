package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

private val UNIT_KEYS =
    listOf(
        RepeatFreq.Daily to "calendar.repeat.unitDays",
        RepeatFreq.Weekly to "calendar.repeat.unitWeeks",
        RepeatFreq.Monthly to "calendar.repeat.unitMonths",
        RepeatFreq.Yearly to "calendar.repeat.unitYears",
    )

private enum class EndKind { Never, Until, Count }

/**
 * Every N days, weeks, months or years: on which weekdays, on which day of the
 * month, and until when. Done hands back the rule as the core keeps it (a
 * plain choice when it says no more). [first] is the series' first day, which
 * the choices are offered from. Mirrors desktop's CustomRepeatDialog.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CustomRepeatDialog(
    value: String,
    first: CivilDate,
    onDone: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val startWeekday = ruleWeekday(first)
    // The week the picker shows, Monday 0: an every-other-week rule made here
    // skips the weeks as they are shown. A rule that already has days keeps
    // its own week start.
    val weekStart = (firstDayOfWeek() + 6) % 7
    var rule by remember {
        val own = parseRepeat(value)
        var rule = initialRule(value, first)
        if (own?.weekdays.isNullOrEmpty()) rule = rule.copy(weekStart = weekStart)
        // The series' first day is always one of a weekly rule's: RFC 5545
        // leaves a start the rule does not match to each client to read its
        // own way.
        if (rule.freq == RepeatFreq.Weekly && startWeekday !in rule.weekdays) rule = rule.copy(weekdays = (rule.weekdays + startWeekday).sorted())
        mutableStateOf(rule)
    }
    // What the number fields hold while being typed, which may be empty.
    var intervalText by remember { mutableStateOf(rule.interval.toString()) }
    var countText by remember { mutableStateOf((rule.end as? RepeatEnd.Count)?.count?.toString() ?: "10") }
    var until by remember { mutableStateOf((rule.end as? RepeatEnd.Until)?.date ?: first) }
    var endKind by remember {
        mutableStateOf(
            when (rule.end) {
                is RepeatEnd.Count -> EndKind.Count
                is RepeatEnd.Until -> EndKind.Until
                RepeatEnd.Never -> EndKind.Never
            },
        )
    }
    var unitMenu by remember { mutableStateOf(false) }
    var monthlyMenu by remember { mutableStateOf(false) }
    var pickingUntil by remember { mutableStateOf(false) }

    fun setFreq(freq: RepeatFreq) {
        rule =
            rule.copy(
                freq = freq,
                weekdays = if (freq == RepeatFreq.Weekly && startWeekday !in rule.weekdays) (rule.weekdays + startWeekday).sorted() else rule.weekdays,
                monthly = if (freq == RepeatFreq.Monthly && rule.monthly == null) MonthlyOn.Day(first.day) else rule.monthly,
            )
    }

    val long = weekdayNames(DateStyle.WeekdayLong)
    val narrow = weekdayNames(DateStyle.WeekdayNarrow)
    val order = (0 until 7).map { (weekStart + it) % 7 }
    val (nth, last) = weekdayPlace(first)
    val monthlyChoices =
        buildList {
            add(MonthlyOn.Day(first.day))
            add(MonthlyOn.Weekday(nth, startWeekday))
            if (last) add(MonthlyOn.Weekday(-1, startWeekday))
            // One a server set, from another day than this event's start, stays offered.
            rule.monthly?.let { if (it !in this) add(it) }
        }

    @Composable
    fun monthlyLabel(monthly: MonthlyOn): String =
        when (monthly) {
            is MonthlyOn.Day -> tr("calendar.repeat.onMonthDay", mapOf("day" to monthly.day))
            is MonthlyOn.Weekday -> tr("calendar.repeat.onWeekday", mapOf("nth" to tr(nthKey(monthly.nth)), "weekday" to long[monthly.weekday]))
        }

    val interval = intervalText.toIntOrNull()
    val count = countText.toIntOrNull()
    val intervalValid = interval != null && interval in 1..MAX_REPEAT_INTERVAL
    val end =
        when (endKind) {
            EndKind.Never -> RepeatEnd.Never
            EndKind.Count -> RepeatEnd.Count(count ?: 0)
            EndKind.Until -> RepeatEnd.Until(until)
        }
    val endValid =
        when (end) {
            RepeatEnd.Never -> true
            is RepeatEnd.Count -> end.count in 1..MAX_REPEAT_COUNT
            is RepeatEnd.Until -> end.date >= first
        }
    val valid = intervalValid && endValid && (rule.freq != RepeatFreq.Weekly || rule.weekdays.isNotEmpty())
    // While a field is being typed into, the summary shows what is valid of it.
    val result = rule.copy(interval = if (intervalValid) interval!! else 1, end = if (endValid) end else RepeatEnd.Never)
    val locale = LocalAppLocale.current

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("calendar.repeat.customTitle")) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(tr("calendar.repeat.repeatEvery"), style = MaterialTheme.typography.labelMedium)
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            value = intervalText,
                            onValueChange = { intervalText = it.filter(Char::isDigit).take(3) },
                            singleLine = true,
                            isError = !intervalValid,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                            modifier = Modifier.width(80.dp),
                        )
                        Box {
                            TextButton(onClick = { unitMenu = true }) {
                                Text(tr(UNIT_KEYS.first { it.first == rule.freq }.second, mapOf("count" to (if (intervalValid) interval else 1))))
                            }
                            DropdownMenu(expanded = unitMenu, onDismissRequest = { unitMenu = false }) {
                                UNIT_KEYS.forEach { (freq, key) ->
                                    DropdownMenuItem(
                                        text = { Text(tr(key, mapOf("count" to (if (intervalValid) interval else 1)))) },
                                        onClick = {
                                            unitMenu = false
                                            setFreq(freq)
                                        },
                                    )
                                }
                            }
                        }
                    }
                }

                if (rule.freq == RepeatFreq.Weekly) {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(tr("calendar.repeat.repeatOn"), style = MaterialTheme.typography.labelMedium)
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            order.forEach { day ->
                                val on = day in rule.weekdays
                                // The series' first day cannot be taken out.
                                val locked = day == startWeekday
                                Box(
                                    Modifier
                                        .size(34.dp)
                                        .clip(CircleShape)
                                        .background(if (on) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHigh)
                                        .clickable(enabled = !locked) {
                                            rule = rule.copy(weekdays = if (on) rule.weekdays - day else (rule.weekdays + day).sorted())
                                        }.semantics {
                                            contentDescription = long[day]
                                            selected = on
                                        },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        narrow[day],
                                        fontWeight = FontWeight.SemiBold,
                                        color = if (on) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }

                val monthly = rule.monthly
                if (rule.freq == RepeatFreq.Monthly && monthly != null) {
                    Box {
                        TextButton(onClick = { monthlyMenu = true }) { Text(monthlyLabel(monthly)) }
                        DropdownMenu(expanded = monthlyMenu, onDismissRequest = { monthlyMenu = false }) {
                            monthlyChoices.forEach { choice ->
                                DropdownMenuItem(
                                    text = { Text(monthlyLabel(choice)) },
                                    onClick = {
                                        monthlyMenu = false
                                        rule = rule.copy(monthly = choice)
                                    },
                                )
                            }
                        }
                    }
                }

                Column {
                    Text(tr("calendar.fields.ends"), style = MaterialTheme.typography.labelMedium)
                    EndRow(endKind == EndKind.Never, tr("calendar.repeat.endNever"), onSelect = { endKind = EndKind.Never })
                    EndRow(endKind == EndKind.Until, tr("calendar.repeat.endOn"), onSelect = { endKind = EndKind.Until }) {
                        TextButton(enabled = endKind == EndKind.Until, onClick = { pickingUntil = true }) {
                            Text(formatDate(localMidnight(until) * 1000, DateStyle.MonthDayYear), color = if (endKind == EndKind.Until && !endValid) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                        }
                    }
                    EndRow(endKind == EndKind.Count, tr("calendar.repeat.endAfter"), onSelect = { endKind = EndKind.Count }) {
                        OutlinedTextField(
                            value = countText,
                            onValueChange = { countText = it.filter(Char::isDigit).take(3) },
                            enabled = endKind == EndKind.Count,
                            isError = endKind == EndKind.Count && !endValid,
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                            modifier = Modifier.width(80.dp),
                        )
                        Text(
                            tr("calendar.repeat.occurrences", mapOf("count" to (count ?: 1))),
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(start = 8.dp),
                        )
                    }
                }

                Text(
                    describeRepeat(result) { key, args -> localizedString(locale, key, args) },
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = { Button(enabled = valid, onClick = { onDone(formatRepeat(result)) }) { Text(tr("buttons.done")) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) } },
    )

    if (pickingUntil) {
        val picker = rememberDatePickerState(initialSelectedDateMillis = until.utcSeconds * 1000)
        DatePickerDialog(
            onDismissRequest = { pickingUntil = false },
            confirmButton = {
                TextButton(onClick = {
                    picker.selectedDateMillis?.let { until = CivilDate.ofUtcSeconds(it / 1000) }
                    pickingUntil = false
                }) { Text(tr("buttons.save")) }
            },
            dismissButton = { TextButton(onClick = { pickingUntil = false }) { Text(tr("buttons.cancel")) } },
        ) { DatePicker(state = picker) }
    }
}

@Composable
private fun EndRow(
    selected: Boolean,
    label: String,
    onSelect: () -> Unit,
    trailing: @Composable () -> Unit = {},
) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).clickable(onClick = onSelect),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = onSelect)
        Text(label, modifier = Modifier.width(64.dp))
        trailing()
    }
}
