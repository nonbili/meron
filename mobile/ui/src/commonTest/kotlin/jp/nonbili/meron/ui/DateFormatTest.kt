package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class DateFormatTest {
    @Test
    fun inboxTimestampReturnsEmptyForUnknownDate() {
        assertEquals("", formatInboxTimestampForTest(0))
    }

    @Test
    fun inboxTimestampUsesTimeForSameLocalDate() {
        assertEquals("09:05", formatInboxTimestampForTest(100, sameDate = true, sameYear = true))
    }

    @Test
    fun inboxTimestampUsesMonthDayForOlderDateInSameYear() {
        assertEquals("Jun 9", formatInboxTimestampForTest(100, sameDate = false, sameYear = true))
    }

    @Test
    fun inboxTimestampIncludesYearForPriorYearDate() {
        assertEquals("Dec 31, 2025", formatInboxTimestampForTest(100, sameDate = false, sameYear = false))
    }

    @Test
    fun messageFullTimestampUsesFullDateTimeStyle() {
        val formatted =
            formatMessageFullTimestamp(
                epochSeconds = 100,
                dateFormatter = { _, style ->
                    when (style) {
                        DateStyle.Time -> "09:05"
                        DateStyle.Weekday -> "Tue"
                        DateStyle.MonthDay -> "Jun 9"
                        DateStyle.MonthDayYear -> "Dec 31, 2025"
                        DateStyle.FullTimestamp -> "Wed, Jun 10, 2026, 09:05"
                    }
                },
            )

        assertEquals("Wed, Jun 10, 2026, 09:05", formatted)
    }

    private fun formatInboxTimestampForTest(
        epochSeconds: Long,
        sameDate: Boolean = false,
        sameYear: Boolean = false,
    ): String =
        formatInboxTimestamp(
            epochSeconds = epochSeconds,
            nowMillis = 200_000,
            sameLocalDate = { _, _ -> sameDate },
            sameLocalYear = { _, _ -> sameYear },
            dateFormatter = { _, style ->
                when (style) {
                    DateStyle.Time -> "09:05"
                    DateStyle.Weekday -> "Tue"
                    DateStyle.MonthDay -> "Jun 9"
                    DateStyle.MonthDayYear -> "Dec 31, 2025"
                    DateStyle.FullTimestamp -> "Wed, Jun 10, 2026, 09:05"
                }
            },
        )
}
