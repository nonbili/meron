package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.filled.Add
import androidx.compose.ui.input.key.key
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.StarredItemsParams
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseStarredItemsPage
import jp.nonbili.meron.shared.parseThreadListPage
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// Header pages fetched from the server per folder sync. A mailbox with no local
// threads yet blocks first paint on this fetch, so it starts with the smaller
// page and deepens to the full one in the background once the list is showing.
internal const val MAILBOX_SYNC_LIMIT = 250
internal const val MAILBOX_FIRST_SYNC_LIMIT = 50

// Header rows one thread-list page holds, per account. Mirrors the core's own
// default (thread_list::DEFAULT_LIMIT) so an unpaged mailbox reads exactly what
// it did before this became explicit.
internal const val MAILBOX_PAGE_SIZE = 50

// Ceiling on the depth an event-driven reload re-requests. Reloads run on every
// sync event, so a mailbox paged very deep would otherwise make each one
// progressively more expensive; past this the list falls back to re-paging.
internal const val MAILBOX_MAX_RELOAD_DEPTH = 500

// Sync events arrive in bursts — cold start alone fires one catch-up sync per
// account per watched folder (INBOX and Sent), plus body prefetch and thread-gap
// fills. Each reload is a full store re-read that repaints the list, so coalesce
// them into one instead of running the burst back to back.
internal const val MAILBOX_RELOAD_DEBOUNCE_MS = 400L

internal fun mailboxCacheKey(
    accountId: String,
    folderId: String,
    query: String,
    filter: FilterMode,
    attachmentsOnly: Boolean = false,
): MailboxCacheKey =
    MailboxCacheKey(
        accountId = accountId.ifBlank { UNIFIED_ACCOUNT_ID },
        folderId = folderId.ifBlank { INBOX_FOLDER }.lowercase(),
        query = query.trim(),
        filter = filter,
        attachmentsOnly = attachmentsOnly,
    )

private fun MeronMobileState.cacheVisibleMailbox() {
    if (!initialThreadsLoaded) return
    val accountId = selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val folderId = selectedCoreFolder.ifBlank { INBOX_FOLDER }
    val key = visibleMailboxKey ?: mailboxCacheKey(accountId, folderId, mailSearch, mailFilter, mailAttachmentsOnly)
    mailboxCache =
        mailboxCache +
        (
            key to
                MailboxLoadResult(
                    folders = coreFolders,
                    folder = folderId,
                    threads = withLocalDraftFlags(threadRemovalGuard.filter(coreThreads)),
                    nextCursor = mailboxCursor,
                    accountCursors = mailboxAccountCursors,
                    pageDepth = mailboxPageDepth,
                )
        )
}

private fun MeronMobileState.restoreCachedMailbox(
    accountId: String,
    folderId: String,
): Boolean {
    val key = mailboxCacheKey(accountId, folderId, mailSearch, mailFilter, mailAttachmentsOnly)
    val cached = mailboxCache[key] ?: return false
    coreFolders = reconcileFolderUnread(cached.folders, folderReadGuard.version)
    if (cached.folders.isNotEmpty()) {
        foldersByAccount = foldersByAccount + reconcileFolderUnread(cached.folders, folderReadGuard.version).groupBy { it.accountId }
    }
    selectedCoreFolder = cached.folder
    coreThreads = withLocalDraftFlags(threadRemovalGuard.filter(cached.threads))
    visibleMailboxKey = key
    mailboxCursor = cached.nextCursor
    mailboxAccountCursors = cached.accountCursors
    mailboxPageDepth = cached.pageDepth
    initialThreadsLoaded = true
    errorBanner = null
    return true
}

internal fun MeronMobileState.selectCoreMailbox(
    accountId: String,
    folderId: String = loadMailFolderForAccount(prefs, accountId),
) {
    cacheVisibleMailbox()
    selectedCoreAccountId = accountId.ifBlank { UNIFIED_ACCOUNT_ID }
    selectedCoreFolder = folderId.ifBlank { INBOX_FOLDER }
    saveLastMailLocation(prefs, selectedCoreAccountId, selectedCoreFolder)
    selectedCoreThread = null
    selectedMailThreadIds = emptySet()
    mailSelectionMenuOpen = false
    messages = emptyList()
    messageCursor = ""
    loadingMoreMessages = false
    if (!restoreCachedMailbox(selectedCoreAccountId, selectedCoreFolder)) {
        coreFolders = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) coreFolders else emptyList()
        coreThreads = emptyList()
        visibleMailboxKey = null
        mailboxCursor = ""
        mailboxAccountCursors = emptyMap()
        mailboxPageDepth = MAILBOX_PAGE_SIZE
        initialThreadsLoaded = false
    }
}

