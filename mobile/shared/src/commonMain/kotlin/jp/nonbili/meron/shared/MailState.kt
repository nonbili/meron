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
    /** The IMAP/SMTP login, which is not always the address. */
    val username: String = "",
    val imapHost: String = "",
    val imapPort: Int = 0,
    val smtpHost: String = "",
    val smtpPort: Int = 0,
    val tls: Boolean = true,
    val starttls: Boolean = false,
    val smtpTls: Boolean = true,
    val smtpStarttls: Boolean = false,
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
    val proxy: ProxySpec = ProxySpec.followApp,
    val signature: SignatureSpec = SignatureSpec.followApp,
)

/**
 * An account's signature choice. [mode] is "global" (follow the app-wide
 * signature, the default), "none" (send no signature) or "custom" (use [html]).
 * The html is kept across mode changes so switching away and back does not lose
 * what the user wrote.
 */
data class SignatureSpec(
    val mode: String = "global",
    val html: String = "",
) {
    companion object {
        val followApp = SignatureSpec()
    }
}

/**
 * A proxy endpoint, used for both the app-wide setting and a per-account
 * override. [mode] is "off"/"http"/"socks5" app-wide, and adds "global" (follow
 * the app proxy, the default) and "direct" (never proxy) per account. An empty
 * [username] means the proxy needs no authentication.
 */
data class ProxySpec(
    val mode: String = "off",
    val host: String = "",
    val port: Int = 0,
    val username: String = "",
    val password: String = "",
) {
    /** Whether this is filled in well enough for the core to actually use it. */
    val usable: Boolean
        get() = (mode == "http" || mode == "socks5") && host.isNotBlank() && port > 0

    companion object {
        val off = ProxySpec()
        val followApp = ProxySpec(mode = "global")
    }
}

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
    /** Wire name: what SELECT, sync and cached rows address the mailbox by. */
    val name: String,
    val unread: Int = 0,
    val role: String = "folder",
    /** `name` decoded from modified UTF-7; equal to `name` for ASCII folders. */
    val displayName: String = name,
    /** Hierarchy separator the server reported for this mailbox, if any. */
    val delimiter: String = "",
)

data class ThreadSummary(
    val id: String,
    val accountId: String,
    val folder: String,
    val folderRole: String = "folder",
    val subject: String,
    val sender: String,
    val preview: String = "",
    val unread: Boolean = false,
    val unreadCount: Int = 0,
    /** Total messages in the thread, read or not; 0 when the core did not group. */
    val messageCount: Int = 0,
    val starred: Boolean = false,
    val hasStarredItems: Boolean = false,
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
    val folderRole: String = "folder",
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
    // Set for images the draft's HTML references as `cid:<inlineId>`; the core
    // builds them as inline MIME parts rather than plain attachments.
    val inlineId: String = "",
)

