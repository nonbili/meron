package jp.nonbili.meron.ui

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

internal actual fun formatDate(
    epochMillis: Long,
    style: DateStyle,
): String {
    val pattern = if (style == DateStyle.Weekday) "EEE" else "MMM d"
    return SimpleDateFormat(pattern, Locale.getDefault()).format(Date(epochMillis))
}
