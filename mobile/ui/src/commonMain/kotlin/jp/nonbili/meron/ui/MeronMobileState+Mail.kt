package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountAliasParams
import jp.nonbili.meron.shared.AccountAliasesParams
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.EmptyFolderParams
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderDeleteParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.bareAddress
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardInlineImages
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.forwardedHtmlQuote
import jp.nonbili.meron.shared.inlineImageToDraftAttachment
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.mailThreadIdFolder
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.notificationThreadId
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderDeleteResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseFolderUnreadChanges
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadActionLocationResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.rewriteMediaRefsToCid
import jp.nonbili.meron.shared.splitAddressList
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

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
): MailboxCacheKey =
    MailboxCacheKey(
        accountId = accountId.ifBlank { UNIFIED_ACCOUNT_ID },
        folderId = folderId.ifBlank { INBOX_FOLDER }.lowercase(),
        query = query.trim(),
        filter = filter,
    )

private fun MeronMobileState.cacheVisibleMailbox() {
    if (!initialThreadsLoaded) return
    val accountId = selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val folderId = selectedCoreFolder.ifBlank { INBOX_FOLDER }
    val key = visibleMailboxKey ?: mailboxCacheKey(accountId, folderId, mailSearch, mailFilter)
    mailboxCache =
        mailboxCache +
        (
            key to
                MailboxLoadResult(
                    folders = coreFolders,
                    folder = folderId,
                    threads = withLocalDraftFlags(coreThreads),
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
    val key = mailboxCacheKey(accountId, folderId, mailSearch, mailFilter)
    val cached = mailboxCache[key] ?: return false
    coreFolders = cached.folders
    if (cached.folders.isNotEmpty()) {
        foldersByAccount = foldersByAccount + cached.folders.groupBy { it.accountId }
    }
    selectedCoreFolder = cached.folder
    coreThreads = withLocalDraftFlags(cached.threads)
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
    folderId: String = INBOX_FOLDER,
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
    val requestKey = mailboxCacheKey(accountId, requestedFolder, query, filter)
    if (syncing && activeMailboxLoadKey == requestKey) {
        Log.i("MailLoad", "sync skipped duplicate account=$accountId folder=$requestedFolder")
        return
    }
    val requestToken = activeMailboxLoadToken + 1
    activeMailboxLoadToken = requestToken
    activeMailboxLoadKey = requestKey
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
    val listLimit =
        if (visibleMailboxKey == requestKey) {
            mailboxPageDepth.coerceIn(MAILBOX_PAGE_SIZE, MAILBOX_MAX_RELOAD_DEPTH)
        } else {
            MAILBOX_PAGE_SIZE
        }
    Log.i(
        "MailLoad",
        "sync start account=$accountId folder=$requestedFolder accounts=${selectedAccounts.size} syncFirst=$syncFirst limit=$syncLimit listLimit=$listLimit query=${query.isNotBlank()} filter=${filter.protocolValue()}",
    )
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (unifiedStarred) {
                    // The starred listing spans folders, so there is no mailbox
                    // to sync first: the core reads whatever the accounts'
                    // own syncs have already starred.
                    loadUnifiedStarred(client = client, query = query, filter = filter, limit = listLimit)
                } else if (accountId == UNIFIED_ACCOUNT_ID) {
                    loadUnifiedInbox(
                        client = client,
                        accounts = selectedAccounts,
                        query = query,
                        filter = filter,
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
                        syncFirst = syncFirst,
                        syncLimit = syncLimit,
                        listLimit = listLimit,
                        refreshSearch = refreshSearch,
                    )
                }
            }
        }.onSuccess { result ->
            val resultKey = mailboxCacheKey(accountId, result.folder, query, filter)
            mailboxCache =
                mailboxCache +
                (
                    resultKey to
                        result.copy(
                            folders = result.folders,
                            folder = result.folder,
                            threads = withLocalDraftFlags(withoutLocallyDiscardedThreads(result.threads)),
                            nextCursor = result.nextCursor,
                            accountCursors = result.accountCursors,
                            pageDepth = listLimit,
                        )
                )
            if (activeMailboxLoadToken != requestToken) {
                Log.w("MailLoad", "sync ignored stale result account=$accountId folder=${result.folder} threads=${result.threads.size}")
                return@onSuccess
            }
            val wasInitialLoad = !initialThreadsLoaded
            val existingIds = coreThreads.map { it.id }.toSet()
            coreFolders = result.folders
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            val folder = result.folder
            selectedCoreFolder = folder
            val parsedThreads = withLocalDraftFlags(withoutLocallyDiscardedThreads(result.threads))
            coreThreads = parsedThreads
            visibleMailboxKey = resultKey
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            mailboxPageDepth = listLimit
            // The open conversation is deliberately left alone here. A refresh
            // returns one page of one mailbox, so the open thread being absent
            // means nothing (it may sit past the page limit, or belong to
            // another account/folder when opened from kanban, starred or a
            // notification). Clearing it on that basis raced every thread open
            // — background syncs run continuously — and left the thread screen
            // with no summary and no messages, spinning forever. Selections
            // that really go away are cleared by the move/archive paths.
            activeMailboxLoadKey = null
            activeMailboxLoadStartedAtMillis = 0L
            blockingMailboxLoadWarned = false
            blockingMailboxLoadSlow = false
            syncing = false
            initialThreadsLoaded = true
            errorBanner = null
            syncError = null
            if (scrollToTopOnSuccess) {
                mailListScrollToTopRequest += 1
            }
            val newCount = if (!wasInitialLoad && syncFirst) parsedThreads.count { it.id !in existingIds } else 0
            status = successStatus ?: if (newCount > 0) "$newCount new message(s)" else ""
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
            if (firstLoad && syncFirst && !unifiedStarred) {
                deepenMailboxSync(accountId, folder, selectedAccounts)
            }
        }.onFailure {
            if (activeMailboxLoadToken != requestToken) {
                Log.w("MailLoad", "sync ignored stale failure account=$accountId", it)
                return@onFailure
            }
            activeMailboxLoadKey = null
            activeMailboxLoadStartedAtMillis = 0L
            blockingMailboxLoadWarned = false
            blockingMailboxLoadSlow = false
            syncing = false
            initialThreadsLoaded = true
            val contextual = it as? AccountSyncException
            val failedAccountId =
                contextual?.accountId
                    ?: accountId.takeUnless { candidate -> candidate == UNIFIED_ACCOUNT_ID }
            val message = contextual?.cause?.message ?: it.message ?: "Sync failed"
            syncError = MobileSyncError(failedAccountId, message)
            errorBanner = null
            status = "Sync failed: ${it.message}"
            Log.w("MailLoad", "sync failed account=$accountId folder=$requestedFolder initialThreadsLoaded=$initialThreadsLoaded syncing=$syncing", it)
        }
    }
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
    val selectedAccounts = pageableCoreAccounts()
    if (selectedAccounts.isEmpty()) return
    loadingMoreThreads = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (accountId == UNIFIED_ACCOUNT_ID && isUnifiedStarredFolder(requestedFolder)) {
                    loadUnifiedStarred(client = client, query = query, filter = filter, beforeCursor = mailboxCursor)
                } else if (accountId == UNIFIED_ACCOUNT_ID) {
                    loadUnifiedInbox(
                        client = client,
                        accounts = selectedAccounts,
                        query = query,
                        filter = filter,
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
                        syncFirst = false,
                        beforeCursor = mailboxCursor,
                    )
                }
            }
        }.onSuccess { result ->
            if (result.folders.isNotEmpty()) {
                coreFolders = result.folders
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            val existingIds = coreThreads.map { it.id }.toSet()
            val appended = withLocalDraftFlags(result.threads).filterNot { it.id in existingIds }
            coreThreads = (coreThreads + appended).sortedByDescending { it.dateEpochSeconds }
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            // One more page is on screen, so event-driven reloads have to re-read
            // this deep to keep it there.
            mailboxPageDepth = (mailboxPageDepth + MAILBOX_PAGE_SIZE).coerceAtMost(MAILBOX_MAX_RELOAD_DEPTH)
            cacheVisibleMailbox()
            loadingMoreThreads = false
            errorBanner = null
            if (!quiet) {
                status = if (appended.isEmpty()) "No older messages." else "Loaded ${appended.size} older message(s)."
            }
        }.onFailure {
            loadingMoreThreads = false
            errorBanner = it.message ?: "Load more failed"
            status = "Load more failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.openDraftCompose(
    message: MessageBody,
    thread: ThreadSummary,
    returnScreen: Screen = Screen.Mail,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val generation = ++composeSessionGeneration
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val copied =
                    forwardableAttachments(message).mapNotNull { attachment ->
                        val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                        data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                    }
                // A saved forward keeps its quoted HTML; without re-deriving it
                // here, reopening the draft would silently downgrade it to the
                // plain body on the next send. The reader rewrote the quote's
                // cid: refs to /media/ paths on the way in, so the inline images
                // have to be resolved again too.
                val quote = forwardedHtmlQuote(message.bodyHtml)
                val inlineImages =
                    if (quote.isBlank()) {
                        emptyList()
                    } else {
                        forwardInlineImages(message).mapNotNull { image ->
                            val data =
                                runCatching {
                                    parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(image.attachment.key)))
                                }.getOrNull()
                            data?.takeIf { it.isNotBlank() }?.let { image to it }
                        }
                    }
                val availableImages = inlineImages.map { it.first }
                val rewrittenQuote =
                    forwardInlineImages(message).fold(rewriteMediaRefsToCid(quote, availableImages)) { html, image ->
                        html.replace("/media/${image.attachment.key.trim()}", "")
                    }
                Triple(
                    copied,
                    rewrittenQuote,
                    inlineImages.map { (image, data) -> inlineImageToDraftAttachment(image, data) },
                )
            }
        }.onSuccess { (copiedAttachments, forwardHtml, inlineAttachments) ->
            if (generation != composeSessionGeneration) return@onSuccess
            // Start from a clean composer: a draft opened after a reply would
            // otherwise inherit that reply's threading headers and be sent into
            // the wrong conversation. The draft's own headers are restored just
            // below — a reply draft has to keep threading where it belongs.
            clearComposeDraftState()
            to = message.to
            cc = message.cc
            bcc = message.bcc
            subject = message.subject
            body = message.body
            // A draft written earlier already carries whatever signature it was
            // written with, so the body stays unmanaged: a later change of From
            // must not rewrite part of it, nor append a second signature.
            composeSignature = null
            attachments = copiedAttachments
            composeForwardHtml = forwardHtml
            composeForwardInlineAttachments = inlineAttachments
            composeFromAccountId = thread.accountId
            composeFromEmail = ""
            // A draft with no Message-ID of its own — imported, or written by
            // something that omitted the header — cannot be addressed on the
            // server: a discard would search for a header that isn't there. The
            // composer takes an id of its own and treats it as unsaved, so its
            // first save creates that draft properly instead of claiming one
            // that was never written.
            val openedDraftId = message.messageId.trim().trim('<', '>')
            composeDraftId = openedDraftId.ifBlank { newDraftMessageId(thread.accountId) }
            composeDraftSaved = openedDraftId.isNotBlank()
            composeDraftAccountId = thread.accountId
            composeInReplyTo = message.inReplyTo
            composeReferences = message.references
            composeReturnScreen = returnScreen
            rememberComposeSeed()
            screen = Screen.Compose
            status = "Draft ready"
        }.onFailure {
            if (generation != composeSessionGeneration) return@onFailure
            status = "Draft open failed: ${it.message}"
        }
    }
}

