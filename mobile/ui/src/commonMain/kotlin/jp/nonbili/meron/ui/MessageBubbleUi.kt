package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.HideImage
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.applyRemoteContentPolicy
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.formatRecipientSummary
import jp.nonbili.meron.shared.htmlHasRemoteMedia
import jp.nonbili.meron.shared.mailBodyCsp
import jp.nonbili.meron.shared.standaloneAttachments
import jp.nonbili.meron.shared.visibleImageAttachments
import kotlin.random.Random

/** Bubble inner padding; capped bodies offset their scrollbar back over it. */
private val BubbleHorizontalPadding = 14.dp

/** Bubble inner padding for an HTML body. Mail HTML usually centres itself in a
 *  wrapper with 20-40px of padding of its own, and on a phone the two gutters
 *  together eat most of the bubble, so the bubble keeps only enough of its own
 *  to hold the body off the rounded corners. The chrome around the body pads
 *  back up to [BubbleHorizontalPadding] so it still lines up bubble to bubble. */
private val HtmlBubbleHorizontalPadding = 6.dp

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
) {
    var menuOpen by remember { mutableStateOf(false) }
    var addressesOpen by remember(message.id) { mutableStateOf(false) }
    val bubbleShape =
        if (outgoing) {
            RoundedCornerShape(topStart = 16.dp, topEnd = 4.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
        } else {
            RoundedCornerShape(topStart = 4.dp, topEnd = 16.dp, bottomEnd = 16.dp, bottomStart = 16.dp)
        }
    val bubbleColor = if (outgoing) chat.bubbleOut else chat.bubbleIn
    val textColor = if (outgoing) chat.bubbleOutText else chat.bubbleInText
    val bodyMaxHeight = 360.dp
    val htmlBody = usesHtmlBody(message, preferHtml, searchQuery)
    val bubblePadding = if (htmlBody) HtmlBubbleHorizontalPadding else BubbleHorizontalPadding
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
                .fillMaxWidth(if (htmlBody) 0.95f else 0.85f)
                .widthIn(max = 560.dp)
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
                val recipients =
                    if (outgoing) {
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
                    message = message,
                    tint = textColor.copy(alpha = 0.55f),
                    actionsEnabled = actionsEnabled,
                    itemActionsEnabled = itemActionsEnabled,
                    onForward = onForward,
                    onEditAsNew = onEditAsNew,
                    onToggleRead = onToggleRead,
                    onToggleStarred = onToggleStarred,
                    onDelete = onDelete,
                    onCopyMessageText = onCopyMessageText,
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
                textColor = textColor,
                preferHtml = preferHtml,
                searchQuery = searchQuery,
                activeSearchMatch = activeSearchMatch,
                showSubject = showSubject,
                bodyMaxHeight = bodyMaxHeight,
                chromeInset = chromeInset,
                remoteContent = remoteContent,
                onOpenAttachment = onOpenAttachment,
                onSaveAttachment = onSaveAttachment,
                loadImageAttachment = loadImageAttachment,
                onOpenImageAttachment = onOpenImageAttachment,
                onOpenHtmlImage = onOpenHtmlImage,
                onOpenUrl = onOpenUrl,
                onRetryLoad = onRetryLoad,
            )
        }
    }
}

/** The overflow menu shared by both conversation layouts: copy actions plus the
 *  per-message read/star and mail actions the caller enables. */
