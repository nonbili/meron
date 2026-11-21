package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private fun List<FolderSummary>.hasOnlyBootstrapInbox(): Boolean = size == 1 && first().name.equals(INBOX_FOLDER, ignoreCase = true)

internal fun MeronMobileState.spamFolderForThread(thread: ThreadSummary): String? {
    if (threadIdIsRss(thread.id) || thread.folderRole == "drafts") return null
    val folders = foldersByAccount[thread.accountId].orEmpty()
    val junk = folders.firstOrNull { it.specialUse == "junk" }?.name ?: unifiedAccountFolder(folders, "junk")
    return junk?.takeUnless { kanbanFolderIdsEqual(it, thread.folder) }
}

internal fun MeronMobileState.markThreadAsSpam(
    thread: ThreadSummary,
    onMoved: () -> Unit = {},
) {
    val folder = spamFolderForThread(thread) ?: return
    moveThreadToFolder(thread, folder) {
        refreshKanbanColumnsForAccount(thread.accountId)
        onMoved()
    }
}

// These moves remove rows on success. A read while the move was pending may
// already have hidden the source row, so a failure needs a fresh cache read.
private fun MeronMobileState.reloadFailedMoveSource(thread: ThreadSummary) {
    if (selectedCoreAccountId == thread.accountId || selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
        syncCoreThreads(syncFirst = false)
    }
    refreshKanbanColumnsForAccount(thread.accountId)
}

internal fun MeronMobileState.ensureThreadActionFolders(
    thread: ThreadSummary,
    includeAllMailAccounts: Boolean,
    onReady: () -> Unit,
) {
    if (threadIdIsRss(thread.id)) {
        status = trs("mobile.status.rssMoveBetweenAccounts")
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val accounts =
        if (includeAllMailAccounts) {
            coreAccounts.filterNot { accountSummaryIsRss(it) }
        } else {
            coreAccounts.filter { it.id == thread.accountId && !accountSummaryIsRss(it) }
        }
    if (accounts.isEmpty()) {
        status = trs("mobile.status.noMailFolders")
        return
    }
    status = trs("folders.loading")
    scope.launch {
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                accounts.associate { account ->
                    var folders = loadAccountFolders(client, account)
                    if (folders.hasOnlyBootstrapInbox()) {
                        withManagedGoogleAuth(client, account.id) {
                            client.sync(
                                SyncMailParams(
                                    accountId = account.id,
                                    folderId = INBOX_FOLDER,
                                    limit = 1,
                                    folders = true,
                                    deferTail = true,
                                ),
                            )
                        }
                        folders = loadAccountFolders(client, account)
                    }
                    account.id to folders
                }
            }
        }.onSuccess { loadedFolders ->
            foldersByAccount = foldersByAccount + loadedFolders.mapValues { (_, folders) -> reconcileFolderUnread(folders, folderReadVersion) }
            status = trs("mobile.status.foldersLoaded")
            onReady()
        }.onFailure {
            status = trsError("mobile.status.loadFoldersFailed", it)
        }
    }
}

internal fun MeronMobileState.moveThreadToFolder(
    thread: ThreadSummary,
    targetFolderId: String,
    targetAccountId: String = thread.accountId,
    onMoved: () -> Unit = {},
) {
    if (threadIdIsRss(thread.id)) {
        status = trs("mobile.status.rssMoveBetweenAccounts")
        return
    }
    if (targetAccountId == thread.accountId && kanbanFolderIdsEqual(targetFolderId, thread.folder)) {
        status = trs("mobile.status.alreadyInFolder", mapOf("folder" to targetFolderId.replaceFirstChar { it.uppercase() }))
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    val selectedBefore = selectedCoreThread
    val messagesBefore = messages
    val removal = suppressRemovedThread(thread.id)
    // Remove the row optimistically, but keep an open conversation selected
    // until the move succeeds. Clearing it here makes the thread-route fallback
    // pop immediately, then the success callback pops the origin route too.
    coreThreads = coreThreads.filterNot { it.id == thread.id }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.filterNot { it.id == thread.id })
        }
    status = trs("mobile.status.moving")
    scope.launch {
        runCatching {
            requireTransferAffected(
                requireCoreOk(
                    withContext(ioDispatcher) {
                        MobileMailCommandClient(core).move(
                            MoveThreadParams(threadId = thread.id, targetFolderId = targetFolderId, targetAccountId = targetAccountId),
                        )
                    },
                ),
                "moved",
            )
        }.onSuccess { response ->
            removal.complete()
            releaseMovedThread(response)
            applyCoreFolderUnreadChanges(response)
            if (selectedCoreThread?.id == thread.id) {
                selectedCoreThread = null
                messages = emptyList()
            }
            status = trs("mobile.status.moveComplete")
            rememberThreadTransfer(RecentThreadTransfer("move", thread.accountId, targetAccountId, targetFolderId))
            onMoved()
        }.onFailure {
            removal.rollback()
            Log.w("Mail", "move thread failed", it)
            coreThreads = removal.filter(threadsBefore)
            kanbanColumns = kanbanBefore.mapValues { (_, state) -> state.copy(threads = removal.filter(state.threads)) }
            selectedCoreThread = selectedBefore
            messages = messagesBefore
            showTransferFailure("mail.toast.moveFailed", it)
        }
    }
}

