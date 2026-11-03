package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.ui.input.key.key
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.EmptyFolderParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderDeleteParams
import jp.nonbili.meron.shared.FolderRenameParams
import jp.nonbili.meron.shared.FolderSetNotifyParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.Rule
import jp.nonbili.meron.shared.RuleStatus
import jp.nonbili.meron.shared.SetTagsParams
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.TagCount
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.mailThreadIdFolder
import jp.nonbili.meron.shared.parseFolderDeleteResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseFolderRenameResponse
import jp.nonbili.meron.shared.parseFolderUnreadChanges
import jp.nonbili.meron.shared.parseReplaceTagResponse
import jp.nonbili.meron.shared.parseReplacedQueries
import jp.nonbili.meron.shared.parseRuleCountResponse
import jp.nonbili.meron.shared.parseRuleStatusResponse
import jp.nonbili.meron.shared.parseRulesResponse
import jp.nonbili.meron.shared.parseSetTagsResponse
import jp.nonbili.meron.shared.parseTagCountsResponse
import jp.nonbili.meron.shared.parseTagListResponse
import jp.nonbili.meron.shared.parseThreadActionLocationResponse
import jp.nonbili.meron.shared.requireCoreAllOk
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

internal fun MeronMobileState.runCoreThreadAction(
    thread: ThreadSummary,
    /** For the log only. */
    label: String,
    /** Shown once the action is done, unless the undo snackbar is. */
    doneStatus: String?,
    /** Shown, with the reason, if the action fails. */
    failedStatus: String,
    action: suspend MobileMailCommandClient.() -> String,
    update: (List<ThreadSummary>) -> List<ThreadSummary>,
    undoMessage: String? = null,
    onUndo: ((String) -> Unit)? = null,
    afterSuccess: (() -> Unit)? = null,
    removesThread: Boolean = false,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    // Apply optimistically so the UI reacts instantly, then revert if the core
    // call fails. Snapshots taken here back the failure rollback.
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    val removal = if (removesThread) suppressRemovedThread(thread.id) else null
    coreThreads = update(coreThreads).forStarredView(selectedCoreAccountId, selectedCoreFolder)
    kanbanColumns =
        kanbanColumns.mapValues { (key, state) ->
            val (accountId, folderId) = kanbanKeyAccountFolder(key)
            val nextThreads = update(state.threads).forStarredView(accountId, folderId)
            val unreadDelta = loadedUnreadCount(nextThreads) - loadedUnreadCount(state.threads)
            state.copy(
                threads = nextThreads,
                unreadCount = state.unreadCount?.let { (it + unreadDelta).coerceAtLeast(0) },
            )
        }
    // The action commits immediately; Undo issues a compensating action (onUndo).
    // Track the commit so an Undo tap waits for it to finish, and is skipped if
    // the commit itself failed (the UI has already rolled back in that case).
    val committed = CompletableDeferred<String?>()
    scope.launch {
        runCatching {
            requireCoreOk(
                withContext(ioDispatcher) {
                    val client = MobileMailCommandClient(core)
                    withManagedGoogleAuth(client, thread.accountId) { client.action() }
                },
            )
        }.onSuccess { response ->
            removal?.complete()
            if (removesThread) releaseMovedThread(response)
            applyCoreFolderUnreadChanges(response)
            if ((undoMessage == null || onUndo == null) && doneStatus != null) status = doneStatus
            committed.complete(response)
            afterSuccess?.invoke()
        }.onFailure {
            removal?.rollback()
            Log.w("Mail", "$label failed", it)
            coreThreads = removal?.filter?.invoke(threadsBefore) ?: threadsBefore
            kanbanColumns = kanbanBefore.mapValues { (_, state) -> state.copy(threads = removal?.filter?.invoke(state.threads) ?: state.threads) }
            status = withReason(failedStatus, it.message) { key, args -> trs(key, args) }
            snackbarHost.currentSnackbarData?.dismiss()
            committed.complete(null)
        }
    }
    // Show the undo snackbar immediately rather than gating it on the round-trip,
    // so the undo window starts the moment the user sees the optimistic change.
    if (undoMessage != null && onUndo != null) {
        scope.launch {
            val result =
                snackbarHost.showSnackbar(
                    message = undoMessage,
                    actionLabel = trs("buttons.undo"),
                    duration = SnackbarDuration.Long,
                )
            if (result == SnackbarResult.ActionPerformed) {
                snackbarHost.currentSnackbarData?.dismiss()
                committed.await()?.let { response -> onUndo(response) }
            }
        }
    }
}

private fun MeronMobileState.applyCoreFolderUnreadChanges(response: String) {
    applyCoreFolderUnreadChangesAt(response, null)
}

private fun MeronMobileState.applyCoreFolderUnreadChangesAt(
    response: String,
    started: Long?,
) {
    val changes =
        parseFolderUnreadChanges(response).map { change ->
            change.copy(unread = folderReadGuard.recordMutation(change.accountId, change.folderId, change.unread, started))
        }
    if (changes.isEmpty()) return
    val nextByAccount = foldersByAccount.toMutableMap()
    changes.groupBy { it.accountId }.forEach { (accountId, accountChanges) ->
        val counts = accountChanges.associateBy { it.folderId }
        nextByAccount[accountId] =
            nextByAccount[accountId].orEmpty().map { folder ->
                counts.entries
                    .firstOrNull { (folderId, _) -> folder.name.equals(folderId, ignoreCase = folder.role == "inbox") }
                    ?.value
                    ?.let { folder.copy(unread = it.unread) } ?: folder
            }
    }
    val nextCoreFolders =
        coreFolders.map { folder ->
            changes
                .firstOrNull {
                    it.accountId == folder.accountId && folder.name.equals(it.folderId, ignoreCase = folder.role == "inbox")
                }?.let { folder.copy(unread = it.unread) } ?: folder
        }
    coreFolders = nextCoreFolders
    foldersByAccount = nextByAccount
}

// The drawer's unread badges read the folder caches, not the list rows, so
// clearing rows optimistically on its own left them showing the pre-mark totals
// until the write answered. These apply the same change to the caches up front;
// the response — and the sync that follows — still replace them with the
// server's own numbers.
private fun MeronMobileState.cachedFolderUnread(
    accountId: String,
    folderId: String,
): Int = folderUnread(foldersByAccount[accountId] ?: coreFolders.filter { it.accountId == accountId }, folderId)

private fun MeronMobileState.applyLocalFolderUnread(counts: Map<Pair<String, String>, Int>) {
    if (counts.isEmpty()) return
    val patch = { folder: FolderSummary ->
        counts.entries
            .firstOrNull { (target, _) ->
                target.first == folder.accountId && folder.name.equals(target.second, ignoreCase = folder.role == INBOX_FOLDER)
            }?.let { folder.copy(unread = it.value) } ?: folder
    }
    foldersByAccount = foldersByAccount.mapValues { (_, folders) -> folders.map(patch) }
    coreFolders = coreFolders.map(patch)
}

// Folder totals after marking [mailFolders] read folder-wide and taking the
// [rssRows] marked here off their own folders, paired with the totals they had.
private fun MeronMobileState.plannedFolderUnread(
    mailFolders: List<Pair<String, String>>,
    rssRows: List<ThreadSummary>,
): Pair<Map<Pair<String, String>, Int>, Map<Pair<String, String>, Int>> {
    val next = mutableMapOf<Pair<String, String>, Int>()
    val before = mutableMapOf<Pair<String, String>, Int>()
    mailFolders.forEach { target ->
        before[target] = cachedFolderUnread(target.first, target.second)
        next[target] = 0
    }
    rssRows.forEach { row ->
        val target = row.accountId to row.folder
        if (target.first.isBlank() || target.second.isBlank()) return@forEach
        before.getOrPut(target) { cachedFolderUnread(target.first, target.second) }
        next[target] = (next[target] ?: before.getValue(target)) - 1
    }
    return next.mapValues { (_, value) -> value.coerceAtLeast(0) } to before
}

