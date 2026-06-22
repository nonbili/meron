package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class BackgroundRefreshTest {
    @Test
    fun accountEligibilityRequiresActiveConnectedAccount() {
        assertTrue(
            shouldBackgroundRefreshAccount(
                accountId = "user1@mail.localhost",
                paused = false,
                needsReconnect = false,
            ),
        )
        assertFalse(shouldBackgroundRefreshAccount(accountId = "", paused = false, needsReconnect = false))
        assertFalse(shouldBackgroundRefreshAccount(accountId = "a1", paused = true, needsReconnect = false))
        assertFalse(shouldBackgroundRefreshAccount(accountId = "a1", paused = false, needsReconnect = true))
    }

    @Test
    fun rssAccountMetadataUsesRssRefreshProtocol() {
        assertTrue(backgroundRefreshUsesRssProtocol(engine = "rss", provider = "custom", authType = "password"))
        assertTrue(backgroundRefreshUsesRssProtocol(engine = "mail", provider = "rss", authType = "password"))
        assertTrue(backgroundRefreshUsesRssProtocol(engine = "mail", provider = "custom", authType = "rss"))
        assertFalse(backgroundRefreshUsesRssProtocol(engine = "mail", provider = "gmail", authType = "gmail_oauth"))
    }

    @Test
    fun summaryMatchesMobileNotificationText() {
        assertEquals("2 account(s) refreshed, 1 failed", backgroundRefreshSummary(refreshed = 2, skipped = 0, failed = 1))
        assertEquals("2 account(s) refreshed", backgroundRefreshSummary(refreshed = 2, skipped = 0, failed = 0))
        assertEquals("No accounts refreshed; 3 skipped", backgroundRefreshSummary(refreshed = 0, skipped = 3, failed = 0))
        assertEquals("No accounts configured", backgroundRefreshSummary(refreshed = 0, skipped = 0, failed = 0))
    }
}