// Whether a core sync event changes what the visible thread list shows. Events
// fire per account and per watched folder — the foreground watchers alone cover
// every account's INBOX *and* Sent — so reloading on all of them re-reads a
// mailbox that did not change. Blank fields mean "unknown" (folder-list syncs
// carry no folder) and reload rather than risk going stale.
internal fun mailEventAffectsVisibleMailbox(
    eventAccount: String,
    eventFolder: String,
    selectedAccountId: String,
    selectedFolder: String,
    unifiedAccountIds: Set<String>,
    unifiedFoldersByAccount: Map<String, List<FolderSummary>> = emptyMap(),
): Boolean {
    if (eventAccount.isBlank()) return true
    val visibleAccount = selectedAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val accountMatches =
        if (visibleAccount == UNIFIED_ACCOUNT_ID) {
            eventAccount in unifiedAccountIds
        } else {
            eventAccount == visibleAccount
        }
    if (!accountMatches) return false
    // A starred item can live in any folder, so any event from an account the
    // listing covers can change it.
    if (visibleAccount == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(selectedFolder)) return true
    if (eventFolder.isBlank()) return true
    val visibleFolder =
        if (visibleAccount == UNIFIED_ACCOUNT_ID) {
            unifiedAccountFolder(
                unifiedFoldersByAccount[eventAccount].orEmpty(),
                selectedFolder,
            ) ?: return false
        } else {
            selectedFolder.ifBlank { INBOX_FOLDER }
        }
    return eventFolder.equals(visibleFolder, ignoreCase = true)
}

// Reload the visible mailbox off a sync event when the event actually concerns
// it, coalescing the burst that arrives on cold start (and whenever several
// accounts sync at once) into a single re-read.
internal fun MeronMobileState.reloadVisibleMailboxFor(
    eventAccount: String,
    eventFolder: String,
) {
    val affected =
        mailEventAffectsVisibleMailbox(
            eventAccount = eventAccount,
            eventFolder = eventFolder,
            selectedAccountId = selectedCoreAccountId,
            selectedFolder = selectedCoreFolder,
            unifiedAccountIds =
                coreAccounts
                    .filter { isUnifiedStarredFolder(selectedCoreFolder) || it.includedInUnified }
                    .map { it.id }
                    .toSet(),
            unifiedFoldersByAccount = foldersByAccount,
        )
    if (!affected) {
        Log.i("MailLoad", "reload skipped unrelated event account=$eventAccount folder=$eventFolder")
        return
    }
    // A fixed window rather than a resettable debounce: a steady event stream (a
    // long first sync on a large mailbox) would keep pushing a resettable timer
    // back and the list would never refresh. Events arriving inside the window
    // need no reload of their own — the one already scheduled re-reads the store
    // after they have landed in it.
    if (mailboxReloadJob?.isActive == true) return
    mailboxReloadJob =
        scope.launch {
            delay(MAILBOX_RELOAD_DEBOUNCE_MS)
            syncCoreThreads(
                accountOverride = selectedCoreAccountId,
                folderOverride = selectedCoreFolder,
                syncFirst = false,
            )
        }
}

// How many further pages a reload may read to get back to the oldest row shown.
internal const val MAILBOX_RELOAD_TOP_UP_PAGES = 10

/**
 * Read on from a reload's [first] page, with [nextPage], until it reaches back
 * to [oldestShown], the oldest row the list showed before. The page depth is
 * only a first guess at how far the reader had scrolled: the core may read past
 * it (an attachments-filtered page skips empty stretches, and its limit counts
 * messages, which group into fewer threads), and the reload cap stops short of
 * a very deep list. Returns the pages joined, and the depth read.
 */
internal suspend fun readMailboxToOldestShown(
    first: MailboxLoadResult,
    listLimit: Int,
    oldestShown: Long?,
    nextPage: suspend (beforeCursor: String, accountCursors: Map<String, String>) -> MailboxLoadResult,
): Pair<MailboxLoadResult, Int> {
    var result = first
    var readDepth = listLimit
    var extra = 0
    while (
        oldestShown != null &&
        result.nextCursor.isNotBlank() &&
        extra < MAILBOX_RELOAD_TOP_UP_PAGES &&
        (result.threads.minOfOrNull { it.dateEpochSeconds } ?: Long.MAX_VALUE) > oldestShown
    ) {
        val more = nextPage(result.nextCursor, result.accountCursors)
        val ids = result.threads.map { it.id }.toSet()
        result =
            result.copy(
                threads = (result.threads + more.threads.filterNot { it.id in ids }).sortedByDescending { it.dateEpochSeconds },
                nextCursor = more.nextCursor,
                accountCursors = more.accountCursors,
                searchIncomplete = result.searchIncomplete || more.searchIncomplete,
            )
        readDepth = (readDepth + MAILBOX_PAGE_SIZE).coerceAtMost(MAILBOX_MAX_RELOAD_DEPTH)
        extra++
    }
    return result to readDepth
}