// Moves a thread back to the folder it was in before an archive/delete and
// restores the pre-action list snapshots, backing the "Undo" snackbar action.
internal fun MeronMobileState.restoreThread(
    thread: ThreadSummary,
    threadsSnapshot: List<ThreadSummary>,
    kanbanSnapshot: Map<String, KanbanColumnState>,
    actionResponse: String,
) {
    if (!coreLoaded) return
    val undoThreadId =
        undoSourceThreadId(thread, actionResponse)
            ?: run {
                status = trs("mobile.status.undoUnavailable")
                return
            }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                requireCoreOk(
                    MobileMailCommandClient(core).move(
                        MoveThreadParams(threadId = undoThreadId, targetFolderId = thread.folder),
                    ),
                )
            }
        }.onSuccess {
            threadRemovalGuard.release(thread.id)
            coreThreads = threadsSnapshot
            kanbanColumns = kanbanSnapshot
            status = trs("mobile.status.restored")
        }.onFailure {
            status = trsError("mobile.status.undoFailed", it)
        }
    }
}

private fun undoSourceThreadId(
    thread: ThreadSummary,
    actionResponse: String,
): String? {
    val location = parseThreadActionLocationResponse(actionResponse)
    if (location.permanent) return null
    if (location.threadId.isNotBlank()) return location.threadId
    if (location.folder.isBlank()) return thread.id
    val threadKey = thread.id.substringAfterLast("#", missingDelimiterValue = "")
    if (thread.accountId.isBlank() || threadKey.isBlank()) return thread.id
    return "${thread.accountId}#${location.folder}#$threadKey"
}

private fun MeronMobileState.refreshUnifiedStarredAfterFlagChange() {
    mailboxCache = mailboxCache.filterKeys { it.accountId != UNIFIED_ACCOUNT_ID || !isUnifiedStarredFolder(it.folderId) }
    if (selectedCoreAccountId != UNIFIED_ACCOUNT_ID || !isUnifiedStarredFolder(selectedCoreFolder)) return
    syncCoreThreads(syncFirst = false)
}

internal fun MeronMobileState.toggleStar(thread: ThreadSummary) {
    val backendThreadId = thread.backendThreadId()
    val isRssThread = threadIdIsRss(backendThreadId)
    val itemIds = listOf(thread.id).takeIf { backendThreadId != thread.id }.orEmpty()
    runCoreThreadAction(
        thread = thread,
        label = if (thread.starred) "Unstar" else "Star",
        doneStatus = trs(if (thread.starred) "mobile.status.starRemoved" else "mobile.status.starAdded"),
        failedStatus = trs(if (thread.starred) "mobile.status.unstarFailed" else "mobile.status.starFailed"),
        action = {
            if (isRssThread) {
                markRssStarred(
                    RssMarkStarredParams(
                        threadId = backendThreadId,
                        starred = !thread.starred,
                        itemKeys = thread.rssItemKeys(),
                    ),
                )
            } else {
                markStarred(MarkStarredParams(threadId = backendThreadId, starred = !thread.starred, messageIds = itemIds))
            }
        },
        update = { threads -> threads.map { if (it.id == thread.id) it.copy(starred = !thread.starred) else it } },
        afterSuccess = {
            refreshUnifiedStarredAfterFlagChange()
        },
    )
}

/**
 * Save a tag change and show the tags the core answers with. Tags are stored
 * locally, so this waits on the database, never the server; a failure means the
 * core refused the tag.
 */
internal fun MeronMobileState.changeThreadTags(
    thread: ThreadSummary,
    add: List<String> = emptyList(),
    remove: List<String> = emptyList(),
    onDone: (List<String>?) -> Unit,
) {
    val backendThreadId = thread.backendThreadId()
    val itemIds = listOf(thread.id).takeIf { backendThreadId != thread.id }.orEmpty()
    scope.launch {
        val tags =
            runCatching {
                parseSetTagsResponse(
                    requireCoreOk(
                        withContext(ioDispatcher) {
                            MobileMailCommandClient(core).setTags(
                                SetTagsParams(threadId = backendThreadId, add = add, remove = remove, messageIds = itemIds),
                            )
                        },
                    ),
                )
            }.getOrNull()
        if (tags != null) {
            updateThreadEverywhere(thread) { it.copy(tags = tags) }
            reloadTagColumns()
        }
        onDone(tags)
    }
}

/** A tag column may gain or lose threads, which no sync announces. */
private fun MeronMobileState.reloadTagColumns() {
    kanbanBoards
        .firstOrNull { it.id == activeKanbanBoardId }
        ?.columns
        ?.filter { kanbanColumnReadsTags(it.copy(query = "${it.query} $kanbanSearch")) }
        ?.forEach { loadKanbanColumn(it) }
}

/**
 * Rename [tag] everywhere, or delete it with no [to], and report to [onDone] whether
 * the core took it. It handles mail and rules; boards live here, so their saved
 * searches go through it to be rewritten. Two columns that come to mean the same
 * search become one, and a search scoped to a column stays on it. On the app's
 * scope, not a dialog's: closing one mid-way must not leave the boards behind.
 */
internal fun MeronMobileState.replaceTag(
    tag: String,
    to: String?,
    onDone: (Boolean) -> Unit,
) {
    val queries = kanbanBoards.flatMap { board -> board.columns.map { it.query } }.filter { it.isNotBlank() }.distinct()
    scope.launch {
        val answer =
            runCatching {
                requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).replaceTag(tag, to, queries) })
            }.getOrNull()
        if (answer != null) applyTagReplacement(tag, parseReplaceTagResponse(answer), queries.zip(parseReplacedQueries(answer)).toMap())
        onDone(answer != null)
    }
}

private fun MeronMobileState.applyTagReplacement(
    tag: String,
    next: String?,
    renamed: Map<String, String>,
) {
    fun swap(thread: ThreadSummary): ThreadSummary =
        if (tag !in thread.tags) {
            thread
        } else {
            thread.copy(tags = (thread.tags - tag + listOfNotNull(next)).distinct().sorted())
        }
    coreThreads = coreThreads.map(::swap)
    selectedCoreThread = selectedCoreThread?.let(::swap)
    kanbanColumns = kanbanColumns.mapValues { (_, state) -> state.copy(threads = state.threads.map(::swap)) }
    // A cached search may no longer list the same mail; a cached mailbox does.
    mailboxCache =
        mailboxCache
            .filterKeys { it.query.isBlank() }
            .mapValues { (_, cached) -> cached.copy(threads = cached.threads.map(::swap)) }

    fun rewrite(column: KanbanColumnSpec) = column.copy(query = renamed[column.query] ?: column.query)
    kanbanBoards
        .flatMap { it.columns }
        .firstOrNull { kanbanColumnKey(it) == kanbanSearchScope }
        ?.let { persistKanbanSearchScope(kanbanColumnKey(rewrite(it))) }
    persistKanbanBoards(
        kanbanBoards.map { board ->
            board.copy(columns = board.columns.map(::rewrite).distinctBy(::kanbanColumnKey))
        },
    )
    val shown = kanbanBoards.flatMap { it.columns }.map(::kanbanColumnKey).toSet()
    kanbanColumns = kanbanColumns.filterKeys { it in shown }
    loadKanbanBoard()
    // Read the list again: a search may gain or lose results, and a read from
    // before this change must not be the last word.
    syncCoreThreads(
        accountOverride = selectedCoreAccountId,
        folderOverride = selectedCoreFolder,
        syncFirst = false,
    )
}

