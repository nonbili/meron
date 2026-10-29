package jp.nonbili.meron.ui

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.interaction.DragInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.PlaylistAdd
import androidx.compose.material.icons.automirrored.filled.ReplyAll
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
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.UnfoldLess
import androidx.compose.material.icons.filled.UnfoldMore
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
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.chrisbanes.haze.HazeInput
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.blur.HazeBlurStyle
import dev.chrisbanes.haze.blur.HazeColorEffect
import dev.chrisbanes.haze.blur.hazeBlur
import dev.chrisbanes.haze.hazeSource
import dev.chrisbanes.haze.rememberHazeState
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.RemoteContentPolicy
import jp.nonbili.meron.shared.ReplyRecipients
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
import kotlinx.coroutines.flow.takeWhile
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
    /** Null when Tasks is switched off, which hides the action entirely. */
    onAddToTasks: (() -> Unit)? = null,
    moveFolders: List<FolderSummary>,
    copyFolders: List<FolderSummary>,
    transferAccounts: List<jp.nonbili.meron.shared.AccountSummary> = emptyList(),
    loadTransferFolders: () -> Unit = {},
    onMoveToFolder: (FolderSummary) -> Unit,
    onCreateFolderAndMove: (String) -> Unit,
    onCopyToFolder: (FolderSummary) -> Unit,
    recentTransfers: List<RecentThreadTransferItem> = emptyList(),
    loadRecentTransferFolders: suspend () -> Unit = {},
    quickReplyBody: String,
    canLoadOlder: Boolean,
    loadingOlder: Boolean,
    onLoadOlder: () -> Unit,
    loadPrintMessages: suspend () -> List<MessageBody>,
    onPrintError: (String) -> Unit,
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
    onReplyAll: () -> Unit,
    canReplyAllToThread: Boolean,
    onSendReply: () -> Unit,
    onRetryReply: () -> Unit,
    quickReplyFromIdentities: List<SendIdentity>,
    quickReplyRecipients: ReplyRecipients,
    quickReplySelectedFrom: SendIdentity?,
    onSelectQuickReplyFrom: (SendIdentity) -> Unit,
    onForward: (MessageBody) -> Unit,
    onReplyToMessage: (MessageBody) -> Unit,
    onReplyAllToMessage: (MessageBody) -> Unit,
    canReplyAllToMessage: (MessageBody) -> Boolean,
    onEditAsNew: (MessageBody) -> Unit,
    readerBottomActions: Boolean,
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
    onRespondToInvite: (MessageBody, String) -> Unit,
    onRemoveInviteEvent: (MessageBody) -> Unit,
    onAddInviteToCalendar: (MessageBody) -> Unit,
    onMessagesRead: (List<String>) -> Unit,
    onViewedToBottom: () -> Unit,
    snackbarHost: SnackbarHostState,
    swipe: ThreadSwipeState,
    /** Pulling the conversation down from its top closes it, as pulling the
     *  message reader down does. Held above the pages, which it moves together. */
    pull: PullToDismissState,
    /** The conversations beside this one in the list it was opened from, null
     *  where there is none (or swiping away is not safe right now). */
    onSwipeToPrevious: (() -> Unit)? = null,
    onSwipeToNext: (() -> Unit)? = null,
    /** False for a conversation laid out beside the open one, to be swiped over
     *  to: it shows as it would open, but takes no input, handles no back, and
     *  neither marks anything read nor loads older mail. */
    active: Boolean = true,
    modifier: Modifier = Modifier,
) {
    val isRss = thread?.let { threadIdIsRss(it.id) } ?: false
    val deleteLabel = tr(thread?.let { threadDeleteActionKey(it.folder, it.folderRole) } ?: "threads.actions.moveToTrash")
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
    val printThread = rememberPrintThread(preferHtml)
    val printError = tr("chat.couldNotPrintThread")
    var printing by remember(thread?.id) { mutableStateOf(false) }
    val printScope = rememberCoroutineScope()
    var overflowOpen by remember(thread?.id) { mutableStateOf(false) }
    LaunchedEffect(overflowOpen, thread?.accountId) {
        if (overflowOpen) loadRecentTransferFolders()
    }
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
        remember(currentThreadAccountId, currentThreadFolder, moveFolders) {
            moveFolders.filterNot { folder ->
                folder.accountId.ifBlank { currentThreadAccountId } == currentThreadAccountId && folder.name.equals(currentThreadFolder, ignoreCase = true)
            }
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
    // The reader's last expand-all or collapse-all from the menu, null until
    // used. Every message without a toggle of its own follows it, earlier ones
    // loaded afterwards included.
    var expandAll by remember(thread?.id) { mutableStateOf<Boolean?>(null) }
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
            // Collapsing all still leaves the newest open: a thread of nothing
            // but summary lines has nothing to read, and it is the one a reply
            // answers.
            ?: expandAll?.let { it || message.id == lastMessageId }
            ?: (message.id == lastMessageId || message.id in unreadOnArrival || searchMatches.contains(message.id))
    }
    // What the menu's expand-all / collapse-all item would do now: true to
    // expand, false to collapse, null where there is nothing to offer.
    val expandAllAction: Boolean? =
        when {
            conversationLayout != ConversationLayout.Traditional -> null

            messages.any { !isMessageExpanded(it) } -> true

            // Collapsing all leaves the newest open, which is all a thread of
            // one has.
            messages.size > 1 -> false

            else -> null
        }
    val listState = rememberLazyListState()
    // The quick reply floats over the end of the list, whose viewport runs on
    // underneath it. What the reader can see ends at the card's top, so
    // everything that asks where the viewport ends — read marking, the
    // keyboard anchor — takes the card's height off first.
    var replyBarHeightPx by remember { mutableIntStateOf(0) }
    // The header pinned over a long message, its top gap included; zero while
    // nothing is pinned.
    var pinnedHeaderHeightPx by remember { mutableIntStateOf(0) }
    // The conversation, captured for the frosted bands at either end of it.
    val hazeState = rememberHazeState()
    val showReplyBar = thread != null && !isRss && messages.isNotEmpty()
    val replyOverlayPx by rememberUpdatedState(if (showReplyBar) replyBarHeightPx else 0)
    var endFollowRequest by remember(thread?.id) { mutableStateOf(0) }
    var followNextLocalSend by remember(thread?.id) { mutableStateOf(false) }
    var keyboardAnchor by remember(thread?.id) { mutableStateOf<Pair<String, Int>?>(null) }
    // Expanding or collapsing a card can dispose the view that holds focus (the
    // tapped card, or its web view), and focus then falls to the next focusable
    // thing, the quick reply, which opens the keyboard. Drop focus first, unless
    // the reply is what the reader is typing in.
    val focusManager = LocalFocusManager.current
    val releaseFocusBeforeToggle = { if (keyboardAnchor == null) focusManager.clearFocus() }
    val currentMessages by rememberUpdatedState(messages)
    val currentHeaderItemCount by rememberUpdatedState(threadHeaderItemCount(canLoadOlder || loadingOlder))

    // The one list item the conversation holds at the top of the viewport, and
    // the sole owner of the scroll while it does. Both the open positioning and
    // an expanded message publish their target here rather than driving the
    // list themselves: a single slot means expanding a message during the open
    // settle window supersedes that positioning — cancelling it outright —
    // instead of the two pulling the list in different directions on every
    // layout change. Cleared once the anchor is released.
    var anchorItem by remember(thread?.id) { mutableStateOf<ThreadListAnchor?>(null) }

    fun requestEndFollow() {
        // Following the end takes the list over from whatever message was being
        // held at the top; left running, the two would pull it opposite ways on
        // every layout change.
        anchorItem = null
        endFollowRequest += 1
    }

    LaunchedEffect(endFollowRequest) {
        if (endFollowRequest == 0) return@LaunchedEffect
        // IME resize and asynchronously measured message bodies can both move
        // the end after the first scroll. Follow layout changes briefly, unless
        // the user takes over by dragging the conversation.
        withTimeoutOrNull(1_000) {
            coroutineScope {
                val follower =
                    launch {
                        snapshotFlow {
                            listState.layoutInfo.run { totalItemsCount to visibleItemsInfo.map { Triple(it.index, it.offset, it.size) } }
                        }.distinctUntilChanged()
                            .collect {
                                val last = listState.layoutInfo.totalItemsCount - 1
                                if (last >= 0) listState.scrollToItem(last)
                            }
                    }
                listState.interactionSource.interactions.first { it is DragInteraction.Start }
                follower.cancel()
            }
        }
    }
    // Expand all / collapse all with the reader at the end of the thread. The
    // list holds the item at the top of the screen, which in a short thread is
    // the first summary line, so everything above the newest message opening
    // would carry the reader off to the start. They were reading the newest
    // message, which neither action collapses: hold the end instead, while the
    // bodies settle, unless the reader drags or a message anchor (a summary
    // tapped open) takes the list over.
    var endHoldRequest by remember(thread?.id) { mutableStateOf(0) }
    LaunchedEffect(endHoldRequest) {
        if (endHoldRequest == 0) return@LaunchedEffect
        anchorItem = null
        withTimeoutOrNull(THREAD_OPEN_ANCHOR_WINDOW_MS) {
            coroutineScope {
                val holder =
                    launch {
                        // Null once an anchor is set, which ends the hold.
                        snapshotFlow {
                            if (anchorItem != null) null else listState.layoutInfo.visibleItemsInfo.map { it.index to it.size }
                        }.distinctUntilChanged()
                            .takeWhile { it != null }
                            .collect {
                                if (!listState.canScrollForward) return@collect
                                val last = listState.layoutInfo.totalItemsCount - 1
                                if (last < 0) return@collect
                                // To the newest message's top, then on to the
                                // end: its bottom is what was on screen.
                                listState.scrollToItem(last)
                                val info = listState.layoutInfo
                                val lastSize = info.visibleItemsInfo.lastOrNull()?.size ?: 0
                                listState.scrollBy((lastSize + info.viewportEndOffset).toFloat())
                            }
                    }
                val dragWatch =
                    launch {
                        listState.interactionSource.interactions.first { it is DragInteraction.Start }
                        holder.cancel()
                    }
                holder.join()
                dragWatch.cancel()
            }
        }
    }
    LaunchedEffect(messages.lastOrNull()?.id) {
        if (!followNextLocalSend || messages.lastOrNull()?.id?.startsWith("local-send-") != true) return@LaunchedEffect
        followNextLocalSend = false
        requestEndFollow()
    }
    LaunchedEffect(keyboardAnchor) {
        val (messageId, bottomGapPx) = keyboardAnchor ?: return@LaunchedEffect
        coroutineScope {
            val anchor =
                launch {
                    snapshotFlow {
                        val info = listState.layoutInfo
                        val messageIndex = currentMessages.indexOfFirst { it.id == messageId }
                        Triple(
                            info.viewportStartOffset to info.viewportEndOffset - replyOverlayPx,
                            info.visibleItemsInfo.map { ListItemGeometry(it.index, it.offset, it.size) },
                            messageIndex.takeIf { it >= 0 }?.plus(currentHeaderItemCount),
                        )
                    }.distinctUntilChanged()
                        .collect { (viewport, visible, targetIndex) ->
                            targetIndex ?: return@collect
                            val item = visible.firstOrNull { it.index == targetIndex }
                            if (item == null) {
                                listState.scrollToItem(targetIndex)
                                return@collect
                            }
                            val desiredBottom = viewport.second - bottomGapPx
                            val correction = item.offset + item.size - desiredBottom
                            if (kotlin.math.abs(correction) > 1) listState.scrollBy(correction.toFloat())
                        }
                }
            listState.interactionSource.interactions.first { it is DragInteraction.Start }
            anchor.cancel()
        }
    }
    // Message the reader just expanded by tapping its summary. Expanding in
    // place leaves the body wherever the tap happened to be; bring its header
    // to the top of the viewport so the message reads from its beginning.
    var scrollToExpandedId by remember(thread?.id) { mutableStateOf<String?>(null) }
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
        enabled = active && searchOpen && !detailsOpen && readerMessage == null && galleryIndex == null,
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
    // Keyed on [active] too: a page positioned while beside the open one starts
    // watching once it is swiped over to.
    LaunchedEffect(thread?.id, active, autoLoadOlderArmed, canLoadOlder, loadingOlder) {
        if (thread == null || !active || !autoLoadOlderArmed || !canLoadOlder || loadingOlder) return@LaunchedEffect
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
    if (thread != null && active) {
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
                    viewportEndOffset = info.viewportEndOffset - replyOverlayPx,
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
                        // A body still loading (or failed) shows only a short
                        // placeholder, which fits on screen without having been
                        // read. It is marked once the body lands and relayouts.
                        .filter { it.unread && !it.bodyMissing }
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
                // It also waits for placeholder bodies, for the same reason.
                if (
                    atBottom &&
                    heldUnreadIds.isEmpty() &&
                    msgs.none { it.unread && it.bodyMissing } &&
                    (!viewedToBottomSent || msgs.any { it.unread })
                ) {
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

    Box(
        modifier =
            modifier
                .fillMaxSize()
                .then(if (active) Modifier else Modifier.blockPointerInput()),
    ) {
        Scaffold(
            topBar = {
                TopAppBar(
                    // The subject is at the top of the list instead, in full;
                    // the bar keeps only its floating buttons.
                    title = {},
                    navigationIcon = {
                        FloatingBarChip(CircleShape, Modifier.padding(start = 4.dp).width(FLOATING_BAR_CHIP_HEIGHT)) {
                            IconButton(onClick = onBack) {
                                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                            }
                        }
                    },
                    actions = {
                        FloatingBarChip(RoundedCornerShape(50), Modifier.padding(end = 4.dp)) {
                            if (printing) CircularProgressIndicator(Modifier.padding(start = 12.dp).size(20.dp))
                            // On the bar, like the desktop header's, and not the
                            // Tasks screen's checklist icon: this files the thread.
                            if (onAddToTasks != null) {
                                IconButton(onClick = onAddToTasks) {
                                    Icon(Icons.AutoMirrored.Filled.PlaylistAdd, contentDescription = tr("tasks.addFromMessage"))
                                }
                            }
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
                                    expandAllAction?.let { expand ->
                                        DropdownMenuItem(
                                            text = { Text(if (expand) tr("chat.expandAll") else tr("chat.collapseAll")) },
                                            leadingIcon = {
                                                Icon(
                                                    if (expand) Icons.Filled.UnfoldMore else Icons.Filled.UnfoldLess,
                                                    contentDescription = null,
                                                )
                                            },
                                            onClick = {
                                                overflowOpen = false
                                                releaseFocusBeforeToggle()
                                                // The list keeps the message at the top of
                                                // the screen where it is as the rows
                                                // around it change height — or, for a
                                                // reader at the end, the end.
                                                if (!listState.canScrollForward) endHoldRequest += 1
                                                expandOverrides = emptyMap()
                                                expandAll = expand
                                            },
                                        )
                                    }
                                    if (!isRss) {
                                        DropdownMenuItem(
                                            text = { Text(tr("chat.actions.printThread")) },
                                            leadingIcon = { Icon(Icons.Filled.Print, contentDescription = null) },
                                            trailingIcon = { if (printing) CircularProgressIndicator(Modifier.size(16.dp)) },
                                            enabled = !printing && thread != null,
                                            onClick = {
                                                overflowOpen = false
                                                printing = true
                                                val subject = thread?.subject.orEmpty()
                                                printScope.launch {
                                                    try {
                                                        printThread(subject, loadPrintMessages())
                                                    } catch (error: kotlinx.coroutines.CancellationException) {
                                                        throw error
                                                    } catch (_: Exception) {
                                                        onPrintError(printError)
                                                    } finally {
                                                        printing = false
                                                    }
                                                }
                                            },
                                        )
                                        // Hidden when the reply target has no other
                                        // recipients: a reply-all identical to the
                                        // reply is a second name for the same action.
                                        if (canReplyAllToThread) {
                                            DropdownMenuItem(
                                                text = { Text(tr("chat.actions.replyAll")) },
                                                leadingIcon = { Icon(Icons.AutoMirrored.Filled.ReplyAll, contentDescription = null) },
                                                onClick = {
                                                    overflowOpen = false
                                                    onReplyAll()
                                                },
                                            )
                                        }
                                        DropdownMenuItem(
                                            text = { Text(if (preferHtml) tr("chat.viewAsPlainText") else tr("chat.viewAsHtml")) },
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
                                                loadTransferFolders()
                                                moveDialogOpen = true
                                            },
                                        )
                                        DropdownMenuItem(
                                            text = { Text(tr("threads.actions.copyTo")) },
                                            leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                                            onClick = {
                                                overflowOpen = false
                                                loadTransferFolders()
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
                        }
                    },
                    // See-through: the conversation scrolls on under the bar,
                    // between its floating chips, and under the status bar.
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, scrolledContainerColor = Color.Transparent),
                )
            },
            snackbarHost = { SnackbarHost(snackbarHost) },
        ) { innerPadding ->
            val layoutDirection = LocalLayoutDirection.current
            val barHeight = innerPadding.calculateTopPadding()
            // The list runs up under the app bar, except while the search bar
            // sits between them.
            val underBar = if (searchOpen) 0.dp else barHeight
            val underBarPx = with(LocalDensity.current) { underBar.roundToPx() }
            // And down under the navigation bar, below the reply card, which
            // itself stays above it.
            val navBarHeight = innerPadding.calculateBottomPadding()
            val contentPadding =
                PaddingValues(
                    start = innerPadding.calculateStartPadding(layoutDirection),
                    top = barHeight - underBar,
                    end = innerPadding.calculateEndPadding(layoutDirection),
                )
            if (moveDialogOpen && thread != null) {
                MoveThreadDialog(
                    thread = thread,
                    folders = targetMoveFolders,
                    accounts = transferAccounts,
                    recentTransfers = recentTransfers,
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
                    accounts = transferAccounts,
                    recentTransfers = recentTransfers,
                    onCopy = { folder ->
                        copyDialogOpen = false
                        onCopyToFolder(folder)
                    },
                    onDismiss = { copyDialogOpen = false },
                )
            }
            // Behind the reply card as well as the messages, so the card floats
            // on the conversation. Not inside the keyboard's padding: the
            // picture keeps its crop while the keyboard comes and goes.
            ChatWallpaperBackground(
                presetId = wallpaperPresetId,
                customUrl = wallpaperCustomUrl,
                modifier = Modifier.fillMaxSize().padding(contentPadding).hazeSource(hazeState, zIndex = 0f),
            )
            Column(
                Modifier
                    .fillMaxSize()
                    .padding(contentPadding)
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
                // Only the conversation itself pages: not while searching it,
                // and not under the reader, gallery or details over it.
                val swipeOpen = !searchOpen && !detailsOpen && readerMessage == null && galleryIndex == null
                Box(
                    Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .then(
                            if (active) {
                                Modifier.threadSwipe(
                                    state = swipe,
                                    onPrevious = onSwipeToPrevious?.takeIf { swipeOpen },
                                    onNext = onSwipeToNext?.takeIf { swipeOpen },
                                )
                            } else {
                                Modifier
                            },
                        )
                        // Not while typing a reply, whose field scrolls too.
                        .pullToDismiss(
                            pull,
                            enabled = active && swipeOpen && WindowInsets.ime.getBottom(LocalDensity.current) == 0,
                            isAtTop = { !listState.canScrollBackward },
                            onDismiss = onBack,
                        ),
                ) {
                    if (messages.isEmpty()) {
                        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    } else {
                        val traditional = conversationLayout == ConversationLayout.Traditional
                        val sidePadding = if (traditional) 8.dp else 12.dp
                        val edgePadding = if (traditional) 8.dp else 16.dp
                        // The last message scrolls clear of the reply card,
                        // whose own top margin is the gap between the two.
                        val replyBarHeight = with(LocalDensity.current) { replyOverlayPx.toDp() }
                        LazyColumn(
                            Modifier
                                .fillMaxSize()
                                .hazeSource(hazeState, zIndex = 1f)
                                .appScrollbar(listState),
                            state = listState,
                            contentPadding =
                                PaddingValues(
                                    start = sidePadding,
                                    end = sidePadding,
                                    top = underBar + edgePadding,
                                    // The reply card's measured height takes in the navigation bar.
                                    bottom = if (replyOverlayPx > 0) replyBarHeight else navBarHeight + edgePadding,
                                ),
                            verticalArrangement = Arrangement.spacedBy(if (traditional) 6.dp else 10.dp),
                        ) {
                            // The subject, in full, at the top of the list. It
                            // scrolls away with the messages.
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
                                            releaseFocusBeforeToggle()
                                            expandOverrides = expandOverrides + (message.id to !expanded)
                                            if (!expanded) scrollToExpandedId = message.id
                                        },
                                        actionsEnabled = !isRss,
                                        itemActionsEnabled = true,
                                        showSubject = isRss,
                                        isRss = isRss,
                                        remoteContent = remoteContentFor(message),
                                        onForward = onForward,
                                        onReplyAllToMessage = onReplyAllToMessage,
                                        canReplyAllToMessage = canReplyAllToMessage,
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
                                        onOpenUrl = services::openUrl,
                                        onRetryLoad = onRetryLoadMessages,
                                        onRespondToInvite = onRespondToInvite,
                                        onRemoveInviteEvent = onRemoveInviteEvent,
                                        onAddInviteToCalendar = onAddInviteToCalendar,
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
                                        onReplyAllToMessage = onReplyAllToMessage,
                                        canReplyAllToMessage = canReplyAllToMessage,
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
                                        onRespondToInvite = onRespondToInvite,
                                        onRemoveInviteEvent = onRemoveInviteEvent,
                                        onAddInviteToCalendar = onAddInviteToCalendar,
                                    )
                                }
                            }
                            item(key = "conversation-end") {
                                Spacer(Modifier.height(1.dp))
                            }
                        }
                        // What scrolls on under the status bar and the app bar,
                        // down to the middle of the pinned header while there
                        // is one, is frosted; so is what shows around the reply
                        // card and under the navigation bar, from a line the
                        // card itself covers: below its top margin and the
                        // curve of its corners. Over the list, under the pinned
                        // header and the card.
                        val fadePx = with(LocalDensity.current) { 16.dp.toPx() }
                        val topBandPx =
                            when {
                                // Its measured height takes in the bar it sits below.
                                pinnedHeaderHeightPx > 0 -> underBarPx + (pinnedHeaderHeightPx - underBarPx) / 2

                                else -> underBarPx
                            }
                        val bottomBandPx =
                            if (replyOverlayPx > 0) {
                                with(LocalDensity.current) { replyOverlayPx - (REPLY_CARD_TOP_MARGIN + REPLY_CARD_RADIUS).roundToPx() }
                            } else {
                                with(LocalDensity.current) { navBarHeight.roundToPx() }
                            }
                        FrostedBand(
                            hazeState,
                            heightPx = topBandPx,
                            // Soft where it ends in the open; the pinned header covers its end otherwise.
                            fadeAtBottomPx = if (pinnedHeaderHeightPx > 0) 0f else fadePx,
                            // A little heavier: the status bar's icons sit on it.
                            tintAlpha = 0.4f,
                            modifier = Modifier.align(Alignment.TopCenter),
                        )
                        FrostedBand(
                            hazeState,
                            heightPx = bottomBandPx,
                            fadeAtTopPx = fadePx,
                            modifier = Modifier.align(Alignment.BottomCenter),
                        )
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
                            val pinnedIndex by remember(headerItemCount, messages.size, underBarPx) {
                                derivedStateOf {
                                    val info = listState.layoutInfo
                                    pinnedHeaderMessageIndex(
                                        visible = info.visibleItemsInfo.map { ListItemGeometry(it.index, it.offset, it.size) },
                                        headerItemCount = headerItemCount,
                                        messageCount = expandedFlags.size,
                                        // The bottom of the app bar, not the top of the screen.
                                        viewportStartOffset = info.viewportStartOffset + underBarPx,
                                        minRemainingPx = minRemainingPx,
                                        expanded = { index -> expandedFlags.getOrElse(index) { false } },
                                    )
                                }
                            }
                            val pinnedMessage = pinnedIndex?.let { messages.getOrNull(it) }
                            pinnedMessage?.let { message ->
                                val outgoing = isOutgoing(message, accountEmail)
                                val textColor = MaterialTheme.colorScheme.onSurface
                                DisposableEffect(Unit) { onDispose { pinnedHeaderHeightPx = 0 } }
                                // A card floating on the conversation, drawn
                                // like the reply card at the other end of it.
                                Surface(
                                    color = MaterialTheme.colorScheme.surface,
                                    shape = RoundedCornerShape(16.dp),
                                    border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f)),
                                    shadowElevation = FLOATING_CARD_ELEVATION,
                                    modifier =
                                        Modifier
                                            .fillMaxWidth()
                                            .onSizeChanged { pinnedHeaderHeightPx = it.height }
                                            .padding(start = 8.dp, end = 8.dp, top = underBar + 6.dp),
                                ) {
                                    Column(Modifier.padding(horizontal = 12.dp, vertical = 6.dp)) {
                                        MessageRowHeader(
                                            compact = true,
                                            message = message,
                                            senderLabel = messageSenderLabel(message, outgoing),
                                            outgoing = outgoing,
                                            isDraft = folderIsDrafts(message.folderId),
                                            isRss = isRss,
                                            textColor = textColor,
                                            mutedColor = textColor.copy(alpha = 0.6f),
                                            remoteContent = remoteContentFor(message),
                                            preferHtml = preferHtml,
                                            searchQuery = normalizedSearch,
                                            actionsEnabled = !isRss,
                                            itemActionsEnabled = true,
                                            onToggleExpanded = {
                                                releaseFocusBeforeToggle()
                                                expandOverrides = expandOverrides + (message.id to false)
                                            },
                                            onForward = onForward,
                                            onReplyAllToMessage = onReplyAllToMessage,
                                            canReplyAllToMessage = canReplyAllToMessage,
                                            onEditAsNew = onEditAsNew,
                                            onOpenDraft = onOpenDraft,
                                            onToggleRead = onToggleMessageRead,
                                            onToggleStarred = onToggleMessageStarred,
                                            onDelete = onDeleteMessage,
                                            onCopyMessageText = onCopyMessageText,
                                            onOpenUrl = services::openUrl,
                                            onComposeTo = onComposeTo,
                                        )
                                    }
                                }
                            }
                        }
                    }
                    // Over the list rather than under it: messages scroll on
                    // behind the card, like desktop's composer.
                    if (showReplyBar) {
                        // With the keyboard up, the list already stops the
                        // navigation bar's height into it, so the same padding
                        // puts the card right on top of the keyboard.
                        Box(
                            Modifier
                                .align(Alignment.BottomCenter)
                                .onSizeChanged { replyBarHeightPx = it.height }
                                .padding(bottom = navBarHeight),
                        ) {
                            ReplyBar(
                                value = quickReplyBody,
                                onChange = onQuickReplyChange,
                                attachments = quickReplyAttachments,
                                failureMessage = quickReplyFailure,
                                sendShortcutMode = sendShortcutMode,
                                onAttach = onQuickReplyAttach,
                                onRemoveAttachment = onRemoveQuickReplyAttachment,
                                onOpenFullEditor = onOpenFullReply,
                                onFocusChange = { focused ->
                                    keyboardAnchor =
                                        if (focused) {
                                            val info = listState.layoutInfo
                                            bottomVisibleMessageAnchor(
                                                visible = info.visibleItemsInfo.map { ListItemGeometry(it.index, it.offset, it.size) },
                                                headerItemCount = currentHeaderItemCount,
                                                messageCount = messages.size,
                                                viewportStartOffset = info.viewportStartOffset + underBarPx,
                                                viewportEndOffset = info.viewportEndOffset - replyOverlayPx,
                                            )?.let { anchor -> messages.getOrNull(anchor.messageIndex)?.id?.let { it to anchor.bottomGapPx } }
                                        } else {
                                            null
                                        }
                                },
                                onSend = {
                                    keyboardAnchor = null
                                    // The reader's own reply is followed from wherever
                                    // they had scrolled to, unlike mail that arrives.
                                    followNextLocalSend = true
                                    onSendReply()
                                },
                                onRetry = onRetryReply,
                                hasContent = quickReplyHasContent,
                                sending = quickReplySending,
                                fromIdentities = quickReplyFromIdentities,
                                recipients = quickReplyRecipients,
                                selectedFrom = quickReplySelectedFrom,
                                onSelectFrom = onSelectQuickReplyFrom,
                            )
                        }
                    }
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
                isRss = isRss,
                actionsEnabled = !isRss,
                remoteContent = remoteContentFor(reader),
                onBack = { readerMessage = null },
                onCopy = { label, value -> services.copyText(label, value) },
                onComposeTo = { email ->
                    readerMessage = null
                    onComposeTo(email)
                },
                bottomActions = readerBottomActions,
                onReplyToMessage = { message ->
                    readerMessage = null
                    onReplyToMessage(message)
                },
                canReplyAllToMessage = canReplyAllToMessage,
                onReplyAllToMessage = { message ->
                    readerMessage = null
                    onReplyAllToMessage(message)
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
                        val feedUrlLabel = tr("chat.feedUrl")
                        IconButton(
                            onClick = { onCopy(feedUrlLabel, feedUrl) },
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
                            val emailLabel = tr("accounts.fields.emailAddress")
                            IconButton(
                                onClick = { onCopy(emailLabel, person.email) },
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

private val REPLY_CARD_TOP_MARGIN = 8.dp
private val REPLY_CARD_RADIUS = 24.dp

// The reply card and the pinned header ride over the conversation, so they
// sit higher than the bubbles (3.dp) that scroll under them.
private val FLOATING_CARD_ELEVATION = 4.dp
internal val FLOATING_BAR_CHIP_HEIGHT = 48.dp

// A card floating on content that scrolls on under it, drawn like the reply
// card and the pinned header.
@Composable
internal fun FloatingCard(
    shape: Shape,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = shape,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f)),
        shadowElevation = FLOATING_CARD_ELEVATION,
        modifier = modifier,
        content = content,
    )
}

// One of an app bar's controls, floating on the content that scrolls on under
// the bar.
@Composable
internal fun FloatingBarChip(
    shape: Shape,
    modifier: Modifier = Modifier,
    content: @Composable RowScope.() -> Unit,
) {
    FloatingCard(shape, modifier.height(FLOATING_BAR_CHIP_HEIGHT)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center, content = content)
    }
}

/**
 * Frosts what [hazeState]'s sources draw behind this band: content scrolling on
 * under the system bars and the floating controls stays visible as movement
 * without its text competing with them. [fadeAtTopPx] and [fadeAtBottomPx]
 * soften an edge that shows in the open.
 *
 * The sources are drawn into an offscreen layer, which MailWebView.android.kt
 * notes crashes Android's WebView natively on some GL-pipeline devices. A Pixel
 * 3a on Skia GL renders HTML bodies into it fine; fall back to a plain veil if
 * reports say otherwise.
 */
@Composable
internal fun FrostedBand(
    hazeState: HazeState,
    heightPx: Int,
    modifier: Modifier = Modifier,
    fadeAtTopPx: Float = 0f,
    fadeAtBottomPx: Float = 0f,
    // Strong enough to keep lines of text from reading as text, light
    // enough that they still show moving underneath.
    tintAlpha: Float = 0.3f,
) {
    if (heightPx <= 0) return
    val background = MaterialTheme.colorScheme.background
    val style =
        remember(background, heightPx, fadeAtTopPx, fadeAtBottomPx, tintAlpha) {
            val height = heightPx.toFloat()
            HazeBlurStyle {
                backgroundColor(background)
                blurRadius(12.dp)
                noiseFactor(0f)
                colorEffects(listOf(HazeColorEffect.tint(background.copy(alpha = tintAlpha))))
                // Where blurring is unavailable (before Android 12); Haze
                // strengthens it to make up for the missing blur.
                fallbackColorEffect(HazeColorEffect.tint(background.copy(alpha = 0.7f)))
                if (fadeAtTopPx > 0f || fadeAtBottomPx > 0f) {
                    mask(
                        Brush.verticalGradient(
                            0f to if (fadeAtTopPx > 0f) Color.Transparent else Color.Black,
                            (fadeAtTopPx / height).coerceIn(0f, 0.5f) to Color.Black,
                            (1f - fadeAtBottomPx / height).coerceIn(0.5f, 1f) to Color.Black,
                            1f to if (fadeAtBottomPx > 0f) Color.Transparent else Color.Black,
                        ),
                    )
                }
            }
        }
    Box(
        modifier
            .fillMaxWidth()
            .height(with(LocalDensity.current) { heightPx.toDp() })
            .hazeBlur(HazeInput.Sources(hazeState), style),
    )
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
    onFocusChange: (Boolean) -> Unit = {},
    onSend: () -> Unit,
    onRetry: () -> Unit = onSend,
    // Whether the bar holds anything worth sending. Passed in rather than read
    // off `value`, because a bar seeded with the account's signature is not
    // blank yet holds nothing the user wrote.
    hasContent: Boolean = value.isNotBlank() || attachments.isNotEmpty(),
    sending: Boolean = false,
    fromIdentities: List<SendIdentity> = emptyList(),
    recipients: ReplyRecipients = ReplyRecipients("", ""),
    selectedFrom: SendIdentity? = null,
    onSelectFrom: (SendIdentity) -> Unit = {},
) {
    // One rounded card floating on the conversation, like desktop's composer:
    // the text and its controls read as a single box rather than as a field
    // set into a bar.
    Surface(
        modifier = Modifier.fillMaxWidth().padding(start = 12.dp, end = 12.dp, top = REPLY_CARD_TOP_MARGIN, bottom = 12.dp),
        shape = RoundedCornerShape(REPLY_CARD_RADIUS),
        // The surface the message cards are drawn on, so the card reads as
        // one of them rather than as a pressed or disabled slab.
        color = MaterialTheme.colorScheme.surface,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f)),
        shadowElevation = FLOATING_CARD_ELEVATION,
    ) {
        Column(Modifier.fillMaxWidth().padding(4.dp)) {
            if (attachments.isNotEmpty()) {
                LazyRow(
                    contentPadding = PaddingValues(start = 10.dp, end = 10.dp, top = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
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
                    Modifier.fillMaxWidth().padding(start = 12.dp, top = 4.dp),
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
            val canSend = !sending && hasContent
            // The text leads and has the card's full width; everything else
            // shares the one line under it, so a phone's width goes to the
            // reply rather than to the buttons beside it.
            // A bare field rather than Material's, whose 56dp minimum made the
            // card half as tall again as it needs to be. The padding is inside
            // the field, so a tap anywhere on the row focuses it.
            val replyTextStyle = MaterialTheme.typography.bodyLarge.copy(color = MaterialTheme.colorScheme.onSurface)
            BasicTextField(
                value = value,
                onValueChange = onChange,
                textStyle = replyTextStyle,
                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                maxLines = 5,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .onFocusChanged { onFocusChange(it.isFocused) }
                        .onPreviewKeyEvent { event ->
                            if (shouldSendFromEditor(event, sendShortcutMode) && canSend) {
                                onSend()
                                true
                            } else {
                                false
                            }
                        },
                decorationBox = { field ->
                    Box(Modifier.padding(start = 14.dp, end = 14.dp, top = 12.dp, bottom = 4.dp)) {
                        if (value.isEmpty()) {
                            Text(
                                tr("composer.placeholders.quickMessage"),
                                style = replyTextStyle,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        field()
                    }
                },
            )
            Row(Modifier.fillMaxWidth().padding(start = 2.dp, end = 4.dp, bottom = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onAttach, modifier = Modifier.size(40.dp)) {
                    Icon(
                        Icons.Filled.AttachFile,
                        contentDescription = tr("composer.actions.attachFiles"),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(20.dp),
                    )
                }
                IconButton(onClick = onOpenFullEditor, modifier = Modifier.size(40.dp)) {
                    Icon(
                        Icons.Filled.OpenInFull,
                        contentDescription = tr("composer.actions.openFullEditor"),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(20.dp),
                    )
                }
                // The recipients sit against the send button and are measured
                // first — they change from thread to thread — so the send-as
                // address after the buttons is the one that truncates, down to
                // a floor that keeps its picker reachable.
                val showRecipients = addressChipItems(recipients.to).isNotEmpty()
                val showFrom = selectedFrom != null && fromIdentities.size > 1
                BoxWithConstraints(Modifier.weight(1f)) {
                    val recipientsMaxWidth = if (showFrom) (maxWidth - REPLY_FROM_MIN_WIDTH).coerceAtLeast(0.dp) else maxWidth
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.weight(1f)) {
                            if (showFrom) {
                                ReplyFromRow(
                                    identities = fromIdentities,
                                    selected = selectedFrom,
                                    onSelect = onSelectFrom,
                                )
                            }
                        }
                        if (showRecipients) {
                            ReplyRecipientsRow(
                                recipients = recipients,
                                onOpenFullEditor = onOpenFullEditor,
                                modifier = Modifier.widthIn(max = recipientsMaxWidth),
                            )
                        }
                    }
                }
                FilledIconButton(
                    onClick = onSend,
                    enabled = canSend,
                    modifier = Modifier.padding(start = 4.dp).size(40.dp),
                ) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = tr("reply.send"), modifier = Modifier.size(20.dp))
                }
            }
        }
    }
}

