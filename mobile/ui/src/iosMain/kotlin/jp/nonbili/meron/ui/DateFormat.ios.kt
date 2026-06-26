package jp.nonbili.meron.ui

import platform.Foundation.NSDate
import platform.Foundation.NSDateFormatter
import platform.Foundation.dateWithTimeIntervalSince1970

internal actual fun formatDate(
    epochMillis: Long,
    style: DateStyle,
): String {
    val formatter = NSDateFormatter()
    formatter.dateFormat = if (style == DateStyle.Weekday) "EEE" else "MMM d"
    return formatter.stringFromDate(NSDate.dateWithTimeIntervalSince1970(epochMillis / 1000.0))
}