internal suspend fun MeronMobileState.loadTagCounts(): List<TagCount> =
    runCatching {
        parseTagCountsResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).tagList() }))
    }.getOrDefault(emptyList())

/** The saved rules, or null if they could not be read. */
internal suspend fun MeronMobileState.loadRules(): List<Rule>? =
    runCatching {
        parseRulesResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).rules() }))
    }.getOrNull()

/** The rules as the core kept them, or null if it refused one. */
internal suspend fun MeronMobileState.saveRules(rules: List<Rule>): List<Rule>? =
    runCatching {
        parseRulesResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).setRules(rules) }))
    }.getOrNull()

internal suspend fun MeronMobileState.ruleMatches(id: String): Int? =
    runCatching {
        parseRuleCountResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).ruleMatches(id) }))
    }.getOrNull()

internal suspend fun MeronMobileState.runRule(id: String): Int? =
    runCatching {
        parseRuleCountResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).runRule(id) }))
    }.getOrNull()

internal suspend fun MeronMobileState.loadRuleStatus(): RuleStatus? =
    runCatching {
        parseRuleStatusResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).ruleStatus() }))
    }.getOrNull()

internal suspend fun MeronMobileState.stopRule(id: String): Boolean = runCatching { requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).stopRule(id) }) }.isSuccess

internal suspend fun MeronMobileState.loadKnownTags(): List<String> =
    runCatching {
        parseTagListResponse(requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).tagList() }))
    }.getOrDefault(emptyList())

internal fun MeronMobileState.toggleRead(thread: ThreadSummary) {
    val backendThreadId = thread.backendThreadId()
    val isRssThread = threadIdIsRss(backendThreadId)
    val itemIds = listOf(thread.id).takeIf { backendThreadId != thread.id }.orEmpty()
    runCoreThreadAction(
        thread = thread,
        label = if (thread.unread) "Mark read" else "Mark unread",
        doneStatus = trs(if (thread.unread) "mail.toast.markedReadCount" else "mail.toast.markedUnreadCount", mapOf("count" to 1)),
        failedStatus = trs(if (thread.unread) "notification.markReadFailed" else "mobile.status.markUnreadFailed"),
        action = {
            readMarkMutex.withLock {
                if (isRssThread) {
                    markRssRead(RssMarkReadParams(threadId = backendThreadId, seen = thread.unread, itemKeys = thread.rssItemKeys()))
                } else {
                    markRead(MarkReadParams(threadId = backendThreadId, seen = thread.unread, messageIds = itemIds))
                }
            }
        },
        // Marking unread flags the newest message only (see the core), so the
        // card comes back as a single unread message rather than claiming every
        // message in the thread is unread.
        update = { threads ->
            threads.map {
                if (it.id == thread.id) {
                    it.copy(unread = !thread.unread, unreadCount = if (thread.unread) 0 else 1)
                } else {
                    it
                }
            }
        },
    )
}

internal fun MeronMobileState.updateMessageEverywhere(
    messageId: String,
    update: (MessageBody) -> MessageBody,
) {
    messages = messages.map { if (it.id == messageId) update(it) else it }
}

internal fun MeronMobileState.toggleMessageRead(message: MessageBody) {
    val thread = selectedCoreThread ?: return
    val backendThreadId = thread.backendThreadId()
    val seen = message.unread
    val (op, changes) = readLedger.begin(listOf(Triple(message.id, message.folderId, message.unread)), unread = !seen)
    applyReadChanges(thread, changes)
    status = trs(if (seen) "mobile.status.markingRead" else "mobile.status.markingUnread")
    scope.launch {
        readMarkMutex.withLock {
            runCatching {
                requireCoreOk(
                    withContext(ioDispatcher) {
                        val client = MobileMailCommandClient(core)
                        // Feed items carry "<thread>#<item key>" ids the core splits
                        // apart, so one item reads back the same way a message does.
                        if (threadIdIsRss(backendThreadId)) {
                            client.markRssRead(
                                RssMarkReadParams(threadId = backendThreadId, seen = seen, itemKeys = listOf(message.id)),
                            )
                        } else {
                            withManagedGoogleAuth(client, thread.accountId) {
                                client.markRead(
                                    MarkReadParams(
                                        threadId = backendThreadId,
                                        seen = seen,
                                        messageIds = listOf(message.id),
                                        folderId = message.folderId,
                                    ),
                                )
                            }
                        }
                    },
                )
            }.onSuccess {
                applyReadChanges(thread, readLedger.finish(op, succeeded = true))
                status = trs(if (seen) "mail.toast.markedReadCount" else "mail.toast.markedUnreadCount", mapOf("count" to 1))
            }.onFailure {
                applyReadChanges(thread, readLedger.finish(op, succeeded = false))
                Log.w("Mail", "toggle message read failed", it)
                status = trsError("mobile.status.messageUpdateFailed", it)
            }
        }
    }
}

// Scroll-driven read marking, like desktop: local state flips optimistically
// and settles through readLedger, so a failed folder turns its messages unread
// again unless a later write still pending covers them. ThreadUi then holds a
// turned-back message like a hand-unread one until it leaves the viewport, so
// reading it again retries instead of looping on a dead connection. For RSS the
// message ids ("<thread>#<item key>") pass through as item keys; the core
// strips the thread prefix.
internal fun MeronMobileState.markMessagesReadOnScroll(messageIds: List<String>) {
    val thread = selectedCoreThread ?: return
    val backendThreadId = thread.backendThreadId()
    val ids = messageIds.distinct().filter { id -> messages.any { it.id == id && it.unread } }
    if (ids.isEmpty()) return
    // Group by folder here, not inside the coroutine: a thread can span folders
    // and `messages` may be replaced (thread switch, page reload, a rollback from
    // a concurrent action) before the IO block runs, which would silently drop
    // the mark instead of sending it.
    val idsByFolder = messages.filter { it.id in ids }.groupBy { it.folderId }
    // One op per folder: each folder's request succeeds or fails on its own.
    val ops =
        idsByFolder.mapValues { (_, folderMessages) ->
            val (op, changes) = readLedger.begin(folderMessages.map { Triple(it.id, it.folderId, it.unread) }, unread = false)
            applyReadChanges(thread, changes)
            op
        }
    scope.launch {
        readMarkMutex.withLock {
            // One response per folder, each carrying only that folder's unread counts.
            val responses = mutableListOf<String>()
            val failedFolders = mutableSetOf<String>()
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (threadIdIsRss(backendThreadId)) {
                    runCatching {
                        requireCoreOk(client.markRssRead(RssMarkReadParams(threadId = backendThreadId, seen = true, itemKeys = ids)))
                    }.onSuccess { responses += it }
                        .onFailure {
                            Log.w("Mail", "scroll mark read failed", it)
                            failedFolders += idsByFolder.keys
                        }
                } else {
                    idsByFolder.forEach { (folder, folderMessages) ->
                        runCatching {
                            requireCoreOk(
                                withManagedGoogleAuth(client, thread.accountId) {
                                    client.markRead(
                                        MarkReadParams(
                                            threadId = backendThreadId,
                                            seen = true,
                                            messageIds = folderMessages.map { it.id },
                                            folderId = folder,
                                        ),
                                    )
                                },
                            )
                        }.onSuccess { responses += it }
                            .onFailure {
                                Log.w("Mail", "scroll mark read failed", it)
                                failedFolders += folder
                            }
                    }
                }
            }
            // Apply whatever came back, so a later folder failing does not discard the
            // unread counts of the folders that succeeded.
            responses.forEach(::applyCoreFolderUnreadChanges)
            ops.forEach { (folder, op) ->
                applyReadChanges(thread, readLedger.finish(op, succeeded = folder !in failedFolders))
            }
        }
    }
}