internal fun draftThreadShouldOpenConversation(messages: List<MessageBody>): Boolean =
    messages.any { it.folderId.isNotBlank() && !folderIsDrafts(it.folderId) } ||
        messages.any { folderIsDrafts(it.folderId) && (it.references.isNotBlank() || it.inReplyTo.isNotBlank()) }

internal fun MeronMobileState.readCoreThread(
    thread: ThreadSummary,
    sourceFolder: String = thread.folder,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val backendThreadId = thread.backendThreadId()
    val readToken = activeThreadReadToken + 1
    activeThreadReadToken = readToken
    val returnScreen = if (screen == Screen.Kanban) screen else Screen.Mail
    val readsDraftThread =
        !threadIdIsRss(backendThreadId) &&
            (thread.folderRole == "drafts" || (thread.folderRole == "folder" && (folderIsDrafts(sourceFolder) || folderIsDrafts(thread.folder))))
    val selectedThread = if (sourceFolder.isNotBlank() && sourceFolder != thread.folder) thread.copy(folder = sourceFolder) else thread
    selectedCoreThread = selectedThread
    messages = emptyList()
    messageCursor = ""
    loadingMoreMessages = false
    previousTopScreen = returnScreen
    if (quickReplyThreadId != backendThreadId) {
        ++quickReplyGeneration
        quickReplyAutosaveJob?.cancel()
        quickReplyAttachments = emptyList()
        quickReplyFailure = ""
        quickReplyDraftId = ""
        quickReplyDraftSaved = false
        quickReplyInReplyTo = ""
        quickReplyReferences = ""
        quickReplyFrom = ""
        quickReplyThreadId = backendThreadId
        // Starts the new thread's bar on the replying account's signature rather
        // than blank — the bar is what gets sent, so it shows what will go out.
        // Set after quickReplyThreadId, which the late-signature re-seed keys on.
        seedQuickReplySignature()
    }
    if (!readsDraftThread) {
        screen = Screen.Thread
    }
    // Nothing is marked read on open: messages are marked incrementally as the
    // user scrolls past them (see the scroll-driven marking in ThreadUi),
    // mirroring desktop.
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (threadIdIsRss(backendThreadId)) {
                    client.readRssThread(RssThreadParams(threadId = backendThreadId))
                } else {
                    withManagedGoogleAuth(client, selectedThread.accountId) {
                        client.readThread(ThreadReadParams(threadId = backendThreadId))
                    }
                }
            }
        }.onSuccess {
            // Drop any superseded read, including an older request for this
            // same conversation after the user backed out and reopened it.
            if (activeThreadReadToken != readToken || selectedCoreThread?.backendThreadId() != backendThreadId) {
                return@onSuccess
            }
            val page = parseThreadReadPage(it)
            messages = mergeLocalSendMessages(messages, page.messages)
            messageCursor = page.nextCursor
            hydrateQuickReplyFromTailDraft(backendThreadId, messages)
            if (readsDraftThread) {
                val draftMessage =
                    page.messages.lastOrNull { message -> folderIsDrafts(message.folderId) }
                        ?: page.messages.lastOrNull()
                if (draftThreadShouldOpenConversation(page.messages)) {
                    screen = Screen.Thread
                } else {
                    draftMessage?.let { message ->
                        openDraftCompose(message, selectedThread, returnScreen = returnScreen)
                    }
                }
            }
        }.onFailure {
            if (activeThreadReadToken != readToken) return@onFailure
            status = "Could not open message: ${it.message}"
        }
    }
}

