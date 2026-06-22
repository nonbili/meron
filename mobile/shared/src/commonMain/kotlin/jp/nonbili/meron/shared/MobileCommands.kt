package jp.nonbili.meron.shared

object MobileCommand {
    const val AccountList = "account.list"
    const val AccountAddPassword = "account.addPassword"
    const val AccountAddOAuth = "account.addOAuth"
    const val AccountExchangeOAuthCode = "account.exchangeOAuthCode"
    const val AccountAddRss = "account.addRss"
    const val FeedAdd = "feed.add"
    const val FeedRemove = "feed.remove"
    const val FeedMove = "feed.move"
    const val FolderList = "mail.folderList"
    const val ThreadList = "mail.threadList"
    const val StarredItems = "mail.starredItems"
    const val ThreadRead = "mail.threadRead"
    const val Sync = "mail.sync"
    const val Send = "mail.send"
    const val Archive = "mail.archive"
    const val Delete = "mail.delete"
    const val MarkRead = "mail.markRead"
    const val MarkStarred = "mail.markStarred"
    const val RssSync = "rss.sync"
    const val RssThread = "rss.thread"
    const val RssMarkRead = "rss.markRead"
    const val RssMarkStarred = "rss.markStarred"
}

data class AddRssAccountParams(
    val feedUrl: String,
    val displayName: String = "",
) {
    fun toJson(): String {
        return jsonObject(
            "feed_url" to feedUrl.jsonString(),
            "display_name" to displayName.jsonString(),
        )
    }
}

data class AddPasswordAccountParams(
    val email: String,
    val displayName: String = "",
    val senderName: String = "",
    val imapHost: String,
    val imapPort: Int = 993,
    val smtpHost: String,
    val smtpPort: Int = 465,
    val username: String,
    val password: String,
    val tls: Boolean = true,
) {
    fun toJson(): String {
        return jsonObject(
            "email" to email.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "imap_host" to imapHost.jsonString(),
            "imap_port" to imapPort.toString(),
            "smtp_host" to smtpHost.jsonString(),
            "smtp_port" to smtpPort.toString(),
            "username" to username.jsonString(),
            "password" to password.jsonString(),
            "tls" to tls.toString(),
        )
    }
}

data class AddOAuthAccountParams(
    val email: String,
    val provider: String,
    val displayName: String = "",
    val senderName: String = "",
    val username: String = "",
    val avatarUrl: String = "",
    val accessToken: String = "",
    val refreshToken: String,
    val tokenExpiresAt: Long = 0,
    val imapHost: String = "",
    val imapPort: Int? = null,
    val smtpHost: String = "",
    val smtpPort: Int? = null,
) {
    fun toJson(): String {
        return jsonObject(
            "email" to email.jsonString(),
            "provider" to provider.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "username" to username.takeIf { it.isNotBlank() }?.jsonString(),
            "avatar_url" to avatarUrl.takeIf { it.isNotBlank() }?.jsonString(),
            "access_token" to accessToken.takeIf { it.isNotBlank() }?.jsonString(),
            "refresh_token" to refreshToken.jsonString(),
            "token_expires_at" to tokenExpiresAt.toString(),
            "imap_host" to imapHost.takeIf { it.isNotBlank() }?.jsonString(),
            "imap_port" to imapPort?.toString(),
            "smtp_host" to smtpHost.takeIf { it.isNotBlank() }?.jsonString(),
            "smtp_port" to smtpPort?.toString(),
        )
    }
}

data class ExchangeOAuthCodeParams(
    val email: String,
    val provider: String,
    val displayName: String = "",
    val senderName: String = "",
    val code: String,
    val clientId: String,
    val clientSecret: String = "",
    val redirectUri: String,
    val codeVerifier: String,
) {
    fun toJson(): String {
        return jsonObject(
            "email" to email.jsonString(),
            "provider" to provider.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "code" to code.jsonString(),
            "client_id" to clientId.jsonString(),
            "client_secret" to clientSecret.takeIf { it.isNotBlank() }?.jsonString(),
            "redirect_uri" to redirectUri.jsonString(),
            "code_verifier" to codeVerifier.jsonString(),
        )
    }
}

