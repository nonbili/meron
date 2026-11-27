package jp.nonbili.meron.shared

/**
 * The search options form, as the core's `search.parse` reads a query and
 * `search.build` writes it back. Dates are `yyyy-mm-dd`, or empty.
 */
data class SearchFields(
    val from: String = "",
    val to: String = "",
    val subject: String = "",
    val tags: String = "",
    val words: String = "",
    val after: String = "",
    val before: String = "",
    val unread: Boolean = false,
    val starred: Boolean = false,
    val hasAttachment: Boolean = false,
) {
    fun toJson(): String =
        jsonObject(
            "from" to from.jsonString(),
            "to" to to.jsonString(),
            "subject" to subject.jsonString(),
            "tags" to tags.jsonString(),
            "words" to words.jsonString(),
            "after" to after.jsonString(),
            "before" to before.jsonString(),
            "unread" to unread.toString(),
            "starred" to starred.toString(),
            "has_attachment" to hasAttachment.toString(),
        )
}

data class FolderListParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("account_id" to accountId.jsonString())
}

data class FolderCreateParams(
    val accountId: String,
    val name: String,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "name" to name.jsonString(),
        )
}

/**
 * Deletes a folder on the server with everything in it, subfolders included; the
 * core rejects special-use folders anywhere in that subtree.
 */
data class FolderDeleteParams(
    val accountId: String,
    val folderId: String,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
        )
}

/**
 * Renames a folder on the server, subfolders included. [name] is the folder's
 * new full path; the core rejects special-use folders like a delete.
 */
data class FolderRenameParams(
    val accountId: String,
    val folderId: String,
    val name: String,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
            "name" to name.jsonString(),
        )
}

data class FolderSetNotifyParams(
    val accountId: String,
    val folderId: String,
    val enabled: Boolean,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
            "enabled" to enabled.toString(),
        )
}

data class ContactSuggestParams(
    val accountId: String = "",
    val query: String = "",
    val limit: Int = 8,
    // frequent (autocomplete default), name, or recent.
    val sort: String = "frequent",
    val directory: Boolean = false,
    /** Only addresses no synced address book holds: "Other contacts". */
    val unsaved: Boolean = false,
) {
    fun toJson(): String =
        jsonObject(
            "account" to accountId.jsonString(),
            "query" to query.jsonString(),
            "limit" to limit.toString(),
            "sort" to if (sort == "frequent") null else sort.jsonString(),
            "directory" to if (directory) "true" else null,
            "unsaved" to if (unsaved) "true" else null,
        )
}

/** Exact-address, cache-only history across all accounts and folders. */
data class ContactThreadsParams(
    val addr: String,
    val beforeCursor: String? = null,
    val limit: Int = 50,
) {
    fun toJson(): String =
        jsonObject(
            "addr" to addr.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "limit" to limit.toString(),
        )
}

data class ThreadListParams(
    val accountId: String,
    val folderId: String = "inbox",
    val query: String = "",
    val filter: String = "all",
    // Keep only threads with attachments; combines with `filter`.
    val attachments: Boolean = false,
    val beforeCursor: String? = null,
    val refresh: Boolean = false,
    // Header rows to return, per account (the unified fan-out applies it to each
    // account's page). Omitted when null so the core keeps its own default; the
    // mailbox passes the depth it has already paged in so a background reload
    // does not shrink the list back to one page.
    val limit: Int? = null,
    // Unified only: which special-use folder each account answers from
    // ("inbox", "sent", "archive", …). Accounts whose server has no such
    // mailbox drop out of the merge. Omitted for a single account, whose
    // folderId already names a real mailbox.
    val folderRole: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
            "query" to query.jsonString(),
            "filter" to filter.jsonString(),
            "attachments" to attachments.toString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "refresh" to refresh.toString(),
            "limit" to limit?.toString(),
            "folder_role" to folderRole?.jsonString(),
        )
}

data class StarredItemsParams(
    val query: String = "",
    val filter: String = "all",
    val attachments: Boolean = false,
    val limit: Int = 50,
    val beforeCursor: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "query" to query.jsonString(),
            "filter" to filter.jsonString(),
            "attachments" to attachments.toString(),
            "limit" to limit.toString(),
            "before_cursor" to beforeCursor?.jsonString(),
        )
}

data class AllocateIdentityParams(
    val accountId: String,
    val draft: Boolean,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "draft" to draft.toString(),
        )
}

