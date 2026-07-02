package jp.nonbili.meron.ui

import platform.Foundation.NSDate
import platform.Foundation.NSDateFormatter
import platform.Foundation.dateWithTimeIntervalSince1970

internal actual fun formatDate(
    epochMillis: Long,
    style: DateStyle,
): String {
    val formatter = NSDateFormatter()
    formatter.dateFormat =
        when (style) {
            DateStyle.Time -> "HH:mm"
            DateStyle.Weekday -> "EEE"
            DateStyle.MonthDay -> "MMM d"
            DateStyle.MonthDayYear -> "MMM d, yyyy"
            DateStyle.FullTimestamp -> "EEE, MMM d, yyyy, HH:mm"
        }
    return formatter.stringFromDate(NSDate.dateWithTimeIntervalSince1970(epochMillis / 1000.0))
}

internal actual fun isSameLocalDate(
    epochMillis: Long,
    referenceEpochMillis: Long,
): Boolean = formattedLocalDateKey(epochMillis, "yyyy-MM-dd") == formattedLocalDateKey(referenceEpochMillis, "yyyy-MM-dd")

internal actual fun isSameLocalYear(
    epochMillis: Long,
    referenceEpochMillis: Long,
): Boolean = formattedLocalDateKey(epochMillis, "yyyy") == formattedLocalDateKey(referenceEpochMillis, "yyyy")

private fun formattedLocalDateKey(
    epochMillis: Long,
    pattern: String,
): String {
    val formatter = NSDateFormatter()
    formatter.dateFormat = pattern
    return formatter.stringFromDate(NSDate.dateWithTimeIntervalSince1970(epochMillis / 1000.0))
}
