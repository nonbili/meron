package jp.nonbili.meron

import jp.nonbili.meron.shared.FolderSummary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AndroidUnreadWidgetTest {
    @Test
    fun feedRowsOpenTheAppWhileMailRowsKeepTheirThreadTarget() {
        assertNull(widgetThreadTarget("rss-a#rss#subscription"))
        assertEquals("uid:51", widgetThreadTarget("a#INBOX#51")?.threadKey)
    }

    @Test
    fun countsOnlyTheInbox() {
        val folders =
            listOf(
                FolderSummary(accountId = "a", name = "Notifications", unread = 9),
                FolderSummary(accountId = "a", name = "INBOX", unread = 3, role = "inbox"),
            )
        assertEquals(3, widgetInbox(folders)?.unread)
    }

    @Test
    fun matchesInboxByNameWithoutRole() {
        assertEquals(4, widgetInbox(listOf(FolderSummary(accountId = "a", name = "Inbox", unread = 4)))?.unread)
        assertNull(widgetInbox(emptyList()))
    }

    @Test
    fun capsTheLabel() {
        assertEquals("0", widgetCountLabel(0))
        assertEquals("999", widgetCountLabel(999))
        assertEquals("999+", widgetCountLabel(1000))
    }
}
