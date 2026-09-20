package jp.nonbili.meron.ui

import platform.Foundation.NSCalendar
import platform.Foundation.NSDate
import platform.Foundation.NSDateComponents
import platform.Foundation.NSDateFormatter
import platform.Foundation.dateWithTimeIntervalSince1970
import platform.Foundation.timeIntervalSince1970

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
            DateStyle.IsoDate -> "yyyy-MM-dd"
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

internal actual fun epochSecondsForLocalDate(
    year: Int,
    month: Int,
    day: Int,
): Long {
    val components =
        NSDateComponents().apply {
            setYear(year.toLong())
            setMonth(month.toLong())
            setDay(day.toLong())
        }
    val date = NSCalendar.currentCalendar.dateFromComponents(components) ?: return 0
    return date.timeIntervalSince1970.toLong()
}
