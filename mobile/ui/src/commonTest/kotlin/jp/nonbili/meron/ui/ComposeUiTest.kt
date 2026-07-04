package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.DraftAttachment
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

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

    @Test
    fun composeAutosaveSnapshotIgnoresEmptyDrafts() {
        assertNull(
            composeAutosaveSnapshot(
                selectedFromKey = "acc|me@example.com",
                to = " ",
                cc = "",
                bcc = "",
                subject = "",
                body = "",
                attachments = emptyList(),
            ),
        )
    }

    @Test
    fun composeAutosaveSnapshotIncludesRecipientSubjectAndBodyContent() {
        val snapshot =
            composeAutosaveSnapshot(
                selectedFromKey = "acc|me@example.com",
                to = "you@example.com",
                cc = "",
                bcc = "",
                subject = "Hello",
                body = "Body",
                attachments = emptyList(),
            )

        assertNotNull(snapshot)
        assertEquals("you@example.com", snapshot.to)
        assertEquals("Hello", snapshot.subject)
        assertEquals("Body", snapshot.body)
    }

    @Test
    fun composeAutosaveSnapshotIncludesAttachmentOnlyDrafts() {
        val attachment = DraftAttachment(id = "att1", displayName = "note.txt")
        val snapshot =
            composeAutosaveSnapshot(
                selectedFromKey = "acc|me@example.com",
                to = "",
                cc = "",
                bcc = "",
                subject = "",
                body = "",
                attachments = listOf(attachment),
            )

        assertNotNull(snapshot)
        assertEquals(listOf(attachment), snapshot.attachments)
    }

    @Test
    fun composeAutosaveSnapshotChangesWhenContentChanges() {
        val first =
            composeAutosaveSnapshot(
                selectedFromKey = "acc|me@example.com",
                to = "you@example.com",
                cc = "",
                bcc = "",
                subject = "Hello",
                body = "First",
                attachments = emptyList(),
            )
        val second =
            composeAutosaveSnapshot(
                selectedFromKey = "acc|me@example.com",
                to = "you@example.com",
                cc = "",
                bcc = "",
                subject = "Hello",
                body = "Second",
                attachments = emptyList(),
            )

        assertNotEquals(first, second)
    }
}
