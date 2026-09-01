package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class ThreadBottomAnchorTest {
    @Test
    fun capturesTheLowestVisibleMessageAndItsBottomGap() {
        val anchor =
            bottomVisibleMessageAnchor(
                visible =
                    listOf(
                        ListItemGeometry(index = 0, offset = -40, size = 80),
                        ListItemGeometry(index = 1, offset = 40, size = 300),
                        ListItemGeometry(index = 2, offset = 350, size = 300),
                        ListItemGeometry(index = 3, offset = 660, size = 100),
                    ),
                headerItemCount = 1,
                messageCount = 2,
                viewportStartOffset = 0,
                viewportEndOffset = 800,
            )

        assertEquals(BottomMessageAnchor(messageIndex = 1, bottomGapPx = 150), anchor)
    }

    @Test
    fun ignoresHeadersEndMarkersAndItemsOutsideTheViewport() {
        assertNull(
            bottomVisibleMessageAnchor(
                visible =
                    listOf(
                        ListItemGeometry(index = 0, offset = 0, size = 80),
                        ListItemGeometry(index = 2, offset = 900, size = 100),
                    ),
                headerItemCount = 1,
                messageCount = 1,
                viewportStartOffset = 0,
                viewportEndOffset = 800,
            ),
        )
    }
}