data class ThreadReadParams(
    val threadId: String,
    val beforeCursor: String? = null,
    val limit: Int? = CONVERSATION_PAGE_SIZE,
    val forPrint: Boolean = false,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "limit" to limit?.toString(),
            "for_print" to if (forPrint) "true" else null,
        )
}

data class AttachmentReadParams(
    val key: String,
) {
    fun toJson(): String = jsonObject("key" to key.jsonString())
}

data class SyncMailParams(
    val accountId: String,
    val folderId: String = "inbox",
    val limit: Int = 50,
    val folders: Boolean = true,
    // Foreground-only: lets the core run the non-gating sync tail (body
    // prefetch, Sent/Drafts headers) after returning, so the inbox list isn't
    // blocked on it. Background workers must leave this false — their
    // execution window ends when the call returns.
    val deferTail: Boolean = false,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
            "limit" to limit.toString(),
            "folders" to folders.toString(),
            "defer_tail" to deferTail.toString(),
        )
}

data class SyncRssParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("account_id" to accountId.jsonString())
}

data class RssThreadParams(
    val threadId: String,
    val beforeCursor: String? = null,
    val limit: Int? = CONVERSATION_PAGE_SIZE,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "limit" to limit?.toString(),
        )
}

data class MobileAttachmentInput(
    val filename: String,
    val mime: String,
    val data: String,
    val inlineId: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "filename" to filename.jsonString(),
            "mime" to mime.jsonString(),
            "data" to data.jsonString(),
            "inline_id" to inlineId.jsonString(),
        )
}

data class SendMailParams(
    val accountId: String,
    val to: String,
    val subject: String,
    val body: String,
    val from: String = "",
    val cc: String = "",
    val bcc: String = "",
    val replyTo: String = "",
    val html: String = "",
    val inReplyTo: String = "",
    val references: String = "",
    val messageId: String = "",
    val attachments: List<MobileAttachmentInput> = emptyList(),
    val encrypt: Boolean? = null,
    /** OpenPGP-sign a message that isn't encrypted; null leaves it to the account's default. */
    val sign: Boolean? = null,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "encrypt" to encrypt?.toString(),
            "plain" to encrypt?.not()?.toString(),
            "sign" to sign?.toString(),
            "from" to from.jsonString(),
            "to" to to.jsonString(),
            "cc" to cc.jsonString(),
            "bcc" to bcc.jsonString(),
            "reply_to" to replyTo.jsonString(),
            "subject" to subject.jsonString(),
            "body" to body.jsonString(),
            "html" to html.jsonString(),
            "in_reply_to" to inReplyTo.jsonString(),
            "references" to references.jsonString(),
            "message_id" to messageId.jsonString(),
            "attachments" to attachments.joinToString(separator = ",", prefix = "[", postfix = "]") { it.toJson() },
        )
}

data class SaveDraftParams(
    val accountId: String,
    val draftId: String,
    val to: String,
    val subject: String,
    val body: String,
    val from: String = "",
    val cc: String = "",
    val bcc: String = "",
    val replyTo: String = "",
    val html: String = "",
    val inReplyTo: String = "",
    val references: String = "",
    val attachments: List<MobileAttachmentInput> = emptyList(),
    val encrypt: Boolean? = null,
    /** The composer's sign switch, recorded with the draft so reopening it keeps the choice. */
    val sign: Boolean? = null,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "encrypt" to encrypt?.toString(),
            "plain" to encrypt?.not()?.toString(),
            "sign" to sign?.toString(),
            "draft_id" to draftId.jsonString(),
            "from" to from.jsonString(),
            "to" to to.jsonString(),
            "cc" to cc.jsonString(),
            "bcc" to bcc.jsonString(),
            "reply_to" to replyTo.jsonString(),
            "subject" to subject.jsonString(),
            "body" to body.jsonString(),
            "html" to html.jsonString(),
            "in_reply_to" to inReplyTo.jsonString(),
            "references" to references.jsonString(),
            "attachments" to attachments.joinToString(separator = ",", prefix = "[", postfix = "]") { it.toJson() },
        )
}

data class DiscardDraftParams(
    val accountId: String,
    val draftId: String,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "draft_id" to draftId.jsonString(),
        )
}

fun ComposeDraft.toSendMailParams(
    accountId: String,
    from: String = "",
): SendMailParams =
    SendMailParams(
        accountId = accountId,
        from = from,
        to = to,
        cc = cc,
        bcc = bcc,
        subject = subject,
        body = body,
        html = html,
        attachments =
            attachments.map {
                MobileAttachmentInput(
                    filename = it.displayName,
                    mime = it.mimeType,
                    data = it.dataBase64,
                    inlineId = it.inlineId,
                )
            },
    )