// Pre-fills the quick-reply bar from an already-saved draft reply sitting at
// the tail of the conversation, so the user can continue and send it inline
// instead of being forced into the full editor. No-op when the tail message
// isn't a draft, or is already the one loaded (e.g. re-entrant calls from
// loadMoreThreadMessages).
internal fun MeronMobileState.hydrateQuickReplyFromTailDraft(
    threadBackendId: String,
    mergedMessages: List<MessageBody>,
) {
    if (quickReplyThreadId != threadBackendId) return
    val tail = mergedMessages.lastOrNull() ?: return
    if (!folderIsDrafts(tail.folderId)) return
    val normalizedTailId = tail.messageId.normalizedComposeDraftId()
    // A draft a send already consumed, whose discard has not come back yet: it
    // holds the text we just sent, not a reply left unfinished.
    if (normalizedTailId.isNotBlank() && normalizedTailId in quickReplyConsumedDraftIds) return
    if (quickReplyDraftId.isNotBlank() && quickReplyDraftId.normalizedComposeDraftId() == normalizedTailId) return
    // Only a draft we can address on the server. A row synced from its envelope
    // carries no Message-ID yet — and no body either — so taking it would put an
    // empty bar in front of the user calling itself their saved draft, under an
    // id that names nothing: the next save would append a second copy and leave
    // this one stranded. Reading the thread back-fills the header.
    val tailDraftId = tail.messageId.trim().trim('<', '>')
    if (tailDraftId.isBlank()) return
    // Only into a bar that is free. Hydration fills an empty reply from a saved
    // draft; it is not entitled to replace a reply the user began before this
    // read came back, nor to drop the id that reply is already saved under.
    if (!quickReplyIsBlank() || quickReplyDraftId.isNotBlank()) return
    quickReplyBody = tail.body
    ++quickReplyGeneration
    quickReplyDraftId = tailDraftId
    quickReplyDraftSaved = true
    quickReplyInReplyTo = tail.inReplyTo
    quickReplyReferences = tail.references
    quickReplyFrom = tail.fromAddr
    quickReplyFailure = ""
    // The saved body already carries whatever signature it was written with, so
    // none of it is this app's to strip, re-seed, or discount as "not content".
    quickReplySignature = null
    if (!tail.hasAttachments) {
        quickReplyAttachments = emptyList()
        return
    }
    scope.launch {
        val copied =
            runCatching {
                withContext(ioDispatcher) {
                    val client = MobileMailCommandClient(core)
                    forwardableAttachments(tail).mapNotNull { attachment ->
                        val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                        data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                    }
                }
            }.getOrElse { emptyList() }
        if (quickReplyThreadId == threadBackendId && quickReplyDraftId.normalizedComposeDraftId() == normalizedTailId) {
            quickReplyAttachments = copied
            ++quickReplyGeneration
        }
    }
}

