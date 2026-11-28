package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.HideImage
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.layout.IntrinsicMeasurable
import androidx.compose.ui.layout.IntrinsicMeasureScope
import androidx.compose.ui.layout.LayoutModifier
import androidx.compose.ui.layout.Measurable
import androidx.compose.ui.layout.MeasureResult
import androidx.compose.ui.layout.MeasureScope
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.formatRecipientSummary
import jp.nonbili.meron.shared.htmlHasRemoteMedia
import jp.nonbili.meron.shared.isInlineMedia
import jp.nonbili.meron.shared.standaloneAttachments
import jp.nonbili.meron.shared.visibleImageAttachments
import kotlinx.coroutines.delay
import kotlin.math.roundToInt

/** Bubble inner padding; capped bodies offset their scrollbar back over it. */
private val BubbleHorizontalPadding = 14.dp

/** Bubble inner padding for an HTML body. Mail HTML usually centres itself in a
 *  wrapper with 20-40px of padding of its own, and on a phone the two gutters
 *  together eat most of the bubble, so the bubble keeps only enough of its own
 *  to hold the body off the rounded corners. The chrome around the body pads
 *  back up to [BubbleHorizontalPadding] so it still lines up bubble to bubble. */
internal val HtmlBubbleHorizontalPadding = 6.dp

/** How long a bubble stays invisible waiting for its web view to report a width
 *  before it shows at full width anyway (a page whose script never runs). */
private const val NATURAL_WIDTH_WAIT_MS = 1000L

/** What each HTML body last reported as the width it needs, so a bubble scrolled
 *  back into the list takes its width on the first frame instead of flashing at
 *  full width. [Dp.Unspecified] is not stored: it means not measured yet. */
private val htmlNaturalWidths = HashMap<String, Dp>()

/** Wide screens cap every bubble here so lines stay readable. */
private val MaxBubbleWidth = 560.dp

/** Leaves something out of its parent's intrinsic width, so a hugging bubble
 *  sizes to its text rather than to, say, a photo's pixel width. */
private object NoIntrinsicWidth : LayoutModifier {
    override fun MeasureScope.measure(
        measurable: Measurable,
        constraints: Constraints,
    ): MeasureResult {
        val placeable = measurable.measure(constraints)
        return layout(placeable.width, placeable.height) { placeable.place(0, 0) }
    }

    override fun IntrinsicMeasureScope.minIntrinsicWidth(
        measurable: IntrinsicMeasurable,
        height: Int,
    ): Int = 0

    override fun IntrinsicMeasureScope.maxIntrinsicWidth(
        measurable: IntrinsicMeasurable,
        height: Int,
    ): Int = 0
}

/** Sizes a bubble to [fraction] of the available width, capped at [MaxBubbleWidth].
 *  Narrower when it can hug its content: [hugIntrinsic] takes the content's own
 *  widest line (plain text, which Compose can measure), widened to fit
 *  [imageColumns] grid tiles at the size a full-width bubble gives them; a
 *  measured HTML [natural] width takes that plus [chrome], widened to whatever
 *  the header and the rest of the bubble need (the web view counts for nothing
 *  there). A [natural] of zero or less means the HTML body fills the bubble. */
private fun Modifier.bubbleWidth(
    fraction: Float,
    natural: Dp = 0.dp,
    chrome: Dp = 0.dp,
    hugIntrinsic: Boolean = false,
    imageColumns: Int = 0,
): Modifier =
    layout { measurable, constraints ->
        val cap = minOf((constraints.maxWidth * fraction).roundToInt(), MaxBubbleWidth.roundToPx())
        val width =
            when {
                hugIntrinsic -> {
                    val grid =
                        if (imageColumns > 0) {
                            val gap = AttachmentImageGridGap.roundToPx()
                            val tile = (cap - chrome.roundToPx() - 2 * gap) / 3
                            imageColumns * tile + (imageColumns - 1) * gap + chrome.roundToPx()
                        } else {
                            0
                        }
                    maxOf(measurable.maxIntrinsicWidth(constraints.maxHeight), grid).coerceAtMost(cap)
                }

                natural > 0.dp -> {
                    maxOf((natural + chrome).roundToPx(), measurable.maxIntrinsicWidth(constraints.maxHeight)).coerceAtMost(cap)
                }

                else -> {
                    cap
                }
            }
        val placeable = measurable.measure(constraints.copy(minWidth = width, maxWidth = width))
        layout(placeable.width, placeable.height) { placeable.place(0, 0) }
    }

