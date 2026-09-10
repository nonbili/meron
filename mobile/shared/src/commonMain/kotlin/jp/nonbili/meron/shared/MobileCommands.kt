package jp.nonbili.meron.shared

const val CONVERSATION_PAGE_SIZE = 10

object MobileCommand {
    const val AccountList = "account.list"
    const val AccountAddPassword = "account.addPassword"
    const val AccountAutodiscover = "account.autodiscover"
    const val AccountAddOAuth = "account.addOAuth"
    const val AccountExchangeOAuthCode = "account.exchangeOAuthCode"
    const val AccountUpdateOAuthToken = "account.updateOAuthToken"
    const val AccountAddRss = "account.addRss"
    const val AccountRemove = "account.remove"
    const val AccountSetName = "account.setName"
    const val AccountSetSenderName = "account.setSenderName"
    const val AccountSetAvatar = "account.setAvatar"
    const val AccountWriteAvatarFile = "account.writeAvatarFile"
    const val AccountSetChatWallpaper = "account.setChatWallpaper"
    const val AccountWriteChatWallpaperFile = "account.writeChatWallpaperFile"
    const val AccountSetImages = "account.setImages"
    const val AccountSetConversationHtml = "account.setConversationHtml"
    const val AccountSetUnified = "account.setUnified"
    const val AccountSetMuted = "account.setMuted"
    const val AccountSetPaused = "account.setPaused"
    const val AccountSetSaveSentCopy = "account.setSaveSentCopy"
    const val AccountSetRssSyncInterval = "account.setRSSSyncInterval"
    const val AccountSetAliases = "account.setAliases"
    const val AccountSetProxy = "account.setProxy"
    const val AccountProbeCert = "account.probeCert"
    const val AccountSetCertPin = "account.setCertPin"
    const val AccountSetSignature = "account.setSignature"
    const val AppPrefsGet = "app.prefsGet"
    const val AppPrefsSet = "app.prefsSet"
    const val AppProxyGet = "app.proxyGet"
    const val AppProxySet = "app.proxySet"
    const val AccountReorder = "account.reorder"
    const val FeedAdd = "feed.add"
    const val FeedRemove = "feed.remove"
    const val FeedMove = "feed.move"
    const val FeedExportOpml = "rss.exportOpml"
    const val FeedImportOpml = "rss.importOpml"
    const val BackupExport = "backup.export"
    const val BackupImport = "backup.import"
    const val FolderList = "mail.folderList"
    const val FolderCreate = "mail.folderCreate"
    const val FolderDelete = "mail.folderDelete"
    const val ContactSuggest = "mail.suggestContacts"
    const val ThreadList = "mail.threadList"
    const val StarredItems = "mail.starredItems"
    const val AllocateIdentity = "mail.allocateIdentity"
    const val ThreadRead = "mail.threadRead"
    const val AttachmentRead = "mail.attachmentRead"
    const val ChangelogFetch = "changelog.fetch"
    const val StorageUsage = "storage.usage"
    const val StorageClearCache = "storage.clearCache"
    const val Sync = "mail.sync"
    const val Send = "mail.send"
    const val SaveDraft = "mail.saveDraft"
    const val DiscardDraft = "mail.discardDraft"
    const val Archive = "mail.archive"
    const val Delete = "mail.delete"
    const val Move = "mail.move"
    const val Copy = "mail.copy"
    const val MarkRead = "mail.markRead"
    const val MarkAllRead = "mail.markAllRead"
    const val EmptyFolder = "mail.emptyFolder"
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
    fun toJson(): String =
        jsonObject(
            "feed_url" to feedUrl.jsonString(),
            "display_name" to displayName.jsonString(),
        )
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
    // Null omits the field, which the core reads as "keep the stored password".
    // Editing an existing account's servers must not require retyping it — the
    // UI never holds one. A blank string would *clear* the credential, so the
    // distinction matters.
    val password: String?,
    val tls: Boolean = true,
    val starttls: Boolean? = null,
    val smtpTls: Boolean? = null,
    val smtpStarttls: Boolean? = null,
    // Certificates the user accepted for a server webpki rejects. Null omits
    // the field, which keeps whatever the stored account already trusts; a new
    // account has nothing to keep, so the pin has to travel with the request
    // that creates it.
    val certPin: String? = null,
    val smtpCertPin: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "imap_host" to imapHost.jsonString(),
            "imap_port" to imapPort.toString(),
            "smtp_host" to smtpHost.jsonString(),
            "smtp_port" to smtpPort.toString(),
            "username" to username.jsonString(),
            "password" to password?.jsonString(),
            "tls" to tls.toString(),
            "starttls" to starttls?.toString(),
            "smtp_tls" to smtpTls?.toString(),
            "smtp_starttls" to smtpStarttls?.toString(),
            "cert_pin" to certPin?.jsonString(),
            "smtp_cert_pin" to smtpCertPin?.jsonString(),
        )
}

