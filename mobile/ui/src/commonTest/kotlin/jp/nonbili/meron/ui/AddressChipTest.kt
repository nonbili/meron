package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.MessageBody
import kotlin.test.Test
import kotlin.test.assertEquals

class AddressChipTest {
    @Test
    fun participantAddressesPreserveQuotedNames() {
        assertEquals(
            listOf("Doe, Jane" to "jane@x.com", "Alice" to "a@y.com"),
            parseAddressList(""""Doe, Jane" <jane@x.com>; Alice <a@y.com>"""),
        )
    }

    @Test
    fun quotedNamesDoNotInflateRecipientSummary() {
        val items = addressChipItems(""""Doe, Jane" <jane@x.com>; Alice <a@y.com>""")
        assertEquals(listOf("Doe, Jane", "Alice"), items.map { it.display })
        assertEquals("Doe, Jane, Alice", summarizeRecipients(items))
    }

    @Test
    fun namedAddressKeepsNameAndAddressApart() {
        val items = addressChipItems(""""Ping Chen" <ping@example.com>""")
        assertEquals(1, items.size)
        assertEquals("Ping Chen", items[0].display)
        assertEquals("ping@example.com", items[0].email)
        assertEquals(""""Ping Chen" <ping@example.com>""", items[0].full)
    }

    @Test
    fun bareAddressHasNoSeparateEmailLine() {
        val items = addressChipItems("ping@example.com, second@example.com")
        assertEquals(listOf("ping@example.com", "second@example.com"), items.map { it.display })
        assertEquals(listOf("", ""), items.map { it.email })
    }

    @Test
    fun nameRepeatingTheAddressIsNotShownTwice() {
        val items = addressChipItems("ping@example.com <ping@example.com>")
        assertEquals("ping@example.com", items[0].display)
        assertEquals("", items[0].email)
    }

    @Test
    fun senderBecomesOneAddressHeader() {
        val message =
            MessageBody(
                id = "1",
                from = "Ping Chen",
                to = "someone@example.com",
                subject = "hi",
                body = "",
                fromAddr = "ping@example.com",
            )
        assertEquals("Ping Chen <ping@example.com>", fullFromAddress(message))
        assertEquals("ping@example.com", fullFromAddress(message.copy(from = "")))
        assertEquals("Ping Chen", fullFromAddress(message.copy(fromAddr = "")))
    }
}
