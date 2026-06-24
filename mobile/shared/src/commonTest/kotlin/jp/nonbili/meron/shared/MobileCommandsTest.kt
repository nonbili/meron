package jp.nonbili.meron.shared

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

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
        val response =
            runSuspend {
                MobileMailCommandClient(core).listAccounts()
            }

        assertEquals(MobileCommand.AccountList, core.lastCommand)
        assertEquals("{}", core.lastPayloadJson)
        assertEquals("""{"accounts":[]}""", response)
    }

    @Test
    fun accountAddPasswordUsesDesktopBridgePayloadShape() {
        val request =
            accountAddPasswordRequest(
                id = 7,
                params =
                    AddPasswordAccountParams(
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
        val response =
            runSuspend {
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
    fun accountAutodiscoverClientUsesSharedCoreCommand() {
        val core = FakeMeronCore("""{"imap_host":"imap.example.com"}""")
        val response =
            runSuspend {
                MobileMailCommandClient(core).autodiscoverAccount(AutodiscoverAccountParams("me@example.com"))
            }

        assertEquals(MobileCommand.AccountAutodiscover, core.lastCommand)
        assertEquals("""{"email":"me@example.com"}""", core.lastPayloadJson)
        assertEquals("""{"imap_host":"imap.example.com"}""", response)
    }

    @Test
    fun accountAddOAuthUsesDesktopBridgePayloadShape() {
        val request =
            accountAddOAuthRequest(
                id = 8,
                params =
                    AddOAuthAccountParams(
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
        val response =
            runSuspend {
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
        val request =
            accountExchangeOAuthCodeRequest(
                id = 10,
                params =
                    ExchangeOAuthCodeParams(
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
        val response =
            runSuspend {
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
        val request =
            accountAddRssRequest(
                id = 9,
                params =
                    AddRssAccountParams(
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
    fun accountAutodiscoverUsesDesktopBridgePayloadShape() {
        assertEquals(
            """{"id":10,"method":"account.autodiscover","params":{"email":"me@example.com"}}""",
            accountAutodiscoverRequest(id = 10, params = AutodiscoverAccountParams("me@example.com")).toJson(),
        )
    }

    @Test
    fun accountSettingCommandsUseDesktopBridgePayloadShapes() {
        assertEquals(
            """{"id":11,"method":"account.remove","params":{"id":"acc1"}}""",
            accountRemoveRequest(id = 11, params = AccountIdParams(accountId = "acc1")).toJson(),
        )
        assertEquals(
            """{"id":12,"method":"account.setName","params":{"id":"acc1","name":"Personal"}}""",
            accountSetNameRequest(id = 12, params = AccountNameParams(accountId = "acc1", name = "Personal")).toJson(),
        )
        assertEquals(
            """{"id":13,"method":"account.setSenderName","params":{"id":"acc1","name":"Sender"}}""",
            accountSetSenderNameRequest(id = 13, params = AccountNameParams(accountId = "acc1", name = "Sender")).toJson(),
        )
        assertEquals(
            """{"id":14,"method":"account.setAvatar","params":{"id":"acc1","avatar_url":"https://example.com/avatar.png"}}""",
            accountSetAvatarRequest(
                id = 14,
                params = AccountAvatarParams(accountId = "acc1", avatarUrl = "https://example.com/avatar.png"),
            ).toJson(),
        )
        assertEquals(
            """{"id":15,"method":"account.writeAvatarFile","params":{"id":"acc1","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}}""",
            accountWriteAvatarFileRequest(
                id = 15,
                params = AccountMediaFileParams(accountId = "acc1", filename = "avatar.png", mime = "image/png", data = "aGVsbG8="),
            ).toJson(),
        )
        assertEquals(
            """{"id":16,"method":"account.setChatWallpaper","params":{"id":"acc1","wallpaper":{"kind":"preset","presetId":"grid"}}}""",
            accountSetChatWallpaperRequest(id = 16, params = AccountChatWallpaperParams(accountId = "acc1", presetId = "grid")).toJson(),
        )
        assertEquals(
            """{"id":17,"method":"account.writeChatWallpaperFile","params":{"id":"acc1","filename":"wallpaper.png","mime":"image/png","data":"aGVsbG8="}}""",
            accountWriteChatWallpaperFileRequest(
                id = 17,
                params = AccountMediaFileParams(accountId = "acc1", filename = "wallpaper.png", mime = "image/png", data = "aGVsbG8="),
            ).toJson(),
        )
        assertEquals(
            """{"id":18,"method":"account.setChatWallpaper","params":{"id":"acc1","wallpaper":null}}""",
            accountSetChatWallpaperRequest(id = 18, params = AccountChatWallpaperParams(accountId = "acc1")).toJson(),
        )
        assertEquals(
            """{"id":19,"method":"account.setImages","params":{"id":"acc1","enabled":true}}""",
            accountSetImagesRequest(id = 19, params = AccountFlagParams(accountId = "acc1", enabled = true)).toJson(),
        )
        assertEquals(
            """{"id":20,"method":"account.setConversationHtml","params":{"id":"acc1","enabled":false}}""",
            accountSetConversationHtmlRequest(id = 20, params = AccountFlagParams(accountId = "acc1", enabled = false)).toJson(),
        )
        assertEquals(
            """{"id":21,"method":"account.setUnified","params":{"id":"acc1","enabled":false}}""",
            accountSetUnifiedRequest(id = 21, params = AccountFlagParams(accountId = "acc1", enabled = false)).toJson(),
        )
        assertEquals(
            """{"id":22,"method":"account.setMuted","params":{"id":"acc1","enabled":true}}""",
            accountSetMutedRequest(id = 22, params = AccountFlagParams(accountId = "acc1", enabled = true)).toJson(),
        )
        assertEquals(
            """{"id":23,"method":"account.setPaused","params":{"id":"acc1","enabled":true}}""",
            accountSetPausedRequest(id = 23, params = AccountFlagParams(accountId = "acc1", enabled = true)).toJson(),
        )
        assertEquals(
            """{"id":24,"method":"account.setRSSSyncInterval","params":{"id":"rss1","minutes":30}}""",
            accountSetRssSyncIntervalRequest(id = 24, params = AccountRssSyncIntervalParams(accountId = "rss1", minutes = 30)).toJson(),
        )
        assertEquals(
            """{"id":25,"method":"account.setAliases","params":{"id":"acc1","aliases":[{"email":"alias@example.com","name":"Alias"}]}}""",
            accountSetAliasesRequest(
                id = 25,
                params =
                    AccountAliasesParams(
                        accountId = "acc1",
                        aliases = listOf(AccountAliasParams(email = "alias@example.com", name = "Alias")),
                    ),
            ).toJson(),
        )
        assertEquals(
            """{"id":26,"method":"account.reorder","params":{"accounts":["acc2","acc1"]}}""",
            accountReorderRequest(id = 26, params = AccountReorderParams(listOf("acc2", "acc1"))).toJson(),
        )
        assertEquals(
            """{"id":27,"method":"mail.suggestContacts","params":{"account":"acc1","query":"bea","limit":8}}""",
            contactSuggestRequest(id = 27, params = ContactSuggestParams(accountId = "acc1", query = "bea")).toJson(),
        )
    }

    @Test
    fun accountSettingClientMethodsUseSharedCoreCommandNames() {
        val core = FakeMeronCore("""{"ok":true}""")
        val client = MobileMailCommandClient(core)

        runSuspend { client.setAccountName(AccountNameParams(accountId = "acc1", name = "Personal")) }
        assertEquals(MobileCommand.AccountSetName, core.lastCommand)
        assertEquals("""{"id":"acc1","name":"Personal"}""", core.lastPayloadJson)

        runSuspend { client.setAccountAvatar(AccountAvatarParams(accountId = "acc1", avatarUrl = "https://example.com/avatar.png")) }
        assertEquals(MobileCommand.AccountSetAvatar, core.lastCommand)
        assertEquals("""{"id":"acc1","avatar_url":"https://example.com/avatar.png"}""", core.lastPayloadJson)

        runSuspend {
            client.writeAccountAvatarFile(
                AccountMediaFileParams(accountId = "acc1", filename = "avatar.png", mime = "image/png", data = "aGVsbG8="),
            )
        }
        assertEquals(MobileCommand.AccountWriteAvatarFile, core.lastCommand)
        assertEquals("""{"id":"acc1","filename":"avatar.png","mime":"image/png","data":"aGVsbG8="}""", core.lastPayloadJson)

        runSuspend { client.setAccountChatWallpaper(AccountChatWallpaperParams(accountId = "acc1", presetId = "grid")) }
        assertEquals(MobileCommand.AccountSetChatWallpaper, core.lastCommand)
        assertEquals("""{"id":"acc1","wallpaper":{"kind":"preset","presetId":"grid"}}""", core.lastPayloadJson)

        runSuspend {
            client.writeAccountChatWallpaperFile(
                AccountMediaFileParams(accountId = "acc1", filename = "wallpaper.png", mime = "image/png", data = "aGVsbG8="),
            )
        }
        assertEquals(MobileCommand.AccountWriteChatWallpaperFile, core.lastCommand)
        assertEquals("""{"id":"acc1","filename":"wallpaper.png","mime":"image/png","data":"aGVsbG8="}""", core.lastPayloadJson)

        runSuspend { client.setAccountUnified(AccountFlagParams(accountId = "acc1", enabled = false)) }
        assertEquals(MobileCommand.AccountSetUnified, core.lastCommand)
        assertEquals("""{"id":"acc1","enabled":false}""", core.lastPayloadJson)

        runSuspend { client.setAccountAliases(AccountAliasesParams("acc1", listOf(AccountAliasParams("alias@example.com", "Alias")))) }
        assertEquals(MobileCommand.AccountSetAliases, core.lastCommand)
        assertEquals("""{"id":"acc1","aliases":[{"email":"alias@example.com","name":"Alias"}]}""", core.lastPayloadJson)

        runSuspend { client.reorderAccounts(AccountReorderParams(listOf("acc2", "acc1"))) }
        assertEquals(MobileCommand.AccountReorder, core.lastCommand)
        assertEquals("""{"accounts":["acc2","acc1"]}""", core.lastPayloadJson)

        runSuspend { client.suggestContacts(ContactSuggestParams(accountId = "acc1", query = "bea", limit = 4)) }
        assertEquals(MobileCommand.ContactSuggest, core.lastCommand)
        assertEquals("""{"account":"acc1","query":"bea","limit":4}""", core.lastPayloadJson)
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
            client.exportOpml(ExportOpmlParams(accountId = "rss-account"))
        }
        assertEquals(MobileCommand.FeedExportOpml, core.lastCommand)
        assertEquals("""{"account":"rss-account"}""", core.lastPayloadJson)

        runSuspend {
            client.importOpml(ImportOpmlParams(accountId = "rss-account", opml = "<opml version=\"2.0\"/>"))
        }
        assertEquals(MobileCommand.FeedImportOpml, core.lastCommand)
        assertEquals("""{"account":"rss-account","opml":"<opml version=\"2.0\"/>"}""", core.lastPayloadJson)

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

        runSuspend { client.createFolder(FolderCreateParams(accountId = "acc1", name = "Work")) }
        assertEquals(MobileCommand.FolderCreate, core.lastCommand)
        assertEquals("""{"account_id":"acc1","name":"Work"}""", core.lastPayloadJson)

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

        runSuspend { client.move(MoveThreadParams(threadId = "thread1", targetFolderId = "Work")) }
        assertEquals(MobileCommand.Move, core.lastCommand)
        assertEquals("""{"thread_id":"thread1","target_folder_id":"Work"}""", core.lastPayloadJson)

        runSuspend { client.copy(CopyThreadParams(threadId = "thread1", targetAccountId = "acc2", targetFolderId = "Archive")) }
        assertEquals(MobileCommand.Copy, core.lastCommand)
        assertEquals("""{"thread_id":"thread1","target_account_id":"acc2","target_folder_id":"Archive"}""", core.lastPayloadJson)

        runSuspend { client.markRead(MarkReadParams(threadId = "thread1", seen = false)) }
        assertEquals(MobileCommand.MarkRead, core.lastCommand)
        assertEquals("""{"thread_id":"thread1","seen":false}""", core.lastPayloadJson)

        runSuspend { client.markAllRead(MarkAllReadParams(accountId = "acc1", folderId = "INBOX")) }
        assertEquals(MobileCommand.MarkAllRead, core.lastCommand)
        assertEquals("""{"account_id":"acc1","folder_id":"INBOX"}""", core.lastPayloadJson)

        runSuspend { client.readAttachment(AttachmentReadParams(key = "acc/INBOX/1/note.txt")) }
        assertEquals(MobileCommand.AttachmentRead, core.lastCommand)
        assertEquals("""{"key":"acc/INBOX/1/note.txt"}""", core.lastPayloadJson)

        runSuspend { client.markStarred(MarkStarredParams(threadId = "thread1", starred = true)) }
        assertEquals(MobileCommand.MarkStarred, core.lastCommand)
        assertEquals("""{"thread_id":"thread1","starred":true}""", core.lastPayloadJson)
    }

    @Test
    fun threadListPreservesFrontendWireFieldNames() {
        val request =
            threadListRequest(
                id = 4,
                params =
                    ThreadListParams(
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
    fun folderCreatePreservesFrontendWireFieldNames() {
        assertEquals(
            """{"id":6,"method":"mail.folderCreate","params":{"account_id":"acc1","name":"Work"}}""",
            folderCreateRequest(id = 6, params = FolderCreateParams(accountId = "acc1", name = "Work")).toJson(),
        )
    }

    @Test
    fun moveThreadPreservesFrontendWireFieldNames() {
        assertEquals(
            """{"id":7,"method":"mail.move","params":{"thread_id":"acc#imap#inbox#thread","target_folder_id":"Work"}}""",
            moveThreadRequest(
                id = 7,
                params = MoveThreadParams(threadId = "acc#imap#inbox#thread", targetFolderId = "Work"),
            ).toJson(),
        )
    }

    @Test
    fun copyThreadPreservesFrontendWireFieldNames() {
        assertEquals(
            """{"id":8,"method":"mail.copy","params":{"thread_id":"acc#imap#inbox#thread","target_account_id":"acc2","target_folder_id":"Archive"}}""",
            copyThreadRequest(
                id = 8,
                params =
                    CopyThreadParams(
                        threadId = "acc#imap#inbox#thread",
                        targetAccountId = "acc2",
                        targetFolderId = "Archive",
                    ),
            ).toJson(),
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
        val request =
            syncMailRequest(
                id = 8,
                params =
                    SyncMailParams(
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
                params =
                    MoveRssFeedParams(
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
                params =
                    RssThreadParams(
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
                params =
                    RssMarkReadParams(
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
                params =
                    RssMarkStarredParams(
                        threadId = "rss-account#rss#feed-1",
                        starred = true,
                        itemKeys = listOf("item-1"),
                    ),
            ).toJson(),
        )
        assertEquals(
            """{"id":37,"method":"rss.exportOpml","params":{"account":"rss-account"}}""",
            feedExportOpmlRequest(id = 37, params = ExportOpmlParams(accountId = "rss-account")).toJson(),
        )
        assertEquals(
            """{"id":38,"method":"rss.importOpml","params":{"account":"rss-account","opml":"<opml/>"}}""",
            feedImportOpmlRequest(id = 38, params = ImportOpmlParams(accountId = "rss-account", opml = "<opml/>")).toJson(),
        )
        assertEquals(
            """{"id":39,"method":"mail.attachmentRead","params":{"key":"acc/INBOX/1/note.txt"}}""",
            attachmentReadRequest(id = 39, params = AttachmentReadParams(key = "acc/INBOX/1/note.txt")).toJson(),
        )
        assertEquals(
            """{"id":40,"method":"storage.usage","params":{}}""",
            storageUsageRequest(id = 40).toJson(),
        )
        assertEquals(
            """{"id":41,"method":"storage.clearCache","params":{}}""",
            storageClearCacheRequest(id = 41).toJson(),
        )
    }

    @Test
    fun sendMailMatchesDesktopComposerPayloadShape() {
        val request =
            sendMailRequest(
                id = 6,
                params =
                    SendMailParams(
                        accountId = "acc1",
                        from = "me@example.com",
                        to = "you@example.com",
                        cc = "cc@example.com",
                        subject = "Hi",
                        body = "Plain",
                        html = "<p>Plain</p>",
                        inReplyTo = "<old@example.com>",
                        references = "<root@example.com>",
                        attachments =
                            listOf(
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
    fun draftCommandsMatchDesktopComposerPayloadShape() {
        val draft =
            ComposeDraft(
                to = "you@example.com",
                cc = "cc@example.com",
                bcc = "bcc@example.com",
                subject = "Draft",
                body = "Body",
                attachments =
                    listOf(
                        DraftAttachment(
                            id = "a1",
                            displayName = "note.txt",
                            mimeType = "text/plain",
                            sizeBytes = 2,
                            dataBase64 = "SGk=",
                        ),
                    ),
            )

        assertEquals(
            """{"id":41,"method":"mail.saveDraft","params":{"account_id":"acc1","draft_id":"draft@example.com","from":"me@example.com","to":"you@example.com","cc":"cc@example.com","bcc":"bcc@example.com","reply_to":"","subject":"Draft","body":"Body","html":"","in_reply_to":"","references":"","attachments":[{"filename":"note.txt","mime":"text/plain","data":"SGk=","inline_id":""}]}}""",
            saveDraftRequest(
                id = 41,
                params =
                    draft.toSaveDraftParams(
                        accountId = "acc1",
                        draftId = "draft@example.com",
                        from = "me@example.com",
                    ),
            ).toJson(),
        )
        assertEquals(
            """{"id":42,"method":"mail.discardDraft","params":{"account_id":"acc1","draft_id":"draft@example.com"}}""",
            discardDraftRequest(id = 42, params = DiscardDraftParams(accountId = "acc1", draftId = "draft@example.com")).toJson(),
        )
    }

    @Test
    fun draftsFolderDetectionCoversCommonServerNames() {
        assertTrue(folderIsDrafts("Drafts"))
        assertTrue(folderIsDrafts("[Gmail]/Drafts"))
        assertTrue(folderIsDrafts("INBOX.Draft"))
    }

    @Test
    fun trashFolderDetectionCoversCommonServerNames() {
        assertTrue(folderIsTrash("Trash"))
        assertTrue(folderIsTrash("[Gmail]/Trash"))
        assertTrue(folderIsTrash("INBOX.Deleted Items"))
    }

    @Test
    fun replyMailParamsUseReplyRecipientAndThreadingHeaders() {
        val request =
            sendMailRequest(
                id = 7,
                params =
                    MessageBody(
                        id = "m1",
                        from = "Ada",
                        to = "Me <me@example.com>",
                        cc = "Project <project@example.com>",
                        subject = "Design",
                        body = "Original",
                        fromAddr = "ada@example.com",
                        replyTo = "Team <team@example.com>",
                        messageId = "old@example.com",
                        references = "<root@example.com>",
                    ).toReplyMailParams(
                        accountId = "acc1",
                        body = "Quick reply",
                        ownAddresses = listOf("me@example.com"),
                    ),
            )

        assertEquals(
            """{"id":7,"method":"mail.send","params":{"account_id":"acc1","from":"","to":"Team <team@example.com>","cc":"Project <project@example.com>","bcc":"","reply_to":"","subject":"Re: Design","body":"Quick reply","html":"","in_reply_to":"<old@example.com>","references":"<root@example.com> <old@example.com>","message_id":"","attachments":[]}}""",
            request.toJson(),
        )
    }

    @Test
    fun replyMailParamsIncludeAttachments() {
        val request =
            sendMailRequest(
                id = 43,
                params =
                    MessageBody(
                        id = "m1",
                        from = "Ada",
                        to = "Me <me@example.com>",
                        subject = "Design",
                        body = "Original",
                        fromAddr = "ada@example.com",
                        messageId = "old@example.com",
                    ).toReplyMailParams(
                        accountId = "acc1",
                        body = "With file",
                        attachments =
                            listOf(
                                DraftAttachment(
                                    id = "a1",
                                    displayName = "reply.txt",
                                    mimeType = "text/plain",
                                    sizeBytes = 2,
                                    dataBase64 = "SGk=",
                                ),
                            ),
                    ),
            )

        assertEquals(
            """{"id":43,"method":"mail.send","params":{"account_id":"acc1","from":"","to":"ada@example.com","cc":"","bcc":"","reply_to":"","subject":"Re: Design","body":"With file","html":"","in_reply_to":"<old@example.com>","references":"<old@example.com>","message_id":"","attachments":[{"filename":"reply.txt","mime":"text/plain","data":"SGk=","inline_id":""}]}}""",
            request.toJson(),
        )
    }

    @Test
    fun replyMailParamsFilterOwnAddressesAndUseOriginalToWhenSenderIsSelf() {
        val request =
            sendMailRequest(
                id = 8,
                params =
                    MessageBody(
                        id = "m2",
                        from = "Me",
                        to = "Ada <ada@example.com>, Alias <alias@example.com>",
                        cc = "Me <me@example.com>, Project <project@example.com>, Ada <ada@example.com>",
                        subject = "Re: Design",
                        body = "Original",
                        fromAddr = "me@example.com",
                        messageId = "<old@example.com>",
                    ).toReplyMailParams(
                        accountId = "acc1",
                        body = "Follow-up",
                        from = "alias@example.com",
                        ownAddresses = listOf("me@example.com", "alias@example.com"),
                    ),
            )

        assertEquals(
            """{"id":8,"method":"mail.send","params":{"account_id":"acc1","from":"alias@example.com","to":"Ada <ada@example.com>","cc":"Project <project@example.com>","bcc":"","reply_to":"","subject":"Re: Design","body":"Follow-up","html":"","in_reply_to":"<old@example.com>","references":"<old@example.com>","message_id":"","attachments":[]}}""",
            request.toJson(),
        )
    }

    @Test
    fun messageForwardAndEditDraftsCarryBodyHeadersAndAttachments() {
        val message =
            MessageBody(
                id = "m3",
                from = "Ada",
                fromAddr = "ada@example.com",
                to = "Me <me@example.com>",
                cc = "Project <project@example.com>",
                subject = "Design",
                body = "Original",
                bodyHtml = """<p>Original</p><img src="/media/acc/INBOX/3/inline.png">""",
                attachments =
                    listOf(
                        MessageAttachment(filename = "note.txt", mimeType = "text/plain", sizeBytes = 2, key = "acc/INBOX/3/note.txt"),
                        MessageAttachment(filename = "inline.png", mimeType = "image/png", sizeBytes = 4, key = "acc/INBOX/3/inline.png"),
                        MessageAttachment(filename = "remote.jpg", mimeType = "image/jpeg", url = "https://example.com/remote.jpg"),
                    ),
            )
        val copied =
            forwardableAttachments(message).map {
                attachmentToDraftAttachment(it, "SGk=")
            }
        val forward = messageForwardDraft(message, copied)
        val edit = messageEditAsNewDraft(message, copied)

        assertEquals("Fwd: Design", forward.subject)
        assertEquals("", forward.to)
        assertEquals(1, forward.attachments.size)
        assertEquals("note.txt", forward.attachments.single().displayName)
        assertEquals(true, forward.body.contains("---------- Forwarded message ---------"))
        assertEquals(true, forward.body.contains("Cc: Project <project@example.com>"))
        assertEquals("Me <me@example.com>", edit.to)
        assertEquals("Project <project@example.com>", edit.cc)
        assertEquals("Design", edit.subject)
        assertEquals("Original", edit.body)
        assertEquals(1, edit.attachments.size)
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
            """{"account_id":"acc1","folder_id":"INBOX"}""",
            MarkAllReadParams(accountId = "acc1", folderId = "INBOX").toJson(),
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
            """{"id":23,"method":"mail.markAllRead","params":{"account_id":"acc1","folder_id":"INBOX"}}""",
            markAllReadRequest(id = 23, MarkAllReadParams(accountId = "acc1", folderId = "INBOX")).toJson(),
        )
        assertEquals(
            """{"id":24,"method":"mail.markStarred","params":{"thread_id":"t1","starred":false}}""",
            markStarredRequest(id = 24, MarkStarredParams(threadId = "t1", starred = false)).toJson(),
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

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String {
            lastCommand = command
            lastPayloadJson = payloadJson
            return response
        }

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
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