data class AutodiscoverAccountParams(
    val email: String,
) {
    fun toJson(): String = jsonObject("email" to email.jsonString())
}

data class AddOAuthAccountParams(
    val email: String,
    val provider: String,
    val displayName: String = "",
    val senderName: String = "",
    val username: String = "",
    val avatarUrl: String = "",
    val accessToken: String = "",
    // Empty for platform-managed accounts (e.g. Android Gmail via AccountManager),
    // where the OS holds the long-lived credential and re-mints access tokens.
    val refreshToken: String = "",
    val tokenExpiresAt: Long = 0,
    val imapHost: String = "",
    val imapPort: Int? = null,
    val smtpHost: String = "",
    val smtpPort: Int? = null,
) {
    fun toJson(): String =
        jsonObject(
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

data class UpdateOAuthTokenParams(
    val accountId: String,
    val accessToken: String,
    val tokenExpiresAt: Long = 0,
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
            "access_token" to accessToken.jsonString(),
            "token_expires_at" to tokenExpiresAt.toString(),
        )
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
    val tokenUrl: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "provider" to provider.jsonString(),
            "display_name" to displayName.jsonString(),
            "sender_name" to senderName.jsonString(),
            "code" to code.jsonString(),
            "client_id" to clientId.jsonString(),
            "client_secret" to clientSecret.takeIf { it.isNotBlank() }?.jsonString(),
            "redirect_uri" to redirectUri.takeIf { it.isNotBlank() }?.jsonString(),
            "code_verifier" to codeVerifier.takeIf { it.isNotBlank() }?.jsonString(),
            "token_url" to tokenUrl.takeIf { it.isNotBlank() }?.jsonString(),
        )
}

data class AddRssFeedParams(
    val accountId: String,
    val feedUrl: String,
) {
    fun toJson(): String =
        jsonObject(
            "account" to accountId.jsonString(),
            "feed_url" to feedUrl.jsonString(),
        )
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
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "target_account" to targetAccountId.jsonString(),
        )
}

data class ExportOpmlParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("account" to accountId.jsonString())
}

data class ImportOpmlParams(
    val accountId: String,
    val opml: String,
) {
    fun toJson(): String =
        jsonObject(
            "account" to accountId.jsonString(),
            "opml" to opml.jsonString(),
        )
}

/**
 * Backup export options. A passphrase encrypts the document; [includeSecrets]
 * additionally puts account passwords and OAuth tokens in it, which the core
 * refuses to do without a passphrase.
 *
 * [appVersion] is the host bundle's version name and [platform] is "android"
 * or "ios"; both are recorded in the backup envelope. The core knows neither:
 * its crate version is not the app's, and desktop and mobile are versioned on
 * separate tracks, so the number only identifies a build alongside the
 * platform that wrote it.
 */
data class ExportBackupParams(
    val includeSecrets: Boolean = false,
    val passphrase: String = "",
    val appVersion: String = "",
    val platform: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "include_secrets" to includeSecrets.toString(),
            "passphrase" to passphrase.jsonString(),
            "app_version" to appVersion.jsonString(),
            "platform" to platform.jsonString(),
        )
}

data class ImportBackupParams(
    val backup: String,
    val passphrase: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "backup" to backup.jsonString(),
            "passphrase" to passphrase.jsonString(),
        )
}

