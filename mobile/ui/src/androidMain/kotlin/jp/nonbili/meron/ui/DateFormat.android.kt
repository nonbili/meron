package jp.nonbili.meron.ui

import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
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
        }
    return SimpleDateFormat(pattern, Locale.getDefault()).format(Date(epochMillis))
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
