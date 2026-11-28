package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class SelectionQuoteTest {
    @Test
    fun quotesEachLineAndDropsSurroundingBlankLines() {
        assertEquals(
            "> - Konversationansicht ausschaltbar.\n>\n> - Dunkles Design.",
            quotedSelectionText("\n  \n- Konversationansicht ausschaltbar.  \r\n\r\n- Dunkles Design.\n\n"),
        )
    }

    @Test
    fun treatsNonBreakingSpacesAsWhitespace() {
        assertEquals("> a b", quotedSelectionText("a b "))
    }

    @Test
    fun isEmptyForWhitespace() {
        assertEquals("", quotedSelectionText(" \n \n"))
    }
}