/**
 * Settings to read from the core store. The `settings` table is authoritative
 * for mobile preferences; the platform store in front of it is only a cache so
 * the first frame can paint before the keyed DB is open.
 */
data class AppPrefsGetParams(
    val keys: List<String>,
) {
    fun toJson(): String = jsonObject("keys" to keys.jsonStringArray())
}

/**
 * One setting to persist. [valueJson] is already-encoded JSON so a preference
 * can be any of the shapes the store holds (string, bool, number, array).
 */
data class AppPrefsSetParams(
    val key: String,
    val valueJson: String,
) {
    fun toJson(): String =
        jsonObject(
            "key" to key.jsonString(),
            "value" to valueJson.ifBlank { "null" },
        )
}

data class AccountIdParams(
    val accountId: String,
) {
    fun toJson(): String = jsonObject("id" to accountId.jsonString())
}

data class AccountNameParams(
    val accountId: String,
    val name: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "name" to name.jsonString(),
        )
}

data class AccountAvatarParams(
    val accountId: String,
    val avatarUrl: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "avatar_url" to avatarUrl.jsonString(),
        )
}

data class AccountMediaFileParams(
    val accountId: String,
    val filename: String,
    val mime: String,
    val data: String,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "filename" to filename.jsonString(),
            "mime" to mime.jsonString(),
            "data" to data.jsonString(),
        )
}

data class AccountChatWallpaperParams(
    val accountId: String,
    val presetId: String = "",
    val customUrl: String = "",
) {
    fun toJson(): String {
        val wallpaper =
            when {
                presetId.isNotBlank() -> {
                    jsonObject(
                        "kind" to "preset".jsonString(),
                        "presetId" to presetId.jsonString(),
                    )
                }

                customUrl.isNotBlank() -> {
                    jsonObject(
                        "kind" to "custom".jsonString(),
                        "url" to customUrl.jsonString(),
                    )
                }

                else -> {
                    "null"
                }
            }
        return jsonObject(
            "id" to accountId.jsonString(),
            "wallpaper" to wallpaper,
        )
    }
}

data class AccountFlagParams(
    val accountId: String,
    val enabled: Boolean,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "enabled" to enabled.toString(),
        )
}

data class AccountSaveSentCopyParams(
    val accountId: String,
    val value: Boolean?,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "value" to (value?.toString() ?: "null"),
        )
}

data class AccountRssSyncIntervalParams(
    val accountId: String,
    val minutes: Int,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "minutes" to minutes.toString(),
        )
}

data class AccountAliasParams(
    val email: String,
    val name: String = "",
) {
    fun toJson(): String =
        jsonObject(
            "email" to email.jsonString(),
            "name" to name.jsonString(),
        )
}

data class AccountAliasesParams(
    val accountId: String,
    val aliases: List<AccountAliasParams>,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "aliases" to aliases.joinToString(separator = ",", prefix = "[", postfix = "]") { it.toJson() },
        )
}

/** Serializes a [ProxySpec] the way the core's proxy parser expects it. */
private fun ProxySpec.toJson(): String =
    jsonObject(
        "mode" to mode.jsonString(),
        "host" to host.jsonString(),
        "port" to port.toString(),
        "username" to username.jsonString(),
        "password" to password.jsonString(),
    )

data class ProxyParams(
    val proxy: ProxySpec,
) {
    fun toJson(): String = jsonObject("proxy" to proxy.toJson())
}

data class AccountProxyParams(
    val accountId: String,
    val proxy: ProxySpec,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "proxy" to proxy.toJson(),
        )
}

/**
 * The server to fetch a certificate from, for a user who has to decide whether
 * to trust it. [protocol] is "imap" or "smtp" — the two can be different
 * daemons — and [starttls] picks the cleartext-then-upgrade form.
 *
 * [proxy] is the account's own proxy choice, so the probe takes the same route
 * as the connection that failed; null follows the app-wide proxy, as a
 * connection with no account override does.
 */
