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
                        DateStyle.IsoDate -> "2026-06-10"
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
                    DateStyle.IsoDate -> "2026-06-10"
                }
            },
        )

    @Test
    fun taskDueDateDropsTheYearOnlyWhenItIsTheCurrentOne() {
        val formatter = { _: Long, style: DateStyle ->
            when (style) {
                DateStyle.MonthDay -> "Jun 9"
                DateStyle.MonthDayYear -> "Jun 9, 2025"
                else -> ""
            }
        }
        assertEquals(
            "Jun 9",
            formatTaskDueDate(epochSeconds = 100, sameLocalYear = { _, _ -> true }, dateFormatter = formatter),
        )
        assertEquals(
            "Jun 9, 2025",
            formatTaskDueDate(epochSeconds = 100, sameLocalYear = { _, _ -> false }, dateFormatter = formatter),
        )
        assertEquals("", formatTaskDueDate(epochSeconds = 0, dateFormatter = formatter))
    }

    @Test
    fun onlyAPastDueDateCountsAsOverdue() {
        assertEquals(true, taskIsOverdue(dueAtSeconds = 100, nowMillis = 200_000))
        assertEquals(false, taskIsOverdue(dueAtSeconds = 400, nowMillis = 200_000))
        // "No due date" is 0, which must never read as overdue.
        assertEquals(false, taskIsOverdue(dueAtSeconds = 0, nowMillis = 200_000))
    }

    @Test
    fun aTypedDueDateRoundTripsAndRejectsNonsense() {
        val epoch = parseTaskDueInput("2026-06-09")
        assertEquals(true, epoch > 0)
        assertEquals("2026-06-09", formatTaskDueInput(epoch))

        assertEquals(0, parseTaskDueInput(""))
        assertEquals(0, parseTaskDueInput("tomorrow"))
        assertEquals(0, parseTaskDueInput("2026-13-09"))
        assertEquals(0, parseTaskDueInput("2026-06"))
        assertEquals("", formatTaskDueInput(0))
    }
}
