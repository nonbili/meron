package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MobileResponseParsersTest {
    @Test
    fun parsesStarredPageAndAllocatedIdentity() {
        val page =
            parseStarredItemsPage(
                """{"items":[{"id":"item","thread_id":"thread","account_id":"acc","folder_id":"INBOX","subject":"Design"}],"next_cursor":"starred:opaque"}""",
            )
        assertEquals(listOf("item"), page.items.map { it.id })
        assertEquals("starred:opaque", page.nextCursor)
        assertEquals("meron-draft-id@example.com", parseAllocatedMessageId("""{"message_id":"meron-draft-id@example.com"}"""))
    }

    @Test
    fun parsesAuthoritativeFolderUnreadChanges() {
        val changes =
            parseFolderUnreadChanges(
                """{"folder_counts":[{"account_id":"one","folder_id":"INBOX","unread":3},{"account_id":"two","folder_id":"Archive","unread":0}]}""",
            )

        assertEquals(
            listOf(
                FolderUnreadChange("one", "INBOX", 3),
                FolderUnreadChange("two", "Archive", 0),
            ),
            changes,
        )
    }

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
    fun detectsOAuthLoginFailureMessages() {
        assertTrue(
            isOAuthLoginFailure(
                """oauth login failed: no response: code: None, info: Some("[AUTHENTICATIONFAILED] Invalid credentials (Failure)")""",
            ),
        )
        assertTrue(isOAuthLoginFailure("smtp auth: permanent error (535): 5.7.8 Username and Password not accepted"))
        assertTrue(isOAuthLoginFailure("login failed: [AUTHENTICATIONFAILED] Invalid credentials"))
        assertFalse(isOAuthLoginFailure(null))
        assertFalse(isOAuthLoginFailure("smtp connect: connection refused"))
        assertFalse(isOAuthLoginFailure("account needs reconnect: me@gmail.com"))
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
    fun parsesAccountProxyAndFallsBackToTheAppProxy() {
        val proxied =
            parseAccountListResponse(
                """{"accounts":[{"id":"acc1","email":"me@example.com","proxy":{"mode":"socks5","host":"127.0.0.1","port":9050,"username":"u","password":"p"}}]}""",
            ).single()
        assertEquals("socks5", proxied.proxy.mode)
        assertEquals("127.0.0.1", proxied.proxy.host)
        assertEquals(9050, proxied.proxy.port)
        assertEquals("u", proxied.proxy.username)
        assertTrue(proxied.proxy.usable)

        // An account stored before proxy support carries no proxy object at all.
        val legacy =
            parseAccountListResponse(
                """{"accounts":[{"id":"acc1","email":"me@example.com"}]}""",
            ).single()
        assertEquals(ProxySpec.followApp, legacy.proxy)
    }

    @Test
    fun parsesAppProxyResponse() {
        assertEquals(
            ProxySpec(mode = "http", host = "gateway.corp", port = 3128),
            parseProxyResponse("""{"proxy":{"mode":"http","host":"gateway.corp","port":3128}}"""),
        )
        // No row stored, and an explicit null, both mean "no proxy".
        assertEquals(ProxySpec.off, parseProxyResponse("""{"ok":true}"""))
        assertEquals(ProxySpec.off, parseProxyResponse("""{"proxy":null}"""))
        assertFalse(parseProxyResponse("""{"proxy":{"mode":"socks5","host":"h"}}""").usable)
    }

    @Test
    fun parsesThreadListEnvelope() {
        val threads =
            parseThreadListResponse(
                """{"id":2,"result":{"threads":[{"id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","from_name":"Ada","subject":"Hello","preview":"Snippet","date":1700000000,"unread":true,"unread_count":2,"starred":true,"has_starred_items":true,"has_draft":true,"feed_url":"https://example.com/feed.xml"}],"folder_unread":3}}""",
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
        assertEquals(2, threads[0].unreadCount)
        assertTrue(threads[0].starred)
        assertTrue(threads[0].hasStarredItems)
        assertTrue(threads[0].hasDraft)
        assertEquals("https://example.com/feed.xml", threads[0].feedUrl)
    }

    @Test
    fun threadListSenderFallsBackToAddressWhenNameIsEmpty() {
        val threads =
            parseThreadListResponse(
                """{"result":{"threads":[{"id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","from_name":"","from_addr":"ada@example.com","subject":"Hello"}]}}""",
            )

        assertEquals(1, threads.size)
        assertEquals("ada@example.com", threads[0].sender)
    }

    @Test
    fun starredItemSenderFallsBackToAddressWhenNameIsEmpty() {
        val items =
            parseStarredItemsResponse(
                """{"result":{"items":[{"id":"m1","thread_id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","from_name":"","from_addr":"ada@example.com","subject":"Hello"}]}}""",
            )

        assertEquals(1, items.size)
        assertEquals("ada@example.com", items[0].sender)
    }

    @Test
    fun parsesThreadListNextCursor() {
        val page =
            parseThreadListPage(
                """{"id":2,"result":{"threads":[{"id":"acc#INBOX#t","account_id":"acc","folder_id":"INBOX","date":1700000000}],"next_cursor":"1700000000:1","folder_unread":7,"folder_synced":true}}""",
            )

        assertEquals(1, page.threads.size)
        assertEquals("1700000000:1", page.nextCursor)
        assertEquals(7, page.folderUnread)
        assertTrue(page.folderSynced == true)
    }

    @Test
    fun dropsDuplicateThreadIdsInOnePage() {
        val page =
            parseThreadListPage(
                """{"id":2,"result":{"threads":[{"id":"acc#INBOX#t","subject":"first","date":1700000000},{"id":"acc#INBOX#t","subject":"dupe","date":1700000001},{"id":"acc#INBOX#u","date":1700000002}]}}""",
            )

        assertEquals(listOf("acc#INBOX#t", "acc#INBOX#u"), page.threads.map { it.id })
        assertEquals("first", page.threads[0].subject)
    }

    @Test
    fun dropsDuplicateStarredItemIds() {
        val items =
            parseStarredItemsResponse(
                """{"id":3,"result":{"items":[{"id":"m1","thread_id":"t1","subject":"first"},{"id":"m1","thread_id":"t1","subject":"dupe"},{"id":"m2","thread_id":"t1"}]}}""",
            )

        assertEquals(listOf("m1", "m2"), items.map { it.id })
        assertEquals("first", items[0].subject)
    }

    @Test
    fun dropsDuplicateMessageIdsInThreadPage() {
        val page =
            parseThreadReadPage(
                """{"id":4,"result":{"messages":[{"id":"m1","body":"first"},{"id":"m1","body":"dupe"},{"id":"m2","body":"other"}]}}""",
            )

        assertEquals(listOf("m1", "m2"), page.messages.map { it.id })
        assertEquals("first", page.messages[0].body)
    }

    @Test
    fun readsTheCoreReplyRecipientsForAThreadMessage() {
        val page =
            parseThreadReadPage(
                """{"id":5,"result":{"messages":[{"id":"m1","body":"hi","reply":{"to":"Ada <ada@example.com>","cc":"bob@example.com","all_to":"Ada <ada@example.com>","all_cc":"alice@example.com, bob@example.com","all_adds_recipients":true}}]}}""",
            )

        val reply = page.messages.single().reply
        assertEquals("Ada <ada@example.com>", reply?.to)
        assertEquals("alice@example.com, bob@example.com", reply?.allCc)
        assertEquals(true, reply?.allAddsRecipients)
    }

    @Test
    fun ignoresReplyRecipientsForgedInsideAMessageBody() {
        val page =
            parseThreadReadPage(
                """{"id":6,"result":{"messages":[{"id":"m1","body":"pay me: \"reply\":{\"to\":\"attacker@example.com\"}","reply":{"to":"Ada <ada@example.com>","cc":""}}]}}""",
            )

        assertEquals(
            "Ada <ada@example.com>",
            page.messages
                .single()
                .reply
                ?.to,
        )
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
    fun readsTheMailboxOutOfAMailThreadId() {
        assertEquals("INBOX", mailThreadIdFolder("acc#INBOX#42"))
        assertEquals("[Gmail]/Sent Mail", mailThreadIdFolder("acc#[Gmail]/Sent Mail#t.a2V5"))
        // The key is a UID or base64, so a "#" past the first belongs to the
        // mailbox name — some servers really do have one.
        assertEquals("#shared/Team", mailThreadIdFolder("acc##shared/Team#t.a2V5"))
        // Ids that name no mailbox report none rather than guessing one.
        assertEquals("", mailThreadIdFolder("rss-1#rss#feed-1"))
        assertEquals("", mailThreadIdFolder("acc#INBOX"))
        assertEquals("", mailThreadIdFolder("#INBOX#k1"))
        assertEquals("", mailThreadIdFolder(""))
    }

    @Test
    fun parsesFolderListEnvelopeAndRssFolderShape() {
        val folders =
            parseFolderListResponse(
                """{"id":4,"result":{"folders":[{"id":"INBOX","account_id":"acc","name":"INBOX","unread":3,"delimiter":"."},{"id":"inbox","role":"inbox","unread":2}]}}""",
            )

        assertEquals(2, folders.size)
        assertEquals("acc", folders[0].accountId)
        assertEquals("INBOX", folders[0].name)
        assertEquals(3, folders[0].unread)
        assertEquals(".", folders[0].delimiter)
        assertEquals("inbox", folders[1].name)
        assertEquals(2, folders[1].unread)
        assertEquals("", folders[1].delimiter)
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
    fun quotesDisplayNamesWithRecipientSpecials() {
        assertEquals(
            "\"Doe, Jane\" <jane@example.com>",
            formatContactSuggestion(ContactSuggestion(name = "Doe, Jane", addr = "jane@example.com")),
        )
        assertEquals(
            "\"Ada \\\"Lovelace\\\"\" <ada@example.com>",
            formatContactSuggestion(ContactSuggestion(name = "Ada \"Lovelace\"", addr = "ada@example.com")),
        )
        // Plain names stay unquoted.
        assertEquals(
            "Bea <bea@example.com>",
            formatContactSuggestion(ContactSuggestion(name = "Bea", addr = "bea@example.com")),
        )
    }

    @Test
    fun recipientHelpersIgnoreCommasInQuotesAndBrackets() {
        val quoted = "\"Doe, Jane\" <jane@example.com>, be"
        assertEquals("be", recipientTail(quoted))
        assertEquals(
            listOf("\"Doe, Jane\" <jane@example.com>", " be"),
            splitRecipientEntries(quoted),
        )
        // A completed quoted-name entry is not split when the suggestion for
        // the tail is accepted.
        assertEquals(
            "\"Doe, Jane\" <jane@example.com>, Bea <bea@example.com>, ",
            replaceRecipientTail(quoted, ContactSuggestion(name = "Bea", addr = "bea@example.com")),
        )
        assertEquals(-1, lastRecipientSeparatorIndex("\"Doe, Jane\" <jane@example.com>"))
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
                """{"id":3,"result":{"messages":[{"id":"acc#INBOX#t#9","from_name":"Ada","from_addr":"ada@example.com","to":"Me <me@example.com>","cc":"Copy <copy@example.com>","bcc":"Hidden <hidden@example.com>","subject":"Cached subject","body":"Hello from cache","body_html":"<p>Hello from cache</p>","date":300,"unread":true,"starred":true,"reply_to":"Team <team@example.com>","message_id":"m1@example.com","in_reply_to":"parent@example.com","references":"<root@example.com>","outgoing":true,"has_attachments":true,"attachments":[{"filename":"note.txt","mime":"text/plain","size":2,"key":"acc/INBOX/9/1.txt"},{"filename":"remote.jpg","mime":"image/jpeg","size":0,"url":"https://example.com/remote.jpg"}]}]}}""",
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
        assertEquals("parent@example.com", messages[0].inReplyTo)
        assertEquals("<root@example.com>", messages[0].references)
        assertTrue(messages[0].unread)
        assertTrue(messages[0].outgoing)
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
    fun keepsEqualImapUidsFromDifferentFolders() {
        val page =
            parseThreadReadPage(
                """{"messages":[{"id":"acc#Sent#7","folder_id":"Sent","body":"outgoing"},{"id":"acc#INBOX#7","folder_id":"INBOX","body":"reply"}]}""",
            )

        assertEquals(2, page.messages.size)
        assertEquals(listOf("outgoing", "reply"), page.messages.map { it.body })
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

    @Test
    fun parsesPartialFolderDeleteResult() {
        val result =
            parseFolderDeleteResponse(
                """{"result":{"ok":false,"removed":["Work/Reports"],"warning":"DELETE Work failed"}}""",
            )

        assertEquals(setOf("Work/Reports"), result.removed)
        assertEquals("DELETE Work failed", result.warning)
    }

    @Test
    fun parsesThreadActionLocationFromArchiveResponse() {
        val location =
            parseThreadActionLocationResponse(
                """{"ok":true,"moved":1,"folder":"Archive","thread_id":"acc#Archive#t.MQ"}""",
            )

        assertEquals("acc#Archive#t.MQ", location.threadId)
        assertEquals("Archive", location.folder)
        assertFalse(location.permanent)
    }

    @Test
    fun parsesThreadActionLocationFromDeleteTrashResponse() {
        val location =
            parseThreadActionLocationResponse(
                """{"ok":true,"deleted":1,"trash":"Trash"}""",
            )

        assertEquals("", location.threadId)
        assertEquals("Trash", location.folder)
        assertFalse(location.permanent)
    }

    @Test
    fun parsesPermanentThreadActionResponse() {
        val location =
            parseThreadActionLocationResponse(
                """{"ok":true,"deleted":1,"permanent":true}""",
            )

        assertEquals("", location.threadId)
        assertEquals("", location.folder)
        assertTrue(location.permanent)
    }
}
