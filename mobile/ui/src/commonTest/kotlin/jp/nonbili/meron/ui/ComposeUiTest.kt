package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountAlias
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SignatureMark
import jp.nonbili.meron.shared.SignaturePlacement
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.bodyWithSignature
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
    fun freshComposeClearsAReplyLeftByThePreviouslyOpenedThread() {
        val state = testState()
        state.appSignatureLoaded = true
        state.to = "sender@example.com"
        state.cc = "copy@example.com"
        state.subject = "Re: Subject"
        state.body = "Reply body"
        state.composeInReplyTo = "message@example.com"
        state.composeReferences = "root@example.com message@example.com"
        state.composeDraftId = "reply-draft@example.com"
        state.composeDraftSaved = true

        state.openCompose()

        assertEquals("", state.to)
        assertEquals("", state.cc)
        assertEquals("", state.subject)
        assertEquals("", state.composeInReplyTo)
        assertEquals("", state.composeReferences)
        assertEquals("", state.composeDraftId)
        assertFalse(state.composeDraftSaved)
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
    fun composeIsBlankIgnoresTheSeededSignature() {
        val state = testState()
        state.composeSignature = SignatureMark("-- \nPing", SignaturePlacement.BelowText)
        state.body = bodyWithSignature("", "-- \nPing")

        assertTrue(state.composeIsBlank())
    }

    @Test
    fun composeIsBlankSeesTextWrittenAboveTheSignature() {
        val state = testState()
        state.composeSignature = SignatureMark("-- \nPing", SignaturePlacement.BelowText)
        state.body = bodyWithSignature("Hello", "-- \nPing")

        assertFalse(state.composeIsBlank())
    }

    @Test
    fun composeIsBlankIgnoresAReplysPrefilledHeaders() {
        val state = testState()
        state.composeSignature = SignatureMark("-- \nPing", SignaturePlacement.BelowText)
        state.body = bodyWithSignature("", "-- \nPing")
        state.to = "noreply@example.com"
        state.subject = "Re: Hello"
        state.rememberComposeSeed()

        assertTrue(state.composeIsBlank())
    }

    @Test
    fun composeIsBlankIgnoresTheChipFieldsTrailingSeparator() {
        val state = testState()
        state.to = "Ray Dalio <ray@example.com>"
        state.subject = "Re: Hello"
        state.rememberComposeSeed()
        // What RecipientChipsInput does to a prefilled field on first render.
        state.to = "Ray Dalio <ray@example.com>, "

        assertTrue(state.composeIsBlank())
    }

    @Test
    fun composeIsBlankSeesARecipientTheUserAdded() {
        val state = testState()
        state.composeSignature = SignatureMark("-- \nPing", SignaturePlacement.BelowText)
        state.body = bodyWithSignature("", "-- \nPing")
        state.to = "noreply@example.com"
        state.subject = "Re: Hello"
        state.rememberComposeSeed()

        state.cc = "someone@example.com"

        assertFalse(state.composeIsBlank())
    }

    @Test
    fun composeIsBlankSeesAnAttachmentOnItsOwn() {
        val state = testState()
        state.composeSignature = SignatureMark("-- \nPing", SignaturePlacement.BelowText)
        state.body = bodyWithSignature("", "-- \nPing")
        state.attachments = listOf(DraftAttachment(id = "a1", displayName = "a.txt", mimeType = "text/plain", sizeBytes = 1))

        assertFalse(state.composeIsBlank())
    }

    @Test
    fun removeDiscardedDraftFromOpenThreadDropsCachedDraftAfterSend() {
        val state = testState()
        state.selectedCoreThread = threadSummary(id = "t1").copy(hasDraft = true)
        state.coreThreads = listOf(threadSummary(id = "t1").copy(hasDraft = true))
        state.locallyDraftedThreadIds = setOf("t1")
        state.messages =
            listOf(
                messageBody(id = "m1", folderId = "INBOX"),
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
                messageBody(id = "local-send-1", folderId = "INBOX"),
            )

        val cleared = state.removeDiscardedDraftFromOpenThread("draft-1")

        assertEquals(listOf("m1", "local-send-1"), state.messages.map { it.id })
        assertEquals("t1", cleared)
        assertFalse(state.selectedCoreThread!!.hasDraft)
        assertFalse(state.coreThreads.single().hasDraft)
        assertTrue(state.locallyDraftedThreadIds.isEmpty())
    }

    @Test
    fun removeDiscardedDraftFromOpenThreadClearsMarkerForAnAutosavedQuickReply() {
        val state = testState()
        state.selectedCoreThread = threadSummary(id = "t1").copy(hasDraft = true)
        state.coreThreads = listOf(threadSummary(id = "t1").copy(hasDraft = true))
        state.locallyDraftedThreadIds = setOf("t1")
        // A quick-reply autosave marks the thread without adding a draft message.
        state.messages = listOf(messageBody(id = "m1", folderId = "INBOX"))

        val cleared = state.removeDiscardedDraftFromOpenThread("draft-1", "t1")

        assertEquals("t1", cleared)
        assertEquals(listOf("m1"), state.messages.map { it.id })
        assertFalse(state.selectedCoreThread!!.hasDraft)
        assertFalse(state.coreThreads.single().hasDraft)
        assertTrue(state.locallyDraftedThreadIds.isEmpty())
    }

    @Test
    fun removeDiscardedDraftFromOpenThreadKeepsMarkerWhenAnotherDraftRemains() {
        val state = testState()
        state.selectedCoreThread = threadSummary(id = "t1").copy(hasDraft = true)
        state.locallyDraftedThreadIds = setOf("t1")
        state.messages =
            listOf(
                messageBody(id = "d0", folderId = "Drafts", messageId = "draft-0"),
                messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"),
            )

        val cleared = state.removeDiscardedDraftFromOpenThread("draft-1", "t1")

        assertNull(cleared)
        assertEquals(listOf("d0"), state.messages.map { it.id })
        assertTrue(state.selectedCoreThread!!.hasDraft)
        assertEquals(setOf("t1"), state.locallyDraftedThreadIds)
    }

    @Test
    fun removeDiscardedDraftFromOpenThreadKeepsMarkerWhileMessagesAreStillLoading() {
        val state = testState()
        state.selectedCoreThread = threadSummary(id = "t1").copy(hasDraft = true)
        state.coreThreads = listOf(threadSummary(id = "t1").copy(hasDraft = true))
        state.locallyDraftedThreadIds = setOf("t1")
        state.messages = emptyList()

        val cleared = state.removeDiscardedDraftFromOpenThread("draft-1")

        assertNull(cleared)
        assertTrue(state.selectedCoreThread!!.hasDraft)
        assertTrue(state.coreThreads.single().hasDraft)
        assertEquals(setOf("t1"), state.locallyDraftedThreadIds)
    }

    @Test
    fun removeDiscardedDraftFromOpenThreadLeavesAnotherThreadsDraftAlone() {
        val state = testState()
        state.selectedCoreThread = threadSummary(id = "t1").copy(hasDraft = true)
        state.coreThreads = listOf(threadSummary(id = "t1").copy(hasDraft = true))
        state.locallyDraftedThreadIds = setOf("t1")
        state.messages = listOf(messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1"))

        val cleared = state.removeDiscardedDraftFromOpenThread("draft-elsewhere")

        assertNull(cleared)
        assertEquals(listOf("d1"), state.messages.map { it.id })
        assertTrue(state.selectedCoreThread!!.hasDraft)
        assertEquals(setOf("t1"), state.locallyDraftedThreadIds)
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

    @Test
    fun quickReplyIdentitiesHiddenWithoutAliases() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount(aliases = emptyList()))
        state.selectedCoreThread = threadSummary(id = "t1")

        assertEquals(emptyList(), state.quickReplyIdentities())
        assertNull(state.selectedQuickReplyIdentity())
    }

    @Test
    fun quickReplyPreselectsTheAliasTheOriginalWasDeliveredTo() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount())
        state.selectedCoreThread = threadSummary(id = "t1")
        state.messages = listOf(messageBody(id = "m1", folderId = "INBOX", to = "sales@example.com"))

        assertEquals(
            listOf("me@example.com", "sales@example.com"),
            state.quickReplyIdentities().map { it.email },
        )
        assertEquals("sales@example.com", state.selectedQuickReplyIdentity()?.email)
    }

    @Test
    fun quickReplyContinuesWithTheAliasUsedByTheNewestOutgoingReply() {
        val state = testState()
        state.coreAccounts =
            listOf(
                aliasAccount(
                    aliases =
                        listOf(
                            AccountAlias(email = "sales@example.com", name = "Sales"),
                            AccountAlias(email = "support@example.com", name = "Support"),
                        ),
                ),
            )
        state.selectedCoreThread = threadSummary(id = "t1")
        val inbound = messageBody(id = "m1", folderId = "INBOX", to = "sales@example.com")
        val outgoing =
            messageBody(
                id = "m2",
                folderId = "Sent",
                to = "sender@example.com",
                fromAddr = "support@example.com",
                outgoing = true,
            )
        state.messages = listOf(inbound, outgoing)

        assertEquals(inbound, state.quickReplyParent())
        assertEquals("support@example.com", state.selectedQuickReplyIdentity()?.email)
        assertEquals("support@example.com", state.resolveQuickReplyFrom(inbound, state.coreAccounts.first()))
    }

    @Test
    fun quickReplyIgnoresInboundMailFromASharedAliasAddress() {
        val state = testState()
        state.coreAccounts =
            listOf(
                aliasAccount(
                    aliases =
                        listOf(
                            AccountAlias(email = "sales@example.com", name = "Sales"),
                            AccountAlias(email = "support@example.com", name = "Support"),
                        ),
                ),
            )
        state.selectedCoreThread = threadSummary(id = "t1")
        val inbound = messageBody(id = "m1", folderId = "INBOX", to = "sales@example.com")
        // A colleague sending from the shared support address. The core flags it
        // outgoing because its From matches a configured identity, but it was
        // delivered to our inbox, so it is not mail we sent.
        val colleague =
            messageBody(
                id = "m2",
                folderId = "INBOX",
                to = "sales@example.com",
                fromAddr = "support@example.com",
                outgoing = true,
            )
        state.messages = listOf(inbound, colleague)

        // The colleague's message is still the one we reply to, and the From
        // falls through to the alias the thread was delivered to.
        assertEquals(colleague, state.quickReplyParent())
        assertEquals("sales@example.com", state.resolveQuickReplyFrom(inbound, state.coreAccounts.first()))
    }

    @Test
    fun quickReplyOverrideWinsOverTheDetectedAlias() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount())
        state.selectedCoreThread = threadSummary(id = "t1")
        val parent = messageBody(id = "m1", folderId = "INBOX", to = "sales@example.com")
        state.messages = listOf(parent)
        state.quickReplyFrom = "me@example.com"

        assertEquals("me@example.com", state.selectedQuickReplyIdentity()?.email)
        // The primary normalizes back to blank, which the send path reads as the default.
        assertEquals("", state.resolveQuickReplyFrom(parent, state.coreAccounts.first()))
    }

    @Test
    fun quickReplyOverrideSendsFromTheChosenAlias() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount())
        state.selectedCoreThread = threadSummary(id = "t1")
        val parent = messageBody(id = "m1", folderId = "INBOX")
        state.messages = listOf(parent)
        state.quickReplyFrom = "sales@example.com"

        assertEquals("sales@example.com", state.resolveQuickReplyFrom(parent, state.coreAccounts.first()))
        assertEquals("sales@example.com", state.selectedQuickReplyIdentity()?.email)
    }

    @Test
    fun hydratedQuickReplyRestoresTheDraftFromIdentity() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount())
        state.selectedCoreThread = threadSummary(id = "t1")
        state.quickReplyThreadId = "t1"
        val parent = messageBody(id = "m1", folderId = "INBOX", to = "sales@example.com")
        val draft =
            messageBody(
                id = "d1",
                folderId = "Drafts",
                messageId = "draft-1",
                fromAddr = "me@example.com",
            )

        state.hydrateQuickReplyFromTailDraft("t1", listOf(parent, draft))

        assertEquals("me@example.com", state.quickReplyFrom)
        assertEquals("me@example.com", state.selectedQuickReplyIdentity()?.email)
    }

    private fun aliasAccount(aliases: List<AccountAlias> = listOf(AccountAlias(email = "sales@example.com", name = "Sales"))): AccountSummary =
        AccountSummary(
            id = "acc",
            email = "me@example.com",
            senderName = "Me",
            aliases = aliases,
        )

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

    @Test
    fun quickReplyDoesNotTakeADraftRowWithoutAMessageId() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount())
        state.selectedCoreThread = threadSummary(id = "t1")
        state.quickReplyThreadId = "t1"
        val parent = messageBody(id = "m1", folderId = "INBOX")
        // Synced from its envelope: no Message-ID, so nothing on the server this
        // bar could later save over or discard.
        val envelopeOnly = messageBody(id = "d1", folderId = "Drafts", fromAddr = "me@example.com")

        state.hydrateQuickReplyFromTailDraft("t1", listOf(parent, envelopeOnly))

        assertEquals("", state.quickReplyDraftId)
        assertFalse(state.quickReplyDraftSaved)

        // Once the read fills the header in, it hydrates as normal.
        state.hydrateQuickReplyFromTailDraft("t1", listOf(parent, envelopeOnly.copy(messageId = "now-known@example.com")))

        assertEquals("now-known@example.com", state.quickReplyDraftId)
        assertTrue(state.quickReplyDraftSaved)
    }

    @Test
    fun quickReplyHydrationLeavesAReplyInProgressAlone() {
        val state = testState()
        state.coreAccounts = listOf(aliasAccount())
        state.selectedCoreThread = threadSummary(id = "t1")
        state.quickReplyThreadId = "t1"
        val parent = messageBody(id = "m1", folderId = "INBOX")
        val draft = messageBody(id = "d1", folderId = "Drafts", messageId = "draft-1", fromAddr = "me@example.com")
        // The bar is visible as soon as the thread opens, so the user can be
        // typing before the read that carries this draft comes back.
        state.quickReplyBody = "A reply typed while the read was out"

        state.hydrateQuickReplyFromTailDraft("t1", listOf(parent, draft))

        assertEquals("A reply typed while the read was out", state.quickReplyBody)
        assertEquals("", state.quickReplyDraftId)
    }

    private fun messageBody(
        id: String,
        folderId: String,
        inReplyTo: String = "",
        references: String = "",
        messageId: String = "",
        to: String = "me@example.com",
        fromAddr: String = "",
        outgoing: Boolean = false,
    ): MessageBody =
        MessageBody(
            id = id,
            folderId = folderId,
            from = "sender@example.com",
            to = to,
            subject = "Subject",
            body = "Body",
            inReplyTo = inReplyTo,
            references = references,
            messageId = messageId,
            fromAddr = fromAddr,
            outgoing = outgoing,
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
            settingsMirror = SettingsMirror(FakeCore(), FakePreferences()) { true },
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
        override fun systemLanguageTag(): String = ""

        override fun applySystem(tag: String) {}

        override fun deviceLanguageTag(): String = "en-US"

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
