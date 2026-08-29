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
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Check
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
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
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
import jp.nonbili.meron.shared.RemoteContentPolicy
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.ThreadMediaItem
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.attachmentMediaRef
import jp.nonbili.meron.shared.buildThreadGalleryImages
import jp.nonbili.meron.shared.buildThreadMediaItems
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.normalizeSenderAddr
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
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
    // See ReplyBar's `hasContent`: a bar holding only its seeded signature has
    // nothing to send.
    quickReplyHasContent: Boolean,
    quickReplySending: Boolean = false,
    sendShortcutMode: SendShortcutMode,
    conversationLayout: ConversationLayout,
    onQuickReplyAttach: () -> Unit,
    onRemoveQuickReplyAttachment: (DraftAttachment) -> Unit,
    onOpenFullReply: () -> Unit,
    onSendReply: () -> Unit,
    onRetryReply: () -> Unit,
    quickReplyFromIdentities: List<SendIdentity>,
    quickReplySelectedFrom: SendIdentity?,
    onSelectQuickReplyFrom: (SendIdentity) -> Unit,
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
    // Whose remote content may load in this conversation: the account's own
    // toggle plus the app-wide sender allowlist. A reveal made here is kept
    // below, for as long as the thread is open.
    remoteContentPolicy: RemoteContentPolicy,
    onAllowRemoteSender: (String) -> Unit,
    onComposeTo: (String) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
    onRetryLoadMessages: () -> Unit,
    onMessagesRead: (List<String>) -> Unit,
    onViewedToBottom: () -> Unit,
    snackbarHost: SnackbarHostState,
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
    // Messages whose remote content the reader revealed by hand. Scoped to the
    // open thread: a reveal is for this reading of this message, where trusting
    // the sender is the decision that outlives it.
    var revealedRemote by remember(thread?.id) { mutableStateOf(emptySet<String>()) }
    var galleryIndex by remember(thread?.id) { mutableStateOf<Int?>(null) }
    var moveDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    var copyDialogOpen by remember(thread?.id) { mutableStateOf(false) }
    var overflowOpen by remember(thread?.id) { mutableStateOf(false) }
    val closeSearch = {
        threadSearch = ""
        searchOpen = false
    }
    val normalizedSearch = threadSearch.trim().lowercase()
    val currentThreadAccountId = thread?.accountId.orEmpty()
    val currentThreadFolder = thread?.folder.orEmpty()
    // The gallery and the shared-media panel follow the same policy the bodies
    // do, so a blocked image has no tile to open and no slide to land on.
    val remoteAllowed: (MessageBody) -> Boolean = { message ->
        remoteContentPolicy.allows(message.fromAddr.ifBlank { message.from }) || message.id in revealedRemote
    }
    val galleryImages =
        remember(messages, remoteContentPolicy, revealedRemote) { buildThreadGalleryImages(messages, remoteAllowed) }
    val mediaItems =
        remember(messages, remoteContentPolicy, revealedRemote) { buildThreadMediaItems(messages, remoteAllowed) }
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
    // Traditional layout only: message ids the user has explicitly expanded or
    // collapsed, overriding the default below. Reset when the thread changes.
    var expandOverrides by remember(thread?.id) { mutableStateOf(emptyMap<String, Boolean>()) }
    // Which messages were unread when they first appeared. Scroll-driven read
    // marking clears `unread` while the user reads, so a default derived from
    // the live flag would collapse an open message out from under them — and
    // reaching the bottom, which marks the whole thread read, would collapse
    // every one of them at once.
    val unreadOnArrival = remember(thread?.id) { mutableSetOf<String>() }
    messages.forEach { if (it.unread) unreadOnArrival += it.id }
    // Mail-client default: the newest message is open, along with anything
    // unread or matched by the in-thread search; everything older collapses to
    // a summary line until the user taps it.
    val lastMessageId = messages.lastOrNull()?.id.orEmpty()

    fun isMessageExpanded(message: MessageBody): Boolean {
        // The match the user navigated to always shows its body: landing on a
        // collapsed summary would hide the very text that was searched for.
        if (message.id == activeSearchId) return true
        return expandOverrides[message.id]
            ?: (message.id == lastMessageId || message.id in unreadOnArrival || searchMatches.contains(message.id))
    }
    val listState = rememberLazyListState()
    // Message the reader just expanded by tapping its summary. Expanding in
    // place leaves the body wherever the tap happened to be; bring its header
    // to the top of the viewport so the message reads from its beginning.
    var scrollToExpandedId by remember(thread?.id) { mutableStateOf<String?>(null) }
    // The one list item the conversation holds at the top of the viewport, and
    // the sole owner of the scroll while it does. Both the open positioning and
    // an expanded message publish their target here rather than driving the
    // list themselves: a single slot means expanding a message during the open
    // settle window supersedes that positioning — cancelling it outright —
    // instead of the two pulling the list in different directions on every
    // layout change. Cleared once the anchor is released.
    var anchorItem by remember(thread?.id) { mutableStateOf<ThreadListAnchor?>(null) }
    val currentMessages by rememberUpdatedState(messages)
    val currentHeaderItemCount by rememberUpdatedState(threadHeaderItemCount(canLoadOlder || loadingOlder))
    LaunchedEffect(anchorItem) {
        val anchorTo = anchorItem ?: return@LaunchedEffect
        // Resolved fresh on every scroll rather than captured once: loading an
        // older page prepends messages and can drop the load-older row, which
        // shifts every list index. A held index would anchor whichever message
        // slid into that slot.

        fun targetIndex(): Int? =
            currentMessages
                .indexOfFirst { it.id == anchorTo.messageId }
                .takeIf { it >= 0 }
                ?.plus(currentHeaderItemCount)
        try {
            val target = targetIndex()
            if (target != null) {
                if (anchorTo.animated) listState.animateScrollToItem(target) else listState.scrollToItem(target)
            }
            // HTML bubbles measure their bodies asynchronously in a WebView, so
            // at this point the list may still be the height it had before and
            // the scroll above silently clamps against its end. Keep
            // re-anchoring while item sizes settle (desktop does the same with
            // a ResizeObserver); stop as soon as the user drags, or after the
            // settle window.
            withTimeoutOrNull(THREAD_OPEN_ANCHOR_WINDOW_MS) {
                coroutineScope {
                    val anchor =
                        launch {
                            snapshotFlow { listState.layoutInfo.visibleItemsInfo.map { it.index to it.size } }
                                .distinctUntilChanged()
                                .collect { targetIndex()?.let { listState.scrollToItem(it) } }
                        }
                    listState.interactionSource.interactions.first { it is DragInteraction.Start }
                    anchor.cancel()
                }
            }
        } finally {
            // In `finally` because a drag arriving mid-animation cancels the
            // scroll with a CancellationException, and an anchor left set with
            // nothing driving it would block the auto-load arming below
            // forever. Only ever releases its own anchor: a newer one cancelled
            // this effect precisely to take the list over.
            if (anchorItem == anchorTo) anchorItem = null
        }
    }
    BackHandler(
        enabled = searchOpen && !detailsOpen && readerMessage == null && galleryIndex == null,
        onBack = closeSearch,
    )
    // One-shot positioning when the thread's messages first arrive, mirroring
    // desktop: jump to the first unread message, or the newest when all read.
    var openScrollPositioned by remember(thread?.id) { mutableStateOf(false) }
    var autoLoadOlderArmed by remember(thread?.id) { mutableStateOf(false) }
    LaunchedEffect(thread?.id, messages.isEmpty()) {
        if (openScrollPositioned || messages.isEmpty()) return@LaunchedEffect
        openScrollPositioned = true
        val target = threadOpenScrollIndex(messages, hasLoadOlderRow = canLoadOlder || loadingOlder)
        val targetId = target?.let { messages.getOrNull(it - threadHeaderItemCount(canLoadOlder || loadingOlder))?.id }
        if (targetId == null) {
            autoLoadOlderArmed = true
            return@LaunchedEffect
        }
        anchorItem = ThreadListAnchor(targetId, animated = false)
        // Scrolling near the top is what asks for the older page, so hold that
        // off until the anchoring has finished moving the list — including when
        // an expanded message took the anchor over in the meantime.
        snapshotFlow { anchorItem }.first { it == null }
        autoLoadOlderArmed = true
    }
    val currentOnLoadOlder by rememberUpdatedState(onLoadOlder)
    LaunchedEffect(thread?.id, autoLoadOlderArmed, canLoadOlder, loadingOlder) {
        if (thread == null || !autoLoadOlderArmed || !canLoadOlder || loadingOlder) return@LaunchedEffect
        snapshotFlow { listState.firstVisibleItemIndex }
            .distinctUntilChanged()
            .first { firstVisibleIndex -> firstVisibleIndex <= THREAD_LOAD_OLDER_ITEM_INDEX }
        currentOnLoadOlder()
    }
    // Mark messages read as their bubbles scroll past the top of the viewport,
    // and the whole thread once the view reaches the bottom — desktop's
    // scroll-driven read marking (useConversationScroll.ts) on mobile. Marked
    // ids are remembered per open so each is sent at most once.
    val currentOnMessagesRead by rememberUpdatedState(onMessagesRead)
    val currentOnViewedToBottom by rememberUpdatedState(onViewedToBottom)
    if (thread != null) {
        val density = LocalDensity.current
        val markedReadIds = remember(thread.id) { mutableSetOf<String>() }
        var viewedToBottomSent by remember(thread.id) { mutableStateOf(false) }
        // Messages the reader turned unread by hand, held out of scroll marking
        // until they leave the viewport — otherwise the next layout change
        // (rotation, a body finishing its measurement, older messages loading)
        // marks them read again and the action looks like it did nothing.
        val heldUnreadIds = remember(thread.id) { mutableSetOf<String>() }
        val previousUnread = remember(thread.id) { mutableMapOf<String, Boolean>() }
        for (id in manualUnreadIds(messages, previousUnread)) {
            heldUnreadIds += id
            // Forget that it was ever marked read, so revisiting the thread and
            // reading it again marks it read as usual.
            markedReadIds -= id
        }
        // A message that is no longer unread has nothing left to protect —
        // the reader marked it read again, or the request rolled back. Holding
        // its id would keep the whole-thread marking below switched off for
        // every other message too.
        heldUnreadIds.retainAll { id -> messages.any { it.id == id && it.unread } }
        messages.forEach { previousUnread[it.id] = it.unread }
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
                // A held message is released once it is out of view; reading it
                // again then marks it read like any other.
                val visibleIndices = snapshot.visible.mapTo(mutableSetOf()) { it.index }
                heldUnreadIds.removeAll { id ->
                    val messageIndex = msgs.indexOfFirst { it.id == id }
                    messageIndex < 0 || (messageIndex + currentHeaderItemCount) !in visibleIndices
                }
                val readIds =
                    readMessageIndices(
                        visible = snapshot.visible,
                        firstVisibleIndex = snapshot.firstVisibleIndex,
                        headerItemCount = currentHeaderItemCount,
                        messageCount = msgs.size,
                        topSlackPx = topSlackPx,
                        viewportEndOffset = snapshot.viewportEndOffset,
                    ).mapNotNull { msgs.getOrNull(it) }
                        .filter { it.unread }
                        .map { it.id }
                        .filter { it !in heldUnreadIds }
                        .filter { markedReadIds.add(it) }
                if (readIds.isNotEmpty()) currentOnMessagesRead(readIds)
                val atBottom =
                    listViewedToBottom(
                        visible = snapshot.visible,
                        totalItemCount = snapshot.totalItemCount,
                        viewportEndOffset = snapshot.viewportEndOffset,
                        bottomSlackPx = bottomSlackPx,
                    )
                // Reaching the bottom marks the whole thread read — including
                // messages on older pages that are not loaded — so it has to
                // stand down while a hand-unread message is on screen, or it
                // undoes the action the held set just protected. It resumes as
                // soon as that message scrolls out of view.
                if (atBottom && heldUnreadIds.isEmpty() && (!viewedToBottomSent || msgs.any { it.unread })) {
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
    LaunchedEffect(scrollToExpandedId, canLoadOlder, loadingOlder) {
        val id = scrollToExpandedId ?: return@LaunchedEffect
        scrollToExpandedId = null
        // The body the tap just revealed has not been measured yet, so the
        // anchor above — not a one-off scroll here — is what gets the header to
        // the top and keeps it there while the body grows into place.
        anchorItem = ThreadListAnchor(id, animated = true)
    }
    LaunchedEffect(activeSearchId, canLoadOlder, loadingOlder) {
        if (activeSearchId.isBlank()) return@LaunchedEffect
        val messageIndex = messages.indexOfFirst { it.id == activeSearchId }
        if (messageIndex >= 0) {
            listState.animateScrollToItem(messageIndex + threadHeaderItemCount(canLoadOlder || loadingOlder))
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

    fun remoteContentFor(message: MessageBody): MessageRemoteContent {
        val sender = normalizeSenderAddr(message.fromAddr.ifBlank { message.from })
        return MessageRemoteContent(
            allowRemote = remoteAllowed(message),
            // Trusting the sender of the user's own mail would be a no-op that
            // still grew the allowlist, so outgoing messages only offer a reveal.
            senderAddress = if (isOutgoing(message, accountEmail)) "" else sender,
            onReveal = { revealedRemote = revealedRemote + message.id },
            onAllowSender = { onAllowRemoteSender(sender) },
        )
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
                            val subtitle = threadHeaderSubtitle(messages, isRss)
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
            snackbarHost = { SnackbarHost(snackbarHost) },
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
                        onClose = closeSearch,
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
                        val traditional = conversationLayout == ConversationLayout.Traditional
                        LazyColumn(
                            Modifier.fillMaxSize().appScrollbar(listState),
                            state = listState,
                            contentPadding =
                                PaddingValues(
                                    horizontal = if (traditional) 8.dp else 12.dp,
                                    vertical = if (traditional) 8.dp else 16.dp,
                                ),
                            verticalArrangement = Arrangement.spacedBy(if (traditional) 6.dp else 10.dp),
                        ) {
                            // The app bar clips the subject to one line, so repeat it
                            // in full at the top of the list. It scrolls away with the
                            // messages; the bar title stays as the fallback.
                            item {
                                Text(
                                    (thread?.subject ?: "").ifBlank { tr("threads.noSubject") },
                                    modifier = Modifier.fillMaxWidth().padding(bottom = 4.dp),
                                    fontWeight = FontWeight.Medium,
                                    style = MaterialTheme.typography.titleLarge.copy(fontSize = 18.sp, lineHeight = 24.sp),
                                )
                            }
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
                                if (conversationLayout == ConversationLayout.Traditional) {
                                    val expanded = isMessageExpanded(message)
                                    MessageRow(
                                        message = message,
                                        outgoing = outgoing,
                                        preferHtml = preferHtml,
                                        searchQuery = normalizedSearch,
                                        activeSearchMatch = message.id == activeSearchId,
                                        expanded = expanded,
                                        onToggleExpanded = {
                                            expandOverrides = expandOverrides + (message.id to !expanded)
                                            if (!expanded) scrollToExpandedId = message.id
                                        },
                                        actionsEnabled = !isRss,
                                        itemActionsEnabled = true,
                                        showSubject = isRss,
                                        isRss = isRss,
                                        remoteContent = remoteContentFor(message),
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
                                        onComposeTo = onComposeTo,
                                        onOpenMessage = { readerMessage = it },
                                        onOpenUrl = services::openUrl,
                                        onRetryLoad = onRetryLoadMessages,
                                    )
                                } else {
                                    MessageBubble(
                                        message = message,
                                        outgoing = outgoing,
                                        chat = chat,
                                        preferHtml = preferHtml,
                                        searchQuery = normalizedSearch,
                                        activeSearchMatch = message.id == activeSearchId,
                                        actionsEnabled = !isRss,
                                        itemActionsEnabled = true,
                                        showSubject = isRss,
                                        isRss = isRss,
                                        remoteContent = remoteContentFor(message),
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
                                        onComposeTo = onComposeTo,
                                        onOpenMessage = { readerMessage = it },
                                        onOpenUrl = services::openUrl,
                                        onRetryLoad = onRetryLoadMessages,
                                    )
                                }
                            }
                        }
                        // Reading a message taller than the screen scrolls its
                        // header away, and with it the tap target that collapses
                        // the message again. Float a copy of that header at the
                        // top for as long as the message is the one being read.
                        if (traditional) {
                            val density = LocalDensity.current
                            val minRemainingPx = with(density) { 72.dp.roundToPx() }
                            val headerItemCount = threadHeaderItemCount(canLoadOlder || loadingOlder)
                            // Recomputed on every recomposition so the derived
                            // state below never reads a stale expansion map.
                            val expandedFlags by rememberUpdatedState(messages.map(::isMessageExpanded))
                            val pinnedIndex by remember(headerItemCount, messages.size) {
                                derivedStateOf {
                                    val info = listState.layoutInfo
                                    pinnedHeaderMessageIndex(
                                        visible = info.visibleItemsInfo.map { ListItemGeometry(it.index, it.offset, it.size) },
                                        headerItemCount = headerItemCount,
                                        messageCount = expandedFlags.size,
                                        viewportStartOffset = info.viewportStartOffset,
                                        minRemainingPx = minRemainingPx,
                                        expanded = { index -> expandedFlags.getOrElse(index) { false } },
                                    )
                                }
                            }
                            val pinnedMessage = pinnedIndex?.let { messages.getOrNull(it) }
                            pinnedMessage?.let { message ->
                                val outgoing = isOutgoing(message, accountEmail)
                                val textColor = MaterialTheme.colorScheme.onSurface
                                Surface(
                                    color = MaterialTheme.colorScheme.surface,
                                    shape = RoundedCornerShape(topStart = 12.dp, topEnd = 12.dp),
                                    shadowElevation = 3.dp,
                                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                                ) {
                                    Column(Modifier.padding(horizontal = 12.dp, vertical = 8.dp)) {
                                        MessageRowHeader(
                                            message = message,
                                            senderLabel = messageSenderLabel(message, outgoing),
                                            outgoing = outgoing,
                                            isDraft = folderIsDrafts(message.folderId),
                                            isRss = isRss,
                                            textColor = textColor,
                                            mutedColor = textColor.copy(alpha = 0.6f),
                                            actionsEnabled = !isRss,
                                            itemActionsEnabled = true,
                                            onToggleExpanded = {
                                                expandOverrides = expandOverrides + (message.id to false)
                                            },
                                            onForward = onForward,
                                            onEditAsNew = onEditAsNew,
                                            onOpenDraft = onOpenDraft,
                                            onToggleRead = onToggleMessageRead,
                                            onToggleStarred = onToggleMessageStarred,
                                            onDelete = onDeleteMessage,
                                            onCopyMessageText = onCopyMessageText,
                                            onComposeTo = onComposeTo,
                                            onOpenMessage = { readerMessage = it },
                                        )
                                    }
                                }
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
                        onRetry = onRetryReply,
                        hasContent = quickReplyHasContent,
                        sending = quickReplySending,
                        fromIdentities = quickReplyFromIdentities,
                        selectedFrom = quickReplySelectedFrom,
                        onSelectFrom = onSelectQuickReplyFrom,
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
                actionsEnabled = !isRss,
                remoteContent = remoteContentFor(reader),
                onBack = { readerMessage = null },
                onCopy = { label, value -> services.copyText(label, value) },
                onComposeTo = { email ->
                    readerMessage = null
                    onComposeTo(email)
                },
                onForward = { message ->
                    readerMessage = null
                    onForward(message)
                },
                onEditAsNew = { message ->
                    readerMessage = null
                    onEditAsNew(message)
                },
                onDelete = { message ->
                    readerMessage = null
                    onDeleteMessage(message)
                },
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
        val detailsListState = rememberLazyListState()
        LazyColumn(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .appScrollbar(detailsListState)
                    .padding(horizontal = 16.dp),
            state = detailsListState,
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
    onRetry: () -> Unit = onSend,
    // Whether the bar holds anything worth sending. Passed in rather than read
    // off `value`, because a bar seeded with the account's signature is not
    // blank yet holds nothing the user wrote.
    hasContent: Boolean = value.isNotBlank() || attachments.isNotEmpty(),
    sending: Boolean = false,
    fromIdentities: List<SendIdentity> = emptyList(),
    selectedFrom: SendIdentity? = null,
    onSelectFrom: (SendIdentity) -> Unit = {},
) {
    Surface(tonalElevation = 3.dp, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column(
            Modifier.fillMaxWidth().padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (selectedFrom != null && fromIdentities.size > 1) {
                ReplyFromRow(
                    identities = fromIdentities,
                    selected = selectedFrom,
                    onSelect = onSelectFrom,
                )
            }
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
                    TextButton(onClick = onRetry, enabled = !sending) {
                        Text(tr("chat.retry"))
                    }
                }
            }
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.Bottom,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val canSend = !sending && hasContent
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

// The reply bar's send-as identity: shows which address the reply goes out from,
// and opens a picker to override it. Only rendered when the account has more
// than one identity — with a single address there is nothing to disclose.
@Composable
private fun ReplyFromRow(
    identities: List<SendIdentity>,
    selected: SendIdentity,
    onSelect: (SendIdentity) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        Row(
            Modifier
                .clip(RoundedCornerShape(8.dp))
                .clickable { expanded = true }
                .padding(horizontal = 6.dp, vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                tr("composer.fields.from"),
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                formatSendIdentity(selected),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
            Icon(
                Icons.Filled.ArrowDropDown,
                contentDescription = tr("composer.actions.chooseSendAddress"),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(16.dp),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            identities.forEach { identity ->
                DropdownMenuItem(
                    text = {
                        Text(formatSendIdentity(identity), maxLines = 1, overflow = TextOverflow.Ellipsis)
                    },
                    leadingIcon = {
                        if (identity.email.equals(selected.email, ignoreCase = true)) {
                            Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                        }
                    },
                    onClick = {
                        expanded = false
                        onSelect(identity)
                    },
                )
            }
        }
    }
}
