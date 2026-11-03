package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SignatureSpec
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** The signature a compose body carries as accounts and entry points change. */
class ComposeSignatureTest {
    @Test
    fun composingFromContactsReturnsToContacts() {
        val state = composeState()
        state.screen = Screen.Contacts
        state.openComposeTo("Contact <contact@example.com>", "b", returnScreen = Screen.Contacts)
        assertEquals(Screen.Compose, state.screen)
        assertEquals(Screen.Contacts, state.composeReturnScreen)
        assertEquals("Contact <contact@example.com>", state.to)
        assertEquals("\n\nFrom B", state.body)
    }

    @Test
    fun newComposeCarriesTheSendingAccountsSignature() {
        val state = composeState()

        state.openCompose()

        assertEquals("\n\nFrom A", state.body)
    }

    @Test
    fun changingFromSwapsTheSignatureForTheNewAccounts() {
        val state = composeState()
        state.openCompose()
        state.body = "Hello${state.body}"

        state.changeComposeIdentity("b", "b@example.com")

        assertEquals("Hello\n\nFrom B", state.body)
        assertEquals("b", state.composeFromAccountId)
    }

    @Test
    fun changingToAnAccountWithoutOneDropsTheSignature() {
        val state = composeState()
        state.openCompose()
        state.body = "Hello${state.body}"

        state.changeComposeIdentity("c", "c@example.com")

        assertEquals("Hello", state.body)
    }

    @Test
    fun anEditedSignatureIsLeftAloneOnAChangeOfFrom() {
        val state = composeState()
        state.openCompose()
        state.body = "Hello\n\nFrom A, but mine now"

        state.changeComposeIdentity("b", "b@example.com")

        assertEquals("Hello\n\nFrom A, but mine now", state.body)
    }

    @Test
    fun mailtoAndComposeToBothGetTheSignature() {
        val state = composeState()

        state.openMailtoCompose(ComposeDraft(to = "x@example.com", subject = "Hi", body = "Sent from a link"))
        assertEquals("Sent from a link\n\nFrom A", state.body)
        assertEquals("x@example.com", state.to)

        state.openComposeTo("y@example.com", "b")
        assertEquals("\n\nFrom B", state.body)
        assertEquals("y@example.com", state.to)
    }

    @Test
    fun aFreshComposeKeepsNothingFromTheReplyBeforeIt() {
        val state = composeState()
        // What escalating a quick reply leaves behind.
        state.composeFromAccountId = "b"
        state.composeFromEmail = "b@example.com"
        state.composeInReplyTo = "<parent@example.com>"
        state.composeReferences = "<root@example.com> <parent@example.com>"

        state.openCompose()

        assertEquals("", state.composeFromAccountId)
        assertEquals("", state.composeFromEmail)
        assertEquals("", state.composeInReplyTo)
        assertEquals("", state.composeReferences)
        // Seeded for the account it will actually send from.
        assertEquals("\n\nFrom A", state.body)
    }

    @Test
    fun aBodyTheAppDidNotComposeNeverCollectsASecondSignature() {
        val state = composeState()
        state.openCompose()
        // "Edit as new" and saved drafts replace the body wholesale; they mark
        // the draft unmanaged because it may already end in a signature.
        state.body = "Hello\n\nFrom A"
        state.composeSignature = null

        state.changeComposeIdentity("b", "b@example.com")

        assertEquals("Hello\n\nFrom A", state.body)
    }

    @Test
    fun anAccountWithoutOneStillPicksUpTheNextAccountsSignature() {
        val state = composeState()
        state.selectedCoreAccountId = "c"
        state.openCompose()
        assertEquals("", state.body)

        state.changeComposeIdentity("b", "b@example.com")

        assertEquals("\n\nFrom B", state.body)
    }

    @Test
    fun forwardingKeepsNothingFromTheReplyBeforeIt() {
        val state = composeState()
        state.composeInReplyTo = "<parent@example.com>"
        state.composeReferences = "<root@example.com> <parent@example.com>"

        // Only the state reset matters here; the message fetch needs a core.
        state.clearComposeDraftState()

        assertEquals("", state.composeInReplyTo)
        assertEquals("", state.composeReferences)
        assertNull(state.composeSignature)
    }

