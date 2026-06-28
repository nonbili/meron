package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MobileResponseParsersTest {
    @Test
    fun parsesAccountListEnvelope() {
        val accounts =
            parseAccountListResponse(
                """{"id":1,"result":{"accounts":[{"id":"acc1","email":"me@example.com","display_name":"Me","needs_reconnect":false}]}}""",
            )

        assertEquals(1, accounts.size)
        assertEquals("acc1", accounts[0].id)
        assertEquals("me@example.com", accounts[0].email)
        assertEquals("Me", accounts[0].displayName)
        assertFalse(accounts[0].needsReconnect)
    }

    @Test
    fun parsesStorageUsageEnvelope() {
        val usage =
            parseStorageUsageResponse(
                """{"id":40,"result":{"cacheBytes":1234,"dbBytes":5678}}""",
            )

        assertEquals(1234, usage.cacheBytes)
        assertEquals(5678, usage.dbBytes)
    }

    @Test
    fun parsesAutodiscoverEnvelope() {
        val discovered =
            parseAutodiscoverResponse(
                """{"id":10,"result":{"imap_host":"imap.gmail.com","imap_port":993,"smtp_host":"smtp.gmail.com","smtp_port":465,"username":"me@gmail.com","provider_name":"Gmail","source":"known","app_password_hint":{"provider":"Gmail","url":"https://example.com/passwords"}}}""",
            )

        assertEquals("imap.gmail.com", discovered.imapHost)
        assertEquals(993, discovered.imapPort)
        assertEquals("smtp.gmail.com", discovered.smtpHost)
        assertEquals(465, discovered.smtpPort)
        assertEquals("me@gmail.com", discovered.username)
        assertEquals("Gmail", discovered.providerName)
        assertEquals("known", discovered.source)
        assertEquals("Gmail", discovered.appPasswordProvider)
        assertEquals("https://example.com/passwords", discovered.appPasswordUrl)
    }

    @Test
    fun parsesAccountPrefsAndAliases() {
        val accounts =
            parseAccountListResponse(
                """{"accounts":[{"id":"acc1","email":"me@example.com","display_name":"Me","sender_name":"Sender","avatar_url":"https://example.com/avatar.png","load_remote_images":true,"included_in_unified":false,"muted":true,"paused":true,"conversation_html":false,"rss_sync_interval_minutes":30,"chat_wallpaper":{"kind":"preset","presetId":"grid"},"aliases":[{"email":"alias@example.com","name":"Alias"}]}]}""",
            )

        val account = accounts.single()
        assertEquals("Sender", account.senderName)
        assertEquals("https://example.com/avatar.png", account.avatarUrl)
        assertTrue(account.loadRemoteImages)
        assertFalse(account.includedInUnified)
        assertTrue(account.muted)
        assertTrue(account.paused)
        assertFalse(account.conversationHtml)
        assertEquals(30, account.rssSyncIntervalMinutes)
        assertEquals("alias@example.com", account.aliases.single().email)
        assertEquals("Alias", account.aliases.single().name)
        assertEquals("preset", account.chatWallpaperKind)
        assertEquals("grid", account.chatWallpaperPresetId)
    }

    @Test
    fun parsesThreadListEnvelope() {
        val threads =
            parseThreadListResponse(
                """{"id":2,"result":{"threads":[{"id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","from_name":"Ada","subject":"Hello","preview":"Snippet","date":1700000000,"unread":true,"starred":true,"feed_url":"https://example.com/feed.xml"}]}}""",
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
        assertEquals("https://example.com/feed.xml", threads[0].feedUrl)
    }

    @Test
    fun parsesThreadListNextCursor() {
        val page =
            parseThreadListPage(
                """{"id":2,"result":{"threads":[{"id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","date":1700000000}],"next_cursor":"1700000000:1"}}""",
            )

        assertEquals(1, page.threads.size)
        assertEquals("1700000000:1", page.nextCursor)
    }

    @Test
    fun toleratesBareResultShape() {
        val accounts = parseAccountListResponse("""{"accounts":[{"id":"rss-1","email":"rss-1.local"}]}""")

        assertEquals("rss-1", accounts.single().id)
    }

    @Test
    fun parsesAccountEngineMetadataForRssBranching() {
        val accounts =
            parseAccountListResponse(
                """{"accounts":[{"id":"rss-1","email":"rss-1.local","display_name":"Feeds","provider":"rss","auth_type":"rss"},{"id":"mail-1","email":"me@example.com","engine":"meron_mail","provider":"gmail","auth_type":"gmail_oauth"}]}""",
            )

        assertTrue(accountSummaryIsRss(accounts[0]))
        assertFalse(accountSummaryIsRss(accounts[1]))
        assertTrue(threadIdIsRss("rss-1#rss#feed-1"))
    }

    @Test
    fun parsesFolderListEnvelopeAndRssFolderShape() {
        val folders =
            parseFolderListResponse(
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
    fun parsesContactSuggestionsAndFormatsRecipients() {
        val contacts =
            parseContactSuggestResponse(
                """{"id":5,"result":{"contacts":[{"name":"Bea","addr":"bea@example.com"},{"name":"","addr":"aki@example.com"}]}}""",
            )

        assertEquals(2, contacts.size)
        assertEquals("Bea <bea@example.com>", formatContactSuggestion(contacts[0]))
        assertEquals("aki@example.com", formatContactSuggestion(contacts[1]))
        assertEquals("ada@example.com, Bea <bea@example.com>, ", replaceRecipientTail("ada@example.com, be", contacts[0]))
        assertEquals("be", recipientTail("ada@example.com, be"))
    }

    @Test
    fun buildsSendIdentitiesAndDetectsReplyAlias() {
        val account =
            AccountSummary(
                id = "acc1",
                email = "me@example.com",
                senderName = "Me",
                aliases = listOf(AccountAlias(email = "alias@example.com")),
            )
        val identities = accountSendIdentities(account)

        assertEquals(2, identities.size)
        assertEquals("Me <me@example.com>", formatSendIdentity(identities[0]))
        assertEquals("Me <alias@example.com>", formatSendIdentity(identities[1]))
        assertEquals(
            "alias@example.com",
            detectReplyFromIdentity(
                MessageBody(
                    id = "m1",
                    from = "Ada",
                    to = "Alias <alias@example.com>",
                    cc = "",
                    subject = "Hello",
                    body = "Body",
                ),
                account,
            ),
        )
        assertEquals(
            "",
            detectReplyFromIdentity(
                MessageBody(
                    id = "m2",
                    from = "Ada",
                    to = "Me <me@example.com>",
                    cc = "",
                    subject = "Hello",
                    body = "Body",
                ),
                account,
            ),
        )
        assertEquals(
            "alias@example.com",
            detectReplyFromIdentity(
                MessageBody(
                    id = "m3",
                    from = "Ada",
                    to = "Team <team@example.com>",
                    cc = "Alias <alias@example.com>",
                    subject = "Hello",
                    body = "Body",
                ),
                account,
            ),
        )
    }

    @Test
    fun parsesThreadReadEnvelope() {
        val messages =
            parseThreadReadResponse(
                """{"id":3,"result":{"messages":[{"id":"acc#INBOX#t#9","from_name":"Ada","from_addr":"ada@example.com","to":"Me <me@example.com>","cc":"Copy <copy@example.com>","bcc":"Hidden <hidden@example.com>","subject":"Cached subject","body":"Hello from cache","body_html":"<p>Hello from cache</p>","date":300,"unread":true,"starred":true,"reply_to":"Team <team@example.com>","message_id":"m1@example.com","references":"<root@example.com>","has_attachments":true,"attachments":[{"filename":"note.txt","mime":"text/plain","size":2,"key":"acc/INBOX/9/1.txt"},{"filename":"remote.jpg","mime":"image/jpeg","size":0,"url":"https://example.com/remote.jpg"}]}]}}""",
            )

        assertEquals(1, messages.size)
        assertEquals("acc#INBOX#t#9", messages[0].id)
        assertEquals("Ada", messages[0].from)
        assertEquals("Me <me@example.com>", messages[0].to)
        assertEquals("Copy <copy@example.com>", messages[0].cc)
        assertEquals("Hidden <hidden@example.com>", messages[0].bcc)
        assertEquals("Cached subject", messages[0].subject)
        assertEquals("Hello from cache", messages[0].body)
        assertEquals("<p>Hello from cache</p>", messages[0].bodyHtml)
        assertEquals(300, messages[0].dateEpochSeconds)
        assertEquals("ada@example.com", messages[0].fromAddr)
        assertEquals("Team <team@example.com>", messages[0].replyTo)
        assertEquals("m1@example.com", messages[0].messageId)
        assertEquals("<root@example.com>", messages[0].references)
        assertTrue(messages[0].unread)
        assertTrue(messages[0].starred)
        assertTrue(messages[0].hasAttachments)
        assertEquals(2, messages[0].attachments.size)
        assertEquals("note.txt", messages[0].attachments[0].filename)
        assertEquals("text/plain", messages[0].attachments[0].mimeType)
        assertEquals(2, messages[0].attachments[0].sizeBytes)
        assertEquals("acc/INBOX/9/1.txt", messages[0].attachments[0].key)
        assertEquals("https://example.com/remote.jpg", messages[0].attachments[1].url)
    }

    @Test
    fun parsesThreadReadNextCursor() {
        val page =
            parseThreadReadPage(
                """{"id":3,"result":{"messages":[{"id":"acc#INBOX#t#8","date":200}],"next_cursor":"uid:8"}}""",
            )

        assertEquals(1, page.messages.size)
        assertEquals("uid:8", page.nextCursor)
    }

    @Test
    fun parsesMediaFileUrlResponse() {
        assertEquals(
            "/media/avatars/acc/avatar.png",
            parseMediaFileUrlResponse("""{"url":"/media/avatars/acc/avatar.png"}"""),
        )
        assertEquals("", parseMediaFileUrlResponse("""{}"""))
        assertEquals("SGk=", parseAttachmentDataResponse("""{"data":"SGk="}"""))
    }

    @Test
    fun parsesMessageFolderIdSoDeletesTargetTheRightFolder() {
        // A thread can span folders; each message carries its own folder_id, which
        // delete/move must use instead of the thread's nominal folder.
        val page =
            parseThreadReadPage(
                """{"messages":[{"id":"acc#INBOX#t#7","folder_id":"INBOX"},{"id":"acc#INBOX#t#17","folder_id":"sent"}]}""",
            )

        assertEquals(2, page.messages.size)
        assertEquals("INBOX", page.messages[0].folderId)
        assertEquals("sent", page.messages[1].folderId)
    }

    @Test
    fun detectsCoreErrorEnvelope() {
        assertEquals(
            "UID COPY: no response",
            coreErrorMessage("""{"error":{"message":"UID COPY: no response"},"id":1}"""),
        )
        // Success payloads have no top-level error object.
        assertNull(coreErrorMessage("""{"ok":true,"deleted":1,"trash":"trash"}"""))
    }

    @Test
    fun requireCoreOkThrowsOnErrorAndPassesThroughSuccess() {
        val success = """{"ok":true,"deleted":1}"""
        assertEquals(success, requireCoreOk(success))

        val failure =
            assertFailsWith<RuntimeException> {
                requireCoreOk("""{"error":{"message":"Trash folder not found"}}""")
            }
        assertEquals("Trash folder not found", failure.message)
    }
}