// The conversation was viewed to the bottom: mark the whole thread read, which
// also covers unread messages on older pages that were never loaded. The core
// marks every message it still has unread, so the op covers every loaded
// message — including one whose own per-message mark is pending and may fail,
// which this request then reads anyway. Unread on unloaded pages exists only in
// the card's count; that share comes off optimistically and goes back on
// failure.
internal fun MeronMobileState.markThreadReadOnScroll() {
    val thread = selectedCoreThread ?: return
    val backendThreadId = thread.backendThreadId()
    if (!thread.unread && messages.none { it.unread }) return
    val (op, changes) = readLedger.begin(messages.map { Triple(it.id, it.folderId, it.unread) }, unread = false)
    val loadedOnCard = changes.count { it.moved && cardCountsFolder(thread, it.folder) }
    val cardUnread = if (thread.unread) thread.unreadCount.coerceAtLeast(1) else 0
    val unloadedOnCard = (cardUnread - loadedOnCard).coerceAtLeast(0)
    applyReadChanges(thread, changes, extraCardDelta = -unloadedOnCard)
    scope.launch {
        readMarkMutex.withLock {
            runCatching {
                requireCoreOk(
                    withContext(ioDispatcher) {
                        val client = MobileMailCommandClient(core)
                        if (threadIdIsRss(backendThreadId)) {
                            client.markRssRead(RssMarkReadParams(threadId = backendThreadId, seen = true))
                        } else {
                            withManagedGoogleAuth(client, thread.accountId) {
                                client.markRead(MarkReadParams(threadId = backendThreadId, seen = true))
                            }
                        }
                    },
                )
            }.onSuccess { response ->
                applyCoreFolderUnreadChanges(response)
                applyReadChanges(thread, readLedger.finish(op, succeeded = true))
            }.onFailure {
                Log.w("Mail", "thread mark read failed", it)
                applyReadChanges(thread, readLedger.finish(op, succeeded = false), extraCardDelta = unloadedOnCard)
            }
        }
    }
}

// Show read-state changes from readLedger. The loaded messages take the
// reported state outright — they may have been replaced by a reload since the
// ledger last spoke — while the card's count moves only by what the ledger saw
// change, in the card's own mailbox (plus `extraCardDelta`, for unread the card
// counts on pages that are not loaded). Relative, not a snapshot, so unread
// that arrived meanwhile stays counted.
private fun MeronMobileState.applyReadChanges(
    thread: ThreadSummary,
    changes: List<MessageReadLedger.Change>,
    extraCardDelta: Int = 0,
) {
    val shown = changes.associate { it.id to it.unread }
    if (messages.any { message -> shown[message.id]?.let { it != message.unread } == true }) {
        messages = messages.map { message -> shown[message.id]?.let { message.copy(unread = it) } ?: message }
    }
    val delta =
        changes.filter { it.moved && cardCountsFolder(thread, it.folder) }.sumOf { if (it.unread) 1 else -1 } + extraCardDelta
    if (delta == 0) return
    currentThreadSummary(thread.id)?.let { current -> updateThreadEverywhere(current) { threadWithUnreadDelta(it, delta) } }
}

// Whether a message in `folder` counts toward the thread card's unread count.
// That count is mailbox-scoped, so reading a Sent reply must not clear the
// INBOX card's remaining unread state. The card's folder comes from the thread
// id, since `thread.folder` holds the Kanban column id when the thread was
// opened from a column: a role for the unified columns, no mailbox at all for
// starred.
private fun MeronMobileState.cardCountsFolder(
    thread: ThreadSummary,
    folder: String,
): Boolean {
    val cardFolder = mailThreadIdFolder(thread.backendThreadId()).ifBlank { thread.folder }
    return threadCardCoversFolder(cardFolder, foldersByAccount[thread.accountId].orEmpty(), folder)
}

// The live copy of a thread card, which may have moved on since an action
// captured it: the open thread, the list, or a Kanban column.
private fun MeronMobileState.currentThreadSummary(threadId: String): ThreadSummary? =
    selectedCoreThread?.takeIf { it.id == threadId }
        ?: coreThreads.firstOrNull { it.id == threadId }
        ?: kanbanColumns.values.firstNotNullOfOrNull { state -> state.threads.firstOrNull { it.id == threadId } }

internal fun MeronMobileState.toggleMessageStarred(message: MessageBody) {
    val thread = selectedCoreThread ?: return
    val backendThreadId = thread.backendThreadId()
    val starred = !message.starred
    val messagesBefore = messages
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    val selectedBefore = selectedCoreThread
    updateMessageEverywhere(message.id) { it.copy(starred = starred) }
    updateThreadEverywhere(thread) { it.copy(starred = messages.any { message -> message.starred }) }
    status = if (starred) "Starring..." else "Unstarring..."
    scope.launch {
        runCatching {
            requireCoreOk(
                withContext(ioDispatcher) {
                    val client = MobileMailCommandClient(core)
                    if (threadIdIsRss(backendThreadId)) {
                        client.markRssStarred(
                            RssMarkStarredParams(threadId = backendThreadId, starred = starred, itemKeys = listOf(message.id)),
                        )
                    } else {
                        withManagedGoogleAuth(client, thread.accountId) {
                            client.markStarred(
                                MarkStarredParams(
                                    threadId = backendThreadId,
                                    starred = starred,
                                    messageIds = listOf(message.id),
                                    folderId = message.folderId,
                                ),
                            )
                        }
                    }
                },
            )
        }.onSuccess {
            refreshUnifiedStarredAfterFlagChange()
            status = trs(if (starred) "mobile.status.starAdded" else "mobile.status.starRemoved")
        }.onFailure {
            messages = messagesBefore
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            selectedCoreThread = selectedBefore
            status = trsError(if (starred) "mobile.status.starFailed" else "mobile.status.unstarFailed", it)
        }
    }
}

internal fun MeronMobileState.deleteMessage(message: MessageBody) {
    val thread = selectedCoreThread ?: return
    // A thread can span folders (e.g. an INBOX message and its replies in Sent),
    // so delete from the message's own folder, not the thread's nominal folder.
    val messageFolder = message.folderId.ifBlank { thread.folder }
    val messagesBefore = messages
    messages = messages.filterNot { it.id == message.id }
    // Deleting the thread's last draft has to take its "Draft" marker down too.
    // The marker is UI-side state the core's next thread list would rewrite, but
    // returning to the list reloads nothing, so leaving it set keeps the badge
    // on a row whose draft is gone. Another draft still here keeps it on.
    // The reply bar's own saved draft lives on the server without a row in
    // messages until a later read brings one back, so the visible rows alone do
    // not say whether this was the thread's last draft.
    val barHoldsADraftHere =
        quickReplyDraftSaved && quickReplyDraftId.isNotBlank() && quickReplyThreadId == thread.backendThreadId()
    val clearedDraftThreadId =
        thread
            .backendThreadId()
            .takeIf {
                folderIsDrafts(messageFolder) &&
                    !barHoldsADraftHere &&
                    messages.none { other -> folderIsDrafts(other.folderId) }
            }
    clearedDraftThreadId?.let { clearThreadDraftEverywhere(it) }
    status = trs("mobile.status.deletingMessage")
    scope.launch {
        runCatching {
            val response =
                withContext(ioDispatcher) {
                    val client = MobileMailCommandClient(core)
                    withManagedGoogleAuth(client, thread.accountId) {
                        client.delete(
                            ThreadActionParams(
                                threadId = thread.id,
                                folderId = messageFolder,
                                messageIds = listOf(message.id),
                            ),
                        )
                    }
                }
            requireCoreOk(response)
        }.onSuccess {
            status = trs("mobile.status.deleteComplete")
        }.onFailure {
            Log.w("Mail", "delete message failed", it)
            messages = messagesBefore
            clearedDraftThreadId?.let { threadId -> markThreadDraftEverywhere(threadId) }
            status = trsError("mail.toast.deleteFailed", it)
        }
    }
}