    @Test
    fun addingASignatureLeavesTheUsersOwnWhitespaceAlone() {
        val state = composeState()
        state.selectedCoreAccountId = "c"
        state.openCompose()
        state.body = "  indented start\n\nand a trailing space \n"

        state.changeComposeIdentity("a", "a@example.com")

        assertEquals("  indented start\n\nand a trailing space \n\nFrom A", state.body)
    }

    @Test
    fun aRestoreInvalidatesTheSignatureBeforeReloading() {
        val state = composeState()
        state.appSignatureLoaded = true

        // Restore invalidates existing reads before its exact reload jobs start,
        // so a compose cannot seed against the pre-restore signature.
        state.invalidateBackupReloads()

        assertFalse(state.appSignatureLoaded)
    }

    @Test
    fun staleSignatureWaiterCannotOverwriteTheNewestCompose() =
        runBlocking {
            val state = composeState(this)
            state.appSignatureLoaded = false

            state.openMailtoCompose(ComposeDraft(to = "old@example.com"))
            state.openMailtoCompose(ComposeDraft(to = "new@example.com"))
            state.appSignatureLoaded = true
            state.appSignatureLoadCompletion.complete(Unit)
            yield()

            assertEquals("new@example.com", state.to)
        }

    @Test
    fun incomingMailtoIsConsumableAndAcceptsAnEqualEventAgain() {
        val events = IncomingMailtoEvents()
        val draft = ComposeDraft(to = "same@example.com")

        events.offer(draft)
        assertEquals(draft, events.draft)
        events.consume()
        assertNull(events.draft)
        events.offer(draft)

        assertEquals(draft, events.draft)
    }

    @Test
    fun accountSwitchIsImmediateWhileSignatureReconciliationIsPending() {
        val state = composeState()
        state.openCompose()
        state.appSignatureLoaded = false

        state.changeComposeIdentity("b", "b@example.com")

        assertEquals("b", state.composeFromAccountId)
        assertEquals("b@example.com", state.composeFromEmail)
        assertEquals("\n\nFrom A", state.body)
        assertEquals(true, state.composeSignaturePending)
    }

    @Test
    fun theReplyBarIsSeededWithTheSendingAccountsSignature() {
        val state = quickReplyState()

        state.seedQuickReplySignature()

        assertEquals("\n\nFrom A", state.quickReplyBody)
        assertEquals("From A", state.quickReplySignature?.text)
    }

    @Test
    fun anAccountThatSendsNoneLeavesTheReplyBarEmpty() {
        val state = quickReplyState(accountId = "c")

        state.seedQuickReplySignature()

        assertEquals("", state.quickReplyBody)
        assertNull(state.quickReplySignature)
    }

    @Test
    fun aReplyBarHoldingOnlyItsSignatureCountsAsBlank() {
        val state = quickReplyState()

        state.seedQuickReplySignature()
        assertTrue(state.quickReplyIsBlank())

        state.quickReplyBody = "t2\n\nFrom A"
        assertFalse(state.quickReplyIsBlank())
    }

    @Test
    fun escalatingHandsTheComposerAStrippedBodyToSeedItsOwn() {
        val state = quickReplyState()
        state.seedQuickReplySignature()
        state.quickReplyBody = "t2\n\nFrom A"

        state.openQuickReplyInFullEditor()

        // Same text, but inserted and tracked by the composer, so a later change
        // of identity can swap it.
        assertEquals("t2\n\nFrom A", state.body)
        assertEquals("From A", state.composeSignature?.text)
        // And the bar behind the composer is a fresh quick reply, not an empty one.
        assertEquals("\n\nFrom A", state.quickReplyBody)
    }

    @Test
    fun escalatingAnEditedSignatureDoesNotProduceASecond() {
        val state = quickReplyState()
        state.seedQuickReplySignature()
        state.quickReplyBody = "t2\n\nFrom A, but mine now"

        state.openQuickReplyInFullEditor()

        assertEquals("t2\n\nFrom A, but mine now", state.body)
        assertNull(state.composeSignature)
    }

