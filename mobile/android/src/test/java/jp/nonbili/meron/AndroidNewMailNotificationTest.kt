package jp.nonbili.meron

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class AndroidNewMailNotificationTest {
    private fun item(
        uid: Long = 0,
        from: String = "",
        subject: String = "",
        preview: String = "",
        threadKey: String = "",
        itemKey: String = "",
    ) = NewMailItem(
        uid = uid,
        from = from,
        subject = subject,
        preview = preview,
        threadKey = threadKey,
        date = 0,
        itemKey = itemKey,
    )

    @Test
    fun childTextShowsSubjectAndBodySnippet() {
        assertEquals(
            "Lunch? - Are we still on for Friday",
            newMailChildText("Lunch?", "Are we still on for Friday"),
        )
        // Expanded form puts the body under the subject rather than after it.
        assertEquals(
            "Lunch?\nAre we still on for Friday",
            newMailChildBigText("Lunch?", "Are we still on for Friday"),
        )
    }

    @Test
    fun childTextDegradesWhenTheBodyIsNotCachedYet() {
        assertEquals("Lunch?", newMailChildText("Lunch?", ""))
        assertEquals("Lunch?", newMailChildBigText("Lunch?", "   "))
        // Nothing at all to show still beats a blank notification line.
        assertEquals("New mail arrived", newMailChildText("", ""))
        assertEquals("New mail arrived", newMailChildBigText(" ", ""))
    }

    @Test
    fun childTitleFallsBackFromSenderToAccount() {
        assertEquals("Aki", newMailChildTitle("Aki", "me@example.com"))
        assertEquals("me@example.com", newMailChildTitle("  ", "me@example.com"))
        assertEquals("New mail", newMailChildTitle("", ""))
    }

    @Test
    fun feedEntriesSharingATitleKeepSeparateNotifications() {
        // A feed is one thread, so two entries of it share a thread key; keying
        // on thread + subject would let the second replace the first in the
        // shade. The entry's own key is what tells them apart.
        val monday = item(subject = "Daily digest", threadKey = "sub-1", itemKey = "key-monday")
        val tuesday = item(subject = "Daily digest", threadKey = "sub-1", itemKey = "key-tuesday")
        assertNotEquals(
            newMailNotificationId("rss-8f2a", monday),
            newMailNotificationId("rss-8f2a", tuesday),
        )
        // The same entry re-posted (a manual refresh racing the periodic one)
        // still updates its own notification rather than stacking a duplicate.
        assertEquals(
            newMailNotificationId("rss-8f2a", monday),
            newMailNotificationId("rss-8f2a", monday.copy(subject = "Daily digest (updated)")),
        )
    }

    @Test
    fun feedEntriesAreIdentifiedWithinTheirOwnFeed() {
        // An entry key is a hash of the GUID and is unique only inside one
        // subscription — core scopes the stored row by feed too. Two feeds
        // carrying the same post (a site feed and its category feed) must not
        // knock each other out of the shade.
        assertNotEquals(
            newMailNotificationId("rss-8f2a", item(threadKey = "sub-1", itemKey = "shared-guid")),
            newMailNotificationId("rss-8f2a", item(threadKey = "sub-2", itemKey = "shared-guid")),
        )
    }

    @Test
    fun feedArrivalsOfferNoArchiveButton() {
        // `mail.archive` does not route feed threads and an RSS account has no
        // archive folder, so the button could only fail — after the receiver had
        // already cleared the row and offered an undo.
        assertEquals(false, newMailSupportsArchive("rss-8f2a"))
        assertEquals(true, newMailSupportsArchive("me@example.com"))
    }

    @Test
    fun summaryLinesReadAsGmailsDo() {
        assertEquals("Aki - Lunch?", newMailInboxLine("Aki", "Lunch?"))
        assertEquals("Aki", newMailInboxLine("Aki", ""))
    }

    @Test
    fun notificationIdIsStablePerMessageAndDistinctPerAccount() {
        val first = item(uid = 42, subject = "Lunch?")
        // Re-posting the same mail (push racing a periodic refresh) updates the
        // one notification instead of stacking a duplicate.
        assertEquals(
            newMailNotificationId("me@example.com", first),
            newMailNotificationId("me@example.com", item(uid = 42, subject = "Lunch? (edited)")),
        )
        assertNotEquals(
            newMailNotificationId("me@example.com", first),
            newMailNotificationId("work@example.com", first),
        )
        assertNotEquals(
            newMailNotificationId("me@example.com", first),
            newMailNotificationId("me@example.com", item(uid = 43, subject = "Lunch?")),
        )
        // Same-account arrivals never collide with that account's summary.
        assertNotEquals(newMailSummaryId("me@example.com"), newMailNotificationId("me@example.com", first))
    }

    @Test
    fun uidlessPayloadsKeyOffTheThread() {
        val feedItem = item(threadKey = "feed-1", subject = "Release notes")
        assertEquals(
            newMailNotificationId("rss", feedItem),
            newMailNotificationId("rss", item(threadKey = "feed-1", subject = "Release notes")),
        )
        assertNotEquals(
            newMailNotificationId("rss", feedItem),
            newMailNotificationId("rss", item(threadKey = "feed-2", subject = "Release notes")),
        )
    }

    @Test
    fun groupKeySeparatesAccounts() {
        assertNotEquals(newMailGroupKey("me@example.com"), newMailGroupKey("work@example.com"))
    }
}