internal fun MeronMobileState.markVisibleMailboxAllRead() {
    val unread = coreThreads.filter { it.unread }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val accountsById = coreAccounts.associateBy { it.id }
    val unifiedStarred = selectedCoreAccountId == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(selectedCoreFolder)
    // Starred rows are single items spread across folders, so there is no
    // mailbox to mark: read them item by item, the way their column does.
    val starredTargets =
        if (unifiedStarred) {
            unread
                .filterNot { threadIdIsRss(it.id) }
                .groupBy { it.backendThreadId() }
                .map { (threadId, rows) -> threadId to rows.map { it.id } }
        } else {
            emptyList()
        }
    val unifiedRole = unifiedFolderRole(selectedCoreFolder)
    val unifiedMailAccounts =
        if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID && !unifiedStarred) {
            coreAccounts.filter { it.includedInUnified && !accountSummaryIsRss(it) }
        } else {
            emptyList()
        }
    // The unified view marks the selected role (Sent, Archive, ...) in each
    // account; the core resolves the mailbox itself, and the resolved names
    // here only steer the optimistic badges and rows.
    val mailTargets =
        if (unifiedStarred) {
            emptyList()
        } else if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
            unifiedMailAccounts.mapNotNull { account ->
                val folders = foldersByAccount[account.id] ?: coreFolders.filter { it.accountId == account.id }
                unifiedAccountFolder(folders, unifiedRole)?.let { account.id to it }
            }
        } else {
            val account = accountsById[selectedCoreAccountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(selectedCoreAccountId to selectedCoreFolder) else emptyList()
        }
    val rssTargets = unread.filter { threadIdIsRss(it.backendThreadId()) }
    if (mailTargets.isEmpty() && unifiedMailAccounts.isEmpty() && starredTargets.isEmpty() && rssTargets.isEmpty()) {
        status = trs("mobile.status.noUnreadMessages")
        return
    }
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    val foldersBefore = foldersByAccount
    val coreFoldersBefore = coreFolders
    // Only cards this mark covers: the loaded rows, and anything else in a
    // mailbox marked folder-wide. Other columns keep their unread state.
    val clearedIds = unread.map { it.id }.toSet()
    val inMarkedFolder = { card: ThreadSummary ->
        mailTargets.any { (accountId, folder) ->
            card.accountId == accountId && card.folder.equals(folder, ignoreCase = folder.equals(INBOX_FOLDER, ignoreCase = true))
        }
    }
    coreThreads = coreThreads.map { if (it.unread) it.copy(unread = false) else it }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(
                threads =
                    state.threads.map {
                        if (it.unread && (it.id in clearedIds || inMarkedFolder(it))) it.copy(unread = false) else it
                    },
            )
        }
    applyLocalFolderUnread(plannedFolderUnread(mailTargets, rssTargets).first)
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val responses = mutableListOf<String>()
                starredTargets.forEach { (threadId, messageIds) ->
                    responses += requireCoreOk(client.markRead(MarkReadParams(threadId = threadId, messageIds = messageIds)))
                }
                if (unifiedMailAccounts.isNotEmpty()) {
                    unifiedMailAccounts.forEach { withManagedGoogleAuth(client, it.id) { "" } }
                    responses +=
                        requireCoreAllOk(
                            client.markAllRead(MarkAllReadParams(accountId = UNIFIED_ACCOUNT_ID, folderId = unifiedRole)),
                        )
                } else {
                    mailTargets.forEach { (accountId, folderId) ->
                        responses +=
                            requireCoreOk(
                                withManagedGoogleAuth(client, accountId) {
                                    client.markAllRead(MarkAllReadParams(accountId = accountId, folderId = folderId))
                                },
                            )
                    }
                }
                rssTargets.groupBy { it.backendThreadId() }.forEach { (threadId, rows) ->
                    requireCoreOk(
                        client.markRssRead(
                            RssMarkReadParams(
                                threadId = threadId,
                                seen = true,
                                itemKeys = rows.flatMap { it.rssItemKeys() }.distinct(),
                            ),
                        ),
                    )
                }
                responses
            }
        }.onSuccess { responses ->
            responses.forEach(::applyCoreFolderUnreadChanges)
            status = trs("mobile.ios.markedUnreadItemCountRead", mapOf("count" to unread.size))
            syncCoreThreads(syncFirst = false)
        }.onFailure {
            Log.w("Mail", "mark all read failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            foldersByAccount = foldersBefore
            coreFolders = coreFoldersBefore
            status = trsError("mobile.status.markAllReadFailed", it)
        }
    }
}

private fun MeronMobileState.markKanbanCardsReadElsewhere(
    key: String,
    readIds: Set<String>,
) {
    kanbanColumns =
        kanbanColumns.mapValues { (otherKey, state) ->
            val cleared = state.threads.filter { it.unread && it.id in readIds }
            if (otherKey == key || cleared.isEmpty()) {
                state
            } else {
                state.copy(
                    threads = state.threads.map { if (it.id in readIds) it.copy(unread = false) else it },
                    unreadCount = state.unreadCount?.let { count -> (count - cleared.sumOf { it.unreadCount.coerceAtLeast(1) }).coerceAtLeast(0) },
                )
            }
        }
}

internal fun MeronMobileState.markKanbanColumnAllRead(column: KanbanColumnSpec) {
    markKanbanColumnsAllRead(listOf(column))
}

internal fun MeronMobileState.markKanbanBoardAllRead() {
    val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
    markKanbanColumnsAllRead(board.columns.distinctBy(::kanbanColumnKey))
}

