package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.formatRecipientSummary

private const val COLLAPSED_PREVIEW_CHARS = 200

/**
 * One message in the traditional conversation layout: a full-width card that
 * collapses to a two-line summary, the way classic mail clients stack a thread.
 * The chat layout's [MessageBubble] is the alternative (see
 * [ConversationLayout]); both share [MessageBodyContent] for the body, so the
 * two differ only in chrome.
 */
@Composable
internal fun MessageRow(
    message: MessageBody,
    outgoing: Boolean,
    preferHtml: Boolean,
    searchQuery: String,
    activeSearchMatch: Boolean,
    expanded: Boolean,
    onToggleExpanded: () -> Unit,
    actionsEnabled: Boolean,
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
    val shape = RoundedCornerShape(12.dp)
    val textColor = MaterialTheme.colorScheme.onSurface
    val mutedColor = textColor.copy(alpha = 0.6f)
    val isDraft = folderIsDrafts(message.folderId)
    val senderLabel = messageSenderLabel(message, outgoing)
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = shape,
        modifier =
            Modifier
                .fillMaxWidth()
                .clip(shape)
                .then(
                    if (activeSearchMatch) {
                        Modifier.border(2.dp, Color(0xFFFFC107), shape)
                    } else {
                        Modifier
                    },
                ),
    ) {
        if (!expanded) {
            CollapsedMessageRow(
                message = message,
                senderLabel = senderLabel,
                isDraft = isDraft,
                textColor = textColor,
                mutedColor = mutedColor,
                remoteContent = remoteContent,
                preferHtml = preferHtml,
                searchQuery = searchQuery,
                onClick = onToggleExpanded,
            )
        } else {
            Column(
                Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                MessageRowHeader(
                    message = message,
                    senderLabel = senderLabel,
                    outgoing = outgoing,
                    isDraft = isDraft,
                    isRss = isRss,
                    textColor = textColor,
                    mutedColor = mutedColor,
                    remoteContent = remoteContent,
                    preferHtml = preferHtml,
                    searchQuery = searchQuery,
                    actionsEnabled = actionsEnabled,
                    itemActionsEnabled = itemActionsEnabled,
                    onToggleExpanded = onToggleExpanded,
                    onForward = onForward,
                    onEditAsNew = onEditAsNew,
                    onOpenDraft = onOpenDraft,
                    onToggleRead = onToggleRead,
                    onToggleStarred = onToggleStarred,
                    onDelete = onDelete,
                    onCopyMessageText = onCopyMessageText,
                    onComposeTo = onComposeTo,
                    onOpenMessage = onOpenMessage,
                )
                MessageBodyContent(
                    message = message,
                    textColor = textColor,
                    preferHtml = preferHtml,
                    searchQuery = searchQuery,
                    activeSearchMatch = activeSearchMatch,
                    showSubject = showSubject,
                    // Uncapped: a full-width reading view shows the whole
                    // message and the conversation list does the scrolling.
                    bodyMaxHeight = Dp.Unspecified,
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
}

@Composable
private fun CollapsedMessageRow(
    message: MessageBody,
    senderLabel: String,
    isDraft: Boolean,
    textColor: Color,
    mutedColor: Color,
    // A collapsed row shows the blocked-content marker too: it is what tells the
    // reader the message is holding something back before they open it.
    remoteContent: MessageRemoteContent,
    preferHtml: Boolean,
    searchQuery: String,
    onClick: () -> Unit,
) {
    val preview =
        remember(message.id, message.body, message.bodyHtml) {
            messagePlainText(message)
                .replace(Regex("\\s+"), " ")
                .trim()
                .take(COLLAPSED_PREVIEW_CHARS)
        }
    Column(
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Row(
            Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                senderLabel,
                modifier = Modifier.weight(1f),
                fontSize = 13.sp,
                fontWeight = if (message.unread) FontWeight.Bold else FontWeight.SemiBold,
                color = textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            BlockedRemoteButton(
                message = message,
                remoteContent = remoteContent,
                preferHtml = preferHtml,
                searchQuery = searchQuery,
            )
            MessageRowBadges(message = message, isDraft = isDraft, mutedColor = mutedColor)
        }
        Text(
            preview,
            fontSize = 12.5.sp,
            color = mutedColor,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Draft/attachment/star markers and the timestamp, shared by both states of a row. */
@Composable
private fun MessageRowBadges(
    message: MessageBody,
    isDraft: Boolean,
    mutedColor: Color,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (isDraft) {
            Text(
                text = tr("chat.draft"),
                color = MaterialTheme.colorScheme.onErrorContainer,
                fontSize = 10.sp,
                fontWeight = FontWeight.Bold,
                modifier =
                    Modifier
                        .clip(RoundedCornerShape(4.dp))
                        .background(MaterialTheme.colorScheme.errorContainer)
                        .padding(horizontal = 4.dp, vertical = 1.dp),
            )
        }
        if (message.hasAttachments) {
            Icon(
                Icons.Filled.AttachFile,
                contentDescription = null,
                modifier = Modifier.size(13.dp),
                tint = mutedColor,
            )
        }
        if (message.starred) {
            Icon(
                Icons.Filled.Star,
                contentDescription = null,
                modifier = Modifier.size(13.dp),
                tint = Color(0xFFF59E0B),
            )
        }
        Text(
            formatInboxTimestamp(message.dateEpochSeconds),
            fontSize = 10.5.sp,
            color = mutedColor,
        )
    }
}

/**
 * The header of an expanded [MessageRow]: sender, badges and the row's actions.
 * The conversation pins a copy of this over the list while a long message
 * scrolls past (see [pinnedHeaderMessageIndex]), so it lives on its own and
 * keeps no state the card below needs to share.
 */
@Composable
internal fun MessageRowHeader(
    message: MessageBody,
    senderLabel: String,
    outgoing: Boolean,
    isDraft: Boolean,
    isRss: Boolean,
    textColor: Color,
    mutedColor: Color,
    // Whether this message's remote content may load, for the header's reveal
    // affordance (see [BlockedRemoteButton]).
    remoteContent: MessageRemoteContent,
    preferHtml: Boolean,
    searchQuery: String,
    actionsEnabled: Boolean,
    itemActionsEnabled: Boolean,
    onToggleExpanded: () -> Unit,
    onForward: (MessageBody) -> Unit,
    onEditAsNew: (MessageBody) -> Unit,
    onOpenDraft: (MessageBody) -> Unit,
    onToggleRead: (MessageBody) -> Unit,
    onToggleStarred: (MessageBody) -> Unit,
    onDelete: (MessageBody) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    onOpenMessage: (MessageBody) -> Unit,
) {
    var addressesOpen by remember(message.id) { mutableStateOf(false) }
    Row(
        Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // Tapping the header collapses the message again; the
        // chevron beside the sender opens the full addresses, as it
        // does on a chat bubble.
        Column(
            Modifier
                .weight(1f)
                .clip(RoundedCornerShape(4.dp))
                .clickable(onClick = onToggleExpanded)
                .padding(vertical = 2.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    senderLabel,
                    modifier = Modifier.weight(1f, fill = false),
                    fontSize = 13.sp,
                    fontWeight = FontWeight.Bold,
                    color = textColor,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                // A feed item has no recipients, so its details
                // could only repeat the feed name above.
                if (!isRss) {
                    Icon(
                        if (addressesOpen) Icons.Filled.ExpandLess else Icons.Filled.ExpandMore,
                        contentDescription =
                            if (addressesOpen) tr("chat.hideDetails") else tr("chat.showDetails"),
                        modifier =
                            Modifier
                                .clip(RoundedCornerShape(4.dp))
                                .clickable { addressesOpen = !addressesOpen }
                                .size(16.dp),
                        tint = mutedColor,
                    )
                }
            }
            if (!outgoing && message.fromAddr.isNotBlank()) {
                Text(
                    message.fromAddr,
                    fontSize = 11.sp,
                    color = mutedColor,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        BlockedRemoteButton(
            message = message,
            remoteContent = remoteContent,
            preferHtml = preferHtml,
            searchQuery = searchQuery,
        )
        MessageRowBadges(message = message, isDraft = isDraft, mutedColor = mutedColor)
        IconButton(
            onClick = { if (isDraft) onOpenDraft(message) else onOpenMessage(message) },
            modifier = Modifier.size(24.dp),
        ) {
            Icon(
                imageVector = if (isDraft) Icons.Filled.Edit else Icons.Filled.OpenInFull,
                contentDescription = if (isDraft) tr("chat.draft") else tr("threads.actions.openInNewTab"),
                modifier = Modifier.size(15.dp),
                tint = mutedColor,
            )
        }
        MessageActionsButton(
            message = message,
            tint = mutedColor,
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
}

/** "To: …" for a message the account sent, the sender's name otherwise. */
@Composable
internal fun messageSenderLabel(
    message: MessageBody,
    outgoing: Boolean,
): String =
    if (outgoing) {
        val recipients = remember(message.to, message.cc) { formatRecipientSummary(message.to, message.cc) }
        if (recipients.isBlank()) {
            message.from.ifBlank { message.fromAddr }
        } else {
            tr("chat.toRecipients", mapOf("recipients" to recipients))
        }
    } else {
        message.from.ifBlank { message.fromAddr }
    }
