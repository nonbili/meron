package jp.nonbili.meron.ui

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
    fun kanbanColumnUnreadUsesTotalReturnedWithPage() {
        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = "acc1", folderId = "inbox"),
                folderUnread = 137,
                loadedThreads =
                    listOf(
                        thread("t1", unread = true),
                        thread("t2", unread = true),
                    ),
            )

        assertEquals(137, count)
    }

    @Test
    fun kanbanColumnUnreadUsesSummedUnifiedTotalReturnedWithPage() {
        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = UNIFIED_ACCOUNT_ID, folderId = INBOX_FOLDER),
                folderUnread = 70,
            )

        assertEquals(70, count)
    }

    @Test
    fun kanbanColumnUnreadTrustsGenuineZeroFolderTotal() {
        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = "acc1", folderId = "inbox"),
                folderUnread = 0,
                loadedThreads = listOf(thread("t1", unread = true)),
            )

        assertEquals(0, count)
    }

    @Test
    fun kanbanColumnUnreadFallsBackToLoadedMessageTotals() {
        val count =
            kanbanColumnUnreadCount(
                column = KanbanColumnSpec(accountId = "acc1", folderId = INBOX_FOLDER),
                folderUnread = null,
                loadedThreads = listOf(thread("t1", unread = true, unreadCount = 2), thread("t2", unread = false)),
            )

        assertEquals(2, count)
    }

    private fun thread(
        id: String,
        unread: Boolean,
        unreadCount: Int = if (unread) 1 else 0,
    ): ThreadSummary =
        ThreadSummary(
            id = id,
            accountId = "acc1",
            folder = "INBOX",
            subject = "Subject",
            sender = "sender@example.com",
            unread = unread,
            unreadCount = unreadCount,
        )
}
