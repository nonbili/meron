package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.BodyLink
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class MessageTextTest {
    private val text = "See [docs](example.com/docs) and https://example.com/raw"
    private val links =
        listOf(
            BodyLink(4, 28, "https://example.com/docs", "docs"),
            BodyLink(33, 56, "https://example.com/raw"),
        )

    @Test
    fun drawsTheCoreLinksWithMarkdownLabelsInPlaceOfTheirSource() {
        val parsed = parseInlineMessageText(text, links)

        assertEquals("See docs and https://example.com/raw", parsed.text)
        assertEquals(
            listOf(
                InlineMessageLink(4, 8, "https://example.com/docs"),
                InlineMessageLink(13, 36, "https://example.com/raw"),
            ),
            parsed.links,
        )
    }

    @Test
    fun leavesTextWithoutLinksAsWritten() {
        val parsed = parseInlineMessageText(text, emptyList())

        assertEquals(text, parsed.text)
        assertTrue(parsed.links.isEmpty())
    }

    @Test
    fun findsBareUrlsItselfOnlyForABodyTheCoreNeverSaw() {
        val body = "Join https://beta.example/join/AbCd1234, ok"

        assertEquals(listOf(BodyLink(5, 39, "https://beta.example/join/AbCd1234")), plainBodyLinks(body, null))
        assertEquals(emptyList(), plainBodyLinks(body, emptyList()))
        assertEquals(
            listOf(
                BodyLink(0, 27, "https://x.example/d", "docs"),
                BodyLink(32, 53, "https://x.example/raw"),
            ),
            plainBodyLinks("[docs](https://x.example/d) and https://x.example/raw", null),
        )
    }

    @Test
    fun splitsTheLinksBetweenReplyAndQuote() {
        val body = "see https://x.example/a\n\n> was https://x.example/b\n"
        val bodyLinks = listOf(BodyLink(4, 23, "https://x.example/a"), BodyLink(31, 50, "https://x.example/b"))
        val split = splitQuotedBody(body, 25, bodyLinks)

        assertEquals("see https://x.example/a", split.reply)
        assertEquals(listOf(bodyLinks[0]), split.replyLinks)
        assertEquals("> was https://x.example/b", split.quote)
        assertEquals(listOf(BodyLink(6, 25, "https://x.example/b")), split.quoteLinks)
    }

    @Test
    fun findsTheLinkALongPressLandsOn() {
        val parsed = parseInlineMessageText(text, links)

        assertEquals(null, messageLinkAtCharOffset(parsed.links, 3))
        assertEquals(parsed.links[0], messageLinkAtCharOffset(parsed.links, 4))
        assertEquals(parsed.links[0], messageLinkAtCharOffset(parsed.links, 7))
        // The end offset is the first character past the link.
        assertEquals(null, messageLinkAtCharOffset(parsed.links, 8))
        assertEquals(parsed.links[1], messageLinkAtCharOffset(parsed.links, 20))
    }
}
