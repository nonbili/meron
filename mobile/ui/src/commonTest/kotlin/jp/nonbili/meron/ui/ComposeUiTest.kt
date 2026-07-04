package jp.nonbili.meron.ui

import kotlin.test.Test
import kotlin.test.assertEquals

class ComposeUiTest {
    @Test
    fun testParseRecipientsEmpty() {
        val (completed, active) = parseRecipients("")
        assertEquals(emptyList(), completed)
        assertEquals("", active)
    }

    @Test
    fun testParseRecipientsSingleNoComma() {
        val (completed, active) = parseRecipients("alice@example.com")
        assertEquals(emptyList(), completed)
        assertEquals("alice@example.com", active)
    }

    @Test
    fun testParseRecipientsWithCommas() {
        val (completed, active) = parseRecipients("alice@example.com, bob@example.com, ch")
        assertEquals(listOf("alice@example.com", "bob@example.com"), completed)
        assertEquals("ch", active)
    }

    @Test
    fun testParseRecipientsWithTrailingComma() {
        val (completed, active) = parseRecipients("alice@example.com, ")
        assertEquals(listOf("alice@example.com"), completed)
        assertEquals("", active)
    }

    @Test
    fun testParseEmailRecipientRawEmail() {
        val (name, email) = parseEmailRecipient("alice@example.com")
        assertEquals("", name)
        assertEquals("alice@example.com", email)
    }

    @Test
    fun testParseEmailRecipientWithNameAngleBrackets() {
        val (name, email) = parseEmailRecipient("Alice Smith <alice@example.com>")
        assertEquals("Alice Smith", name)
        assertEquals("alice@example.com", email)
    }

    @Test
    fun testParseEmailRecipientWithNameParentheses() {
        val (name, email) = parseEmailRecipient("alice@example.com (Alice Smith)")
        assertEquals("Alice Smith", name)
        assertEquals("alice@example.com", email)
    }
}