/** True when the bubble shows the sender's HTML rather than plain text: the
 *  search highlighter works on the plain body, so an open search turns it off. */
internal fun usesHtmlBody(
    message: MessageBody,
    preferHtml: Boolean,
    searchQuery: String,
): Boolean = preferHtml && message.bodyHtml.isNotBlank() && searchQuery.isBlank()

@Composable
internal fun MessageBubble(
    message: MessageBody,
    outgoing: Boolean,
    chat: ChatColors,
    preferHtml: Boolean,
    searchQuery: String,
    activeSearchMatch: Boolean,
    actionsEnabled: Boolean,
    // Read and star work per item on feed threads too, unlike the mail-only
    // actions (forward, edit as new, delete) [actionsEnabled] gates.
    itemActionsEnabled: Boolean,
    showSubject: Boolean,
    isRss: Boolean,
    remoteContent: MessageRemoteContent,
    onForward: (MessageBody) -> Unit,
    onReplyAllToMessage: (MessageBody) -> Unit,
    canReplyAllToMessage: (MessageBody) -> Boolean,
    onEditAsNew: (MessageBody) -> Unit,
    onOpenDraft: (MessageBody) -> Unit,
    onToggleRead: (MessageBody) -> Unit,
    onToggleStarred: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onOpenImageAttachment: (MessageAttachment) -> Unit,
    onOpenHtmlImage: (String) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    onOpenMessage: (MessageBody) -> Unit,
    onOpenUrl: (String) -> Unit,
    onRetryLoad: () -> Unit,
    onRespondToInvite: (MessageBody, String) -> Unit = { _, _ -> },
    onRemoveInviteEvent: (MessageBody) -> Unit = {},
    onAddInviteToCalendar: (MessageBody) -> Unit = {},
) {
    var menuOpen by remember { mutableStateOf(false) }
    var addressesOpen by remember(message.id) { mutableStateOf(false) }
    val bubbleShape =
        if (outgoing) {
            RoundedCornerShape(topStart = 16.dp, topEnd = 4.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
        } else {
            RoundedCornerShape(topStart = 4.dp, topEnd = 16.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
        }
    val themedBubbleColor = if (outgoing) chat.bubbleOut else chat.bubbleIn
    val textColor = if (outgoing) chat.bubbleOutText else chat.bubbleInText
    // Capped, a long body scrolls inside its bubble; uncapped (the full
    // messages setting), it grows to fit and only the conversation scrolls.
    val bodyMaxHeight = if (LocalChatFullMessages.current) Dp.Unspecified else 360.dp
    val visibleTranslations = LocalMessageTranslations.current.takeIf { searchQuery.isBlank() }
    val translated = visibleTranslations?.state(message)?.translatedBody != null
    val htmlBody = !translated && usesHtmlBody(message, preferHtml, searchQuery)
    // The web view paints the mail on white, so a tinted light bubble (Material
    // You) would frame it as a square white box inside a rounded card. Let the
    // whole bubble be white instead, so the header and body read as one card.
    val bubbleColor =
        if (htmlBody && !LocalDarkMailBodies.current && themedBubbleColor.luminance() > 0.5f) {
            Color.White
        } else {
            themedBubbleColor
        }
    val bubblePadding = if (htmlBody) HtmlBubbleHorizontalPadding else BubbleHorizontalPadding
    // Attached pictures lay out against the bubble's width outside the web view,
    // so a bubble carrying them keeps its full width whatever its text needs.
    val hasImages = standaloneAttachments(message).any { it.mimeType.startsWith("image/") }
    val hugsText = htmlBody && !hasImages
    // A plain-text bubble shrinks to its images as well: one or two take only the
    // columns they fill. Counted the way the body decides what it shows.
    val visibleImageCount =
        if (htmlBody) {
            0
        } else {
            visibleImageAttachments(
                standaloneAttachments(message).filter { it.mimeType.startsWith("image/") },
                remoteContent.allowRemote,
            ).size
        }
    val naturalWidthKey = "${message.id}:${message.bodyHtml.hashCode()}:${LocalDensity.current.fontScale}"
    var naturalWidth by remember(naturalWidthKey) { mutableStateOf(htmlNaturalWidths[naturalWidthKey] ?: Dp.Unspecified) }
    // An unmeasured body is laid out at full width but not painted: showing it
    // would flash it wide before it shrinks to its text.
    val widthPending = hugsText && naturalWidth == Dp.Unspecified
    LaunchedEffect(widthPending) {
        if (widthPending) {
            delay(NATURAL_WIDTH_WAIT_MS)
            if (naturalWidth == Dp.Unspecified) naturalWidth = 0.dp
        }
    }
    // What the chrome around an HTML body adds back to sit where it always does.
    val chromeInset = BubbleHorizontalPadding - bubblePadding
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Column(
            Modifier
                // Bubble width tracks the screen: ~85% of available width so it
                // grows on tablets, capped so it stays readable on wide screens.
                // HTML mail is laid out for a wider page than a phone bubble, so
                // it gets the extra tenth (desktop widens its HTML bubbles too).
                .bubbleWidth(
                    fraction = if (htmlBody) 0.95f else 0.85f,
                    natural = if (hugsText && naturalWidth != Dp.Unspecified) naturalWidth else 0.dp,
                    // A dp of slack so a line measured at max-content never wraps on rounding.
                    chrome = bubblePadding * 2 + 1.dp,
                    hugIntrinsic = !htmlBody,
                    imageColumns = minOf(visibleImageCount, 3),
                ).alpha(if (widthPending) 0f else 1f)
                .shadow(3.dp, bubbleShape, clip = false)
                .clip(bubbleShape)
                .then(
                    if (activeSearchMatch) {
                        Modifier.border(2.dp, Color(0xFFFFC107), bubbleShape)
                    } else {
                        Modifier
                    },
                ).background(bubbleColor)
                .padding(start = bubblePadding, end = bubblePadding, top = 8.dp, bottom = 6.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            // Sender, timestamp and the actions menu share one row to keep the
            // bubble compact, matching the desktop reader's header layout.
            Row(
                Modifier.fillMaxWidth().padding(horizontal = chromeInset),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                // Tapping the sender (or, on outgoing bubbles, the recipient
                // summary) expands the full addresses, the way clicking the
                // sender does on desktop.
                // An incoming message names its sender, and the recipients only
                // when a reply-all would reach someone else: without them the
                // reader answers the sender alone without ever noticing the
                // others. A message addressed to us alone stays quiet — the
                // line would only repeat our own name.
                val recipients =
                    if (outgoing || canReplyAllToMessage(message)) {
                        remember(message.to, message.cc) { formatRecipientSummary(message.to, message.cc) }
                    } else {
                        ""
                    }
                // The toggle is on every bubble, even a draft with no recipients
                // yet: the details always lead with From, and a chevron that
                // came and went between messages read as an arbitrary
                // difference between them. A feed item is the exception — it
                // has no recipients at all, so the details could only repeat
                // the feed name the header already shows.
                Row(
                    Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(4.dp))
                        .then(if (isRss) Modifier else Modifier.clickable { addressesOpen = !addressesOpen }),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    if (!outgoing) {
                        Text(
                            message.from.ifBlank { message.fromAddr },
                            modifier = Modifier.weight(1f, fill = false),
                            fontSize = 12.5.sp,
                            fontWeight = FontWeight.Bold,
                            color = MaterialTheme.colorScheme.primary,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        if (recipients.isNotBlank()) {
                            Text(
                                tr("chat.toRecipients", mapOf("recipients" to recipients)),
                                modifier = Modifier.weight(1f, fill = false),
                                fontSize = 11.sp,
                                color = textColor.copy(alpha = 0.6f),
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    } else if (recipients.isNotBlank()) {
                        // An outgoing bubble has no sender to name, and a reply and a
                        // forward of the same text look identical without recipients —
                        // so the slot shows who received it instead.
                        Text(
                            tr("chat.toRecipients", mapOf("recipients" to recipients)),
                            modifier = Modifier.weight(1f, fill = false),
                            fontSize = 11.sp,
                            color = textColor.copy(alpha = 0.6f),
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                    if (!isRss) {
                        Icon(
                            if (addressesOpen) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                            contentDescription = if (addressesOpen) tr("chat.hideDetails") else tr("chat.showDetails"),
                            modifier = Modifier.size(14.dp),
                            tint = textColor.copy(alpha = 0.55f),
                        )
                    }
                }
                if (folderIsDrafts(message.folderId)) {
                    Surface(
                        shape = RoundedCornerShape(4.dp),
                        color = MaterialTheme.colorScheme.errorContainer,
                        modifier = Modifier.padding(end = 4.dp),
                    ) {
                        Text(
                            text = tr("chat.draft"),
                            color = MaterialTheme.colorScheme.onErrorContainer,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.padding(horizontal = 4.dp, vertical = 1.dp),
                        )
                    }
                }
                RemoteContentButton(
                    message = message,
                    remoteContent = remoteContent,
                    preferHtml = preferHtml && !translated,
                    searchQuery = searchQuery,
                )
                Text(
                    formatInboxTimestamp(message.dateEpochSeconds),
                    fontSize = 10.5.sp,
                    color = textColor.copy(alpha = 0.55f),
                )
                val isDraft = folderIsDrafts(message.folderId)
                IconButton(
                    onClick = {
                        if (isDraft) {
                            onOpenDraft(message)
                        } else {
                            onOpenMessage(message)
                        }
                    },
                    modifier = Modifier.size(24.dp),
                ) {
                    Icon(
                        imageVector = if (isDraft) Icons.Filled.Edit else Icons.Filled.OpenInFull,
                        contentDescription = if (isDraft) tr("chat.draft") else tr("threads.actions.openInNewTab"),
                        modifier = Modifier.size(15.dp),
                        tint = textColor.copy(alpha = 0.55f),
                    )
                }
                MessageActionsButton(
                    preferHtml = preferHtml,
                    allowRemote = remoteContent.allowRemote,
                    message = message,
                    translations = visibleTranslations,
                    isRss = isRss,
                    tint = textColor.copy(alpha = 0.55f),
                    actionsEnabled = actionsEnabled,
                    itemActionsEnabled = itemActionsEnabled,
                    onForward = onForward,
                    onReplyAllToMessage = onReplyAllToMessage,
                    canReplyAllToMessage = canReplyAllToMessage,
                    onEditAsNew = onEditAsNew,
                    onToggleRead = onToggleRead,
                    onToggleStarred = onToggleStarred,
                    onDelete = onDelete,
                    onCopyMessageText = onCopyMessageText,
                    onOpenUrl = onOpenUrl,
                )
            }
            if (addressesOpen) {
                MessageAddressDetails(
                    message = message,
                    onCopy = onCopyMessageText,
                    onComposeTo = onComposeTo,
                    textColor = textColor,
                    modifier = Modifier.padding(bottom = 2.dp, start = chromeInset, end = chromeInset),
                )
            }
            MessageBodyContent(
                message = message,
                isRss = isRss,
                textColor = textColor,
                preferHtml = preferHtml,
                searchQuery = searchQuery,
                activeSearchMatch = activeSearchMatch,
                showSubject = showSubject,
                bodyMaxHeight = bodyMaxHeight,
                chromeInset = chromeInset,
                surface = bubbleColor,
                remoteContent = remoteContent,
                onOpenAttachment = onOpenAttachment,
                onSaveAttachment = onSaveAttachment,
                loadImageAttachment = loadImageAttachment,
                onOpenImageAttachment = onOpenImageAttachment,
                onOpenHtmlImage = onOpenHtmlImage,
                onOpenUrl = onOpenUrl,
                onRetryLoad = onRetryLoad,
                onRespondToInvite = { response -> onRespondToInvite(message, response) },
                onRemoveInviteEvent = { onRemoveInviteEvent(message) },
                onAddInviteToCalendar = { onAddInviteToCalendar(message) },
                imageColumns = if (htmlBody) 3 else visibleImageCount.coerceIn(1, 3),
                onHtmlNaturalWidth =
                    if (hugsText) {
                        { width ->
                            naturalWidth = width.coerceAtLeast(0.dp)
                            htmlNaturalWidths[naturalWidthKey] = naturalWidth
                        }
                    } else {
                        null
                    },
            )
        }
    }
}

/** The overflow button shared by both conversation layouts, opening [MessageActionsMenu]. */
@Composable
internal fun MessageActionsButton(
    preferHtml: Boolean,
    allowRemote: Boolean,
    message: MessageBody,
    translations: MessageTranslations?,
    isRss: Boolean,
    tint: Color,
    actionsEnabled: Boolean,
    itemActionsEnabled: Boolean,
    onForward: (MessageBody) -> Unit,
    onReplyAllToMessage: (MessageBody) -> Unit,
    canReplyAllToMessage: (MessageBody) -> Boolean,
    onEditAsNew: (MessageBody) -> Unit,
    onToggleRead: (MessageBody) -> Unit,
    onToggleStarred: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
    onOpenUrl: (String) -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { menuOpen = true }, modifier = Modifier.size(24.dp)) {
            Icon(
                Icons.Filled.MoreVert,
                contentDescription = tr("chat.moreMessageActions"),
                modifier = Modifier.size(16.dp),
                tint = tint,
            )
        }
        MessageActionsMenu(
            expanded = menuOpen,
            onDismiss = { menuOpen = false },
            preferHtml = preferHtml,
            allowRemote = allowRemote,
            message = message,
            translations = translations,
            isRss = isRss,
            actionsEnabled = actionsEnabled,
            onForward = onForward,
            onReplyAllToMessage = onReplyAllToMessage,
            canReplyAllToMessage = canReplyAllToMessage,
            onEditAsNew = onEditAsNew,
            onToggleRead = if (itemActionsEnabled) onToggleRead else null,
            onToggleStarred = if (itemActionsEnabled) onToggleStarred else null,
            onDelete = onDelete,
            onCopyMessageText = onCopyMessageText,
            onOpenUrl = onOpenUrl,
        )
    }
}

/**
 * Everything below a message's header: the optional subject, a calendar
 * invitation's card, the body (HTML or plain), standalone attachments, and the
 * send-status line. MessageBubble (chat) and MessageRow (traditional) each wrap
 * it in their own chrome, so the two layouts differ only in the frame around
 * this.
 */
@Composable
internal fun ColumnScope.MessageBodyContent(
    message: MessageBody,
    isRss: Boolean,
    textColor: Color,
    preferHtml: Boolean,
    searchQuery: String,
    activeSearchMatch: Boolean,
    showSubject: Boolean,
    bodyMaxHeight: Dp,
    // Horizontal inset for everything but the HTML body: the chat bubble trims
    // its own padding for HTML mail (see [HtmlBubbleHorizontalPadding]) and pads
    // the rest of the message back to where it sits in every other bubble.
    chromeInset: Dp = 0.dp,
    // The bubble or row this sits in: what a darkened HTML body blends onto.
    surface: Color,
    // Whether this message's remote content may load, and the two ways the
    // reader can change that (see [MessageRemoteContent]).
    remoteContent: MessageRemoteContent,
    onOpenAttachment: (MessageAttachment) -> Unit,
    onSaveAttachment: (MessageAttachment) -> Unit,
    loadImageAttachment: suspend (MessageAttachment) -> ImageBitmap?,
    onOpenImageAttachment: (MessageAttachment) -> Unit,
    onOpenHtmlImage: (String) -> Unit,
    onOpenUrl: (String) -> Unit,
    onRetryLoad: () -> Unit,
    // Accept / Maybe / Decline on an invitation's card, with the response.
    onRespondToInvite: (String) -> Unit = {},
    // A cancelled invitation's "Remove from calendar".
    onRemoveInviteEvent: () -> Unit = {},
    // An answered invitation's "Add to calendar", once its event is gone.
    onAddInviteToCalendar: () -> Unit = {},
    // HTML body only: the width a plain-text-like document needs (zero when it
    // should fill the bubble). Null where the caller doesn't size to the body.
    onHtmlNaturalWidth: ((Dp) -> Unit)? = null,
    // How many columns the image grid lays out; the bubble passes fewer for one
    // or two images, and sizes itself to them (see bubbleWidth).
    imageColumns: Int = 3,
) {
    MessageCryptoStatus(message)
    if (showSubject && message.subject.isNotBlank()) {
        Text(
            text = highlightedMessageText(message.subject, searchQuery, activeSearchMatch),
            modifier = Modifier.padding(horizontal = chromeInset),
            color = textColor,
            fontSize = 16.sp,
            lineHeight = 21.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
    message.invite?.let { invite ->
        InviteCard(
            invite = invite,
            outgoing = message.outgoing,
            textColor = textColor,
            onRespond = onRespondToInvite,
            onRemoveFromCalendar = onRemoveInviteEvent,
            onAddToCalendar = onAddInviteToCalendar,
            modifier = Modifier.padding(horizontal = chromeInset).padding(bottom = 6.dp),
        )
    }
    if (message.verificationCode.isNotEmpty() && !message.outgoing) {
        VerificationCodeCard(
            code = message.verificationCode,
            textColor = textColor,
            modifier = Modifier.padding(horizontal = chromeInset).padding(bottom = 6.dp),
        )
    }
    val translations = LocalMessageTranslations.current.takeIf { searchQuery.isBlank() }
    val translation = translations?.state(message)
    val translatedBody = translation?.translatedBody
    val htmlBody = translatedBody == null && usesHtmlBody(message, preferHtml, searchQuery)
    if (translation != null) {
        MessageTranslationControls(message, translation, translations, textColor, Modifier.padding(horizontal = chromeInset))
    }
    val standaloneAttachmentsForMessage = standaloneAttachments(message)
    val (imageAttachments, otherAttachments) =
        standaloneAttachmentsForMessage.partition { it.mimeType.startsWith("image/") }
    val visibleImages = visibleImageAttachments(imageAttachments, remoteContent.allowRemote)
    if (htmlBody) {
        // The document reports the width it needs instead (see bubbleWidth); the
        // bubble's intrinsic width is left to the header and what else it holds.
        Box(Modifier.then(NoIntrinsicWidth)) {
            HtmlMessageBody(
                html = message.bodyHtml,
                mediaMissing = message.mediaMissing,
                quoteKey = message.id,
                allowRemote = remoteContent.allowRemote,
                isRss = isRss,
                maxHeight = bodyMaxHeight,
                onOpenUrl = onOpenUrl,
                onOpenImage = onOpenHtmlImage,
                // Opt-in (the auto-fit setting): a bubble fitting a 640px mail
                // renders it at about half size.
                fitWideContent = LocalAutoFitMessages.current,
                onNaturalWidth = onHtmlNaturalWidth ?: {},
                darkSurface = surface,
            )
        }
    } else if (message.bodyMissing) {
        // The core has no cached body — a different state from a genuinely
        // empty message, so not "(no content)". Still downloading in the
        // background, it arrives by itself; only a failed fetch offers a retry.
        Column(Modifier.padding(horizontal = chromeInset)) {
            Text(
                tr(if (message.bodyLoading) "common.loading" else "chat.messageLoadFailed"),
                color = textColor.copy(alpha = 0.6f),
                fontSize = 15.5.sp,
                lineHeight = 21.sp,
            )
            if (!message.bodyLoading) {
                TextButton(onClick = onRetryLoad, modifier = Modifier.align(Alignment.End)) {
                    Text(tr("chat.retry"))
                }
            }
        }
    } else {
        // Subject is the conversation title (top bar); the body shows the
        // message text, matching the desktop chat reader.
        // The quoted tail folds behind a toggle (see QuoteFold), and opens on its
        // own while the in-thread search matches inside it.
        val quoted =
            remember(message.body, message.bodyQuoteStart, message.bodyLinks) {
                splitQuotedBody(message.body, message.bodyQuoteStart, plainBodyLinks(message.body, message.bodyLinks))
            }
        var quoteOpen by remember(message.id) { mutableStateOf(QuoteFoldMemory.isOpen(message.id)) }
        val showQuote = quoteOpen || quoteMatchesSearch(quoted.quote, searchQuery)
        val bodyStyle =
            messageBodyTextStyle(
                MaterialTheme.typography.bodyLarge.copy(
                    fontSize = 15.5.sp,
                    lineHeight = 21.sp,
                ),
            )
        val bodyText: @Composable () -> Unit = {
            Column {
                SelectableMessageText(
                    text = translatedBody ?: quoted.reply.ifBlank { "(no content)" },
                    links =
                        when {
                            translatedBody != null -> plainBodyLinks(translatedBody, null)
                            quoted.reply.isBlank() -> emptyList()
                            else -> quoted.replyLinks
                        },
                    onOpenUrl = onOpenUrl,
                    searchQuery = searchQuery,
                    activeSearchMatch = activeSearchMatch,
                    color = if (message.body.isBlank()) textColor.copy(alpha = 0.6f) else textColor,
                    style = bodyStyle,
                )
                if (quoted.quote.isNotEmpty()) {
                    QuoteToggle(
                        open = showQuote,
                        color = textColor,
                        onToggle = {
                            quoteOpen = !showQuote
                            QuoteFoldMemory.setOpen(message.id, quoteOpen)
                        },
                    )
                    if (showQuote) {
                        SelectableMessageText(
                            text = quoted.quote,
                            links = quoted.quoteLinks,
                            onOpenUrl = onOpenUrl,
                            searchQuery = searchQuery,
                            activeSearchMatch = activeSearchMatch,
                            color = textColor,
                            style = bodyStyle,
                        )
                    }
                }
            }
        }
        if (bodyMaxHeight == Dp.Unspecified) {
            // Uncapped (the traditional layout, or full chat messages): the
            // message is as tall as it needs to be and the conversation list
            // scrolls it. A nested scroller here would be measured with an
            // infinite height by the lazy list and throw.
            Box(Modifier.padding(horizontal = chromeInset)) { bodyText() }
        } else {
            val bodyScrollState = rememberScrollState()
            Box(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = chromeInset)
                    .heightIn(max = bodyMaxHeight)
                    .appScrollbar(
                        bodyScrollState,
                        color = textColor.copy(alpha = 0.4f),
                        endOffset = BubbleHorizontalPadding,
                    ).verticalScroll(bodyScrollState),
            ) {
                bodyText()
            }
        }
    }
    if (visibleImages.isNotEmpty() || otherAttachments.isNotEmpty()) {
        Column(
            Modifier.padding(horizontal = chromeInset),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            if (visibleImages.isNotEmpty()) {
                AttachmentImageGrid(
                    images = visibleImages,
                    mediaMissing = message.mediaMissing,
                    loadImageAttachment = loadImageAttachment,
                    onOpen = onOpenImageAttachment,
                    columns = imageColumns,
                    // The tiles take the width they are given, and a picture's
                    // own size would otherwise count as the bubble's content.
                    modifier = Modifier.then(NoIntrinsicWidth),
                )
            }
            otherAttachments.forEach { attachment ->
                AttachmentRow(
                    attachment = attachment,
                    textColor = textColor,
                    onOpen = { onOpenAttachment(attachment) },
                    onSave = { onSaveAttachment(attachment) },
                )
            }
        }
    }
    // Send lifecycle for an optimistically inserted reply: shown until the
    // canonical sent message replaces it on re-fetch (which clears the
    // status). On failure the bubble stays visible so the reply isn't lost.
    when (message.sendStatus) {
        SendStatus.Sending -> {
            Text(
                tr("composer.status.sending"),
                modifier = Modifier.align(Alignment.End).padding(horizontal = chromeInset),
                fontSize = 10.5.sp,
                color = textColor.copy(alpha = 0.55f),
            )
        }

        SendStatus.Failed -> {
            Text(
                tr("mobile.status.failedToSend"),
                modifier = Modifier.align(Alignment.End).padding(horizontal = chromeInset),
                fontSize = 10.5.sp,
                color = MaterialTheme.colorScheme.error,
            )
        }

        SendStatus.None -> {
            Unit
        }
    }
}

@Composable
internal fun RemoteContentButton(
    message: MessageBody,
    remoteContent: MessageRemoteContent,
    preferHtml: Boolean,
    searchQuery: String,
    modifier: Modifier = Modifier,
    iconSize: Dp = 15.dp,
) {
    if (message.crypto?.encrypted == true) return
    val canLoadRemote = !remoteContent.allowRemote
    val showsHtml = usesHtmlBody(message, preferHtml, searchQuery)
    val hiddenImageCount =
        if (canLoadRemote) {
            standaloneAttachments(message).count { it.mimeType.startsWith("image/") && !isInlineMedia(it) }
        } else {
            0
        }
    val blocked = canLoadRemote && (hiddenImageCount > 0 || (showsHtml && htmlHasRemoteMedia(message.bodyHtml)))
    val canReveal = canLoadRemote && (hiddenImageCount > 0 || showsHtml)
    val hasSender = remoteContent.senderAddress.isNotEmpty()
    if (!canReveal && !hasSender) return

    val senderAllowed = remoteContent.senderAllowed
    val enabled = canReveal || !senderAllowed
    var menuOpen by remember { mutableStateOf(false) }
    Box(modifier) {
        IconButton(onClick = { menuOpen = true }, enabled = enabled, modifier = Modifier.size(24.dp)) {
            Icon(
                when {
                    blocked -> Icons.Filled.HideImage
                    senderAllowed -> Icons.Filled.Check
                    else -> Icons.Filled.Image
                },
                contentDescription =
                    tr(
                        when {
                            blocked -> "chat.remoteBlocked"
                            senderAllowed -> "chat.remoteSenderAllowed"
                            else -> "settings.privacy.remoteSenders"
                        },
                    ),
                modifier = Modifier.size(iconSize),
                tint =
                    if (enabled) {
                        MaterialTheme.colorScheme.primary.copy(alpha = 0.65f)
                    } else {
                        LocalContentColor.current
                    },
            )
        }
        DropdownMenu(expanded = menuOpen && enabled, onDismissRequest = { menuOpen = false }) {
            if (canReveal) {
                DropdownMenuItem(
                    text = {
                        Text(
                            if (hiddenImageCount > 0) {
                                tr("chat.showImages", mapOf("count" to hiddenImageCount))
                            } else {
                                tr("chat.showRemoteContent")
                            },
                        )
                    },
                    onClick = {
                        menuOpen = false
                        remoteContent.onReveal()
                    },
                )
            }
            if (hasSender) {
                DropdownMenuItem(
                    enabled = !senderAllowed,
                    text = {
                        Text(
                            if (senderAllowed) {
                                tr("chat.remoteSenderAllowed")
                            } else {
                                tr("chat.allowRemoteFrom", mapOf("sender" to remoteContent.senderAddress))
                            },
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    },
                    onClick = {
                        menuOpen = false
                        remoteContent.onAllowSender()
                    },
                )
            }
        }
    }
}

// Compose Constraints packs sizes into bit fields and cannot represent
// dimensions past ~262k px; sizing the WebView to an unclamped page height
// (a very tall newsletter, or a bogus negative report from the JS bridge)
// makes measurement throw. 20000dp stays well under the limit at any density.
internal val MailBodyMaxReportedHeight = 20_000.dp

internal fun clampMailBodyHeight(reported: Dp): Dp = reported.coerceIn(0.dp, MailBodyMaxReportedHeight)