@Composable
internal fun MessageActionsButton(
    message: MessageBody,
    tint: Color,
    actionsEnabled: Boolean,
    itemActionsEnabled: Boolean,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onToggleRead: (MessageBody) -> Unit,
    onToggleStarred: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
    Box {
        val messageTextLabel = tr("chat.messageText")
        val subjectLabel = tr("composer.fields.subject")
        val messageIdLabel = tr("chat.messageId")
        val noSubjectLabel = tr("threads.noSubject")
        IconButton(onClick = { menuOpen = true }, modifier = Modifier.size(24.dp)) {
            Icon(
                Icons.Filled.MoreVert,
                contentDescription = tr("chat.moreMessageActions"),
                modifier = Modifier.size(16.dp),
                tint = tint,
            )
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            DropdownMenuItem(
                text = { Text(tr("chat.copyMessageText")) },
                onClick = {
                    menuOpen = false
                    onCopyMessageText(messageTextLabel, messagePlainText(message))
                },
            )
            DropdownMenuItem(
                text = { Text(tr("chat.copySubject")) },
                onClick = {
                    menuOpen = false
                    onCopyMessageText(subjectLabel, message.subject.ifBlank { noSubjectLabel })
                },
            )
            if (message.messageId.isNotBlank()) {
                DropdownMenuItem(
                    text = { Text(tr("chat.copyMessageId")) },
                    onClick = {
                        menuOpen = false
                        onCopyMessageText(messageIdLabel, message.messageId)
                    },
                )
            }
            if (itemActionsEnabled) {
                DropdownMenuItem(
                    text = { Text(if (message.unread) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread")) },
                    onClick = {
                        menuOpen = false
                        onToggleRead(message)
                    },
                )
                DropdownMenuItem(
                    text = { Text(if (message.starred) tr("chat.unstar") else tr("chat.star")) },
                    onClick = {
                        menuOpen = false
                        onToggleStarred(message)
                    },
                )
            }
            if (actionsEnabled) {
                DropdownMenuItem(
                    text = { Text(tr("chat.actions.forward")) },
                    onClick = {
                        menuOpen = false
                        onForward(message)
                    },
                )
                DropdownMenuItem(
                    text = { Text(tr("chat.actions.editAsNewMessage")) },
                    onClick = {
                        menuOpen = false
                        onEditAsNew(message)
                    },
                )
                DropdownMenuItem(
                    text = { Text(tr("chat.actions.deleteMessage"), color = MaterialTheme.colorScheme.error) },
                    onClick = {
                        menuOpen = false
                        onDelete(message)
                    },
                )
            }
        }
    }
}

/**
 * Everything below a message's header: the optional subject, the body (HTML or
 * plain), standalone attachments, and the send-status line. MessageBubble (chat)
 * and MessageRow (traditional) each wrap it in their own chrome, so the two
 * layouts differ only in the frame around this.
 */
@Composable
internal fun ColumnScope.MessageBodyContent(
    message: MessageBody,
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
) {
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
    val htmlBody = usesHtmlBody(message, preferHtml, searchQuery)
    val standaloneAttachmentsForMessage = standaloneAttachments(message)
    val (imageAttachments, otherAttachments) =
        standaloneAttachmentsForMessage.partition { it.mimeType.startsWith("image/") }
    val visibleImages = visibleImageAttachments(imageAttachments, remoteContent.allowRemote)
    RemoteContentNotice(
        remoteContent = remoteContent,
        hiddenImageCount = imageAttachments.size - visibleImages.size,
        bodyHasRemoteMedia = htmlBody && htmlHasRemoteMedia(message.bodyHtml),
        modifier = Modifier.padding(horizontal = chromeInset),
    )
    if (htmlBody) {
        HtmlMessageBody(
            html = message.bodyHtml,
            allowRemote = remoteContent.allowRemote,
            maxHeight = bodyMaxHeight,
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenHtmlImage,
        )
    } else if (message.bodyMissing) {
        // The core has no cached body (the on-demand fetch failed) — a
        // different state from a genuinely empty message, so offer a retry
        // instead of "(no content)".
        Column(Modifier.padding(horizontal = chromeInset)) {
            Text(
                tr("chat.messageLoadFailed"),
                color = textColor.copy(alpha = 0.6f),
                fontSize = 15.5.sp,
                lineHeight = 21.sp,
            )
            TextButton(onClick = onRetryLoad, modifier = Modifier.align(Alignment.End)) {
                Text(tr("chat.retry"))
            }
        }
    } else {
        // Subject is the conversation title (top bar); the body shows the
        // message text, matching the desktop chat reader.
        val bodyText: @Composable () -> Unit = {
            SelectableMessageText(
                text = message.body.ifBlank { "(no content)" },
                onOpenUrl = onOpenUrl,
                searchQuery = searchQuery,
                activeSearchMatch = activeSearchMatch,
                color = if (message.body.isBlank()) textColor.copy(alpha = 0.6f) else textColor,
                style =
                    messageBodyTextStyle(
                        MaterialTheme.typography.bodyLarge.copy(
                            fontSize = 15.5.sp,
                            lineHeight = 21.sp,
                        ),
                    ),
            )
        }
        if (bodyMaxHeight == Dp.Unspecified) {
            // Uncapped (the traditional layout): the message is as tall as it
            // needs to be and the conversation list scrolls it. A nested
            // scroller here would be measured with an infinite height by the
            // lazy list and throw.
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
                    loadImageAttachment = loadImageAttachment,
                    onOpen = onOpenImageAttachment,
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
                "Sending…",
                modifier = Modifier.align(Alignment.End).padding(horizontal = chromeInset),
                fontSize = 10.5.sp,
                color = textColor.copy(alpha = 0.55f),
            )
        }

        SendStatus.Failed -> {
            Text(
                "Failed to send",
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

/**
 * The "remote content blocked" strip above a message body: reveal this message's
 * remote content once, or trust its sender for good. Renders nothing when the
 * content is already allowed, or when the message has no remote content to hold
 * back — a plain note from a colleague must not grow a banner it has no use for.
 */
@Composable
internal fun RemoteContentNotice(
    remoteContent: MessageRemoteContent,
    hiddenImageCount: Int,
    bodyHasRemoteMedia: Boolean,
    modifier: Modifier = Modifier,
) {
    if (remoteContent.allowRemote) return
    if (hiddenImageCount == 0 && !bodyHasRemoteMedia) return
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Column(
        modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.55f))
            .padding(horizontal = 10.dp, vertical = 6.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Icon(
                Icons.Filled.HideImage,
                contentDescription = null,
                modifier = Modifier.size(14.dp),
                tint = muted,
            )
            Text(
                tr("chat.remoteBlocked"),
                modifier = Modifier.weight(1f),
                color = muted,
                fontSize = 11.5.sp,
            )
            TextButton(onClick = remoteContent.onReveal, contentPadding = RemoteNoticeButtonPadding) {
                Text(
                    if (hiddenImageCount > 0) {
                        tr("chat.showImages", mapOf("count" to hiddenImageCount))
                    } else {
                        tr("chat.showRemoteContent")
                    },
                    fontSize = 11.5.sp,
                )
            }
        }
        // Trusting the sender is app-wide and outlives the thread, so it is the
        // quieter of the two actions rather than a second peer button.
        if (remoteContent.senderAddress.isNotEmpty()) {
            TextButton(
                onClick = remoteContent.onAllowSender,
                contentPadding = RemoteNoticeButtonPadding,
            ) {
                Text(
                    tr("chat.allowRemoteFrom", mapOf("sender" to remoteContent.senderAddress)),
                    fontSize = 11.5.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

private val RemoteNoticeButtonPadding = PaddingValues(horizontal = 8.dp, vertical = 0.dp)

// Compose Constraints packs sizes into bit fields and cannot represent
// dimensions past ~262k px; sizing the WebView to an unclamped page height
// (a very tall newsletter, or a bogus negative report from the JS bridge)
// makes measurement throw. 20000dp stays well under the limit at any density.
internal val MailBodyMaxReportedHeight = 20_000.dp

internal fun clampMailBodyHeight(reported: Dp): Dp = reported.coerceIn(0.dp, MailBodyMaxReportedHeight)

/** A one-off token for the viewer's `script-src`. Only has to be unguessable by
 *  the mail rendered beside it, which never sees this process. */
private fun randomScriptNonce(): String = buildString { repeat(4) { append(Random.nextInt(1, Int.MAX_VALUE).toString(36)) } }

@Composable
internal fun HtmlMessageBody(
    html: String,
    // Whether this message's remote content may load. The mail is spliced into
    // the document below, so its own baked CSP meta ends up outside that
    // document's head, where a meta policy is ignored: the head built here is
    // what actually enforces the decision on both platforms.
    allowRemote: Boolean,
    maxHeight: Dp = Dp.Unspecified,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit = {},
    fitWideContent: Boolean = false,
) {
    // The WebView can't tell Compose how tall its content is, so a tiny script
    // reports document height through a platform bridge and we size the view to
    // it. The bubble caps the height (desktop uses 360px) and the WebView scrolls
    // past that; the full-screen reader passes no cap and shows the whole email.
    var contentHeight by remember(html) { mutableStateOf(0.dp) }
    // A WebView reaches neither the app's text sizes nor the system font-size
    // setting, so the reading typography is baked into the stylesheet below.
    // Sizing the text rather than zooming the page is what the overrides here
    // already assume: they flatten the mail's own sizes, so scaling the two
    // declarations scales every body, and images and tables keep fitting the
    // width they were laid out for.
    // Plain-text bodies are sized in sp, which carries the system font setting
    // on both platforms. Where the web view doesn't apply that setting itself,
    // fold it in here so an HTML mail and a text one stay the same size.
    val systemFontScale = if (MailWebViewFollowsSystemFontScale) 1f else LocalDensity.current.fontScale
    val bodyFontSize = scaledCssPx(MESSAGE_HTML_BASE_PX * systemFontScale, LocalMessageFontScale.current)
    // A fresh nonce per document admits the measurement script below and nothing
    // else: the mail is spliced into this page, so a script of its own that
    // survived the core's sanitiser would still have no way to name the token.
    val scriptNonce = remember(html, allowRemote) { randomScriptNonce() }
    val mobileHtml =
        remember(html, allowRemote, scriptNonce, fitWideContent, bodyFontSize) {
            val body = applyRemoteContentPolicy(html, allowRemote)
            """
            <!doctype html>
            <!-- The self-sizing WebView needs its document boxes to follow the
                 message. Newsletter resets commonly force html/body to
                 height:100%, pinning them to the empty initial viewport. The
                 override goes inline rather than into the head stylesheet:
                 sender styles are parsed later, and between two equally
                 specific !important rules the later one wins, while an inline
                 declaration outranks every stylesheet rule of the same
                 importance wherever the sender's <style> sits. -->
            <html style="height: auto !important; min-height: 0 !important;">
            <head>
              <meta http-equiv="Content-Security-Policy" content="${mailBodyCsp(allowRemote, scriptNonce)}">
              <meta id="meron-viewport" name="viewport" content="width=device-width, initial-scale=1.0">
              <style>
                html, body {
                  margin: 0;
                  padding: 0;
                  width: 100%;
                  /* `anywhere` also shrinks min-content to a single glyph, so a
                     narrow table cell (a 32px spacer holding a name, say) would
                     wrap its text one character per line. `break-word` still
                     breaks long words that would overflow, but leaves intrinsic
                     widths alone. */
                  overflow-wrap: break-word;
                  word-break: normal;
                  font-size: $bodyFontSize;
                  line-height: 1.45;
                }
                body, p, div, span, td, th, li, a {
                  font-size: $bodyFontSize !important;
                  line-height: 1.45 !important;
                }
                /* Preheaders hide their inbox-preview text with an inline
                   font-size:0; the override above would resurrect it as a
                   column of stray characters. */
                [style*="font-size:0"]:not([style*="font-size:0."]),
                [style*="font-size: 0"]:not([style*="font-size: 0."]) {
                  font-size: 0 !important;
                }
                /* max-width alone keeps fixed-pixel layouts (width="600") inside
                   the bubble. Forcing width:auto on top of it would also beat
                   the width="100%" attribute every email layout table relies on,
                   shrinking rows to their content and stranding right-aligned
                   cells and full-width dividers. */
                table {
                  max-width: 100% !important;
                }
                /* ...but max-width cannot shrink a table below its min-content
                   width, and nested fixed-width tables make that constraint
                   circular: the outer table's cell is sized by the inner
                   <table width="640">, so the inner table's max-width:100%
                   resolves against a 640px cell and never clamps, leaving the
                   outer table a 640px min-content width to inherit. The page
                   then lays out wider than the view and is clipped, not
                   scrolled. Clearing the width only where it is declared in
                   pixels breaks that chain at its source while leaving the
                   width="100%" tables above untouched. */
                table[width]:not([width$="%"]) {
                  width: auto !important;
                }
                table.code .diff-line-num {
                  width: 35px !important;
                  min-width: 35px;
                  white-space: nowrap;
                }
                td.line_content pre,
                th.line_content pre {
                  margin: 0 !important;
                  padding: 0 !important;
                  border: 0 !important;
                  border-radius: 0;
                  overflow-wrap: anywhere;
                  white-space: pre-wrap;
                }
                td.line_content pre code,
                th.line_content pre code {
                  min-width: 0;
                }
                img {
                  max-width: 100% !important;
                  height: auto !important;
                }
                div[data-meron-image-grid] {
                  display: flex !important;
                  flex-wrap: wrap !important;
                  gap: 4px !important;
                }
                div[data-meron-image-grid] > * {
                  flex: 1 1 30% !important;
                  max-width: calc(33.333% - 3px) !important;
                  box-sizing: border-box !important;
                  margin: 0 !important;
                }
              </style>
            </head>
            <body style="height: auto !important; min-height: 0 !important;">$body
              <script nonce="$scriptNonce">
                (function () {
                  // Feed/newsletter HTML often lists photos as a bare run of
                  // sibling `<img>` (or single-image `<p>`/`<div>`) elements,
                  // which would otherwise stack one per row at full width.
                  // Wrap runs of 2+ into a flex grid so they tile 2-3 across.
                  function isImageOnlyBlock(el) {
                    if (!el || el.nodeType !== 1) return false;
                    if (el.tagName === 'IMG') return true;
                    if (el.children.length !== 1) return false;
                    for (var i = 0; i < el.childNodes.length; i++) {
                      var n = el.childNodes[i];
                      if (n.nodeType === 3 && n.textContent.trim().length > 0) return false;
                    }
                    return isImageOnlyBlock(el.children[0]);
                  }
                  function findImageBlock(img) {
                    var node = img;
                    while (node.parentElement && node.parentElement !== document.body) {
                      if (isImageOnlyBlock(node.parentElement)) {
                        node = node.parentElement;
                      } else {
                        break;
                      }
                    }
                    return node;
                  }
                  function groupConsecutiveImages() {
                    var imgs = Array.prototype.slice.call(document.querySelectorAll('img'));
                    var blocks = [];
                    var seen = [];
                    imgs.forEach(function (img) {
                      var block = findImageBlock(img);
                      if (seen.indexOf(block) === -1) {
                        seen.push(block);
                        blocks.push(block);
                      }
                    });
                    var i = 0;
                    while (i < blocks.length) {
                      var run = [blocks[i]];
                      var j = i + 1;
                      while (
                        j < blocks.length &&
                        run[run.length - 1].nextElementSibling === blocks[j] &&
                        run[run.length - 1].parentElement === blocks[j].parentElement
                      ) {
                        run.push(blocks[j]);
                        j++;
                      }
                      if (run.length > 1) {
                        var grid = document.createElement('div');
                        grid.setAttribute('data-meron-image-grid', '1');
                        run[0].parentNode.insertBefore(grid, run[0]);
                        run.forEach(function (block) {
                          grid.appendChild(block);
                        });
                      }
                      i = j;
                    }
                  }
                  // Mail bodies arrive as whole documents and routinely carry
                  // their own <meta name="viewport">, which lands in our body
                  // and, being later in the document, is the one Blink honours.
                  // A single stray `content="target-densitydpi=device-dpi"` --
                  // common in mail templates, and what Gemini's welcome mail
                  // ships -- declares no width at all, so with useWideViewPort
                  // on the layout falls back to the 980px desktop default and
                  // the whole page renders at ~0.37 scale: legible mail shrunk
                  // to nothing. Dropping every viewport but ours puts the width
                  // back under our control, which is also what applyWidthFit
                  // below assumes when it measures and rewrites.
                  function dropForeignViewports() {
                    var metas = document.querySelectorAll('meta[name="viewport"]');
                    var ours = null;
                    for (var i = 0; i < metas.length; i++) {
                      if (metas[i].id === 'meron-viewport') {
                        ours = metas[i];
                      } else if (metas[i].parentNode) {
                        metas[i].parentNode.removeChild(metas[i]);
                      }
                    }
                    // Removing a meta does not make Blink recompute the viewport
                    // -- the mail's description stays in effect over an empty
                    // head -- but writing to one does. Re-asserting the same
                    // content is what actually applies the width above.
                    if (ours) ours.setAttribute('content', ours.getAttribute('content'));
                  }
                  // The CSS overrides reflow most mail into the view. What they
                  // cannot shrink -- a wide <pre>, an oversized image, a fixed
                  // width in an inline style rather than the width attribute --
                  // would otherwise be clipped outright, because the view is
                  // sized to its content and never scrolls sideways. Widening
                  // the viewport to the content's natural width instead makes
                  // WebView scale the whole page down to fit.
                  var fitWide = ${if (fitWideContent) "true" else "false"};
                  var fitScale = 1;
                  var viewWidth = 0;
                  function visibleWidth() {
                    return (
                      (window.visualViewport && window.visualViewport.width) ||
                      window.innerWidth ||
                      0
                    );
                  }
                  function applyWidthFit() {
                    // Pin once: after the viewport widens, the page is no longer
                    // overflowing, so re-measuring would just undo the fit.
                    if (!fitWide || fitScale !== 1) return;
                    // By id, not by name: a mail's own viewport meta would win
                    // over ours in Blink, so rewriting the first match could
                    // rewrite a tag the engine is already ignoring.
                    var meta = document.getElementById('meron-viewport');
                    if (!meta) return;
                    // Captured before the viewport widens, so it stays the view's
                    // width in dp -- the denominator the height bridge needs.
                    if (!viewWidth) viewWidth = visibleWidth();
                    if (!viewWidth) return;
                    var natural = Math.max(
                      document.documentElement.scrollWidth || 0,
                      document.body ? document.body.scrollWidth : 0
                    );
                    // A little slack: sub-pixel table borders routinely round up
                    // and are not worth shrinking the whole page for.
                    if (natural <= viewWidth + 2) return;
                    fitScale = viewWidth / natural;
                    // Dropping initial-scale lets WebView pick the fit scale.
                    meta.setAttribute('content', 'width=' + Math.ceil(natural));
                  }
                  // The root element is the one that scrolls, so its scrollHeight
                  // never drops below the view's own height -- sizing off it pins
                  // a short mail to whatever height the view was first laid out
                  // at and lets the measurement only ever grow, padding a
                  // one-line message out with blank space. Measure the boxes,
                  // whose heights are auto and so track the content, and keep
                  // scrollHeight purely as an overflow signal for content that
                  // escapes the body (an absolutely positioned block whose
                  // containing block is the initial one reaches nothing else).
                  //
                  // That signal only says anything while the view is still
                  // shorter than the content: once it grows to fit, scrollHeight
                  // and clientHeight agree again and the reading is
                  // indistinguishable from an empty document. So an extent, once
                  // seen, is carried as a floor for the rest of this document --
                  // re-deriving it each pass is what would make the view
                  // oscillate between the overflow height and the empty box.
                  //
                  // A floor is only worth carrying for a scroll area the boxes
                  // cannot account for. A tall ordinary mail overflows the
                  // view's first layout too, and retaining that would pin it
                  // there: text that reflows shorter once the view settles at
                  // its width could never give the height back.
                  //
                  // The desktop frame applies the same rules; its arithmetic
                  // (and the tests pinning it) lives in chat/frameHeight.ts.
                  var overflowExtent = 0;
                  function contentHeight() {
                    var root = document.documentElement;
                    var body = document.body;
                    var h = 0;
                    if (body) {
                      var rect = body.getBoundingClientRect();
                      h = Math.max(h, rect.top + rect.height, rect.top + body.scrollHeight);
                    }
                    if (root) {
                      var rootRect = root.getBoundingClientRect();
                      h = Math.max(h, rootRect.top + rootRect.height);
                      if (
                        root.scrollHeight > root.clientHeight + 1 &&
                        root.scrollHeight > h + 1
                      ) {
                        overflowExtent = Math.max(overflowExtent, root.scrollHeight);
                      }
                    }
                    return Math.max(h, overflowExtent);
                  }
                  function report() {
                    applyWidthFit();
                    // The measurement is in the (possibly widened) layout
                    // viewport's CSS pixels; the view renders it at fitScale, so
                    // scale it back to dp or the view gets sized to a phantom tail.
                    var h = Math.ceil(contentHeight() * fitScale);
                    if (window.MeronHeight && window.MeronHeight.report) {
                      window.MeronHeight.report(h);
                    } else if (
                      window.webkit &&
                      window.webkit.messageHandlers &&
                      window.webkit.messageHandlers.meronHeight
                    ) {
                      window.webkit.messageHandlers.meronHeight.postMessage(h);
                    }
                  }
                  document.addEventListener('click', function (event) {
                    var target = event.target;
                    var image = target && target.closest ? target.closest('img[src]') : null;
                    if (image) {
                      var src = image.getAttribute('src');
                      if (src) {
                        event.preventDefault();
                        if (window.MeronImage && window.MeronImage.open) {
                          window.MeronImage.open(src);
                        } else if (
                          window.webkit &&
                          window.webkit.messageHandlers &&
                          window.webkit.messageHandlers.meronImage
                        ) {
                          window.webkit.messageHandlers.meronImage.postMessage(src);
                        }
                        return;
                      }
                    }
                    var anchor = target && target.closest ? target.closest('a[href]') : null;
                    if (!anchor) return;
                    var href = anchor.getAttribute('href');
                    if (!href || href.charAt(0) === '#') return;
                    event.preventDefault();
                    var url = anchor.href || href;
                    if (window.MeronLink && window.MeronLink.open) {
                      window.MeronLink.open(url);
                    } else if (
                      window.webkit &&
                      window.webkit.messageHandlers &&
                      window.webkit.messageHandlers.meronLink
                    ) {
                      window.webkit.messageHandlers.meronLink.postMessage(url);
                    }
                  });
                  // Before the first report: every width the script measures,
                  // and the view height derived from them, is read under the
                  // viewport this leaves in place.
                  dropForeignViewports();
                  groupConsecutiveImages();
                  window.addEventListener('load', report);
                  document.addEventListener('DOMContentLoaded', report);
                  if (window.ResizeObserver) {
                    new ResizeObserver(report).observe(document.documentElement);
                  }
                  setTimeout(report, 300);
                })();
              </script>
            </body>
            </html>
            """.trimIndent()
        }
    val measured = contentHeight > 0.dp
    val capped = maxHeight != Dp.Unspecified && measured && contentHeight > maxHeight
    val webViewModifier =
        Modifier
            .fillMaxWidth()
            .then(
                if (measured) {
                    Modifier.height(contentHeight)
                } else {
                    Modifier.heightIn(min = 80.dp)
                },
            )

    if (capped) {
        val htmlScrollState = rememberScrollState()
        Box(
            Modifier
                .fillMaxWidth()
                .height(maxHeight)
                .appScrollbar(htmlScrollState, endOffset = HtmlBubbleHorizontalPadding)
                .verticalScroll(htmlScrollState),
        ) {
            MailWebViewWithLinkMenu(
                html = mobileHtml,
                onContentHeight = { contentHeight = clampMailBodyHeight(it) },
                onOpenUrl = onOpenUrl,
                onOpenImage = onOpenImage,
                modifier = webViewModifier,
                fitWideContent = fitWideContent,
            )
        }
    } else {
        MailWebViewWithLinkMenu(
            html = mobileHtml,
            onContentHeight = { contentHeight = clampMailBodyHeight(it) },
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenImage,
            modifier = webViewModifier,
            fitWideContent = fitWideContent,
        )
    }
}

/** The web view plus the link menu it asks for on long press. The menu is anchored to
 *  a box laid out exactly like the web view, so the reported press position places it. */
@Composable
private fun MailWebViewWithLinkMenu(
    html: String,
    onContentHeight: (Dp) -> Unit,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit,
    modifier: Modifier,
    fitWideContent: Boolean,
) {
    var menuTarget by remember { mutableStateOf<MessageLinkMenuTarget?>(null) }
    Box(modifier) {
        MailWebView(
            html = html,
            onContentHeight = onContentHeight,
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenImage,
            onLinkLongPress = { url, offset -> menuTarget = MessageLinkMenuTarget(url, offset) },
            fitWideContent = fitWideContent,
            modifier = Modifier.fillMaxSize(),
        )
        MessageLinkContextMenu(
            target = menuTarget,
            onDismiss = { menuTarget = null },
            onOpenUrl = onOpenUrl,
        )
    }
}
