package jp.nonbili.meron.ui

import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.GregorianCalendar
import java.util.Locale

internal actual fun formatDate(
    epochMillis: Long,
    style: DateStyle,
): String {
    val pattern =
        when (style) {
            DateStyle.Time -> "HH:mm"
            DateStyle.Weekday -> "EEE"
            DateStyle.MonthDay -> "MMM d"
            DateStyle.MonthDayYear -> "MMM d, yyyy"
            DateStyle.FullTimestamp -> "EEE, MMM d, yyyy, HH:mm"
            DateStyle.IsoDate -> "yyyy-MM-dd"
        }
    // The ISO date is parsed back as Gregorian fields, so it must not follow a
    // locale calendar such as th-TH's Buddhist one.
    val locale = if (style == DateStyle.IsoDate) Locale.US else Locale.getDefault()
    return SimpleDateFormat(pattern, locale).format(Date(epochMillis))
}

internal actual fun isSameLocalDate(
    epochMillis: Long,
    referenceEpochMillis: Long,
): Boolean {
    val date = Calendar.getInstance().apply { timeInMillis = epochMillis }
    val reference = Calendar.getInstance().apply { timeInMillis = referenceEpochMillis }
    return date.get(Calendar.ERA) == reference.get(Calendar.ERA) &&
        date.get(Calendar.YEAR) == reference.get(Calendar.YEAR) &&
        date.get(Calendar.DAY_OF_YEAR) == reference.get(Calendar.DAY_OF_YEAR)
}

internal actual fun isSameLocalYear(
    epochMillis: Long,
    referenceEpochMillis: Long,
): Boolean {
    val date = Calendar.getInstance().apply { timeInMillis = epochMillis }
    val reference = Calendar.getInstance().apply { timeInMillis = referenceEpochMillis }
    return date.get(Calendar.ERA) == reference.get(Calendar.ERA) &&
        date.get(Calendar.YEAR) == reference.get(Calendar.YEAR)
}

internal actual fun epochSecondsForLocalDate(
    year: Int,
    month: Int,
    day: Int,
): Long {
    val calendar =
        GregorianCalendar().apply {
            clear()
            set(year, month - 1, day)
        }
    return calendar.timeInMillis / 1000
}
