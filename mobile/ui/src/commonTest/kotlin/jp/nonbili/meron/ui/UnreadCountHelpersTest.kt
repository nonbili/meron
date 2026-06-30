package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ThreadSummary
import kotlin.test.Test
import kotlin.test.assertEquals

class UnreadCountHelpersTest {
    @Test
    fun folderUnreadTreatsInboxCaseInsensitively() {
        val folders =
            listOf(
                FolderSummary(accountId = "acc1", name = "INBOX", unread = 12),
                FolderSummary(accountId = "acc1", name = "Archive", unread = 5),
            )

        assertEquals(12, folderUnread(folders, "inbox"))
        assertEquals(5, folderUnread(folders, "Archive"))
        assertEquals(0, folderUnread(folders, "archive"))
    }

    @Test
    fun kanbanColumnUnreadUsesFolderTotalForMailColumn() {
        val foldersByAccount =
            mapOf(
                "acc1" to listOf(FolderSummary(accountId = "acc1", name = "INBOX", unread = 137)),
            )

        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = "acc1", folderId = "inbox"),
                foldersByAccount = foldersByAccount,
                accounts = listOf(account("acc1")),
                loadedThreads =
                    listOf(
                        thread("t1", unread = true),
                        thread("t2", unread = true),
                    ),
            )

        assertEquals(137, count)
    }

    @Test
    fun kanbanColumnUnreadSumsIncludedUnifiedAccounts() {
        val foldersByAccount =
            mapOf(
                "acc1" to listOf(FolderSummary(accountId = "acc1", name = "INBOX", unread = 70)),
                "acc2" to listOf(FolderSummary(accountId = "acc2", name = "INBOX", unread = 50)),
            )

        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = UNIFIED_ACCOUNT_ID, folderId = INBOX_FOLDER),
                foldersByAccount = foldersByAccount,
                accounts = listOf(account("acc1"), account("acc2", includedInUnified = false)),
            )

        assertEquals(70, count)
    }

    @Test
    fun kanbanColumnUnreadFallsBackToLoadedStarredItems() {
        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = UNIFIED_ACCOUNT_ID, folderId = STARRED_FOLDER),
                foldersByAccount = emptyMap(),
                accounts = emptyList(),
                loadedThreads = listOf(thread("t1", unread = true), thread("t2", unread = false)),
            )

        assertEquals(1, count)
    }

    private fun account(
        id: String,
        includedInUnified: Boolean = true,
    ): AccountSummary = AccountSummary(id = id, email = "$id@example.com", includedInUnified = includedInUnified)

    private fun thread(
        id: String,
        unread: Boolean,
    ): ThreadSummary =
        ThreadSummary(
            id = id,
            accountId = "acc1",
            folder = "INBOX",
            subject = "Subject",
            sender = "sender@example.com",
            unread = unread,
        )
}
