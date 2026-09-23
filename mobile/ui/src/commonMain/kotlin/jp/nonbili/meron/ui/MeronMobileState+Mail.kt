package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.ui.input.key.key
import jp.nonbili.meron.shared.EmptyFolderParams
import jp.nonbili.meron.shared.FolderDeleteParams
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
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.mailThreadIdFolder
import jp.nonbili.meron.shared.parseFolderDeleteResponse
import jp.nonbili.meron.shared.parseFolderUnreadChanges
import jp.nonbili.meron.shared.parseThreadActionLocationResponse
import jp.nonbili.meron.shared.requireCoreAllOk
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

internal fun MeronMobileState.runCoreThreadAction(
    thread: ThreadSummary,
    label: String,
    action: suspend MobileMailCommandClient.() -> String,
    update: (List<ThreadSummary>) -> List<ThreadSummary>,
    undoMessage: String? = null,
    onUndo: ((String) -> Unit)? = null,
    afterSuccess: (() -> Unit)? = null,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    // Apply optimistically so the UI reacts instantly, then revert if the core
    // call fails. Snapshots taken here back the failure rollback.
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    coreThreads = update(coreThreads).forStarredView(selectedCoreAccountId, selectedCoreFolder)
    kanbanColumns =
        kanbanColumns.mapValues { (key, state) ->
            val nextThreads = update(state.threads).forStarredView(key.substringBefore("\n"), key.substringAfter("\n"))
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
            applyCoreFolderUnreadChanges(response)
            if (undoMessage == null || onUndo == null) status = "$label complete"
            committed.complete(response)
            afterSuccess?.invoke()
        }.onFailure {
            Log.w("Mail", "$label failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            status = "$label failed: ${it.message}"
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
                    actionLabel = "Undo",
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
                status = "Undo unavailable"
                return
            }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).move(
                    MoveThreadParams(threadId = undoThreadId, targetFolderId = thread.folder),
                )
            }
        }.onSuccess {
            coreThreads = threadsSnapshot
            kanbanColumns = kanbanSnapshot
            status = "Restored"
        }.onFailure {
            status = "Undo failed: ${it.message}"
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

internal fun MeronMobileState.toggleRead(thread: ThreadSummary) {
    val backendThreadId = thread.backendThreadId()
    val isRssThread = threadIdIsRss(backendThreadId)
    val itemIds = listOf(thread.id).takeIf { backendThreadId != thread.id }.orEmpty()
    runCoreThreadAction(
        thread = thread,
        label = if (thread.unread) "Mark read" else "Mark unread",
        action = {
            if (isRssThread) {
                markRssRead(RssMarkReadParams(threadId = backendThreadId, seen = thread.unread, itemKeys = thread.rssItemKeys()))
            } else {
                markRead(MarkReadParams(threadId = backendThreadId, seen = thread.unread, messageIds = itemIds))
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
    val messagesBefore = messages
    val selectedBefore = selectedCoreThread
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    updateMessageEverywhere(message.id) { it.copy(unread = !seen) }
    val updatedUnread = messages.any { it.unread }
    updateThreadEverywhere(thread) { it.copy(unread = updatedUnread) }
    selectedCoreThread = selectedCoreThread?.copy(unread = updatedUnread)
    status = if (seen) "Marking read..." else "Marking unread..."
    scope.launch {
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
            status = if (seen) "Marked read" else "Marked unread"
        }.onFailure {
            messages = messagesBefore
            selectedCoreThread = selectedBefore
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            Log.w("Mail", "toggle message read failed", it)
            status = "Message update failed: ${it.message}"
        }
    }
}

// Scroll-driven read marking. Best-effort like desktop: local state flips
// optimistically and failures are only logged — the messages stay unread in
// the core and get re-sent by a later scroll or thread-level mark. For RSS
// the message ids ("<thread>#<item key>") pass through as item keys; the core
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
    // A card's unread count is mailbox-scoped, so only the messages in the card's
    // own folder come off it — reading a Sent reply must not clear the INBOX
    // card's remaining unread state. That folder comes from the thread id, since
    // `thread.folder` holds the Kanban column id when the thread was opened from
    // a column: a role for the unified columns, no mailbox at all for starred.
    val cardFolder = mailThreadIdFolder(backendThreadId).ifBlank { thread.folder }
    val cardFolders = foldersByAccount[thread.accountId].orEmpty()
    val readInThreadFolder =
        idsByFolder.entries.sumOf { (folder, folderMessages) ->
            if (threadCardCoversFolder(cardFolder, cardFolders, folder)) folderMessages.size else 0
        }
    messages = messages.map { if (it.id in ids) it.copy(unread = false) else it }
    updateThreadEverywhere(thread) { threadAfterMessagesRead(it, readInThreadFolder) }
    scope.launch {
        // One response per folder, each carrying only that folder's unread counts.
        val responses = mutableListOf<String>()
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (threadIdIsRss(backendThreadId)) {
                    responses +=
                        requireCoreOk(
                            client.markRssRead(RssMarkReadParams(threadId = backendThreadId, seen = true, itemKeys = ids)),
                        )
                } else {
                    withManagedGoogleAuth(client, thread.accountId) {
                        idsByFolder.forEach { (folder, folderMessages) ->
                            responses +=
                                requireCoreOk(
                                    client.markRead(
                                        MarkReadParams(
                                            threadId = backendThreadId,
                                            seen = true,
                                            messageIds = folderMessages.map { it.id },
                                            folderId = folder,
                                        ),
                                    ),
                                )
                        }
                        ""
                    }
                }
            }
        }.onFailure {
            Log.w("Mail", "scroll mark read failed", it)
        }
        // Apply whatever came back, so a later folder failing does not discard the
        // unread counts of the folders that succeeded.
        responses.forEach(::applyCoreFolderUnreadChanges)
    }
}

