package jp.nonbili.meron.ui

internal enum class DateStyle { Weekday, MonthDay }

/** Locale-aware short date; only this piece needs platform date formatting. */
internal expect fun formatDate(
    epochMillis: Long,
    style: DateStyle,
): String

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