data class AddRssFeedParams(
    val accountId: String,
    val feedUrl: String,
) {
    fun toJson(): String {
        return jsonObject(
            "account" to accountId.jsonString(),
            "feed_url" to feedUrl.jsonString(),
        )
    }
}

data class RemoveRssFeedParams(
    val threadId: String,
) {
    fun toJson(): String = jsonObject("thread_id" to threadId.jsonString())
}

data class MoveRssFeedParams(
    val threadId: String,
    val targetAccountId: String,
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "target_account" to targetAccountId.jsonString(),
        )
    }
}

data class FolderListParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("account_id" to accountId.jsonString())
}

data class ThreadListParams(
    val accountId: String,
    val folderId: String = "inbox",
    val query: String = "",
    val filter: String = "all",
    val beforeCursor: String? = null,
    val refresh: Boolean = false,
) {
    fun toJson(): String {
        return jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
            "query" to query.jsonString(),
            "filter" to filter.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "refresh" to refresh.toString(),
        )
    }
}

data class ThreadReadParams(
    val threadId: String,
    val beforeCursor: String? = null,
    val limit: Int? = null,
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "limit" to limit?.toString(),
        )
    }
}

data class SyncMailParams(
    val accountId: String,
    val folderId: String = "inbox",
    val limit: Int = 50,
    val folders: Boolean = true,
) {
    fun toJson(): String {
        return jsonObject(
            "account_id" to accountId.jsonString(),
            "folder_id" to folderId.jsonString(),
            "limit" to limit.toString(),
            "folders" to folders.toString(),
        )
    }
}

data class SyncRssParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("account_id" to accountId.jsonString())
}

data class RssThreadParams(
    val threadId: String,
    val beforeCursor: String? = null,
    val limit: Int? = null,
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "limit" to limit?.toString(),
        )
    }
}

data class MobileAttachmentInput(
    val filename: String,
    val mime: String,
    val data: String,
    val inlineId: String = "",
) {
    fun toJson(): String {
        return jsonObject(
            "filename" to filename.jsonString(),
            "mime" to mime.jsonString(),
            "data" to data.jsonString(),
            "inline_id" to inlineId.jsonString(),
        )
    }
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
) {
    fun toJson(): String {
        return jsonObject(
            "account_id" to accountId.jsonString(),
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
}

fun ComposeDraft.toSendMailParams(accountId: String, from: String = ""): SendMailParams {
    return SendMailParams(
        accountId = accountId,
        from = from,
        to = to,
        cc = cc,
        bcc = bcc,
        subject = subject,
        body = body,
        attachments = attachments.map {
            MobileAttachmentInput(
                filename = it.displayName,
                mime = it.mimeType,
                data = it.dataBase64,
            )
        },
    )
}

fun MessageBody.toReplyMailParams(accountId: String, body: String): SendMailParams {
    val recipient = replyTo.ifBlank { fromAddr }.ifBlank { from }
    val replySubject = if (subject.startsWith("Re:", ignoreCase = true)) subject else "Re: $subject"
    val parentMessageId = messageId.trim().trim('<', '>')
    val parentReference = parentMessageId.takeIf { it.isNotBlank() }?.let { "<$it>" }.orEmpty()
    val nextReferences = listOf(references.trim(), parentReference)
        .filter { it.isNotBlank() }
        .joinToString(" ")
    return SendMailParams(
        accountId = accountId,
        to = recipient,
        subject = replySubject,
        body = body,
        inReplyTo = parentReference,
        references = nextReferences,
    )
}

data class ThreadActionParams(
    val threadId: String,
    val folderId: String? = null,
    val messageIds: List<String> = emptyList(),
) {
    fun archiveJson(): String = jsonObject("thread_id" to threadId.jsonString())

    fun deleteJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "folder" to folderId?.jsonString(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
    }
}

data class MarkReadParams(
    val threadId: String,
    val seen: Boolean = true,
    val messageIds: List<String> = emptyList(),
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "seen" to seen.toString(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
    }
}

data class MarkStarredParams(
    val threadId: String,
    val starred: Boolean,
    val messageIds: List<String> = emptyList(),
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "starred" to starred.toString(),
            "message_ids" to messageIds.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
    }
}

data class RssMarkReadParams(
    val threadId: String,
    val seen: Boolean = true,
    val itemKeys: List<String> = emptyList(),
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "seen" to seen.toString(),
            "item_keys" to itemKeys.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
    }
}