// The conversation was viewed to the bottom: mark the whole thread read, which
// also covers unread messages on older pages that were never loaded.
internal fun MeronMobileState.markThreadReadOnScroll() {
    val thread = selectedCoreThread ?: return
    val backendThreadId = thread.backendThreadId()
    if (!thread.unread && messages.none { it.unread }) return
    messages = messages.map { if (it.unread) it.copy(unread = false) else it }
    updateThreadEverywhere(thread) { it.copy(unread = false, unreadCount = 0) }
    scope.launch {
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
        }.onFailure {
            Log.w("Mail", "thread mark read failed", it)
        }
    }
}

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
            status = if (starred) "Starred" else "Unstarred"
        }.onFailure {
            messages = messagesBefore
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            selectedCoreThread = selectedBefore
            status = "Star failed: ${it.message}"
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
    status = "Deleting message..."
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
            status = "Delete complete"
        }.onFailure {
            Log.w("Mail", "delete message failed", it)
            messages = messagesBefore
            clearedDraftThreadId?.let { threadId -> markThreadDraftEverywhere(threadId) }
            status = "Delete failed: ${it.message}"
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
        status = "No unread messages."
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
            status = "Marked ${unread.size} unread item(s) read"
            syncCoreThreads(syncFirst = false)
        }.onFailure {
            Log.w("Mail", "mark all read failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            foldersByAccount = foldersBefore
            coreFolders = coreFoldersBefore
            status = "Mark all read failed: ${it.message}"
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
            val starred = isUnifiedStarredColumn(column)
            val mailAccounts =
                if (starred) {
                    emptyList()
                } else if (column.accountId == UNIFIED_ACCOUNT_ID) {
                    accounts.filter { it.includedInUnified && !accountSummaryIsRss(it) }
                } else {
                    accounts.filter { it.id == column.accountId && !accountSummaryIsRss(it) }
                }
            val writes =
                mailAccounts.isNotEmpty() ||
                    (starred && unread.any { !threadIdIsRss(it.id) }) ||
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
                starred,
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
                val starred = plan.starred
                val mailAccounts = plan.mailAccounts
                val unifiedTarget =
                    if (column.accountId != UNIFIED_ACCOUNT_ID && mailAccounts.any { it.includedInUnified }) {
                        columns.firstOrNull {
                            it.accountId == UNIFIED_ACCOUNT_ID && !isUnifiedStarredColumn(it) &&
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
                            if (starred) {
                                unread.filterNot { threadIdIsRss(it.id) }.groupBy { it.backendThreadId() }.forEach { (threadId, rows) ->
                                    val messageIds = rows.map { it.id }.distinct().sorted()
                                    responses +=
                                        requests.run("thread:$threadId:$messageIds") {
                                            requireCoreOk(client.markRead(MarkReadParams(threadId = threadId, messageIds = messageIds)))
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
            applyCoreFolderUnreadChanges(response)
            status = "Folder emptied"
            if (column != null) loadKanbanColumn(column, refresh = true) else syncCoreThreads(syncFirst = false)
        }.onFailure {
            Log.w("Mail", "empty folder failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            status = "Empty folder failed: ${it.message}"
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
            // The mailbox view may have been sitting in any folder that just went away.
            if (selectedCoreAccountId == accountId && selectedCoreFolder in removed) {
                selectCoreMailbox(accountId, INBOX_FOLDER)
                syncCoreThreads(accountOverride = accountId, folderOverride = INBOX_FOLDER, syncFirst = false)
            }
            status = result.warning ?: "Folder deleted"
        }.onFailure {
            Log.w("Mail", "delete folder failed", it)
            status = "Delete folder failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.archiveOrRemove(thread: ThreadSummary) {
    if (threadIdIsRss(thread.id)) {
        if (thread.rssItemKeys().isNotEmpty()) {
            status = "RSS items cannot be removed."
            return
        }
        runCoreThreadAction(
            thread = thread,
            label = "Remove feed",
            action = { removeRssFeed(RemoveRssFeedParams(threadId = thread.id)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
            afterSuccess = {
                syncCoreThreads(
                    accountOverride = thread.accountId,
                    folderOverride = INBOX_FOLDER,
                    syncFirst = false,
                    successStatus = "Feed removed",
                )
            },
        )
    } else {
        val threadsSnapshot = coreThreads
        val kanbanSnapshot = kanbanColumns
        runCoreThreadAction(
            thread = thread,
            label = "Archive",
            action = { archive(ThreadActionParams(threadId = thread.id)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
            undoMessage = "Archived",
            onUndo = { response -> restoreThread(thread, threadsSnapshot, kanbanSnapshot, response) },
        )
    }
}

internal fun MeronMobileState.deleteThread(thread: ThreadSummary) {
    if (threadIdIsRss(thread.backendThreadId())) {
        status = if (thread.rssItemKeys().isNotEmpty()) "RSS items cannot be deleted." else "Use Remove feed for RSS feeds."
        return
    }
    val threadsSnapshot = coreThreads
    val kanbanSnapshot = kanbanColumns
    runCoreThreadAction(
        thread = thread,
        label = threadDeleteActionLabel(thread.folder, thread.folderRole),
        action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
        update = { threads -> threads.filterNot { it.id == thread.id } },
        undoMessage = "Deleted",
        onUndo = { response -> restoreThread(thread, threadsSnapshot, kanbanSnapshot, response) },
    )
}