private fun MeronMobileState.markKanbanColumnsAllRead(columns: List<KanbanColumnSpec>) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    if (kanbanMarkingRead || columns.isEmpty()) return
    kanbanMarkingRead = true
    val accounts = coreAccounts
    // Capture every column's targets up front, then clear them all optimistically
    // so the board flips at once. A failed column puts back only its own rows and
    // badge; other columns are never rolled back.
    val plans =
        columns.map { column ->
            val key = kanbanColumnKey(column)
            val state = kanbanColumns[key]
            val unread = state?.threads.orEmpty().filter { it.unread }
            // A saved search marks only its matches read, never the whole folder,
            // so it goes thread by thread like the starred aggregate.
            val perItem = isUnifiedStarredColumn(column) || column.query.isNotBlank()
            val mailAccounts =
                if (perItem) {
                    emptyList()
                } else if (column.accountId == UNIFIED_ACCOUNT_ID) {
                    accounts.filter { it.includedInUnified && !accountSummaryIsRss(it) }
                } else {
                    accounts.filter { it.id == column.accountId && !accountSummaryIsRss(it) }
                }
            val writes =
                mailAccounts.isNotEmpty() ||
                    (perItem && unread.any { !threadIdIsRss(it.id) }) ||
                    unread.any { threadIdIsRss(it.backendThreadId()) }
            val mailFolders =
                mailAccounts.mapNotNull { account ->
                    val folderId =
                        if (column.accountId == UNIFIED_ACCOUNT_ID) {
                            unifiedAccountFolder(foldersByAccount[account.id].orEmpty(), column.folderId)
                        } else {
                            column.folderId
                        }
                    folderId?.let { account.id to it }
                }
            val (folderUnreadNext, folderUnreadBefore) =
                plannedFolderUnread(mailFolders, unread.filter { threadIdIsRss(it.backendThreadId()) })
            KanbanMarkReadPlan(
                column,
                key,
                unread,
                perItem,
                mailAccounts,
                writes,
                state?.unreadCount,
                folderUnreadNext,
                folderUnreadBefore,
            )
        }
    val folderTargets = plans.flatMap { it.folderUnread.keys }.toSet()
    for (plan in plans) {
        val readIds = plan.unread.map { it.id }.toSet()
        updateKanbanColumn(plan.key) { state ->
            state.copy(
                threads = state.threads.map { if (it.id in readIds) it.copy(unread = false) else it },
                unreadCount = if (plan.writes) 0 else state.unreadCount,
            )
        }
        applyLocalFolderUnread(plan.folderUnread)
    }
    val optimisticIds = plans.flatMap { plan -> plan.unread.map { it.id } }.toSet()
    val coreUnreadBefore = coreThreads.filter { it.id in optimisticIds }.associate { it.id to it.unread }
    coreThreads = coreThreads.map { if (it.id in optimisticIds) it.copy(unread = false) else it }
    scope.launch(start = kotlinx.coroutines.CoroutineStart.UNDISPATCHED) {
        folderReadGuard.begin(folderTargets)
        val boardReadVersion = folderReadGuard.version
        val confirmedFolders = mutableMapOf<Pair<String, String>, Int>()
        val succeededIds = mutableSetOf<String>()
        val failedIds = mutableSetOf<String>()
        try {
            val requests = KanbanReadRequests()
            val writeVersions = mutableMapOf<String, Long>()
            var failures = 0
            val marked = mutableSetOf<Pair<String, String>>()
            for (plan in plans) {
                val column = plan.column
                val key = plan.key
                val unread = plan.unread
                val perItem = plan.perItem
                val mailAccounts = plan.mailAccounts
                val unifiedTarget =
                    if (column.accountId != UNIFIED_ACCOUNT_ID && mailAccounts.any { it.includedInUnified }) {
                        // A saved unified search marks only its matches, so it never stands in.
                        columns.firstOrNull {
                            it.accountId == UNIFIED_ACCOUNT_ID && !isUnifiedStarredColumn(it) && it.query.isBlank() &&
                                unifiedColumnMatchesFolder(it.folderId, foldersByAccount[column.accountId].orEmpty(), column.folderId)
                        }
                    } else {
                        null
                    }
                val writeColumn = unifiedTarget ?: column
                val writeVersion = writeVersions.getOrPut(kanbanColumnKey(writeColumn)) { folderReadGuard.version }
                val result =
                    runCatching {
                        withContext(ioDispatcher) {
                            val client = MobileMailCommandClient(core)
                            val responses = mutableListOf<String>()
                            if (mailAccounts.isNotEmpty()) {
                                responses +=
                                    requests.run("folder:${kanbanColumnKey(writeColumn)}") {
                                        val params = MarkAllReadParams(accountId = writeColumn.accountId, folderId = writeColumn.folderId)
                                        val response =
                                            if (writeColumn.accountId == UNIFIED_ACCOUNT_ID) {
                                                accounts
                                                    .filter { it.includedInUnified && !accountSummaryIsRss(it) }
                                                    .forEach { withManagedGoogleAuth(client, it.id) { "" } }
                                                client.markAllRead(params)
                                            } else {
                                                withManagedGoogleAuth(client, writeColumn.accountId) {
                                                    client.markAllRead(params)
                                                }
                                            }
                                        requireCoreAllOk(response)
                                    }
                            }
                            if (perItem) {
                                unread.filterNot { threadIdIsRss(it.id) }.groupBy { it.backendThreadId() }.forEach { (threadId, rows) ->
                                    // A starred card is one message; a saved search's card is the
                                    // whole thread (its id is the thread's), which the core marks
                                    // only when no message ids narrow it, as in toggleRead.
                                    val messageIds =
                                        if (rows.any { it.id == threadId }) emptyList() else rows.map { it.id }.distinct().sorted()
                                    responses +=
                                        requests.run("thread:$threadId:$messageIds") {
                                            withManagedGoogleAuth(client, rows.first().accountId) {
                                                requireCoreOk(client.markRead(MarkReadParams(threadId = threadId, messageIds = messageIds)))
                                            }
                                        }
                                }
                            }
                            unread.filter { threadIdIsRss(it.backendThreadId()) }.groupBy { it.backendThreadId() }.forEach { (threadId, rows) ->
                                val itemKeys = rows.flatMap { it.rssItemKeys() }.distinct().sorted()
                                responses +=
                                    requests.run("rss:$threadId:$itemKeys") {
                                        requireCoreOk(client.markRssRead(RssMarkReadParams(threadId = threadId, seen = true, itemKeys = itemKeys)))
                                    }
                            }
                            responses
                        }
                    }
                result
                    .onSuccess { responses ->
                        // Reapply: a column reload that raced the write may have put
                        // the pre-write rows and badge back over the optimistic clear.
                        val readIds = unread.map { it.id }.toSet()
                        updateKanbanColumn(key) { state ->
                            state.copy(
                                threads = state.threads.map { if (it.id in readIds) it.copy(unread = false) else it },
                                unreadCount = if (plan.writes) 0 else state.unreadCount,
                            )
                        }
                        coreThreads = coreThreads.map { if (it.id in readIds) it.copy(unread = false) else it }
                        // A thread-by-thread write raises no folder change for the
                        // board to reload on, so other columns holding the same
                        // cards (the folder's own, another saved search) follow here.
                        if (plan.perItem) markKanbanCardsReadElsewhere(key, readIds)
                        responses.forEach { applyCoreFolderUnreadChangesAt(it, writeVersion) }
                        val changes = responses.flatMap(::parseFolderUnreadChanges)
                        for ((target, count) in plan.folderUnread) {
                            confirmedFolders[folderReadTarget(target.first, target.second)] = changes
                                .lastOrNull {
                                    it.accountId == target.first && it.folderId.equals(target.second, ignoreCase = target.second.equals(INBOX_FOLDER, true))
                                }?.unread ?: count
                        }
                        applyLocalFolderUnread(
                            plan.folderUnread.mapValues { (target, _) ->
                                folderReadGuard.resolveMutation(target.first, target.second, confirmedFolders.getValue(folderReadTarget(target.first, target.second)), writeVersion)
                            },
                        )
                        succeededIds += readIds
                        marked += unread.map { it.accountId to it.backendThreadId() }
                    }.onFailure {
                        if (it is kotlinx.coroutines.CancellationException) throw it
                        Log.w("Mail", "kanban mark all read failed", it)
                        failures++
                        val readIds = unread.map { it.id }.toSet()
                        failedIds += readIds
                        updateKanbanColumn(key) { state ->
                            state.copy(
                                threads = state.threads.map { if (it.id in readIds) it.copy(unread = true) else it },
                                unreadCount = if (plan.writes) plan.unreadCountBefore else state.unreadCount,
                            )
                        }
                        applyLocalFolderUnread(
                            plan.folderUnreadBefore.mapValues { (target, previous) ->
                                folderReadGuard.resolveMutation(target.first, target.second, confirmedFolders[folderReadTarget(target.first, target.second)] ?: previous, boardReadVersion)
                            },
                        )
                    }
            }
            // A row shared with a column that did succeed is read on the server.
            val revertIds = failedIds - succeededIds
            if (revertIds.isNotEmpty()) {
                coreThreads = coreThreads.map { thread -> coreUnreadBefore[thread.id]?.takeIf { thread.id in revertIds }?.let { thread.copy(unread = it) } ?: thread }
            }
            val language = loadAppLanguageTag(prefs).ifBlank { "en" }
            status =
                if (failures > 0) {
                    localizedString(language, "notification.markReadFailed")
                } else {
                    localizedString(language, "mail.toast.markedReadCount", mapOf("count" to marked.size))
                }
        } finally {
            folderReadGuard.end(folderTargets)
            kanbanMarkingRead = false
        }
    }
}