internal fun ThreadSummary.backendThreadId(): String = threadId.ifBlank { id }

/** Item keys are present only when a starred RSS row represents one article. */
internal fun ThreadSummary.rssItemKeys(): List<String> = listOf(id).takeIf { threadIdIsRss(backendThreadId()) && threadId.isNotBlank() && threadId != id }.orEmpty()

// Re-read the currently open thread and replace its message list with the
// canonical copy from the core. Used after sending a quick reply so the stored
// sent message replaces the optimistic one. Runs on ioDispatcher; guards against
// the user having switched threads while the read was in flight.
internal suspend fun MeronMobileState.reloadCurrentThreadMessages() {
    val thread = selectedCoreThread ?: return
    if (!coreLoaded) return
    val response =
        withContext(ioDispatcher) {
            val client = MobileMailCommandClient(core)
            if (threadIdIsRss(thread.id)) {
                client.readRssThread(RssThreadParams(threadId = thread.id))
            } else {
                withManagedGoogleAuth(client, thread.accountId) {
                    client.readThread(ThreadReadParams(threadId = thread.id))
                }
            }
        }
    if (selectedCoreThread?.id != thread.id) return
    val page = parseThreadReadPage(response)
    messages = mergeLocalSendMessages(messages, page.messages)
    messageCursor = page.nextCursor
}

// Retry loading bodies for the open thread. Re-reading is enough: the core
// re-attempts the on-demand IMAP fetch for any message without a cached body.
internal fun MeronMobileState.retryOpenThreadLoad() {
    scope.launch {
        runCatching { reloadCurrentThreadMessages() }
            .onFailure { status = "Could not open message: ${it.message}" }
    }
}

