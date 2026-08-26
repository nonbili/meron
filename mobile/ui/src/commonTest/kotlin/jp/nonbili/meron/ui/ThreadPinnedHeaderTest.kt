package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ThreadPinnedHeaderTest {
    private fun pin(
        visible: List<ListItemGeometry>,
        expanded: (Int) -> Boolean = { true },
        viewportStartOffset: Int = 0,
    ): Int? =
        pinnedHeaderMessageIndex(
            visible = visible,
            headerItemCount = 1,
            messageCount = 3,
            viewportStartOffset = viewportStartOffset,
            minRemainingPx = 100,
            expanded = expanded,
        )

    @Test
    fun pinsTheMessageStraddlingTheTopEdge() {
        val visible =
            listOf(
                ListItemGeometry(index = 1, offset = -400, size = 900),
                ListItemGeometry(index = 2, offset = 500, size = 300),
            )
        assertEquals(0, pin(visible))
    }

    @Test
    fun noPinWhenTheTopMessageStartsInsideTheViewport() {
        val visible = listOf(ListItemGeometry(index = 1, offset = 20, size = 900))
        assertNull(pin(visible))
    }

    @Test
    fun noPinWhenTheStraddlingMessageIsCollapsed() {
        val visible =
            listOf(
                ListItemGeometry(index = 1, offset = -40, size = 200),
                ListItemGeometry(index = 2, offset = 160, size = 900),
            )
        assertNull(pin(visible, expanded = { it != 0 }))
    }

    @Test
    fun noPinWhenAlmostAllOfTheMessageIsScrolledAway() {
        val visible =
            listOf(
                ListItemGeometry(index = 1, offset = -880, size = 900),
                ListItemGeometry(index = 2, offset = 20, size = 900),
            )
        assertNull(pin(visible))
    }

    @Test
    fun listItemsAboveTheMessagesNeverPin() {
        val visible =
            listOf(
                ListItemGeometry(index = 0, offset = -60, size = 80),
                ListItemGeometry(index = 1, offset = 20, size = 900),
            )
        assertNull(pin(visible))
    }

    @Test
    fun contentPaddingShiftsTheTopEdge() {
        val visible = listOf(ListItemGeometry(index = 1, offset = -10, size = 900))
        assertNull(pin(visible, viewportStartOffset = -24))
        assertEquals(0, pin(visible, viewportStartOffset = 0))
    }
}
