package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.ThreadMediaItem
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.attachmentMediaRef
import jp.nonbili.meron.shared.buildThreadGalleryImages
import jp.nonbili.meron.shared.buildThreadMediaItems
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ThreadScreen(
    thread: ThreadSummary?,
    messages: List<MessageBody>,
    accountEmail: String,
    wallpaperPresetId: String,
    wallpaperCustomUrl: String,
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
    quickReplySending: Boolean = false,
    sendShortcutMode: SendShortcutMode,
    onQuickReplyAttach: () -> Unit,
    onRemoveQuickReplyAttachment: (DraftAttachment) -> Unit,
    onOpenFullReply: () -> Unit,
    onSendReply: () -> Unit,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onOpenDraft: (MessageBody) -> Unit,
    onToggleMessageRead: (MessageBody) -> Unit,
    onToggleMessageStarred: (MessageBody) -> Unit,
    onDeleteMessage: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onShareImageAttachment: (MessageAttachment) -> Unit,
    onCopyImageAttachment: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onComposeTo: (String) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
    onRetryLoadMessages: () -> Unit,
    onMessagesScrolledPast: (List<String>) -> Unit,
    onViewedToBottom: () -> Unit,
) {
    val isRss = thread?.let { threadIdIsRss(it.id) } ?: false
    val deleteLabel = thread?.let { threadDeleteActionLabel(it.folder, it.folderRole) } ?: "Move to Trash"
    val chat = LocalChatColors.current
    val services = LocalPlatformServices.current
    var searchOpen by remember(thread?.id) { mutableStateOf(false) }
    var threadSearch by remember(thread?.id) { mutableStateOf("") }
    var activeSearchIndex by remember(thread?.id) { mutableStateOf(0) }
    var detailsOpen by remember(thread?.id) { mutableStateOf(false) }
    var readerMessage by remember(thread?.id) { mutableStateOf<MessageBody?>(null) }
    var galleryIndex by remember(thread?.id) { mutableStateOf<Int?>(null) }
    var moveDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    var copyDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    var overflowOpen by remember(thread?.id) { mutableStateOf(false) }
    val normalizedSearch = threadSearch.trim().lowercase()
    val currentThreadAccountId = thread?.accountId.orEmpty()
    val currentThreadFolder = thread?.folder.orEmpty()
    val galleryImages = remember(messages) { buildThreadGalleryImages(messages) }
    val mediaItems = remember(messages) { buildThreadMediaItems(messages) }
    val targetMoveFolders =
        remember(currentThreadFolder, moveFolders) {
            moveFolders.filterNot { folder -> folder.name.equals(currentThreadFolder, ignoreCase = true) }
        }
    val targetCopyFolders =
        remember(currentThreadAccountId, currentThreadFolder, copyFolders) {
            copyFolders.filterNot { folder ->
                folder.accountId == currentThreadAccountId && folder.name.equals(currentThreadFolder, ignoreCase = true)
            }
        }
    val searchMatches =
        remember(messages, normalizedSearch) {
            if (normalizedSearch.isBlank()) {
                emptyList()
            } else {
                messages
                    .filter {
                        threadMessageSearchText(
                            it,
                        ).contains(normalizedSearch)
                    }.map { it.id }
            }
        }
    val activeSearchId = searchMatches.getOrNull(activeSearchIndex).orEmpty()
    val listState = rememberLazyListState()
    // One-shot positioning when the thread's messages first arrive, mirroring
    // desktop: jump to the first unread message, or the newest when all read.
    var openScrollPositioned by remember(thread?.id) { mutableStateOf(false) }
    var autoLoadOlderArmed by remember(thread?.id) { mutableStateOf(false) }
    LaunchedEffect(thread?.id, messages.isEmpty()) {
        if (openScrollPositioned || messages.isEmpty()) return@LaunchedEffect
        openScrollPositioned = true
        val headerItemCount = if (canLoadOlder || loadingOlder) 1 else 0
        val target = threadOpenScrollIndex(messages, headerItemCount)
        if (target == null) {
            autoLoadOlderArmed = true
            return@LaunchedEffect
        }
        listState.scrollToItem(target)
        // HTML bubbles measure their bodies asynchronously in a WebView, so at
        // this point the list may still fit the viewport and the scroll above
        // silently clamps to the top. Keep re-anchoring to the target while
        // item sizes settle (desktop does the same with a ResizeObserver);
        // stop as soon as the user drags, or after the settle window.
        withTimeoutOrNull(THREAD_OPEN_ANCHOR_WINDOW_MS) {
            coroutineScope {
                val anchor =
                    launch {
                        snapshotFlow { listState.layoutInfo.visibleItemsInfo.map { it.index to it.size } }
                            .distinctUntilChanged()
                            .collect { listState.scrollToItem(target) }
                    }
                listState.interactionSource.interactions.first { it is DragInteraction.Start }
                anchor.cancel()
            }
        }
        autoLoadOlderArmed = true
    }
    val currentOnLoadOlder by rememberUpdatedState(onLoadOlder)
    LaunchedEffect(thread?.id, autoLoadOlderArmed, canLoadOlder, loadingOlder) {
        if (thread == null || !autoLoadOlderArmed || !canLoadOlder || loadingOlder) return@LaunchedEffect
        snapshotFlow { listState.firstVisibleItemIndex }
            .distinctUntilChanged()
            .first { firstVisibleIndex -> firstVisibleIndex == 0 }
        currentOnLoadOlder()
    }
    // Mark messages read as their bubbles scroll past the top of the viewport,
    // and the whole thread once the view reaches the bottom — desktop's
    // scroll-driven read marking (useConversationScroll.ts) on mobile. Marked
    // ids are remembered per open so each is sent at most once.
    val currentMessages by rememberUpdatedState(messages)
    val currentHeaderItemCount by rememberUpdatedState(if (canLoadOlder || loadingOlder) 1 else 0)
    val currentOnMessagesScrolledPast by rememberUpdatedState(onMessagesScrolledPast)
    val currentOnViewedToBottom by rememberUpdatedState(onViewedToBottom)
    if (thread != null) {
        val density = LocalDensity.current
        val markedReadIds = remember(thread.id) { mutableSetOf<String>() }
        var viewedToBottomSent by remember(thread.id) { mutableStateOf(false) }
        LaunchedEffect(thread.id) {
            val topSlackPx = with(density) { 24.dp.roundToPx() }
            val bottomSlackPx = with(density) { 160.dp.roundToPx() }
            snapshotFlow {
                val info = listState.layoutInfo
                ThreadScrollSnapshot(
                    firstVisibleIndex = listState.firstVisibleItemIndex,
                    visible = info.visibleItemsInfo.map { ListItemGeometry(it.index, it.offset, it.size) },
                    totalItemCount = info.totalItemsCount,
                    viewportEndOffset = info.viewportEndOffset,
                )
            }.collect { snapshot ->
                val msgs = currentMessages
                if (msgs.isEmpty()) return@collect
                val passedIds =
                    scrolledPastMessageIndices(
                        visible = snapshot.visible,
                        firstVisibleIndex = snapshot.firstVisibleIndex,
                        headerItemCount = currentHeaderItemCount,
                        messageCount = msgs.size,
                        topSlackPx = topSlackPx,
                    ).mapNotNull { msgs.getOrNull(it) }
                        .filter { it.unread }
                        .map { it.id }
                        .filter { markedReadIds.add(it) }
                if (passedIds.isNotEmpty()) currentOnMessagesScrolledPast(passedIds)
                val atBottom =
                    listViewedToBottom(
                        visible = snapshot.visible,
                        totalItemCount = snapshot.totalItemCount,
                        viewportEndOffset = snapshot.viewportEndOffset,
                        bottomSlackPx = bottomSlackPx,
                    )
                if (atBottom && (!viewedToBottomSent || msgs.any { it.unread })) {
                    viewedToBottomSent = true
                    currentOnViewedToBottom()
                }
            }
        }
    }
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
        activeSearchIndex =
            when {
                next < 0 -> searchMatches.lastIndex
                next > searchMatches.lastIndex -> 0
                else -> next
            }
    }

    fun galleryIndexForAttachment(attachment: MessageAttachment): Int? {
        val ref = attachmentMediaRef(attachment)
        return galleryImages
            .indexOfFirst { image ->
                (image.attachment.key.isNotBlank() && image.attachment.key == attachment.key) ||
                    (image.ref == ref && ref.isNotBlank())
            }.takeIf { it >= 0 }
    }

    fun openGalleryForAttachment(attachment: MessageAttachment) {
        galleryIndexForAttachment(attachment)?.let { galleryIndex = it } ?: onOpenAttachment(attachment)
    }

    fun openGalleryForHtmlSrc(src: String) {
        val normalized = src.trim()
        if (normalized.isBlank()) return
        val idx =
            galleryImages.indexOfFirst { image ->
                image.ref == normalized ||
                    normalized.endsWith(image.ref) ||
                    (image.attachment.key.isNotBlank() && normalized.contains("/media/${image.attachment.key}"))
            }
        if (idx >= 0) {
            galleryIndex = idx
        } else {
            services.openUrl(normalized)
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = {
                        Column(Modifier.clickable { detailsOpen = true }) {
                            Text(
                                thread?.subject?.ifBlank { "(no subject)" } ?: "Conversation",
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                                fontWeight = FontWeight.SemiBold,
                                style = MaterialTheme.typography.titleMedium,
                            )
                            val subtitle = threadHeaderSubtitle(messages, accountEmail, isRss)
                            if (subtitle.isNotBlank()) {
                                Text(
                                    subtitle,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    },
                    navigationIcon = {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                        }
                    },
                    actions = {
                        IconButton(onClick = onToggleStar) {
                            Icon(
                                if (thread?.starred == true) Icons.Filled.Star else Icons.Filled.StarBorder,
                                contentDescription = tr("chat.star"),
                                tint = if (thread?.starred == true) chat.star else MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Box {
                            IconButton(onClick = { overflowOpen = true }) {
                                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreActions"))
                            }
                            DropdownMenu(expanded = overflowOpen, onDismissRequest = { overflowOpen = false }) {
                                DropdownMenuItem(
                                    text = { Text(if (isRss) tr("feeds.actions.deleteFeed") else tr("threads.actions.archiveThread")) },
                                    leadingIcon = { Icon(if (isRss) Icons.Filled.Delete else Icons.Filled.Archive, contentDescription = null) },
                                    onClick = {
                                        overflowOpen = false
                                        onArchive()
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text(tr("chat.searchThread")) },
                                    leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                                    onClick = {
                                        overflowOpen = false
                                        searchOpen = !searchOpen
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text(if (isRss) tr("chat.feedDetails") else tr("chat.conversationDetails")) },
                                    leadingIcon = { Icon(Icons.Filled.Info, contentDescription = null) },
                                    onClick = {
                                        overflowOpen = false
                                        detailsOpen = true
                                    },
                                )
                                if (!isRss) {
                                    DropdownMenuItem(
                                        text = { Text(if (preferHtml) "View as plain text" else "View as HTML") },
                                        leadingIcon = { Icon(Icons.Filled.Code, contentDescription = null) },
                                        onClick = {
                                            overflowOpen = false
                                            onPreferHtmlChange(!preferHtml)
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(tr("threads.actions.moveTo")) },
                                        leadingIcon = { Icon(Icons.Outlined.FolderOpen, contentDescription = null) },
                                        onClick = {
                                            overflowOpen = false
                                            moveDialogOpen = true
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(tr("threads.actions.copyTo")) },
                                        leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                                        onClick = {
                                            overflowOpen = false
                                            copyDialogOpen = true
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(deleteLabel, color = MaterialTheme.colorScheme.error) },
                                        leadingIcon = { Icon(Icons.Filled.Delete, contentDescription = null, tint = MaterialTheme.colorScheme.error) },
                                        onClick = {
                                            overflowOpen = false
                                            onDelete()
                                        },
                                    )
                                }
                            }
                        }
                    },
                )
            },
        ) { innerPadding ->
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
                    .padding(innerPadding)
                    .consumeWindowInsets(innerPadding)
                    .imePadding(),
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
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    ChatWallpaperBackground(
                        presetId = wallpaperPresetId,
                        customUrl = wallpaperCustomUrl,
                        modifier = Modifier.matchParentSize(),
                    )
                    if (messages.isEmpty()) {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    } else {
                        LazyColumn(
                            Modifier.fillMaxSize(),
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
                                                Text(tr("threads.actions.loadMore"))
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
                                    showSubject = isRss,
                                    onForward = onForward,
                                    onEditAsNew = onEditAsNew,
                                    onOpenDraft = onOpenDraft,
                                    onToggleRead = onToggleMessageRead,
                                    onToggleStarred = onToggleMessageStarred,
                                    onDelete = onDeleteMessage,
                                    onOpenAttachment = onOpenAttachment,
                                    onSaveAttachment = onSaveAttachment,
                                    loadImageAttachment = loadImageAttachment,
                                    onOpenImageAttachment = ::openGalleryForAttachment,
                                    onOpenHtmlImage = ::openGalleryForHtmlSrc,
                                    onCopyMessageText = onCopyMessageText,
                                    onOpenMessage = { readerMessage = it },
                                    onOpenUrl = services::openUrl,
                                    onRetryLoad = onRetryLoadMessages,
                                )
                            }
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
                        sending = quickReplySending,
                    )
                }
            }
        }

        if (detailsOpen) {
            ConversationDetailsScreen(
                subject = thread?.subject?.takeIf { it.isNotBlank() } ?: tr("threads.noSubject"),
                messages = messages,
                mediaItems = mediaItems,
                loadImageAttachment = loadImageAttachment,
                isRss = isRss,
                feedUrl = thread?.feedUrl.orEmpty(),
                ownEmail = accountEmail,
                onBack = { detailsOpen = false },
                onComposeTo = { email ->
                    detailsOpen = false
                    onComposeTo(email)
                },
                onCopy = { label, value ->
                    services.copyText(label, value)
                },
                onOpenAttachment = onOpenAttachment,
                onSaveAttachment = onSaveAttachment,
                onOpenGalleryIndex = { index -> galleryIndex = index },
                onOpenUrl = services::openUrl,
            )
        }

        // Overlay the full-screen message reader when open
        readerMessage?.let { reader ->
            MessageReaderScreen(
                message = reader,
                preferHtml = preferHtml,
                onBack = { readerMessage = null },
                onCopy = { label, value -> services.copyText(label, value) },
                onOpenAttachment = onOpenAttachment,
                onSaveAttachment = onSaveAttachment,
                loadImageAttachment = loadImageAttachment,
                onOpenImageAttachment = ::openGalleryForAttachment,
                onOpenHtmlImage = ::openGalleryForHtmlSrc,
                onOpenUrl = services::openUrl,
            )
        }
        galleryIndex?.let { index ->
            if (galleryImages.getOrNull(index) != null) {
                ThreadImageGallery(
                    images = galleryImages,
                    index = index,
                    onIndexChange = { galleryIndex = it },
                    onClose = { galleryIndex = null },
                    onSave = onSaveAttachment,
                    onShare = onShareImageAttachment,
                    onCopy = onCopyImageAttachment,
                    loadImageAttachment = loadImageAttachment,
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun ConversationDetailsScreen(
    subject: String,
    messages: List<MessageBody>,
    mediaItems: List<ThreadMediaItem>,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    isRss: Boolean,
    feedUrl: String,
    ownEmail: String,
    onBack: () -> Unit,
    onComposeTo: (String) -> Unit,
    onCopy: (String, String) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    onOpenGalleryIndex: (Int) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    val participants = remember(messages, isRss, ownEmail) { conversationParticipants(messages, ownEmail, isRss) }
    val attachments = remember(messages) { messages.flatMap { it.attachments }.asReversed() }
    val fileAttachments = remember(attachments) { attachments.filter { !it.mimeType.startsWith("image/") && !it.mimeType.startsWith("video/") } }
    val mediaRows = remember(mediaItems) { mediaItems.chunked(3) }
    val subjectLabel = tr("composer.fields.subject")

    BackHandler(onBack = onBack)

    Scaffold(
        modifier = Modifier.fillMaxSize(),
        topBar = {
            TopAppBar(
                title = { Text(if (isRss) tr("chat.feedDetails") else tr("chat.conversationDetails")) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                    }
                },
            )
        },
    ) { innerPadding ->
        LazyColumn(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            // Section 0: Subject
            item {
                Text(
                    text = subjectLabel.uppercase(),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                )
                Row(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .padding(vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = subject,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(
                        onClick = { onCopy(subjectLabel, subject) },
                        modifier = Modifier.size(36.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.ContentCopy,
                            contentDescription = tr("chat.copySubject"),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
                HorizontalDivider(
                    modifier = Modifier.padding(top = 12.dp),
                    color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                )
            }

            // Section 1: Feed URL
            if (isRss && feedUrl.isNotBlank()) {
                item {
                    Text(
                        text = tr("chat.feedUrl").uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                    )
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .clickable { onOpenUrl(feedUrl) }
                                .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(
                            text = feedUrl,
                            color = MaterialTheme.colorScheme.primary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(
                            onClick = { onCopy("Feed URL", feedUrl) },
                            modifier = Modifier.size(36.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.ContentCopy,
                                contentDescription = tr("common.copy"),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                    HorizontalDivider(
                        modifier = Modifier.padding(top = 12.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                }
            }

            // Section 2: People
            if (participants.isNotEmpty()) {
                item {
                    Text(
                        text = tr("chat.people", mapOf("count" to participants.size)).uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
                    )
                }
                items(participants, key = { it.email }) { person ->
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Avatar(person.name.ifBlank { person.email }, 36.dp)
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = person.name.ifBlank { person.email } + if (person.isSelf) " (${tr("chat.you")})" else "",
                                fontWeight = FontWeight.Medium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            if (person.name.isNotBlank() && person.name != person.email) {
                                Text(
                                    text = person.email,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            IconButton(
                                onClick = { onCopy("Email address", person.email) },
                                modifier = Modifier.size(36.dp),
                            ) {
                                Icon(
                                    imageVector = Icons.Filled.ContentCopy,
                                    contentDescription = tr("chat.copyEmailAddress"),
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            if (!person.isSelf) {
                                IconButton(
                                    onClick = { onComposeTo(person.email) },
                                    modifier = Modifier.size(36.dp),
                                ) {
                                    Icon(
                                        imageVector = Icons.Filled.Email,
                                        contentDescription = tr("mobile.tabs.mail"),
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                        }
                    }
                }
            }

            // Section 3: Media
            if (mediaItems.isNotEmpty()) {
                item {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 12.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                    Text(
                        text = tr("chat.media").uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                }
                itemsIndexed(
                    mediaRows,
                    key = { index, row -> "$index:${row.joinToString("|") { "${it.type}-${it.filename}" }}" },
                ) { _, row ->
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        row.forEach { item ->
                            Box(Modifier.weight(1f)) {
                                ConversationMediaTile(
                                    item = item,
                                    loadImageAttachment = loadImageAttachment,
                                    onOpen = {
                                        val imageIndex = item.galleryIndex
                                        if (item.type == "image" && imageIndex != null) {
                                            onOpenGalleryIndex(imageIndex)
                                        } else {
                                            onOpenAttachment(item.attachment)
                                        }
                                    },
                                )
                            }
                        }
                        repeat(3 - row.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }

            // Section 3: Files
            if (fileAttachments.isNotEmpty()) {
                item {
                    HorizontalDivider(
                        modifier = Modifier.padding(vertical = 12.dp),
                        color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                    )
                    Text(
                        text = tr("chat.files").uppercase(),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                }
                items(fileAttachments.withIndex().toList(), key = { "file-${it.index}" }) { indexed ->
                    val attachment = indexed.value
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(8.dp))
                                .padding(vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.AttachFile,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.size(24.dp),
                        )
                        Column(Modifier.weight(1f)) {
                            Text(
                                text = attachment.filename.ifBlank { tr("chat.attachment") },
                                fontWeight = FontWeight.Medium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text =
                                    listOf(
                                        attachment.mimeType,
                                        formatBytes(attachment.sizeBytes),
                                    ).filter { it.isNotBlank() }.joinToString(" · "),
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            TextButton(
                                onClick = { onOpenAttachment(attachment) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(tr("kanban.actions.openThread"), fontSize = 12.sp)
                            }
                            TextButton(
                                onClick = { onSaveAttachment(attachment) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                                modifier = Modifier.height(32.dp),
                            ) {
                                Text(tr("buttons.save"), fontSize = 12.sp)
                            }
                        }
                    }
                }
            }
        }
    }
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
                placeholder = { Text(tr("chat.searchThread")) },
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                trailingIcon = {
                    if (query.isNotBlank()) {
                        IconButton(onClick = { onQueryChange("") }) {
                            Icon(Icons.Filled.Close, contentDescription = tr("common.clearSearch"))
                        }
                    }
                },
                singleLine = true,
                modifier = Modifier.weight(1f),
            )
            Text(matchLabel, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(42.dp))
            TextButton(onClick = onPrevious, enabled = canNavigate) { Text(tr("chat.previousMatch")) }
            TextButton(onClick = onNext, enabled = canNavigate) { Text(tr("chat.nextMatch")) }
            IconButton(onClick = onClose) {
                Icon(Icons.Filled.Close, contentDescription = tr("chat.closeThreadSearch"))
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
    sending: Boolean = false,
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
                        tr("reply.failedDraftKept"),
                        modifier = Modifier.weight(1f),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                    TextButton(onClick = onSend, enabled = !sending && (value.isNotBlank() || attachments.isNotEmpty())) {
                        Text(tr("chat.retry"))
                    }
                }
            }
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val canSend = !sending && (value.isNotBlank() || attachments.isNotEmpty())
                TextField(
                    value = value,
                    onValueChange = onChange,
                    placeholder = { Text(tr("composer.placeholders.quickMessage")) },
                    shape = RoundedCornerShape(24.dp),
                    colors =
                        TextFieldDefaults.colors(
                            focusedContainerColor = MaterialTheme.colorScheme.surface,
                            unfocusedContainerColor = MaterialTheme.colorScheme.surface,
                            focusedIndicatorColor = Color.Transparent,
                            unfocusedIndicatorColor = Color.Transparent,
                            disabledIndicatorColor = Color.Transparent,
                        ),
                    trailingIcon = {
                        Row {
                            IconButton(onClick = onAttach) {
                                Icon(Icons.Filled.AttachFile, contentDescription = tr("composer.actions.attachFiles"), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                            IconButton(onClick = onOpenFullEditor) {
                                Icon(Icons.Filled.OpenInFull, contentDescription = tr("composer.actions.openFullEditor"), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    },
                    modifier =
                        Modifier
                            .weight(1f)
                            .onPreviewKeyEvent { event ->
                                if (shouldSendFromEditor(event, sendShortcutMode) && canSend) {
                                    onSend()
                                    true
                                } else {
                                    false
                                }
                            },
                    maxLines = 5,
                )
                FilledIconButton(
                    onClick = onSend,
                    enabled = canSend,
                    modifier = Modifier.size(48.dp),
                ) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = tr("reply.send"))
                }
            }
        }
    }
}
