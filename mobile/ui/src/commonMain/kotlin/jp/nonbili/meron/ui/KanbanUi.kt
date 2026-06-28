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
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Check
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
import androidx.compose.material.icons.filled.Remove
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
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
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
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

@Composable
internal fun MailSearchFilterBar(
    search: String,
    filter: FilterMode,
    onSearchChange: (String) -> Unit,
    onSearchSubmit: () -> Unit,
    onFilterChange: (FilterMode) -> Unit,
) {
    Column(
        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface).padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OutlinedTextField(
            value = search,
            onValueChange = onSearchChange,
            modifier = Modifier.fillMaxWidth().height(44.dp),
            textStyle = MaterialTheme.typography.bodyMedium,
            placeholder = { Text(tr("mobile.mail.searchCachedMail"), style = MaterialTheme.typography.bodyMedium) },
            leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null, modifier = Modifier.size(19.dp)) },
            trailingIcon = {
                if (search.isNotBlank()) {
                    IconButton(
                        onClick = {
                            onSearchChange("")
                            onSearchSubmit()
                        },
                        modifier = Modifier.size(34.dp),
                    ) {
                        Icon(Icons.Filled.Close, contentDescription = tr("common.clearSearch"), modifier = Modifier.size(18.dp))
                    }
                }
            },
            singleLine = true,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterMode.values().forEach { mode ->
                FilterChip(
                    selected = filter == mode,
                    onClick = { onFilterChange(mode) },
                    label = { Text(mode.label()) },
                )
            }
            Spacer(Modifier.weight(1f))
            TextButton(onClick = onSearchSubmit) {
                Text(tr("buttons.done"))
            }
        }
    }
}

@Composable
internal fun KanbanHeaderSearchField(
    search: String,
    searchScope: String,
    columns: List<KanbanColumnSpec>,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    onSearchChange: (String) -> Unit,
    onSearchScopeChange: (String) -> Unit,
    onSearchSubmit: () -> Unit,
) {
    var scopeMenuOpen by remember { mutableStateOf(false) }
    val normalizedScope = searchScope.ifBlank { "all" }
    val scopedColumn = columns.firstOrNull { kanbanColumnKey(it) == normalizedScope }
    val scopeLabel = scopedColumn?.let { columnTitle(it, accounts, foldersByAccount) } ?: "All"
    Box(Modifier.fillMaxWidth()) {
        Surface(
            modifier = Modifier.fillMaxWidth().heightIn(min = 52.dp),
            shape = RoundedCornerShape(8.dp),
            color = MaterialTheme.colorScheme.surface,
            border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        ) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(start = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    Icons.Filled.Search,
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                BasicTextField(
                    value = search,
                    onValueChange = onSearchChange,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = MaterialTheme.colorScheme.onSurface),
                    modifier =
                        Modifier
                            .weight(1f)
                            .padding(start = 10.dp)
                            .onPreviewKeyEvent { event ->
                                if (event.type == KeyEventType.KeyUp && event.key == Key.Enter) {
                                    onSearchSubmit()
                                    true
                                } else {
                                    false
                                }
                            },
                    decorationBox = { innerTextField ->
                        Box(contentAlignment = Alignment.CenterStart) {
                            if (search.isBlank()) {
                                Text(
                                    "Search board",
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            innerTextField()
                        }
                    },
                )
                if (search.isNotBlank()) {
                    IconButton(
                        onClick = {
                            onSearchChange("")
                            onSearchSubmit()
                        },
                        modifier = Modifier.size(36.dp),
                    ) {
                        Icon(Icons.Filled.Close, contentDescription = tr("common.clearSearch"), modifier = Modifier.size(18.dp))
                    }
                }
                Box(
                    Modifier
                        .padding(horizontal = 2.dp)
                        .width(1.dp)
                        .height(30.dp)
                        .background(MaterialTheme.colorScheme.outlineVariant),
                )
                Box(
                    Modifier
                        .height(44.dp)
                        .widthIn(min = 66.dp, max = 112.dp)
                        .clickable { scopeMenuOpen = true },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        scopeLabel,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        fontSize = 12.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 10.dp),
                    )
                }
            }
        }
        Box(
            Modifier
                .align(Alignment.TopEnd)
                .height(52.dp)
                .widthIn(min = 66.dp, max = 112.dp),
        ) {
            DropdownMenu(expanded = scopeMenuOpen, onDismissRequest = { scopeMenuOpen = false }) {
                DropdownMenuItem(
                    text = { Text(tr("kanban.searchScope.allColumns")) },
                    leadingIcon = { Icon(Icons.Filled.ViewKanban, contentDescription = null) },
                    onClick = {
                        scopeMenuOpen = false
                        onSearchScopeChange("all")
                        onSearchSubmit()
                    },
                )
                columns.forEach { column ->
                    DropdownMenuItem(
                        text = { Text(columnTitle(column, accounts, foldersByAccount), maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        leadingIcon = { Avatar(columnAvatarLabel(column, accounts), 22.dp) },
                        onClick = {
                            scopeMenuOpen = false
                            onSearchScopeChange(kanbanColumnKey(column))
                            onSearchSubmit()
                        },
                    )
                }
            }
        }
    }
}