data class ProbeCertParams(
    val host: String,
    val port: Int,
    val protocol: String,
    val starttls: Boolean,
    val proxy: ProxySpec? = null,
) {
    fun toJson(): String =
        jsonObject(
            "host" to host.jsonString(),
            "port" to port.toString(),
            "protocol" to protocol.jsonString(),
            "starttls" to starttls.toString(),
            "proxy" to proxy?.toJson(),
        )
}

/**
 * Pins for the certificates the user accepted on an existing account. A null
 * pin leaves that server's stored pin alone; the core clears one only when the
 * key is sent explicitly as null, which nothing does yet.
 */
data class AccountCertPinParams(
    val accountId: String,
    val certPin: String? = null,
    val smtpCertPin: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            "cert_pin" to certPin?.jsonString(),
            "smtp_cert_pin" to smtpCertPin?.jsonString(),
        )
}

data class AccountSignatureParams(
    val accountId: String,
    val signature: SignatureSpec?,
) {
    fun toJson(): String =
        jsonObject(
            "id" to accountId.jsonString(),
            // A null signature clears the override, so the account follows the
            // app-wide signature again.
            "signature" to
                (
                    signature?.let {
                        jsonObject(
                            "mode" to it.mode.jsonString(),
                            "html" to it.html.jsonString(),
                        )
                    } ?: "null"
                ),
        )
}