fun ComposeDraft.toSaveDraftParams(
    accountId: String,
    draftId: String,
    from: String = "",
): SaveDraftParams =
    SaveDraftParams(
        accountId = accountId,
        draftId = draftId,
        from = from,
        to = to,
        cc = cc,
        bcc = bcc,
        subject = subject,
        body = body,
        html = html,
        attachments =
            attachments.map {
                MobileAttachmentInput(
                    filename = it.displayName,
                    mime = it.mimeType,
                    data = it.dataBase64,
                    inlineId = it.inlineId,
                )
            },
    )

fun MessageBody.toReplyMailParams(
    accountId: String,
    body: String,
    from: String = "",
    attachments: List<DraftAttachment> = emptyList(),
    replyAll: Boolean = false,
): SendMailParams {
    val recipients = buildReplyRecipients(this, replyAll)
    val replySubject = if (subject.startsWith("Re:", ignoreCase = true)) subject else "Re: $subject"
    val parentMessageId = messageId.trim().trim('<', '>')
    val parentReference = parentMessageId.takeIf { it.isNotBlank() }?.let { "<$it>" }.orEmpty()
    val nextReferences =
        listOf(references.trim(), parentReference)
            .filter { it.isNotBlank() }
            .joinToString(" ")
    return SendMailParams(
        accountId = accountId,
        from = from,
        to = recipients.to,
        cc = recipients.cc,
        subject = replySubject,
        body = body,
        inReplyTo = parentReference,
        references = nextReferences,
        attachments =
            attachments.map {
                MobileAttachmentInput(
                    filename = it.displayName,
                    mime = it.mimeType,
                    data = it.dataBase64,
                )
            },
    )
}

// A reply with the original quoted under it (see [replyQuotedPlainBody]): the
// quote joins the plain body, and an HTML original's quote makes the reply's
// HTML alternative, with the user's text converted above it and the inline
// images the quote references attached. A body that already quotes the message
// — a saved quoted reply, reopened — is not quoted twice: one that still ends in
// the quote untouched keeps its HTML, and one whose quote the user edited (its
// [marker], the attribution line, still there) goes out exactly as written.
fun SendMailParams.withReplyQuote(
    quoteText: String,
    quoteHtml: String,
    marker: String,
    inlineImages: List<DraftAttachment> = emptyList(),
): SendMailParams {
    val quote = quoteText.trim()
    if (quote.isBlank()) return this
    val sent = body.trim()
    val quoted = sent.endsWith(quote)
    if (!quoted && marker.isNotBlank() && sent.contains(marker)) return this
    val own = if (quoted) sent.removeSuffix(quote).trim() else sent
    val html = if (quoteHtml.isBlank()) "" else plainTextToHtml(own) + quoteHtml
    return copy(
        body =
            when {
                quoted -> sent
                own.isBlank() -> quote
                else -> own + quoteText
            },
        html = html,
        attachments =
            if (html.isBlank()) {
                attachments
            } else {
                attachments + inlineImages.map { MobileAttachmentInput(it.displayName, it.mimeType, it.dataBase64, it.inlineId) }
            },
    )
}

data class ThreadActionParams(
    val threadId: String,
    val folderId: String? = null,
    val messageIds: List<String> = emptyList(),
) {
    fun archiveJson(): String = jsonObject("thread_id" to threadId.jsonString())

    fun deleteJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "folder" to folderId?.jsonString(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
}

data class MoveThreadParams(
    val threadId: String,
    val targetFolderId: String,
    val targetAccountId: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "target_folder_id" to targetFolderId.jsonString(),
            "target_account_id" to targetAccountId?.jsonString(),
        )
}

data class CopyThreadParams(
    val threadId: String,
    val targetAccountId: String,
    val targetFolderId: String,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "target_account_id" to targetAccountId.jsonString(),
            "target_folder_id" to targetFolderId.jsonString(),
        )
}

data class MarkReadParams(
    val threadId: String,
    val seen: Boolean = true,
    val messageIds: List<String> = emptyList(),
    val folderId: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "seen" to seen.toString(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
            "folder" to folderId?.jsonString(),
        )
}

data class MarkAllReadParams(
    val accountId: String,
    val folderId: String = "inbox",
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
        )
}

/** Permanently deletes a Trash or Junk folder; the core rejects any other role. */
data class EmptyFolderParams(
    val accountId: String,
    val folderId: String,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
        )
}