// The least the send-as address keeps beside long recipients: its label and
// the dropdown arrow.
private val REPLY_FROM_MIN_WIDTH = 72.dp

// Who the reply goes to. The bar has no address fields, and a plain reply keeps
// the Cc while dropping the other To recipients — a recipient set worth
// disclosing rather than leaving to be discovered in the Sent copy. Read-only:
// tapping opens the full editor, where the addresses can actually be edited.
@Composable
private fun ReplyRecipientsRow(
    recipients: ReplyRecipients,
    onOpenFullEditor: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val to = addressChipItems(recipients.to)
    if (to.isEmpty()) return
    val cc = addressChipItems(recipients.cc)
    Row(
        modifier
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onOpenFullEditor)
            .padding(horizontal = 6.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Text(
            tr("composer.fields.to"),
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            summarizeRecipients(to),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f, fill = false),
        )
        if (cc.isNotEmpty()) {
            Text(
                tr("composer.fields.cc"),
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                summarizeRecipients(cc),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
    }
}

// How many names the recipient row spells out before collapsing the rest.
private const val VISIBLE_RECIPIENT_NAMES = 2

internal fun summarizeRecipients(items: List<AddressChipItem>): String =
    if (items.size <= VISIBLE_RECIPIENT_NAMES) {
        items.joinToString(", ") { it.display }
    } else {
        val shown = items.take(VISIBLE_RECIPIENT_NAMES).joinToString(", ") { it.display }
        "$shown +${items.size - VISIBLE_RECIPIENT_NAMES}"
    }

// The reply bar's send-as identity: shows which address the reply goes out from,
// and opens a picker to override it. Only rendered when the account has more
// than one identity — with a single address there is nothing to disclose.
@Composable
private fun ReplyFromRow(
    identities: List<SendIdentity>,
    selected: SendIdentity,
    onSelect: (SendIdentity) -> Unit,
    modifier: Modifier = Modifier,
) {
    var expanded by remember { mutableStateOf(false) }
    Box(modifier) {
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
