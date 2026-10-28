package jp.nonbili.meron.ui

import androidx.compose.runtime.Composable

// Event alerts, Apple Calendar style: app-wide defaults for timed and all-day
// events, and up to two alerts an event can set for itself. A timed event's
// leads are minutes before its start; an all-day event's are days before its
// date, at 09:00. The core (meron-core/src/reminders.rs) plans the
// notifications. Mirrors desktop's lib/eventAlerts.ts.

/** The leads offered for timed events, in minutes. */
internal val alertOptionsMinutes = listOf(0, 5, 10, 15, 30, 60, 120, 1440, 2880)

/** The leads offered for all-day events, in days. */
internal val allDayAlertOptionsDays = listOf(0, 1, 2, 7)

/** The default alert setting that turns event alerts off. */
internal const val NO_ALERT = -1

internal const val DEFAULT_ALERT_MINUTES = 10

internal const val DEFAULT_ALL_DAY_ALERT_DAYS = 0

/** The leads an event of this kind offers. */
internal fun alertOptions(allDay: Boolean): List<Int> = if (allDay) allDayAlertOptionsDays else alertOptionsMinutes

/** "At time of event", "15 minutes before", "2 hours before", "1 day before";
 *  for an all-day event "On the day (9 a.m.)", "2 days before (9 a.m.)". */
@Composable
internal fun alertLabel(
    lead: Int,
    allDay: Boolean = false,
): String =
    when {
        lead == NO_ALERT -> tr("calendar.alert.none")
        allDay && lead == 0 -> tr("calendar.alert.allDayOnDay")
        allDay && lead == 7 -> tr("calendar.alert.allDayWeek")
        allDay -> tr("calendar.alert.allDayDays", mapOf("count" to lead))
        lead == 0 -> tr("calendar.alert.atStart")
        lead % 1440 == 0 -> tr("calendar.alert.days", mapOf("count" to lead / 1440))
        lead % 60 == 0 -> tr("calendar.alert.hours", mapOf("count" to lead / 60))
        else -> tr("calendar.alert.minutes", mapOf("count" to lead))
    }