// `html` is the optional rich alternative sent alongside the plain `body`. The
// mobile composer edits plain text only, so it is populated just for forwards,
// where dropping the original's HTML part would strip the message down to its
// (often empty) text alternative.
data class ComposeDraft(
    val to: String = "",
    val cc: String = "",
    val bcc: String = "",
    val subject: String = "",
    val body: String = "",
    val attachments: List<DraftAttachment> = emptyList(),
    val html: String = "",
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
    val suffix = Random.nextLong(0, Long.MAX_VALUE).toString(36)
    return "local-draft-$suffix"
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

// Whether a message lives in the account's inbox — i.e. it was delivered to us.
// IMAP names the inbox "INBOX" regardless of locale, so a name check is enough.
fun folderIsInbox(folder: String): Boolean = folder.trim().equals("INBOX", ignoreCase = true)

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

/** Whether an account id names an RSS account. Core mints them as "rss-<uuid>"
 *  and the desktop bridge routes on the same prefix. Unlike
 *  [accountSummaryIsRss] this needs no account row, so it works in a background
 *  receiver that has only the ids a notification carried. */
fun accountIdIsRss(accountId: String): Boolean = accountId.startsWith("rss-")

/**
 * The mailbox a mail thread id names, or "" when [threadId] carries none (an
 * RSS thread, or an id the core did not mint). Mail ids are
 * `account#folder#key`, where the key is a UID or base64 and so never contains
 * "#" while a mailbox name can, which makes the folder everything between the
 * first and last separator — the same split the core's `parse_thread_id` does.
 *
 * The card's own folder has to come from here rather than from a `folder`
 * field, which the UI overwrites with a Kanban column id when a thread is
 * opened from a column.
 */
fun mailThreadIdFolder(threadId: String): String {
    if (threadIdIsRss(threadId)) return ""
    val first = threadId.indexOf('#')
    val last = threadId.lastIndexOf('#')
    if (first <= 0 || last <= first) return ""
    return threadId.substring(first + 1, last)
}

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

// Separates the user's own text from the quoted original in a forward draft.
// Both body variants carry it, and the send path locates the quote by it.
const val FORWARD_QUOTE_MARKER = "---------- Forwarded message ---------"

// Wrapper the HTML quote block opens with. Matches the desktop composer's markup
// so a forward looks the same whichever client wrote it, and lets a reopened
// draft find where its quote begins.
private const val FORWARD_HTML_OPEN = "<p><br></p><div class=\"meron-forwarded-message\">"

// `bodyHtml` is prepared for the reader by the core: it is wrapped in a complete
// document and gets a CSP plus reader-only image styles injected into its head.
// A forward must carry the message content, not those presentation/security
// additions. Preserve original head styles because many HTML mails rely on them,
// then return a fragment that can safely sit inside the forward quote.
private val READER_CHARSET_META = Regex("""(?is)<meta\s+charset="utf-8"\s*/?>""")
private val READER_CSP_META =
    Regex("""(?is)<meta\s+http-equiv="Content-Security-Policy"\s+content="[^"]*"\s*/?>""")
private val READER_IMAGE_STYLE =
    Regex(
        """(?is)<style>\s*img,video\{max-width:100%;height:auto\}\s*img\{cursor:zoom-in\}\s*</style>""",
    )
private val HTML_HEAD = Regex("""(?is)<head\b[^>]*>(.*?)</head\s*>""")
private val HTML_BODY = Regex("""(?is)<body\b[^>]*>(.*?)</body\s*>""")
private val HTML_STYLE = Regex("""(?is)<style\b[^>]*>.*?</style\s*>""")

private fun forwardableHtmlSource(html: String): String {
    val cleaned =
        html
            .replace(READER_CHARSET_META, "")
            .replace(READER_CSP_META, "")
            .replace(READER_IMAGE_STYLE, "")
    val body = HTML_BODY.find(cleaned)?.groupValues?.get(1) ?: return cleaned
    val styles =
        HTML_HEAD
            .find(cleaned)
            ?.groupValues
            ?.get(1)
            ?.let { head -> HTML_STYLE.findAll(head).joinToString("") { it.value } }
            .orEmpty()
    return styles + body
}

fun forwardedPlainBody(
    message: MessageBody,
    dateLabel: String = "",
): String {
    val headers =
        listOf(
            FORWARD_QUOTE_MARKER,
            headerLine("From", message.from.ifBlank { message.fromAddr }),
            headerLine("Date", dateLabel),
            headerLine("Subject", message.subject.ifBlank { "(no subject)" }),
            headerLine("To", message.to),
            headerLine("Cc", message.cc),
        ).filter { it.isNotBlank() }
    return "\n\n${headers.joinToString("\n")}\n\n${message.body}"
}

private fun escapeHtml(text: String): String =
    text
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")

// Plain text as HTML paragraphs: blank-line-separated blocks become <p>, single
// newlines become <br>. Mirrors the desktop composer so both clients quote a
// text-only original the same way.
fun plainTextToHtml(text: String): String {
    if (text.isBlank()) return ""
    return text
        .split(Regex("\n{2,}"))
        .joinToString("") { paragraph ->
            "<p>" + paragraph.split("\n").joinToString("<br>") { escapeHtml(it) } + "</p>"
        }
}

// An image the forwarded HTML shows inline. The reader rewrote the original's
// `cid:` refs to `/media/<key>` for local display, so forwarding that HTML as-is
// would point the recipient at this device's media paths: each one has to go
// back out as a `cid:` part with its bytes attached.
data class ForwardInlineImage(
    val attachment: MessageAttachment,
    val inlineId: String,
)

fun forwardInlineImages(message: MessageBody): List<ForwardInlineImage> =
    message.attachments
        .filter { it.key.trim().isNotBlank() && message.bodyHtml.contains("/media/${it.key.trim()}") }
        .mapIndexed { index, attachment ->
            ForwardInlineImage(attachment, "meron-forward-$index@meron")
        }

fun rewriteMediaRefsToCid(
    html: String,
    images: List<ForwardInlineImage>,
): String =
    images.fold(html) { acc, image ->
        acc.replace("/media/${image.attachment.key.trim()}", "cid:${image.inlineId}")
    }

private fun rewriteForwardMediaRefs(
    html: String,
    allImages: List<ForwardInlineImage>,
    availableImages: List<ForwardInlineImage>,
): String =
    allImages.fold(rewriteMediaRefsToCid(html, availableImages)) { acc, image ->
        // Never expose this device's cache paths to the recipient when an
        // inline part could not be read and therefore could not be rebuilt.
        acc.replace("/media/${image.attachment.key.trim()}", "")
    }

fun forwardedHtmlBody(
    message: MessageBody,
    dateLabel: String = "",
    inlineImages: List<ForwardInlineImage> = emptyList(),
): String {
    val quoted = message.bodyHtml.takeIf { it.isNotBlank() }?.let(::forwardableHtmlSource) ?: plainTextToHtml(message.body)
    if (quoted.isBlank()) return ""
    val rows =
        listOf(
            "From" to message.from.ifBlank { message.fromAddr },
            "Date" to dateLabel,
            "Subject" to message.subject.ifBlank { "(no subject)" },
            "To" to message.to,
            "Cc" to message.cc,
        ).filter { it.second.isNotBlank() }
    val header =
        rows.joinToString("") { (label, value) ->
            "<div><strong>${escapeHtml(label)}:</strong> ${escapeHtml(value.trim())}</div>"
        }
    val body = rewriteForwardMediaRefs(quoted, forwardInlineImages(message), inlineImages)
    return "$FORWARD_HTML_OPEN<p>$FORWARD_QUOTE_MARKER</p>$header<br>$body</div>"
}

// Assemble the HTML alternative for a forward at send time. The mobile composer
// only edits plain text, so the user's own words live above the quote marker in
// `body`; they are converted to HTML and prepended to the quote block. Returns ""
// when the marker is gone (the user deleted or rewrote the quote): there is then
// no reliable boundary between their text and the quote, and sending plain-only
// beats sending a body that no longer matches what they typed.
fun forwardHtmlForSend(
    body: String,
    forwardedHtml: String,
): String {
    if (forwardedHtml.isBlank()) return ""
    val marker = body.indexOf(FORWARD_QUOTE_MARKER)
    if (marker < 0) return ""
    return plainTextToHtml(body.substring(0, marker).trim()) + forwardedHtml
}

// Recover the quote block from a saved forward draft's HTML, so reopening one and
// sending it keeps the rich original instead of silently downgrading to plain.
fun forwardedHtmlQuote(bodyHtml: String): String {
    val source = forwardableHtmlSource(bodyHtml)
    val start = source.indexOf(FORWARD_HTML_OPEN)
    return if (start < 0) "" else source.substring(start)
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

fun inlineImageToDraftAttachment(
    image: ForwardInlineImage,
    dataBase64: String,
): DraftAttachment = attachmentToDraftAttachment(image.attachment, dataBase64).copy(inlineId = image.inlineId)

fun messageForwardDraft(
    message: MessageBody,
    attachments: List<DraftAttachment> = emptyList(),
    dateLabel: String = "",
    inlineImages: List<ForwardInlineImage> = emptyList(),
): ComposeDraft =
    ComposeDraft(
        to = "",
        cc = "",
        bcc = "",
        subject = forwardedSubject(message.subject),
        body = forwardedPlainBody(message, dateLabel),
        attachments = attachments,
        // Only quote HTML when the original had an HTML part; a text-only mail
        // stays a text-only forward rather than gaining a synthesized one.
        html = if (message.bodyHtml.isBlank()) "" else forwardedHtmlBody(message, dateLabel, inlineImages),
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

// Recipient summary for an outgoing bubble header ("to nonbili/meron, Comment").
// Display name when the address carries one, otherwise the local part. To and Cc
// are merged and de-duplicated: an outgoing reply and an outgoing forward can
// carry the same subject and the same text, so who received it is what tells
// them apart. Mirrors the desktop bubble header.
fun formatRecipientSummary(vararg lists: String): String {
    val seen = mutableSetOf<String>()
    val names = mutableListOf<String>()
    for (list in lists) {
        for (entry in splitAddressList(list)) {
            val address = bareAddress(entry)
            val key = address.lowercase()
            if (key.isBlank() || !seen.add(key)) continue
            val displayName =
                entry
                    .substringBefore('<', missingDelimiterValue = "")
                    .trim()
                    .trim('"', '\'')
                    .trim()
            names += displayName.ifBlank { address.substringBefore('@') }
        }
    }
    return names.joinToString(", ")
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
