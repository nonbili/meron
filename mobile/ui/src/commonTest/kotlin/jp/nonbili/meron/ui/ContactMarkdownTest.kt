package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ContactMarkdownTest {
    @Test
    fun rendersBasicBlocksAndPreservesFencedCodeLiterally() {
        val blocks =
            parseContactMarkdown(
                "# Name\n\nNotes\ncontinued\n- friend\n2. colleague\n> quote\n```kotlin\n**literal** [link](https://example.com)\n```",
            )
        assertEquals(
            listOf(
                ContactMarkdownKind.Heading,
                ContactMarkdownKind.Paragraph,
                ContactMarkdownKind.ListItem,
                ContactMarkdownKind.ListItem,
                ContactMarkdownKind.Quote,
                ContactMarkdownKind.Code,
            ),
            blocks.map { it.kind },
        )
        assertEquals(1, blocks.first().level)
        assertEquals("Notes\ncontinued", blocks[1].content.text)
        assertEquals("• ", blocks[2].marker)
        assertEquals("2. ", blocks[3].marker)
        assertEquals("**literal** [link](https://example.com)", blocks.last().content.text)
        assertTrue(
            blocks
                .last()
                .content.spans
                .isEmpty(),
        )
    }

    @Test
    fun rendersInlineStylesAndExplicitLinksWithCorrectRanges() {
        val parsed = parseContactMarkdownInline("**Bold** *em* `code` [site](https://example.com/a_(b))")
        assertEquals("Bold em code site", parsed.text)
        assertEquals(
            listOf(
                ContactMarkdownSpan(0, 4, ContactMarkdownStyle.Bold),
                ContactMarkdownSpan(5, 7, ContactMarkdownStyle.Emphasis),
                ContactMarkdownSpan(8, 12, ContactMarkdownStyle.Code),
                ContactMarkdownSpan(13, 17, url = "https://example.com/a_(b)"),
            ),
            parsed.spans,
        )
    }

    @Test
    fun supportsNestedFormattingWithoutLinksInCode() {
        val parsed = parseContactMarkdownInline("**bold _em_** `[x](https://example.com)`")
        assertEquals("bold em [x](https://example.com)", parsed.text)
        assertTrue(parsed.spans.contains(ContactMarkdownSpan(5, 7, ContactMarkdownStyle.Emphasis)))
        assertTrue(parsed.spans.contains(ContactMarkdownSpan(0, 7, ContactMarkdownStyle.Bold)))
        assertTrue(parsed.spans.none { it.url != null })
    }

    @Test
    fun imagesOnlyShowAltTextAndHtmlIsNeverInterpreted() {
        val parsed =
            parseContactMarkdownInline(
                "![portrait](https://tracker.example/pixel) <img src=\"https://tracker.example\"> <b>name</b>",
            )
        assertEquals("portrait <img src=\"https://tracker.example\"> <b>name</b>", parsed.text)
        assertTrue(parsed.spans.isEmpty())
    }

    @Test
    fun onlyExplicitSafeLinksAreClickable() {
        val parsed =
            parseContactMarkdownInline(
                "https://example.com [bad](javascript:alert(1)) [file](file:///secret) [ok](mailto:a@example.com)",
            )
        assertEquals("https://example.com bad file ok", parsed.text)
        assertEquals(listOf("mailto:a@example.com"), parsed.spans.mapNotNull { it.url })
    }

    @Test
    fun rejectsDangerousRelativeObfuscatedAndMalformedDestinations() {
        listOf(
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///secret",
            "tel:123",
            "intent://x",
            "//example.com",
            "/relative",
            "example.com",
            " https://example.com",
            "https://",
            "https:///path",
            "https://user@example.com",
            "https://example.com\\evil",
            "https://example.com/\npath",
            "https://example.com/\u0000",
            "mailto:",
            "java&#115;cript:alert(1)",
            "https://<script>",
        ).forEach { assertNull(safeContactMarkdownUrl(it), it) }
        listOf("http://example.com", "HTTPS://example.com/path?q=1#part", "mailto:a@example.com").forEach {
            assertEquals(it, safeContactMarkdownUrl(it))
        }
    }

    @Test
    fun malformedMarkdownAndEscapesRemainReadable() {
        assertEquals("[unfinished](url **open", parseContactMarkdownInline("[unfinished](url **open").text)
        assertEquals("*plain*", parseContactMarkdownInline("\\*plain\\*").text)
        assertTrue(parseContactMarkdownInline("\\*plain\\*").spans.isEmpty())
        assertTrue(parseContactMarkdown("\n\r\n").isEmpty())
        val code = parseContactMarkdown("~~~\n[unsafe](javascript:x)\nno closing fence").single()
        assertEquals(ContactMarkdownKind.Code, code.kind)
        assertTrue(code.content.spans.isEmpty())
    }
}
