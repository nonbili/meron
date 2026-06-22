package jp.nonbili.meron.shared

data class AccountSummary(
    val id: String,
    val email: String,
    val displayName: String = "",
    val needsReconnect: Boolean = false,
    val engine: String = "",
    val provider: String = "",
    val authType: String = "",
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
    val dateEpochSeconds: Long = 0,
)

data class MessageBody(
    val id: String,
    val from: String,
    val to: String,
    val subject: String,
    val body: String,
    val dateEpochSeconds: Long = 0,
    val fromAddr: String = "",
    val replyTo: String = "",
    val messageId: String = "",
    val references: String = "",
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
        get() = to.isNotBlank() && subject.isNotBlank() && body.isNotBlank()
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

fun accountSummaryIsRss(account: AccountSummary): Boolean {
    return account.engine == "rss" || account.provider == "rss" || account.authType == "rss"
}

fun threadIdIsRss(threadId: String): Boolean = threadId.contains("#rss#")
