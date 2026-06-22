package jp.nonbili.meron.shared

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals

class MobileCommandsTest {
    @Test
    fun accountListUsesDesktopBridgeMethodName() {
        assertEquals(
            """{"id":3,"method":"account.list","params":{}}""",
            accountListRequest(id = 3).toJson(),
        )
    }

    @Test
    fun accountListClientUsesSharedCoreCommand() {
        val core = FakeMeronCore("""{"accounts":[]}""")
        val response = runSuspend {
            MobileMailCommandClient(core).listAccounts()
        }

        assertEquals(MobileCommand.AccountList, core.lastCommand)
        assertEquals("{}", core.lastPayloadJson)
        assertEquals("""{"accounts":[]}""", response)
    }

    @Test
    fun accountAddPasswordUsesDesktopBridgePayloadShape() {
        val request = accountAddPasswordRequest(
            id = 7,
            params = AddPasswordAccountParams(
                email = "me@example.com",
                displayName = "Me",
                senderName = "Sender",
                imapHost = "imap.example.com",
                imapPort = 993,
                smtpHost = "smtp.example.com",
                smtpPort = 587,
                username = "me@example.com",
                password = "secret",
                tls = true,
            ),
        )

        assertEquals(
            """{"id":7,"method":"account.addPassword","params":{"email":"me@example.com","display_name":"Me","sender_name":"Sender","imap_host":"imap.example.com","imap_port":993,"smtp_host":"smtp.example.com","smtp_port":587,"username":"me@example.com","password":"secret","tls":true}}""",
            request.toJson(),
        )
    }

    @Test
    fun accountAddPasswordClientUsesSharedCoreCommand() {
        val core = FakeMeronCore("""{"account":{"id":"me@example.com"}}""")
        val response = runSuspend {
            MobileMailCommandClient(core).addPasswordAccount(
                AddPasswordAccountParams(
                    email = "me@example.com",
                    imapHost = "imap.example.com",
                    smtpHost = "smtp.example.com",
                    username = "me@example.com",
                    password = "secret",
                ),
            )
        }

        assertEquals(MobileCommand.AccountAddPassword, core.lastCommand)
        assertEquals(
            """{"email":"me@example.com","display_name":"","sender_name":"","imap_host":"imap.example.com","imap_port":993,"smtp_host":"smtp.example.com","smtp_port":465,"username":"me@example.com","password":"secret","tls":true}""",
            core.lastPayloadJson,
        )
        assertEquals("""{"account":{"id":"me@example.com"}}""", response)
    }

    @Test
    fun accountAddOAuthUsesDesktopBridgePayloadShape() {
        val request = accountAddOAuthRequest(
            id = 8,
            params = AddOAuthAccountParams(
                email = "me@gmail.com",
                provider = "gmail",
                displayName = "Me",
                senderName = "Sender",
                username = "me@gmail.com",
                avatarUrl = "https://example.com/avatar.png",
                accessToken = "access",
                refreshToken = "refresh",
                tokenExpiresAt = 1_700_000_000,
            ),
        )

        assertEquals(
            """{"id":8,"method":"account.addOAuth","params":{"email":"me@gmail.com","provider":"gmail","display_name":"Me","sender_name":"Sender","username":"me@gmail.com","avatar_url":"https://example.com/avatar.png","access_token":"access","refresh_token":"refresh","token_expires_at":1700000000}}""",
            request.toJson(),
        )
    }

    @Test
    fun accountAddOAuthClientUsesSharedCoreCommand() {
        val core = FakeMeronCore("""{"account":{"id":"me@gmail.com"}}""")
        val response = runSuspend {
            MobileMailCommandClient(core).addOAuthAccount(
                AddOAuthAccountParams(
                    email = "me@gmail.com",
                    provider = "gmail",
                    refreshToken = "refresh",
                ),
            )
        }

        assertEquals(MobileCommand.AccountAddOAuth, core.lastCommand)
        assertEquals(
            """{"email":"me@gmail.com","provider":"gmail","display_name":"","sender_name":"","refresh_token":"refresh","token_expires_at":0}""",
            core.lastPayloadJson,
        )
        assertEquals("""{"account":{"id":"me@gmail.com"}}""", response)
    }

