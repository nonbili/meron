package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CalendarItem
import jp.nonbili.meron.shared.TaskListSummary
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SyncServicesTest {
    @Test
    fun calendarColorsMatchDesktopsPicks() {
        assertEquals("#0082c9", calendarColorHex("cal-1", "#0082C9"))
        // The same presets desktop's calendarColor picks for a calendar
        // without a colour of its own, so a calendar looks the same on both.
        assertEquals("#f59e0b", calendarColorHex("cal-1", ""))
        assertEquals("#ef4444", calendarColorHex("https://dav.example.test/cal/work/", ""))
        assertEquals("#14b8a6", calendarColorHex("Ωmega-calendar-identifier-that-is-long-enough-to-overflow", ""))
    }

    @Test
    fun hiddenCalendarsLeaveTasksAndOtherCalendarsShown() {
        fun item(
            key: String,
            source: String = "event",
            calendar: String = "",
        ) = CalendarItem(id = key, key = key, source = source, title = key, start = 0, end = 0, calendar = calendar)
        val items = listOf(item("here"), item("work", calendar = "c1"), item("home", calendar = "c2"), item("task", source = "task"))
        assertEquals(items, visibleCalendarItems(items, emptySet()))
        assertEquals(listOf("home", "task"), visibleCalendarItems(items, setOf(LOCAL_CALENDAR, "c1")).map { it.key })
    }

    @Test
    fun taskListsGroupByAccountAfterTheLocalOnes() {
        val groups =
            taskListGroups(
                listOf(
                    TaskListSummary("g1", "My Tasks", provider = "google", account = "sam@example.test"),
                    TaskListSummary("local", "My Tasks"),
                    TaskListSummary("c1", "Work", provider = "caldav", account = "sam@example.test"),
                    TaskListSummary("g2", "My Tasks", provider = "google", account = "alex@example.test"),
                    TaskListSummary("g3", "Errands", provider = "google", account = "sam@example.test"),
                ),
            )
        assertEquals(
            listOf(
                "" to listOf("local"),
                "sam@example.test · Google Tasks" to listOf("g1", "g3"),
                "sam@example.test · CalDAV" to listOf("c1"),
                "alex@example.test · Google Tasks" to listOf("g2"),
            ),
            groups.map { group -> group.label to group.lists.map { it.id } },
        )
    }

    @Test
    fun taskListsOfTwoServersWithOneUsernameNameTheirServers() {
        val groups =
            taskListGroups(
                listOf(
                    TaskListSummary("a", "Tasks", provider = "caldav", account = "sam", accountId = "1", server = "https://dav.one.test/"),
                    TaskListSummary("b", "Tasks", provider = "caldav", account = "sam", accountId = "2", server = "https://dav.two.test/"),
                ),
            )
        assertEquals(listOf("sam · CalDAV (dav.one.test)", "sam · CalDAV (dav.two.test)"), groups.map { it.label })
    }

    @Test
    fun plainHttpWarnsOnlyForAnotherMachine() {
        assertTrue(sendsPlainText("http://dav.example.test/"))
        assertTrue(sendsPlainText(" HTTP://192.168.1.5:8080/dav "))
        assertFalse(sendsPlainText("https://dav.example.test/"))
        assertFalse(sendsPlainText("http://localhost:5232/"))
        assertFalse(sendsPlainText("http://127.0.0.1/"))
        assertFalse(sendsPlainText("http://[::1]:8080/"))
    }

    @Test
    fun parsesAccountsAsTheCoreReportsThem() {
        val accounts =
            parseDavAccounts(
                Json
                    .parseToJsonElement(
                        """{"accounts":[{"id":"a1","server_url":"https://dav.example.test/","username":"mira@example.test","google":false,
                        "display_name":"","name":"dav.example.test","last_sync_at":1700000000,"last_error":"","sync_interval":15,
                        "calendars":[{"id":"c1","name":"Work","color":"#ff0000","read_only":true,"enabled":true,"events":true,"tasks":false}]}]}""",
                    ).jsonObject,
            )
        val account = accounts.single()
        assertEquals("dav.example.test", account.name)
        assertEquals(15, account.syncInterval)
        assertEquals(1_700_000_000, account.lastSyncAt)
        assertTrue(account.calendars.single().readOnly)
        assertFalse(account.calendars.single().tasksEnabled)
    }
}

