package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.MessageBody
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ThreadOpenScrollTest {
    @Test
    fun jumpsToFirstUnreadMessage() {
        val messages = listOf(message("m1"), message("m2", unread = true), message("m3", unread = true))

        assertEquals(1, threadOpenScrollIndex(messages, headerItemCount = 0))
    }

    @Test
    fun offsetsForLoadOlderHeaderRow() {
        val messages = listOf(message("m1"), message("m2", unread = true))

        assertEquals(2, threadOpenScrollIndex(messages, headerItemCount = 1))
    }

    @Test
    fun jumpsToNewestMessageWhenAllRead() {
        val messages = listOf(message("m1"), message("m2"), message("m3"))

        assertEquals(2, threadOpenScrollIndex(messages, headerItemCount = 0))
        assertEquals(3, threadOpenScrollIndex(messages, headerItemCount = 1))
    }

    @Test
    fun staysPutWhenTargetIsAlreadyAtTop() {
        assertNull(threadOpenScrollIndex(listOf(message("m1", unread = true), message("m2")), headerItemCount = 0))
        assertNull(threadOpenScrollIndex(listOf(message("m1")), headerItemCount = 0))
        assertNull(threadOpenScrollIndex(emptyList(), headerItemCount = 1))
    }

    @Test
    fun firstUnreadStillNeedsScrollPastHeaderRow() {
        val messages = listOf(message("m1", unread = true), message("m2"))

        assertEquals(1, threadOpenScrollIndex(messages, headerItemCount = 1))
    }

    private fun message(
        id: String,
        unread: Boolean = false,
    ): MessageBody =
        MessageBody(
            id = id,
            from = "sender@example.com",
            to = "me@example.com",
            subject = "Subject",
            body = "Body",
            unread = unread,
        )
}