    @Test
    fun accountExchangeOAuthCodeUsesMobileCorePayloadShape() {
        val request = accountExchangeOAuthCodeRequest(
            id = 10,
            params = ExchangeOAuthCodeParams(
                email = "me@gmail.com",
                provider = "gmail",
                displayName = "Me",
                senderName = "Sender",
                code = "auth-code",
                clientId = "client",
                clientSecret = "secret",
                redirectUri = defaultOAuthRedirectUri(),
                codeVerifier = "verifier",
            ),
        )

        assertEquals(
            """{"id":10,"method":"account.exchangeOAuthCode","params":{"email":"me@gmail.com","provider":"gmail","display_name":"Me","sender_name":"Sender","code":"auth-code","client_id":"client","client_secret":"secret","redirect_uri":"jp.nonbili.meron.oauth://oauth","code_verifier":"verifier"}}""",
            request.toJson(),
        )
    }

    @Test
    fun accountExchangeOAuthCodeClientUsesSharedCoreCommand() {
        val core = FakeMeronCore("""{"account":{"id":"me@gmail.com"}}""")
        val response = runSuspend {
            MobileMailCommandClient(core).exchangeOAuthCode(
                ExchangeOAuthCodeParams(
                    email = "me@gmail.com",
                    provider = "gmail",
                    code = "auth-code",
                    clientId = "client",
                    redirectUri = defaultOAuthRedirectUri(),
                    codeVerifier = "verifier",
                ),
            )
        }

        assertEquals(MobileCommand.AccountExchangeOAuthCode, core.lastCommand)
        assertEquals(
            """{"email":"me@gmail.com","provider":"gmail","display_name":"","sender_name":"","code":"auth-code","client_id":"client","redirect_uri":"jp.nonbili.meron.oauth://oauth","code_verifier":"verifier"}""",
            core.lastPayloadJson,
        )
        assertEquals("""{"account":{"id":"me@gmail.com"}}""", response)
    }

    @Test
    fun accountAddRssUsesDesktopBridgePayloadShape() {
        val request = accountAddRssRequest(
            id = 9,
            params = AddRssAccountParams(
                feedUrl = "https://example.com/feed.xml",
                displayName = "News",
            ),
        )

        assertEquals(
            """{"id":9,"method":"account.addRss","params":{"feed_url":"https://example.com/feed.xml","display_name":"News"}}""",
            request.toJson(),
        )
    }

    @Test
    fun rssClientMethodsUseSharedCoreCommandNames() {
        val core = FakeMeronCore("""{"ok":true}""")
        val client = MobileMailCommandClient(core)

        runSuspend {
            client.addRssAccount(
                AddRssAccountParams(
                    feedUrl = "https://example.com/feed.xml",
                    displayName = "News",
                ),
            )
        }
        assertEquals(MobileCommand.AccountAddRss, core.lastCommand)
        assertEquals(
            """{"feed_url":"https://example.com/feed.xml","display_name":"News"}""",
            core.lastPayloadJson,
        )

        runSuspend {
            client.addRssFeed(
                AddRssFeedParams(
                    accountId = "rss-account",
                    feedUrl = "https://example.com/other.xml",
                ),
            )
        }
        assertEquals(MobileCommand.FeedAdd, core.lastCommand)
        assertEquals(
            """{"account":"rss-account","feed_url":"https://example.com/other.xml"}""",
            core.lastPayloadJson,
        )

        runSuspend {
            client.removeRssFeed(RemoveRssFeedParams(threadId = "rss-account#rss#feed-1"))
        }
        assertEquals(MobileCommand.FeedRemove, core.lastCommand)
        assertEquals("""{"thread_id":"rss-account#rss#feed-1"}""", core.lastPayloadJson)

        runSuspend {
            client.moveRssFeed(
                MoveRssFeedParams(
                    threadId = "rss-account#rss#feed-1",
                    targetAccountId = "other-rss-account",
                ),
            )
        }
        assertEquals(MobileCommand.FeedMove, core.lastCommand)
        assertEquals(
            """{"thread_id":"rss-account#rss#feed-1","target_account":"other-rss-account"}""",
            core.lastPayloadJson,
        )

        runSuspend {
            client.syncRss(SyncRssParams(accountId = "rss-account"))
        }
        assertEquals(MobileCommand.RssSync, core.lastCommand)
        assertEquals("""{"account_id":"rss-account"}""", core.lastPayloadJson)

        runSuspend {
            client.readRssThread(RssThreadParams(threadId = "rss-account#rss#feed-1"))
        }
        assertEquals(MobileCommand.RssThread, core.lastCommand)
        assertEquals("""{"thread_id":"rss-account#rss#feed-1"}""", core.lastPayloadJson)

        runSuspend {
            client.markRssRead(
                RssMarkReadParams(
                    threadId = "rss-account#rss#feed-1",
                    seen = false,
                    itemKeys = listOf("item-1"),
                ),
            )
        }
        assertEquals(MobileCommand.RssMarkRead, core.lastCommand)
        assertEquals(
            """{"thread_id":"rss-account#rss#feed-1","seen":false,"item_keys":["item-1"]}""",
            core.lastPayloadJson,
        )

        runSuspend {
            client.markRssStarred(
                RssMarkStarredParams(
                    threadId = "rss-account#rss#feed-1",
                    starred = true,
                    itemKeys = listOf("item-1"),
                ),
            )
        }
        assertEquals(MobileCommand.RssMarkStarred, core.lastCommand)
        assertEquals(
            """{"thread_id":"rss-account#rss#feed-1","starred":true,"item_keys":["item-1"]}""",
            core.lastPayloadJson,
        )
    }

