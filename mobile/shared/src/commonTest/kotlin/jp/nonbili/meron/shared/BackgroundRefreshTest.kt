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
    fun pollIntervalCyclesThroughKnownOptions() {
        assertEquals(5, nextPollIntervalMinutes(0))
        assertEquals(15, nextPollIntervalMinutes(5))
        assertEquals(0, nextPollIntervalMinutes(60))
        // Unknown stored values snap to 15 before advancing.
        assertEquals(15, coercePollIntervalMinutes(7))
        assertEquals(30, coercePollIntervalMinutes(30))
        assertEquals(30, nextPollIntervalMinutes(7))
    }

    @Test
    fun backgroundDelayFloorsAtFifteenMinutes() {
        assertEquals(15 * 60.0, backgroundRefreshDelaySeconds(0))
        assertEquals(15 * 60.0, backgroundRefreshDelaySeconds(5))
        assertEquals(30 * 60.0, backgroundRefreshDelaySeconds(30))
    }

    @Test
    fun summaryMatchesMobileNotificationText() {
        assertEquals("2 account(s) refreshed, 1 failed", backgroundRefreshSummary(refreshed = 2, skipped = 0, failed = 1))
        assertEquals("2 account(s) refreshed", backgroundRefreshSummary(refreshed = 2, skipped = 0, failed = 0))
        assertEquals("No accounts refreshed; 3 skipped", backgroundRefreshSummary(refreshed = 0, skipped = 3, failed = 0))
        assertEquals("No accounts configured", backgroundRefreshSummary(refreshed = 0, skipped = 0, failed = 0))
    }
}