data class AccountReorderParams(
    val accountIds: List<String>,
) {
    fun toJson(): String =
        jsonObject(
            "accounts" to accountIds.jsonStringArray(),
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

data class ContactSuggestParams(
    val accountId: String,
    val query: String = "",
    val limit: Int = 8,
) {
    fun toJson(): String =
        jsonObject(
            "account" to accountId.jsonString(),
            "query" to query.jsonString(),
            "limit" to limit.toString(),
        )
}

data class ThreadListParams(
    val accountId: String,
    val folderId: String = "inbox",
    val query: String = "",
    val filter: String = "all",
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
            "before_cursor" to beforeCursor?.jsonString(),
            "refresh" to refresh.toString(),
            "limit" to limit?.toString(),
            "folder_role" to folderRole?.jsonString(),
        )
}

data class StarredItemsParams(
    val query: String = "",
    val filter: String = "all",
    val limit: Int = 50,
    val beforeCursor: String? = null,
) {
    fun toJson(): String =
        jsonObject(
            "query" to query.jsonString(),
            "filter" to filter.jsonString(),
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
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "before_cursor" to beforeCursor?.jsonString(),
            "limit" to limit?.toString(),
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
) {
    fun toJson(): String =
        jsonObject(
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
) {
    fun toJson(): String =
        jsonObject(
            "account_id" to accountId.jsonString(),
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
) {
    fun toJson(): String =
        jsonObject(
            "thread_id" to threadId.jsonString(),
            "target_folder_id" to targetFolderId.jsonString(),
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

class MobileMailCommandClient(
    private val core: MeronCore,
) {
    suspend fun assertProtocol() {
        requireProtocolVersion(core.protocolVersion())
    }

    suspend fun listAccounts(): String = core.invoke(MobileCommand.AccountList)

    suspend fun addPasswordAccount(params: AddPasswordAccountParams): String = core.invoke(MobileCommand.AccountAddPassword, params.toJson())

    suspend fun autodiscoverAccount(params: AutodiscoverAccountParams): String = core.invoke(MobileCommand.AccountAutodiscover, params.toJson())

    suspend fun addOAuthAccount(params: AddOAuthAccountParams): String = core.invoke(MobileCommand.AccountAddOAuth, params.toJson())

    suspend fun exchangeOAuthCode(params: ExchangeOAuthCodeParams): String = core.invoke(MobileCommand.AccountExchangeOAuthCode, params.toJson())

    suspend fun updateOAuthToken(params: UpdateOAuthTokenParams): String = core.invoke(MobileCommand.AccountUpdateOAuthToken, params.toJson())

    suspend fun addRssAccount(params: AddRssAccountParams): String = core.invoke(MobileCommand.AccountAddRss, params.toJson())

    suspend fun removeAccount(params: AccountIdParams): String = core.invoke(MobileCommand.AccountRemove, params.toJson())

    suspend fun setAccountName(params: AccountNameParams): String = core.invoke(MobileCommand.AccountSetName, params.toJson())

    suspend fun setAccountSenderName(params: AccountNameParams): String = core.invoke(MobileCommand.AccountSetSenderName, params.toJson())

    suspend fun setAccountAvatar(params: AccountAvatarParams): String = core.invoke(MobileCommand.AccountSetAvatar, params.toJson())

    suspend fun writeAccountAvatarFile(params: AccountMediaFileParams): String = core.invoke(MobileCommand.AccountWriteAvatarFile, params.toJson())

    suspend fun setAccountChatWallpaper(params: AccountChatWallpaperParams): String = core.invoke(MobileCommand.AccountSetChatWallpaper, params.toJson())

    suspend fun writeAccountChatWallpaperFile(params: AccountMediaFileParams): String = core.invoke(MobileCommand.AccountWriteChatWallpaperFile, params.toJson())

    suspend fun setAccountImages(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetImages, params.toJson())

    suspend fun setAccountConversationHtml(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetConversationHtml, params.toJson())

    suspend fun setAccountUnified(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetUnified, params.toJson())

    suspend fun setAccountMuted(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetMuted, params.toJson())

    suspend fun setAccountPaused(params: AccountFlagParams): String = core.invoke(MobileCommand.AccountSetPaused, params.toJson())

    suspend fun setAccountSaveSentCopy(params: AccountSaveSentCopyParams): String = core.invoke(MobileCommand.AccountSetSaveSentCopy, params.toJson())

    suspend fun setAccountRssSyncInterval(params: AccountRssSyncIntervalParams): String = core.invoke(MobileCommand.AccountSetRssSyncInterval, params.toJson())

    suspend fun setAccountAliases(params: AccountAliasesParams): String = core.invoke(MobileCommand.AccountSetAliases, params.toJson())

    suspend fun setAccountProxy(params: AccountProxyParams): String = core.invoke(MobileCommand.AccountSetProxy, params.toJson())

    suspend fun probeCert(params: ProbeCertParams): String = core.invoke(MobileCommand.AccountProbeCert, params.toJson())

    suspend fun setAccountCertPin(params: AccountCertPinParams): String = core.invoke(MobileCommand.AccountSetCertPin, params.toJson())

    suspend fun setAccountSignature(params: AccountSignatureParams): String = core.invoke(MobileCommand.AccountSetSignature, params.toJson())

    suspend fun getProxy(): String = core.invoke(MobileCommand.AppProxyGet)

    suspend fun getPrefs(params: AppPrefsGetParams): String = core.invoke(MobileCommand.AppPrefsGet, params.toJson())

    suspend fun setPref(params: AppPrefsSetParams): String = core.invoke(MobileCommand.AppPrefsSet, params.toJson())

    suspend fun setProxy(params: ProxyParams): String = core.invoke(MobileCommand.AppProxySet, params.toJson())

    suspend fun reorderAccounts(params: AccountReorderParams): String = core.invoke(MobileCommand.AccountReorder, params.toJson())

    suspend fun addRssFeed(params: AddRssFeedParams): String = core.invoke(MobileCommand.FeedAdd, params.toJson())

    suspend fun removeRssFeed(params: RemoveRssFeedParams): String = core.invoke(MobileCommand.FeedRemove, params.toJson())

    suspend fun moveRssFeed(params: MoveRssFeedParams): String = core.invoke(MobileCommand.FeedMove, params.toJson())

    suspend fun exportOpml(params: ExportOpmlParams): String = core.invoke(MobileCommand.FeedExportOpml, params.toJson())

    suspend fun importOpml(params: ImportOpmlParams): String = core.invoke(MobileCommand.FeedImportOpml, params.toJson())

    suspend fun exportBackup(params: ExportBackupParams): String = core.invoke(MobileCommand.BackupExport, params.toJson())

    suspend fun importBackup(params: ImportBackupParams): String = core.invoke(MobileCommand.BackupImport, params.toJson())

    suspend fun listFolders(params: FolderListParams): String = core.invoke(MobileCommand.FolderList, params.toJson())

    suspend fun createFolder(params: FolderCreateParams): String = core.invoke(MobileCommand.FolderCreate, params.toJson())

    suspend fun deleteFolder(params: FolderDeleteParams): String = core.invoke(MobileCommand.FolderDelete, params.toJson())

    suspend fun suggestContacts(params: ContactSuggestParams): String = core.invoke(MobileCommand.ContactSuggest, params.toJson())

    suspend fun listThreads(params: ThreadListParams): String = core.invoke(MobileCommand.ThreadList, params.toJson())

    suspend fun listStarredItems(params: StarredItemsParams = StarredItemsParams()): String = core.invoke(MobileCommand.StarredItems, params.toJson())

    suspend fun allocateIdentity(params: AllocateIdentityParams): String = core.invoke(MobileCommand.AllocateIdentity, params.toJson())

    suspend fun readThread(params: ThreadReadParams): String = core.invoke(MobileCommand.ThreadRead, params.toJson())

    suspend fun readAttachment(params: AttachmentReadParams): String = core.invoke(MobileCommand.AttachmentRead, params.toJson())

    suspend fun fetchChangelog(): String = core.invoke(MobileCommand.ChangelogFetch)

    suspend fun storageUsage(): String = core.invoke(MobileCommand.StorageUsage)

    suspend fun clearStorageCache(): String = core.invoke(MobileCommand.StorageClearCache)

    suspend fun sync(params: SyncMailParams): String = core.invoke(MobileCommand.Sync, params.toJson())

    suspend fun syncRss(params: SyncRssParams): String = core.invoke(MobileCommand.RssSync, params.toJson())

    suspend fun readRssThread(params: RssThreadParams): String = core.invoke(MobileCommand.RssThread, params.toJson())

    suspend fun markRssRead(params: RssMarkReadParams): String = core.invoke(MobileCommand.RssMarkRead, params.toJson())

    suspend fun markRssStarred(params: RssMarkStarredParams): String = core.invoke(MobileCommand.RssMarkStarred, params.toJson())

    suspend fun send(params: SendMailParams): String = core.invoke(MobileCommand.Send, params.toJson())

    suspend fun saveDraft(params: SaveDraftParams): String = core.invoke(MobileCommand.SaveDraft, params.toJson())

    suspend fun discardDraft(params: DiscardDraftParams): String = core.invoke(MobileCommand.DiscardDraft, params.toJson())

    suspend fun archive(params: ThreadActionParams): String = core.invoke(MobileCommand.Archive, params.archiveJson())

    suspend fun delete(params: ThreadActionParams): String = core.invoke(MobileCommand.Delete, params.deleteJson())

    suspend fun move(params: MoveThreadParams): String = core.invoke(MobileCommand.Move, params.toJson())

    suspend fun copy(params: CopyThreadParams): String = core.invoke(MobileCommand.Copy, params.toJson())

    suspend fun markRead(params: MarkReadParams): String = core.invoke(MobileCommand.MarkRead, params.toJson())

    suspend fun markAllRead(params: MarkAllReadParams): String = core.invoke(MobileCommand.MarkAllRead, params.toJson())

    suspend fun emptyFolder(params: EmptyFolderParams): String = core.invoke(MobileCommand.EmptyFolder, params.toJson())

    suspend fun markStarred(params: MarkStarredParams): String = core.invoke(MobileCommand.MarkStarred, params.toJson())
}

fun accountListRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.AccountList)

fun accountAddPasswordRequest(
    id: Long = 1,
    params: AddPasswordAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAddPassword, params.toJson())

fun accountAutodiscoverRequest(
    id: Long = 1,
    params: AutodiscoverAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAutodiscover, params.toJson())

fun accountAddOAuthRequest(
    id: Long = 1,
    params: AddOAuthAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAddOAuth, params.toJson())

fun accountExchangeOAuthCodeRequest(
    id: Long = 1,
    params: ExchangeOAuthCodeParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountExchangeOAuthCode, params.toJson())

fun accountAddRssRequest(
    id: Long = 1,
    params: AddRssAccountParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountAddRss, params.toJson())

fun accountRemoveRequest(
    id: Long = 1,
    params: AccountIdParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountRemove, params.toJson())

fun accountSetNameRequest(
    id: Long = 1,
    params: AccountNameParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetName, params.toJson())

fun accountSetSenderNameRequest(
    id: Long = 1,
    params: AccountNameParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetSenderName, params.toJson())

fun accountSetAvatarRequest(
    id: Long = 1,
    params: AccountAvatarParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetAvatar, params.toJson())

fun accountWriteAvatarFileRequest(
    id: Long = 1,
    params: AccountMediaFileParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountWriteAvatarFile, params.toJson())

fun accountSetChatWallpaperRequest(
    id: Long = 1,
    params: AccountChatWallpaperParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetChatWallpaper, params.toJson())

fun accountWriteChatWallpaperFileRequest(
    id: Long = 1,
    params: AccountMediaFileParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountWriteChatWallpaperFile, params.toJson())

fun accountSetImagesRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetImages, params.toJson())

fun accountSetConversationHtmlRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetConversationHtml, params.toJson())

fun accountSetUnifiedRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetUnified, params.toJson())

fun accountSetMutedRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetMuted, params.toJson())

fun accountSetPausedRequest(
    id: Long = 1,
    params: AccountFlagParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetPaused, params.toJson())

fun accountSetRssSyncIntervalRequest(
    id: Long = 1,
    params: AccountRssSyncIntervalParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetRssSyncInterval, params.toJson())

fun accountSetAliasesRequest(
    id: Long = 1,
    params: AccountAliasesParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetAliases, params.toJson())

fun accountReorderRequest(
    id: Long = 1,
    params: AccountReorderParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountReorder, params.toJson())

fun feedAddRequest(
    id: Long = 1,
    params: AddRssFeedParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedAdd, params.toJson())

fun feedRemoveRequest(
    id: Long = 1,
    params: RemoveRssFeedParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedRemove, params.toJson())

fun feedMoveRequest(
    id: Long = 1,
    params: MoveRssFeedParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedMove, params.toJson())

fun feedExportOpmlRequest(
    id: Long = 1,
    params: ExportOpmlParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedExportOpml, params.toJson())

fun feedImportOpmlRequest(
    id: Long = 1,
    params: ImportOpmlParams,
): CoreRequest = CoreRequest(id, MobileCommand.FeedImportOpml, params.toJson())

fun appPrefsGetRequest(
    id: Long = 1,
    params: AppPrefsGetParams,
): CoreRequest = CoreRequest(id, MobileCommand.AppPrefsGet, params.toJson())

fun appPrefsSetRequest(
    id: Long = 1,
    params: AppPrefsSetParams,
): CoreRequest = CoreRequest(id, MobileCommand.AppPrefsSet, params.toJson())

fun backupExportRequest(
    id: Long = 1,
    params: ExportBackupParams,
): CoreRequest = CoreRequest(id, MobileCommand.BackupExport, params.toJson())

fun backupImportRequest(
    id: Long = 1,
    params: ImportBackupParams,
): CoreRequest = CoreRequest(id, MobileCommand.BackupImport, params.toJson())

fun folderListRequest(
    id: Long = 1,
    params: FolderListParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderList, params.toJson())

fun folderCreateRequest(
    id: Long = 1,
    params: FolderCreateParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderCreate, params.toJson())

fun folderDeleteRequest(
    id: Long = 1,
    params: FolderDeleteParams,
): CoreRequest = CoreRequest(id, MobileCommand.FolderDelete, params.toJson())

fun contactSuggestRequest(
    id: Long = 1,
    params: ContactSuggestParams,
): CoreRequest = CoreRequest(id, MobileCommand.ContactSuggest, params.toJson())

fun threadListRequest(
    id: Long = 1,
    params: ThreadListParams,
): CoreRequest = CoreRequest(id, MobileCommand.ThreadList, params.toJson())

fun starredItemsRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.StarredItems)