    @Test
    fun clientMethodsUseSharedCoreCommandNames() {
        val core = FakeMeronCore("""{"ok":true}""")
        val client = MobileMailCommandClient(core)

        runSuspend { client.listFolders(FolderListParams(accountId = "acc1")) }
        assertEquals(MobileCommand.FolderList, core.lastCommand)
        assertEquals("""{"account_id":"acc1"}""", core.lastPayloadJson)

        runSuspend { client.listThreads(ThreadListParams(accountId = "acc1")) }
        assertEquals(MobileCommand.ThreadList, core.lastCommand)
        assertEquals("""{"account_id":"acc1","folder_id":"inbox","query":"","filter":"all","refresh":false}""", core.lastPayloadJson)

        runSuspend { client.listStarredItems() }
        assertEquals(MobileCommand.StarredItems, core.lastCommand)
        assertEquals("{}", core.lastPayloadJson)

        runSuspend { client.readThread(ThreadReadParams(threadId = "thread1")) }
        assertEquals(MobileCommand.ThreadRead, core.lastCommand)
        assertEquals("""{"thread_id":"thread1"}""", core.lastPayloadJson)

        runSuspend { client.sync(SyncMailParams(accountId = "acc1")) }
        assertEquals(MobileCommand.Sync, core.lastCommand)
        assertEquals("""{"account_id":"acc1","folder_id":"inbox","limit":50,"folders":true}""", core.lastPayloadJson)

        runSuspend {
            client.send(
                SendMailParams(
                    accountId = "acc1",
                    to = "you@example.com",
                    subject = "Hi",
                    body = "Hello",
                ),
            )
        }
        assertEquals(MobileCommand.Send, core.lastCommand)

        runSuspend { client.archive(ThreadActionParams(threadId = "thread1")) }
        assertEquals(MobileCommand.Archive, core.lastCommand)
        assertEquals("""{"thread_id":"thread1"}""", core.lastPayloadJson)

        runSuspend { client.delete(ThreadActionParams(threadId = "thread1")) }
        assertEquals(MobileCommand.Delete, core.lastCommand)
        assertEquals("""{"thread_id":"thread1"}""", core.lastPayloadJson)

        runSuspend { client.markRead(MarkReadParams(threadId = "thread1", seen = false)) }
        assertEquals(MobileCommand.MarkRead, core.lastCommand)
        assertEquals("""{"thread_id":"thread1","seen":false}""", core.lastPayloadJson)

        runSuspend { client.markStarred(MarkStarredParams(threadId = "thread1", starred = true)) }
        assertEquals(MobileCommand.MarkStarred, core.lastCommand)
        assertEquals("""{"thread_id":"thread1","starred":true}""", core.lastPayloadJson)
    }

