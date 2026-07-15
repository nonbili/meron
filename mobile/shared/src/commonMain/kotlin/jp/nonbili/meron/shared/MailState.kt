package jp.nonbili.meron.shared

import kotlin.random.Random

data class AccountSummary(
    val id: String,
    val email: String,
    val displayName: String = "",
    val senderName: String = "",
    val avatarUrl: String = "",
    val needsReconnect: Boolean = false,
    val engine: String = "",
    val provider: String = "",
    val authType: String = "",
    val imapHost: String = "",
    val imapPort: Int = 0,
    val smtpHost: String = "",
    val smtpPort: Int = 0,
    val loadRemoteImages: Boolean = false,
    val includedInUnified: Boolean = true,
    val muted: Boolean = false,
    val paused: Boolean = false,
    val conversationHtml: Boolean = true,
    val saveSentCopy: Boolean? = null,
    val rssSyncIntervalMinutes: Int = 60,
    val aliases: List<AccountAlias> = emptyList(),
    val chatWallpaperKind: String = "",
    val chatWallpaperPresetId: String = "",
    val chatWallpaperUrl: String = "",
)

data class AccountAlias(
    val email: String,
    val name: String = "",
)

data class ContactSuggestion(
    val name: String,
    val addr: String,
)

data class SendIdentity(
    val accountId: String,
    val email: String,
    val name: String = "",
)

data class FolderSummary(
    val accountId: String,
    val name: String,
    val unread: Int = 0,
)

data class ThreadSummary(
    val id: String,
    val accountId: String,
    val folder: String,
    val subject: String,
    val sender: String,
    val preview: String = "",
    val unread: Boolean = false,
    val starred: Boolean = false,
    val hasDraft: Boolean = false,
    val dateEpochSeconds: Long = 0,
    val feedUrl: String = "",
    val threadId: String = "",
)

data class StarredItemSummary(
    val id: String,
    val threadId: String,
    val accountId: String,
    val folder: String,
    val subject: String,
    val sender: String,
    val preview: String = "",
    val unread: Boolean = false,
    val dateEpochSeconds: Long = 0,
)

data class MessageAttachment(
    val filename: String,
    val mimeType: String = "application/octet-stream",
    val sizeBytes: Long = 0,
    val key: String = "",
    val url: String = "",
)

data class ThreadGalleryImage(
    val attachment: MessageAttachment,
    val ref: String,
    val filename: String,
    val messageId: String,
)

data class ThreadMediaItem(
    val attachment: MessageAttachment,
    val ref: String,
    val filename: String,
    val type: String,
    val galleryIndex: Int? = null,
)

data class StorageUsage(
    val cacheBytes: Long = 0,
    val dbBytes: Long = 0,
)

// One release in the in-app changelog (the GitHub releases atom feed, filtered
// by the core to the mobile `android/v*` tags).
data class ChangelogRelease(
    val version: String,
    val tag: String,
    val date: String,
    val notes: List<String>,
)

// Send lifecycle for an optimistically inserted message. None covers both a
// freshly synced message and a successfully sent one (which is replaced by its
// canonical copy on re-fetch). Mirrors desktop's 'sending' | 'sent' | 'failed'.
enum class SendStatus {
    None,
    Sending,
    Failed,
}

data class MessageBody(
    val id: String,
    val folderId: String = "",
    val from: String,
    val to: String,
    val cc: String = "",
    val bcc: String = "",
    val subject: String,
    val body: String,
    val bodyHtml: String = "",
    val dateEpochSeconds: Long = 0,
    val fromAddr: String = "",
    val replyTo: String = "",
    val messageId: String = "",
    val inReplyTo: String = "",
    val references: String = "",
    val unread: Boolean = false,
    // Sent by this account, classified by the core (own address or Sent-folder
    // provenance) — true even for aliases not configured in meron.
    val outgoing: Boolean = false,
    val starred: Boolean = false,
    val hasAttachments: Boolean = false,
    // True when the core has no cached body for this message (the on-demand
    // fetch failed), as opposed to a message whose body is genuinely empty.
    val bodyMissing: Boolean = false,
    val attachments: List<MessageAttachment> = emptyList(),
    val sendStatus: SendStatus = SendStatus.None,
)

data class DraftAttachment(
    val id: String,
    val displayName: String,
    val mimeType: String = "application/octet-stream",
    val sizeBytes: Long = 0,
    val dataBase64: String = "",
)

