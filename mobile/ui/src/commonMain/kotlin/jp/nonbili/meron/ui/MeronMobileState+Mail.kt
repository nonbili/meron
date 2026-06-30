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
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
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
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import kotlin.math.abs

private fun mailboxCacheKey(
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
    val key = mailboxCacheKey(accountId, folderId, mailSearch, mailFilter)
    mailboxCache =
        mailboxCache +
            (key to
                MailboxLoadResult(
                    folders = coreFolders,
                    folder = folderId,
                    threads = coreThreads,
                    nextCursor = mailboxCursor,
                    accountCursors = mailboxAccountCursors,
                ))
}

private fun MeronMobileState.restoreCachedMailbox(
    accountId: String,
    folderId: String,
): Boolean {
    val cached = mailboxCache[mailboxCacheKey(accountId, folderId, mailSearch, mailFilter)] ?: return false
    coreFolders = cached.folders
    if (cached.folders.isNotEmpty()) {
        foldersByAccount = foldersByAccount + cached.folders.groupBy { it.accountId }
    }
    selectedCoreFolder = cached.folder
    coreThreads = cached.threads
    mailboxCursor = cached.nextCursor
    mailboxAccountCursors = cached.accountCursors
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
    selectedCoreThread = null
    selectedMailThreadIds = emptySet()
    mailSelectionMenuOpen = false
    messages = emptyList()
    messageCursor = ""
    loadingMoreMessages = false
    if (!restoreCachedMailbox(selectedCoreAccountId, selectedCoreFolder)) {
        coreFolders = if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) coreFolders else emptyList()
        coreThreads = emptyList()
        mailboxCursor = ""
        mailboxAccountCursors = emptyMap()
        initialThreadsLoaded = false
    }
}

internal fun MeronMobileState.syncCoreThreads(
    accountOverride: String? = null,
    folderOverride: String? = null,
    syncFirst: Boolean = true,
) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    val accountId = accountOverride ?: selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val requestedFolder = folderOverride ?: selectedCoreFolder.ifBlank { INBOX_FOLDER }
    val query = mailSearch
    val filter = mailFilter
    val selectedAccounts =
        if (accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts.filter { it.includedInUnified }
        } else {
            coreAccounts.filter { it.id == accountId }
        }
    if (selectedAccounts.isEmpty()) {
        Log.w("MailLoad", "syncCoreThreads no selected accounts account=$accountId folder=$requestedFolder")
        status = if (accountId == UNIFIED_ACCOUNT_ID) "No accounts are included in Unified inbox." else "No account selected."
        initialThreadsLoaded = true
        return
    }
    val requestKey = mailboxCacheKey(accountId, requestedFolder, query, filter)
    activeMailboxLoadKey = requestKey
    syncing = true
    Log.i(
        "MailLoad",
        "sync start account=$accountId folder=$requestedFolder accounts=${selectedAccounts.size} syncFirst=$syncFirst query=${query.isNotBlank()} filter=${filter.protocolValue()}",
    )
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (accountId == UNIFIED_ACCOUNT_ID) {
                    val results =
                        selectedAccounts.map { account ->
                            account.id to
                                loadAccountInbox(
                                    client,
                                    account,
                                    INBOX_FOLDER,
                                    query = query,
                                    filter = filter,
                                    syncFirst = syncFirst,
                                )
                        }
                    MailboxLoadResult(
                        folders = results.flatMap { it.second.folders },
                        folder = INBOX_FOLDER,
                        threads = results.flatMap { it.second.threads }.sortedByDescending { it.dateEpochSeconds },
                        accountCursors =
                            results
                                .mapNotNull { (id, result) -> result.nextCursor.takeIf { it.isNotBlank() }?.let { id to it } }
                                .toMap(),
                    )
                } else {
                    loadAccountInbox(
                        client,
                        selectedAccounts.first(),
                        requestedFolder,
                        query = query,
                        filter = filter,
                        syncFirst = syncFirst,
                    )
                }
            }
        }.onSuccess { result ->
            val resultKey = mailboxCacheKey(accountId, result.folder, query, filter)
            mailboxCache =
                mailboxCache +
                    (resultKey to
                        result.copy(
                            folders = result.folders,
                            folder = result.folder,
                            threads = result.threads,
                            nextCursor = result.nextCursor,
                            accountCursors = result.accountCursors,
                        ))
            if (activeMailboxLoadKey != requestKey) {
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
            val parsedThreads = result.threads
            coreThreads = parsedThreads
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            if (selectedCoreThread?.id !in parsedThreads.map { it.id }) {
                selectedCoreThread = null
                messages = emptyList()
            }
            activeMailboxLoadKey = null
            syncing = false
            initialThreadsLoaded = true
            errorBanner = null
            val newCount = if (!wasInitialLoad && syncFirst) parsedThreads.count { it.id !in existingIds } else 0
            status = if (newCount > 0) "$newCount new message(s)" else ""
            Log.i(
                "MailLoad",
                "sync success account=$accountId folder=$folder threads=${parsedThreads.size} cursor=${mailboxCursor.isNotBlank()} accountCursors=${mailboxAccountCursors.size} initialThreadsLoaded=$initialThreadsLoaded syncing=$syncing",
            )
        }.onFailure {
            if (activeMailboxLoadKey != requestKey) {
                Log.w("MailLoad", "sync ignored stale failure account=$accountId", it)
                return@onFailure
            }
            activeMailboxLoadKey = null
            syncing = false
            initialThreadsLoaded = true
            errorBanner = it.message ?: "Sync failed"
            status = "Sync failed: ${it.message}"
            Log.w("MailLoad", "sync failed account=$accountId folder=$requestedFolder initialThreadsLoaded=$initialThreadsLoaded syncing=$syncing", it)
        }
    }
}

