package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.ProxySpec
import jp.nonbili.meron.shared.SignatureSpec
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ComposeSaveLifecycleTest {
    @Test
    fun fullComposeSavesAreSerialized() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "you@example.com"
            state.subject = "First"
            state.body = "First body${state.body}"

            state.autoSaveComposeDraft()
            core.firstSaveStarted.await()
            state.subject = "Second"
            state.autoSaveComposeDraft()
            yield()

            assertEquals(1, core.saveCalls)
            core.releaseFirstSave.complete(Unit)
            core.secondSaveFinished.await()

            assertEquals(2, core.saveCalls)
            assertTrue(core.savedPayloads[0].contains("\"subject\":\"First\""))
            assertTrue(core.savedPayloads[1].contains("\"subject\":\"Second\""))
            assertEquals("draft-1@example.com", state.composeDraftId)
        }

    @Test
    fun obsoleteSaveDiscardsItsAllocatedRemoteDraft() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "old@example.com"
            state.subject = "Old"
            state.body = "Old body${state.body}"

            state.autoSaveComposeDraft()
            core.firstSaveStarted.await()
            state.openCompose()
            core.releaseFirstSave.complete(Unit)
            core.discardFinished.await()

            assertTrue(core.discardPayloads.single().contains("draft-1@example.com"))
            assertEquals("", state.composeDraftId)
            assertEquals("", state.to)
        }

    @Test
    fun closingSaveSurvivesOpeningAnotherComposer() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "saved@example.com"
            state.subject = "Keep me"
            state.body = "Closing body${state.body}"

            state.closeCompose()
            core.firstSaveStarted.await()
            state.openCompose()
            core.releaseFirstSave.complete(Unit)
            core.firstSaveFinished.await()
            yield()

            assertEquals(1, core.saveCalls)
            assertTrue(core.savedPayloads.single().contains("\"subject\":\"Keep me\""))
            assertTrue(core.discardPayloads.isEmpty())
            assertEquals("", state.to)
        }

    @Test
    fun saveResultCannotCrossAnAccountSwitch() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "old@example.com"
            state.subject = "Old account"
            state.body = "Old body${state.body}"

            state.autoSaveComposeDraft()
            core.firstSaveStarted.await()
            state.changeComposeIdentity("b", "b@example.com")
            core.releaseFirstSave.complete(Unit)
            core.discardFinished.await()

            assertEquals("b", state.composeFromAccountId)
            assertEquals("", state.composeDraftId)
            assertEquals("", state.composeDraftAccountId)
            assertTrue(core.discardPayloads.single().contains("draft-1@example.com"))
        }

    @Test
    fun existingIdAutosaveCannotResurrectAfterSend() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "you@example.com"
            state.subject = "Send"
            state.body = "Send body${state.body}"
            state.composeDraftId = "existing-a@example.com"
            state.composeDraftSaved = true
            state.composeDraftAccountId = "a"

            state.autoSaveComposeDraft()
            core.firstSaveStarted.await()
            state.sendMail()
            yield()
            assertEquals(0, core.sendCalls)
            core.releaseFirstSave.complete(Unit)
            core.sendFinished.await()
            core.discardFinished.await()
            awaitState { !state.composeSendInFlight && state.composeDraftId.isEmpty() }

            assertEquals("", state.composeDraftId)
            assertEquals(false, state.composeDraftSaved)
            assertEquals(1, core.discardPayloads.size)
            assertTrue(core.discardPayloads.single().contains("existing-a@example.com"))
        }

    @Test
    fun failedPostSendDraftDiscardRemainsQueuedForCleanup() =
        runBlocking {
            val core = SaveCore().apply { discardFails = true }
            val state = state(core, this)
            state.openCompose()
            state.to = "you@example.com"
            state.subject = "Send"
            state.body = "Send body${state.body}"
            state.composeDraftId = "existing-a@example.com"
            state.composeDraftSaved = true
            state.composeDraftAccountId = "a"

            state.sendMail()
            core.sendFinished.await()
            core.discardFinished.await()
            withTimeout(1_000) {
                while (state.composeSendInFlight) yield()
            }

            assertEquals(
                listOf(ComposeDraftOwner("a", "existing-a@example.com", "")),
                state.composeDraftCleanupOwners,
            )
            assertEquals("", state.composeDraftId)
        }

    @Test
    fun completedQuickReplySaveSurvivesNavigationWithoutMutatingTheNewThread() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.selectedCoreThread =
                ThreadSummary(id = "old", accountId = "a", folder = "INBOX", subject = "Old", sender = "old@example.com")
            state.quickReplyThreadId = "old"
            state.messages =
                listOf(
                    MessageBody(
                        id = "old-message",
                        folderId = "INBOX",
                        from = "Old",
                        fromAddr = "old@example.com",
                        to = "a@example.com",
                        subject = "Old",
                        body = "Original",
                        messageId = "old@example.com",
                    ),
                )
            state.quickReplyBody = "Saved reply"
            state.quickReplyDraftId = "existing-reply@example.com"
            state.quickReplyDraftSaved = true

            state.autoSaveQuickReplyDraft()
            core.firstSaveStarted.await()
            state.selectedCoreThread =
                ThreadSummary(id = "new", accountId = "a", folder = "INBOX", subject = "New", sender = "new@example.com")
            state.quickReplyThreadId = "new"
            state.quickReplyBody = "New thread reply"
            state.quickReplyDraftId = "new-reply@example.com"
            ++state.quickReplyGeneration
            core.releaseFirstSave.complete(Unit)
            core.firstSaveFinished.await()
            yield()

            assertTrue(core.discardPayloads.isEmpty())
            assertEquals("new", state.quickReplyThreadId)
            assertEquals("New thread reply", state.quickReplyBody)
            assertEquals("new-reply@example.com", state.quickReplyDraftId)
        }

    @Test
    fun obsoleteQuickReplySavePublishesItsNewRemoteIdForTheSameEditor() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            prepareQuickReply(state)
            state.onQuickReplyBodyChange("First version")

            state.autoSaveQuickReplyDraft()
            core.firstSaveStarted.await()
            state.onQuickReplyBodyChange("Second version")
            core.releaseFirstSave.complete(Unit)
            core.firstSaveFinished.await()
            withTimeout(1_000) {
                while (state.quickReplyDraftId.isBlank()) yield()
            }
            assertEquals("draft-1@example.com", state.quickReplyDraftId)

            state.autoSaveQuickReplyDraft()
            core.secondSaveFinished.await()

            assertEquals(1, core.allocationCalls)
            assertTrue(core.savedPayloads[1].contains("\"draft_id\":\"draft-1@example.com\""))
            assertTrue(core.discardPayloads.isEmpty())
        }

    @Test
    fun quickReplySendWaitingForSaveDiscardsTheNewlyAllocatedDraft() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            prepareQuickReply(state)
            state.onQuickReplyBodyChange("Reply")

            state.autoSaveQuickReplyDraft()
            core.firstSaveStarted.await()
            state.sendQuickReply()
            yield()
            assertEquals(0, core.sendCalls)
            core.releaseFirstSave.complete(Unit)
            core.sendFinished.await()
            core.discardFinished.await()

            assertEquals(2, core.allocationCalls)
            assertTrue(core.discardPayloads.single().contains("draft-1@example.com"))
        }

    @Test
    fun reopeningTheThreadWhileTheSentDraftIsBeingDiscardedKeepsTheReplyBarClear() =
        runBlocking {
            val core = SaveCore().apply { holdDiscard = CompletableDeferred() }
            val state = state(core, this)
            prepareQuickReply(state)
            state.quickReplyDraftId = "reply-draft@example.com"
            state.quickReplyDraftSaved = true
            state.onQuickReplyBodyChange("Reply")

            state.sendQuickReply()
            core.sendFinished.await()
            core.discardFinished.await()

            // The user backs out to another conversation and comes straight
            // back. Reopening resets the reply bar, and the read still returns
            // the consumed draft — the discard has not landed yet — as the
            // conversation tail.
            state.quickReplyThreadId = "other"
            state.quickReplyBody = ""
            state.quickReplyDraftId = ""
            state.quickReplyDraftSaved = false
            state.quickReplyThreadId = "thread"
            state.hydrateQuickReplyFromTailDraft("thread", state.messages + consumedDraft())

            assertEquals("", state.quickReplyBody)
            assertEquals("", state.quickReplyDraftId)

            core.holdDiscard!!.complete(Unit)
            withTimeout(1_000) {
                while (state.quickReplySendInFlight) yield()
            }
            assertTrue(state.quickReplyConsumedDraftIds.isEmpty())
        }

    @Test
    fun leavingTheThreadDuringTheSendStillDiscardsTheDraftItConsumed() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            prepareQuickReply(state)
            state.quickReplyDraftId = "reply-draft@example.com"
            state.quickReplyDraftSaved = true
            state.onQuickReplyBodyChange("Reply")

            state.sendQuickReply()
            // The user opens another conversation while the send is out, which
            // resets the bar and moves the editor generation on.
            state.quickReplyThreadId = "other"
            state.quickReplyDraftId = ""
            state.quickReplyDraftSaved = false
            ++state.quickReplyGeneration
            withTimeout(5_000) {
                core.sendFinished.await()
                core.discardFinished.await()
            }

            // Nothing points at that draft any more; leaving it is what puts it
            // in Drafts next to the reply it was sent as.
            assertTrue(core.discardPayloads.single().contains("reply-draft@example.com"))
        }

    @Test
    fun aConsumedDraftStaysOutOfTheConversationAfterReopening() =
        runBlocking {
            val core = SaveCore().apply { holdDiscard = CompletableDeferred() }
            val state = state(core, this)
            prepareQuickReply(state)
            state.quickReplyDraftId = "reply-draft@example.com"
            state.quickReplyDraftSaved = true
            state.onQuickReplyBodyChange("Reply")

            state.sendQuickReply()
            withTimeout(5_000) {
                core.sendFinished.await()
                core.discardFinished.await()
            }

            // Reopening clears the bar's id, so only the send's own hold is left
            // to keep the draft it consumed out of the conversation.
            state.quickReplyThreadId = "other"
            state.quickReplyBody = ""
            state.quickReplyDraftId = ""
            state.quickReplyDraftSaved = false
            state.quickReplyThreadId = "thread"
            state.messages = state.messages + consumedDraft()

            assertTrue(state.visibleThreadMessages().none { it.id == "draft-row" })

            core.holdDiscard!!.complete(Unit)
            withTimeout(1_000) {
                while (state.quickReplySendInFlight) yield()
            }
            assertTrue(state.messages.none { it.id == "draft-row" })
        }

    @Test
    fun openingADraftWithoutAMessageIdDoesNotClaimItAsSaved() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            val thread =
                ThreadSummary(id = "thread", accountId = "a", folder = "Drafts", subject = "Subject", sender = "a@example.com")
            val draft =
                MessageBody(
                    id = "draft-row",
                    folderId = "Drafts",
                    from = "A",
                    fromAddr = "a@example.com",
                    to = "you@example.com",
                    subject = "Subject",
                    body = "Half written",
                    messageId = "",
                )

            state.openDraftCompose(draft, thread)
            withTimeout(5_000) { while (state.screen != Screen.Compose) yield() }

            // Nothing on the server answers to this id, so a discard would find
            // nothing and a save must create the draft rather than replace one.
            assertEquals(false, state.composeDraftSaved)
            assertTrue(state.composeDraftId.startsWith("local-draft-"))

            state.openDraftCompose(draft.copy(messageId = "real-draft@example.com"), thread)
            withTimeout(5_000) { while (state.composeDraftId != "real-draft@example.com") yield() }

            assertTrue(state.composeDraftSaved)
        }

    @Test
    fun aSendNeverAdoptsTheDraftOfAThreadOpenedWhileItWasOut() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            prepareQuickReply(state)
            state.onQuickReplyBodyChange("Reply")

            // Nothing saved yet, so the send has no draft of its own to claim.
            state.sendQuickReply()
            // Another conversation opens while the send is out and hydrates its
            // own saved draft into the bar.
            state.quickReplyThreadId = "other-thread"
            state.quickReplyDraftId = "someone-elses-draft@example.com"
            state.quickReplyDraftSaved = true
            ++state.quickReplyGeneration
            withTimeout(5_000) { core.sendFinished.await() }
            withTimeout(5_000) {
                while (state.quickReplySendInFlight) yield()
            }

            // That draft belongs to a thread this send never touched.
            assertTrue(core.discardPayloads.isEmpty())
            assertEquals("someone-elses-draft@example.com", state.quickReplyDraftId)
        }

    @Test
    fun aDraftRehydratedDuringTheSendIsStillDiscarded() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            prepareQuickReply(state)
            state.quickReplyDraftId = "reply-draft@example.com"
            state.quickReplyDraftSaved = true
            state.onQuickReplyBodyChange("Reply")

            state.sendQuickReply()
            // Leaving and reopening the conversation during the send: the read
            // still returns the consumed draft as the tail.
            state.quickReplyThreadId = "other"
            state.quickReplyBody = ""
            state.quickReplyDraftId = ""
            state.quickReplyDraftSaved = false
            state.quickReplyThreadId = "thread"
            state.hydrateQuickReplyFromTailDraft("thread", state.messages + consumedDraft())

            // The hold runs from the click, so the sent text cannot come back
            // into the bar and be mistaken for a newer reply.
            assertEquals("", state.quickReplyDraftId)
            withTimeout(5_000) {
                core.sendFinished.await()
                core.discardFinished.await()
            }
            assertTrue(core.discardPayloads.single().contains("reply-draft@example.com"))
        }

    @Test
    fun aHandedOverDraftIsHeldAndSettledBeforeTheIdentityAllocation() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            prepareQuickReply(state)
            state.onQuickReplyBodyChange("Reply")
            // An autosave from before the click landed while the bar had already
            // moved on, so it handed its id to the send rather than the bar.
            state.quickReplySendDraftHandover = "thread" to "handed-over@example.com"

            state.sendQuickReply()
            // Reopening the conversation while the send is still allocating must
            // not pull the text being sent back into the bar.
            state.hydrateQuickReplyFromTailDraft(
                "thread",
                state.messages + consumedDraft().copy(messageId = "handed-over@example.com"),
            )
            assertEquals("", state.quickReplyDraftId)

            withTimeout(5_000) {
                core.sendFinished.await()
                core.discardFinished.await()
            }
            assertTrue(core.discardPayloads.single().contains("handed-over@example.com"))
            assertEquals(null, state.quickReplySendDraftHandover)
        }

    @Test
    fun aFailedAllocationDoesNotLeaveAHandoverForTheNextSend() =
        runBlocking {
            val core = SaveCore().apply { allocationFails = true }
            val state = state(core, this)
            prepareQuickReply(state)
            state.onQuickReplyBodyChange("Reply")
            state.quickReplySendDraftHandover = "thread" to "handed-over@example.com"

            state.sendQuickReply()
            withTimeout(5_000) {
                while (state.quickReplySendInFlight) yield()
            }

            // Left behind, a later send in this thread would consume it and
            // delete the safety copy of a reply that never went out.
            assertEquals(null, state.quickReplySendDraftHandover)
            assertTrue(state.quickReplyConsumedDraftIds.isEmpty())
            assertTrue(core.discardPayloads.isEmpty())
        }

    @Test
    fun aFailedSendLeavesItsDraftReachableAgain() =
        runBlocking {
            val core = SaveCore().apply { sendFails = true }
            val state = state(core, this)
            prepareQuickReply(state)
            state.quickReplyDraftId = "reply-draft@example.com"
            state.quickReplyDraftSaved = true
            state.onQuickReplyBodyChange("Reply")

            state.sendQuickReply()
            core.sendFinished.await()
            withTimeout(1_000) {
                while (state.quickReplySendInFlight) yield()
            }

            assertTrue(state.quickReplyConsumedDraftIds.isEmpty())
            assertTrue(core.discardPayloads.isEmpty())

            state.quickReplyThreadId = "other"
            state.quickReplyBody = ""
            state.quickReplyDraftId = ""
            state.quickReplyDraftSaved = false
            state.quickReplyThreadId = "thread"
            state.hydrateQuickReplyFromTailDraft("thread", state.messages + consumedDraft())

            assertEquals("Reply", state.quickReplyBody)
            assertEquals("reply-draft@example.com", state.quickReplyDraftId)
        }

    @Test
    fun existingIdAutosaveCannotResurrectAfterDiscard() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "you@example.com"
            state.subject = "Discard"
            state.body = "Discard body${state.body}"
            state.composeDraftId = "existing-a@example.com"
            state.composeDraftSaved = true
            state.composeDraftAccountId = "a"

            state.autoSaveComposeDraft()
            core.firstSaveStarted.await()
            state.discardComposeDraft()
            yield()
            assertEquals(0, core.discardPayloads.size)
            core.releaseFirstSave.complete(Unit)
            core.discardFinished.await()
            awaitState { state.composeDraftId.isEmpty() && !state.composeDraftSaved }

            assertEquals("", state.composeDraftId)
            assertEquals(false, state.composeDraftSaved)
            assertEquals(1, core.discardPayloads.size)
            assertTrue(core.discardPayloads.single().contains("existing-a@example.com"))
        }

    @Test
    fun savedDraftSwitchingAccountsAllocatesBIdAndDiscardsExactAId() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "you@example.com"
            state.subject = "Move"
            state.body = "Move body${state.body}"

            state.autoSaveComposeDraft()
            core.firstSaveStarted.await()
            core.releaseFirstSave.complete(Unit)
            core.firstSaveFinished.await()
            withTimeout(1_000) {
                while (state.composeDraftId.isBlank()) yield()
            }
            assertEquals("draft-1@example.com", state.composeDraftId)

            state.changeComposeIdentity("b", "b@example.com")
            assertTrue(state.composeDraftId.startsWith("local-draft-"))
            assertEquals(false, state.composeDraftSaved)
            assertEquals("", state.composeDraftAccountId)
            state.autoSaveComposeDraft()
            core.secondSaveFinished.await()
            core.discardFinished.await()
            awaitState { state.composeDraftId == "draft-2@example.com" && state.composeDraftAccountId == "b" }

            assertEquals("draft-2@example.com", state.composeDraftId)
            assertEquals("b", state.composeDraftAccountId)
            assertTrue(core.savedPayloads[1].contains("\"account_id\":\"b\""))
            assertTrue(core.savedPayloads[1].contains("\"draft_id\":\"draft-2@example.com\""))
            assertEquals(1, core.discardPayloads.size)
            assertTrue(core.discardPayloads.single().contains("\"account_id\":\"a\""))
            assertTrue(core.discardPayloads.single().contains("draft-1@example.com"))
        }

    @Test
    fun signaturePendingBlocksSave() =
        runBlocking {
            val core = SaveCore()
            val state = state(core, this)
            state.openCompose()
            state.to = "you@example.com"
            state.subject = "Pending"
            state.body = "Body${state.body}"
            state.composeSignaturePending = true

            state.saveComposeDraft()
            yield()

            assertEquals(0, core.saveCalls)
            assertEquals("Waiting for signature before saving.", state.status)
        }

    @Test
    fun olderAccountAndProxyLoadsCannotApplyAfterNewerLoads() =
        runBlocking {
            val core = ReloadCore()
            val state = state(core, this)

            val oldAccounts = state.listAccounts()
            val oldProxy = state.loadAppProxy()
            core.oldAccountsStarted.await()
            core.oldProxyStarted.await()
            val newAccounts = state.listAccounts()
            val newProxy = state.loadAppProxy()
            core.newAccountsStarted.await()
            core.newProxyStarted.await()

            core.newAccounts.complete("""{"accounts":[{"id":"new","email":"new@example.com"}]}""")
            core.newProxy.complete("""{"proxy":{"mode":"http","host":"new.proxy","port":8080}}""")
            newAccounts?.join()
            newProxy?.join()
            core.oldAccounts.complete("""{"accounts":[{"id":"old","email":"old@example.com"}]}""")
            core.oldProxy.complete("""{"proxy":{"mode":"http","host":"old.proxy","port":8080}}""")
            oldAccounts?.join()
            oldProxy?.join()

            assertEquals("new", state.coreAccounts.single().id)
            assertEquals(ProxySpec("http", "new.proxy", 8080), state.appProxy)
        }

    /**
     * Wait for composer state the core call writes *after* it returns. The
     * `CompletableDeferred`s in [SaveCore] are completed inside the call, which
     * runs on [ioDispatcher], so awaiting one can resume this test before the
     * caller's `withContext` return is even queued on the `runBlocking` loop —
     * a single `yield()` is not a barrier across that.
     */
    private suspend fun awaitState(settled: () -> Boolean) {
        withTimeout(1_000) {
            while (!settled()) yield()
        }
    }

    private fun state(
        core: MeronCore,
        scope: CoroutineScope,
    ): MeronMobileState =
        MeronMobileState(
            scope = scope,
            core = core,
            coreLoaded = true,
            prefs = TestPreferences(),
            kanbanPrefs = TestPreferences(),
            services = TestPlatformServices(),
            locale = TestLocaleController(),
            mobileHost = DefaultMobileHost(),
            settingsMirror = SettingsMirror(core, TestPreferences()) { true },
        ).apply {
            coreAccounts =
                listOf(
                    AccountSummary(
                        id = "a",
                        email = "a@example.com",
                        signature = SignatureSpec("custom", "<p>From A</p>"),
                    ),
                    AccountSummary(
                        id = "b",
                        email = "b@example.com",
                        signature = SignatureSpec("custom", "<p>From B</p>"),
                    ),
                )
            selectedCoreAccountId = "a"
            appSignatureLoaded = true
        }

    /** The server copy of the quick reply just sent, still sitting in Drafts. */
    private fun consumedDraft(): MessageBody =
        MessageBody(
            id = "draft-row",
            folderId = "Drafts",
            from = "A",
            fromAddr = "a@example.com",
            to = "sender@example.com",
            subject = "Re: Subject",
            body = "Reply",
            messageId = "reply-draft@example.com",
        )

    private fun prepareQuickReply(state: MeronMobileState) {
        state.selectedCoreThread =
            ThreadSummary(id = "thread", accountId = "a", folder = "INBOX", subject = "Subject", sender = "sender@example.com")
        state.quickReplyThreadId = "thread"
        state.messages =
            listOf(
                MessageBody(
                    id = "message",
                    folderId = "INBOX",
                    from = "Sender",
                    fromAddr = "sender@example.com",
                    to = "a@example.com",
                    subject = "Subject",
                    body = "Original",
                    messageId = "original@example.com",
                ),
            )
    }

    private class SaveCore : MeronCore {
        val firstSaveStarted = CompletableDeferred<Unit>()
        val firstSaveFinished = CompletableDeferred<Unit>()
        val releaseFirstSave = CompletableDeferred<Unit>()
        val secondSaveFinished = CompletableDeferred<Unit>()
        val sendFinished = CompletableDeferred<Unit>()
        val discardFinished = CompletableDeferred<Unit>()
        val savedPayloads = mutableListOf<String>()
        val discardPayloads = mutableListOf<String>()
        var saveCalls = 0
        var sendCalls = 0
        var discardFails = false
        var allocationFails = false
        var sendFails = false
        var holdDiscard: CompletableDeferred<Unit>? = null
        var allocationCalls = 0

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String =
            when (command) {
                MobileCommand.AllocateIdentity -> {
                    allocationCalls++
                    if (allocationFails) throw RuntimeException("allocation failed")
                    """{"message_id":"draft-$allocationCalls@example.com"}"""
                }

                MobileCommand.SaveDraft -> {
                    savedPayloads += payloadJson
                    saveCalls++
                    if (saveCalls == 1) {
                        firstSaveStarted.complete(Unit)
                        releaseFirstSave.await()
                        firstSaveFinished.complete(Unit)
                    } else {
                        secondSaveFinished.complete(Unit)
                    }
                    "{}"
                }

                MobileCommand.DiscardDraft -> {
                    discardPayloads += payloadJson
                    discardFinished.complete(Unit)
                    // Set by tests that need the post-send discard to stay in
                    // flight while they act; nothing to wait on otherwise.
                    holdDiscard?.await()
                    if (discardFails) throw RuntimeException("discard failed")
                    "{}"
                }

                MobileCommand.Send -> {
                    sendCalls++
                    sendFinished.complete(Unit)
                    if (sendFails) throw RuntimeException("send failed")
                    "{}"
                }

                else -> {
                    "{}"
                }
            }

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
    }

    private class ReloadCore : MeronCore {
        val oldAccountsStarted = CompletableDeferred<Unit>()
        val newAccountsStarted = CompletableDeferred<Unit>()
        val oldProxyStarted = CompletableDeferred<Unit>()
        val newProxyStarted = CompletableDeferred<Unit>()
        val oldAccounts = CompletableDeferred<String>()
        val newAccounts = CompletableDeferred<String>()
        val oldProxy = CompletableDeferred<String>()
        val newProxy = CompletableDeferred<String>()
        private var accountCalls = 0
        private var proxyCalls = 0

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String =
            when (command) {
                MobileCommand.AccountList -> {
                    accountCalls++
                    if (accountCalls == 1) {
                        oldAccountsStarted.complete(Unit)
                        oldAccounts.await()
                    } else {
                        newAccountsStarted.complete(Unit)
                        newAccounts.await()
                    }
                }

                MobileCommand.AppProxyGet -> {
                    proxyCalls++
                    if (proxyCalls == 1) {
                        oldProxyStarted.complete(Unit)
                        oldProxy.await()
                    } else {
                        newProxyStarted.complete(Unit)
                        newProxy.await()
                    }
                }

                else -> {
                    "{}"
                }
            }

        override fun events(): CoreEventStream =
            object : CoreEventStream {
                override fun subscribe(listener: (CoreEvent) -> Unit): CloseableHandle = CloseableHandle {}
            }

        override suspend fun protocolVersion(): Int = 0
    }

    private class TestPreferences : AppPreferences {
        private val strings = mutableMapOf<String, String>()

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
        ): Boolean = default

        override fun putBoolean(
            key: String,
            value: Boolean,
        ) {}

        override fun getInt(
            key: String,
            default: Int,
        ): Int = default

        override fun putInt(
            key: String,
            value: Int,
        ) {}

        override fun getStringSet(
            key: String,
            default: Set<String>,
        ): Set<String> = default

        override fun putStringSet(
            key: String,
            value: Set<String>,
        ) {}

        override fun remove(key: String) {
            strings.remove(key)
        }
    }

    private class TestPlatformServices : PlatformServices {
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

    private class TestLocaleController : LocaleController {
        override fun systemLanguageTag(): String = ""

        override fun applySystem(tag: String) {}

        override fun deviceLanguageTag(): String = "en-US"

        override fun displayName(tag: String): String = tag
    }
}
