package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class LinkifiedTextTest {
    @Test
    fun findsMeetingLinksInInvitationNotes() {
        val notes =
            """
            -::~:~::~:~::-
            Join with Google Meet: https://meet.example.com/abc-defg-hij
            More numbers: https://tel.example.com/abc?pin=123&hs=7

            Learn more (https://support.example.com/a/answer/1).
            """.trimIndent()
        assertEquals(
            listOf(
                "https://meet.example.com/abc-defg-hij",
                "https://tel.example.com/abc?pin=123&hs=7",
                "https://support.example.com/a/answer/1",
            ),
            findBareUrls(notes).map { it.text },
        )
        findBareUrls(notes).forEach { assertEquals(it.text, notes.substring(it.start, it.end)) }
    }

    @Test
    fun keepsBalancedBracketsAndLinksWww() {
        assertEquals(
            listOf("https://en.example.org/wiki/Foo_(bar)" to "https://en.example.org/wiki/Foo_(bar)", "www.example.com" to "http://www.example.com"),
            findBareUrls("See https://en.example.org/wiki/Foo_(bar), or www.example.com!").map { it.text to it.href },
        )
    }

    @Test
    fun skipsBareDomainsAndEmbeddedSchemes() {
        assertEquals(emptyList(), findBareUrls("example.com, user@www.example.com, xhttps://example.com, www.x"))
    }
}