internal fun mergeLocalSendMessages(
    current: List<MessageBody>,
    refreshed: List<MessageBody>,
): List<MessageBody> {
    val refreshedIds = refreshed.map { it.id }.toSet()
    val refreshedMessageIds =
        refreshed
            .mapNotNull { it.messageId.normalizedMessageId().takeIf(String::isNotBlank) }
            .toSet()
    val unresolved =
        current.filter { message ->
            val localSend = message.id.startsWith("local-send-")
            val localDraft = message.id.startsWith("local-draft-")
            if (!localSend && !localDraft && message.sendStatus == SendStatus.None) return@filter false
            if (message.id in refreshedIds) return@filter false
            val messageId = message.messageId.normalizedMessageId()
            messageId.isBlank() || messageId !in refreshedMessageIds
        }
    // Fallback candidates: outgoing, non-draft rows this read newly revealed. A
    // message we were already showing before the send cannot be its server
    // copy, and a draft — even one holding this very reply — is not a sent copy.
    val knownIds = current.map { it.id }.toSet()
    val candidates = refreshed.filter { it.outgoing && it.id !in knownIds && !folderIsDrafts(it.folderId) }
    val paired = pairLocalSendsWithServerCopies(unresolved.filter { it.id.startsWith("local-send-") }, candidates)
    val local = unresolved.filter { it.id !in paired }
    if (local.isEmpty()) return refreshed
    return (refreshed + local).sortedBy { it.dateEpochSeconds }
}

// How far the server's Date header may sit from the moment we rendered the
// bubble and still be the same message — enough for a slow submission plus
// modest clock skew, short enough not to swallow a genuinely later reply.
private const val SENT_COPY_MATCH_WINDOW_SECONDS = 600L

// Match optimistic bubbles to the server's copies of them when the Message-ID
// we generated did not come back. Proton Bridge replaces that id with one of
// its own (`@protonmail.internalid`), so identity has to come from the
// envelope: same sender, same subject, same recipients, and a send time close
// to when we rendered the bubble.
//
// Two replies into one thread share every one of those fields, so pairing is
// decided globally rather than by first match: every plausible pair is ranked
// by whether the content matches and then by how far apart the two times are,
// and pairs are taken best-first. That keeps a copy arriving out of order from
// claiming the wrong bubble — which would hide one reply and show the other
// twice — while still settling on time alone when a server reflows the body it
// stored and no content match exists.
//
// Returns the ids of the bubbles that found a copy.
private fun pairLocalSendsWithServerCopies(
    locals: List<MessageBody>,
    candidates: List<MessageBody>,
): Set<String> {
    val ranked =
        locals
            .flatMap { local ->
                candidates
                    .filter { candidate -> isPlausibleSentCopy(local, candidate) }
                    .map { candidate ->
                        Triple(
                            local.id to candidate.id,
                            if (local.contentSignature() == candidate.contentSignature()) 0 else 1,
                            abs(candidate.dateEpochSeconds - local.dateEpochSeconds),
                        )
                    }
            }.sortedWith(compareBy({ it.second }, { it.third }))

    val pairedLocals = mutableSetOf<String>()
    val claimed = mutableSetOf<String>()
    for ((pair, _, _) in ranked) {
        val (localId, candidateId) = pair
        if (localId in pairedLocals || candidateId in claimed) continue
        pairedLocals += localId
        claimed += candidateId
    }
    return pairedLocals
}

// The envelope test every pair must clear before ranking.
private fun isPlausibleSentCopy(
    local: MessageBody,
    candidate: MessageBody,
): Boolean {
    if (bareAddress(candidate.fromAddr).lowercase() != bareAddress(local.fromAddr).lowercase()) return false
    if (candidate.subject.trim() != local.subject.trim()) return false
    if (candidate.recipientKey() != local.recipientKey()) return false
    return abs(candidate.dateEpochSeconds - local.dateEpochSeconds) <= SENT_COPY_MATCH_WINDOW_SECONDS
}

// What distinguishes two replies that share an envelope: what they say and what
// they carry. Whitespace-insensitive, since a server may rewrap the body it
// stored — a mismatch demotes a pair rather than rejecting it.
private fun MessageBody.contentSignature(): String {
    val normalizedBody =
        body
            .split(whitespaceRun)
            .filter { it.isNotBlank() }
            .joinToString(" ")
            .lowercase()
    val files = attachments.map { it.filename.trim().lowercase() }.sorted().joinToString("|")
    return "$files\u0000$normalizedBody"
}

private val whitespaceRun = Regex("\\s+")

// Order-independent set of the bare To/Cc addresses, for envelope comparison.
private fun MessageBody.recipientKey(): String =
    (splitAddressList(to) + splitAddressList(cc))
        .map { bareAddress(it).lowercase() }
        .filter { it.isNotBlank() }
        .sorted()
        .joinToString(",")

private fun String.normalizedMessageId(): String = trim().trim('<', '>').lowercase()

// Re-read the open thread on a push/sync event so live IDLE updates (new mail,
// or our own sent copy) appear in the conversation, not just the thread list.
// Mirrors desktop's refreshOpenThread: skip when the event is for a different
// account than the open thread (it may differ from the selected mailbox account
// in unified / kanban / starred views).
internal suspend fun MeronMobileState.refreshOpenThreadFor(eventAccount: String) {
    val open = selectedCoreThread ?: return
    if (eventAccount.isNotBlank() && open.accountId.isNotBlank() && open.accountId != eventAccount) {
        return
    }
    runCatching { reloadCurrentThreadMessages() }
}