    @Test
    fun aReplyBarSeededBeforeTheAppSignatureLandsIsSeededAgain() =
        runBlocking {
            val state = quickReplyState(this, accountId = "d")
            state.appSignatureLoaded = false

            state.seedQuickReplySignature()
            assertEquals("", state.quickReplyBody)

            state.appSignatureHtml = "<p>App wide</p>"
            state.appSignatureLoaded = true
            state.appSignatureLoadCompletion.complete(Unit)
            yield()

            assertEquals("\n\nApp wide", state.quickReplyBody)
        }

    @Test
    fun aLateAppSignatureDoesNotOverwriteWhatTheUserTyped() =
        runBlocking {
            val state = quickReplyState(this, accountId = "d")
            state.appSignatureLoaded = false
            state.seedQuickReplySignature()
            state.quickReplyBody = "already typing"

            state.appSignatureHtml = "<p>App wide</p>"
            state.appSignatureLoaded = true
            state.appSignatureLoadCompletion.complete(Unit)
            yield()

            assertEquals("already typing", state.quickReplyBody)
        }

    @Test
    fun rewritingTheAppSignatureUpdatesAnUntouchedReplyBar() {
        // Account "d" follows the app-wide signature rather than its own.
        val state = quickReplyState(accountId = "d")
        state.appSignatureHtml = "<p>App wide</p>"
        state.seedQuickReplySignature()
        assertEquals("\n\nApp wide", state.quickReplyBody)

        state.appSignatureHtml = "<p>Rewritten</p>"
        state.reseedUntouchedQuickReply()

        assertEquals("\n\nRewritten", state.quickReplyBody)
    }

    @Test
    fun rewritingAnAccountSignatureUpdatesAnUntouchedReplyBar() {
        val state = quickReplyState()
        state.seedQuickReplySignature()
        assertEquals("\n\nFrom A", state.quickReplyBody)

        state.coreAccounts =
            state.coreAccounts.map {
                if (it.id == "a") it.copy(signature = SignatureSpec("custom", "<p>From A, revised</p>")) else it
            }
        state.reseedUntouchedQuickReply()

        assertEquals("\n\nFrom A, revised", state.quickReplyBody)
    }

    @Test
    fun rewritingASignatureLeavesAReplyBarInProgressAlone() {
        val state = quickReplyState()
        state.seedQuickReplySignature()
        state.quickReplyBody = "half typed\n\nFrom A"

        state.appSignatureHtml = "<p>Rewritten</p>"
        state.reseedUntouchedQuickReply()

        assertEquals("half typed\n\nFrom A", state.quickReplyBody)
    }

    @Test
    fun rewritingASignatureLeavesAHydratedDraftAlone() {
        val state = quickReplyState()
        // What hydrateQuickReplyFromTailDraft leaves behind: an unmanaged body
        // that already carries whatever signature it was written with.
        state.quickReplyBody = "saved draft body"
        state.quickReplySignature = null
        state.quickReplyDraftId = "draft-1"
        state.quickReplyDraftSaved = true

        state.appSignatureHtml = "<p>Rewritten</p>"
        state.reseedUntouchedQuickReply()

        assertEquals("saved draft body", state.quickReplyBody)
    }

    @Test
    fun aQuotedReplyFoldsTheQuoteAwayUnderTheSignature() =
        runBlocking {
            val state = quickReplyState(this)
            state.quoteReplies = true

            state.replyToMessage(state.messages.single(), replyAll = false)
            yield()

            assertEquals("\n\nFrom A", state.body)
            assertEquals("\n\n${attribution()}\n> Original message", state.composeFoldedQuote?.text)
            assertTrue(state.composeIsBlank())

            // Unfolded, it joins the body under the signature, and is still not
            // the user's writing.
            state.expandComposeQuote()
            assertEquals("\n\nFrom A\n\n${attribution()}\n> Original message", state.body)
            assertNull(state.composeFoldedQuote)
            assertEquals(attribution(), state.composeQuoteMarker)
            assertTrue(state.composeIsBlank())
            state.body = "Sounds good${state.body}"
            assertFalse(state.composeIsBlank())
        }