@Composable
internal fun KanbanScreen(
    modifier: Modifier,
    accounts: List<AccountSummary>,
    board: KanbanBoardSpec?,
    columns: Map<String, KanbanColumnState>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    filter: FilterMode,
    search: String,
    searchScope: String,
    onOpen: (ThreadSummary) -> Unit,
    selectedThreadIds: Set<String>,
    selectionActive: Boolean,
    onToggleSelected: (ThreadSummary) -> Unit,
    onLongPress: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onRefreshColumn: (KanbanColumnSpec) -> Unit,
    onLoadMoreColumn: (KanbanColumnSpec) -> Unit,
    onMarkColumnAllRead: (KanbanColumnSpec) -> Unit,
    onRemoveColumn: (KanbanColumnSpec) -> Unit,
    onMoveColumn: (KanbanColumnSpec, Int) -> Unit,
    onSearchColumn: (KanbanColumnSpec) -> Unit,
    onAddColumn: () -> Unit,
    showSenderImages: Boolean,
    kanbanColumnWidth: Dp,
) {
    if (accounts.isEmpty()) {
        EmptyState(
            icon = Icons.Filled.PersonAdd,
            title = tr("empty.welcomeTitle"),
            text = tr("empty.kanbanSetupText"),
            actionLabel = tr("accounts.actions.addAccount"),
            onAction = {},
        )
        return
    }
    val boardColumns = board?.columns.orEmpty()
    val boardBackground = boardBackgroundBrush(board)
    var minimizedColumns by remember(board?.id) { mutableStateOf(emptySet<String>()) }
    Column(
        modifier.then(
            if (boardBackground != null) {
                Modifier.background(boardBackground)
            } else {
                Modifier.background(MaterialTheme.colorScheme.background)
            },
        ),
    ) {
        if (boardColumns.isEmpty()) {
            EmptyState(
                icon = Icons.Filled.ViewKanban,
                title = tr("empty.noColumns"),
                text = tr("empty.noColumnsText"),
                actionLabel = tr("kanban.actions.addColumn"),
                onAction = onAddColumn,
            )
        } else {
            LazyRow(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 12.dp, top = 8.dp, end = 12.dp, bottom = 16.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(boardColumns, key = { kanbanColumnKey(it) }) { column ->
                    val key = kanbanColumnKey(column)
                    if (key in minimizedColumns) {
                        KanbanMinimizedColumn(
                            column = column,
                            accounts = accounts,
                            foldersByAccount = foldersByAccount,
                            unread = (columns[key] ?: KanbanColumnState()).threads.count { it.unread },
                            onRestore = { minimizedColumns = minimizedColumns - key },
                        )
                    } else {
                        KanbanColumn(
                            column = column,
                            state = columns[key] ?: KanbanColumnState(),
                            accounts = accounts,
                            foldersByAccount = foldersByAccount,
                            filter = filter,
                            search = search,
                            searchScope = searchScope,
                            onOpen = onOpen,
                            selectedThreadIds = selectedThreadIds,
                            selectionActive = selectionActive,
                            onToggleSelected = onToggleSelected,
                            onLongPress = onLongPress,
                            onToggleStar = onToggleStar,
                            onRefresh = { onRefreshColumn(column) },
                            onLoadMore = { onLoadMoreColumn(column) },
                            onMarkAllRead = { onMarkColumnAllRead(column) },
                            onRemove = { onRemoveColumn(column) },
                            onMoveLeft = { onMoveColumn(column, -1) },
                            onMoveRight = { onMoveColumn(column, 1) },
                            onMinimize = { minimizedColumns = minimizedColumns + key },
                            onSearch = { onSearchColumn(column) },
                            showSenderImages = showSenderImages,
                            kanbanColumnWidth = kanbanColumnWidth,
                        )
                    }
                }
            }
        }
    }
}

