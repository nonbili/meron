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
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
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
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

internal fun MeronMobileState.persistKanbanBoards(next: List<KanbanBoardSpec>) {
    kanbanBoards = next
    saveKanbanBoards(kanbanPrefs, next)
    if (activeKanbanBoardId.isBlank() || next.none { it.id == activeKanbanBoardId }) {
        activeKanbanBoardId = next.firstOrNull()?.id.orEmpty()
        saveActiveKanbanBoardId(kanbanPrefs, activeKanbanBoardId)
    }
}

internal fun MeronMobileState.persistKanbanFilter(next: FilterMode) {
    kanbanFilter = next
    saveKanbanFilter(kanbanPrefs, next)
}

internal fun MeronMobileState.persistKanbanSearch(next: String) {
    kanbanSearch = next
    saveKanbanSearch(kanbanPrefs, next)
}

internal fun MeronMobileState.persistKanbanSearchScope(next: String) {
    kanbanSearchScope = next.ifBlank { "all" }
    saveKanbanSearchScope(kanbanPrefs, kanbanSearchScope)
}

internal fun MeronMobileState.kanbanColumnSearchQuery(column: KanbanColumnSpec): String {
    val query = kanbanSearch.trim()
    if (query.isBlank()) return ""
    val scope = kanbanSearchScope.ifBlank { "all" }
    return if (scope == "all" || scope == kanbanColumnKey(column)) query else ""
}

internal fun MeronMobileState.updateKanbanColumn(
    key: String,
    update: (KanbanColumnState) -> KanbanColumnState,
) {
    kanbanColumns = kanbanColumns + (key to update(kanbanColumns[key] ?: KanbanColumnState()))
}

internal fun MeronMobileState.updateThreadEverywhere(
    thread: ThreadSummary,
    update: (ThreadSummary) -> ThreadSummary,
) {
    val next = update(thread)
    coreThreads = coreThreads.map { if (it.id == thread.id) next else it }
    selectedCoreThread = selectedCoreThread?.let { if (it.id == thread.id) next else it }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.map { if (it.id == thread.id) next else it })
        }
}

internal fun MeronMobileState.removeThreadEverywhere(threadId: String) {
    coreThreads = coreThreads.filterNot { it.id == threadId }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = state.threads.filterNot { it.id == threadId })
        }
    if (selectedCoreThread?.id == threadId) {
        selectedCoreThread = null
        messages = emptyList()
    }
}

internal suspend fun MeronMobileState.fetchKanbanColumn(
    client: MobileMailCommandClient,
    column: KanbanColumnSpec,
    refresh: Boolean,
    beforeCursor: String? = null,
    accountCursors: Map<String, String> = emptyMap(),
): MailboxLoadResult {
    val columnQuery = kanbanColumnSearchQuery(column)
    return if (column.accountId == UNIFIED_ACCOUNT_ID) {
        val unifiedAccounts = coreAccounts.filter { it.includedInUnified }
        val accounts =
            if (accountCursors.isEmpty()) {
                unifiedAccounts
            } else {
                unifiedAccounts.filter { accountCursors[it.id].orEmpty().isNotBlank() }
            }
        val results =
            accounts.map { account ->
                account.id to
                    loadAccountInbox(
                        client,
                        account,
                        INBOX_FOLDER,
                        query = columnQuery,
                        filter = kanbanFilter,
                        syncFirst = refresh,
                        beforeCursor = accountCursors[account.id],
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
        val account =
            coreAccounts.firstOrNull { it.id == column.accountId }
                ?: return MailboxLoadResult(emptyList(), column.folderId, emptyList())
        loadAccountInbox(
            client,
            account,
            column.folderId,
            query = columnQuery,
            filter = kanbanFilter,
            syncFirst = refresh,
            beforeCursor = beforeCursor,
        )
    }
}

internal fun MeronMobileState.loadKanbanColumn(
    column: KanbanColumnSpec,
    refresh: Boolean = false,
) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    val key = kanbanColumnKey(column)
    updateKanbanColumn(key) { it.copy(loading = true, error = null) }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                fetchKanbanColumn(client, column, refresh)
            }
        }.onSuccess { result ->
            val columnQuery = kanbanColumnSearchQuery(column)
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            updateKanbanColumn(key) {
                it.copy(
                    threads = result.threads,
                    loading = false,
                    loadingMore = false,
                    error = null,
                    nextCursor = if (columnQuery.isBlank()) result.nextCursor else "",
                    accountCursors = if (columnQuery.isBlank()) result.accountCursors else emptyMap(),
                )
            }
        }.onFailure {
            updateKanbanColumn(key) { state -> state.copy(loading = false, error = it.message ?: "Load failed") }
            status = "Kanban load failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.loadMoreKanbanColumn(column: KanbanColumnSpec) {
    if (!coreLoaded || kanbanColumnSearchQuery(column).isNotBlank()) return
    val key = kanbanColumnKey(column)
    val state = kanbanColumns[key] ?: return
    val hasCursor = if (column.accountId == UNIFIED_ACCOUNT_ID) state.accountCursors.isNotEmpty() else state.nextCursor.isNotBlank()
    if (state.loadingMore || !hasCursor) return
    updateKanbanColumn(key) { it.copy(loadingMore = true, error = null) }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                fetchKanbanColumn(
                    client = client,
                    column = column,
                    refresh = false,
                    beforeCursor = state.nextCursor,
                    accountCursors = state.accountCursors,
                )
            }
        }.onSuccess { result ->
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + result.folders.groupBy { it.accountId }
            }
            updateKanbanColumn(key) { current ->
                val existingIds = current.threads.map { it.id }.toSet()
                val appended = result.threads.filterNot { it.id in existingIds }
                current.copy(
                    threads = (current.threads + appended).sortedByDescending { it.dateEpochSeconds },
                    loadingMore = false,
                    error = null,
                    nextCursor = result.nextCursor,
                    accountCursors = result.accountCursors,
                )
            }
        }.onFailure {
            updateKanbanColumn(key) { state -> state.copy(loadingMore = false, error = it.message ?: "Load failed") }
            status = "Kanban load more failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.refreshKanbanColumnsForMailEvent(
    accountId: String,
    folderId: String,
) {
    val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
    val folder = folderId.ifBlank { INBOX_FOLDER }
    val accountIncludedInUnified =
        coreAccounts.firstOrNull { it.id == accountId }?.includedInUnified == true
    board.columns
        .filter { column ->
            val directFolderMatch =
                column.accountId == accountId &&
                    column.folderId.equals(folder, ignoreCase = true)
            val unifiedInboxMatch =
                column.accountId == UNIFIED_ACCOUNT_ID &&
                    folder.equals(INBOX_FOLDER, ignoreCase = true) &&
                    accountIncludedInUnified
            directFolderMatch || unifiedInboxMatch
        }
        .distinctBy(::kanbanColumnKey)
        .forEach { column ->
            // The IDLE/event path has already synced the core DB. Re-read the
            // affected active Kanban columns from cache without another IMAP pass.
            loadKanbanColumn(column, refresh = false)
        }
}

internal fun MeronMobileState.loadKanbanBoard(refresh: Boolean = false) {
    val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
    if (kanbanSearchScope != "all" && board.columns.none { kanbanColumnKey(it) == kanbanSearchScope }) {
        persistKanbanSearchScope("all")
    }
    board.columns.forEach { column -> loadKanbanColumn(column, refresh) }
}