// Permanently delete every message in a Trash or Junk folder. The core re-checks
// the folder role, so a stale menu can never empty anything else. Callers confirm
// first: there is no Trash left to restore from.
internal fun MeronMobileState.emptyMailFolder(
    accountId: String,
    folderId: String,
    column: KanbanColumnSpec? = null,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val account = coreAccounts.firstOrNull { it.id == accountId }
    if (accountId == UNIFIED_ACCOUNT_ID || account == null || accountSummaryIsRss(account)) return

    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    val removal = suppressEmptiedFolder(accountId, folderId)
    val inFolder = { thread: ThreadSummary -> thread.accountId == accountId && thread.folder == folderId }
    coreThreads = coreThreads.filterNot(inFolder)
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.filterNot(inFolder))
        }
    selectedMailThreadIds = emptySet()
    mailSelectionMenuOpen = false

    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                requireCoreOk(
                    withManagedGoogleAuth(client, accountId) {
                        client.emptyFolder(EmptyFolderParams(accountId = accountId, folderId = folderId))
                    },
                )
            }
        }.onSuccess { response ->
            removal.complete()
            applyCoreFolderUnreadChanges(response)
            status = trs("mobile.status.folderEmptied")
            if (column != null) loadKanbanColumn(column, refresh = true) else syncCoreThreads(syncFirst = false)
        }.onFailure {
            removal.rollback()
            Log.w("Mail", "empty folder failed", it)
            coreThreads = removal.filter(threadsBefore)
            kanbanColumns = kanbanBefore.mapValues { (_, state) -> state.copy(threads = removal.filter(state.threads)) }
            status = trsError("mobile.status.emptyFolderFailed", it)
        }
    }
}

// Delete a folder on the server, with everything nested under it, their mail and
// any board column showing one of them. The core re-checks that no special-use
// folder is in the subtree, so a stale menu can never delete Sent or Archive.
// Callers confirm first: the server keeps no copy of what the folders held.
internal fun MeronMobileState.deleteMailFolder(
    accountId: String,
    folderId: String,
    column: KanbanColumnSpec? = null,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val account = coreAccounts.firstOrNull { it.id == accountId }
    if (accountId == UNIFIED_ACCOUNT_ID || account == null || accountSummaryIsRss(account)) return

    // The delete takes the subtree with it, so the local cleanup below has to
    // cover every folder under it, not just the one the menu named.
    val expectedRemoved =
        (nestedFolders(foldersByAccount[accountId].orEmpty(), accountId, folderId).map { it.name } + folderId)
            .toSet()

    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val response =
                    requireCoreOk(
                        withManagedGoogleAuth(client, accountId) {
                            client.deleteFolder(FolderDeleteParams(accountId = accountId, folderId = folderId))
                        },
                    )
                parseFolderDeleteResponse(response) to loadAccountFolders(client, account)
            }
        }.onSuccess { (result, folders) ->
            // New cores report the exact successful prefix when a later DELETE
            // fails. Older cores omit it after a complete success.
            val removed = result.removed.ifEmpty { expectedRemoved }
            foldersByAccount = foldersByAccount + (accountId to folders)
            coreFolders = coreFolders.filterNot { it.accountId == accountId && it.name in removed }
            val inFolder = { thread: ThreadSummary -> thread.accountId == accountId && thread.folder in removed }
            coreThreads = coreThreads.filterNot(inFolder)
            selectedMailThreadIds = emptySet()
            mailSelectionMenuOpen = false
            removed.forEach { removeKanbanColumnsForFolder(accountId, it) }
            // Core stopped watching the deleted folders; have the live-push
            // service drop the ones it was counting on.
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
            // The mailbox view may have been sitting in any folder that just went away.
            if (selectedCoreAccountId == accountId && selectedCoreFolder in removed) {
                selectCoreMailbox(accountId, INBOX_FOLDER)
                syncCoreThreads(accountOverride = accountId, folderOverride = INBOX_FOLDER, syncFirst = false)
            }
            status = result.warning ?: trs("mobile.status.folderDeleted")
        }.onFailure {
            Log.w("Mail", "delete folder failed", it)
            status = trsError("mobile.status.deleteFolderFailed", it)
        }
    }
}

/** Ask for a new folder's name in [account]; see [createMailFolder]. */
internal fun MeronMobileState.promptNewFolder(
    account: AccountSummary,
    placement: NewFolderPlacement,
) {
    folderNameInput = ""
    pendingNewFolder = NewFolderTarget(account, placement)
}

// Create a folder on the server and show it where the action asked for it: a
// new board column, in place of a column, or in the mail view.
internal fun MeronMobileState.createMailFolder(
    target: NewFolderTarget,
    name: String,
) {
    val trimmed = name.trim()
    if (trimmed.isBlank()) {
        status = trs("folders.nameRequired")
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val account = target.account
    scope.launch {
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                withManagedGoogleAuth(client, account.id) {
                    client.createFolder(FolderCreateParams(accountId = account.id, name = trimmed))
                }
                loadAccountFolders(client, account)
            }
        }.onSuccess { folders ->
            val reconciled = reconcileFolderUnread(folders, folderReadVersion)
            foldersByAccount = foldersByAccount + (account.id to reconciled)
            if (selectedCoreAccountId == account.id) coreFolders = reconciled
            val created = folders.folderCreatedAs(trimmed)?.name ?: trimmed
            // A slow server may answer after the user went elsewhere; the folder
            // is made either way, but the view stays where they are.
            when (val placement = target.placement) {
                is NewFolderPlacement.AddColumn -> {
                    if (activeKanbanBoardId == placement.boardId) {
                        addKanbanColumn(KanbanColumnSpec(account.id, created))
                    }
                }

                is NewFolderPlacement.SwitchColumn -> {
                    if (activeKanbanBoardId == placement.boardId) {
                        switchKanbanColumnFolder(placement.column, created)
                    }
                }

                is NewFolderPlacement.OpenMailbox -> {
                    if (selectedCoreAccountId == account.id && selectedCoreFolder == placement.folderId) {
                        selectCoreMailbox(account.id, created)
                        syncCoreThreads(accountOverride = account.id, folderOverride = created, syncFirst = false)
                    }
                }
            }
            if (pendingNewFolder == target) pendingNewFolder = null
            folderNameInput = ""
            status = trs("mobile.status.folderCreated")
        }.onFailure {
            status = trsError("mobile.status.createFolderFailed", it)
        }
    }
}

/** Ask for [folder]'s new name; see [renameMailFolder]. */
internal fun MeronMobileState.promptRenameFolder(folder: FolderSummary) {
    val leaf = folderLeafName(folder)
    folderNameInput = leaf
    pendingRenameFolder =
        RenameFolderTarget(
            accountId = folder.accountId,
            folderId = folder.name,
            folderName = leaf,
            parent = folder.displayName.removeSuffix(leaf),
        )
}