data class ComposeDraft(
    val to: String = "",
    val cc: String = "",
    val bcc: String = "",
    val subject: String = "",
    val body: String = "",
    val attachments: List<DraftAttachment> = emptyList(),
) {
    constructor(to: String, subject: String, body: String) : this(to, "", "", subject, body, emptyList())

    val canSend: Boolean
        get() = to.isNotBlank() && subject.isNotBlank() && (body.isNotBlank() || attachments.isNotEmpty())
}

data class ReplyRecipients(
    val to: String,
    val cc: String,
)

fun newDraftMessageId(accountId: String = ""): String {
    val domain = accountId.substringAfter('@', missingDelimiterValue = "meron").ifBlank { "meron" }
    val suffix = Random.nextLong(0, Long.MAX_VALUE).toString(36)
    return "meron-draft-$suffix@$domain"
}

fun folderIsDrafts(folder: String): Boolean {
    val normalized = folder.trim().lowercase()
    return normalized == "drafts" ||
        normalized == "draft" ||
        normalized.endsWith("/drafts") ||
        normalized.endsWith("/draft") ||
        normalized.endsWith(".drafts") ||
        normalized.endsWith(".draft") ||
        normalized.endsWith("]drafts") ||
        normalized.endsWith("]draft")
}

fun folderIsTrash(folder: String): Boolean {
    val normalized = folder.trim().lowercase()
    return normalized == "trash" ||
        normalized == "bin" ||
        normalized == "deleted" ||
        normalized == "deleted items" ||
        normalized.endsWith("/trash") ||
        normalized.endsWith("/bin") ||
        normalized.endsWith("/deleted items") ||
        normalized.endsWith(".trash") ||
        normalized.endsWith(".bin") ||
        normalized.endsWith(".deleted items") ||
        normalized.endsWith("]trash") ||
        normalized.endsWith("]bin") ||
        normalized.endsWith("]deleted items")
}

data class MailUiState(
    val accounts: List<AccountSummary> = emptyList(),
    val folders: List<FolderSummary> = emptyList(),
    val threads: List<ThreadSummary> = emptyList(),
    val selectedAccountId: String? = null,
    val selectedFolder: String? = null,
    val selectedThreadId: String? = null,
    val selectedThread: List<MessageBody> = emptyList(),
    val draft: ComposeDraft = ComposeDraft(),
    val syncing: Boolean = false,
    val error: String? = null,
)

fun accountSummaryIsRss(account: AccountSummary): Boolean = account.engine == "rss" || account.provider == "rss" || account.authType == "rss"

fun threadIdIsRss(threadId: String): Boolean = threadId.contains("#rss#")

// Display names carrying recipient-list specials must be quoted (RFC 5322
// quoted-string), otherwise a "Doe, Jane <j@x>" entry splits into two bogus
// recipients everywhere the list is comma-parsed.
private const val DISPLAY_NAME_SPECIALS = ",;<>@\"\\"

private fun quoteDisplayName(name: String): String =
    if (name.any { it in DISPLAY_NAME_SPECIALS }) {
        "\"" + name.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
    } else {
        name
    }

fun formatContactSuggestion(contact: ContactSuggestion): String {
    val name = contact.name.trim()
    val addr = contact.addr.trim()
    return if (name.isNotBlank() && !name.equals(addr, ignoreCase = true)) {
        "${quoteDisplayName(name)} <$addr>"
    } else {
        addr
    }
}

// Index of the last comma that actually separates recipients — commas inside
// a double-quoted display name or inside `<...>` don't count. -1 when none.
fun lastRecipientSeparatorIndex(value: String): Int {
    var inQuotes = false
    var inBrackets = false
    var last = -1
    value.forEachIndexed { index, ch ->
        when {
            ch == '"' -> inQuotes = !inQuotes
            ch == '<' && !inQuotes -> inBrackets = true
            ch == '>' && !inQuotes -> inBrackets = false
            ch == ',' && !inQuotes && !inBrackets -> last = index
        }
    }
    return last
}

// Split on recipient-separating commas only, preserving raw (untrimmed)
// segments so callers keep their own whitespace semantics.
fun splitRecipientEntries(value: String): List<String> {
    val parts = mutableListOf<String>()
    var start = 0
    var inQuotes = false
    var inBrackets = false
    value.forEachIndexed { index, ch ->
        when {
            ch == '"' -> {
                inQuotes = !inQuotes
            }

            ch == '<' && !inQuotes -> {
                inBrackets = true
            }

            ch == '>' && !inQuotes -> {
                inBrackets = false
            }

            ch == ',' && !inQuotes && !inBrackets -> {
                parts.add(value.substring(start, index))
                start = index + 1
            }
        }
    }
    parts.add(value.substring(start))
    return parts
}