internal fun MeronMobileState.syncCoreThreads(
    accountOverride: String? = null,
    folderOverride: String? = null,
    syncFirst: Boolean = true,
    successStatus: String? = null,
    scrollToTopOnSuccess: Boolean = false,
    refreshSearch: Boolean = false,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val accountId = accountOverride ?: selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val requestedFolder = folderOverride ?: selectedCoreFolder.ifBlank { INBOX_FOLDER }
    val unifiedStarred = accountId == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(requestedFolder)
    val query = mailSearch
    val filter = mailFilter
    val attachmentsOnly = mailAttachmentsOnly
    val selectedAccounts =
        if (accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts.filter { it.includedInUnified }
        } else {
            coreAccounts.filter { it.id == accountId }
        }
    if (selectedAccounts.isEmpty() && !unifiedStarred) {
        Log.w("MailLoad", "syncCoreThreads no selected accounts account=$accountId folder=$requestedFolder")
        status = if (accountId == UNIFIED_ACCOUNT_ID) "No accounts are included in Unified inbox." else "No account selected."
        initialThreadsLoaded = true
        return
    }
    val requestKey = mailboxCacheKey(accountId, requestedFolder, query, filter, attachmentsOnly)
    if (syncing && activeMailboxLoadKey == requestKey) {
        Log.i("MailLoad", "sync skipped duplicate account=$accountId folder=$requestedFolder")
        deferredMailboxReload =
            DeferredMailboxReload(
                key = requestKey,
                refreshSearch = refreshSearch || deferredMailboxReload?.takeIf { it.key == requestKey }?.refreshSearch == true,
            )
        return
    }
    val requestToken = activeMailboxLoadToken + 1
    activeMailboxLoadToken = requestToken
    activeMailboxLoadKey = requestKey
    // This load reads the store after anything a skipped reload was reacting to.
    deferredMailboxReload = null
    activeMailboxLoadStartedAtMillis = currentTimeMillis()
    blockingMailboxLoadWarned = false
    blockingMailboxLoadSlow = false
    syncing = true
    // A mailbox with no local threads yet blocks first paint on the server
    // fetch: sync a small first page now and deepen in the background below.
    val firstLoad = !initialThreadsLoaded
    val syncLimit = if (firstLoad) MAILBOX_FIRST_SYNC_LIMIT else MAILBOX_SYNC_LIMIT
    // Re-read as deep as the visible mailbox has already been paged. Reloads run
    // on every sync event, and reading only the first page here would drop the
    // pages the user scrolled through — the list shrinks under them, the keyed
    // scroll anchor disappears, and the position clamps to the end. A request for
    // a *different* mailbox (folder switch, new search, filter change) starts at
    // one page, since that list is not on screen yet.
    val reloadingVisible = visibleMailboxKey == requestKey
    val listLimit =
        if (reloadingVisible) {
            mailboxPageDepth.coerceIn(MAILBOX_PAGE_SIZE, MAILBOX_MAX_RELOAD_DEPTH)
        } else {
            MAILBOX_PAGE_SIZE
        }
    val oldestShown = if (reloadingVisible) coreThreads.minOfOrNull { it.dateEpochSeconds } else null
    Log.i(
        "MailLoad",
        "sync start account=$accountId folder=$requestedFolder accounts=${selectedAccounts.size} syncFirst=$syncFirst limit=$syncLimit listLimit=$listLimit query=${query.isNotBlank()} filter=${filter.protocolValue()}",
    )
    scope.launchThreadListRead(threadRemovalGuard) { read ->
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val first =
                    if (unifiedStarred) {
                        // The starred listing spans folders, so there is no mailbox
                        // to sync first: the core reads whatever the accounts'
                        // own syncs have already starred.
                        loadUnifiedStarred(client = client, query = query, filter = filter, attachmentsOnly = attachmentsOnly, limit = listLimit)
                    } else if (accountId == UNIFIED_ACCOUNT_ID) {
                        loadUnifiedInbox(
                            client = client,
                            accounts = selectedAccounts,
                            query = query,
                            filter = filter,
                            attachmentsOnly = attachmentsOnly,
                            syncFirst = syncFirst,
                            syncLimit = syncLimit,
                            listLimit = listLimit,
                            refreshSearch = refreshSearch,
                            folderRole = requestedFolder,
                        )
                    } else {
                        loadAccountInbox(
                            client,
                            selectedAccounts.first(),
                            requestedFolder,
                            query = query,
                            filter = filter,
                            attachmentsOnly = attachmentsOnly,
                            syncFirst = syncFirst,
                            syncLimit = syncLimit,
                            listLimit = listLimit,
                            refreshSearch = refreshSearch,
                        )
                    }
                readMailboxToOldestShown(
                    first = first,
                    listLimit = listLimit,
                    oldestShown = oldestShown,
                ) { beforeCursor, accountCursors ->
                    if (unifiedStarred) {
                        loadUnifiedStarred(client = client, query = query, filter = filter, attachmentsOnly = attachmentsOnly, beforeCursor = beforeCursor)
                    } else if (accountId == UNIFIED_ACCOUNT_ID) {
                        loadUnifiedInbox(
                            client = client,
                            accounts = selectedAccounts,
                            query = query,
                            filter = filter,
                            attachmentsOnly = attachmentsOnly,
                            syncFirst = false,
                            beforeCursor = beforeCursor,
                            refreshSearch = refreshSearch,
                            folderRole = requestedFolder,
                        )
                    } else {
                        loadAccountInbox(
                            client,
                            selectedAccounts.first(),
                            requestedFolder,
                            query = query,
                            filter = filter,
                            attachmentsOnly = attachmentsOnly,
                            syncFirst = false,
                            beforeCursor = beforeCursor,
                            refreshSearch = refreshSearch,
                        )
                    }
                }
            }
        }.onSuccess { (result, readDepth) ->
            val resultKey = mailboxCacheKey(accountId, result.folder, query, filter, attachmentsOnly)
            mailboxCache =
                mailboxCache +
                (
                    resultKey to
                        result.copy(
                            folders = result.folders,
                            folder = result.folder,
                            threads = withLocalDraftFlags(withoutLocallyDiscardedThreads(read.filter(result.threads))),
                            nextCursor = result.nextCursor,
                            accountCursors = result.accountCursors,
                            pageDepth = readDepth,
                        )
                )
            if (activeMailboxLoadToken != requestToken || accountId != selectedCoreAccountId) {
                // A selection change without a replacement load still owns the
                // loading flags. An older request must leave a newer load alone.
                if (activeMailboxLoadToken == requestToken) {
                    finishMailboxLoad()
                    deferredMailboxReload = null
                }
                Log.w("MailLoad", "sync ignored stale result account=$accountId folder=${result.folder} threads=${result.threads.size}")
                return@onSuccess
            }
            val wasInitialLoad = !initialThreadsLoaded
            val existingIds = coreThreads.map { it.id }.toSet()
            coreFolders = reconcileFolderUnread(result.folders, folderReadVersion)
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + reconcileFolderUnread(result.folders, folderReadVersion).groupBy { it.accountId }
            }
            val folder = result.folder
            selectedCoreFolder = folder
            saveLastMailLocation(prefs, accountId, folder)
            val parsedThreads = withLocalDraftFlags(withoutLocallyDiscardedThreads(read.filter(result.threads)))
            coreThreads = parsedThreads
            visibleMailboxKey = resultKey
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            mailboxPageDepth = readDepth
            // The open conversation is deliberately left alone here. A refresh
            // returns one page of one mailbox, so the open thread being absent
            // means nothing (it may sit past the page limit, or belong to
            // another account/folder when opened from kanban, starred or a
            // notification). Clearing it on that basis raced every thread open
            // — background syncs run continuously — and left the thread screen
            // with no summary and no messages, spinning forever. Selections
            // that really go away are cleared by the move/archive paths.
            finishMailboxLoad()
            errorBanner = null
            syncError = null
            if (scrollToTopOnSuccess) {
                mailListScrollToTopRequest += 1
            }
            val newCount = if (!wasInitialLoad && syncFirst) parsedThreads.count { it.id !in existingIds } else 0
            status = successStatus
                ?: if (result.searchIncomplete) {
                    // The server half of the search failed; the list is only what
                    // this device has cached, so older matches may be missing.
                    trs("threads.searchServerUnavailable")
                } else if (newCount > 0) {
                    "$newCount new message(s)"
                } else {
                    ""
                }
            Log.i(
                "MailLoad",
                "sync success account=$accountId folder=$folder threads=${parsedThreads.size} cursor=${mailboxCursor.isNotBlank()} accountCursors=${mailboxAccountCursors.size} initialThreadsLoaded=$initialThreadsLoaded syncing=$syncing",
            )
            // Search is cache-first on mobile: paint indexed matches before
            // starting the potentially expensive IMAP search. The second load
            // leaves those matches visible and replaces them when it completes.
            if (query.isNotBlank() && mailSearch == query && !refreshSearch) {
                syncCoreThreads(
                    accountOverride = accountId,
                    folderOverride = folder,
                    syncFirst = false,
                    refreshSearch = true,
                )
            }
            // After the live search above: it reads the store afresh and so
            // stands in for a reload that stepped aside. Re-running that reload
            // first would start a cache read the live search then steps aside
            // for, and the two would take turns for good.
            runDeferredMailboxReload(requestKey, accountId, requestedFolder)
            if (firstLoad && syncFirst && !unifiedStarred) {
                deepenMailboxSync(accountId, folder, selectedAccounts)
            }
        }.onFailure {
            if (activeMailboxLoadToken != requestToken || accountId != selectedCoreAccountId) {
                if (activeMailboxLoadToken == requestToken) {
                    finishMailboxLoad()
                    deferredMailboxReload = null
                }
                Log.w("MailLoad", "sync ignored stale failure account=$accountId", it)
                return@onFailure
            }
            finishMailboxLoad()
            runDeferredMailboxReload(requestKey, accountId, requestedFolder)
            val contextual = it as? AccountSyncException
            val failedAccountId =
                contextual?.accountId
                    ?: accountId.takeUnless { candidate -> candidate == UNIFIED_ACCOUNT_ID }
            val message = contextual?.cause?.message ?: it.message ?: "Sync failed"
            syncError = MobileSyncError(failedAccountId, message)
            // The failing account is named by the request, not read out of the text.
            if (failedAccountId != null && message.contains("account needs reconnect")) {
                refreshAccountReconnectState(failedAccountId)
            }
            errorBanner = null
            status = "Sync failed: ${it.message}"
            Log.w("MailLoad", "sync failed account=$accountId folder=$requestedFolder initialThreadsLoaded=$initialThreadsLoaded syncing=$syncing", it)
        }
    }
}