fun threadReadRequest(
    id: Long = 1,
    params: ThreadReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.ThreadRead, params.toJson())

fun attachmentReadRequest(
    id: Long = 1,
    params: AttachmentReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.AttachmentRead, params.toJson())

fun setAccountProxyRequest(
    id: Long = 1,
    params: AccountProxyParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetProxy, params.toJson())

fun probeCertRequest(
    id: Long = 1,
    params: ProbeCertParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountProbeCert, params.toJson())

fun setAccountCertPinRequest(
    id: Long = 1,
    params: AccountCertPinParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetCertPin, params.toJson())

fun setAccountSignatureRequest(
    id: Long = 1,
    params: AccountSignatureParams,
): CoreRequest = CoreRequest(id, MobileCommand.AccountSetSignature, params.toJson())

fun proxyGetRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.AppProxyGet)

fun proxySetRequest(
    id: Long = 1,
    params: ProxyParams,
): CoreRequest = CoreRequest(id, MobileCommand.AppProxySet, params.toJson())

fun storageUsageRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.StorageUsage)

fun storageClearCacheRequest(id: Long = 1): CoreRequest = CoreRequest(id, MobileCommand.StorageClearCache)

fun syncMailRequest(
    id: Long = 1,
    params: SyncMailParams,
): CoreRequest = CoreRequest(id, MobileCommand.Sync, params.toJson())

