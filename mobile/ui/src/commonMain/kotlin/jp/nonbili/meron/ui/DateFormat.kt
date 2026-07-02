package jp.nonbili.meron.ui

internal enum class DateStyle { Time, Weekday, MonthDay, MonthDayYear, FullTimestamp }

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
