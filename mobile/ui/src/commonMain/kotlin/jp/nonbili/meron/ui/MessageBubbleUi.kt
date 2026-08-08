package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.formatRecipientSummary
import jp.nonbili.meron.shared.standaloneAttachments

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
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (outgoing) Arrangement.End else Arrangement.Start,
    ) {
        Column(
            Modifier
                // Bubble width tracks the screen: ~85% of available width so it
                // grows on tablets, capped so it stays readable on wide screens.
                .fillMaxWidth(0.85f)
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
                .padding(start = 14.dp, end = 14.dp, top = 8.dp, bottom = 6.dp),
            verticalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            // Sender, timestamp and the actions menu share one row to keep the
            // bubble compact, matching the desktop reader's header layout.
            Row(
                Modifier.fillMaxWidth(),
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
                // difference between them.
                Row(
                    Modifier
                        .weight(1f)
                        .clip(RoundedCornerShape(4.dp))
                        .clickable { addressesOpen = !addressesOpen },
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
                    Icon(
                        if (addressesOpen) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                        contentDescription = if (addressesOpen) tr("chat.hideDetails") else tr("chat.showDetails"),
                        modifier = Modifier.size(14.dp),
                        tint = textColor.copy(alpha = 0.55f),
                    )
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
                    modifier = Modifier.padding(bottom = 2.dp),
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
            color = textColor,
            fontSize = 16.sp,
            lineHeight = 21.sp,
            fontWeight = FontWeight.SemiBold,
        )
    }
    if (preferHtml && message.bodyHtml.isNotBlank() && searchQuery.isBlank()) {
        HtmlMessageBody(
            html = message.bodyHtml,
            maxHeight = bodyMaxHeight,
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenHtmlImage,
        )
    } else if (message.bodyMissing) {
        // The core has no cached body (the on-demand fetch failed) — a
        // different state from a genuinely empty message, so offer a retry
        // instead of "(no content)".
        Column {
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
                    MaterialTheme.typography.bodyLarge.copy(
                        fontSize = 15.5.sp,
                        lineHeight = 21.sp,
                    ),
            )
        }
        if (bodyMaxHeight == Dp.Unspecified) {
            // Uncapped (the traditional layout): the message is as tall as it
            // needs to be and the conversation list scrolls it. A nested
            // scroller here would be measured with an infinite height by the
            // lazy list and throw.
            bodyText()
        } else {
            Box(
                Modifier
                    .fillMaxWidth()
                    .heightIn(max = bodyMaxHeight)
                    .verticalScroll(rememberScrollState()),
            ) {
                bodyText()
            }
        }
    }
    val standaloneAttachmentsForMessage = standaloneAttachments(message)
    if (standaloneAttachmentsForMessage.isNotEmpty()) {
        val (imageAttachments, otherAttachments) =
            standaloneAttachmentsForMessage.partition { it.mimeType.startsWith("image/") }
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (imageAttachments.isNotEmpty()) {
                AttachmentImageGrid(
                    images = imageAttachments,
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
                modifier = Modifier.align(Alignment.End),
                fontSize = 10.5.sp,
                color = textColor.copy(alpha = 0.55f),
            )
        }

        SendStatus.Failed -> {
            Text(
                "Failed to send",
                modifier = Modifier.align(Alignment.End),
                fontSize = 10.5.sp,
                color = MaterialTheme.colorScheme.error,
            )
        }

        SendStatus.None -> {
            Unit
        }
    }
}

// Compose Constraints packs sizes into bit fields and cannot represent
// dimensions past ~262k px; sizing the WebView to an unclamped page height
// (a very tall newsletter, or a bogus negative report from the JS bridge)
// makes measurement throw. 20000dp stays well under the limit at any density.
internal val MailBodyMaxReportedHeight = 20_000.dp

internal fun clampMailBodyHeight(reported: Dp): Dp = reported.coerceIn(0.dp, MailBodyMaxReportedHeight)

@Composable
internal fun HtmlMessageBody(
    html: String,
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
    val mobileHtml =
        remember(html, fitWideContent) {
            """
            <!doctype html>
            <html>
            <head>
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
                  font-size: 16px;
                  line-height: 1.45;
                }
                body, p, div, span, td, th, li, a {
                  font-size: 16px !important;
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
            <body>$html
              <script>
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
                  function report() {
                    applyWidthFit();
                    // scrollHeight is in the (possibly widened) layout viewport's
                    // CSS pixels; the view renders it at fitScale, so scale it
                    // back to dp or the view gets sized to a phantom tail.
                    var h = Math.ceil(
                      Math.max(
                        document.documentElement.scrollHeight || 0,
                        document.body ? document.body.scrollHeight : 0
                      ) * fitScale
                    );
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
        Box(
            Modifier
                .fillMaxWidth()
                .height(maxHeight)
                .verticalScroll(rememberScrollState()),
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