private fun MeronMobileState.finishMailboxLoad() {
    activeMailboxLoadKey = null
    activeMailboxLoadStartedAtMillis = 0L
    blockingMailboxLoadWarned = false
    blockingMailboxLoadSlow = false
    syncing = false
    initialThreadsLoaded = true
}

// A reload that stepped aside for the load that just settled was asking about
// a store that load had already read — the draft a post-send discard removed
// is still in the rows just painted, so the card keeps its Draft badge and
// counts the draft. Run the reload now that nothing is in its way.
private fun MeronMobileState.runDeferredMailboxReload(
    settledKey: MailboxCacheKey,
    accountId: String,
    folder: String,
) {
    val deferred = deferredMailboxReload?.takeIf { it.key == settledKey } ?: return
    deferredMailboxReload = null
    syncCoreThreads(
        accountOverride = accountId,
        folderOverride = folder,
        syncFirst = false,
        refreshSearch = deferred.refreshSearch,
    )
}

// Second phase of a first-load sync: fetch the full header page for each mail
// account, then re-read the store so the visible list picks up the older
// threads. Best-effort — the small first page is already on screen, so a
// failure here only costs depth, not the inbox.
private fun MeronMobileState.deepenMailboxSync(
    accountId: String,
    folder: String,
    accounts: List<AccountSummary>,
) {
    val mailAccounts = accounts.filterNot { accountSummaryIsRss(it) }
    if (mailAccounts.isEmpty()) return
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                mailAccounts.forEach { account ->
                    withManagedGoogleAuth(client, account.id) {
                        val accountFolders =
                            parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
                        val targetFolder =
                            unifiedAccountFolder(accountFolders, folder) ?: return@withManagedGoogleAuth "{}"
                        client.sync(
                            SyncMailParams(
                                accountId = account.id,
                                folderId = targetFolder,
                                limit = MAILBOX_SYNC_LIMIT,
                                folders = false,
                                deferTail = true,
                            ),
                        )
                    }
                }
            }
        }.onSuccess {
            Log.i("MailLoad", "deep sync done account=$accountId folder=$folder accounts=${mailAccounts.size}")
            syncCoreThreads(accountOverride = accountId, folderOverride = folder, syncFirst = false)
        }.onFailure {
            Log.w("MailLoad", "deep sync failed account=$accountId folder=$folder", it)
        }
    }
}

