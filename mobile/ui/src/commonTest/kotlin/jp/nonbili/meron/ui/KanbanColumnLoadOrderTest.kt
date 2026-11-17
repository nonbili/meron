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
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeout
import kotlin.concurrent.Volatile
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertSame

/**
 * Two reloads of one Kanban column can be out at once — the Sent copy event's
 * and a quick reply's post-discard one — and the earlier read the store before
 * the discard. Landing last, it used to put the card back with the draft
 * counted and badged; only the newest request may write.
 */
class KanbanColumnLoadOrderTest {
    @Test
    fun missingRecentDestinationsDoNotReloadAnAlreadyLoadedAccount() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val thread = ThreadSummary(id = "a#Work#t1", accountId = "a", folder = "Work", subject = "Subject", sender = "Sender")
            state.recentThreadTransfers = listOf(RecentThreadTransfer("copy", "a", "a", "Deleted"))
            state.loadRecentTransferFolders(thread)
            assertEquals(1, core.folderListCalls)
            repeat(3) { state.loadRecentTransferFolders(thread) }
            assertEquals(1, core.folderListCalls)
            state.foldersByAccount = mapOf("a" to emptyList())
            state.loadRecentTransferFolders(thread)
            assertEquals(1, core.folderListCalls)
        }

    @Test
    fun recentFoldersLoadWithoutOpeningTheCopyPicker() =
        runBlocking {
            val state = state(GatedCore(), this)
            val thread = ThreadSummary(id = "a#Work#t1", accountId = "a", folder = "Work", subject = "Subject", sender = "Sender")
            state.recentThreadTransfers = listOf(RecentThreadTransfer("copy", "a", "a", "INBOX"))
            assertEquals(emptyList(), state.recentTransferItems(thread))
            state.loadRecentTransferFolders(thread)
            assertEquals(listOf("INBOX"), state.recentTransferItems(thread).map { it.folder.name })
            state.foldersByAccount = mapOf("a" to listOf(FolderSummary("a", "work"), FolderSummary("a", "INBOX")))
            state.recentThreadTransfers = listOf(RecentThreadTransfer("move", "a", "a", "work"), RecentThreadTransfer("copy", "a", "a", "inbox"))
            assertEquals(listOf("work", "INBOX"), state.recentTransferItems(thread).map { it.folder.name })
            assertEquals(listOf("work"), state.recentTransferItems(thread.copy(folder = "inbox")).map { it.folder.name })
        }

    @Test
    fun crossAccountMoveKeepsTheTargetAccountForMenusAndBoardMoves() =
        runBlocking {
            for (boardMove in listOf(false, true)) {
                val core = GatedCore()
                core.gate.complete(Unit)
                val state = state(core, this)
                val thread = ThreadSummary(id = "a#INBOX#t1", accountId = "a", folder = "INBOX", subject = "Subject", sender = "Sender")
                state.coreAccounts = state.coreAccounts + AccountSummary(id = "b", email = "b@example.com")
                if (boardMove) {
                    state.moveThreadToColumn(thread, KanbanColumnSpec("b", "INBOX"))
                } else {
                    state.moveThreadToFolder(thread, "INBOX", "b")
                }
                waitUntil { state.recentThreadTransfers.isNotEmpty() }
                assertEquals(
                    "b",
                    core.actionRequests
                        .single()
                        .second
                        .jsonStringValue("target_account_id"),
                )
                assertEquals(RecentThreadTransfer("move", "a", "b", "INBOX"), state.recentThreadTransfers.single())
            }
        }

    @Test
    fun kanbanDragRecordsItsDestinationAndRejectsZeroMoves() =
        runBlocking {
            for (zero in listOf(false, true)) {
                val core = GatedCore().apply { zeroMove = zero }
                core.gate.complete(Unit)
                val state = state(core, this)
                val thread = ThreadSummary(id = "a#INBOX#t1", accountId = "a", folder = "INBOX", subject = "Subject", sender = "Sender")
                state.moveThreadToColumn(thread, KanbanColumnSpec("a", "Work"))
                waitUntil { core.actionCalls > 0 }
                delay(50)
                assertEquals(if (zero) emptyList() else listOf(RecentThreadTransfer("move", "a", "a", "Work")), state.recentThreadTransfers)
            }
        }

    @Test
    fun successfulMovesPromoteRecentDestinationsWithoutDuplicates() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val first = RecentThreadTransfer("move", "a", "a", "Work")
            state.rememberThreadTransfer(first)
            state.rememberThreadTransfer(first.copy(action = "copy", targetFolderId = "Receipts"))
            state.rememberThreadTransfer(first)
            assertEquals(listOf(first, first.copy(action = "copy", targetFolderId = "Receipts")), state.recentThreadTransfers)
            assertEquals(state.recentThreadTransfers, loadRecentThreadTransfers(state.prefs))
        }

    @Test
    fun recentShortcutsExcludeCurrentMissingAndOtherAccountDestinationsAndStopAtTwo() =
        runBlocking {
            val state = state(GatedCore(), this)
            state.coreAccounts = listOf(AccountSummary(id = "a", email = "a@example.com"))
            state.foldersByAccount = mapOf("a" to listOf("INBOX", "Work", "Receipts", "Older").map { FolderSummary("a", it) })
            state.recentThreadTransfers =
                listOf(
                    RecentThreadTransfer("copy", "b", "a", "Work"),
                    RecentThreadTransfer("move", "a", "a", "INBOX"),
                    RecentThreadTransfer("move", "a", "a", "Deleted"),
                    RecentThreadTransfer("move", "a", "a", "Work"),
                    RecentThreadTransfer("copy", "a", "a", "Receipts"),
                    RecentThreadTransfer("move", "a", "a", "Older"),
                    RecentThreadTransfer("move", "a", "a", "Receipts"),
                    RecentThreadTransfer("move", "a", "a", "INBOX"),
                )
            val thread = ThreadSummary(id = "a#INBOX#t1", accountId = "a", folder = "INBOX", subject = "Subject", sender = "Sender")
            val items = state.recentTransferItems(thread)
            assertEquals(listOf("Work", "Receipts", "Older", "Receipts"), items.map { it.destination })
            assertEquals(listOf("Work", "Older", "Receipts"), items.forAction("move").map { it.destination })
            assertEquals(listOf("Receipts"), items.forAction("copy").map { it.destination })
            assertEquals(emptyList(), state.recentTransferItems(null))
        }

    @Test
    fun staleColumnLoadLandingLastDoesNotOverwriteTheNewerRows() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val column = KanbanColumnSpec(accountId = "a", folderId = "INBOX")
            val key = kanbanColumnKey(column)

            state.loadKanbanColumn(column)
            waitUntil { core.threadListCalls == 1 }
            // The post-discard reload, while the first one is still out.
            state.loadKanbanColumn(column)
            waitUntil { state.kanbanColumns[key]?.threads?.isNotEmpty() == true }
            val fresh = state.kanbanColumns[key]?.threads?.single()
            assertEquals(2, fresh?.messageCount)

            core.gate.complete(Unit)
            delay(50)
            val card = state.kanbanColumns[key]?.threads?.single()
            assertEquals(2, card?.messageCount)
            assertFalse(card?.hasDraft ?: true)
            assertFalse(state.kanbanColumns[key]?.loading ?: true)
        }

    @Test
    fun archiveCannotBeRestoredByAnOlderMailboxReload() = removalSurvivesLateRead(kanban = false, loadMore = false)

    @Test
    fun archiveCannotBeRestoredByAnOlderColumnReload() = removalSurvivesLateRead(kanban = true, loadMore = false)

    @Test
    fun deleteCannotBeRestoredByOlderMailboxPagination() = removalSurvivesLateRead(kanban = false, loadMore = true)

    @Test
    fun moveCannotBeRestoredByOlderColumnPagination() = removalSurvivesLateRead(kanban = true, loadMore = true)

    @Test
    fun failedArchiveRestoresRowsAndReleasesTheOlderRead() = removalSurvivesLateRead(kanban = false, loadMore = false, actionFails = true)

    @Test
    fun failedColumnMoveReloadsASourceRowHiddenByAPendingRead() =
        runBlocking {
            val childScope = CoroutineScope(coroutineContext + Job(coroutineContext[Job]))
            try {
                val core = GatedCore(gateActions = true, actionFails = true)
                val state = state(core, childScope)
                val thread = ThreadSummary(id = "a:INBOX:t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
                val column = KanbanColumnSpec(accountId = "a", folderId = "INBOX")
                val key = kanbanColumnKey(column)
                state.kanbanBoards = listOf(KanbanBoardSpec(id = "board", name = "Board", columns = listOf(column)))
                state.activeKanbanBoardId = "board"
                state.kanbanColumns = mapOf(key to KanbanColumnState(threads = listOf(thread)))
                state.loadKanbanColumn(column)
                waitUntil { core.threadListCalls == 1 }
                state.moveThreadToColumn(thread, KanbanColumnSpec(accountId = "a", folderId = "Archive"))
                waitUntil { core.actionCalls == 1 }
                core.gate.complete(Unit)
                waitUntil { state.kanbanColumns[key]?.loading == false }
                assertEquals(emptyList(), state.kanbanColumns[key]?.threads)
                core.actionGate.complete(Unit)
                waitUntil { state.kanbanColumns[key]?.threads?.isNotEmpty() == true && !state.syncing }
                assertEquals(
                    thread.id,
                    state.kanbanColumns[key]
                        ?.threads
                        ?.single()
                        ?.id,
                )
                assertEquals(thread.id, state.coreThreads.single().id)
                waitUntil { state.statusIsError }
                assertEquals("Action rejected", state.status)
                state.status = "Move complete"
                assertFalse(state.statusIsError)
            } finally {
                childScope.cancel()
            }
        }

    @Test
    fun notificationMailboxCannotRestoreAnArchivedThread() = removalSurvivesLateRead(kanban = false, loadMore = false, notification = true)

    @Test
    fun aCompletedColumnMoveDoesNotReleaseAnOverlappingArchive() =
        runBlocking {
            val childScope = CoroutineScope(coroutineContext + Job(coroutineContext[Job]))
            try {
                val core = GatedCore()
                val moveGate = CompletableDeferred<Unit>()
                val archiveGate = CompletableDeferred<Unit>()
                core.actionGates[MobileCommand.Move] = moveGate
                core.actionGates[MobileCommand.Archive] = archiveGate
                val state = state(core, childScope)
                val thread = ThreadSummary(id = "a:INBOX:t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
                state.coreThreads = listOf(thread)
                state.moveThreadToColumn(thread, KanbanColumnSpec(accountId = "a", folderId = "Archive"))
                state.archiveOrRemove(thread)
                waitUntil { core.actionCalls == 2 }
                moveGate.complete(Unit)
                waitUntil { state.status == "Move complete" }
                // Begin a mailbox read after the first mutation has completed.
                state.openNotificationThread(NotificationThreadTarget(accountId = "a", folder = "INBOX"))
                waitUntil { core.threadListCalls >= 2 }
                waitUntil { !state.syncing }
                assertEquals(emptyList(), state.coreThreads)
                archiveGate.complete(Unit)
                waitUntil { state.threadRemovalGuard.filter(listOf(thread)).isNotEmpty() }
                core.gate.complete(Unit)
                waitUntil { state.kanbanColumns.values.none { it.loading } }
            } finally {
                childScope.cancel()
            }
        }

    // Archiving in the Archive folder (Gmail's All Mail) takes the Inbox copies
    // out; the thread stays listed there, so its card stays.
    @Test
    fun archivingInTheArchiveFolderKeepsTheCard() =
        checkArchiveInPlace(
            reply = """{"folder":"Archive","moved":1,"ok":true,"source_folder":"INBOX","thread_id":"a#Archive#t1"}""",
            cardStays = true,
            status = "mail.toast.archivedCount",
            inboxCardStays = false,
        )

    @Test
    fun archivingAnAlreadyArchivedThreadSaysSo() =
        checkArchiveInPlace(
            reply = """{"already_archived":true,"folder":"Archive","moved":0,"ok":true,"thread_id":"a#Archive#t1"}""",
            cardStays = true,
            status = "mail.toast.alreadyArchived",
        )

    // A Gmail label named "Archive" has the archive role too, but the account
    // archives to All Mail: the thread leaves, so its card goes.
    @Test
    fun archivingFromAFolderThatOnlyLooksLikeTheArchiveRemovesTheCard() =
        checkArchiveInPlace(
            reply = """{"folder":"[Gmail]/All Mail","moved":1,"ok":true,"thread_id":"a#[Gmail]/All Mail#t1"}""",
            cardStays = false,
            status = "mail.toast.archivedCount",
        )

    private fun checkArchiveInPlace(
        reply: String?,
        cardStays: Boolean,
        status: String,
        inboxCardStays: Boolean = true,
    ) = runBlocking {
        val childScope = CoroutineScope(coroutineContext + Job(coroutineContext[Job]))
        try {
            val core = GatedCore()
            core.archiveReply = reply
            val gate = CompletableDeferred<Unit>()
            core.actionGates[MobileCommand.Archive] = gate
            val state = state(core, childScope)
            val thread =
                ThreadSummary(id = "a#Archive#t1", accountId = "a", folder = "Archive", folderRole = "archive", subject = "Hello", sender = "Ada")
            // The same conversation's Inbox copy, in a Kanban column and a cached list.
            val inboxThread = ThreadSummary(id = "a#INBOX#t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
            val inboxColumn = kanbanColumnKey(KanbanColumnSpec(accountId = "a", folderId = "INBOX"))
            state.kanbanColumns = mapOf(inboxColumn to KanbanColumnState(threads = listOf(inboxThread)))
            state.coreThreads = listOf(thread)
            val cacheKey = MailboxCacheKey(accountId = "a", folderId = "Archive", query = "", filter = FilterMode.All)
            val inboxKey = cacheKey.copy(folderId = "INBOX")
            state.mailboxCache =
                mapOf(
                    cacheKey to MailboxLoadResult(folders = emptyList(), folder = "Archive", threads = listOf(thread)),
                    inboxKey to MailboxLoadResult(folders = emptyList(), folder = "INBOX", threads = listOf(inboxThread)),
                )
            state.archiveOrRemove(thread)
            waitUntil { core.actionCalls == 1 }
            // Nothing leaves while the archive is under way.
            assertEquals(listOf(thread.id), state.coreThreads.map { it.id })
            gate.complete(Unit)
            val expected = if (status == "mail.toast.archivedCount") state.trs(status, mapOf("count" to 1)) else state.trs(status)
            waitUntil { state.status == expected }
            val listed = if (cardStays) listOf(thread.id) else emptyList()
            assertEquals(listed, state.coreThreads.map { it.id })
            // The cached mailbox agrees, so switching back doesn't revive it.
            assertEquals(listed, state.mailboxCache[cacheKey]?.threads?.map { it.id })
            val inbox = if (inboxCardStays) listOf(inboxThread.id) else emptyList()
            assertEquals(inbox, state.kanbanColumns[inboxColumn]?.threads?.map { it.id })
            assertEquals(inbox, state.mailboxCache[inboxKey]?.threads?.map { it.id })
        } finally {
            childScope.cancel()
        }
    }

    @Test
    fun cachedMailboxesRetainTheirOtherRowsAndCursorsAfterRemoval() = checkCachedRemoval(success = true)

    @Test
    fun overlappingFailuresRestoreCachedRowsAfterAnInterveningRead() = checkCachedRemoval(success = false)

    private fun checkCachedRemoval(success: Boolean) =
        runBlocking {
            val state = state(GatedCore(), this)
            val thread = ThreadSummary(id = "a:INBOX:t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
            val other = thread.copy(id = "a:INBOX:t2", dateEpochSeconds = 2)
            val keys = listOf(mailboxCacheKey("a", "INBOX", "", FilterMode.All), mailboxCacheKey(UNIFIED_ACCOUNT_ID, "INBOX", "", FilterMode.All))
            val cached = MailboxLoadResult(emptyList(), "INBOX", listOf(other, thread), nextCursor = "older", pageDepth = 100)
            state.mailboxCache = keys.associateWith { cached }
            val first = state.suppressRemovedThread(thread.id)
            val second = state.suppressRemovedThread(thread.id)
            // Another read caches only the visible rows while both writes are out.
            state.mailboxCache = keys.associateWith { cached.copy(threads = listOf(other)) }
            if (success) first.complete() else first.rollback()
            second.rollback()
            keys.forEach { key ->
                assertEquals(if (success) listOf(other) else listOf(other, thread), state.mailboxCache[key]?.threads)
                assertEquals("older", state.mailboxCache[key]?.nextCursor)
                assertEquals(100, state.mailboxCache[key]?.pageDepth)
            }
        }

    @Test
    fun manualMoveBackReleasesTheDestinationInASlowMailboxRead() =
        runBlocking {
            val childScope = CoroutineScope(coroutineContext + Job(coroutineContext[Job]))
            try {
                val core = GatedCore()
                val state = state(core, childScope)
                val thread = ThreadSummary(id = "a:INBOX:t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
                state.coreThreads = listOf(thread)
                state.syncCoreThreads(syncFirst = false)
                waitUntil { core.threadListCalls == 1 }
                var moves = 0
                state.moveThreadToFolder(thread, "Archive") { moves++ }
                waitUntil { moves == 1 }
                state.moveThreadToFolder(thread.copy(id = "a:Archive:t1", folder = "Archive"), "INBOX") { moves++ }
                waitUntil { moves == 2 }
                core.gate.complete(Unit)
                waitUntil { !state.syncing }
                assertEquals(thread.id, state.coreThreads.single().id)
            } finally {
                childScope.cancel()
            }
        }

    @Test
    fun emptyFolderKeepsUnloadedRowsOutOfAnOlderMailboxRead() =
        runBlocking {
            val childScope = CoroutineScope(coroutineContext + Job(coroutineContext[Job]))
            try {
                val core = GatedCore(gateActions = true, emptyAfterActions = true, listedFolder = "Trash")
                val state = state(core, childScope)
                state.selectedCoreFolder = "Trash"
                state.syncCoreThreads(syncFirst = false)
                waitUntil { core.threadListCalls == 1 }
                state.emptyMailFolder("a", "Trash")
                waitUntil { core.actionCalls == 1 }
                core.gate.complete(Unit)
                waitUntil { !state.syncing }
                assertEquals(emptyList(), state.coreThreads)
                core.actionGate.complete(Unit)
                waitUntil { core.threadListCalls == 2 && !state.syncing }
                assertEquals(emptyList(), state.coreThreads)
            } finally {
                childScope.cancel()
            }
        }

    @Test
    fun permanentDeleteCannotReleaseItsSourceIdFromChange() = removalSurvivesLateRead(kanban = false, loadMore = true, permanentDelete = true)

    @Test
    fun undoMovesFromTheTopLevelDestinationInsteadOfTheChangeSource() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val thread = ThreadSummary(id = "a#INBOX#t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
            state.restoreThread(
                thread,
                listOf(thread),
                emptyMap(),
                """{"change":{"source_folder":"INBOX","thread_id":"a#INBOX#t1"},"folder":"Archive","moved":1,"ok":true,"thread_id":"a#Archive#t1"}""",
            )
            waitUntil { state.status == "Restored" }
            val (command, payload) = core.actionRequests.single()
            assertEquals(MobileCommand.Move, command)
            assertEquals("a#Archive#t1", payload.jsonStringValue("thread_id"))
            assertEquals("INBOX", payload.jsonStringValue("target_folder_id"))
            assertEquals(listOf(thread), state.coreThreads)
        }

    @Test
    fun permanentDeleteWithOnlyAChangeSourceDoesNotOfferUndo() =
        runBlocking {
            val core = GatedCore()
            val state = state(core, this)
            val thread = ThreadSummary(id = "a#Trash#t1", accountId = "a", folder = "Trash", subject = "Hello", sender = "Ada")
            state.restoreThread(
                thread,
                listOf(thread),
                emptyMap(),
                """{"change":{"source_folder":"Trash","thread_id":"a#Trash#t1"},"deleted":1,"ok":true,"permanent":true}""",
            )
            assertEquals("Undo unavailable", state.status)
            assertEquals(emptyList(), core.actionRequests)
            assertEquals(emptyList(), state.coreThreads)
        }

    @Test
    fun cacheRollbackLeavesUnaffectedMailboxOrderAndInstanceAlone() =
        runBlocking {
            val state = state(GatedCore(), this)
            val thread = ThreadSummary(id = "a:INBOX:t1", accountId = "a", folder = "INBOX", subject = "Hello", sender = "Ada")
            val older = thread.copy(id = "a:Archive:older", folder = "Archive", dateEpochSeconds = 1)
            val newer = older.copy(id = "a:Archive:newer", dateEpochSeconds = 2)
            val affectedKey = mailboxCacheKey("a", "INBOX", "", FilterMode.All)
            val untouchedKey = mailboxCacheKey("a", "Archive", "", FilterMode.All)
            val unaffected = MailboxLoadResult(emptyList(), "Archive", listOf(older, newer))
            state.mailboxCache =
                mapOf(
                    affectedKey to MailboxLoadResult(emptyList(), "INBOX", listOf(thread)),
                    untouchedKey to unaffected,
                )
            val removal = state.suppressRemovedThread(thread.id)
            removal.rollback()
            assertSame(unaffected, state.mailboxCache[untouchedKey])
            assertEquals(listOf(older, newer), state.mailboxCache[untouchedKey]?.threads)
        }

    private fun removalSurvivesLateRead(
        kanban: Boolean,
        loadMore: Boolean,
        actionFails: Boolean = false,
        notification: Boolean = false,
        permanentDelete: Boolean = false,
    ) = runBlocking {
        val childScope = CoroutineScope(coroutineContext + Job(coroutineContext[Job]))
        try {
            val folder = if (permanentDelete) "Trash" else "INBOX"
            val core = GatedCore(gateActions = true, actionFails = actionFails, permanentDelete = permanentDelete, listedFolder = folder)
            val state = state(core, childScope)
            val thread = ThreadSummary(id = "a:$folder:t1", accountId = "a", folder = folder, subject = "Hello", sender = "Ada")
            val column = KanbanColumnSpec(accountId = "a", folderId = folder)
            val key = kanbanColumnKey(column)
            val cacheKey = mailboxCacheKey("a", folder, "", FilterMode.All)
            state.selectedCoreFolder = folder
            state.coreThreads = listOf(thread)
            state.visibleMailboxKey = cacheKey
            state.mailboxCursor = "older"
            state.mailboxCache = mapOf(cacheKey to MailboxLoadResult(emptyList(), folder, listOf(thread)))
            state.kanbanColumns = mapOf(key to KanbanColumnState(threads = listOf(thread), nextCursor = "older"))
            if (notification) {
                state.openNotificationThread(NotificationThreadTarget(accountId = "a", folder = folder))
            } else if (kanban) {
                if (loadMore) state.loadMoreKanbanColumn(column) else state.loadKanbanColumn(column)
            } else {
                if (loadMore) state.loadMoreCoreThreads() else state.syncCoreThreads(syncFirst = false)
            }
            waitUntil { core.threadListCalls == 1 }
            when {
                kanban && loadMore -> state.moveThreadToFolder(thread, "Archive")
                loadMore -> state.deleteThread(thread)
                else -> state.archiveOrRemove(thread)
            }
            waitUntil { core.actionCalls == 1 }
            assertEquals(emptyList(), state.coreThreads)
            assertEquals(emptyList(), state.threadRemovalGuard.filter(state.mailboxCache[cacheKey]!!.threads))
            core.actionGate.complete(Unit)
            waitUntil { state.threadRemovalGuard.filter(listOf(thread)).isNotEmpty() }
            // The backend write is done, but the old read still holds its row.
            core.gate.complete(Unit)
            waitUntil { !state.syncing && !state.loadingMoreThreads && state.kanbanColumns[key]?.loading != true && state.kanbanColumns[key]?.loadingMore != true }
            if (actionFails) {
                assertEquals(thread.id, state.coreThreads.single().id)
                assertEquals(
                    thread.id,
                    state.kanbanColumns[key]
                        ?.threads
                        ?.single()
                        ?.id,
                )
            } else {
                assertEquals(emptyList(), state.coreThreads)
                assertEquals(emptyList(), state.kanbanColumns[key]?.threads)
                assertFalse(state.mailboxCache.values.any { cached -> cached.threads.any { it.id == thread.id } })
            }
            // A subsequent valid read can display a moved-back copy or new reply.
            if (kanban) state.loadKanbanColumn(column) else state.syncCoreThreads(syncFirst = false)
            waitUntil { core.threadListCalls >= 2 && !state.syncing && state.kanbanColumns[key]?.loading != true }
            assertEquals(
                thread.id,
                if (kanban) {
                    state.kanbanColumns[key]
                        ?.threads
                        ?.single()
                        ?.id
                } else {
                    state.coreThreads.single().id
                },
            )
        } finally {
            childScope.cancel()
        }
    }

    private suspend fun waitUntil(condition: () -> Boolean) {
        withTimeout(5_000) {
            while (!condition()) delay(5)
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
            prefs = MemoryPreferences(),
            kanbanPrefs = MemoryPreferences(),
            services = NoopPlatformServices(),
            locale = NoopLocaleController(),
            mobileHost = DefaultMobileHost(),
            settingsMirror = SettingsMirror(core, MemoryPreferences()) { true },
        ).apply {
            coreAccounts =
                listOf(
                    AccountSummary(
                        id = "a",
                        email = "a@example.com",
                        imapHost = "127.0.0.1",
                        imapPort = 1143,
                        smtpHost = "127.0.0.1",
                        smtpPort = 1025,
                        tls = false,
                        starttls = true,
                        smtpTls = false,
                        smtpStarttls = true,
                    ),
                )
            selectedCoreAccountId = "a"
            selectedCoreFolder = "INBOX"
            initialThreadsLoaded = true
        }

    /** Holds the first thread list read open until [gate] completes; every
     *  later read answers at once with one message fewer on the card. */
    private class GatedCore(
        private val gateActions: Boolean = false,
        private val actionFails: Boolean = false,
        private val permanentDelete: Boolean = false,
        private val emptyAfterActions: Boolean = false,
        private val listedFolder: String = "INBOX",
    ) : MeronCore {
        var zeroMove = false

        /** Answers Archive with this instead of the usual move into "Archive". */
        var archiveReply: String? = null
        var folderListCalls = 0
        val actionGate = CompletableDeferred<Unit>()
        val actionGates = mutableMapOf<String, CompletableDeferred<Unit>>()

        // Commands arrive on the IO dispatcher, two at once when actions
        // overlap, while the test polls the counts from its own thread.
        private val bookkeeping = Mutex()
        val actionRequests = mutableListOf<Pair<String, String>>()

        @Volatile
        var actionCalls = 0

        @Volatile
        private var actionsCompleted = false
        val gate = CompletableDeferred<Unit>()

        @Volatile
        var threadListCalls = 0

        override suspend fun invoke(
            command: String,
            payloadJson: String,
        ): String =
            when (command) {
                MobileCommand.FolderList -> {
                    folderListCalls++
                    """{"folders":[{"account_id":"a","name":"INBOX","role":"inbox"}]}"""
                }

                MobileCommand.ThreadList -> {
                    val call =
                        bookkeeping.withLock {
                            threadListCalls += 1
                            threadListCalls
                        }
                    if (emptyAfterActions && actionsCompleted) {
                        """{"threads":[]}"""
                    } else if (call == 1) {
                        gate.await()
                        card(messageCount = 3, hasDraft = true)
                    } else {
                        card(messageCount = 2, hasDraft = false)
                    }
                }

                MobileCommand.Archive, MobileCommand.Delete, MobileCommand.Move, MobileCommand.EmptyFolder -> {
                    bookkeeping.withLock {
                        actionRequests.add(command to payloadJson)
                        actionCalls += 1
                    }
                    val gate = actionGates[command]
                    if (gate != null) {
                        gate.await()
                    } else if (gateActions) {
                        actionGate.await()
                    }
                    actionsCompleted = true
                    when {
                        zeroMove && command == MobileCommand.Move -> {
                            """{"ok":true,"moved":0}"""
                        }

                        command == MobileCommand.Archive && archiveReply != null -> {
                            archiveReply!!
                        }

                        actionFails -> {
                            """{"error":{"message":"Action rejected"}}"""
                        }

                        else -> {
                            val sourceId = payloadJson.jsonStringValue("thread_id")
                            val destination =
                                when (command) {
                                    MobileCommand.Archive -> "Archive"
                                    MobileCommand.Delete -> "Trash"
                                    MobileCommand.Move -> payloadJson.jsonStringValue("target_folder_id")
                                    else -> ""
                                }
                            val change = """{"account_id":"a","removed":true,"source_folder":"$listedFolder","thread_id":"$sourceId"}"""
                            when {
                                command == MobileCommand.EmptyFolder -> {
                                    """{"ok":true}"""
                                }

                                command == MobileCommand.Delete && permanentDelete -> {
                                    """{"change":$change,"deleted":1,"ok":true,"permanent":true}"""
                                }

                                else -> {
                                    val destinationId =
                                        if (sourceId.contains('#')) {
                                            "${sourceId.substringBefore('#')}#$destination#${sourceId.substringAfterLast('#')}"
                                        } else {
                                            "a:$destination:t1"
                                        }
                                    if (command == MobileCommand.Delete) {
                                        """{"change":$change,"deleted":1,"ok":true,"thread_id":"$destinationId","trash":"$destination"}"""
                                    } else {
                                        """{"change":$change,"folder":"$destination","moved":1,"ok":true,"thread_id":"$destinationId"}"""
                                    }
                                }
                            }
                        }
                    }
                }

                else -> {
                    "{}"
                }
            }

        private fun card(
            messageCount: Int,
            hasDraft: Boolean,
        ): String =
            """{"threads":[{"id":"a:$listedFolder:t1","account_id":"a","folder_id":"$listedFolder","subject":"Re: hello",""" +
                """"message_count":$messageCount,"has_draft":$hasDraft,"date":1}]}"""

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