/** Opens the conversation a tapped notification names, layered over whatever
 *  the user was looking at: the mailbox behind it keeps its account, folder,
 *  search and filter, so backing out returns to the Mail or Kanban view the tap
 *  interrupted rather than to the notification's own folder. The thread list is
 *  loaded only to find the thread and is then discarded. */
internal fun MeronMobileState.openNotificationThread(target: NotificationThreadTarget) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    if (target.threadKey.isBlank()) {
        openNotificationMailbox(target)
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val accounts =
                    coreAccounts.takeIf { accounts -> accounts.any { it.id == target.accountId } }
                        ?: parseAccountListResponse(client.listAccounts())
                val account =
                    accounts.firstOrNull { it.id == target.accountId }
                        ?: error("Account not found: ${target.accountId}")
                val expectedThreadId = notificationThreadId(target.accountId, target.folder, target.threadKey)
                var result =
                    loadAccountInbox(
                        client = client,
                        account = account,
                        requestedFolder = target.folder,
                        query = "",
                        filter = FilterMode.All,
                        syncFirst = false,
                    )
                var thread = result.threads.firstOrNull { it.id == expectedThreadId }
                if (thread == null) {
                    result =
                        loadAccountInbox(
                            client = client,
                            account = account,
                            requestedFolder = target.folder,
                            query = "",
                            filter = FilterMode.All,
                            syncFirst = true,
                        )
                    thread = result.threads.firstOrNull { it.id == expectedThreadId }
                }
                Triple(accounts, result, thread ?: error("Thread not found"))
            }
        }.onSuccess { (accounts, result, thread) ->
            // Only what the thread view itself needs, never the mailbox state:
            // the account list a cold start has not fetched yet, and the folder
            // names the notification's account is filed under.
            if (coreAccounts.isEmpty()) {
                coreAccounts = accounts
            }
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            readCoreThread(thread)
        }.onFailure {
            status = "Could not open notification: ${it.message}"
        }
    }
}

/** A group summary names an account and folder but no conversation, so this one
 *  does navigate the mailbox. Hiding an account from the side nav only drops it
 *  from the drawer list; its mail is still reachable, including from here. */
private fun MeronMobileState.openNotificationMailbox(target: NotificationThreadTarget) {
    mailSearch = ""
    mailFilter = FilterMode.All
    selectedCoreAccountId = target.accountId
    selectedCoreFolder = target.folder
    syncing = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val accounts =
                    coreAccounts.takeIf { accounts -> accounts.any { it.id == target.accountId } }
                        ?: parseAccountListResponse(client.listAccounts())
                val account =
                    accounts.firstOrNull { it.id == target.accountId }
                        ?: error("Account not found: ${target.accountId}")
                val result =
                    loadAccountInbox(
                        client = client,
                        account = account,
                        requestedFolder = target.folder,
                        query = "",
                        filter = FilterMode.All,
                        syncFirst = false,
                    )
                accounts to result
            }
        }.onSuccess { (accounts, result) ->
            if (coreAccounts.isEmpty()) {
                coreAccounts = accounts
            }
            coreFolders = result.folders
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            selectedCoreFolder = result.folder
            coreThreads = withLocalDraftFlags(result.threads)
            visibleMailboxKey = mailboxCacheKey(target.accountId, result.folder, "", FilterMode.All)
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            mailboxPageDepth = MAILBOX_PAGE_SIZE
            syncing = false
            initialThreadsLoaded = true
            selectedMailThreadIds = emptySet()
            // Loading the mailbox is not enough on its own: nothing else on this
            // path moves the app off the screen it was on, so the tap would land
            // behind Settings, Kanban, Compose, or a thread left open.
            selectedCoreThread = null
            previousTopScreen = Screen.Mail
            screen = Screen.Mail
        }.onFailure {
            syncing = false
            status = "Could not open notification: ${it.message}"
        }
    }
}

