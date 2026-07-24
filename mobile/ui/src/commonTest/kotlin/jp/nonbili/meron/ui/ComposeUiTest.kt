package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.CoroutineScope
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

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
    fun testParseRecipientsKeepsQuotedCommaNameAsOneChip() {
        val (completed, active) = parseRecipients("\"Doe, Jane\" <jane@example.com>, bo")
        assertEquals(listOf("\"Doe, Jane\" <jane@example.com>"), completed)
        assertEquals("bo", active)
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

    @Test
    fun threadsWithDraftFlagMarksVisibleThreadListRow() {
        val threads =
            listOf(
                threadSummary(id = "acc#INBOX#one"),
                threadSummary(id = "acc#INBOX#two"),
            )

        val updated = threadsWithDraftFlag(threads, "acc#INBOX#two")

        assertFalse(updated[0].hasDraft)
        assertTrue(updated[1].hasDraft)
    }

    @Test
    fun threadsWithDraftFlagCanMatchThreadIdAlias() {
        val threads =
            listOf(
                threadSummary(id = "row-1", threadId = "acc#INBOX#one"),
            )

        val updated = threadsWithDraftFlag(threads, "acc#INBOX#one")

        assertTrue(updated.single().hasDraft)
    }

    @Test
    fun draftThreadWithCachedAncestorOpensConversation() {
        val messages =
            listOf(
                messageBody(id = "m1", folderId = "INBOX"),
                messageBody(id = "d1", folderId = "Drafts"),
            )

        assertTrue(draftThreadShouldOpenConversation(messages))
    }

    @Test
    fun referencedDraftThreadOpensConversation() {
        val messages =
            listOf(
                messageBody(id = "d1", folderId = "Drafts", references = "root@example.com"),
            )

        assertTrue(draftThreadShouldOpenConversation(messages))
    }

    @Test
    fun inReplyToDraftThreadOpensConversation() {
        val messages =
            listOf(
                messageBody(id = "d1", folderId = "Drafts", inReplyTo = "root@example.com"),
            )

        assertTrue(draftThreadShouldOpenConversation(messages))
    }

    @Test
    fun standaloneDraftThreadOpensComposer() {
        val messages =
            listOf(
                messageBody(id = "d1", folderId = "Drafts"),
            )

        assertFalse(draftThreadShouldOpenConversation(messages))
    }

    @Test
    fun visibleThreadMessagesHidesTailDraftHydratedIntoQuickReply() {
        val state = testState()
        state.messages =
            listOf(
                messageBody(id = "m1", folderId = "INBOX"),
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
            )
        state.quickReplyDraftId = "draft-1"

        val visible = state.visibleThreadMessages()

        assertEquals(listOf("m1"), visible.map { it.id })
    }

    @Test
    fun visibleThreadMessagesHidesHydratedDraftBeforeOptimisticSend() {
        val state = testState()
        state.messages =
            listOf(
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
                messageBody(id = "m2", folderId = "INBOX"),
            )
        state.quickReplyDraftId = "draft-1"

        val visible = state.visibleThreadMessages()

        assertEquals(listOf("m2"), visible.map { it.id })
    }

    @Test
    fun visibleThreadMessagesKeepsUnrelatedOlderDraftVisible() {
        val state = testState()
        state.messages =
            listOf(
                messageBody(id = "d0", folderId = "Drafts", messageId = "draft-0"),
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
            )
        state.quickReplyDraftId = "draft-1"

        val visible = state.visibleThreadMessages()

        assertEquals(listOf("d0"), visible.map { it.id })
    }

    @Test
    fun removeDiscardedDraftFromOpenThreadDropsCachedDraftAfterSend() {
        val state = testState()
        state.messages =
            listOf(
                messageBody(id = "m1", folderId = "INBOX"),
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
                messageBody(id = "local-send-1", folderId = "INBOX"),
            )

        state.removeDiscardedDraftFromOpenThread("draft-1")

        assertEquals(listOf("m1", "local-send-1"), state.messages.map { it.id })
    }

    @Test
    fun visibleThreadMessagesReturnsAllWhenNoQuickReplyDraftHydrated() {
        val state = testState()
        state.messages =
            listOf(
                messageBody(id = "m1", folderId = "INBOX"),
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
            )

        assertEquals(state.messages, state.visibleThreadMessages())
    }

    private fun threadSummary(
        id: String,
        threadId: String = "",
    ): ThreadSummary =
        ThreadSummary(
            id = id,
            accountId = "acc",
            folder = "INBOX",
            subject = "Subject",
            sender = "sender@example.com",
            threadId = threadId,
        )

    private fun messageBody(
        id: String,
        folderId: String,
        inReplyTo: String = "",
        references: String = "",
        messageId: String = "",
    ): MessageBody =
        MessageBody(
            id = id,
            folderId = folderId,
            from = "sender@example.com",
            to = "me@example.com",
            subject = "Subject",
            body = "Body",
            inReplyTo = inReplyTo,
            references = references,
            messageId = messageId,
        )

    private fun testState(): MeronMobileState =
        MeronMobileState(
            scope = CoroutineScope(EmptyCoroutineContext),
            core = FakeCore(),
            coreLoaded = true,
            prefs = FakePreferences(),
            kanbanPrefs = FakePreferences(),
            services = FakePlatformServices(),
            locale = FakeLocaleController(),
            mobileHost = DefaultMobileHost(),
        )

    private class FakePlatformServices : PlatformServices {
        override fun openUrl(url: String) {}

        override fun openOAuthUrl(
            url: String,
            callbackScheme: String,
            onCallback: (String) -> Unit,
            onFailure: (String) -> Unit,
        ) {}

        override fun copyText(
            label: String,
            value: String,
        ) {}

        override fun copyImage(
            bytes: ByteArray,
            mimeType: String,
            label: String,
        ) {}

        override fun shareFile(
            bytes: ByteArray,
            fileName: String,
            mimeType: String,
        ) {}

        override fun saveFile(
            bytes: ByteArray,
            fileName: String,
            mimeType: String,
        ) {}

        override fun pickFile(
            mimeTypes: List<String>,
            onPicked: (PickedFile?) -> Unit,
        ) {}

        override fun pickImage(onPicked: (PickedFile?) -> Unit) {}
    }

    private class FakePreferences : AppPreferences {
        private val strings = mutableMapOf<String, String>()
        private val booleans = mutableMapOf<String, Boolean>()
        private val ints = mutableMapOf<String, Int>()
        private val stringSets = mutableMapOf<String, Set<String>>()

        override fun getString(
            key: String,
            default: String,
        ): String = strings[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            strings[key] = value
        }

        override fun getBoolean(
            key: String,
            default: Boolean,
        ): Boolean = booleans[key] ?: default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {
            booleans[key] = value
        }

        override fun getInt(
            key: String,
            default: Int,
        ): Int = ints[key] ?: default

        override fun putInt(
            key: String,
            value: Int,
        ) {
            ints[key] = value
        }

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = stringSets[key] ?: default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {
            stringSets[key] = value
        }

        override fun remove(key: String) {
            strings.remove(key)
            booleans.remove(key)
            ints.remove(key)
            stringSets.remove(key)
        }
    }

    private class FakeLocaleController : LocaleController {
        override fun currentLanguageTag(): String = ""

        override fun apply(tag: String) {}

        override fun displayName(tag: String): String = tag
    }

    private class FakeCore : MeronCore {
        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String = "{}"

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
    }
}
