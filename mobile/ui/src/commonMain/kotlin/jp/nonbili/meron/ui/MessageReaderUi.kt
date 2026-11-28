package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Forward
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.ReplyAll
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.rememberHazeState
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.standaloneAttachments
import jp.nonbili.meron.shared.visibleImageAttachments

// Full-screen reader for a single message — the mobile equivalent of the desktop
// "open in new tab" reader, showing the full header plus the message body.
@OptIn(ExperimentalMaterial3Api::class, ExperimentalComposeUiApi::class)
@Composable
internal fun MessageReaderScreen(
    message: MessageBody,
    preferHtml: Boolean,
    isRss: Boolean,
    actionsEnabled: Boolean,
    remoteContent: MessageRemoteContent,
    onBack: () -> Unit,
    onCopy: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    onForward: (MessageBody) -> Unit,
    onReplyToMessage: (MessageBody) -> Unit,
    onReplyAllToMessage: (MessageBody) -> Unit,
    canReplyAllToMessage: (MessageBody) -> Boolean,
    onEditAsNew: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onOpenImageAttachment: (MessageAttachment) -> Unit,
    onOpenHtmlImage: (String) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    var menuOpen by remember(message.id) { mutableStateOf(false) }
    val translations = LocalMessageTranslations.current
    val translation = translations?.state(message)
    val translatedBody = translation?.translatedBody
    val scrollState = rememberScrollState()
    val density = LocalDensity.current
    val pull = rememberPullToDismissState(message.id)
    // The message, captured for the frosted bands under the floating bars.
    val hazeState = rememberHazeState()
    var bottomCardHeightPx by remember { mutableIntStateOf(0) }

    BackHandler(onBack = onBack)

    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .pullToDismissLayer(pull)
                .pullToDismiss(
                    pull,
                    isAtTop = { !scrollState.canScrollBackward },
                    isAtBottom = if (LocalPullUpToClose.current) ({ !scrollState.canScrollForward }) else null,
                    onDismiss = onBack,
                ).pointerInput(Unit) {
                    detectTapGestures { }
                },
    ) {
        Scaffold(
            topBar = {
                TopAppBar(
                    modifier = Modifier.pullToDismissHandle(pull, onDismiss = onBack),
                    // The subject lives in the content area below, so the bar
                    // stays empty and gives the body the full width.
                    title = {},
                    navigationIcon = {
                        FloatingBarChip(CircleShape, Modifier.padding(start = 4.dp).width(FLOATING_BAR_CHIP_HEIGHT)) {
                            IconButton(onClick = onBack) {
                                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                            }
                        }
                    },
                    actions = {
                        FloatingBarChip(CircleShape, Modifier.padding(end = 4.dp).width(FLOATING_BAR_CHIP_HEIGHT)) {
                            IconButton(onClick = { menuOpen = true }) {
                                Icon(Icons.Filled.MoreVert, contentDescription = tr("chat.moreMessageActions"))
                            }
                            MessageActionsMenu(
                                expanded = menuOpen,
                                onDismiss = { menuOpen = false },
                                preferHtml = preferHtml,
                                allowRemote = remoteContent.allowRemote,
                                message = message,
                                isRss = isRss,
                                actionsEnabled = actionsEnabled,
                                onForward = onForward,
                                onReplyAllToMessage = onReplyAllToMessage,
                                canReplyAllToMessage = canReplyAllToMessage,
                                onEditAsNew = onEditAsNew,
                                onToggleRead = null,
                                onToggleStarred = null,
                                onDelete = onDelete,
                                onCopyMessageText = onCopy,
                                onOpenUrl = onOpenUrl,
                                // Reply, reply all, forward and delete are in the bottom bar.
                                mailActionsInBar = true,
                                translations = translations,
                            )
                        }
                    },
                    // See-through, like the thread's: the message scrolls on
                    // under the bar, between its floating buttons.
                    colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent, scrolledContainerColor = Color.Transparent),
                )
            },
            bottomBar = {
                if (actionsEnabled) {
                    MessageActionsCard(
                        message = message,
                        canReplyAll = canReplyAllToMessage(message),
                        canForward = !message.bodyMissing,
                        onReply = { onReplyToMessage(message) },
                        onReplyAll = { onReplyAllToMessage(message) },
                        onForward = { onForward(message) },
                        onDelete = { onDelete(message) },
                        modifier =
                            Modifier
                                .navigationBarsPadding()
                                .padding(start = 12.dp, end = 12.dp, bottom = 12.dp)
                                .onSizeChanged { bottomCardHeightPx = it.height },
                    )
                }
            },
        ) { innerPadding ->
            Box(Modifier.fillMaxSize()) {
                val htmlBody = translatedBody == null && preferHtml && message.bodyHtml.isNotBlank()
                // The web view paints mail on white, so a tinted card would frame it
                // as a square white box; a light card goes white to hold it cleanly.
                val surface = MaterialTheme.colorScheme.surface
                val cardColor =
                    if (htmlBody && !LocalDarkMailBodies.current && surface.luminance() > 0.5f) Color.White else surface
                val cardShape = RoundedCornerShape(16.dp)
                val layoutDirection = LocalLayoutDirection.current
                // Runs on under both bars; the scrolled content starts and ends
                // clear of them.
                Column(
                    Modifier
                        .fillMaxSize()
                        .padding(
                            PaddingValues(
                                start = innerPadding.calculateStartPadding(layoutDirection),
                                end = innerPadding.calculateEndPadding(layoutDirection),
                            ),
                        ).background(MaterialTheme.colorScheme.background)
                        .readerHazeSource(hazeState)
                        .appScrollbar(scrollState)
                        .verticalScroll(scrollState)
                        .padding(
                            start = 12.dp,
                            end = 12.dp,
                            top = innerPadding.calculateTopPadding() + 16.dp,
                            bottom = innerPadding.calculateBottomPadding() + 16.dp,
                        ),
                    verticalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Text(
                        message.subject.ifBlank { tr("threads.noSubject") },
                        modifier = Modifier.padding(horizontal = 4.dp),
                        fontWeight = FontWeight.SemiBold,
                        style = MaterialTheme.typography.headlineSmall,
                    )
                    Column(
                        Modifier
                            .fillMaxWidth()
                            .shadow(3.dp, cardShape, clip = false)
                            .clip(cardShape)
                            .background(cardColor)
                            .padding(horizontal = 14.dp, vertical = 12.dp),
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            MessageCryptoStatus(message)
                            // The From chip already names the sender, so there is no
                            // separate sender line. Tapping any chip copies the full
                            // `Name <addr>`.
                            MessageAddressDetails(
                                message = message,
                                onCopy = onCopy,
                                onComposeTo = onComposeTo,
                                textColor = MaterialTheme.colorScheme.onSurfaceVariant,
                                labelWidth = 40.dp,
                            )
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text(
                                    formatMessageFullTimestamp(message.dateEpochSeconds),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                                RemoteContentButton(
                                    message = message,
                                    remoteContent = remoteContent,
                                    preferHtml = htmlBody,
                                    searchQuery = "",
                                )
                            }
                        }
                        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                        val standaloneAttachmentsForMessage = standaloneAttachments(message)
                        val (imageAttachments, otherAttachments) =
                            standaloneAttachmentsForMessage.partition { it.mimeType.startsWith("image/") }
                        val visibleImages = visibleImageAttachments(imageAttachments, remoteContent.allowRemote)
                        if (translation != null) {
                            MessageTranslationControls(message, translation, translations, MaterialTheme.colorScheme.onSurface)
                        }
                        if (translatedBody != null) {
                            val style = messageBodyTextStyle(MaterialTheme.typography.bodyLarge)
                            val quoted =
                                remember(message.body, message.bodyQuoteStart, message.bodyLinks) {
                                    splitQuotedBody(message.body, message.bodyQuoteStart, plainBodyLinks(message.body, message.bodyLinks))
                                }
                            SelectableMessageText(text = translatedBody, links = emptyList(), onOpenUrl = onOpenUrl, style = style)
                            if (quoted.quote.isNotEmpty()) {
                                var quoteOpen by remember(message.id) { mutableStateOf(QuoteFoldMemory.isOpen(message.id)) }
                                QuoteToggle(
                                    open = quoteOpen,
                                    color = MaterialTheme.colorScheme.onSurface,
                                    onToggle = {
                                        quoteOpen = !quoteOpen
                                        QuoteFoldMemory.setOpen(message.id, quoteOpen)
                                    },
                                )
                                if (quoteOpen) {
                                    SelectableMessageText(text = quoted.quote, links = quoted.quoteLinks, onOpenUrl = onOpenUrl, style = style)
                                }
                            }
                        } else if (htmlBody) {
                            // Only the reader shrinks over-wide mail to fit: it has the
                            // full screen to scale into, where a bubble would render the
                            // same mail as an unreadable thumbnail.
                            HtmlMessageBody(
                                html = message.bodyHtml,
                                mediaMissing = message.mediaMissing,
                                quoteKey = message.id,
                                allowRemote = remoteContent.allowRemote,
                                isRss = isRss,
                                onOpenUrl = onOpenUrl,
                                onOpenImage = onOpenHtmlImage,
                                fitWideContent = true,
                                darkSurface = cardColor,
                            )
                        } else {
                            SelectableMessageText(
                                text =
                                    message.body.ifBlank {
                                        when {
                                            message.bodyLoading -> tr("common.loading")
                                            message.bodyMissing -> tr("chat.messageLoadFailed")
                                            else -> "(no content)"
                                        }
                                    },
                                links =
                                    if (message.body.isBlank()) {
                                        emptyList()
                                    } else {
                                        plainBodyLinks(message.body, message.bodyLinks)
                                    },
                                onOpenUrl = onOpenUrl,
                                style = messageBodyTextStyle(MaterialTheme.typography.bodyLarge),
                            )
                        }
                        if (visibleImages.isNotEmpty() || otherAttachments.isNotEmpty()) {
                            HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                            if (visibleImages.isNotEmpty()) {
                                AttachmentImageGrid(
                                    images = visibleImages,
                                    mediaMissing = message.mediaMissing,
                                    loadImageAttachment = loadImageAttachment,
                                    onOpen = onOpenImageAttachment,
                                )
                            }
                            otherAttachments.forEach { attachment ->
                                AttachmentRow(
                                    attachment = attachment,
                                    textColor = MaterialTheme.colorScheme.onSurface,
                                    onOpen = { onOpenAttachment(attachment) },
                                    onSave = { onSaveAttachment(attachment) },
                                )
                            }
                        }
                    }
                }
                // Frosted under the status bar and the app bar, and around the
                // floating actions down through the navigation bar: from the
                // middle of the card, which covers where the band begins.
                val fadePx = with(density) { 16.dp.toPx() }
                val bottomCardPx = if (actionsEnabled) bottomCardHeightPx else 0
                FrostedBand(
                    hazeState,
                    heightPx = with(density) { innerPadding.calculateTopPadding().roundToPx() },
                    fadeAtBottomPx = fadePx,
                    // A little heavier: the status bar's icons sit on it.
                    tintAlpha = 0.4f,
                    modifier = Modifier.align(Alignment.TopCenter),
                )
                FrostedBand(
                    hazeState,
                    heightPx = with(density) { innerPadding.calculateBottomPadding().roundToPx() } - bottomCardPx / 2,
                    fadeAtTopPx = if (bottomCardPx > 0) 0f else fadePx,
                    modifier = Modifier.align(Alignment.BottomCenter),
                )
            }
        }
    }
}