internal fun MeronMobileState.addFeedToSelectedRssAccount() {
    if (addFeedSubmitting) return
    val account = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }
    val feedUrl = addFeedUrl.trim()
    if (account == null || !accountSummaryIsRss(account)) {
        addFeedError = "Select an RSS account first."
        return
    }
    if (feedUrl.isBlank()) {
        addFeedError = "Feed URL is required."
        return
    }
    if (!coreLoaded) {
        addFeedError = coreUnavailableMessage
        return
    }
    addFeedError = ""
    addFeedSubmitting = true
    status = "Adding feed..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).addRssFeed(
                    AddRssFeedParams(accountId = account.id, feedUrl = feedUrl),
                )
            }
        }.onSuccess {
            addFeedSubmitting = false
            addFeedUrl = ""
            addFeedError = ""
            showAddFeedDialog = false
            status = "Feed added"
            // feed.add already fetched and stored the new feed's items, so
            // re-fetching here would be a redundant (and slow) network round-trip.
            syncCoreThreads(accountOverride = account.id, syncFirst = false, successStatus = "Feed added")
        }.onFailure {
            addFeedSubmitting = false
            addFeedError = "Add feed failed: ${it.message}"
        }
    }
}

// Accounts that still have a pagination cursor for the visible mailbox. Shared
// between loadMoreCoreThreads and the UI's canLoadMore flag so the load-more
// affordance never shows when a load would silently no-op (e.g. the only
// remaining cursors belong to accounts excluded from the Unified inbox).
internal fun pageableMailAccounts(
    selectedAccountId: String,
    accounts: List<AccountSummary>,
    mailboxCursor: String,
): List<AccountSummary> {
    val accountId = selectedAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    return if (accountId == UNIFIED_ACCOUNT_ID) {
        accounts.filter { it.includedInUnified && mailboxCursor.isNotBlank() }
    } else {
        accounts.filter { it.id == accountId && mailboxCursor.isNotBlank() }
    }
}