    @Test
    fun threadListPreservesFrontendWireFieldNames() {
        val request = threadListRequest(
            id = 4,
            params = ThreadListParams(
                accountId = "acc1",
                folderId = "inbox",
                query = "design",
                filter = "unread",
                beforeCursor = "1700000000:44",
                refresh = true,
            ),
        )

        assertEquals(
            """{"id":4,"method":"mail.threadList","params":{"account_id":"acc1","folder_id":"inbox","query":"design","filter":"unread","before_cursor":"1700000000:44","refresh":true}}""",
            request.toJson(),
        )
    }

    @Test
    fun threadReadOmitsOptionalPaginationWhenUnset() {
        assertEquals(
            """{"id":5,"method":"mail.threadRead","params":{"thread_id":"acc#imap#inbox#thread"}}""",
            threadReadRequest(id = 5, params = ThreadReadParams(threadId = "acc#imap#inbox#thread")).toJson(),
        )
    }

    @Test
    fun syncMailMatchesMobileCorePayloadShape() {
        val request = syncMailRequest(
            id = 8,
            params = SyncMailParams(
                accountId = "acc1",
                folderId = "INBOX",
                limit = 25,
                folders = false,
            ),
        )

        assertEquals(
            """{"id":8,"method":"mail.sync","params":{"account_id":"acc1","folder_id":"INBOX","limit":25,"folders":false}}""",
            request.toJson(),
        )
    }

    @Test
    fun rssCommandsMatchMobileCorePayloadShapes() {
        assertEquals(
            """{"id":31,"method":"feed.remove","params":{"thread_id":"rss-account#rss#feed-1"}}""",
            feedRemoveRequest(
                id = 31,
                params = RemoveRssFeedParams(threadId = "rss-account#rss#feed-1"),
            ).toJson(),
        )
        assertEquals(
            """{"id":32,"method":"feed.move","params":{"thread_id":"rss-account#rss#feed-1","target_account":"target-rss"}}""",
            feedMoveRequest(
                id = 32,
                params = MoveRssFeedParams(
                    threadId = "rss-account#rss#feed-1",
                    targetAccountId = "target-rss",
                ),
            ).toJson(),
        )
        assertEquals(
            """{"id":33,"method":"rss.sync","params":{"account_id":"rss-account"}}""",
            syncRssRequest(id = 33, params = SyncRssParams(accountId = "rss-account")).toJson(),
        )
        assertEquals(
            """{"id":34,"method":"rss.thread","params":{"thread_id":"rss-account#rss#feed-1","before_cursor":"1700000000:abc","limit":25}}""",
            rssThreadRequest(
                id = 34,
                params = RssThreadParams(
                    threadId = "rss-account#rss#feed-1",
                    beforeCursor = "1700000000:abc",
                    limit = 25,
                ),
            ).toJson(),
        )
        assertEquals(
            """{"id":35,"method":"rss.markRead","params":{"thread_id":"rss-account#rss#feed-1","seen":false,"item_keys":["item-1","item-2"]}}""",
            rssMarkReadRequest(
                id = 35,
                params = RssMarkReadParams(
                    threadId = "rss-account#rss#feed-1",
                    seen = false,
                    itemKeys = listOf("item-1", "item-2"),
                ),
            ).toJson(),
        )
        assertEquals(
            """{"id":36,"method":"rss.markStarred","params":{"thread_id":"rss-account#rss#feed-1","starred":true,"item_keys":["item-1"]}}""",
            rssMarkStarredRequest(
                id = 36,
                params = RssMarkStarredParams(
                    threadId = "rss-account#rss#feed-1",
                    starred = true,
                    itemKeys = listOf("item-1"),
                ),
            ).toJson(),
        )
    }

    @Test
    fun sendMailMatchesDesktopComposerPayloadShape() {
        val request = sendMailRequest(
            id = 6,
            params = SendMailParams(
                accountId = "acc1",
                from = "me@example.com",
                to = "you@example.com",
                cc = "cc@example.com",
                subject = "Hi",
                body = "Plain",
                html = "<p>Plain</p>",
                inReplyTo = "<old@example.com>",
                references = "<root@example.com>",
                attachments = listOf(
                    MobileAttachmentInput(
                        filename = "note.txt",
                        mime = "text/plain",
                        data = "SGk=",
                    ),
                ),
            ),
        )

        assertEquals(
            """{"id":6,"method":"mail.send","params":{"account_id":"acc1","from":"me@example.com","to":"you@example.com","cc":"cc@example.com","bcc":"","reply_to":"","subject":"Hi","body":"Plain","html":"<p>Plain</p>","in_reply_to":"<old@example.com>","references":"<root@example.com>","message_id":"","attachments":[{"filename":"note.txt","mime":"text/plain","data":"SGk=","inline_id":""}]}}""",
            request.toJson(),
        )
    }