class RepeatRuleTest {
    @Test
    fun keepsTheCanonicalFormTheCoreStores() {
        for (stored in listOf(
            "weekly",
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE",
            "FREQ=MONTHLY;BYDAY=-1FR;COUNT=6",
            "FREQ=MONTHLY;BYMONTHDAY=20;UNTIL=20270630",
            "FREQ=YEARLY;INTERVAL=3",
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;WKST=SU",
        )) {
            assertEquals(stored, formatRepeat(parseRepeat(stored)!!))
        }
        assertEquals("daily", formatRepeat(parseRepeat("FREQ=DAILY")!!))
        assertEquals("FREQ=WEEKLY;BYDAY=MO,WE", formatRepeat(parseRepeat("FREQ=WEEKLY;BYDAY=WE,MO,WE")!!))
        // A week start is kept only where it picks the weeks.
        assertEquals("FREQ=WEEKLY;BYDAY=TU,TH", formatRepeat(parseRepeat("FREQ=WEEKLY;BYDAY=TU,TH;WKST=SU")!!))
    }

    @Test
    fun leavesOutWhatTheEditorCannotSay() {
        assertNull(parseRepeat(""))
        assertNull(parseRepeat("custom"))
        assertNull(parseRepeat("FREQ=HOURLY"))
        assertNull(parseRepeat("FREQ=DAILY;BYDAY=MO"))
        assertNull(parseRepeat("FREQ=MONTHLY;BYDAY=MO,TU;BYSETPOS=-1"))
    }

    @Test
    fun startsTheCustomEditorFromTheEventsOwnDay() {
        // Friday 20 November 2026: the third Friday, not the last.
        val day = CivilDate(2026, 11, 20)
        assertEquals(listOf(4), initialRule("", day).weekdays)
        assertEquals(MonthlyOn.Day(20), initialRule("monthly", day).monthly)
        assertEquals(3 to false, weekdayPlace(day))
        assertEquals(4 to true, weekdayPlace(CivilDate(2026, 11, 27)))
    }

    @Test
    fun movesWithItsSeriesAsTheCoreMovesIt() {
        val friday = CivilDate(2026, 11, 20)
        val tuesday = CivilDate(2026, 11, 24)
        assertEquals("FREQ=WEEKLY;BYDAY=TU,FR;COUNT=4", alignRepeat("FREQ=WEEKLY;BYDAY=MO,FR;COUNT=4", friday, tuesday))
        assertEquals("FREQ=WEEKLY;BYDAY=TU,SA", alignRepeat("FREQ=WEEKLY;BYDAY=TU,FR", friday, tuesday))
        assertEquals("FREQ=MONTHLY;BYMONTHDAY=24", alignRepeat("FREQ=MONTHLY;BYMONTHDAY=20", friday, tuesday))
        assertEquals("FREQ=MONTHLY;BYDAY=4TU", alignRepeat("FREQ=MONTHLY;BYDAY=3FR", friday, tuesday))
        assertEquals("FREQ=MONTHLY;BYDAY=-1TU", alignRepeat("FREQ=MONTHLY;BYDAY=-1FR", CivilDate(2026, 11, 27), tuesday))
        assertEquals("weekly", alignRepeat("weekly", friday, tuesday))
        // An every-other-week rule's weeks move with it.
        assertEquals(
            "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,TU;WKST=TU",
            alignRepeat("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,SU", CivilDate(2026, 10, 11), CivilDate(2026, 10, 12)),
        )
    }

    @Test
    fun describesEachPart() {
        // Echo keys and values, so the shape can be checked without a catalog.
        val t = { key: String, values: Map<String, Any?> ->
            "${key.removePrefix("calendar.repeat.")}(${values.entries.joinToString(",") { "${it.key}=${it.value}" }})"
        }
        val count = describeRepeat(parseRepeat("FREQ=MONTHLY;BYDAY=-1FR;COUNT=6")!!, t)
        assertTrue(count.startsWith("summaryCount(rule=summaryOnWeekday(rule=everyMonths(count=1),nth=nthLast(),weekday="), count)
        assertTrue(count.endsWith(",count=6)"), count)
        assertTrue(describeRepeat(parseRepeat("FREQ=MONTHLY;BYDAY=5FR")!!, t).contains("nth=nth5()"))
    }
}