/**
 * Reply, reply all, forward and delete as one floating card: at the foot of the
 * message reader, and under a conversation when the conversation-footer setting
 * puts them there instead of the reply bar. Without a [message] (nothing to
 * answer yet) only delete shows.
 */
@Composable
internal fun MessageActionsCard(
    message: MessageBody?,
    canReplyAll: Boolean,
    // Forwarding builds on the body and its attachments, so it waits for them.
    canForward: Boolean,
    onReply: () -> Unit,
    onReplyAll: () -> Unit,
    onForward: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier = Modifier,
    deleteLabel: String = tr("buttons.delete"),
) {
    FloatingCard(RoundedCornerShape(24.dp), modifier) {
        Row(Modifier.fillMaxWidth().padding(4.dp)) {
            if (message != null) {
                ReaderBottomAction(Icons.AutoMirrored.Filled.Reply, tr("chat.actions.reply"), onClick = onReply)
                if (canReplyAll) {
                    ReaderBottomAction(Icons.AutoMirrored.Filled.ReplyAll, tr("chat.actions.replyAll"), onClick = onReplyAll)
                }
                ReaderBottomAction(Icons.AutoMirrored.Filled.Forward, tr("chat.actions.forward"), enabled = canForward, onClick = onForward)
            }
            ReaderBottomAction(Icons.Filled.Delete, deleteLabel, tint = MaterialTheme.colorScheme.error, onClick = onDelete)
        }
    }
}

// Each action takes an equal share of the card, its label wrapping rather than
// clipping, so four of them (with Reply all) still fit a narrow phone, large
// text, or a long translation.
@Composable
private fun RowScope.ReaderBottomAction(
    icon: ImageVector,
    label: String,
    tint: Color = LocalContentColor.current,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    Column(
        Modifier
            .weight(1f)
            .clip(RoundedCornerShape(12.dp))
            .clickable(enabled = enabled, onClick = onClick)
            .alpha(if (enabled) 1f else 0.4f)
            .padding(horizontal = 4.dp, vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, contentDescription = null, tint = tint)
        Text(label, style = MaterialTheme.typography.labelSmall, color = tint, maxLines = 2, textAlign = TextAlign.Center)
    }
}