    @Test
    fun replyMailParamsUseReplyRecipientAndThreadingHeaders() {
        val request = sendMailRequest(
            id = 7,
            params = MessageBody(
                id = "m1",
                from = "Ada",
                to = "Me <me@example.com>",
                subject = "Design",
                body = "Original",
                fromAddr = "ada@example.com",
                replyTo = "Team <team@example.com>",
                messageId = "old@example.com",
                references = "<root@example.com>",
            ).toReplyMailParams(accountId = "acc1", body = "Quick reply"),
        )

        assertEquals(
            """{"id":7,"method":"mail.send","params":{"account_id":"acc1","from":"","to":"Team <team@example.com>","cc":"","bcc":"","reply_to":"","subject":"Re: Design","body":"Quick reply","html":"","in_reply_to":"<old@example.com>","references":"<root@example.com> <old@example.com>","message_id":"","attachments":[]}}""",
            request.toJson(),
        )
    }

    @Test
    fun threadActionsUseCurrentBridgePayloads() {
        assertEquals(
            """{"thread_id":"t1"}""",
            ThreadActionParams(threadId = "t1").archiveJson(),
        )
        assertEquals(
            """{"thread_id":"t1","folder":"Trash","message_ids":["m1","m2"]}""",
            ThreadActionParams(threadId = "t1", folderId = "Trash", messageIds = listOf("m1", "m2")).deleteJson(),
        )
        assertEquals(
            """{"thread_id":"t1","seen":false,"message_ids":["m1"]}""",
            MarkReadParams(threadId = "t1", seen = false, messageIds = listOf("m1")).toJson(),
        )
        assertEquals(
            """{"thread_id":"t1","starred":true,"message_ids":["m1"]}""",
            MarkStarredParams(threadId = "t1", starred = true, messageIds = listOf("m1")).toJson(),
        )
        assertEquals(
            """{"id":20,"method":"mail.archive","params":{"thread_id":"t1"}}""",
            archiveThreadRequest(id = 20, ThreadActionParams(threadId = "t1")).toJson(),
        )
        assertEquals(
            """{"id":21,"method":"mail.delete","params":{"thread_id":"t1","folder":"Trash"}}""",
            deleteThreadRequest(id = 21, ThreadActionParams(threadId = "t1", folderId = "Trash")).toJson(),
        )
        assertEquals(
            """{"id":22,"method":"mail.markRead","params":{"thread_id":"t1","seen":true}}""",
            markReadRequest(id = 22, MarkReadParams(threadId = "t1", seen = true)).toJson(),
        )
        assertEquals(
            """{"id":23,"method":"mail.markStarred","params":{"thread_id":"t1","starred":false}}""",
            markStarredRequest(id = 23, MarkStarredParams(threadId = "t1", starred = false)).toJson(),
        )
    }

    @Test
    fun jsonBuilderEscapesPayloadStrings() {
        assertEquals(
            """{"account_id":"a\"b","folder_id":"line\nbreak","query":"","filter":"all","refresh":false}""",
            ThreadListParams(accountId = "a\"b", folderId = "line\nbreak").toJson(),
        )
    }

    private class FakeMeronCore(
        private val response: String,
    ) : MeronCore {
        var lastCommand = ""
        var lastPayloadJson = ""

        override suspend fun invoke(command: String, payloadJson: String): String {
            lastCommand = command
            lastPayloadJson = payloadJson
            return response
        }

        override fun events(): CoreEventStream {
            return object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle {
                    return CloseableHandle {}
                }
            }
        }

        override suspend fun protocolVersion(): Int = EXPECTED_PROTOCOL_VERSION
    }
}

private fun <T> runSuspend(block: suspend () -> T): T {
    var value: T? = null
    var error: Throwable? = null
    block.startCoroutine(
        Continuation(EmptyCoroutineContext) {
            it.onSuccess { output -> value = output }
            it.onFailure { thrown -> error = thrown }
        },
    )
    error?.let { throw it }
    @Suppress("UNCHECKED_CAST")
    return value as T
}