data class MarkStarredParams(
    val threadId: String,
    val starred: Boolean,
    val messageIds: List<String> = emptyList(),
    val folderId: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "starred" to starred.toString(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
            "folder" to folderId?.jsonString(),
        )
}

data class SetTagsParams(
    val threadId: String,
    val add: List<String> = emptyList(),
    val remove: List<String> = emptyList(),
    val messageIds: List<String> = emptyList(),
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "add" to add.jsonStringArray(),
            "remove" to remove.jsonStringArray(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
}

/**
 * One thing a message has to say, or not say, for a rule to take it. [field] is `from`, `to` or
 * `subject` (comma-separated alternatives, any of which matches), `body` or `subject_or_body` (one
 * piece of text, waiting for the body to download), `words` (search text, only from an older rule),
 * `tag`, or `attachment`, `unread` or `starred`, which take no [value].
 */
data class RuleCondition(
    val field: String = "from",
    val value: String = "",
    /** Matches the mail the condition otherwise wouldn't. */
    val negate: Boolean = false,
) {
    val valued: Boolean get() = this.field !in setOf("attachment", "unread", "starred")

    /** Whether this reads the body: such a rule waits for it, and can't be one for sent mail. */
    val readsBody: Boolean get() = this.field == "body" || this.field == "subject_or_body"

    fun toJson(): String =
        jsonObject(
            "field" to field.jsonString(),
            "value" to value.jsonString(),
            "negate" to negate.toString(),
        )
}

data class Rule(
    val id: String = "",
    /** None for all mail. */
    val conditions: List<RuleCondition> = emptyList(),
    /** Any one of [conditions] is enough, rather than all of them. */
    val matchAny: Boolean = false,
    val trigger: String = "arrival",
    val delaySeconds: Long = 0,
    val tags: List<String> = emptyList(),
    val markRead: Boolean = false,
    val star: Boolean = false,
    val moveTo: String = "",
    val folder: String = "",
    /** The account of [folder]; empty for the mail's own account. */
    val folderAccount: String = "",
    /** The accounts the rule runs on; empty for all of them. */
    val accounts: List<String> = emptyList(),
    val copyFolder: String = "",
    val copyAccount: String = "",
    /** Addresses to forward matching mail to, comma-separated; empty for none. */
    val forwardTo: String = "",
    /** Kept, but not run. */
    val disabled: Boolean = false,
    /** Runs on mail the user sent, as it shows up in Sent, instead of on mail that arrives. */
    val sent: Boolean = false,
) {
    fun shareSource(): String = jsonObject("meron_rule" to "1", "rule" to copy(id = "").toJson())

    fun toJson(): String =
        jsonObject(
            "id" to id.jsonString(),
            "conditions" to conditions.joinToString(separator = ",", prefix = "[", postfix = "]") { it.toJson() },
            "match_any" to matchAny.toString(),
            "tags" to tags.jsonStringArray(),
            "trigger" to trigger.jsonString(),
            "delay_seconds" to delaySeconds.toString(),
            "mark_read" to markRead.toString(),
            "star" to star.toString(),
            "move_to" to moveTo.jsonString(),
            "folder" to folder.jsonString(),
            "folder_account" to folderAccount.jsonString(),
            "accounts" to accounts.jsonStringArray(),
            "copy_folder" to copyFolder.jsonString(),
            "copy_account" to copyAccount.jsonString(),
            "forward_to" to forwardTo.jsonString(),
            "disabled" to disabled.toString(),
            "sent" to sent.toString(),
        )
}

/** One line of what rules did. [subject] is empty for a run by hand, which logs one line. */
data class RuleLogEntry(
    val at: Long = 0,
    val ruleId: String = "",
    val subject: String = "",
    val count: Int = 1,
    val failed: Boolean = false,
    val detail: String = "",
)

/** What rules did lately, and how many messages each rule run by hand has left. */
data class RuleStatus(
    val log: List<RuleLogEntry> = emptyList(),
    val running: Map<String, Int> = emptyMap(),
)

data class RssMarkReadParams(
    val threadId: String,
    val seen: Boolean = true,
    val itemKeys: List<String> = emptyList(),
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "seen" to seen.toString(),
            "item_keys" to itemKeys.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
}

data class RssMarkStarredParams(
    val threadId: String,
    val starred: Boolean,
    val itemKeys: List<String> = emptyList(),
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "starred" to starred.toString(),
            "item_keys" to itemKeys.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
}
