package jp.nonbili.meron.ui

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.RemoteContentPolicy
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.ExperimentalEncodingApi

/** The open conversation, with the ones beside it in its list a page to either side. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun OpenThreadPager(
    state: MeronMobileState,
    threadOpen: Boolean,
    popAppBack: () -> Unit,
    pickAttachmentInto: ((DraftAttachment) -> Unit) -> Unit,
    selectedThreadAccountId: String,
    pull: PullToDismissState,
) {
    with(state) {
        BackHandler(enabled = threadOpen, onBack = popAppBack)
        // Without a summary there is nothing to show and nothing to
        // load, so leave rather than sit on an endless spinner (the
        // thread was moved away, or the route was restored without it).
        LaunchedEffect(selectedCoreThread == null) {
            if (selectedCoreThread == null && threadOpen) popAppBack()
        }
        val threadSwipe = rememberThreadSwipeState()
        // Pages beside the open one never show a snackbar; theirs goes nowhere.
        val previewSnackbarHost = remember { SnackbarHostState() }
        val (newerThread, olderThread) = threadBrowseNeighbors()
        LaunchedEffect(newerThread?.id, newerThread?.dateEpochSeconds, olderThread?.id, olderThread?.dateEpochSeconds) {
            prefetchThreadPreviews(listOfNotNull(newerThread, olderThread))
        }
        // Swiping away would drop what is typed in the reply bar, which
        // only saves once its debounce runs.
        val swipeable = quickReplyIsBlank()
        val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
        // The open conversation, with the ones beside it in its list a
        // page to either side: newer towards the start, older towards
        // the end. Keyed by thread, so the page swiped over to keeps its
        // composition (scroll, loaded bodies) as it becomes the open one.
        val pages =
            listOfNotNull(
                newerThread?.let { it to if (rtl) 1 else -1 },
                selectedCoreThread to 0,
                olderThread?.let { it to if (rtl) -1 else 1 },
            )
        Box(
            Modifier
                .fillMaxSize()
                // Covers the screen underneath to its touches too.
                .pointerInput(Unit) { detectTapGestures { } }
                .pullToDismissLayer(pull)
                .onSizeChanged { threadSwipe.pageWidthPx = it.width.toFloat() },
        ) {
            for ((pageThread, slot) in pages) {
                key(pageThread?.id) {
                    MessageTranslationScope {
                        val active = slot == 0
                        val pageAccount = pageThread?.accountId?.let { accountId -> coreAccounts.firstOrNull { it.id == accountId } }
                        val pagePreferHtml = pageAccount?.let { conversationHtmlOverrides[it.id] ?: it.conversationHtml } ?: true
                        val preview = pageThread?.let { threadPreviews[it.id]?.page }
                        val pageMessages = if (active) visibleThreadMessages() else preview?.messages.orEmpty()
                        val pageCursor = if (active) messageCursor else preview?.nextCursor.orEmpty()
                        ThreadScreen(
                            active = active,
                            modifier = Modifier.graphicsLayer { translationX = threadSwipe.offsetPx + slot * size.width },
                            loadPrintMessages = ::loadThreadForPrinting,
                            onPrintError = { status = it },
                            thread = pageThread,
                            messages = pageMessages,
                            accountEmail = pageAccount?.email.orEmpty(),
                            wallpaperPresetId = pageAccount?.chatWallpaperPresetId.orEmpty(),
                            wallpaperCustomUrl =
                                pageAccount
                                    ?.takeIf { it.chatWallpaperKind == "custom" }
                                    ?.chatWallpaperUrl
                                    .orEmpty(),
                            preferHtml = pagePreferHtml,
                            onPreferHtmlChange = { preferHtml ->
                                if (selectedThreadAccountId.isNotBlank()) {
                                    conversationHtmlOverrides = conversationHtmlOverrides + (selectedThreadAccountId to preferHtml)
                                }
                            },
                            onBack = popAppBack,
                            onArchive = {
                                selectedCoreThread?.let {
                                    archiveOrRemove(it)
                                    popAppBack()
                                }
                            },
                            onMarkAsSpam =
                                pageThread
                                    ?.takeIf { active && spamFolderForThread(it) != null }
                                    ?.let { thread -> { markThreadAsSpam(thread, onMoved = popAppBack) } },
                            isRssArticle = pageThread?.let { threadIdIsRss(it.id) && it.rssItemKeys().isNotEmpty() } == true,
                            // A feed's own page, not one of its entries opened on its own.
                            onRenameFeed =
                                pageThread
                                    ?.takeIf { active && threadIdIsRss(it.id) && it.rssItemKeys().isEmpty() }
                                    ?.let { feed -> { name -> renameFeed(feed, name) } },
                            onDelete = {
                                selectedCoreThread?.let {
                                    deleteThread(it)
                                    popAppBack()
                                }
                            },
                            onToggleStar = {
                                selectedCoreThread?.let { t ->
                                    toggleStar(t)
                                    selectedCoreThread = t.copy(starred = !t.starred)
                                }
                            },
                            toolbar = conversationToolbar,
                            onTags =
                                selectedCoreThread
                                    ?.takeIf { !threadIdIsRss(it.id) }
                                    ?.let { thread -> { selectedTagThread = thread } },
                            onAddToTasks =
                                selectedCoreThread
                                    ?.takeIf { tasksEnabled && !threadIdIsRss(it.id) }
                                    ?.let { thread ->
                                        {
                                            scope.launch {
                                                requestAddMailToTasks(thread)
                                            }
                                            Unit
                                        }
                                    },
                            moveFolders =
                                coreAccounts
                                    .filterNot { accountSummaryIsRss(it) }
                                    .flatMap { account -> foldersByAccount[account.id].orEmpty() },
                            transferAccounts = coreAccounts.filterNot { accountSummaryIsRss(it) },
                            loadTransferFolders = {
                                selectedCoreThread?.let { thread -> ensureThreadActionFolders(thread, includeAllMailAccounts = true) {} }
                            },
                            copyFolders =
                                coreAccounts
                                    .filterNot { accountSummaryIsRss(it) }
                                    .flatMap { account -> foldersByAccount[account.id].orEmpty() },
                            onMoveToFolder = { folder ->
                                selectedCoreThread?.let { thread ->
                                    moveThreadToFolder(thread, folder.name, folder.accountId.ifBlank { thread.accountId }) {
                                        popAppBack()
                                    }
                                }
                            },
                            recentTransfers = if (active) recentTransferItems(selectedCoreThread) else emptyList(),
                            loadRecentTransferFolders = { loadRecentTransferFolders(selectedCoreThread) },
                            onCreateFolderAndMove = { name ->
                                selectedCoreThread?.let { thread ->
                                    createFolderAndMoveThread(thread, name) {
                                        popAppBack()
                                    }
                                }
                            },
                            onCopyToFolder = { folder ->
                                selectedCoreThread?.let { thread ->
                                    copyThreadToFolder(thread, folder)
                                }
                            },
                            quickReplyBody = if (active) quickReplyBody else pageThread?.let(::seededQuickReplyBody).orEmpty(),
                            canLoadOlder = pageCursor.isNotBlank(),
                            loadingOlder = active && loadingMoreMessages,
                            onLoadOlder = ::loadMoreThreadMessages,
                            onQuickReplyChange = ::onQuickReplyBodyChange,
                            quickReplyAttachments = if (active) quickReplyAttachments else emptyList(),
                            quickReplyFailure = if (active) quickReplyFailure else "",
                            quickReplyHasContent = active && !quickReplyIsBlank(),
                            quickReplySending = active && quickReplySendInFlight,
                            sendShortcutMode = sendShortcutMode,
                            conversationLayout = conversationLayout,
                            onQuickReplyAttach = {
                                pickAttachmentInto { picked ->
                                    quickReplyAttachments = quickReplyAttachments + picked
                                    ++quickReplyGeneration
                                    quickReplyFailure = ""
                                    autoSaveQuickReplyDraft()
                                }
                            },
                            onRemoveQuickReplyAttachment = { attachment ->
                                quickReplyAttachments = quickReplyAttachments.filterNot { it.id == attachment.id }
                                ++quickReplyGeneration
                                quickReplyFailure = ""
                                if (quickReplyIsBlank()) {
                                    discardQuickReplyDraftIfEmpty()
                                } else {
                                    autoSaveQuickReplyDraft()
                                }
                            },
                            onOpenFullReply = { openQuickReplyInFullEditor() },
                            onReplyAll = { openQuickReplyInFullEditor(replyAll = true) },
                            canReplyAllToThread = canReplyAllToThread(pageThread, pageMessages),
                            onSendReply = ::sendQuickReply,
                            onRetryReply = ::retryQuickReplySend,
                            quickReplyFromIdentities = quickReplyIdentities(pageThread),
                            quickReplyRecipients = quickReplyRecipients(pageThread, pageMessages),
                            quickReplySelectedFrom = selectedQuickReplyIdentity(pageThread, pageMessages),
                            onSelectQuickReplyFrom = { identity ->
                                quickReplyFrom = identity.email
                                ++quickReplyGeneration
                                // The saved draft carries the From header, so re-save it
                                // against the newly chosen identity instead of waiting
                                // for the next keystroke's debounce.
                                if (quickReplyDraftSaved) autoSaveQuickReplyDraft()
                            },
                            onForward = { openMessageCompose(it, forward = true) },
                            onReplyToMessage = { replyToMessage(it, replyAll = false) },
                            onReplyAllToMessage = ::replyAllToMessage,
                            canReplyAllToMessage = ::canReplyAllToMessage,
                            onEditAsNew = { openMessageCompose(it, forward = false) },
                            conversationFooter = if (active) effectiveConversationFooter() else conversationFooter,
                            conversationReplyTarget = conversationActionTarget(pageThread, pageMessages),
                            onOpenDraft = { message ->
                                selectedCoreThread?.let { thread ->
                                    openDraftCompose(message, thread, returnScreen = Screen.Thread)
                                }
                            },
                            onToggleMessageRead = ::toggleMessageRead,
                            onToggleMessageStarred = ::toggleMessageStarred,
                            onDeleteMessage = { message -> deleteMessage(message, onThreadEmptied = popAppBack) },
                            onOpenAttachment = ::openMessageAttachment,
                            onSaveAttachment = ::saveMessageAttachment,
                            onShareImageAttachment = ::shareImageAttachment,
                            onCopyImageAttachment = ::copyImageAttachment,
                            loadImageAttachment = { attachment ->
                                val cacheKey = attachment.key.ifBlank { attachment.url }
                                loadCachedImageBitmap(cacheKey) {
                                    if (attachment.key.isNotBlank()) {
                                        withContext(ioDispatcher) { decodeImageBitmap(readAttachmentBytes(attachment)) }
                                    } else {
                                        loadImageBitmapRef(attachment.url)
                                    }
                                }
                            },
                            remoteContentPolicy =
                                RemoteContentPolicy(
                                    // Feed accounts load remote content by definition —
                                    // a feed item is little else — which is what the
                                    // core defaults them to (see `load_remote_images`).
                                    accountAllows = pageAccount?.loadRemoteImages ?: false,
                                    allowedSenders = remoteImageSenders,
                                ),
                            onAllowRemoteSender = { addr -> setRemoteImageSender(addr, true) },
                            onComposeTo = { email ->
                                openComposeTo(email, selectedCoreThread?.accountId ?: selectedCoreAccountId)
                            },
                            onCopyMessageText = { label, value ->
                                services.copyText(label, value)
                                status = trs("common.copied")
                            },
                            onRetryLoadMessages = { retryOpenThreadLoad() },
                            onRespondToInvite = { message, response -> scope.launch { respondToInvite(message, response) } },
                            onRemoveInviteEvent = { message -> scope.launch { removeInviteFromCalendar(message) } },
                            onAddInviteToCalendar = { message -> scope.launch { addInviteToCalendar(message) } },
                            onMessagesRead = ::markMessagesReadOnScroll,
                            onViewedToBottom = ::markThreadReadOnScroll,
                            snackbarHost = if (active) snackbarHost else previewSnackbarHost,
                            swipe = threadSwipe,
                            pull = pull,
                            onSwipeToPrevious = newerThread?.takeIf { swipeable }?.let { thread -> { browseToThread(thread) } },
                            onSwipeToNext = olderThread?.takeIf { swipeable }?.let { thread -> { browseToThread(thread) } },
                        )
                    }
                }
            }
        }
    }
}