data class RssMarkStarredParams(
    val threadId: String,
    val starred: Boolean,
    val itemKeys: List<String> = emptyList(),
) {
    fun toJson(): String {
        return jsonObject(
            "thread_id" to threadId.jsonString(),
            "starred" to starred.toString(),
            "item_keys" to itemKeys.takeUnless { it.isEmpty() }?.jsonStringArray(),
        )
    }
}

class MobileMailCommandClient(
    private val core: MeronCore,
) {
    suspend fun assertProtocol() {
        requireProtocolVersion(core.protocolVersion())
    }

    suspend fun listAccounts(): String = core.invoke(MobileCommand.AccountList)

    suspend fun addPasswordAccount(params: AddPasswordAccountParams): String {
        return core.invoke(MobileCommand.AccountAddPassword, params.toJson())
    }

    suspend fun addOAuthAccount(params: AddOAuthAccountParams): String {
        return core.invoke(MobileCommand.AccountAddOAuth, params.toJson())
    }

    suspend fun exchangeOAuthCode(params: ExchangeOAuthCodeParams): String {
        return core.invoke(MobileCommand.AccountExchangeOAuthCode, params.toJson())
    }

    suspend fun addRssAccount(params: AddRssAccountParams): String {
        return core.invoke(MobileCommand.AccountAddRss, params.toJson())
    }

    suspend fun addRssFeed(params: AddRssFeedParams): String {
        return core.invoke(MobileCommand.FeedAdd, params.toJson())
    }

    suspend fun removeRssFeed(params: RemoveRssFeedParams): String {
        return core.invoke(MobileCommand.FeedRemove, params.toJson())
    }

    suspend fun moveRssFeed(params: MoveRssFeedParams): String {
        return core.invoke(MobileCommand.FeedMove, params.toJson())
    }

    suspend fun listFolders(params: FolderListParams): String = core.invoke(MobileCommand.FolderList, params.toJson())

    suspend fun listThreads(params: ThreadListParams): String = core.invoke(MobileCommand.ThreadList, params.toJson())

    suspend fun listStarredItems(): String = core.invoke(MobileCommand.StarredItems)

    suspend fun readThread(params: ThreadReadParams): String = core.invoke(MobileCommand.ThreadRead, params.toJson())

    suspend fun sync(params: SyncMailParams): String = core.invoke(MobileCommand.Sync, params.toJson())

    suspend fun syncRss(params: SyncRssParams): String = core.invoke(MobileCommand.RssSync, params.toJson())

    suspend fun readRssThread(params: RssThreadParams): String = core.invoke(MobileCommand.RssThread, params.toJson())

    suspend fun markRssRead(params: RssMarkReadParams): String = core.invoke(MobileCommand.RssMarkRead, params.toJson())

    suspend fun markRssStarred(params: RssMarkStarredParams): String = core.invoke(MobileCommand.RssMarkStarred, params.toJson())

    suspend fun send(params: SendMailParams): String = core.invoke(MobileCommand.Send, params.toJson())

    suspend fun archive(params: ThreadActionParams): String = core.invoke(MobileCommand.Archive, params.archiveJson())

    suspend fun delete(params: ThreadActionParams): String = core.invoke(MobileCommand.Delete, params.deleteJson())

    suspend fun markRead(params: MarkReadParams): String = core.invoke(MobileCommand.MarkRead, params.toJson())

    suspend fun markStarred(params: MarkStarredParams): String = core.invoke(MobileCommand.MarkStarred, params.toJson())
}

fun accountListRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.AccountList)

