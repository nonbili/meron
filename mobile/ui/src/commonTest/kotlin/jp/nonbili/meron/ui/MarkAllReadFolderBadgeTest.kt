package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.CloseableHandle
import jp.nonbili.meron.shared.CoreEvent
import jp.nonbili.meron.shared.CoreEventStream
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MeronCore
import jp.nonbili.meron.shared.MobileCommand
import jp.nonbili.meron.shared.ThreadSummary
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertEquals

/**
 * The drawer's unread badges are folder totals, not counts of the loaded rows,
 * so marking a mailbox or a column read has to clear them in the same breath as
 * the rows — they used to sit at the pre-mark number until the write came back.
 */
class MarkAllReadFolderBadgeTest {
    @Test
    fun markingTheMailboxReadClearsTheDrawerBadgeBeforeTheWriteAnswers() {
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)

            state.markVisibleMailboxAllRead()

            assertEquals(0, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(0, folderUnread(state.coreFolders, INBOX_FOLDER))
            core.gate.complete(Unit)
        }
    }

    @Test
    fun aFailedMailboxWritePutsTheDrawerBadgeBack() =
        runBlocking {
            val core = GatedCore().apply { gate.complete(Unit) }
            core.markAllReadFails = true
            val state = state(core, this)

            state.markVisibleMailboxAllRead()

            assertEquals(0, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            waitUntil { state.status.startsWith("Mark all read failed") }
            assertEquals(12, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(12, folderUnread(state.coreFolders, INBOX_FOLDER))
        }

    @Test
    fun markingAColumnReadClearsTheDrawerBadgeBeforeTheWriteAnswers() {
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)

            state.markKanbanColumnAllRead(KanbanColumnSpec(accountId = "a", folderId = "INBOX"))

            assertEquals(0, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            core.gate.complete(Unit)
        }
    }

    @Test
    fun aFailedColumnWritePutsTheDrawerBadgeBack() =
        runBlocking {
            val core = GatedCore().apply { gate.complete(Unit) }
            core.markAllReadFails = true
            val state = state(core, this)

            state.markKanbanColumnAllRead(KanbanColumnSpec(accountId = "a", folderId = "INBOX"))

            assertEquals(0, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            waitUntil { !state.kanbanMarkingRead }
            assertEquals(12, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(12, folderUnread(state.coreFolders, INBOX_FOLDER))
        }

    @Test
    fun staleColumnRefreshCannotRestoreDrawerCountsDuringOrAfterMarkRead() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val column = KanbanColumnSpec(accountId = "a", folderId = "INBOX")
            state.markKanbanColumnAllRead(column)

            state.loadKanbanColumn(column)
            waitUntil { state.kanbanColumns["a\nINBOX"]?.loading == false }
            assertEquals(0, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))

            core.holdThreads = true
            state.loadKanbanColumn(column)
            core.threadsStarted.await()
            core.gate.complete(Unit)
            waitUntil { !state.kanbanMarkingRead }
            core.threadsGate.complete(Unit)
            waitUntil { state.kanbanColumns["a\nINBOX"]?.loading == false }
            assertEquals(0, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(0, folderUnread(state.coreFolders, INBOX_FOLDER))

            // A refresh started after completion can show newly arrived mail.
            core.holdThreads = false
            state.loadKanbanColumn(column)
            waitUntil { state.kanbanColumns["a\nINBOX"]?.loading == false }
            assertEquals(12, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
        }

    @Test
    fun failedUnifiedPlanPreservesEarlierSuccessfulSharedFolderCount() =
        runBlocking {
            val core =
                GatedCore().apply {
                    gate.complete(Unit)
                    failRss = true
                    confirmedUnread = 2
                }
            val state = state(core, this)
            val direct = KanbanColumnSpec(accountId = "a", folderId = "INBOX")
            val unified = KanbanColumnSpec(accountId = UNIFIED_ACCOUNT_ID, folderId = INBOX_FOLDER)
            state.kanbanBoards = listOf(KanbanBoardSpec(id = "board", name = "Board", columns = listOf(direct, unified)))
            state.activeKanbanBoardId = "board"
            state.kanbanColumns = state.kanbanColumns + (
                kanbanColumnKey(unified) to
                    KanbanColumnState(
                        threads = listOf(ThreadSummary(id = "rss-account#rss#feed-1", accountId = "rss-account", folder = "rss", subject = "Feed", sender = "Feed", unread = true)),
                        unreadCount = 13,
                    )
            )
            state.markKanbanBoardAllRead()
            waitUntil { !state.kanbanMarkingRead }
            assertEquals(2, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(2, folderUnread(state.coreFolders, INBOX_FOLDER))
        }

    @Test
    fun cancellationBeforeLaunchDoesNotLeaveFolderHolds() =
        runBlocking {
            val job = kotlinx.coroutines.Job().apply { cancel() }
            val core = GatedCore()
            val state = state(core, CoroutineScope(coroutineContext + job))
            state.markKanbanColumnAllRead(KanbanColumnSpec(accountId = "a", folderId = "INBOX"))
            waitUntil { !state.kanbanMarkingRead }
            val fresh = listOf(FolderSummary(accountId = "a", name = "INBOX", unread = 7))
            assertEquals(7, state.reconcileFolderUnread(fresh, state.folderReadGuard.version).single().unread)
        }

    @Test
    fun coreFolderCacheProtectsCountsBeforeAccountCacheIsPopulated() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            state.foldersByAccount = emptyMap()
            state.markKanbanColumnAllRead(KanbanColumnSpec(accountId = "a", folderId = "INBOX"))
            val stale = listOf(FolderSummary(accountId = "a", name = "INBOX", unread = 12))
            assertEquals(0, state.reconcileFolderUnread(stale, state.folderReadGuard.version).single().unread)
            core.gate.complete(Unit)
            waitUntil { !state.kanbanMarkingRead }
        }

    @Test
    fun concurrentMutationCountSurvivesAnOlderBoardResponse() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            state.markKanbanColumnAllRead(KanbanColumnSpec(accountId = "a", folderId = "INBOX"))
            state.runCoreThreadAction(
                thread = state.coreThreads.first(),
                label = "Mark unread",
                action = { """{"ok":true,"folder_counts":[{"account_id":"a","folder_id":"INBOX","unread":1}]}""" },
                update = { it },
            )
            waitUntil { state.status == "Mark unread complete" }
            assertEquals(1, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(1, folderUnread(state.coreFolders, INBOX_FOLDER))
            core.gate.complete(Unit)
            waitUntil { !state.kanbanMarkingRead }
            assertEquals(1, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(1, folderUnread(state.coreFolders, INBOX_FOLDER))
        }

    @Test
    fun unifiedMarkAllReadMarksTheSelectedRoleNotTheInbox() =
        runBlocking {
            val core = GatedCore().apply { gate.complete(Unit) }
            val state = state(core, this)
            val sent = FolderSummary(accountId = "a", name = "Sent Items", role = "sent", unread = 3)
            val sentRow = ThreadSummary(id = "a#Sent Items#s", accountId = "a", folder = "Sent Items", subject = "Re", sender = "Me", unread = true)
            state.foldersByAccount = mapOf("a" to (state.foldersByAccount["a"].orEmpty() + sent))
            state.coreFolders = state.coreFolders + sent
            state.selectedCoreAccountId = UNIFIED_ACCOUNT_ID
            state.selectedCoreFolder = "sent"
            state.coreThreads = listOf(sentRow)

            state.markVisibleMailboxAllRead()

            assertEquals(0, folderUnread(state.foldersByAccount["a"], "Sent Items"))
            assertEquals(12, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            // The Inbox column's card is not in Sent, so it stays unread.
            assertEquals(
                true,
                state.kanbanColumns["a\nINBOX"]
                    ?.threads
                    ?.single()
                    ?.unread,
            )
            waitUntil { core.markAllReadPayloads.isNotEmpty() }
            assertEquals(true, core.markAllReadPayloads.single().contains("\"folder_id\":\"sent\""))
        }

    @Test
    fun aPartiallyFailedUnifiedWriteRollsBack() =
        runBlocking {
            val core =
                GatedCore().apply {
                    gate.complete(Unit)
                    markAllReadResponse = """{"ok":false,"failures":[{"account_id":"a","message":"Offline"}],"folder_counts":[]}"""
                }
            val state = state(core, this)
            state.selectedCoreAccountId = UNIFIED_ACCOUNT_ID
            state.selectedCoreFolder = INBOX_FOLDER

            state.markVisibleMailboxAllRead()

            waitUntil { state.status.startsWith("Mark all read failed") }
            assertEquals(12, folderUnread(state.foldersByAccount["a"], INBOX_FOLDER))
            assertEquals(true, state.coreThreads.single().unread)
        }

    private suspend fun waitUntil(condition: () -> Boolean) {
        withTimeout(5_000) {
            while (!condition()) delay(5)
        }
    }

    private fun state(
        core: MeronCore,
        scope: CoroutineScope,
    ): MeronMobileState {
        val row =
            ThreadSummary(
                id = "a#INBOX#root",
                accountId = "a",
                folder = "INBOX",
                subject = "Release",
                sender = "Sender",
                unread = true,
            )
        val inbox = FolderSummary(accountId = "a", name = "INBOX", role = "inbox", unread = 12)
        return MeronMobileState(
            scope = scope,
            core = core,
            coreLoaded = true,
            prefs = MemoryPreferences(),
            kanbanPrefs = MemoryPreferences(),
            services = NoopPlatformServices(),
            locale = NoopLocaleController(),
            mobileHost = DefaultMobileHost(),
            settingsMirror = SettingsMirror(core, MemoryPreferences()) { true },
        ).apply {
            coreAccounts = listOf(AccountSummary(id = "a", email = "a@example.com"))
            selectedCoreAccountId = "a"
            selectedCoreFolder = "INBOX"
            initialThreadsLoaded = true
            coreThreads = listOf(row)
            coreFolders = listOf(inbox)
            foldersByAccount = mapOf("a" to listOf(inbox))
            kanbanColumns = mapOf("a\nINBOX" to KanbanColumnState(threads = listOf(row), unreadCount = 12))
        }
    }

    /** Holds the mark-read write open until [gate] completes, or fails it. */
    private class GatedCore : MeronCore {
        val gate = CompletableDeferred<Unit>()
        var confirmedUnread = 0
        var failRss = false
        var markAllReadFails = false
        var markAllReadResponse: String? = null
        val markAllReadPayloads = mutableListOf<String>()
        var holdThreads = false
        val threadsStarted = CompletableDeferred<Unit>()
        val threadsGate = CompletableDeferred<Unit>()

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String =
            when (command) {
                MobileCommand.MarkAllRead -> {
                    markAllReadPayloads += payloadJson
                    gate.await()
                    if (markAllReadFails) error("Server rejected the write")
                    markAllReadResponse ?: """{"ok":true,"folder_counts":[{"account_id":"a","folder_id":"INBOX","unread":$confirmedUnread}]}"""
                }

                MobileCommand.RssMarkRead -> {
                    if (failRss) error("RSS write failed")
                    "{}"
                }

                MobileCommand.FolderList -> {
                    """{"folders":[{"account_id":"a","name":"INBOX","role":"inbox","unread":12}]}"""
                }

                MobileCommand.ThreadList -> {
                    if (holdThreads) {
                        threadsStarted.complete(Unit)
                        threadsGate.await()
                    }
                    """{"threads":[]}"""
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

    private class MemoryPreferences : AppPreferences {
        private val values = mutableMapOf<String, String>()

        override fun getString(
            key: String,
            default: String,
        ): String = values[key] ?: default

        override fun putString(
            key: String,
            value: String,
        ) {
            values[key] = value
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
            values.remove(key)
        }
    }

    private class NoopPlatformServices : PlatformServices {
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

    private class NoopLocaleController : LocaleController {
        override fun systemLanguageTag(): String = ""

        override fun applySystem(tag: String) {}

        override fun deviceLanguageTag(): String = "en-US"

        override fun displayName(tag: String): String = tag
    }
}