fun replaceRecipientTail(
    value: String,
    contact: ContactSuggestion,
): String {
    val index = lastRecipientSeparatorIndex(value)
    val head = if (index < 0) "" else value.substring(0, index + 1)
    val prefix = if (head.isBlank()) "" else "$head "
    return "$prefix${formatContactSuggestion(contact)}, "
}

fun recipientTail(value: String): String {
    val index = lastRecipientSeparatorIndex(value)
    return (if (index < 0) value else value.substring(index + 1)).trim()
}

fun accountSendIdentities(account: AccountSummary): List<SendIdentity> {
    val primary =
        SendIdentity(
            accountId = account.id,
            email = account.email,
            name = account.senderName,
        )
    val aliases =
        account.aliases.map { alias ->
            SendIdentity(
                accountId = account.id,
                email = alias.email,
                name = alias.name.ifBlank { account.senderName },
            )
        }
    return (listOf(primary) + aliases).filter { it.email.isNotBlank() }
}

fun ownAddressList(accounts: List<AccountSummary>): List<String> =
    accounts
        .flatMap { account -> listOf(account.email) + account.aliases.map { it.email } }
        .map { it.trim().lowercase() }
        .filter { it.isNotBlank() }
        .distinct()

fun formatSendIdentity(identity: SendIdentity): String =
    if (identity.name.isNotBlank()) {
        "${identity.name} <${identity.email}>"
    } else {
        identity.email
    }

fun detectReplyFromIdentity(
    message: MessageBody,
    account: AccountSummary,
): String {
    val recipients =
        splitAddressList(listOf(message.to, message.cc).filter { it.isNotBlank() }.joinToString(", "))
            .map { bareAddress(it).lowercase() }
            .filter { it.isNotBlank() }
            .toSet()
    val match =
        accountSendIdentities(account).firstOrNull { identity ->
            recipients.contains(identity.email.trim().lowercase())
        } ?: return ""
    return if (match.email.equals(account.email, ignoreCase = true)) "" else match.email
}

fun forwardedSubject(subject: String): String {
    val trimmed = subject.trim()
    return when {
        trimmed.isBlank() -> "Fwd: (no subject)"
        trimmed.startsWith("Fwd:", ignoreCase = true) || trimmed.startsWith("Fw:", ignoreCase = true) -> trimmed
        else -> "Fwd: $trimmed"
    }
}

fun forwardedPlainBody(message: MessageBody): String {
    val headers =
        listOf(
            "---------- Forwarded message ---------",
            headerLine("From", message.from.ifBlank { message.fromAddr }),
            headerLine("Subject", message.subject.ifBlank { "(no subject)" }),
            headerLine("To", message.to),
            headerLine("Cc", message.cc),
        ).filter { it.isNotBlank() }
    return "\n\n${headers.joinToString("\n")}\n\n${message.body}"
}

fun forwardableAttachments(message: MessageBody): List<MessageAttachment> =
    message.attachments.filter { attachment ->
        val key = attachment.key.trim()
        key.isNotBlank() && attachment.url.isBlank() && !message.bodyHtml.contains("/media/$key")
    }

// Attachments to list (with a Save row) below the message body. Excludes
// attachments already rendered inline in bodyHtml, which would otherwise show
// up twice — once inline, once as a redundant row. Mail inlines images via
// cid: (rewritten to "/media/<key>"); feed items keep their original remote
// image URL in the html instead, so both references are checked.
fun standaloneAttachments(message: MessageBody): List<MessageAttachment> =
    message.attachments.filter { attachment ->
        val key = attachment.key.trim()
        val referencedByKey = key.isNotBlank() && message.bodyHtml.contains("/media/$key")
        val url = attachment.url.trim()
        val referencedByUrl = url.isNotBlank() && message.bodyHtml.contains(url)
        !referencedByKey && !referencedByUrl
    }

fun attachmentMediaRef(attachment: MessageAttachment): String {
    val key = attachment.key.trim()
    if (key.isNotBlank()) return "/media/$key"
    return attachment.url.trim()
}

fun buildThreadGalleryImages(messages: List<MessageBody>): List<ThreadGalleryImage> =
    messages.flatMap { message ->
        message.attachments
            .filter { it.mimeType.startsWith("image/") }
            .mapNotNull { attachment ->
                val ref = attachmentMediaRef(attachment)
                if (ref.isBlank()) {
                    null
                } else {
                    ThreadGalleryImage(
                        attachment = attachment,
                        ref = ref,
                        filename = attachment.filename.ifBlank { "Image" },
                        messageId = message.id,
                    )
                }
            }
    }