internal fun MeronMobileState.pageableCoreAccounts(): List<AccountSummary> =
    if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(selectedCoreFolder)) {
        coreAccounts.filter { mailboxCursor.isNotBlank() }
    } else {
        pageableMailAccounts(selectedCoreAccountId, coreAccounts, mailboxCursor)
    }

// `quiet` suppresses the "Loaded N older message(s)" status for auto-fired
// pagination — store reloads (e.g. after a background sync event) shrink the
// list back to its first page, and the resulting refetch chain would otherwise
// toast once per page.
internal fun MeronMobileState.loadMoreCoreThreads(quiet: Boolean = false) {
    if (!coreLoaded || loadingMoreThreads) return
    val accountId = selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val requestedFolder = selectedCoreFolder.ifBlank { INBOX_FOLDER }
    val query = mailSearch
    val filter = mailFilter
    val attachmentsOnly = mailAttachmentsOnly
    val selectedAccounts = pageableCoreAccounts()
    if (selectedAccounts.isEmpty()) return
    loadingMoreThreads = true
    scope.launchThreadListRead(threadRemovalGuard) { read ->
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (accountId == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(requestedFolder)) {
                    loadUnifiedStarred(client = client, query = query, filter = filter, attachmentsOnly = attachmentsOnly, beforeCursor = mailboxCursor)
                } else if (accountId == UNIFIED_ACCOUNT_ID) {
                    loadUnifiedInbox(
                        client = client,
                        accounts = selectedAccounts,
                        query = query,
                        filter = filter,
                        attachmentsOnly = attachmentsOnly,
                        syncFirst = false,
                        beforeCursor = mailboxCursor,
                        folderRole = requestedFolder,
                    )
                } else {
                    loadAccountInbox(
                        client,
                        selectedAccounts.first(),
                        requestedFolder,
                        query = query,
                        filter = filter,
                        attachmentsOnly = attachmentsOnly,
                        syncFirst = false,
                        beforeCursor = mailboxCursor,
                    )
                }
            }
        }.onSuccess { result ->
            if (result.folders.isNotEmpty()) {
                coreFolders = reconcileFolderUnread(result.folders, folderReadVersion)
                foldersByAccount = foldersByAccount + reconcileFolderUnread(result.folders, folderReadVersion).groupBy { it.accountId }
            }
            val existingIds = coreThreads.map { it.id }.toSet()
            val appended = withLocalDraftFlags(read.filter(result.threads)).filterNot { it.id in existingIds }
            coreThreads = read.filter(coreThreads + appended).sortedByDescending { it.dateEpochSeconds }
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            // One more page is on screen, so event-driven reloads have to re-read
            // this deep to keep it there.
            mailboxPageDepth = (mailboxPageDepth + MAILBOX_PAGE_SIZE).coerceAtMost(MAILBOX_MAX_RELOAD_DEPTH)
            cacheVisibleMailbox()
            loadingMoreThreads = false
            errorBanner = null
            if (result.searchIncomplete) {
                // Older server matches could not be fetched; the list ends at
                // what this device has cached.
                status = trs("threads.searchServerUnavailable")
            } else if (!quiet) {
                status = if (appended.isEmpty()) "No older messages." else "Loaded ${appended.size} older message(s)."
            }
        }.onFailure {
            loadingMoreThreads = false
            errorBanner = it.message ?: "Load more failed"
            status = "Load more failed: ${it.message}"
        }
    }
}