internal fun MeronMobileState.addFeedToSelectedRssAccount() {
    val account = coreAccounts.firstOrNull { it.id == selectedCoreAccountId }
    val feedUrl = addFeedUrl.trim()
    if (account == null || !accountSummaryIsRss(account)) {
        status = "Select an RSS account first."
        return
    }
    if (feedUrl.isBlank()) {
        status = "Feed URL is required."
        return
    }
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    status = "Adding feed..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).addRssFeed(
                    AddRssFeedParams(accountId = account.id, feedUrl = feedUrl),
                )
            }
        }.onSuccess {
            addFeedUrl = ""
            showAddFeedDialog = false
            status = "Feed added"
            syncCoreThreads(accountOverride = account.id, syncFirst = true)
        }.onFailure {
            status = "Add feed failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.loadMoreCoreThreads() {
    if (!coreLoaded || loadingMoreThreads) return
    val accountId = selectedCoreAccountId.ifBlank { UNIFIED_ACCOUNT_ID }
    val requestedFolder = selectedCoreFolder.ifBlank { INBOX_FOLDER }
    val query = mailSearch
    val filter = mailFilter
    val selectedAccounts =
        if (accountId == UNIFIED_ACCOUNT_ID) {
            coreAccounts.filter { it.includedInUnified && mailboxAccountCursors[it.id].orEmpty().isNotBlank() }
        } else {
            coreAccounts.filter { it.id == accountId && mailboxCursor.isNotBlank() }
        }
    if (selectedAccounts.isEmpty()) return
    loadingMoreThreads = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (accountId == UNIFIED_ACCOUNT_ID) {
                    val results =
                        selectedAccounts.map { account ->
                            account.id to
                                loadAccountInbox(
                                    client,
                                    account,
                                    INBOX_FOLDER,
                                    query = query,
                                    filter = filter,
                                    syncFirst = false,
                                    beforeCursor = mailboxAccountCursors[account.id],
                                )
                        }
                    MailboxLoadResult(
                        folders = results.flatMap { it.second.folders },
                        folder = INBOX_FOLDER,
                        threads = results.flatMap { it.second.threads }.sortedByDescending { it.dateEpochSeconds },
                        accountCursors =
                            results
                                .mapNotNull { (id, result) -> result.nextCursor.takeIf { it.isNotBlank() }?.let { id to it } }
                                .toMap(),
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
            val appended = result.threads.filterNot { it.id in existingIds }
            coreThreads = (coreThreads + appended).sortedByDescending { it.dateEpochSeconds }
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            cacheVisibleMailbox()
            loadingMoreThreads = false
            errorBanner = null
            status = if (appended.isEmpty()) "No older messages." else "Loaded ${appended.size} older message(s)."
        }.onFailure {
            loadingMoreThreads = false
            errorBanner = it.message ?: "Load more failed"
            status = "Load more failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.loadStarredItems() {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    syncing = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { MobileMailCommandClient(core).listStarredItems() }
        }.onSuccess {
            starredItems = parseStarredItemsResponse(it)
            syncing = false
            status = "${starredItems.size} starred item(s)"
        }.onFailure {
            syncing = false
            status = "Starred load failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.openDraftCompose(
    message: MessageBody,
    thread: ThreadSummary,
) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                forwardableAttachments(message).mapNotNull { attachment ->
                    val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                    data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                }
            }
        }.onSuccess { copiedAttachments ->
            to = message.to
            cc = message.cc
            bcc = message.bcc
            subject = message.subject
            body = message.body
            attachments = copiedAttachments
            composeFromAccountId = thread.accountId
            composeFromEmail = ""
            composeDraftId =
                message.messageId
                    .trim()
                    .trim('<', '>')
                    .ifBlank { newDraftMessageId(thread.accountId) }
            composeDraftSaved = true
            composeReturnScreen = Screen.Mail
            screen = Screen.Compose
            status = "Draft ready"
        }.onFailure {
            status = "Draft open failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.readCoreThread(thread: ThreadSummary) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    val backendThreadId = thread.backendThreadId()
    selectedCoreThread = thread
    messages = emptyList()
    messageCursor = ""
    loadingMoreMessages = false
    previousTopScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
    screen = Screen.Thread
    if (thread.unread) {
        coreThreads = coreThreads.map { if (it.id == thread.id) it.copy(unread = false) else it }
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val response =
                    if (threadIdIsRss(backendThreadId)) {
                        client.readRssThread(RssThreadParams(threadId = backendThreadId))
                    } else {
                        client.readThread(ThreadReadParams(threadId = backendThreadId))
                    }
                if (thread.unread) {
                    runCatching {
                        if (threadIdIsRss(backendThreadId)) {
                            client.markRssRead(RssMarkReadParams(threadId = backendThreadId, seen = true))
                        } else {
                            client.markRead(MarkReadParams(threadId = backendThreadId, seen = true))
                        }
                    }
                }
                response
            }
        }.onSuccess {
            val page = parseThreadReadPage(it)
            messages = mergeLocalSendMessages(messages, page.messages)
            messageCursor = page.nextCursor
            updateThreadEverywhere(thread) { current -> current.copy(unread = false) }
            if (!threadIdIsRss(thread.id) && folderIsDrafts(thread.folder)) {
                page.messages.lastOrNull()?.let { message ->
                    openDraftCompose(message, thread)
                }
            }
        }.onFailure {
            status = "Could not open message: ${it.message}"
        }
    }
}