internal fun MeronMobileState.copyThreadToFolder(
    thread: ThreadSummary,
    target: FolderSummary,
) {
    if (threadIdIsRss(thread.id)) {
        status = trs("mobile.status.rssCannotCopyToMail")
        return
    }
    val targetAccountId = target.accountId.ifBlank { thread.accountId }
    val targetAccount = coreAccounts.firstOrNull { it.id == targetAccountId }
    if (targetAccount == null || accountSummaryIsRss(targetAccount)) {
        status = trs("mobile.status.chooseMailFolder")
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    status = trs("mobile.status.copying")
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                requireTransferAffected(
                    requireCoreOk(
                        MobileMailCommandClient(core).copy(
                            CopyThreadParams(
                                threadId = thread.id,
                                targetAccountId = targetAccountId,
                                targetFolderId = target.name,
                            ),
                        ),
                    ),
                    "copied",
                )
            }
        }.onSuccess {
            rememberThreadTransfer(RecentThreadTransfer("copy", thread.accountId, targetAccountId, target.name))
            status = trs("mobile.status.copyComplete")
        }.onFailure {
            showTransferFailure("mail.toast.copyFailed", it)
        }
    }
}

internal fun MeronMobileState.createFolderAndMoveThread(
    thread: ThreadSummary,
    name: String,
    onMoved: () -> Unit = {},
) {
    val trimmed = name.trim()
    if (threadIdIsRss(thread.id)) {
        status = trs("mobile.status.rssMoveBetweenAccounts")
        return
    }
    if (trimmed.isBlank()) {
        status = trs("folders.nameRequired")
        return
    }
    val account = coreAccounts.firstOrNull { it.id == thread.accountId }
    if (account == null) {
        status = trs("mobile.status.accountNotFound")
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    status = trs("mobile.status.creatingFolder")
    val removal = suppressRemovedThread(thread.id)
    scope.launch {
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                withManagedGoogleAuth(client, thread.accountId) {
                    client.createFolder(FolderCreateParams(accountId = thread.accountId, name = trimmed))
                }
                val folders = loadAccountFolders(client, account)
                val createdFolder = folders.folderCreatedAs(trimmed)
                val created = createdFolder?.name ?: trimmed
                if (created.equals(thread.folder, ignoreCase = true)) {
                    throw IllegalStateException(trs("mobile.status.alreadyInFolder", mapOf("folder" to (createdFolder?.displayName ?: trimmed))))
                }
                val response =
                    withManagedGoogleAuth(client, thread.accountId) {
                        requireTransferAffected(requireCoreOk(client.move(MoveThreadParams(threadId = thread.id, targetFolderId = created))), "moved")
                    }
                folders to response
            }
        }.onSuccess { (folders, response) ->
            removal.complete()
            releaseMovedThread(response)
            foldersByAccount = foldersByAccount + (account.id to reconcileFolderUnread(folders, folderReadVersion))
            val destination = folders.folderCreatedAs(trimmed)?.name ?: trimmed
            rememberThreadTransfer(RecentThreadTransfer("move", thread.accountId, thread.accountId, destination))
            removeThreadEverywhere(thread.id)
            if (selectedCoreThread?.id == thread.id) {
                selectedCoreThread = null
                messages = emptyList()
            }
            status = trs("mobile.status.folderCreatedAndMoved")
            onMoved()
        }.onFailure {
            removal.rollback()
            reloadFailedMoveSource(thread)
            showTransferFailure("mobile.status.createFolderFailed", it)
        }
    }
}

internal fun MeronMobileState.moveThreadToColumn(
    thread: ThreadSummary,
    target: KanbanColumnSpec,
) {
    if (target.accountId == UNIFIED_ACCOUNT_ID) {
        status = trs("mobile.status.moveToFolderColumn")
        return
    }
    val targetAccount = coreAccounts.firstOrNull { it.id == target.accountId }
    if (targetAccount == null) {
        status = trs("mobile.status.targetAccountNotFound")
        return
    }
    if (threadIdIsRss(thread.id)) {
        if (!accountSummaryIsRss(targetAccount)) {
            status = trs("mobile.status.rssOnlyToRssAccounts")
            return
        }
        val removal = suppressRemovedThread(thread.id)
        scope.launch {
            runCatching {
                withContext(ioDispatcher) {
                    requireCoreOk(
                        MobileMailCommandClient(core).moveRssFeed(
                            MoveRssFeedParams(threadId = thread.id, targetAccountId = target.accountId),
                        ),
                    )
                }
            }.onSuccess { response ->
                removal.complete()
                releaseMovedThread(response)
                removeThreadEverywhere(thread.id)
                loadKanbanColumn(target, refresh = false)
                status = trs("mobile.status.moveComplete")
            }.onFailure {
                removal.rollback()
                reloadFailedMoveSource(thread)
                showTransferFailure("mail.toast.moveFailed", it)
            }
        }
        return
    }
    if (accountSummaryIsRss(targetAccount)) {
        status = trs("mobile.status.mailCannotMoveToFeeds")
        return
    }
    val removal = suppressRemovedThread(thread.id)
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                requireTransferAffected(
                    requireCoreOk(
                        MobileMailCommandClient(core).move(
                            MoveThreadParams(threadId = thread.id, targetFolderId = target.folderId, targetAccountId = target.accountId),
                        ),
                    ),
                    "moved",
                )
            }
        }.onSuccess { response ->
            rememberThreadTransfer(RecentThreadTransfer("move", thread.accountId, target.accountId, target.folderId))
            removal.complete()
            releaseMovedThread(response)
            removeThreadEverywhere(thread.id)
            loadKanbanColumn(target, refresh = false)
            status = trs("mobile.status.moveComplete")
        }.onFailure {
            removal.rollback()
            reloadFailedMoveSource(thread)
            showTransferFailure("mail.toast.moveFailed", it)
        }
    }
}
