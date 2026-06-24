package jp.nonbili.meron

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Base64
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.ui.graphics.asImageBitmap
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
import androidx.compose.ui.platform.LocalContext
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
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.FileProvider
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
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
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.URL
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID
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
            modifier = Modifier.fillMaxWidth(),
            placeholder = { Text("Search mail") },
            leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
            trailingIcon = {
                if (search.isNotBlank()) {
                    IconButton(onClick = {
                        onSearchChange("")
                        onSearchSubmit()
                    }) {
                        Icon(Icons.Filled.Close, contentDescription = "Clear search")
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
                Text("Apply")
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
    onFilter: (FilterMode) -> Unit,
    onSearch: (String) -> Unit,
    onOpen: (ThreadSummary) -> Unit,
    onLongPress: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onArchive: (ThreadSummary) -> Unit,
    onRefreshColumn: (KanbanColumnSpec) -> Unit,
    onLoadMoreColumn: (KanbanColumnSpec) -> Unit,
    onMarkColumnAllRead: (KanbanColumnSpec) -> Unit,
    onRemoveColumn: (KanbanColumnSpec) -> Unit,
    onMoveColumn: (KanbanColumnSpec, Int) -> Unit,
    onAddColumn: () -> Unit,
    showSenderImages: Boolean,
    kanbanColumnWidth: Dp,
) {
    if (accounts.isEmpty()) {
        EmptyState(
            icon = Icons.Filled.PersonAdd,
            title = "Welcome to Meron",
            text = "Add an account to use Kanban.",
            actionLabel = "Add account",
            onAction = {},
        )
        return
    }
    val boardColumns = board?.columns.orEmpty()
    val boardBackground = boardBackgroundBrush(board)
    Column(
        modifier.then(
            if (boardBackground != null) {
                Modifier.background(boardBackground)
            } else {
                Modifier.background(MaterialTheme.colorScheme.background)
            },
        ),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (board?.hasBoardStyle() == true) {
                KanbanBoardTile(board, 38.dp)
            }
            OutlinedTextField(
                value = search,
                onValueChange = onSearch,
                placeholder = { Text("Search board") },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            FilterModeButton(filter, onFilter)
        }
        if (boardColumns.isEmpty()) {
            EmptyState(
                icon = Icons.Filled.ViewKanban,
                title = "No columns",
                text = "Add a column to start using this board.",
                actionLabel = "Add column",
                onAction = onAddColumn,
            )
        } else {
            LazyRow(
                Modifier.fillMaxSize(),
                contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 88.dp),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(boardColumns, key = { kanbanColumnKey(it) }) { column ->
                    KanbanColumn(
                        column = column,
                        state = columns[kanbanColumnKey(column)] ?: KanbanColumnState(),
                        accounts = accounts,
                        foldersByAccount = foldersByAccount,
                        filter = filter,
                        search = search,
                        onOpen = onOpen,
                        onLongPress = onLongPress,
                        onToggleStar = onToggleStar,
                        onArchive = onArchive,
                        onRefresh = { onRefreshColumn(column) },
                        onLoadMore = { onLoadMoreColumn(column) },
                        onMarkAllRead = { onMarkColumnAllRead(column) },
                        onRemove = { onRemoveColumn(column) },
                        onMoveLeft = { onMoveColumn(column, -1) },
                        onMoveRight = { onMoveColumn(column, 1) },
                        showSenderImages = showSenderImages,
                        kanbanColumnWidth = kanbanColumnWidth,
                    )
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
            Icon(Icons.Filled.FilterList, contentDescription = "Filter")
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
    onOpen: (ThreadSummary) -> Unit,
    onLongPress: (ThreadSummary) -> Unit,
    onToggleStar: (ThreadSummary) -> Unit,
    onArchive: (ThreadSummary) -> Unit,
    onRefresh: () -> Unit,
    onLoadMore: () -> Unit,
    onMarkAllRead: () -> Unit,
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
    showSenderImages: Boolean,
    kanbanColumnWidth: Dp,
) {
    val visibleThreads = state.threads.filteredKanbanThreads(filter, search)
    val canLoadMore = search.isBlank() && (state.nextCursor.isNotBlank() || state.accountCursors.isNotEmpty())
    Card(
        modifier = Modifier.width(kanbanColumnWidth).fillMaxSize(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(Modifier.fillMaxSize()) {
            KanbanColumnHeader(
                column = column,
                accounts = accounts,
                foldersByAccount = foldersByAccount,
                count = visibleThreads.size,
                unread = visibleThreads.count { it.unread },
                onRefresh = onRefresh,
                onMarkAllRead = onMarkAllRead,
                onRemove = onRemove,
                onMoveLeft = onMoveLeft,
                onMoveRight = onMoveRight,
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
                        if (search.isBlank()) "No ${filter.emptyNoun()} here" else "No matches",
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
                    contentPadding = PaddingValues(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(visibleThreads, key = { it.id }) { thread ->
                        KanbanThreadCard(
                            thread = thread,
                            showSenderImages = showSenderImages,
                            onOpen = { onOpen(thread) },
                            onLongPress = { onLongPress(thread) },
                            onToggleStar = { onToggleStar(thread) },
                            onArchive = { onArchive(thread) },
                        )
                    }
                    if (canLoadMore || state.loadingMore) {
                        item {
                            Box(Modifier.fillMaxWidth().padding(8.dp), contentAlignment = Alignment.Center) {
                                if (state.loadingMore) {
                                    CircularProgressIndicator(Modifier.size(22.dp))
                                } else {
                                    OutlinedButton(onClick = onLoadMore) {
                                        Text("Load older")
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
    count: Int,
    unread: Int,
    onRefresh: () -> Unit,
    onMarkAllRead: () -> Unit,
    onRemove: () -> Unit,
    onMoveLeft: () -> Unit,
    onMoveRight: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth().padding(start = 12.dp, end = 4.dp, top = 10.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Icon(
            folderIcon(column.folderId),
            contentDescription = null,
            modifier = Modifier.size(19.dp),
            tint = MaterialTheme.colorScheme.primary,
        )
        Column(Modifier.weight(1f)) {
            Text(
                columnTitle(column, accounts, foldersByAccount),
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                "$count cards${if (unread > 0) " · $unread unread" else ""}",
                fontSize = 11.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Box {
            IconButton(onClick = { menuOpen = true }) {
                Icon(Icons.Filled.MoreVert, contentDescription = "Column actions")
            }
            DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                DropdownMenuItem(text = { Text("Refresh") }, onClick = {
                    menuOpen = false
                    onRefresh()
                })
                DropdownMenuItem(text = { Text("Mark all read") }, onClick = {
                    menuOpen = false
                    onMarkAllRead()
                }, enabled = unread > 0)
                DropdownMenuItem(text = { Text("Move left") }, onClick = {
                    menuOpen = false
                    onMoveLeft()
                })
                DropdownMenuItem(text = { Text("Move right") }, onClick = {
                    menuOpen = false
                    onMoveRight()
                })
                DropdownMenuItem(text = { Text("Remove column") }, onClick = {
                    menuOpen = false
                    onRemove()
                })
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)
@Composable
internal fun KanbanThreadCard(
    thread: ThreadSummary,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
    onLongPress: () -> Unit,
    onToggleStar: () -> Unit,
    onArchive: () -> Unit,
) {
    val chat = LocalChatColors.current
    val dismissState =
        rememberSwipeToDismissBoxState(
            confirmValueChange = { value ->
                if (value == SwipeToDismissBoxValue.EndToStart) {
                    onArchive()
                    true
                } else {
                    false
                }
            },
        )
    SwipeToDismissBox(
        state = dismissState,
        enableDismissFromStartToEnd = false,
        backgroundContent = {
            Box(
                Modifier
                    .fillMaxSize()
                    .clip(
                        RoundedCornerShape(8.dp),
                    ).background(MaterialTheme.colorScheme.primaryContainer)
                    .padding(horizontal = 16.dp),
                contentAlignment = Alignment.CenterEnd,
            ) {
                Icon(if (threadIdIsRss(thread.id)) Icons.Filled.Delete else Icons.Filled.Archive, contentDescription = null)
            }
        },
    ) {
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
}

@Composable
internal fun AccountSettingsDialog(
    account: AccountSummary,
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    showInNavigation: Boolean,
    onSave: (
        displayName: String,
        senderName: String,
        avatarUrl: String,
        wallpaperPresetId: String,
        loadRemoteImages: Boolean,
        conversationHtml: Boolean,
        includedInUnified: Boolean,
        showInNavigation: Boolean,
        muted: Boolean,
        paused: Boolean,
        rssSyncIntervalMinutes: Int,
        aliasesText: String,
    ) -> Unit,
    onPickAvatar: () -> Unit,
    onPickWallpaper: () -> Unit,
    onMoveUp: () -> Unit,
    onMoveDown: () -> Unit,
    onRemove: () -> Unit,
    onDismiss: () -> Unit,
) {
    val isRss = accountSummaryIsRss(account)
    var displayName by remember(account.id) { mutableStateOf(account.displayName) }
    var senderName by remember(account.id) { mutableStateOf(account.senderName) }
    var avatarUrl by remember(account.id) { mutableStateOf(account.avatarUrl) }
    var wallpaperPresetId by remember(account.id) { mutableStateOf(account.chatWallpaperPresetId) }
    var loadRemoteImages by remember(account.id) { mutableStateOf(account.loadRemoteImages || isRss) }
    var conversationHtml by remember(account.id) { mutableStateOf(account.conversationHtml) }
    var includedInUnified by remember(account.id) { mutableStateOf(account.includedInUnified) }
    var visibleInNavigation by remember(account.id, showInNavigation) { mutableStateOf(showInNavigation) }
    var muted by remember(account.id) { mutableStateOf(account.muted) }
    var paused by remember(account.id) { mutableStateOf(account.paused) }
    var intervalText by remember(account.id) { mutableStateOf(account.rssSyncIntervalMinutes.toString()) }
    var aliasesText by remember(account.id) {
        mutableStateOf(
            account.aliases.joinToString("\n") { alias ->
                if (alias.name.isBlank()) alias.email else "${alias.email}, ${alias.name}"
            },
        )
    }
    val interval = intervalText.toIntOrNull()?.coerceIn(5, 1440) ?: account.rssSyncIntervalMinutes.coerceIn(5, 1440)
    var confirmRemove by remember(account.id) { mutableStateOf(false) }

    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text("Remove account?") },
            text = {
                Text(
                    "Remove ${account.email.ifBlank { account.id }}? Cached mail for this account will be deleted from this device.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmRemove = false
                    onRemove()
                }) {
                    Text("Remove", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmRemove = false }) { Text("Cancel") }
            },
        )
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Account settings") },
        text = {
            LazyColumn(
                modifier = Modifier.heightIn(max = 520.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            account.email.ifBlank { account.id },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedTextField(
                            value = displayName,
                            onValueChange = { displayName = it },
                            label = { Text(if (isRss) "Feed group name" else "Account name") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        if (!isRss) {
                            OutlinedTextField(
                                value = senderName,
                                onValueChange = { senderName = it },
                                label = { Text("Sender name") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                        }
                        OutlinedTextField(
                            value = avatarUrl,
                            onValueChange = { avatarUrl = it },
                            label = { Text("Avatar URL") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        TextButton(onClick = onPickAvatar) {
                            Text("Choose avatar image")
                        }
                        OutlinedTextField(
                            value = wallpaperPresetId,
                            onValueChange = { wallpaperPresetId = it },
                            label = { Text("Chat wallpaper preset") },
                            supportingText = { Text("Blank for default; try grid, dots, forest, ocean, sunset") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                        TextButton(onClick = onPickWallpaper) {
                            Text("Choose wallpaper image")
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            TextButton(onClick = onMoveUp, enabled = canMoveUp) {
                                Text("Move up")
                            }
                            TextButton(onClick = onMoveDown, enabled = canMoveDown) {
                                Text("Move down")
                            }
                        }
                    }
                }
                item {
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        AccountSwitchRow("Show in unified inbox", includedInUnified) { includedInUnified = it }
                        AccountSwitchRow("Show in navigation", visibleInNavigation) { visibleInNavigation = it }
                        AccountSwitchRow("Mute notifications", muted) { muted = it }
                        AccountSwitchRow("Pause automatic sync", paused) { paused = it }
                        AccountSwitchRow("Load remote images", loadRemoteImages) { loadRemoteImages = it }
                        AccountSwitchRow("Render HTML messages", conversationHtml) { conversationHtml = it }
                    }
                }
                if (isRss) {
                    item {
                        OutlinedTextField(
                            value = intervalText,
                            onValueChange = { intervalText = it.filter(Char::isDigit).take(4) },
                            label = { Text("RSS sync interval minutes") },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                        )
                    }
                } else {
                    item {
                        OutlinedTextField(
                            value = aliasesText,
                            onValueChange = { aliasesText = it },
                            label = { Text("Send-as aliases") },
                            supportingText = { Text("One per line: email, optional name") },
                            modifier = Modifier.fillMaxWidth(),
                            minLines = 2,
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    onSave(
                        displayName,
                        senderName,
                        avatarUrl,
                        wallpaperPresetId,
                        loadRemoteImages,
                        conversationHtml,
                        includedInUnified,
                        visibleInNavigation,
                        muted,
                        paused,
                        interval,
                        aliasesText,
                    )
                },
            ) {
                Text("Save")
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = { confirmRemove = true }) {
                    Text("Remove", color = MaterialTheme.colorScheme.error)
                }
                TextButton(onClick = onDismiss) {
                    Text("Cancel")
                }
            }
        },
    )
}

@Composable
internal fun AccountSwitchRow(
    label: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth().heightIn(min = 44.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(label, modifier = Modifier.weight(1f), maxLines = 2, overflow = TextOverflow.Ellipsis)
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
internal fun KanbanBoardDialog(
    boards: List<KanbanBoardSpec>,
    activeBoardId: String,
    name: String,
    avatarUrl: String,
    wallpaperPresetId: String,
    wallpaperUrl: String,
    onNameChange: (String) -> Unit,
    onAvatarUrlChange: (String) -> Unit,
    onWallpaperPresetChange: (String) -> Unit,
    onWallpaperUrlChange: (String) -> Unit,
    onSelect: (String) -> Unit,
    onRename: () -> Unit,
    onSaveAppearance: () -> Unit,
    onCreate: () -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Kanban boards") },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                item {
                    OutlinedTextField(
                        value = name,
                        onValueChange = onNameChange,
                        label = { Text("Board name") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                item {
                    OutlinedTextField(
                        value = avatarUrl,
                        onValueChange = onAvatarUrlChange,
                        label = { Text("Board image URL") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                item {
                    OutlinedTextField(
                        value = wallpaperPresetId,
                        onValueChange = onWallpaperPresetChange,
                        label = { Text("Wallpaper preset") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                item {
                    OutlinedTextField(
                        value = wallpaperUrl,
                        onValueChange = onWallpaperUrlChange,
                        label = { Text("Wallpaper image URL") },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                items(boards, key = { it.id }) { board ->
                    SidebarLikeDialogRow(
                        selected = board.id == activeBoardId,
                        title = board.name,
                        subtitle = "${board.columns.size} columns${if (board.avatarUrl.isNotBlank() || board.wallpaperPresetId.isNotBlank() || board.wallpaperUrl.isNotBlank()) " · customized" else ""}",
                        onClick = { onSelect(board.id) },
                    )
                }
            }
        },
        confirmButton = {
            Row {
                TextButton(onClick = onRename) { Text("Rename") }
                TextButton(onClick = onSaveAppearance) { Text("Save style") }
            }
        },
        dismissButton = {
            Row {
                TextButton(onClick = onCreate) { Text("New") }
                TextButton(onClick = onDelete) { Text("Delete") }
                TextButton(onClick = onDismiss) { Text("Done") }
            }
        },
    )
}

@Composable
internal fun KanbanColumnDialog(
    accounts: List<AccountSummary>,
    board: KanbanBoardSpec?,
    foldersByAccount: Map<String, List<FolderSummary>>,
    onAddColumn: (KanbanColumnSpec) -> Unit,
    onCreateFolder: (AccountSummary) -> Unit,
    onDismiss: () -> Unit,
) {
    val selected =
        board
            ?.columns
            .orEmpty()
            .map(::kanbanColumnKey)
            .toSet()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Add column") },
        text = {
            LazyColumn(Modifier.heightIn(max = 460.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                item {
                    val unified = KanbanColumnSpec(UNIFIED_ACCOUNT_ID, INBOX_FOLDER)
                    SidebarLikeDialogRow(
                        selected = selected.contains(kanbanColumnKey(unified)),
                        title = "Unified inbox",
                        subtitle = "All accounts",
                        onClick = { onAddColumn(unified) },
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
                                Text("Create folder")
                            }
                        }
                    }
                    val visibleFolders =
                        if (folders.isEmpty()) {
                            listOf(FolderSummary(account.id, INBOX_FOLDER, 0))
                        } else {
                            folders
                        }
                    items(visibleFolders, key = { "${account.id}\n${it.name}" }) { folder ->
                        val column = KanbanColumnSpec(account.id, folder.name)
                        SidebarLikeDialogRow(
                            selected = selected.contains(kanbanColumnKey(column)),
                            title = folder.name.replaceFirstChar { it.uppercase() },
                            subtitle = if (accountSummaryIsRss(account)) "Feed" else account.email,
                            onClick = { onAddColumn(column) },
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Done") }
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
        title = { Text(thread.subject.ifBlank { "(no subject)" }, maxLines = 2, overflow = TextOverflow.Ellipsis) },
        text = {
            LazyColumn(Modifier.heightIn(max = 430.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item { DialogAction("Open", onOpen) }
                item { DialogAction(if (thread.starred) "Unstar" else "Star", onToggleStar) }
                item { DialogAction(if (thread.unread) "Mark read" else "Mark unread", onToggleRead) }
                if (threadIdIsRss(thread.id) && thread.feedUrl.isNotBlank()) {
                    item { DialogAction("Copy feed URL", onCopyFeedUrl) }
                }
                item { DialogAction(if (threadIdIsRss(thread.id)) "Remove feed" else "Archive", onArchive) }
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
                            "Move to",
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 10.dp),
                        )
                    }
                    items(moveTargets, key = { kanbanColumnKey(it) }) { target ->
                        val account = accounts.firstOrNull { it.id == target.accountId }
                        DialogAction("${account?.displayName?.ifBlank { account.email } ?: target.accountId} / ${target.folderId}") {
                            onMove(target)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Close") }
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
    }
}
