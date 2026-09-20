package jp.nonbili.meron.ui

/** [IsoDate] is the machine-readable `YYYY-MM-DD` a typed due date uses; every
 *  other style is for display and follows the device's locale. */
internal enum class DateStyle { Time, Weekday, MonthDay, MonthDayYear, FullTimestamp, IsoDate }

/** Locale-aware short date; only this piece needs platform date formatting. */
internal expect fun formatDate(
    epochMillis: Long,
    style: DateStyle,
): String

internal expect fun isSameLocalDate(
    epochMillis: Long,
    referenceEpochMillis: Long,
): Boolean

internal expect fun isSameLocalYear(
    epochMillis: Long,
    referenceEpochMillis: Long,
): Boolean

/** Local midnight on the given calendar date, as epoch seconds. A due date is a
 *  day, not an instant, so it is anchored where the user's day starts. */
internal expect fun epochSecondsForLocalDate(
    year: Int,
    month: Int,
    day: Int,
): Long

internal fun formatInboxTimestamp(
    epochSeconds: Long,
    nowMillis: Long = currentTimeMillis(),
    sameLocalDate: (Long, Long) -> Boolean = ::isSameLocalDate,
    sameLocalYear: (Long, Long) -> Boolean = ::isSameLocalYear,
    dateFormatter: (Long, DateStyle) -> String = ::formatDate,
): String {
    if (epochSeconds <= 0) return ""
    val thenMillis = epochSeconds * 1000
    return when {
        sameLocalDate(thenMillis, nowMillis) -> dateFormatter(thenMillis, DateStyle.Time)
        sameLocalYear(thenMillis, nowMillis) -> dateFormatter(thenMillis, DateStyle.MonthDay)
        else -> dateFormatter(thenMillis, DateStyle.MonthDayYear)
    }
}

internal fun formatMessageFullTimestamp(
    epochSeconds: Long,
    dateFormatter: (Long, DateStyle) -> String = ::formatDate,
): String {
    if (epochSeconds <= 0) return ""
    return dateFormatter(epochSeconds * 1000, DateStyle.FullTimestamp)
}

internal fun formatRelativeTime(epochSeconds: Long): String {
    if (epochSeconds <= 0) return ""
    val nowMillis = currentTimeMillis()
    val thenMillis = epochSeconds * 1000
    val diff = nowMillis - thenMillis
    return when {
        diff < 60_000 -> "now"
        diff < 3_600_000 -> "${diff / 60_000}m"
        diff < 86_400_000 -> "${diff / 3_600_000}h"
        diff < 7 * 86_400_000L -> formatDate(thenMillis, DateStyle.Weekday)
        else -> formatDate(thenMillis, DateStyle.MonthDay)
    }
}

/** A task's due date: month/day, with the year once it isn't the current one. */
internal fun formatTaskDueDate(
    epochSeconds: Long,
    nowMillis: Long = currentTimeMillis(),
    sameLocalYear: (Long, Long) -> Boolean = ::isSameLocalYear,
    dateFormatter: (Long, DateStyle) -> String = ::formatDate,
): String {
    if (epochSeconds <= 0) return ""
    val thenMillis = epochSeconds * 1000
    val style = if (sameLocalYear(thenMillis, nowMillis)) DateStyle.MonthDay else DateStyle.MonthDayYear
    return dateFormatter(thenMillis, style)
}

/** Whether a due date has already passed, so the row can call it out. */
internal fun taskIsOverdue(
    dueAtSeconds: Long,
    nowMillis: Long = currentTimeMillis(),
): Boolean = dueAtSeconds > 0 && dueAtSeconds * 1000 < nowMillis && !isSameLocalDate(dueAtSeconds * 1000, nowMillis)