fun accountAddPasswordRequest(id: Long = 1, params: AddPasswordAccountParams): CoreRequest {
    return CoreRequest(id, MobileCommand.AccountAddPassword, params.toJson())
}

fun accountAddOAuthRequest(id: Long = 1, params: AddOAuthAccountParams): CoreRequest {
    return CoreRequest(id, MobileCommand.AccountAddOAuth, params.toJson())
}

fun accountExchangeOAuthCodeRequest(id: Long = 1, params: ExchangeOAuthCodeParams): CoreRequest {
    return CoreRequest(id, MobileCommand.AccountExchangeOAuthCode, params.toJson())
}

fun accountAddRssRequest(id: Long = 1, params: AddRssAccountParams): CoreRequest {
    return CoreRequest(id, MobileCommand.AccountAddRss, params.toJson())
}

fun feedAddRequest(id: Long = 1, params: AddRssFeedParams): CoreRequest {
    return CoreRequest(id, MobileCommand.FeedAdd, params.toJson())
}

fun feedRemoveRequest(id: Long = 1, params: RemoveRssFeedParams): CoreRequest {
    return CoreRequest(id, MobileCommand.FeedRemove, params.toJson())
}

fun feedMoveRequest(id: Long = 1, params: MoveRssFeedParams): CoreRequest {
    return CoreRequest(id, MobileCommand.FeedMove, params.toJson())
}

fun folderListRequest(id: Long = 1, params: FolderListParams): CoreRequest {
    return CoreRequest(id, MobileCommand.FolderList, params.toJson())
}

fun threadListRequest(id: Long = 1, params: ThreadListParams): CoreRequest {
    return CoreRequest(id, MobileCommand.ThreadList, params.toJson())
}

fun threadReadRequest(id: Long = 1, params: ThreadReadParams): CoreRequest {
    return CoreRequest(id, MobileCommand.ThreadRead, params.toJson())
}

fun syncMailRequest(id: Long = 1, params: SyncMailParams): CoreRequest {
    return CoreRequest(id, MobileCommand.Sync, params.toJson())
}

fun syncRssRequest(id: Long = 1, params: SyncRssParams): CoreRequest {
    return CoreRequest(id, MobileCommand.RssSync, params.toJson())
}

fun rssThreadRequest(id: Long = 1, params: RssThreadParams): CoreRequest {
    return CoreRequest(id, MobileCommand.RssThread, params.toJson())
}

fun sendMailRequest(id: Long = 1, params: SendMailParams): CoreRequest {
    return CoreRequest(id, MobileCommand.Send, params.toJson())
}

fun archiveThreadRequest(id: Long = 1, params: ThreadActionParams): CoreRequest {
    return CoreRequest(id, MobileCommand.Archive, params.archiveJson())
}

fun deleteThreadRequest(id: Long = 1, params: ThreadActionParams): CoreRequest {
    return CoreRequest(id, MobileCommand.Delete, params.deleteJson())
}

fun markReadRequest(id: Long = 1, params: MarkReadParams): CoreRequest {
    return CoreRequest(id, MobileCommand.MarkRead, params.toJson())
}

fun markStarredRequest(id: Long = 1, params: MarkStarredParams): CoreRequest {
    return CoreRequest(id, MobileCommand.MarkStarred, params.toJson())
}

fun rssMarkReadRequest(id: Long = 1, params: RssMarkReadParams): CoreRequest {
    return CoreRequest(id, MobileCommand.RssMarkRead, params.toJson())
}

fun rssMarkStarredRequest(id: Long = 1, params: RssMarkStarredParams): CoreRequest {
    return CoreRequest(id, MobileCommand.RssMarkStarred, params.toJson())
}

private fun jsonObject(vararg fields: Pair<String, String?>): String {
    return fields
        .filter { it.second != null }
        .joinToString(separator = ",", prefix = "{", postfix = "}") { (name, value) -> "${name.jsonString()}:$value" }
}

private fun List<String>.jsonStringArray(): String {
    return joinToString(separator = ",", prefix = "[", postfix = "]") { it.jsonString() }
}
