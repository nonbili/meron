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
        val today = epochSecondsForLocalDate(2026, 6, 10)
        val yesterday = epochSecondsForLocalDate(2026, 6, 9)
        val tomorrow = epochSecondsForLocalDate(2026, 6, 11)
        val noon = today * 1000 + 12 * 60 * 60 * 1000
        assertEquals(true, taskIsOverdue(yesterday, noon))
        assertEquals(false, taskIsOverdue(today, noon))
        assertEquals(false, taskIsOverdue(tomorrow, noon))
        assertEquals(false, taskIsOverdue(0, noon))
        assertEquals(false, taskIsOverdue(today, tomorrow * 1000 - 1))
        assertEquals(true, taskIsOverdue(today, tomorrow * 1000))
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

    @Test
    fun aPickedDueDateRoundTripsThroughUtc() {
        val due = parseTaskDueInput("2026-06-09")
        // 2026-06-09T00:00:00Z
        assertEquals(1_780_963_200_000L, taskDueToPickerMillis(due))
        assertEquals(due, pickerMillisToTaskDue(1_780_963_200_000L))
        assertEquals("2024-02-29", formatTaskDueInput(pickerMillisToTaskDue(taskDueToPickerMillis(parseTaskDueInput("2024-02-29"))!!)))
        assertEquals(null, taskDueToPickerMillis(0))
    }
}