// Rename a folder on the server, subfolders included. The folder stays where it
// is nested; only its own name changes. Whatever showed a folder that moved
// follows it: the mail view, board columns and recent move/copy targets. The
// core re-checks that no special-use folder is in the subtree.
internal fun MeronMobileState.renameMailFolder(
    target: RenameFolderTarget,
    name: String,
) {
    val trimmed = name.trim()
    if (trimmed.isBlank()) {
        status = trs("folders.nameRequired")
        return
    }
    if (trimmed == target.folderName) {
        pendingRenameFolder = null
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val accountId = target.accountId
    val account = coreAccounts.firstOrNull { it.id == accountId }
    if (accountId == UNIFIED_ACCOUNT_ID || account == null || accountSummaryIsRss(account)) return

    scope.launch {
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val response =
                    requireCoreOk(
                        withManagedGoogleAuth(client, accountId) {
                            client.renameFolder(
                                FolderRenameParams(
                                    accountId = accountId,
                                    folderId = target.folderId,
                                    name = target.parent + trimmed,
                                ),
                            )
                        },
                    )
                parseFolderRenameResponse(response) to loadAccountFolders(client, account)
            }
        }.onSuccess { (renames, folders) ->
            // Mailbox and column reads still in flight hold the old names.
            mailboxFolderGeneration += 1
            val reconciled = reconcileFolderUnread(folders, folderReadVersion)
            foldersByAccount = foldersByAccount + (accountId to reconciled)
            if (selectedCoreAccountId == accountId) coreFolders = reconciled
            // A thread's id names its old folder, and the core fetches the
            // folder's mail again under the new name.
            coreThreads = coreThreads.filterNot { it.accountId == accountId && it.folder in renames }
            // So does an open conversation's, wherever it was opened from; a
            // read still in flight for it is dropped along with it.
            if (selectedCoreThread?.let { it.accountId == accountId && it.folder in renames } == true) {
                selectedCoreThread = null
                messages = emptyList()
            }
            selectedMailThreadIds = emptySet()
            mailSelectionMenuOpen = false
            renameKanbanColumnsForFolder(accountId, renames)
            renameRecentTransferTargets(accountId, renames)
            renameMailFolderForAccount(prefs, accountId, renames)
            // The core moved the opt-ins along; have the live-push service
            // watch the new names.
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
            // Cached mailboxes hold the old folder list (and threads under the
            // old names), which restoring one would write back.
            mailboxCache = mailboxCache.filterKeys { it.accountId != accountId && it.accountId != UNIFIED_ACCOUNT_ID }
            val renamedSelected = renames[selectedCoreFolder].takeIf { selectedCoreAccountId == accountId }
            if (renamedSelected != null) {
                selectCoreMailbox(accountId, renamedSelected)
                // The rename dropped the folder's cached mail, so fetch it from
                // the server rather than reading an empty cache.
                syncCoreThreads(accountOverride = accountId, folderOverride = renamedSelected)
            } else if (selectedCoreAccountId == accountId || selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                // Supersedes any read in flight, which would show the old names.
                syncCoreThreads(syncFirst = false)
            }
            resyncRenamedFolders(account, renames.values - setOfNotNull(renamedSelected))
            if (pendingRenameFolder == target) pendingRenameFolder = null
            folderNameInput = ""
            status = trs("folders.rename.done", mapOf("folder" to trimmed))
        }.onFailure {
            Log.w("Mail", "rename folder failed", it)
            status = trsError("folders.rename.failed", it, mapOf("folder" to target.folderName))
        }
    }
}

// The rename dropped every moved folder's cached mail, so their unread counts
// and starred mail are missing until each is fetched again: fetch them in the
// background, then refresh the folder list's counts and the views showing
// them. Best effort; opening a folder fetches it anyway.
private fun MeronMobileState.resyncRenamedFolders(
    account: AccountSummary,
    folderIds: Collection<String>,
) {
    if (folderIds.isEmpty() || account.paused || account.needsReconnect) return
    scope.launch {
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                folderIds.forEach { folderId ->
                    runCatching {
                        withManagedGoogleAuth(client, account.id) {
                            client.sync(SyncMailParams(accountId = account.id, folderId = folderId, folders = false))
                        }
                    }.onFailure { Log.w("Mail", "resync renamed folder failed", it) }
                }
                loadAccountFolders(client, account)
            }
        }.onSuccess { folders ->
            val reconciled = reconcileFolderUnread(folders, folderReadVersion)
            foldersByAccount = foldersByAccount + (account.id to reconciled)
            if (selectedCoreAccountId == account.id) coreFolders = reconciled
            // These syncs raise no mail event, so re-read the views that can
            // show the fetched mail (Starred, unified) from the cache.
            if (selectedCoreAccountId == account.id || selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
                syncCoreThreads(syncFirst = false)
            }
            kanbanBoards
                .firstOrNull { it.id == activeKanbanBoardId }
                ?.columns
                ?.filter { it.accountId == account.id || it.accountId == UNIFIED_ACCOUNT_ID }
                ?.forEach { loadKanbanColumn(it) }
        }
    }
}

/** Opt a folder in to (or out of) background sync and new-mail notifications. */
internal fun MeronMobileState.setMailFolderNotify(
    accountId: String,
    folderId: String,
    folderName: String,
    enabled: Boolean,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val account = coreAccounts.firstOrNull { it.id == accountId }
    if (accountId == UNIFIED_ACCOUNT_ID || account == null || accountSummaryIsRss(account)) return

    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                parseFolderListResponse(
                    requireCoreOk(
                        client.setFolderNotify(
                            FolderSetNotifyParams(accountId = accountId, folderId = folderId, enabled = enabled),
                        ),
                    ),
                )
            }
        }.onSuccess { folders ->
            foldersByAccount = foldersByAccount + (accountId to folders)
            coreFolders =
                coreFolders.map {
                    if (it.accountId == accountId && it.name == folderId) it.copy(notify = enabled) else it
                }
            // The live-push service watches opted-in folders; have it re-read them.
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
            status =
                trs(
                    if (enabled) "folders.notify.enabled" else "folders.notify.disabled",
                    mapOf("folder" to folderName),
                )
        }.onFailure {
            Log.w("Mail", "set folder notify failed", it)
            status = it.message ?: trs("folders.notify.failed", mapOf("folder" to folderName))
        }
    }
}

internal fun MeronMobileState.archiveOrRemove(thread: ThreadSummary) {
    if (threadIdIsRss(thread.id)) {
        if (thread.rssItemKeys().isNotEmpty()) {
            status = trs("mobile.status.rssItemsCannotBeRemoved")
            return
        }
        runCoreThreadAction(
            thread = thread,
            label = "Remove feed",
            doneStatus = trs("feeds.toast.feedRemoved"),
            failedStatus = trs("feeds.toast.failedToRemove"),
            removesThread = true,
            action = { removeRssFeed(RemoveRssFeedParams(threadId = thread.id)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
            afterSuccess = {
                syncCoreThreads(
                    accountOverride = thread.accountId,
                    folderOverride = INBOX_FOLDER,
                    syncFirst = false,
                    successStatus = trs("feeds.toast.feedRemoved"),
                )
            },
        )
    } else {
        val threadsSnapshot = coreThreads
        val kanbanSnapshot = kanbanColumns
        runCoreThreadAction(
            thread = thread,
            label = "Archive",
            doneStatus = null,
            failedStatus = trs("mail.toast.archiveFailed"),
            removesThread = true,
            action = { archive(ThreadActionParams(threadId = thread.id)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
            undoMessage = trs("mail.toast.archivedCount", mapOf("count" to 1)),
            onUndo = { response -> restoreThread(thread, threadsSnapshot, kanbanSnapshot, response) },
        )
    }
}

internal fun MeronMobileState.deleteThread(thread: ThreadSummary) {
    if (threadIdIsRss(thread.backendThreadId())) {
        status = trs(if (thread.rssItemKeys().isNotEmpty()) "mobile.status.rssItemsCannotBeDeleted" else "mobile.status.useRemoveFeed")
        return
    }
    val threadsSnapshot = coreThreads
    val kanbanSnapshot = kanbanColumns
    runCoreThreadAction(
        thread = thread,
        label = "Delete",
        doneStatus = null,
        failedStatus = trs("mail.toast.deleteFailed"),
        removesThread = true,
        action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
        update = { threads -> threads.filterNot { it.id == thread.id } },
        undoMessage = trs(threadDeletedStatusKey(thread.folder, thread.folderRole), mapOf("count" to 1)),
        onUndo = { response -> restoreThread(thread, threadsSnapshot, kanbanSnapshot, response) },
    )
}
