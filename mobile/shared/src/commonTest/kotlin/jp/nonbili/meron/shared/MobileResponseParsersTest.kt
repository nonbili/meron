package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class MobileResponseParsersTest {
    @Test
    fun parsesAccountListEnvelope() {
        val accounts = parseAccountListResponse(
            """{"id":1,"result":{"accounts":[{"id":"acc1","email":"me@example.com","display_name":"Me","needs_reconnect":false}]}}""",
        )

        assertEquals(1, accounts.size)
        assertEquals("acc1", accounts[0].id)
        assertEquals("me@example.com", accounts[0].email)
        assertEquals("Me", accounts[0].displayName)
        assertFalse(accounts[0].needsReconnect)
    }

    @Test
    fun parsesThreadListEnvelope() {
        val threads = parseThreadListResponse(
            """{"id":2,"result":{"threads":[{"id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","from_name":"Ada","subject":"Hello","preview":"Snippet","date":1700000000,"unread":true,"starred":true}]}}""",
        )

        assertEquals(1, threads.size)
        assertEquals("acc#INBOX#t", threads[0].id)
        assertEquals("acc", threads[0].accountId)
        assertEquals("INBOX", threads[0].folder)
        assertEquals("Ada", threads[0].sender)
        assertEquals("Hello", threads[0].subject)
        assertEquals("Snippet", threads[0].preview)
        assertEquals(1_700_000_000, threads[0].dateEpochSeconds)
        assertTrue(threads[0].unread)
        assertTrue(threads[0].starred)
    }

    @Test
    fun toleratesBareResultShape() {
        val accounts = parseAccountListResponse("""{"accounts":[{"id":"rss-1","email":"rss-1.local"}]}""")

        assertEquals("rss-1", accounts.single().id)
    }

    @Test
    fun parsesAccountEngineMetadataForRssBranching() {
        val accounts = parseAccountListResponse(
            """{"accounts":[{"id":"rss-1","email":"rss-1.local","display_name":"Feeds","provider":"rss","auth_type":"rss"},{"id":"mail-1","email":"me@example.com","engine":"meron_mail","provider":"gmail","auth_type":"gmail_oauth"}]}""",
        )

        assertTrue(accountSummaryIsRss(accounts[0]))
        assertFalse(accountSummaryIsRss(accounts[1]))
        assertTrue(threadIdIsRss("rss-1#rss#feed-1"))
    }

    @Test
    fun parsesFolderListEnvelopeAndRssFolderShape() {
        val folders = parseFolderListResponse(
            """{"id":4,"result":{"folders":[{"id":"INBOX","account_id":"acc","name":"INBOX","unread":3},{"id":"inbox","role":"inbox","unread":2}]}}""",
        )

        assertEquals(2, folders.size)
        assertEquals("acc", folders[0].accountId)
        assertEquals("INBOX", folders[0].name)
        assertEquals(3, folders[0].unread)
        assertEquals("inbox", folders[1].name)
        assertEquals(2, folders[1].unread)
    }

    @Test
    fun parsesThreadReadEnvelope() {
        val messages = parseThreadReadResponse(
            """{"id":3,"result":{"messages":[{"id":"acc#INBOX#t#9","from_name":"Ada","from_addr":"ada@example.com","to":"Me <me@example.com>","subject":"Cached subject","body":"Hello from cache","date":300,"unread":true,"reply_to":"Team <team@example.com>","message_id":"m1@example.com","references":"<root@example.com>"}]}}""",
        )

        assertEquals(1, messages.size)
        assertEquals("acc#INBOX#t#9", messages[0].id)
        assertEquals("Ada", messages[0].from)
        assertEquals("Me <me@example.com>", messages[0].to)
        assertEquals("Cached subject", messages[0].subject)
        assertEquals("Hello from cache", messages[0].body)
        assertEquals(300, messages[0].dateEpochSeconds)
        assertEquals("ada@example.com", messages[0].fromAddr)
        assertEquals("Team <team@example.com>", messages[0].replyTo)
        assertEquals("m1@example.com", messages[0].messageId)
        assertEquals("<root@example.com>", messages[0].references)
    }
}