    @Test
    fun removingAFoldedQuoteDropsIt() =
        runBlocking {
            val state = quickReplyState(this)
            state.quoteReplies = true
            state.replyToMessage(state.messages.single(), replyAll = false)
            yield()

            state.removeComposeQuote()

            assertNull(state.composeFoldedQuote)
            assertEquals("\n\nFrom A", state.body)
        }

    @Test
    fun anUnquotedReplyStaysEmpty() {
        val state = quickReplyState()

        state.replyToMessage(state.messages.single(), replyAll = false)

        assertEquals("\n\nFrom A", state.body)
    }

    @Test
    fun escalatingAQuotedReplyQuotesUnderTheTextAndSignature() =
        runBlocking {
            val state = quickReplyState(this)
            state.quoteReplies = true
            state.seedQuickReplySignature()
            state.quickReplyBody = "t2\n\nFrom A"

            state.openQuickReplyInFullEditor()
            yield()

            assertEquals("t2\n\nFrom A", state.body)
            assertEquals("\n\n${attribution()}\n> Original message", state.composeFoldedQuote?.text)
            assertEquals("From A", state.composeSignature?.text)
        }

    @Test
    fun escalatingABarThatAlreadyHoldsItsQuoteDoesNotQuoteAgain() =
        runBlocking {
            val state = quickReplyState(this)
            state.quoteReplies = true
            // A saved quoted reply, hydrated into the bar.
            state.quickReplyBody = "t2\n\nFrom A\n\n${attribution()}\n> Original message"
            state.quickReplySignature = null

            state.openQuickReplyInFullEditor()
            yield()

            assertEquals("t2\n\nFrom A\n\n${attribution()}\n> Original message", state.body)
            assertNull(state.composeSignature)
            assertNull(state.composeFoldedQuote)
        }

    @Test
    fun escalatingKeepsWhatWasTypedWhileTheQuoteLoaded() =
        runBlocking {
            val state = quickReplyState(this)
            state.quoteReplies = true
            state.seedQuickReplySignature()
            state.quickReplyBody = "t2\n\nFrom A"

            state.openQuickReplyInFullEditor()
            // Still loading the quote: the bar takes another keystroke.
            state.quickReplyBody = "t2 and more\n\nFrom A"
            yield()

            assertEquals("t2 and more\n\nFrom A", state.body)
        }

    private fun attribution() = "On ${formatMessageFullTimestamp(0)}, Them <them@example.com> wrote:"

    private fun quickReplyState(
        scope: CoroutineScope = CoroutineScope(EmptyCoroutineContext),
        accountId: String = "a",
    ): MeronMobileState {
        val state = composeState(scope)
        state.coreAccounts = state.coreAccounts + account("d", SignatureSpec("global", ""))
        val thread =
            ThreadSummary(
                id = "t-1",
                accountId = accountId,
                folder = "INBOX",
                subject = "Subject",
                sender = "them@example.com",
            )
        state.selectedCoreThread = thread
        state.quickReplyThreadId = thread.backendThreadId()
        state.messages =
            listOf(
                MessageBody(
                    id = "root",
                    folderId = "INBOX",
                    from = "Them",
                    fromAddr = "them@example.com",
                    to = "$accountId@example.com",
                    subject = "Subject",
                    body = "Original message",
                    messageId = "root@example.com",
                ),
            )
        return state
    }

    private fun composeState(scope: CoroutineScope = CoroutineScope(EmptyCoroutineContext)): MeronMobileState {
        val state = testState(scope)
        state.coreAccounts =
            listOf(
                account("a", SignatureSpec("custom", "<p>From A</p>")),
                account("b", SignatureSpec("custom", "<p>From B</p>")),
                account("c", SignatureSpec("none", "<p>Unused</p>")),
            )
        state.selectedCoreAccountId = "a"
        state.appSignatureLoaded = true
        return state
    }

    private fun account(
        id: String,
        signature: SignatureSpec,
    ) = AccountSummary(id = id, email = "$id@example.com", signature = signature)

    private fun testState(scope: CoroutineScope): MeronMobileState =
        MeronMobileState(
            scope = scope,
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