@Composable
internal fun FilterModeButton(
    value: FilterMode,
    onChange: (FilterMode) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { expanded = true }) {
            Icon(Icons.Filled.FilterList, contentDescription = tr("filters.label"))
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            FilterMode.values().forEach { mode ->
                DropdownMenuItem(
                    text = { Text(mode.label()) },
                    onClick = {
                        expanded = false
                        onChange(mode)
                    },
                )
            }
        }
    }
}

@Composable
internal fun KanbanColumn(
    column: KanbanColumnSpec,
    state: KanbanColumnState,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    filter: FilterMode,
    search: String,
    searchScope: String,
    onOpen: (ThreadSummary) -> Unit,
    selectedThreadIds: Set<String>,
    selectionActive: Boolean,
    onToggleSelected: (ThreadSummary) -> Unit,
    onLongPress: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onRefresh: () -> Unit,
    onLoadMore: () -> Unit,
    onMarkAllRead: () -> Unit,
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
    onMinimize: () -> Unit,
    onSearch: () -> Unit,
    showSenderImages: Boolean,
    kanbanColumnWidth: Dp,
) {
    val columnKey = kanbanColumnKey(column)
    val columnSearch = if (searchScope.ifBlank { "all" } == "all" || searchScope == columnKey) search else ""
    val visibleThreads = state.threads.filteredKanbanThreads(filter, columnSearch)
    val canLoadMore = columnSearch.isBlank() && (state.nextCursor.isNotBlank() || state.accountCursors.isNotEmpty())
    val accountsById = remember(accounts) { accounts.associateBy { it.id } }
    val showAccountBadge = column.accountId == UNIFIED_ACCOUNT_ID
    Card(
        modifier =
            Modifier
                .width(kanbanColumnWidth)
                .fillMaxSize()
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp)),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(Modifier.fillMaxSize()) {
            KanbanColumnHeader(
                column = column,
                accounts = accounts,
                foldersByAccount = foldersByAccount,
                unread = visibleThreads.count { it.unread },
                onRefresh = onRefresh,
                onMarkAllRead = onMarkAllRead,
                onRemove = onRemove,
                onMoveLeft = onMoveLeft,
                onMoveRight = onMoveRight,
                onMinimize = onMinimize,
                onSearch = onSearch,
            )
            if (state.loading) {
                Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(Modifier.size(24.dp))
                }
            }
            if (state.error != null) {
                Text(
                    state.error,
                    color = MaterialTheme.colorScheme.error,
                    fontSize = 12.sp,
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (!state.loading && visibleThreads.isEmpty()) {
                Box(Modifier.fillMaxSize().padding(18.dp), contentAlignment = Alignment.Center) {
                    Text(
                        if (search.isBlank()) {
                            tr(
                                when (filter) {
                                    FilterMode.All -> "kanban.emptyAllColumn"
                                    FilterMode.Unread -> "kanban.emptyUnreadColumn"
                                    FilterMode.Starred -> "kanban.emptyStarredColumn"
                                },
                            )
                        } else {
                            tr("mobile.mail.noSearchMatches")
                        },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        fontSize = 13.sp,
                    )
                }
            } else {
                val columnListState = rememberLazyListState()
                val nearBottom by remember {
                    derivedStateOf {
                        val lastVisible =
                            columnListState.layoutInfo.visibleItemsInfo
                                .lastOrNull()
                                ?.index ?: 0
                        lastVisible >= visibleThreads.size - 3
                    }
                }
                LaunchedEffect(nearBottom, canLoadMore, state.loadingMore) {
                    if (nearBottom && canLoadMore && !state.loadingMore) onLoadMore()
                }
                LazyColumn(
                    Modifier.fillMaxSize(),
                    state = columnListState,
                    contentPadding = PaddingValues(bottom = 8.dp),
                ) {
                    items(visibleThreads, key = { it.id }) { thread ->
                        MailRow(
                            thread = thread,
                            account = accountsById[thread.accountId].takeIf { showAccountBadge },
                            showSenderImages = showSenderImages,
                            selected = thread.id in selectedThreadIds,
                            selectionActive = selectionActive,
                            onOpen = {
                                if (selectionActive) {
                                    onToggleSelected(thread)
                                } else {
                                    onOpen(thread)
                                }
                            },
                            onLongPress = { onLongPress(thread) },
                            onToggleStar = { onToggleStar(thread) },
                            onCopyFeedUrl = null,
                        )
                        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.7f))
                    }
                    if (canLoadMore || state.loadingMore) {
                        item {
                            Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.Center) {
                                if (state.loadingMore) {
                                    CircularProgressIndicator(Modifier.size(22.dp))
                                } else {
                                    OutlinedButton(onClick = onLoadMore) {
                                        Text(tr("threads.actions.loadMore"))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun KanbanColumnHeader(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    unread: Int,
    onRefresh: () -> Unit,
    onMarkAllRead: () -> Unit,
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
    onMinimize: () -> Unit,
    onSearch: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Row(
        Modifier
            .fillMaxWidth()
            .height(48.dp)
            .padding(start = 12.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Avatar(columnAvatarLabel(column, accounts), 26.dp)
        Row(
            Modifier.weight(1f),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                columnTitle(column, accounts, foldersByAccount),
                fontWeight = FontWeight.SemiBold,
                fontSize = 12.sp,
                modifier = Modifier.weight(1f, fill = false),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (unread > 0) {
                KanbanUnreadBadge(unread)
            }
        }
        IconButton(onClick = onMinimize, modifier = Modifier.size(34.dp)) {
            Icon(Icons.Filled.Remove, contentDescription = tr("kanban.actions.minimizeColumn"), modifier = Modifier.size(18.dp))
        }
        Box {
            IconButton(onClick = { menuOpen = true }, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Filled.MoreVert, contentDescription = tr("kanban.actions.columnActions"), modifier = Modifier.size(18.dp))
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                DropdownMenuItem(text = { Text(tr("kanban.actions.searchColumn")) }, leadingIcon = {
                    Icon(Icons.Filled.Search, contentDescription = null)
                }, onClick = {
                    menuOpen = false
                    onSearch()
                })
                DropdownMenuItem(text = { Text(tr("mobile.actions.refresh")) }, onClick = {
                    menuOpen = false
                    onRefresh()
                })
                DropdownMenuItem(text = { Text(tr("threads.actions.markAllAsRead")) }, onClick = {
                    menuOpen = false
                    onMarkAllRead()
                }, enabled = unread > 0)
                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                DropdownMenuItem(text = { Text(tr("kanban.actions.moveColumnLeft")) }, onClick = {
                    menuOpen = false
                    onMoveLeft()
                })
                DropdownMenuItem(text = { Text(tr("kanban.actions.moveColumnRight")) }, onClick = {
                    menuOpen = false
                    onMoveRight()
                })
                HorizontalDivider(Modifier.padding(vertical = 4.dp))
                DropdownMenuItem(text = { Text(tr("kanban.actions.minimizeColumn")) }, onClick = {
                    menuOpen = false
                    onMinimize()
                })
                DropdownMenuItem(text = { Text(tr("kanban.actions.hideColumn")) }, onClick = {
                    menuOpen = false
                    onRemove()
                })
            }
        }
    }
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
}

@Composable
internal fun KanbanMinimizedColumn(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    unread: Int,
    onRestore: () -> Unit,
) {
    Card(
        modifier =
            Modifier
                .width(58.dp)
                .fillMaxSize()
                .clickable(onClick = onRestore)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp)),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(
            Modifier.fillMaxSize().padding(horizontal = 6.dp, vertical = 8.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Avatar(columnAvatarLabel(column, accounts), 26.dp)
            CollapsedColumnTitle(columnTitle(column, accounts, foldersByAccount))
            if (unread > 0) {
                KanbanUnreadBadge(unread)
            }
        }
    }
}

@Composable
internal fun CollapsedColumnTitle(title: String) {
    Column(
        Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        title.split(' ')
            .filter { it.isNotBlank() }
            .take(3)
            .forEach { word ->
                Text(
                    word,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    fontSize = 10.5.sp,
                    lineHeight = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                )
            }
    }
}

@Composable
internal fun KanbanUnreadBadge(
    unread: Int,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier
            .heightIn(min = 18.dp)
            .widthIn(min = 18.dp)
            .clip(CircleShape)
            .background(MaterialTheme.colorScheme.primary)
            .padding(horizontal = 5.dp, vertical = 2.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            unread.toString(),
            color = MaterialTheme.colorScheme.onPrimary,
            fontSize = 10.sp,
            lineHeight = 10.sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
        )
    }
}

internal fun columnAvatarLabel(
    column: KanbanColumnSpec,
    accounts: List<AccountSummary>,
): String {
    if (column.accountId == UNIFIED_ACCOUNT_ID) return "Unified inbox"
    val account = accounts.firstOrNull { it.id == column.accountId }
    return account?.displayName?.ifBlank { account.email } ?: column.accountId
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
internal fun KanbanThreadCard(
    thread: ThreadSummary,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
    onLongPress: () -> Unit,
    onToggleStar: () -> Unit,
) {
    val chat = LocalChatColors.current
    Card(
        modifier =
            Modifier
                .fillMaxWidth()
                .combinedClickable(onClick = onOpen, onLongClick = onLongPress),
        shape = RoundedCornerShape(8.dp),
        colors =
            CardDefaults.cardColors(
                containerColor =
                    if (thread.unread) {
                        MaterialTheme.colorScheme.primary.copy(
                            alpha = 0.08f,
                        )
                    } else {
                        MaterialTheme.colorScheme.surface
                    },
            ),
    ) {
        Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                SenderAvatar(
                    label = thread.sender.ifBlank { thread.accountId },
                    enabled = showSenderImages,
                    size = 26.dp,
                )
                Text(
                    thread.sender.ifBlank { thread.accountId }.substringBefore('@'),
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 12.sp,
                    modifier = Modifier.weight(1f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    formatRelativeTime(thread.dateEpochSeconds),
                    fontSize = 10.5.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Text(
                thread.subject.ifBlank { "(no subject)" },
                fontSize = 12.sp,
                fontWeight = if (thread.unread) FontWeight.SemiBold else FontWeight.Normal,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            if (thread.preview.isNotBlank()) {
                Text(
                    thread.preview,
                    fontSize = 11.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (thread.unread) {
                    Box(Modifier.size(7.dp).clip(CircleShape).background(MaterialTheme.colorScheme.primary))
                }
                Spacer(Modifier.weight(1f))
                IconButton(onClick = onToggleStar, modifier = Modifier.size(28.dp)) {
                    Icon(
                        if (thread.starred) Icons.Filled.Star else Icons.Filled.StarBorder,
                        contentDescription = if (thread.starred) "Unstar" else "Star",
                        tint = if (thread.starred) chat.star else MaterialTheme.colorScheme.outline,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        }
    }
}

@Composable
internal fun KanbanColumnDialog(
    accounts: List<AccountSummary>,
    board: KanbanBoardSpec?,
    foldersByAccount: Map<String, List<FolderSummary>>,
    onApply: (List<KanbanColumnSpec>) -> Unit,
    onCreateFolder: (AccountSummary) -> Unit,
    onDismiss: () -> Unit,
) {
    // Local multi-select state: tapping a row toggles it; nothing is applied to
    // the board until "Done". Keyed by kanbanColumnKey so order is preserved
    // for re-application.
    val selected = remember(board) {
        mutableStateMapOf<String, KanbanColumnSpec>().apply {
            board?.columns.orEmpty().forEach { put(kanbanColumnKey(it), it) }
        }
    }
    fun toggle(column: KanbanColumnSpec) {
        val key = kanbanColumnKey(column)
        if (selected.remove(key) == null) selected[key] = column
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("kanban.actions.addColumn")) },
        text = {
            LazyColumn(Modifier.heightIn(max = 460.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                item {
                    Text(
                        tr("kanban.addColumnsHint"),
                        fontSize = 12.sp,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                }
                item {
                    val unified = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                    SidebarLikeDialogRow(
                        selected = selected.contains(kanbanColumnKey(unified)),
                        title = tr("kanban.columns.unifiedInbox"),
                        subtitle = tr("kanban.allAccounts"),
                        onClick = { toggle(unified) },
                        leadingIcon = Icons.Filled.Inbox,
                    )
                }
                accounts.forEach { account ->
                    item {
                        Text(
                            account.displayName.ifBlank { account.email.ifBlank { account.id } },
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                    val folders = foldersByAccount[account.id].orEmpty()
                    if (!accountSummaryIsRss(account)) {
                        item {
                            TextButton(onClick = { onCreateFolder(account) }) {
                                Icon(Icons.Filled.Add, contentDescription = null, modifier = Modifier.size(16.dp))
                                Spacer(Modifier.width(6.dp))
                                Text(tr("folders.create"))
                            }
                        }
                    }
                    // Always surface folders that are already board columns, even if
                    // they haven't been fetched into foldersByAccount yet — otherwise
                    // existing columns can't be shown as selected or removed.
                    val boardFolders =
                        board
                            ?.columns
                            .orEmpty()
                            .filter { it.accountId == account.id }
                            .map { it.folderId }
                    // Board folders first so existing columns keep their stored folderId
                    // (folder name casing may differ from the fetched list, and the
                    // selection key is case-sensitive).
                    val visibleFolders =
                        buildList {
                            val seen = mutableSetOf<String>()
                            (boardFolders + folders.map { it.name }).forEach { name ->
                                if (seen.add(name.lowercase())) add(FolderSummary(account.id, name, 0))
                            }
                            if (isEmpty()) add(FolderSummary(account.id, INBOX_FOLDER, 0))
                        }
                    items(visibleFolders, key = { "${account.id}\n${it.name}" }) { folder ->
                        val column = KanbanColumnSpec(account.id, folder.name)
                        val isRss = accountSummaryIsRss(account)
                        SidebarLikeDialogRow(
                            selected = selected.contains(kanbanColumnKey(column)),
                            title = folder.name.replaceFirstChar { it.uppercase() },
                            subtitle = null,
                            onClick = { toggle(column) },
                            leadingIcon =
                                when {
                                    isRss -> Icons.Filled.RssFeed
                                    folder.name.equals(INBOX_FOLDER, ignoreCase = true) -> Icons.Filled.Inbox
                                    else -> Icons.Outlined.FolderOpen
                                },
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onApply(selected.values.toList()) }) { Text(tr("buttons.done")) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) }
        },
    )
}

@Composable
internal fun KanbanThreadActionDialog(
    thread: ThreadSummary,
    board: KanbanBoardSpec?,
    accounts: List<AccountSummary>,
    onDismiss: () -> Unit,
    onOpen: () -> Unit,
    onToggleStar: () -> Unit,
    onToggleRead: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    onCopyFeedUrl: () -> Unit,
    onMove: (KanbanColumnSpec) -> Unit,
) {
    val deleteLabel = threadDeleteActionLabel(thread.folder)
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(thread.subject.ifBlank { tr("threads.noSubject") }, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        text = {
            LazyColumn(Modifier.heightIn(max = 430.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item { DialogAction(tr("kanban.actions.openThread"), onOpen) }
                item { DialogAction(if (thread.starred) tr("chat.unstar") else tr("chat.star"), onToggleStar) }
                item { DialogAction(if (thread.unread) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread"), onToggleRead) }
                if (threadIdIsRss(thread.id) && thread.feedUrl.isNotBlank()) {
                    item { DialogAction(tr("feeds.copyUrl"), onCopyFeedUrl) }
                }
                item { DialogAction(if (threadIdIsRss(thread.id)) tr("feeds.actions.deleteFeed") else tr("threads.actions.archiveThread"), onArchive) }
                if (!threadIdIsRss(thread.id)) {
                    item { DialogAction(deleteLabel, onDelete) }
                }
                val moveTargets =
                    board
                        ?.columns
                        .orEmpty()
                        .filter { it.accountId != UNIFIED_ACCOUNT_ID }
                        .filter { target ->
                            val targetAccount = accounts.firstOrNull { it.id == target.accountId }
                            targetAccount != null && (threadIdIsRss(thread.id) == accountSummaryIsRss(targetAccount))
                        }.filterNot { it.accountId == thread.accountId && it.folderId.equals(thread.folder, ignoreCase = true) }
                if (moveTargets.isNotEmpty()) {
                    item {
                        Text(
                            tr("threads.actions.moveTo"),
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 10.dp),
                        )
                    }
                    items(moveTargets, key = { kanbanColumnKey(it) }) { target ->
                        val account = accounts.firstOrNull { it.id == target.accountId }
                        val targetLabel = listOf(account?.displayName?.ifBlank { account.email } ?: target.accountId, target.folderId).joinToString(" / ")
                        DialogAction(targetLabel) {
                            onMove(target)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(tr("buttons.close")) }
        },
    )
}

@Composable
internal fun DialogAction(
    label: String,
    onClick: () -> Unit,
) {
    TextButton(onClick = onClick, modifier = Modifier.fillMaxWidth()) {
        Text(label, modifier = Modifier.weight(1f))
    }
}

@Composable
internal fun SidebarLikeDialogRow(
    selected: Boolean,
    title: String,
    subtitle: String?,
    onClick: () -> Unit,
    leadingIcon: ImageVector? = null,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.12f) else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (leadingIcon != null) {
            Icon(
                leadingIcon,
                contentDescription = null,
                modifier = Modifier.size(16.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.width(10.dp))
        }
        Column(Modifier.weight(1f)) {
            Text(
                title,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle != null) {
                Text(
                    subtitle,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontSize = 11.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        if (selected) {
            Spacer(Modifier.width(10.dp))
            Icon(
                Icons.Filled.Check,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
                tint = MaterialTheme.colorScheme.primary,
            )
        }
    }
}
