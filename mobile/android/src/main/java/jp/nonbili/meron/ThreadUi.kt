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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Inbox
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
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.ListItem
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.DrawerValue
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.AnnotatedString
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
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ThreadScreen(
    thread: ThreadSummary?,
    messages: List<MessageBody>,
    accountEmail: String,
    preferHtml: Boolean,
    onPreferHtmlChange: (Boolean) -> Unit,
    onBack: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    onToggleStar: () -> Unit,
    moveFolders: List<FolderSummary>,
    copyFolders: List<FolderSummary>,
    onMoveToFolder: (FolderSummary) -> Unit,
    onCreateFolderAndMove: (String) -> Unit,
    onCopyToFolder: (FolderSummary) -> Unit,
    quickReplyBody: String,
    canLoadOlder: Boolean,
    loadingOlder: Boolean,
    onLoadOlder: () -> Unit,
    onQuickReplyChange: (String) -> Unit,
    quickReplyAttachments: List<DraftAttachment>,
    quickReplyFailure: String,
    sendShortcutMode: SendShortcutMode,
    onQuickReplyAttach: () -> Unit,
    onRemoveQuickReplyAttachment: (DraftAttachment) -> Unit,
    onOpenFullReply: () -> Unit,
    onSendReply: () -> Unit,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onToggleMessageRead: (MessageBody) -> Unit,
    onToggleMessageStarred: (MessageBody) -> Unit,
    onDeleteMessage: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onComposeTo: (String) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
) {
    val isRss = thread?.let { threadIdIsRss(it.id) } ?: false
    val deleteLabel = thread?.let { threadDeleteActionLabel(it.folder) } ?: "Move to Trash"
    val chat = LocalChatColors.current
    val context = LocalContext.current
    var searchOpen by remember(thread?.id) { mutableStateOf(false) }
    var threadSearch by remember(thread?.id) { mutableStateOf("") }
    var activeSearchIndex by remember(thread?.id) { mutableStateOf(0) }
    var detailsOpen by remember(thread?.id) { mutableStateOf(false) }
    var moveDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    var copyDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    val normalizedSearch = threadSearch.trim().lowercase()
    val currentThreadAccountId = thread?.accountId.orEmpty()
    val currentThreadFolder = thread?.folder.orEmpty()
    val targetMoveFolders = remember(currentThreadFolder, moveFolders) {
        moveFolders.filterNot { folder -> folder.name.equals(currentThreadFolder, ignoreCase = true) }
    }
    val targetCopyFolders = remember(currentThreadAccountId, currentThreadFolder, copyFolders) {
        copyFolders.filterNot { folder ->
            folder.accountId == currentThreadAccountId && folder.name.equals(currentThreadFolder, ignoreCase = true)
        }
    }
    val searchMatches = remember(messages, normalizedSearch) {
        if (normalizedSearch.isBlank()) emptyList() else messages.filter { threadMessageSearchText(it).contains(normalizedSearch) }.map { it.id }
    }
    val activeSearchId = searchMatches.getOrNull(activeSearchIndex).orEmpty()
    val listState = rememberLazyListState()
    LaunchedEffect(normalizedSearch) {
        activeSearchIndex = 0
    }
    LaunchedEffect(searchMatches.size, activeSearchIndex) {
        if (activeSearchIndex >= searchMatches.size) activeSearchIndex = 0
    }
    LaunchedEffect(activeSearchId, canLoadOlder, loadingOlder) {
        if (activeSearchId.isBlank()) return@LaunchedEffect
        val messageIndex = messages.indexOfFirst { it.id == activeSearchId }
        if (messageIndex >= 0) {
            val offset = if (canLoadOlder || loadingOlder) 1 else 0
            listState.animateScrollToItem(messageIndex + offset)
        }
    }

    fun goToSearchMatch(delta: Int) {
        if (searchMatches.isEmpty()) return
        val next = activeSearchIndex + delta
        activeSearchIndex = when {
            next < 0 -> searchMatches.lastIndex
            next > searchMatches.lastIndex -> 0
            else -> next
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        thread?.subject?.ifBlank { "(no subject)" } ?: "Conversation",
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { detailsOpen = true }) {
                        Icon(Icons.Filled.Info, contentDescription = "Conversation details")
                    }
                    IconButton(onClick = { searchOpen = !searchOpen }) {
                        Icon(Icons.Filled.Search, contentDescription = "Search conversation")
                    }
                    IconButton(onClick = onToggleStar) {
                        Icon(
                            if (thread?.starred == true) Icons.Filled.Star else Icons.Filled.StarBorder,
                            contentDescription = "Star",
                            tint = if (thread?.starred == true) chat.star else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    IconButton(onClick = onArchive) {
                        Icon(if (isRss) Icons.Filled.Delete else Icons.Filled.Archive, contentDescription = "Archive")
                    }
                    if (!isRss) {
                        IconButton(onClick = { moveDialogOpen = true }) {
                            Icon(Icons.Outlined.FolderOpen, contentDescription = "Move to folder")
                        }
                        IconButton(onClick = { copyDialogOpen = true }) {
                            Icon(Icons.Filled.ContentCopy, contentDescription = "Copy to folder")
                        }
                        IconButton(onClick = onDelete) {
                            Icon(Icons.Filled.Delete, contentDescription = deleteLabel)
                        }
                    }
                },
            )
        },
    ) { innerPadding ->
        if (detailsOpen) {
            ConversationDetailsDialog(
                messages = messages,
                isRss = isRss,
                ownEmail = accountEmail,
                onDismiss = { detailsOpen = false },
                onComposeTo = { email ->
                    detailsOpen = false
                    onComposeTo(email)
                },
                onCopy = { label, value ->
                    copyToClipboard(context, label, value)
                },
                onOpenAttachment = onOpenAttachment,
                onSaveAttachment = onSaveAttachment,
            )
        }
        if (moveDialogOpen && thread != null) {
            MoveThreadDialog(
                thread = thread,
                folders = targetMoveFolders,
                onMove = { folder ->
                    moveDialogOpen = false
                    onMoveToFolder(folder)
                },
                onCreateAndMove = { name ->
                    moveDialogOpen = false
                    onCreateFolderAndMove(name)
                },
                onDismiss = { moveDialogOpen = false },
            )
        }
        if (copyDialogOpen && thread != null) {
            CopyThreadDialog(
                thread = thread,
                folders = targetCopyFolders,
                onCopy = { folder ->
                    copyDialogOpen = false
                    onCopyToFolder(folder)
                },
                onDismiss = { copyDialogOpen = false },
            )
        }
        Column(
            Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background)
                .padding(innerPadding),
        ) {
            if (searchOpen) {
                ConversationSearchBar(
                    query = threadSearch,
                    onQueryChange = { threadSearch = it },
                    matchLabel = if (normalizedSearch.isBlank()) "" else "${if (searchMatches.isEmpty()) 0 else activeSearchIndex + 1}/${searchMatches.size}",
                    canNavigate = searchMatches.isNotEmpty(),
                    onPrevious = { goToSearchMatch(-1) },
                    onNext = { goToSearchMatch(1) },
                    onClose = {
                        threadSearch = ""
                        searchOpen = false
                    },
                )
            }
            if (!isRss) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("View", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    FilterChip(
                        selected = preferHtml,
                        onClick = { onPreferHtmlChange(true) },
                        label = { Text("HTML") },
                    )
                    FilterChip(
                        selected = !preferHtml,
                        onClick = { onPreferHtmlChange(false) },
                        label = { Text("Plain text") },
                    )
                }
            }
            if (messages.isEmpty()) {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            } else {
                LazyColumn(
                    Modifier.weight(1f),
                    state = listState,
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    if (canLoadOlder || loadingOlder) {
                        item {
                            Box(Modifier.fillMaxWidth().padding(bottom = 4.dp), contentAlignment = Alignment.Center) {
                                if (loadingOlder) {
                                    CircularProgressIndicator(Modifier.size(24.dp))
                                } else {
                                    OutlinedButton(onClick = onLoadOlder) {
                                        Text("Load older")
                                    }
                                }
                            }
                        }
                    }
                    items(messages, key = { it.id }) { message ->
                        val outgoing = isOutgoing(message, accountEmail)
                        MessageBubble(
                            message = message,
                            outgoing = outgoing,
                            chat = chat,
                            preferHtml = preferHtml,
                            searchQuery = normalizedSearch,
                            activeSearchMatch = message.id == activeSearchId,
                            actionsEnabled = !isRss,
                            onForward = onForward,
                            onEditAsNew = onEditAsNew,
                            onToggleRead = onToggleMessageRead,
                            onToggleStarred = onToggleMessageStarred,
                            onDelete = onDeleteMessage,
                            onOpenAttachment = onOpenAttachment,
                            onSaveAttachment = onSaveAttachment,
                            onCopyMessageText = onCopyMessageText,
                        )
                    }
                }
            }
            if (thread != null && !isRss && messages.isNotEmpty()) {
                ReplyBar(
                    value = quickReplyBody,
                    onChange = onQuickReplyChange,
                    attachments = quickReplyAttachments,
                    failureMessage = quickReplyFailure,
                    sendShortcutMode = sendShortcutMode,
                    onAttach = onQuickReplyAttach,
                    onRemoveAttachment = onRemoveQuickReplyAttachment,
                    onOpenFullEditor = onOpenFullReply,
                    onSend = onSendReply,
                )
            }
        }
    }
}