internal suspend fun MeronMobileState.loadAccountInbox(
    client: MobileMailCommandClient,
    account: AccountSummary,
    requestedFolder: String,
    query: String = mailSearch,
    filter: FilterMode = mailFilter,
    attachmentsOnly: Boolean = mailAttachmentsOnly,
    syncFirst: Boolean = true,
    beforeCursor: String? = null,
    syncLimit: Int = MAILBOX_SYNC_LIMIT,
    listLimit: Int = MAILBOX_PAGE_SIZE,
    refreshSearch: Boolean = true,
): MailboxLoadResult {
    // When syncFirst is false we read whatever the local (encrypted) store
    // already has — used on startup so the inbox shows instantly without a
    // server round-trip. Pull-to-sync / "Sync now" still fetch from server.
    Log.i(
        "MailLoad",
        "loadAccountInbox start account=${account.id} requestedFolder=$requestedFolder syncFirst=$syncFirst beforeCursor=${beforeCursor?.isNotBlank() == true} query=${query.isNotBlank()} filter=${filter.protocolValue()}",
    )
    if (syncFirst) {
        if (accountSummaryIsRss(account)) {
            Log.i("MailLoad", "loadAccountInbox sync rss account=${account.id}")
            client.syncRss(SyncRssParams(accountId = account.id))
        } else {
            Log.i("MailLoad", "loadAccountInbox sync mail account=${account.id} folder=$requestedFolder")
            withManagedGoogleAuth(client, account.id) {
                client.sync(
                    SyncMailParams(
                        accountId = account.id,
                        folderId = requestedFolder,
                        limit = syncLimit,
                        folders = true,
                        deferTail = true,
                    ),
                )
            }
        }
    }
    val foldersJson = client.listFolders(FolderListParams(accountId = account.id))
    val folders = parseFolderListResponse(foldersJson)
    // Server folder names are case-sensitive ("INBOX"), but the default
    // request uses "inbox"; match case-insensitively and fall back to a real
    // inbox before the first folder.
    val folder =
        folders.firstOrNull { it.name.equals(requestedFolder, ignoreCase = true) }?.name
            ?: folders.firstOrNull { it.name.equals(INBOX_FOLDER, ignoreCase = true) }?.name
            ?: folders.firstOrNull()?.name
            ?: requestedFolder
    Log.i("MailLoad", "loadAccountInbox folders account=${account.id} count=${folders.size} resolvedFolder=$folder")
    if (!accountSummaryIsRss(account) && query.isNotBlank() && refreshSearch) {
        // The refreshed search is a live IMAP operation even when syncFirst is
        // false. Keep token upkeep out of the cache-first request so local
        // matches can paint without waiting for the network.
        ensureManagedGoogleToken(client, account.id)
    }
    val threadsJson =
        client.listThreads(
            ThreadListParams(
                accountId = account.id,
                folderId = folder,
                query = query.trim(),
                filter = filter.protocolValue(),
                attachments = attachmentsOnly,
                beforeCursor = beforeCursor,
                refresh = refreshSearch,
                // Paging forward always fetches a single page; only a reload of
                // the whole mailbox re-requests the depth already on screen.
                limit = if (beforeCursor == null) listLimit else MAILBOX_PAGE_SIZE,
            ),
        )
    val page = parseThreadListPage(threadsJson)
    val foldersWithPageUnread =
        page.folderUnread?.let { unread ->
            folders.map { item ->
                if (item.name.equals(folder, ignoreCase = folder.equals(INBOX_FOLDER, ignoreCase = true))) {
                    item.copy(unread = unread)
                } else {
                    item
                }
            }
        } ?: folders
    Log.i(
        "MailLoad",
        "loadAccountInbox threads account=${account.id} folder=$folder count=${page.threads.size} cursor=${page.nextCursor.isNotBlank()}",
    )
    return MailboxLoadResult(
        folders = foldersWithPageUnread,
        folder = folder,
        threads = page.threads,
        unreadCount = page.folderUnread,
        nextCursor = page.nextCursor,
        folderSynced = page.folderSynced,
        searchIncomplete = page.searchIncomplete,
    )
}