fun syncRssRequest(
    id: Long = 1,
    params: SyncRssParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssSync, params.toJson())

fun rssThreadRequest(
    id: Long = 1,
    params: RssThreadParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssThread, params.toJson())

fun sendMailRequest(
    id: Long = 1,
    params: SendMailParams,
): CoreRequest = CoreRequest(id, MobileCommand.Send, params.toJson())

fun saveDraftRequest(
    id: Long = 1,
    params: SaveDraftParams,
): CoreRequest = CoreRequest(id, MobileCommand.SaveDraft, params.toJson())

fun discardDraftRequest(
    id: Long = 1,
    params: DiscardDraftParams,
): CoreRequest = CoreRequest(id, MobileCommand.DiscardDraft, params.toJson())

fun archiveThreadRequest(
    id: Long = 1,
    params: ThreadActionParams,
): CoreRequest = CoreRequest(id, MobileCommand.Archive, params.archiveJson())

fun moveThreadRequest(
    id: Long = 1,
    params: MoveThreadParams,
): CoreRequest = CoreRequest(id, MobileCommand.Move, params.toJson())

fun copyThreadRequest(
    id: Long = 1,
    params: CopyThreadParams,
): CoreRequest = CoreRequest(id, MobileCommand.Copy, params.toJson())

