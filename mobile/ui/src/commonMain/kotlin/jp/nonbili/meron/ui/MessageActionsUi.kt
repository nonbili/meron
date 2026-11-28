package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Forward
import androidx.compose.material.icons.automirrored.filled.ReplyAll
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.EditNote
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.MarkEmailRead
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.Translate
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.folderIsDrafts

/** A message's actions menu, shared by the conversation layouts and the reader:
 *  copy actions plus the read/star and mail actions the caller enables. */
@Composable
internal fun MessageActionsMenu(
    expanded: Boolean,
    onDismiss: () -> Unit,
    preferHtml: Boolean,
    allowRemote: Boolean,
    message: MessageBody,
    isRss: Boolean,
    actionsEnabled: Boolean,
    onForward: (MessageBody) -> Unit,
    onReplyAllToMessage: (MessageBody) -> Unit,
    canReplyAllToMessage: (MessageBody) -> Boolean,
    onEditAsNew: (MessageBody) -> Unit,
    // Null where the message can't be marked from here (the reader).
    onToggleRead: ((MessageBody) -> Unit)?,
    onToggleStarred: ((MessageBody) -> Unit)?,
    onDelete: (MessageBody) -> Unit,
    onCopyMessageText: (String, String) -> Unit,
    onOpenUrl: (String) -> Unit,
    // Reply all, forward and delete sit in the reader's bottom bar instead.
    mailActionsInBar: Boolean = false,
    translations: MessageTranslations? = null,
) {
    val printMessage = rememberPrintMessage(preferHtml, allowRemote)
    val translation = translations?.state(message)
    val messageTextLabel = tr("chat.messageText")
    val subjectLabel = tr("composer.fields.subject")
    val messageIdLabel = tr("chat.messageId")
    val noSubjectLabel = tr("threads.noSubject")
    val linkLabel = tr("composer.toolbar.link")

    @Composable
    fun Item(
        label: String,
        icon: ImageVector,
        enabled: Boolean = true,
        danger: Boolean = false,
        supportingText: String? = null,
        onClick: () -> Unit,
    ) {
        val error = MaterialTheme.colorScheme.error
        DropdownMenuItem(
            text = {
                Column {
                    Text(label, color = if (danger) error else Color.Unspecified)
                    supportingText?.let {
                        Text(it, style = MaterialTheme.typography.bodySmall)
                    }
                }
            },
            leadingIcon = { Icon(icon, contentDescription = null, tint = if (danger) error else LocalContentColor.current) },
            enabled = enabled,
            onClick = {
                onDismiss()
                onClick()
            },
        )
    }

    DropdownMenu(expanded = expanded, onDismissRequest = onDismiss) {
        if (!isRss) {
            // The core supplies plain text for both plain-text and HTML email.
            if (translations != null && !message.bodyMissing && message.translatableText().isNotBlank()) {
                val label =
                    when {
                        translation?.translatedBody != null -> "chat.actions.showOriginal"
                        translation?.result != null -> "chat.translation.showTranslation"
                        translations.attribution != null -> "chat.translation.withGoogle"
                        else -> "chat.actions.translate"
                    }
                Item(
                    tr(label),
                    Icons.Filled.Translate,
                    enabled = translation?.loading != true,
                    supportingText = if (label == "chat.translation.withGoogle") tr("chat.translation.onDevicePrivacy") else null,
                ) {
                    translations.toggle(message)
                }
            }
            Item(tr("chat.actions.print"), Icons.Filled.Print, enabled = !message.bodyMissing) {
                printMessage(message)
            }
        }
        if (isRss && message.link.isNotBlank()) {
            Item(tr("chat.actions.openLink"), Icons.Filled.Public) { onOpenUrl(message.link) }
            Item(tr("chat.actions.copyLinkAddress"), Icons.Filled.Link) {
                onCopyMessageText(linkLabel, message.link)
            }
        }
        Item(tr("chat.copyMessageText"), Icons.Filled.ContentCopy) {
            onCopyMessageText(messageTextLabel, messagePlainText(message))
        }
        Item(tr("chat.copySubject"), Icons.Filled.ContentCopy) {
            onCopyMessageText(subjectLabel, message.subject.ifBlank { noSubjectLabel })
        }
        if (message.messageId.isNotBlank()) {
            Item(tr("chat.copyMessageId"), Icons.Filled.ContentCopy) {
                onCopyMessageText(messageIdLabel, message.messageId)
            }
        }
        if (onToggleRead != null) {
            Item(
                if (message.unread) tr("threads.actions.markAsRead") else tr("threads.actions.markAsUnread"),
                if (message.unread) Icons.Filled.MarkEmailRead else Icons.Filled.MarkEmailUnread,
            ) { onToggleRead(message) }
        }
        if (onToggleStarred != null) {
            Item(
                if (message.starred) tr("chat.unstar") else tr("chat.star"),
                if (message.starred) Icons.Filled.StarBorder else Icons.Filled.Star,
            ) { onToggleStarred(message) }
        }
        if (actionsEnabled) {
            // A draft has no sender to reply to: replying to one would
            // thread a new "Re:" under an unsent message instead of
            // opening it to edit. A message with no other recipients has
            // nobody for reply-all to add, which makes it the reply the
            // bar already sends.
            if (!mailActionsInBar && !folderIsDrafts(message.folderId) && canReplyAllToMessage(message)) {
                Item(tr("chat.actions.replyAll"), Icons.AutoMirrored.Filled.ReplyAll) { onReplyAllToMessage(message) }
            }
            if (!mailActionsInBar) {
                Item(tr("chat.actions.forward"), Icons.AutoMirrored.Filled.Forward) { onForward(message) }
            }
            Item(tr("chat.actions.editAsNewMessage"), Icons.Filled.EditNote) { onEditAsNew(message) }
            if (!mailActionsInBar) {
                Item(tr("chat.actions.deleteMessage"), Icons.Filled.Delete, danger = true) { onDelete(message) }
            }
        }
    }
}