private fun ThreadSummary.backendThreadId(): String = threadId.ifBlank { id }

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
                client.readThread(ThreadReadParams(threadId = thread.id))
            }
        }
    if (selectedCoreThread?.id != thread.id) return
    val page = parseThreadReadPage(response)
    messages = mergeLocalSendMessages(messages, page.messages)
    messageCursor = page.nextCursor
}

private fun mergeLocalSendMessages(
    current: List<MessageBody>,
    refreshed: List<MessageBody>,
): List<MessageBody> {
    val refreshedIds = refreshed.map { it.id }.toSet()
    val refreshedMessageIds =
        refreshed
            .mapNotNull { it.messageId.normalizedMessageId().takeIf(String::isNotBlank) }
            .toSet()
    val local =
        current.filter { message ->
            val localSend = message.id.startsWith("local-send-")
            if (!localSend && message.sendStatus == SendStatus.None) return@filter false
            if (message.id in refreshedIds) return@filter false
            val messageId = message.messageId.normalizedMessageId()
            messageId.isBlank() || messageId !in refreshedMessageIds
        }
    if (local.isEmpty()) return refreshed
    return (refreshed + local).sortedBy { it.dateEpochSeconds }
}

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

internal fun MeronMobileState.openNotificationThread(target: NotificationThreadTarget) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
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
                val account = accounts.firstOrNull { it.id == target.accountId }
                    ?: error("Account not found: ${target.accountId}")
                val expectedThreadId = notificationThreadId(target)
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
            if (coreAccounts.isEmpty()) {
                coreAccounts = accounts
            }
            coreFolders = result.folders
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            selectedCoreFolder = result.folder
            coreThreads = result.threads
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            syncing = false
            initialThreadsLoaded = true
            selectedMailThreadIds = emptySet()
            readCoreThread(thread)
        }.onFailure {
            syncing = false
            status = "Could not open notification: ${it.message}"
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
private fun notificationThreadId(target: NotificationThreadTarget): String {
    val folder = if (target.folder.equals(INBOX_FOLDER, ignoreCase = true)) "INBOX" else target.folder
    target.threadKey.removePrefix("uid:").takeIf { target.threadKey.startsWith("uid:") }?.let { uid ->
        return "${target.accountId}#$folder#$uid"
    }
    val encoded = Base64.UrlSafe.encode(target.threadKey.encodeToByteArray()).trimEnd('=')
    return "${target.accountId}#$folder#t.$encoded"
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
                    client.readThread(ThreadReadParams(threadId = thread.id, beforeCursor = cursor))
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

internal fun MeronMobileState.readStarredItem(item: StarredItemSummary) {
    readCoreThread(
        ThreadSummary(
            id = item.threadId,
            accountId = item.accountId,
            folder = item.folder,
            subject = item.subject,
            sender = item.sender,
            preview = item.preview,
            unread = item.unread,
            starred = true,
            dateEpochSeconds = item.dateEpochSeconds,
        ),
    )
}

internal fun MeronMobileState.runStarredItemAction(
    item: StarredItemSummary,
    label: String,
    action: suspend MobileMailCommandClient.() -> String,
    update: (List<StarredItemSummary>) -> List<StarredItemSummary>,
) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { MobileMailCommandClient(core).action() }
        }.onSuccess {
            starredItems = update(starredItems)
            status = "$label complete"
        }.onFailure {
            status = "$label failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.toggleStarredItemRead(item: StarredItemSummary) {
    val isRssItem = threadIdIsRss(item.threadId)
    runStarredItemAction(
        item = item,
        label = if (item.unread) "Mark read" else "Mark unread",
        action = {
            if (isRssItem) {
                markRssRead(RssMarkReadParams(threadId = item.threadId, seen = item.unread, itemKeys = listOf(item.id)))
            } else {
                markRead(MarkReadParams(threadId = item.threadId, seen = item.unread, messageIds = listOf(item.id)))
            }
        },
        update = { rows -> rows.map { if (it.id == item.id) it.copy(unread = !item.unread) else it } },
    )
}

internal fun MeronMobileState.unstarStarredItem(item: StarredItemSummary) {
    val isRssItem = threadIdIsRss(item.threadId)
    runStarredItemAction(
        item = item,
        label = "Unstar",
        action = {
            if (isRssItem) {
                markRssStarred(RssMarkStarredParams(threadId = item.threadId, starred = false, itemKeys = listOf(item.id)))
            } else {
                markStarred(MarkStarredParams(threadId = item.threadId, starred = false, messageIds = listOf(item.id)))
            }
        },
        update = { rows -> rows.filterNot { it.id == item.id } },
    )
}

internal fun MeronMobileState.deleteStarredMailItem(item: StarredItemSummary) {
    if (threadIdIsRss(item.threadId)) {
        status = "RSS items cannot be deleted."
        return
    }
    runStarredItemAction(
        item = item,
        label = threadDeleteActionLabel(item.folder),
        action = { delete(ThreadActionParams(threadId = item.threadId, folderId = item.folder, messageIds = listOf(item.id))) },
        update = { rows -> rows.filterNot { it.id == item.id } },
    )
}

internal fun MeronMobileState.runCoreThreadAction(
    thread: ThreadSummary,
    label: String,
    action: suspend MobileMailCommandClient.() -> String,
    update: (List<ThreadSummary>) -> List<ThreadSummary>,
    undoMessage: String? = null,
    onUndo: (() -> Unit)? = null,
) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    // Apply optimistically so the UI reacts instantly, then revert if the core
    // call fails. Snapshots taken here back the failure rollback.
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    coreThreads = update(coreThreads)
    kanbanColumns = kanbanColumns.mapValues { (_, state) -> state.copy(threads = update(state.threads)) }
    // The action commits immediately; Undo issues a compensating action (onUndo).
    // Track the commit so an Undo tap waits for it to finish, and is skipped if
    // the commit itself failed (the UI has already rolled back in that case).
    val committed = CompletableDeferred<Boolean>()
    scope.launch {
        runCatching {
            requireCoreOk(withContext(ioDispatcher) { MobileMailCommandClient(core).action() })
        }.onSuccess {
            if (undoMessage == null || onUndo == null) status = "$label complete"
            committed.complete(true)
        }.onFailure {
            Log.w("Mail", "$label failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            status = "$label failed: ${it.message}"
            snackbarHost.currentSnackbarData?.dismiss()
            committed.complete(false)
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
            if (result == SnackbarResult.ActionPerformed && committed.await()) onUndo()
        }
    }
}

// Moves a thread back to the folder it was in before an archive/delete and
// restores the pre-action list snapshots, backing the "Undo" snackbar action.
internal fun MeronMobileState.restoreThread(
    thread: ThreadSummary,
    threadsSnapshot: List<ThreadSummary>,
    kanbanSnapshot: Map<String, KanbanColumnState>,
) {
    if (!coreLoaded) return
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).move(
                    MoveThreadParams(threadId = thread.id, targetFolderId = thread.folder),
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

internal fun MeronMobileState.toggleStar(thread: ThreadSummary) {
    val backendThreadId = thread.backendThreadId()
    val isRssThread = threadIdIsRss(backendThreadId)
    val messageIds = listOf(thread.id).takeIf { backendThreadId != thread.id }
    runCoreThreadAction(
        thread = thread,
        label = if (thread.starred) "Unstar" else "Star",
        action = {
            if (isRssThread) {
                markRssStarred(RssMarkStarredParams(threadId = backendThreadId, starred = !thread.starred))
            } else {
                markStarred(MarkStarredParams(threadId = backendThreadId, starred = !thread.starred, messageIds = messageIds.orEmpty()))
            }
        },
        update = { threads -> threads.map { if (it.id == thread.id) it.copy(starred = !thread.starred) else it } },
    )
}

internal fun MeronMobileState.toggleRead(thread: ThreadSummary) {
    val isRssThread = threadIdIsRss(thread.id)
    runCoreThreadAction(
        thread = thread,
        label = if (thread.unread) "Mark read" else "Mark unread",
        action = {
            if (isRssThread) {
                markRssRead(RssMarkReadParams(threadId = thread.id, seen = thread.unread))
            } else {
                markRead(MarkReadParams(threadId = thread.id, seen = thread.unread))
            }
        },
        update = { threads -> threads.map { if (it.id == thread.id) it.copy(unread = !thread.unread) else it } },
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
                    MobileMailCommandClient(core).markRead(
                        MarkReadParams(threadId = thread.id, seen = seen, messageIds = listOf(message.id)),
                    )
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

internal fun MeronMobileState.toggleMessageStarred(message: MessageBody) {
    val thread = selectedCoreThread ?: return
    val starred = !message.starred
    status = if (starred) "Starring..." else "Unstarring..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).markStarred(
                    MarkStarredParams(threadId = thread.id, starred = starred, messageIds = listOf(message.id)),
                )
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
                    MobileMailCommandClient(core).delete(
                        ThreadActionParams(
                            threadId = thread.id,
                            folderId = messageFolder,
                            messageIds = listOf(message.id),
                        ),
                    )
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
    if (unread.isEmpty()) {
        status = "No unread messages."
        return
    }
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    val accountsById = coreAccounts.associateBy { it.id }
    val mailTargets =
        if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
            unread
                .map { it.accountId }
                .distinct()
                .filter { accountId -> accountsById[accountId]?.let { !accountSummaryIsRss(it) } ?: true }
                .map { accountId -> accountId to INBOX_FOLDER }
        } else {
            val account = accountsById[selectedCoreAccountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(selectedCoreAccountId to selectedCoreFolder) else emptyList()
        }
    val rssTargets = unread.filter { threadIdIsRss(it.id) }
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
                mailTargets.forEach { (accountId, folderId) ->
                    requireCoreOk(client.markAllRead(MarkAllReadParams(accountId = accountId, folderId = folderId)))
                }
                rssTargets.forEach { thread ->
                    requireCoreOk(client.markRssRead(RssMarkReadParams(threadId = thread.id, seen = true)))
                }
            }
        }.onSuccess {
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
    if (unread.isEmpty()) {
        status = "No unread cards."
        return
    }
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    val accountsById = coreAccounts.associateBy { it.id }
    val mailTargets =
        if (isUnifiedStarredColumn(column)) {
            unread
                .filterNot { threadIdIsRss(it.id) }
                .map { thread -> thread.backendThreadId() to listOf(thread.id) }
        } else if (column.accountId == UNIFIED_ACCOUNT_ID) {
            unread
                .map { it.accountId }
                .distinct()
                .filter { accountId -> accountsById[accountId]?.let { !accountSummaryIsRss(it) } ?: true }
                .map { accountId -> accountId to emptyList() }
        } else {
            val account = accountsById[column.accountId]
            if (account != null && !accountSummaryIsRss(account)) listOf(column.accountId to emptyList()) else emptyList()
        }
    val rssTargets = unread.filter { threadIdIsRss(it.id) }
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    updateKanbanColumn(key) { state ->
        state.copy(threads = state.threads.map { if (it.unread) it.copy(unread = false) else it })
    }
    coreThreads =
        coreThreads.map { thread ->
            if (unread.any { it.id == thread.id }) thread.copy(unread = false) else thread
        }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                mailTargets.forEach { (target, messageIds) ->
                    if (isUnifiedStarredColumn(column)) {
                        requireCoreOk(client.markRead(MarkReadParams(threadId = target, messageIds = messageIds)))
                    } else {
                        val folderId = if (column.accountId == UNIFIED_ACCOUNT_ID) INBOX_FOLDER else column.folderId
                        requireCoreOk(client.markAllRead(MarkAllReadParams(accountId = target, folderId = folderId)))
                    }
                }
                rssTargets.forEach { thread ->
                    requireCoreOk(client.markRssRead(RssMarkReadParams(threadId = thread.id, seen = true)))
                }
            }
        }.onSuccess {
            status = "Marked ${unread.size} Kanban card(s) read"
        }.onFailure {
            Log.w("Mail", "kanban mark all read failed", it)
            coreThreads = threadsBefore
            kanbanColumns = kanbanBefore
            status = "Kanban mark all read failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.archiveOrRemove(thread: ThreadSummary) {
    if (threadIdIsRss(thread.id)) {
        runCoreThreadAction(
            thread = thread,
            label = "Remove feed",
            action = { removeRssFeed(RemoveRssFeedParams(threadId = thread.id)) },
            update = { threads -> threads.filterNot { it.id == thread.id } },
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
            onUndo = { restoreThread(thread, threadsSnapshot, kanbanSnapshot) },
        )
    }
}

internal fun MeronMobileState.deleteThread(thread: ThreadSummary) {
    val threadsSnapshot = coreThreads
    val kanbanSnapshot = kanbanColumns
    runCoreThreadAction(
        thread = thread,
        label = threadDeleteActionLabel(thread.folder),
        action = { delete(ThreadActionParams(threadId = thread.id, folderId = thread.folder)) },
        update = { threads -> threads.filterNot { it.id == thread.id } },
        undoMessage = "Deleted",
        onUndo = { restoreThread(thread, threadsSnapshot, kanbanSnapshot) },
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
        status = "Rust core not packaged."
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
                        ensureManagedGoogleToken(client, account.id)
                        client.sync(
                            SyncMailParams(
                                accountId = account.id,
                                folderId = INBOX_FOLDER,
                                limit = 1,
                                folders = true,
                            ),
                        )
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
        status = "Rust core not packaged."
        return
    }
    val threadsBefore = coreThreads
    val kanbanBefore = kanbanColumns
    val selectedBefore = selectedCoreThread
    val messagesBefore = messages
    removeThreadEverywhere(thread.id)
    if (selectedCoreThread?.id == thread.id) {
        selectedCoreThread = null
        messages = emptyList()
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
        return
    }
    status = "Creating folder..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.createFolder(FolderCreateParams(accountId = thread.accountId, name = trimmed))
                val folders = loadAccountFolders(client, account)
                val created = folders.firstOrNull { it.name.equals(trimmed, ignoreCase = true) }?.name ?: trimmed
                if (created.equals(thread.folder, ignoreCase = true)) {
                    throw IllegalStateException("Already in $created.")
                }
                client.move(MoveThreadParams(threadId = thread.id, targetFolderId = created))
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
