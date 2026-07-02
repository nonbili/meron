package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class ScrollReadMarkingTest {
    @Test
    fun countsMessagesFullyAboveViewportAsPassed() {
        // Items 0-1 scrolled off the top entirely, item 2 is the first visible.
        val visible =
            listOf(
                ListItemGeometry(index = 2, offset = -40, size = 200),
                ListItemGeometry(index = 3, offset = 170, size = 300),
            )

        val passed =
            scrolledPastMessageIndices(
                visible = visible,
                firstVisibleIndex = 2,
                headerItemCount = 0,
                messageCount = 4,
                topSlackPx = 24,
            )

        assertEquals(listOf(0, 1), passed)
    }

    @Test
    fun visibleBubbleCountsOnceItsBottomClearsTheSlack() {
        val stillVisible = ListItemGeometry(index = 0, offset = -80, size = 110)
        val past = ListItemGeometry(index = 0, offset = -100, size = 110)

        assertEquals(
            emptyList(),
            scrolledPastMessageIndices(listOf(stillVisible), 0, headerItemCount = 0, messageCount = 2, topSlackPx = 24),
        )
        assertEquals(
            listOf(0),
            scrolledPastMessageIndices(listOf(past), 0, headerItemCount = 0, messageCount = 2, topSlackPx = 24),
        )
    }

    @Test
    fun headerRowDoesNotCountAsAPassedMessage() {
        // The load-older row (item 0) scrolled off; message 0 (item 1) still visible.
        val visible = listOf(ListItemGeometry(index = 1, offset = 10, size = 300))

        val passed =
            scrolledPastMessageIndices(
                visible = visible,
                firstVisibleIndex = 1,
                headerItemCount = 1,
                messageCount = 2,
                topSlackPx = 24,
            )

        assertEquals(emptyList(), passed)
    }

    @Test
    fun nothingPassedAtTheTopOfTheList() {
        val visible =
            listOf(
                ListItemGeometry(index = 0, offset = 0, size = 200),
                ListItemGeometry(index = 1, offset = 210, size = 200),
            )

        assertEquals(
            emptyList(),
            scrolledPastMessageIndices(visible, 0, headerItemCount = 0, messageCount = 2, topSlackPx = 24),
        )
    }

    @Test
    fun viewedToBottomRequiresLastItemNearViewportEnd() {
        val lastItemNear = listOf(ListItemGeometry(index = 4, offset = 700, size = 200))
        val lastItemFar = listOf(ListItemGeometry(index = 4, offset = 700, size = 500))
        val notLastItem = listOf(ListItemGeometry(index = 3, offset = 700, size = 100))

        assertTrue(listViewedToBottom(lastItemNear, totalItemCount = 5, viewportEndOffset = 800, bottomSlackPx = 160))
        assertFalse(listViewedToBottom(lastItemFar, totalItemCount = 5, viewportEndOffset = 800, bottomSlackPx = 160))
        assertFalse(listViewedToBottom(notLastItem, totalItemCount = 5, viewportEndOffset = 800, bottomSlackPx = 160))
        assertFalse(listViewedToBottom(emptyList(), totalItemCount = 5, viewportEndOffset = 800, bottomSlackPx = 160))
    }

    @Test
    fun shortThreadThatFitsTheViewportCountsAsViewedToBottom() {
        val visible =
            listOf(
                ListItemGeometry(index = 0, offset = 0, size = 200),
                ListItemGeometry(index = 1, offset = 210, size = 200),
            )

        assertTrue(listViewedToBottom(visible, totalItemCount = 2, viewportEndOffset = 800, bottomSlackPx = 160))
    }
}