internal fun MeronMobileState.loadMoreThreadMessages() {
    val thread = selectedCoreThread ?: return
    if (!coreLoaded || messageCursor.isBlank() || loadingMoreMessages) return
    val cursor = messageCursor
    loadingMoreMessages = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (threadIdIsRss(thread.id)) {
                    client.readRssThread(RssThreadParams(threadId = thread.id, beforeCursor = cursor))
                } else {
                    withManagedGoogleAuth(client, thread.accountId) {
                        client.readThread(ThreadReadParams(threadId = thread.id, beforeCursor = cursor))
                    }
                }
            }
        }.onSuccess {
            val page = parseThreadReadPage(it)
            val existingIds = messages.map { message -> message.id }.toSet()
            val older = page.messages.filterNot { message -> message.id in existingIds }
            messages = (older + messages).sortedBy { message -> message.dateEpochSeconds }
            messageCursor = page.nextCursor
            loadingMoreMessages = false
            status = if (older.isEmpty()) "No older messages in this thread." else "Loaded ${older.size} older message(s)."
        }.onFailure {
            loadingMoreMessages = false
            status = "Could not load older messages: ${it.message}"
        }
    }
}

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
    coreThreads = update(coreThreads)
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            val nextThreads = update(state.threads)
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
    val changes = parseFolderUnreadChanges(response)
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
    foldersByAccount = nextByAccount
    coreFolders =
        coreFolders.map { folder ->
            changes
                .firstOrNull {
                    it.accountId == folder.accountId && folder.name.equals(it.folderId, ignoreCase = folder.role == "inbox")
                }?.let { folder.copy(unread = it.unread) } ?: folder
        }
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
    status = if (starred) "Starring..." else "Unstarring..."
    scope.launch {
        runCatching {
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
            }
        }.onSuccess {
            updateMessageEverywhere(message.id) { it.copy(starred = starred) }
            val updatedStarred = messages.any { it.starred }
            updateThreadEverywhere(thread) { it.copy(starred = updatedStarred) }
            selectedCoreThread = selectedCoreThread?.copy(starred = updatedStarred)
            status = if (starred) "Starred" else "Unstarred"
        }.onFailure {
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
    val mailTargets =
        if (unifiedStarred) {
            emptyList()
        } else if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts
                .filter { it.includedInUnified && !accountSummaryIsRss(it) }
                .map { account -> account.id to INBOX_FOLDER }
        } else {
            val account = accountsById[selectedCoreAccountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(selectedCoreAccountId to selectedCoreFolder) else emptyList()
        }
    val rssTargets = unread.filter { threadIdIsRss(it.backendThreadId()) }
    if (mailTargets.isEmpty() && starredTargets.isEmpty() && rssTargets.isEmpty()) {
        status = "No unread messages."
        return
    }
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    coreThreads = coreThreads.map { if (it.unread) it.copy(unread = false) else it }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.map { if (it.unread) it.copy(unread = false) else it })
        }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val responses = mutableListOf<String>()
                starredTargets.forEach { (threadId, messageIds) ->
                    responses += requireCoreOk(client.markRead(MarkReadParams(threadId = threadId, messageIds = messageIds)))
                }
                if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID && mailTargets.isNotEmpty()) {
                    mailTargets.forEach { (accountId, _) -> withManagedGoogleAuth(client, accountId) { "" } }
                    responses +=
                        requireCoreOk(
                            client.markAllRead(MarkAllReadParams(accountId = UNIFIED_ACCOUNT_ID, folderId = INBOX_FOLDER)),
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
            status = "Mark all read failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.markKanbanColumnAllRead(column: KanbanColumnSpec) {
    val key = kanbanColumnKey(column)
    val unread = kanbanColumns[key]?.threads.orEmpty().filter { it.unread }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val accountsById = coreAccounts.associateBy { it.id }
    val mailTargets =
        if (isUnifiedStarredColumn(column)) {
            unread
                .filterNot { threadIdIsRss(it.id) }
                .map { thread -> thread.backendThreadId() to listOf(thread.id) }
        } else if (column.accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts
                .filter { it.includedInUnified && !accountSummaryIsRss(it) }
                .map { account -> account.id to emptyList<String>() }
        } else {
            val account = accountsById[column.accountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(column.accountId to emptyList()) else emptyList()
        }
    val rssTargets = unread.filter { threadIdIsRss(it.backendThreadId()) }
    if (mailTargets.isEmpty() && rssTargets.isEmpty()) {
        status = "No unread cards."
        return
    }
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    updateKanbanColumn(key) { state ->
        state.copy(
            threads = state.threads.map { if (it.unread) it.copy(unread = false) else it },
            unreadCount = 0,
        )
    }
    coreThreads =
        coreThreads.map { thread ->
            if (unread.any { it.id == thread.id }) thread.copy(unread = false) else thread
        }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val responses = mutableListOf<String>()
                if (column.accountId == UNIFIED_ACCOUNT_ID && !isUnifiedStarredColumn(column)) {
                    mailTargets.forEach { (accountId, _) -> withManagedGoogleAuth(client, accountId) { "" } }
                    responses +=
                        requireCoreOk(
                            client.markAllRead(MarkAllReadParams(accountId = UNIFIED_ACCOUNT_ID, folderId = column.folderId)),
                        )
                } else {
                    mailTargets.forEach { (target, messageIds) ->
                        if (isUnifiedStarredColumn(column)) {
                            responses += requireCoreOk(client.markRead(MarkReadParams(threadId = target, messageIds = messageIds)))
                        } else {
                            responses +=
                                requireCoreOk(
                                    withManagedGoogleAuth(client, target) {
                                        client.markAllRead(MarkAllReadParams(accountId = target, folderId = column.folderId))
                                    },
                                )
                        }
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
            status = "Marked ${unread.size} Kanban card(s) read"
        }.onFailure {
            Log.w("Mail", "kanban mark all read failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            status = "Kanban mark all read failed: ${it.message}"
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

private fun List<FolderSummary>.hasOnlyBootstrapInbox(): Boolean = size == 1 && first().name.equals(INBOX_FOLDER, ignoreCase = true)

internal fun MeronMobileState.ensureThreadActionFolders(
    thread: ThreadSummary,
    includeAllMailAccounts: Boolean,
    onReady: () -> Unit,
) {
    if (threadIdIsRss(thread.id)) {
        status = "RSS feeds move between RSS accounts from Kanban."
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
        status = "No mail folders available."
        return
    }
    status = "Loading folders..."
    scope.launch {
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
            foldersByAccount = foldersByAccount + loadedFolders
            status = "Loaded folders"
            onReady()
        }.onFailure {
            status = "Load folders failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.moveThreadToFolder(
    thread: ThreadSummary,
    targetFolderId: String,
    onMoved: () -> Unit = {},
) {
    if (threadIdIsRss(thread.id)) {
        status = "RSS feeds move between RSS accounts from Kanban."
        return
    }
    if (targetFolderId.equals(thread.folder, ignoreCase = true)) {
        status = "Already in ${targetFolderId.replaceFirstChar { it.uppercase() }}."
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
    // Remove the row optimistically, but keep an open conversation selected
    // until the move succeeds. Clearing it here makes the thread-route fallback
    // pop immediately, then the success callback pops the origin route too.
    coreThreads = coreThreads.filterNot { it.id == thread.id }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.filterNot { it.id == thread.id })
        }
    status = "Moving..."
    scope.launch {
        runCatching {
            requireCoreOk(
                withContext(ioDispatcher) {
                    MobileMailCommandClient(core).move(
                        MoveThreadParams(threadId = thread.id, targetFolderId = targetFolderId),
                    )
                },
            )
        }.onSuccess {
            if (selectedCoreThread?.id == thread.id) {
                selectedCoreThread = null
                messages = emptyList()
            }
            status = "Move complete"
            onMoved()
        }.onFailure {
            Log.w("Mail", "move thread failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            selectedCoreThread = selectedBefore
            messages = messagesBefore
            status = "Move failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.copyThreadToFolder(
    thread: ThreadSummary,
    target: FolderSummary,
) {
    if (threadIdIsRss(thread.id)) {
        status = "RSS feeds can't be copied to mail folders."
        return
    }
    val targetAccountId = target.accountId.ifBlank { thread.accountId }
    val targetAccount = coreAccounts.firstOrNull { it.id == targetAccountId }
    if (targetAccount == null || accountSummaryIsRss(targetAccount)) {
        status = "Choose a mail account folder."
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    status = "Copying..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).copy(
                    CopyThreadParams(
                        threadId = thread.id,
                        targetAccountId = targetAccountId,
                        targetFolderId = target.name,
                    ),
                )
            }
        }.onSuccess {
            status = "Copy complete"
        }.onFailure {
            status = "Copy failed: ${it.message}"
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
        status = "RSS feeds move between RSS accounts from Kanban."
        return
    }
    if (trimmed.isBlank()) {
        status = "Folder name is required."
        return
    }
    val account = coreAccounts.firstOrNull { it.id == thread.accountId }
    if (account == null) {
        status = "Account not found."
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    status = "Creating folder..."
    scope.launch {
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
                    throw IllegalStateException("Already in ${createdFolder?.displayName ?: trimmed}.")
                }
                withManagedGoogleAuth(client, thread.accountId) {
                    client.move(MoveThreadParams(threadId = thread.id, targetFolderId = created))
                }
                folders to created
            }
        }.onSuccess { (folders, _) ->
            foldersByAccount = foldersByAccount + (account.id to folders)
            removeThreadEverywhere(thread.id)
            if (selectedCoreThread?.id == thread.id) {
                selectedCoreThread = null
                messages = emptyList()
            }
            status = "Folder created and move complete"
            onMoved()
        }.onFailure {
            status = "Create folder failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.moveThreadToColumn(
    thread: ThreadSummary,
    target: KanbanColumnSpec,
) {
    if (target.accountId == UNIFIED_ACCOUNT_ID) {
        status = "Move to an account folder column."
        return
    }
    val targetAccount = coreAccounts.firstOrNull { it.id == target.accountId }
    if (targetAccount == null) {
        status = "Target account not found."
        return
    }
    if (threadIdIsRss(thread.id)) {
        if (!accountSummaryIsRss(targetAccount)) {
            status = "RSS feeds can only move to RSS accounts."
            return
        }
        scope.launch {
            runCatching {
                withContext(ioDispatcher) {
                    MobileMailCommandClient(core).moveRssFeed(
                        MoveRssFeedParams(threadId = thread.id, targetAccountId = target.accountId),
                    )
                }
            }.onSuccess {
                removeThreadEverywhere(thread.id)
                loadKanbanColumn(target, refresh = false)
                status = "Move complete"
            }.onFailure {
                status = "Move failed: ${it.message}"
            }
        }
        return
    }
    if (accountSummaryIsRss(targetAccount)) {
        status = "Mail threads can't move into RSS feeds."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).move(
                    MoveThreadParams(threadId = thread.id, targetFolderId = target.folderId),
                )
            }
        }.onSuccess {
            removeThreadEverywhere(thread.id)
            loadKanbanColumn(target, refresh = false)
            status = "Move complete"
        }.onFailure {
            status = "Move failed: ${it.message}"
        }
    }
}
