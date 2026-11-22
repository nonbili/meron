package jp.nonbili.meron.ui

import platform.Foundation.NSCalendar
import platform.Foundation.NSCalendarIdentifierGregorian
import platform.Foundation.NSCalendarUnitDay
import platform.Foundation.NSCalendarUnitHour
import platform.Foundation.NSCalendarUnitMinute
import platform.Foundation.NSCalendarUnitMonth
import platform.Foundation.NSCalendarUnitWeekday
import platform.Foundation.NSCalendarUnitYear
import platform.Foundation.NSDate
import platform.Foundation.NSDateComponents
import platform.Foundation.NSDateFormatter
import platform.Foundation.NSLocale
import platform.Foundation.NSTimeZone
import platform.Foundation.dateWithTimeIntervalSince1970
import platform.Foundation.localTimeZone
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
            DateStyle.WeekdayNarrow -> "EEEEE"
            DateStyle.WeekdayLong -> "EEEE"
            DateStyle.MonthDay -> "MMM d"
            DateStyle.MonthDayYear -> "MMM d, yyyy"
            DateStyle.WeekdayDate -> "EEE, MMM d, yyyy"
            DateStyle.MonthYear -> "LLLL yyyy"
            DateStyle.FullTimestamp -> "EEE, MMM d, yyyy, HH:mm"
            DateStyle.IsoDate -> "yyyy-MM-dd"
        }
    if (style == DateStyle.IsoDate) {
        // Parsed back as Gregorian fields, so it must not follow the user's
        // calendar setting (Buddhist, Japanese, ...).
        formatter.calendar = gregorianCalendar()
        formatter.locale = NSLocale(localeIdentifier = "en_US_POSIX")
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
    val date = gregorianCalendar().dateFromComponents(components) ?: return 0
    return date.timeIntervalSince1970.toLong()
}

internal actual fun localDateParts(epochMillis: Long): LocalDateParts {
    val calendar = gregorianCalendar()
    val date = NSDate.dateWithTimeIntervalSince1970(epochMillis / 1000.0)
    val units =
        NSCalendarUnitYear or NSCalendarUnitMonth or NSCalendarUnitDay or NSCalendarUnitWeekday or
            NSCalendarUnitHour or NSCalendarUnitMinute
    val parts = calendar.components(units, fromDate = date)
    return LocalDateParts(
        year = parts.year.toInt(),
        month = parts.month.toInt(),
        day = parts.day.toInt(),
        // NSCalendar numbers Sunday 1 … Saturday 7.
        weekday = parts.weekday.toInt() - 1,
        hour = parts.hour.toInt(),
        minute = parts.minute.toInt(),
    )
}

internal actual fun epochSecondsForLocalDateTime(
    year: Int,
    month: Int,
    day: Int,
    hour: Int,
    minute: Int,
): Long {
    val components =
        NSDateComponents().apply {
            setYear(year.toLong())
            setMonth(month.toLong())
            setDay(day.toLong())
            setHour(hour.toLong())
            setMinute(minute.toLong())
        }
    val date = gregorianCalendar().dateFromComponents(components) ?: return 0
    return date.timeIntervalSince1970.toLong()
}

internal actual fun firstDayOfWeek(): Int = (NSCalendar.currentCalendar.firstWeekday.toInt() - 1).coerceIn(0, 6)

private fun gregorianCalendar(): NSCalendar = NSCalendar(calendarIdentifier = NSCalendarIdentifierGregorian).apply { timeZone = NSTimeZone.localTimeZone }
