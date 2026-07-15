package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
    onOpenMessage: (MessageBody) -> Unit,
    onOpenUrl: (String) -> Unit,
    onRetryLoad: () -> Unit,
) {
    var menuOpen by remember { mutableStateOf(false) }
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
                if (!outgoing) {
                    Text(
                        message.from.ifBlank { message.fromAddr },
                        modifier = Modifier.weight(1f),
                        fontSize = 12.5.sp,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.primary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                } else {
                    Spacer(Modifier.weight(1f))
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
                            tint = textColor.copy(alpha = 0.55f),
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
                        if (actionsEnabled) {
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
                // Subject is the conversation title (top bar); the bubble shows the
                // message body, matching the desktop chat reader.
                Box(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(max = bodyMaxHeight)
                        .verticalScroll(rememberScrollState()),
                ) {
                    Text(
                        highlightedMessageText(message.body.ifBlank { "(no content)" }, searchQuery, activeSearchMatch),
                        color = if (message.body.isBlank()) textColor.copy(alpha = 0.6f) else textColor,
                        fontSize = 15.5.sp,
                        lineHeight = 21.sp,
                    )
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
    }
}

@Composable
internal fun HtmlMessageBody(
    html: String,
    maxHeight: Dp = Dp.Unspecified,
    onOpenUrl: (String) -> Unit,
    onOpenImage: (String) -> Unit = {},
) {
    // The WebView can't tell Compose how tall its content is, so a tiny script
    // reports document height through a platform bridge and we size the view to
    // it. The bubble caps the height (desktop uses 360px) and the WebView scrolls
    // past that; the full-screen reader passes no cap and shows the whole email.
    var contentHeight by remember(html) { mutableStateOf(0.dp) }
    val mobileHtml =
        remember(html) {
            """
            <!doctype html>
            <html>
            <head>
              <meta name="viewport" content="width=device-width, initial-scale=1.0">
              <style>
                html, body {
                  margin: 0;
                  padding: 0;
                  width: 100%;
                  overflow-wrap: anywhere;
                  word-break: normal;
                  font-size: 16px;
                  line-height: 1.45;
                }
                body, p, div, span, td, th, li, a {
                  font-size: 16px !important;
                  line-height: 1.45 !important;
                }
                table {
                  max-width: 100% !important;
                  width: auto !important;
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
                  function report() {
                    var h = Math.ceil(
                      Math.max(
                        document.documentElement.scrollHeight || 0,
                        document.body ? document.body.scrollHeight : 0
                      )
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
            MailWebView(
                html = mobileHtml,
                onContentHeight = { contentHeight = it },
                onOpenUrl = onOpenUrl,
                onOpenImage = onOpenImage,
                modifier = webViewModifier,
            )
        }
    } else {
        MailWebView(
            html = mobileHtml,
            onContentHeight = { contentHeight = it },
            onOpenUrl = onOpenUrl,
            onOpenImage = onOpenImage,
            modifier = webViewModifier,
        )
    }
}