@Composable
internal fun ConversationDetailsDialog(
    messages: List<MessageBody>,
    isRss: Boolean,
    ownEmail: String,
    onDismiss: () -> Unit,
    onComposeTo: (String) -> Unit,
    onCopy: (String, String) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
) {
    val participants = remember(messages, isRss, ownEmail) { conversationParticipants(messages, ownEmail, isRss) }
    val attachments = remember(messages) { messages.flatMap { it.attachments }.asReversed() }
    val mediaCount = attachments.count { it.mimeType.startsWith("image/") || it.mimeType.startsWith("video/") }
    val fileCount = attachments.size - mediaCount
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Conversation details") },
        text = {
            LazyColumn(
                modifier = Modifier.heightIn(max = 460.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                item {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        AssistiveStat("People", participants.size.toString())
                        AssistiveStat("Media", mediaCount.toString())
                        AssistiveStat("Files", fileCount.toString())
                    }
                }
                item {
                    Text("People", fontWeight = FontWeight.SemiBold)
                }
                if (participants.isEmpty()) {
                    item {
                        Text("No conversation participants.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    items(participants, key = { it.email }) { person ->
                        Row(
                            Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Avatar(person.name.ifBlank { person.email }, 34.dp)
                            Column(Modifier.weight(1f)) {
                                Text(
                                    person.name.ifBlank { person.email } + if (person.isSelf) " (you)" else "",
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    "${person.email} · ${person.count}",
                                    fontSize = 12.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            TextButton(onClick = { onCopy("Email address", person.email) }) { Text("Copy") }
                            if (!person.isSelf) {
                                TextButton(onClick = { onComposeTo(person.email) }) { Text("Mail") }
                            }
                        }
                    }
                }
                item {
                    HorizontalDivider()
                    Text("Shared files", fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp))
                }
                if (attachments.isEmpty()) {
                    item {
                        Text("No shared files in loaded messages.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                } else {
                    items(attachments.withIndex().toList(), key = { it.index }) { indexed ->
                        val attachment = indexed.value
                        Row(
                            Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(Icons.Filled.AttachFile, contentDescription = null)
                            Column(Modifier.weight(1f)) {
                                Text(attachment.filename.ifBlank { "Attachment" }, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(
                                    listOf(attachment.mimeType, formatBytes(attachment.sizeBytes)).filter { it.isNotBlank() }.joinToString(" · "),
                                    fontSize = 12.sp,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            TextButton(onClick = { onOpenAttachment(attachment) }) { Text("Open") }
                            TextButton(onClick = { onSaveAttachment(attachment) }) { Text("Save") }
                        }
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
internal fun MoveThreadDialog(
    thread: ThreadSummary,
    folders: List<FolderSummary>,
    onMove: (FolderSummary) -> Unit,
    onCreateAndMove: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var newFolderName by remember(thread.id) { mutableStateOf("") }
    val trimmedNewFolderName = newFolderName.trim()
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Move conversation") },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item {
                    Text(
                        thread.subject.ifBlank { "(no subject)" },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                item {
                    OutlinedTextField(
                        value = newFolderName,
                        onValueChange = { newFolderName = it },
                        label = { Text("New folder") },
                        singleLine = true,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 8.dp),
                    )
                }
                item {
                    Button(
                        onClick = { onCreateAndMove(trimmedNewFolderName) },
                        enabled = trimmedNewFolderName.isNotBlank(),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("Create & Move")
                    }
                }
                if (folders.isEmpty()) {
                    item {
                        Text(
                            "No other folders available.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                } else {
                    items(folders, key = { "${it.accountId}\n${it.name}" }) { folder ->
                        DialogAction(folder.name.replaceFirstChar { it.uppercase() }) {
                            onMove(folder)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}

@Composable
internal fun CopyThreadDialog(
    thread: ThreadSummary,
    folders: List<FolderSummary>,
    onCopy: (FolderSummary) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Copy conversation") },
        text = {
            LazyColumn(Modifier.heightIn(max = 420.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                item {
                    Text(
                        thread.subject.ifBlank { "(no subject)" },
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                if (folders.isEmpty()) {
                    item {
                        Text(
                            "No copy targets available.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            style = MaterialTheme.typography.bodyMedium,
                            modifier = Modifier.padding(vertical = 12.dp),
                        )
                    }
                } else {
                    items(folders, key = { "${it.accountId}\n${it.name}" }) { folder ->
                        val label = if (folder.accountId.isBlank() || folder.accountId == thread.accountId) {
                            folder.name.replaceFirstChar { it.uppercase() }
                        } else {
                            "${folder.accountId} / ${folder.name.replaceFirstChar { it.uppercase() }}"
                        }
                        DialogAction(label) {
                            onCopy(folder)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}

@Composable
internal fun AssistiveStat(label: String, value: String) {
    Surface(
        shape = RoundedCornerShape(8.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
    ) {
        Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(value, fontWeight = FontWeight.Bold)
            Text(label, fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
internal fun ImagePreviewDialog(
    preview: AndroidImagePreview,
    onShare: () -> Unit,
    onCopy: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(preview.title, maxLines = 1, overflow = TextOverflow.Ellipsis)
        },
        text = {
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(min = 220.dp, max = 520.dp)
                    .background(Color.Black, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center,
            ) {
                Image(
                    bitmap = preview.bitmap.asImageBitmap(),
                    contentDescription = preview.title,
                    modifier = Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)),
                    contentScale = ContentScale.Fit,
                )
            }
        },
        dismissButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = onShare) { Text("Share") }
                TextButton(onClick = onCopy) { Text("Copy") }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Done") } },
    )
}

@Composable
internal fun ConversationSearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    matchLabel: String,
    canNavigate: Boolean,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    onClose: () -> Unit,
) {
    Surface(tonalElevation = 2.dp, color = MaterialTheme.colorScheme.surfaceContainer) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedTextField(
                value = query,
                onValueChange = onQueryChange,
                placeholder = { Text("Search conversation") },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                trailingIcon = {
                    if (query.isNotBlank()) {
                        IconButton(onClick = { onQueryChange("") }) {
                            Icon(Icons.Filled.Close, contentDescription = "Clear search")
                        }
                    }
                },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            Text(matchLabel, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(42.dp))
            TextButton(onClick = onPrevious, enabled = canNavigate) { Text("Prev") }
            TextButton(onClick = onNext, enabled = canNavigate) { Text("Next") }
            IconButton(onClick = onClose) {
                Icon(Icons.Filled.Close, contentDescription = "Close search")
            }
        }
    }
}

@Composable
internal fun MessageBubble(
    message: MessageBody,
    outgoing: Boolean,
    chat: ChatColors,
    preferHtml: Boolean,
    searchQuery: String,
    activeSearchMatch: Boolean,
    actionsEnabled: Boolean,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onToggleRead: (MessageBody) -> Unit,
    onToggleStarred: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    val bubbleShape = if (outgoing) {
        RoundedCornerShape(topStart = 16.dp, topEnd = 4.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
    } else {
        RoundedCornerShape(topStart = 4.dp, topEnd = 16.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
    }
    val bubbleColor = if (outgoing) chat.bubbleOut else chat.bubbleIn
    val textColor = if (outgoing) chat.bubbleOutText else chat.bubbleInText
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Column(
            Modifier
                .widthIn(max = 320.dp)
                .clip(bubbleShape)
                .then(
                    if (activeSearchMatch) {
                        Modifier.border(2.dp, Color(0xFFFFC107), bubbleShape)
                    } else {
                        Modifier
                    },
                )
                .background(bubbleColor)
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            if (!outgoing) {
                Text(
                    message.from.ifBlank { message.fromAddr },
                    fontSize = 12.5.sp,
                    fontWeight = FontWeight.Bold,
                    color = MaterialTheme.colorScheme.primary,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (preferHtml && message.bodyHtml.isNotBlank() && searchQuery.isBlank()) {
                HtmlMessageBody(html = message.bodyHtml)
            } else {
                // Subject is the conversation title (top bar); the bubble shows the
                // message body, matching the desktop chat reader.
                Text(
                    highlightedMessageText(message.body.ifBlank { "(no content)" }, searchQuery, activeSearchMatch),
                    color = if (message.body.isBlank()) textColor.copy(alpha = 0.6f) else textColor,
                )
            }
            if (message.attachments.isNotEmpty()) {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    message.attachments.forEach { attachment ->
                        AttachmentRow(
                            attachment = attachment,
                            textColor = textColor,
                            onOpen = { onOpenAttachment(attachment) },
                            onSave = { onSaveAttachment(attachment) },
                        )
                    }
                }
            }
            Text(
                formatRelativeTime(message.dateEpochSeconds),
                fontSize = 10.5.sp,
                color = textColor.copy(alpha = 0.6f),
                modifier = Modifier.align(Alignment.End),
            )
            Box(modifier = Modifier.align(Alignment.End)) {
                TextButton(onClick = { menuOpen = true }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = null, modifier = Modifier.size(16.dp))
                    Text("More")
                }
                DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
                    DropdownMenuItem(
                        text = { Text("Copy message text") },
                        onClick = {
                            menuOpen = false
                            onCopyMessageText("Message text", messagePlainText(message))
                        },
                    )
                    DropdownMenuItem(
                        text = { Text("Copy subject") },
                        onClick = {
                            menuOpen = false
                            onCopyMessageText("Subject", message.subject.ifBlank { "(no subject)" })
                        },
                    )
                    if (message.messageId.isNotBlank()) {
                        DropdownMenuItem(
                            text = { Text("Copy message ID") },
                            onClick = {
                                menuOpen = false
                                onCopyMessageText("Message ID", message.messageId)
                            },
                        )
                    }
                    if (actionsEnabled) {
                        DropdownMenuItem(
                            text = { Text(if (message.unread) "Mark read" else "Mark unread") },
                            onClick = {
                                menuOpen = false
                                onToggleRead(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text(if (message.starred) "Unstar" else "Star") },
                            onClick = {
                                menuOpen = false
                                onToggleStarred(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Forward") },
                            onClick = {
                                menuOpen = false
                                onForward(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Edit as new") },
                            onClick = {
                                menuOpen = false
                                onEditAsNew(message)
                            },
                        )
                        DropdownMenuItem(
                            text = { Text("Delete message", color = MaterialTheme.colorScheme.error) },
                            onClick = {
                                menuOpen = false
                                onDelete(message)
                            },
                        )
                    }
                }
            }
        }
    }
}

internal fun threadMessageSearchText(message: MessageBody): String {
    return listOf(
        message.subject,
        message.from,
        message.fromAddr,
        message.to,
        message.cc,
        message.body,
        message.bodyHtml.replace(Regex("<[^>]+>"), " "),
        message.attachments.joinToString(" ") { it.filename },
    ).joinToString(" ").lowercase()
}

internal fun threadDeleteActionLabel(folder: String): String {
    return when {
        folderIsDrafts(folder) -> "Discard draft"
        folderIsTrash(folder) -> "Delete forever"
        else -> "Move to Trash"
    }
}

internal fun messagePlainText(message: MessageBody): String {
    return message.body.ifBlank {
        message.bodyHtml
            .replace(Regex("<br\\s*/?>", RegexOption.IGNORE_CASE), "\n")
            .replace(Regex("</p>", RegexOption.IGNORE_CASE), "\n")
            .replace(Regex("<[^>]+>"), " ")
            .replace(Regex("[ \\t]+"), " ")
            .trim()
    }.ifBlank { "(no content)" }
}

internal fun conversationParticipants(messages: List<MessageBody>, ownEmail: String, isRss: Boolean): List<ConversationParticipant> {
    if (isRss) return emptyList()
    data class MutableParticipant(var name: String, val email: String, var count: Int, val isSelf: Boolean)
    val own = ownEmail.trim().lowercase()
    val byEmail = linkedMapOf<String, MutableParticipant>()

    fun add(name: String, email: String) {
        val normalized = email.trim().trim('<', '>', ',', ';').lowercase()
        if (normalized.isBlank() || !normalized.contains("@")) return
        val existing = byEmail[normalized]
        if (existing != null) {
            existing.count += 1
            if ((existing.name.isBlank() || existing.name == existing.email) && name.isNotBlank() && name != email) {
                existing.name = name
            }
        } else {
            byEmail[normalized] = MutableParticipant(
                name = name.takeIf { it.isNotBlank() && it != email } ?: normalized,
                email = normalized,
                count = 1,
                isSelf = normalized == own,
            )
        }
    }

    messages.forEach { message ->
        add(message.from, message.fromAddr.ifBlank { message.from })
        parseAddressList(message.to).forEach { (name, email) -> add(name, email) }
        parseAddressList(message.cc).forEach { (name, email) -> add(name, email) }
    }
    return byEmail.values
        .map { ConversationParticipant(it.name, it.email, it.count, it.isSelf) }
        .sortedWith(compareBy<ConversationParticipant> { it.isSelf }.thenByDescending { it.count })
}

internal fun parseAddressList(value: String): List<Pair<String, String>> {
    if (value.isBlank()) return emptyList()
    return value.split(',', ';').mapNotNull { raw ->
        val entry = raw.trim()
        if (entry.isBlank()) return@mapNotNull null
        val bracket = Regex("""^(.*)<([^>]+)>$""").matchEntire(entry)
        if (bracket != null) {
            val name = bracket.groupValues[1].trim().trim('"')
            val email = bracket.groupValues[2].trim()
            return@mapNotNull name to email
        }
        entry to entry
    }
}

internal fun copyToClipboard(context: Context, label: String, value: String) {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText(label, value))
}

internal fun highlightedMessageText(text: String, query: String, active: Boolean): AnnotatedString {
    if (query.isBlank()) return AnnotatedString(text)
    val lower = text.lowercase()
    val needle = query.lowercase()
    return buildAnnotatedString {
        var start = 0
        while (start < text.length) {
            val index = lower.indexOf(needle, start)
            if (index < 0) {
                append(text.substring(start))
                break
            }
            append(text.substring(start, index))
            withStyle(
                SpanStyle(
                    background = if (active) Color(0xFFFFD54F) else Color(0xFFFFECB3),
                    color = Color.Black,
                    fontWeight = FontWeight.SemiBold,
                ),
            ) {
                append(text.substring(index, index + needle.length))
            }
            start = index + needle.length
        }
    }
}

@Composable
internal fun HtmlMessageBody(html: String) {
    AndroidView(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 160.dp, max = 420.dp),
        factory = { context ->
            WebView(context).apply {
                webViewClient = WebViewClient()
                settings.javaScriptEnabled = false
                settings.domStorageEnabled = false
                settings.loadWithOverviewMode = true
                settings.useWideViewPort = true
                isVerticalScrollBarEnabled = true
                isHorizontalScrollBarEnabled = false
            }
        },
        update = { webView ->
            webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
        },
    )
}

@Composable
internal fun AttachmentRow(
    attachment: MessageAttachment,
    textColor: Color,
    onOpen: () -> Unit,
    onSave: () -> Unit,
) {
    Surface(
        onClick = onOpen,
        shape = RoundedCornerShape(8.dp),
        color = textColor.copy(alpha = 0.08f),
        contentColor = textColor,
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(18.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    attachment.filename.ifBlank { "Attachment" },
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    listOf(attachment.mimeType, formatBytes(attachment.sizeBytes)).filter { it.isNotBlank() }.joinToString(" · "),
                    fontSize = 10.5.sp,
                    color = textColor.copy(alpha = 0.7f),
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            TextButton(onClick = onSave) {
                Text("Save")
            }
        }
    }
}

@Composable
internal fun ReplyBar(
    value: String,
    onChange: (String) -> Unit,
    attachments: List<DraftAttachment>,
    failureMessage: String,
    sendShortcutMode: SendShortcutMode,
    onAttach: () -> Unit,
    onRemoveAttachment: (DraftAttachment) -> Unit,
    onOpenFullEditor: () -> Unit,
    onSend: () -> Unit,
) {
    Surface(tonalElevation = 3.dp, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column(
            Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (attachments.isNotEmpty()) {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    items(attachments, key = { it.id }) { attachment ->
                        FilterChip(
                            selected = false,
                            onClick = { onRemoveAttachment(attachment) },
                            label = {
                                Text(
                                    attachment.displayName,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            },
                            leadingIcon = {
                                Icon(Icons.Filled.AttachFile, contentDescription = null, modifier = Modifier.size(16.dp))
                            },
                        )
                    }
                }
            }
            if (failureMessage.isNotBlank()) {
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Icon(Icons.Filled.ErrorOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                    Text(
                        "Reply failed. Your draft is still here.",
                        modifier = Modifier.weight(1f),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                    TextButton(onClick = onSend, enabled = value.isNotBlank() || attachments.isNotEmpty()) {
                        Text("Retry")
                    }
                }
            }
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(Icons.AutoMirrored.Filled.Reply, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                OutlinedTextField(
                    value = value,
                    onValueChange = onChange,
                    placeholder = { Text("Reply") },
                    supportingText = { Text("${sendShortcutMode.label()} sends") },
                    modifier = Modifier
                        .weight(1f)
                        .onPreviewKeyEvent { event ->
                            if (shouldSendFromEditor(event, sendShortcutMode) && (value.isNotBlank() || attachments.isNotEmpty())) {
                                onSend()
                                true
                            } else {
                                false
                            }
                        },
                    maxLines = 4,
                )
                IconButton(onClick = onAttach) {
                    Icon(Icons.Filled.AttachFile, contentDescription = "Attach")
                }
                IconButton(onClick = onOpenFullEditor) {
                    Icon(Icons.Filled.OpenInFull, contentDescription = "Open full editor")
                }
                IconButton(onClick = onSend, enabled = value.isNotBlank() || attachments.isNotEmpty()) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send reply", tint = MaterialTheme.colorScheme.primary)
                }
            }
        }
    }
}
