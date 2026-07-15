package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import kotlin.test.Test
import kotlin.test.assertEquals

class RssAccountLifecycleTest {
    @Test
    fun accountNameStartsAtRss() {
        assertEquals("RSS", nextRssAccountDisplayName(emptyList()))
    }

    @Test
    fun accountNameUsesFirstAvailableSuffix() {
        val accounts = listOf(rss("RSS"), rss("RSS1"), rss("RSS3"))

        assertEquals("RSS2", nextRssAccountDisplayName(accounts))
    }

    @Test
    fun accountNameIgnoresCaseAndNonRssAccounts() {
        val accounts = listOf(rss(" rss "), mail("RSS1"))

        assertEquals("RSS1", nextRssAccountDisplayName(accounts))
    }

    private fun rss(name: String) = AccountSummary(id = "rss-$name", email = "", displayName = name, engine = "rss")

    private fun mail(name: String) = AccountSummary(id = "mail-$name", email = "$name@example.com", displayName = name, engine = "imap")
}