fun buildThreadMediaItems(messages: List<MessageBody>): List<ThreadMediaItem> {
    var imageIndex = 0
    val items =
        messages.flatMap { message ->
            message.attachments.mapNotNull { attachment ->
                val mime = attachment.mimeType
                val type =
                    when {
                        mime.startsWith("image/") -> "image"
                        mime.startsWith("video/") -> "video"
                        else -> return@mapNotNull null
                    }
                val ref = attachmentMediaRef(attachment)
                if (ref.isBlank()) return@mapNotNull null
                val galleryIndex = if (type == "image") imageIndex++ else null
                ThreadMediaItem(
                    attachment = attachment,
                    ref = ref,
                    filename = attachment.filename.ifBlank { if (type == "image") "Image" else "Video" },
                    type = type,
                    galleryIndex = galleryIndex,
                )
            }
        }
    return items.asReversed()
}

fun attachmentToDraftAttachment(
    attachment: MessageAttachment,
    dataBase64: String,
): DraftAttachment =
    DraftAttachment(
        id = attachment.key.ifBlank { attachment.filename },
        displayName = attachment.filename,
        mimeType = attachment.mimeType.ifBlank { "application/octet-stream" },
        sizeBytes = attachment.sizeBytes,
        dataBase64 = dataBase64,
    )

fun messageForwardDraft(
    message: MessageBody,
    attachments: List<DraftAttachment> = emptyList(),
): ComposeDraft =
    ComposeDraft(
        to = "",
        cc = "",
        bcc = "",
        subject = forwardedSubject(message.subject),
        body = forwardedPlainBody(message),
        attachments = attachments,
    )

fun messageEditAsNewDraft(
    message: MessageBody,
    attachments: List<DraftAttachment> = emptyList(),
): ComposeDraft =
    ComposeDraft(
        to = message.to,
        cc = message.cc,
        bcc = message.bcc,
        subject = message.subject,
        body = message.body,
        attachments = attachments,
    )

private fun headerLine(
    label: String,
    value: String,
): String {
    val trimmed = value.trim()
    return if (trimmed.isBlank()) "" else "$label: $trimmed"
}

fun buildReplyRecipients(
    message: MessageBody,
    ownAddresses: List<String> = emptyList(),
): ReplyRecipients {
    val own = ownAddresses.map { it.trim().lowercase() }.filter { it.isNotBlank() }.toSet()
    val isOwnSender = own.contains(message.fromAddr.trim().lowercase())
    val toSource =
        if (isOwnSender) {
            message.to
        } else {
            message.replyTo.ifBlank { message.fromAddr }.ifBlank { message.from }
        }
    val toList =
        splitAddressList(toSource).filter { entry ->
            val addr = bareAddress(entry).lowercase()
            addr.isNotBlank() && !own.contains(addr)
        }
    val toAddrs = toList.map { bareAddress(it).lowercase() }.toSet()
    val ccList =
        splitAddressList(message.cc).filter { entry ->
            val addr = bareAddress(entry).lowercase()
            addr.isNotBlank() && !own.contains(addr) && !toAddrs.contains(addr)
        }
    return ReplyRecipients(
        to = toList.joinToString(", "),
        cc = ccList.joinToString(", "),
    )
}

fun splitAddressList(value: String): List<String> {
    val entries = mutableListOf<String>()
    var quote: Char? = null
    var angleDepth = 0
    var start = 0
    value.forEachIndexed { index, ch ->
        when {
            quote != null -> {
                if (ch == quote) quote = null
            }

            ch == '"' || ch == '\'' -> {
                quote = ch
            }

            ch == '<' -> {
                angleDepth += 1
            }

            ch == '>' && angleDepth > 0 -> {
                angleDepth -= 1
            }

            ch == ',' && angleDepth == 0 -> {
                value
                    .substring(start, index)
                    .trim()
                    .takeIf { it.isNotBlank() }
                    ?.let { entries += it }
                start = index + 1
            }
        }
    }
    value
        .substring(start)
        .trim()
        .takeIf { it.isNotBlank() }
        ?.let { entries += it }
    return entries
}

fun bareAddress(value: String): String {
    val trimmed = value.trim()
    val insideAngles =
        trimmed
            .substringAfter('<', missingDelimiterValue = "")
            .substringBefore('>')
            .trim()
    return insideAngles.ifBlank { trimmed }.trim().trim('"', '\'')
}