internal suspend fun MeronMobileState.loadUnifiedInbox(
    client: MobileMailCommandClient,
    accounts: List<AccountSummary>,
    query: String = mailSearch,
    filter: FilterMode = mailFilter,
    attachmentsOnly: Boolean = mailAttachmentsOnly,
    syncFirst: Boolean = true,
    beforeCursor: String? = null,
    syncLimit: Int = MAILBOX_SYNC_LIMIT,
    listLimit: Int = MAILBOX_PAGE_SIZE,
    refreshSearch: Boolean = true,
    /**
     * Which special-use folder the view is on. The unified view addresses
     * mailboxes by role: the core resolves each account's own Sent/Archive/…
     * and leaves out accounts whose server has none.
     */
    folderRole: String = INBOX_FOLDER,
): MailboxLoadResult {
    val role = unifiedFolderRole(folderRole)
    if (syncFirst) {
        accounts.forEach { account ->
            withSyncAccountContext(account.id) {
                if (accountSummaryIsRss(account)) {
                    if (role == INBOX_FOLDER) {
                        client.syncRss(SyncRssParams(accountId = account.id))
                    }
                } else {
                    withManagedGoogleAuth(client, account.id) {
                        var accountFolders =
                            parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
                        var targetFolder = unifiedAccountFolder(accountFolders, role)
                        // A cold cache cannot resolve a provider-specific Sent /
                        // Archive name. Refresh folder metadata through Inbox,
                        // then resolve the role again before syncing its mailbox.
                        if (targetFolder == null) {
                            client.sync(
                                SyncMailParams(
                                    accountId = account.id,
                                    folderId = INBOX_FOLDER,
                                    limit = syncLimit,
                                    folders = true,
                                    deferTail = true,
                                ),
                            )
                            accountFolders =
                                parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
                            targetFolder = unifiedAccountFolder(accountFolders, role)
                        }
                        if (targetFolder != null) {
                            client.sync(
                                SyncMailParams(
                                    accountId = account.id,
                                    folderId = targetFolder,
                                    limit = syncLimit,
                                    folders = true,
                                    deferTail = true,
                                ),
                            )
                        } else {
                            "{}"
                        }
                    }
                }
            }
        }
    }
    val folders =
        accounts.flatMap { account ->
            withSyncAccountContext(account.id) {
                parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
            }
        }
    if (query.isNotBlank() && refreshSearch) {
        // Unified live search fans out inside one core call and falls back per
        // account, so refresh all managed mail tokens before issuing it.
        accounts.filterNot(::accountSummaryIsRss).forEach { account ->
            ensureManagedGoogleToken(client, account.id)
        }
    }
    val page =
        parseThreadListPage(
            client.listThreads(
                ThreadListParams(
                    accountId = UNIFIED_ACCOUNT_ID,
                    folderId = role,
                    folderRole = role,
                    query = query.trim(),
                    filter = filter.protocolValue(),
                    attachments = attachmentsOnly,
                    beforeCursor = beforeCursor,
                    refresh = refreshSearch,
                    // The core fans this out per account, so the limit is a
                    // per-account depth here too — matching how load-more appends
                    // one page from each account.
                    limit = if (beforeCursor == null) listLimit else MAILBOX_PAGE_SIZE,
                ),
            ),
        )
    return MailboxLoadResult(
        folders = folders,
        folder = role,
        threads = page.threads,
        // Only the inbox rolls up into the drawer's unread badge; the other
        // unified folders have no badge to keep in sync.
        unreadCount = page.folderUnread.takeIf { role == INBOX_FOLDER },
        nextCursor = page.nextCursor,
        folderSynced = page.folderSynced,
        searchIncomplete = page.searchIncomplete,
    )
}

/**
 * The unified starred listing. Unlike the other unified folders this is not a
 * mailbox any account owns: the core returns the starred *items* themselves —
 * single messages and feed entries across every account — which the list shows
 * as rows the same way a thread is shown.
 */
internal suspend fun MeronMobileState.loadUnifiedStarred(
    client: MobileMailCommandClient,
    query: String = mailSearch,
    filter: FilterMode = mailFilter,
    attachmentsOnly: Boolean = mailAttachmentsOnly,
    beforeCursor: String? = null,
    limit: Int = MAILBOX_PAGE_SIZE,
): MailboxLoadResult {
    val page =
        parseStarredItemsPage(
            client.listStarredItems(
                StarredItemsParams(
                    query = query.trim(),
                    filter = filter.protocolValue(),
                    attachments = attachmentsOnly,
                    limit = limit,
                    beforeCursor = beforeCursor,
                ),
            ),
        )
    val rows = page.items.map { it.toThreadSummary() }
    return MailboxLoadResult(
        folders = emptyList(),
        folder = STARRED_FOLDER,
        threads = rows,
        nextCursor = page.nextCursor,
    )
}

internal suspend fun MeronMobileState.loadAccountFolders(
    client: MobileMailCommandClient,
    account: AccountSummary,
): List<FolderSummary> {
    val foldersJson = client.listFolders(FolderListParams(accountId = account.id))
    return parseFolderListResponse(foldersJson)
}