fun deleteThreadRequest(
    id: Long = 1,
    params: ThreadActionParams,
): CoreRequest = CoreRequest(id, MobileCommand.Delete, params.deleteJson())

fun markReadRequest(
    id: Long = 1,
    params: MarkReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.MarkRead, params.toJson())

fun markAllReadRequest(
    id: Long = 1,
    params: MarkAllReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.MarkAllRead, params.toJson())

fun emptyFolderRequest(
    id: Long = 1,
    params: EmptyFolderParams,
): CoreRequest = CoreRequest(id, MobileCommand.EmptyFolder, params.toJson())

fun markStarredRequest(
    id: Long = 1,
    params: MarkStarredParams,
): CoreRequest = CoreRequest(id, MobileCommand.MarkStarred, params.toJson())

fun rssMarkReadRequest(
    id: Long = 1,
    params: RssMarkReadParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssMarkRead, params.toJson())

fun rssMarkStarredRequest(
    id: Long = 1,
    params: RssMarkStarredParams,
): CoreRequest = CoreRequest(id, MobileCommand.RssMarkStarred, params.toJson())

private fun jsonObject(vararg fields: Pair<String, String?>): String =
    fields
        .filter { it.second != null }
        .joinToString(separator = ",", prefix = "{", postfix = "}") { (name, value) -> "${name.jsonString()}:$value" }

private fun List<String>.jsonStringArray(): String = joinToString(separator = ",", prefix = "[", postfix = "]") { it.jsonString() }
