package jp.nonbili.meron.ui

import java.util.Locale
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals

/** Due dates must stay Gregorian under a locale whose default calendar is not. */
class TaskDueDateLocaleTest {
    private val original = Locale.getDefault()

    @BeforeTest
    fun useBuddhistCalendarLocale() {
        Locale.setDefault(Locale.forLanguageTag("th-TH"))
    }

    @AfterTest
    fun restoreLocale() {
        Locale.setDefault(original)
    }

    @Test
    fun aDueDateRoundTripsUnderThaiLocale() {
        val due = parseTaskDueInput("2026-06-09")
        assertEquals("2026-06-09", formatTaskDueInput(due))
        // 2026-06-09T00:00:00Z
        assertEquals(1_780_963_200_000L, taskDueToPickerMillis(due))
        assertEquals(due, pickerMillisToTaskDue(1_780_963_200_000L))
    }
}
