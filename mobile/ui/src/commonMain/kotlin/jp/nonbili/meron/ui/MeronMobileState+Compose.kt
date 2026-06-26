package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.FilterList
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.OpenInFull
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material.icons.filled.ViewKanban
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.material.icons.outlined.FolderOpen
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountAliasParams
import jp.nonbili.meron.shared.AccountAliasesParams
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.CopyThreadParams
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.DraftAttachment
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.FolderCreateParams
import jp.nonbili.meron.shared.FolderListParams
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ImportOpmlParams
import jp.nonbili.meron.shared.MarkAllReadParams
import jp.nonbili.meron.shared.MarkReadParams
import jp.nonbili.meron.shared.MarkStarredParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.MoveRssFeedParams
import jp.nonbili.meron.shared.MoveThreadParams
import jp.nonbili.meron.shared.OAuthAuthorizationRequest
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import kotlin.math.abs

internal fun MeronMobileState.defaultSendAccountId(): String =
    selectedCoreAccountId.takeIf { selected ->
        selected != UNIFIED_ACCOUNT_ID && coreAccounts.any { it.id == selected && !accountSummaryIsRss(it) }
    } ?: coreAccounts.firstOrNull { !accountSummaryIsRss(it) }?.id.orEmpty()

internal fun MeronMobileState.composeIdentityCandidates(): List<SendIdentity> =
    coreAccounts
        .filter { !accountSummaryIsRss(it) && !it.needsReconnect }
        .flatMap { accountSendIdentities(it) }

internal fun MeronMobileState.selectedComposeIdentity(): SendIdentity? {
    val candidates = composeIdentityCandidates()
    return candidates.firstOrNull { it.accountId == composeFromAccountId && it.email == composeFromEmail }
        ?: candidates.firstOrNull { it.accountId == defaultSendAccountId() }
        ?: candidates.firstOrNull()
}

internal fun MeronMobileState.clearComposeDraftState() {
    attachments = emptyList()
    to = ""
    cc = ""
    bcc = ""
    subject = ""
    body = ""
    composeFromAccountId = ""
    composeFromEmail = ""
    composeDraftId = ""
    composeDraftSaved = false
    composeInReplyTo = ""
    composeReferences = ""
    recipientSuggestionField = ""
    recipientSuggestions = emptyList()
}

internal fun MeronMobileState.loadRecipientSuggestions(
    field: String,
    value: String,
) {
    val accountId = defaultSendAccountId()
    if (accountId.isBlank() || !coreLoaded) {
        recipientSuggestions = emptyList()
        recipientSuggestionField = field
        return
    }
    recipientSuggestionField = field
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.suggestContacts(
                    ContactSuggestParams(
                        accountId = accountId,
                        query = recipientTail(value),
                        limit = 6,
                    ),
                )
            }
        }.onSuccess {
            if (recipientSuggestionField == field) {
                recipientSuggestions = parseContactSuggestResponse(it)
            }
        }.onFailure {
            if (recipientSuggestionField == field) {
                recipientSuggestions = emptyList()
            }
        }
    }
}

internal fun MeronMobileState.acceptRecipientSuggestion(
    field: String,
    contact: ContactSuggestion,
) {
    when (field) {
        "to" -> to = replaceRecipientTail(to, contact)
        "cc" -> cc = replaceRecipientTail(cc, contact)
        "bcc" -> bcc = replaceRecipientTail(bcc, contact)
    }
    recipientSuggestions = emptyList()
}

internal fun MeronMobileState.sendMail() {
    val identity = selectedComposeIdentity()
    val accountId = identity?.accountId ?: defaultSendAccountId()
    val savedDraftId = composeDraftId.takeIf { composeDraftSaved }
    if (accountId.isBlank()) {
        status = "Select or add an account before sending."
        return
    }
    val draft = ComposeDraft(to.trim(), cc.trim(), bcc.trim(), subject.trim(), body.trim(), attachments)
    if (!draft.canSend) {
        status = "Complete To, Subject, and Body or Attachments before sending."
        return
    }
    status = "Sending..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val params =
                    draft.toSendMailParams(accountId = accountId, from = identity?.email.orEmpty()).copy(
                        inReplyTo = composeInReplyTo,
                        references = composeReferences,
                    )
                client.send(params)
                if (!savedDraftId.isNullOrBlank()) {
                    runCatching { client.discardDraft(DiscardDraftParams(accountId = accountId, draftId = savedDraftId)) }
                }
            }
        }.onSuccess {
            clearComposeDraftState()
            screen = previousTopScreen
            errorBanner = null
            status = "Message sent"
            syncCoreThreads()
        }.onFailure {
            errorBanner = it.message ?: "Send failed"
            status = "Send failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.saveComposeDraft() {
    val identity = selectedComposeIdentity()
    val accountId = identity?.accountId ?: defaultSendAccountId()
    if (accountId.isBlank()) {
        status = "Select or add an account before saving."
        return
    }
    val draft = ComposeDraft(to.trim(), cc.trim(), bcc.trim(), subject.trim(), body.trim(), attachments)
    if (listOf(draft.to, draft.cc, draft.bcc, draft.subject, draft.body).all { it.isBlank() } && draft.attachments.isEmpty()) {
        status = "Nothing to save."
        return
    }
    val draftId = composeDraftId.ifBlank { newDraftMessageId(accountId) }
    composeDraftId = draftId
    status = "Saving draft..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val params =
                    draft
                        .toSaveDraftParams(
                            accountId = accountId,
                            draftId = draftId,
                            from = identity?.email.orEmpty(),
                        ).copy(
                            inReplyTo = composeInReplyTo,
                            references = composeReferences,
                        )
                MobileMailCommandClient(core).saveDraft(
                    params,
                )
            }
        }.onSuccess {
            composeDraftSaved = true
            status = "Draft saved"
            syncCoreThreads(syncFirst = false)
        }.onFailure {
            status = "Draft save failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.openQuickReplyInFullEditor() {
    val thread = selectedCoreThread
    val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    val parent = messages.lastOrNull()
    if (accountId.isBlank() || thread == null || parent == null) {
        status = "Open a mail thread before replying."
        return
    }
    if (threadIdIsRss(thread.id)) {
        status = "RSS items do not support replies."
        return
    }
    val replyFrom =
        coreAccounts
            .firstOrNull { it.id == accountId }
            ?.let { detectReplyFromIdentity(parent, it) }
            .orEmpty()
    val params =
        parent.toReplyMailParams(
            accountId = accountId,
            body = quickReplyBody.trim(),
            from = replyFrom,
            ownAddresses = ownAddressList(coreAccounts),
            attachments = quickReplyAttachments,
        )
    to = params.to
    cc = params.cc
    bcc = params.bcc
    subject = params.subject
    body = params.body
    attachments = quickReplyAttachments
    composeFromAccountId = accountId
    composeFromEmail = replyFrom
    composeDraftId = ""
    composeDraftSaved = false
    composeInReplyTo = params.inReplyTo
    composeReferences = params.references
    quickReplyBody = ""
    quickReplyAttachments = emptyList()
    quickReplyFailure = ""
    previousTopScreen = Screen.Thread
    screen = Screen.Compose
    status = "Reply opened in full editor"
}

internal fun MeronMobileState.discardComposeDraft() {
    val identity = selectedComposeIdentity()
    val accountId = identity?.accountId ?: defaultSendAccountId()
    val draftId = composeDraftId.takeIf { composeDraftSaved }
    if (!draftId.isNullOrBlank() && accountId.isBlank()) {
        status = "Select or add an account before discarding."
        return
    }
    status = "Discarding draft..."
    scope.launch {
        runCatching {
            if (!draftId.isNullOrBlank()) {
                withContext(ioDispatcher) {
                    MobileMailCommandClient(core).discardDraft(
                        DiscardDraftParams(accountId = accountId, draftId = draftId),
                    )
                }
            }
        }.onSuccess {
            clearComposeDraftState()
            screen = previousTopScreen
            status = "Draft discarded"
            syncCoreThreads(syncFirst = false)
        }.onFailure {
            status = "Draft discard failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.sendQuickReply() {
    val thread = selectedCoreThread
    val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    val parent = messages.lastOrNull()
    val replyBody = quickReplyBody.trim()
    if (accountId.isBlank() || thread == null || parent == null) {
        status = "Open a mail thread before replying."
        return
    }
    if (threadIdIsRss(thread.id)) {
        status = "RSS items do not support replies."
        return
    }
    if (replyBody.isBlank() && quickReplyAttachments.isEmpty()) {
        status = "Write a reply or attach a file before sending."
        return
    }
    quickReplyFailure = ""
    status = "Sending reply..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val replyFrom =
                    coreAccounts
                        .firstOrNull { it.id == accountId }
                        ?.let { detectReplyFromIdentity(parent, it) }
                        .orEmpty()
                MobileMailCommandClient(core).send(
                    parent.toReplyMailParams(
                        accountId = accountId,
                        body = replyBody,
                        from = replyFrom,
                        ownAddresses = ownAddressList(coreAccounts),
                        attachments = quickReplyAttachments,
                    ),
                )
            }
        }.onSuccess {
            quickReplyBody = ""
            quickReplyAttachments = emptyList()
            quickReplyFailure = ""
            status = "Reply sent"
        }.onFailure {
            val message = it.message ?: "Send failed"
            quickReplyFailure = message
            status = "Reply failed: $message"
        }
    }
}

internal fun MeronMobileState.openMessageCompose(
    message: MessageBody,
    forward: Boolean,
) {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                forwardableAttachments(message).mapNotNull { attachment ->
                    val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                    data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                }
            }
        }.onSuccess { copiedAttachments ->
            val draft =
                if (forward) {
                    messageForwardDraft(message, copiedAttachments)
                } else {
                    messageEditAsNewDraft(message, copiedAttachments)
                }
            to = draft.to
            cc = draft.cc
            bcc = draft.bcc
            subject = draft.subject
            body = draft.body
            attachments = draft.attachments
            composeFromAccountId = selectedCoreThread?.accountId ?: selectedCoreAccountId.takeIf { it != UNIFIED_ACCOUNT_ID }.orEmpty()
            composeFromEmail = ""
            composeDraftId = ""
            composeDraftSaved = false
            previousTopScreen = Screen.Thread
            screen = Screen.Compose
            status = if (forward) "Forward draft ready" else "Copied message into compose"
        }.onFailure {
            status = if (forward) "Forward failed: ${it.message}" else "Edit as new failed: ${it.message}"
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
internal suspend fun MeronMobileState.readAttachmentBytes(attachment: MessageAttachment): ByteArray {
    val response = MobileMailCommandClient(core).readAttachment(AttachmentReadParams(attachment.key))
    val data = parseAttachmentDataResponse(response)
    if (data.isBlank()) error("Attachment data is empty")
    return Base64.Default.decode(data)
}

internal fun MeronMobileState.saveMessageAttachment(attachment: MessageAttachment) {
    if (attachment.key.isBlank()) {
        status =
            if (attachment.url.isNotBlank()) "Remote attachments can be opened but are not cached for saving." else "Attachment is not cached."
        return
    }
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    pendingAttachmentSave = attachment
    launchAttachmentSave(safeAttachmentFilename(attachment.filename))
}

internal fun MeronMobileState.openMessageAttachment(attachment: MessageAttachment) {
    if (attachment.url.isNotBlank()) {
        services.openUrl(attachment.url)
        return
    }
    if (attachment.key.isBlank()) {
        status = "Attachment is not cached."
        return
    }
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { readAttachmentBytes(attachment) }
        }.onSuccess { bytes ->
            if (attachment.mimeType.startsWith("image/")) {
                val image = decodeImageBitmap(bytes)
                if (image == null) {
                    status = "Attachment image could not be decoded"
                    return@onSuccess
                }
                imagePreview =
                    ImagePreview(
                        title = attachment.filename.ifBlank { "Image" },
                        image = image,
                        bytes = bytes,
                        mimeType = attachment.mimeType.ifBlank { "image/*" },
                        fileName = safeAttachmentFilename(attachment.filename),
                    )
            } else {
                services.shareFile(
                    bytes,
                    safeAttachmentFilename(attachment.filename),
                    attachment.mimeType.ifBlank { "application/octet-stream" },
                )
            }
        }.onFailure {
            status = "Attachment open failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.shareImagePreview(preview: ImagePreview) {
    services.shareFile(preview.bytes, preview.fileName, preview.mimeType.ifBlank { "image/*" })
}

internal fun MeronMobileState.copyImagePreview(preview: ImagePreview) {
    services.copyImage(preview.bytes, preview.mimeType.ifBlank { "image/*" }, preview.title.ifBlank { "Image" })
    status = "Image copied."
}

internal fun MeronMobileState.openCompose() {
    to = ""
    cc = ""
    bcc = ""
    subject = ""
    body = ""
    attachments = emptyList()
    composeDraftId = ""
    composeDraftSaved = false
    previousTopScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
    screen = Screen.Compose
}

// Re-open account setup pre-filled so the user can fix credentials. OAuth
// accounts re-run the browser sign-in; password accounts re-enter the
// password (the IMAP/SMTP host fields keep their last values).
internal fun MeronMobileState.reconnectAccount(account: AccountSummary) {
    val isOAuth = account.authType == "oauth" || account.provider == "gmail" || account.provider == "outlook"
    when {
        accountSummaryIsRss(account) -> {
            addSection = 2
            passwordServerSettingsOpen = false
        }

        isOAuth -> {
            oauthEmail = account.email
            if (account.provider == "gmail" || account.provider == "outlook") oauthProvider = account.provider
            oauthAuthorizationCode = ""
            addSection = 0
            passwordServerSettingsOpen = false
        }

        else -> {
            email = account.email
            username = account.email
            password = ""
            if (account.imapHost.isNotBlank()) host = account.imapHost
            if (account.imapPort > 0) imapPort = account.imapPort.toString()
            if (account.smtpHost.isNotBlank()) smtpHost = account.smtpHost
            if (account.smtpPort > 0) smtpPort = account.smtpPort.toString()
            addSection = 1
            passwordServerSettingsOpen = true
        }
    }
    errorBanner = null
    previousTopScreen = if (screen == Screen.Kanban || screen == Screen.Starred) screen else Screen.Mail
    screen = Screen.AddAccount
}

internal fun MeronMobileState.createKanbanBoard(): String {
    val board = defaultKanbanBoard(coreAccounts).copy(name = "Kanban board ${kanbanBoards.size + 1}")
    persistKanbanBoards(kanbanBoards + board)
    activeKanbanBoardId = board.id
    saveActiveKanbanBoardId(kanbanPrefs, board.id)
    loadKanbanBoard(refresh = false)
    return board.id
}

internal fun MeronMobileState.updateKanbanBoard(
    boardId: String,
    name: String,
    avatarUrl: String,
    wallpaperPresetId: String,
    wallpaperUrl: String,
) {
    val trimmedName = name.trim()
    if (trimmedName.isBlank()) return
    persistKanbanBoards(
        kanbanBoards.map { board ->
            if (board.id == boardId) {
                board.copy(
                    name = trimmedName,
                    avatarUrl = avatarUrl.trim(),
                    wallpaperPresetId = wallpaperPresetId.trim(),
                    wallpaperUrl = wallpaperUrl.trim(),
                )
            } else {
                board
            }
        },
    )
}

internal fun MeronMobileState.deleteKanbanBoard(boardId: String) {
    val next = kanbanBoards.filterNot { it.id == boardId }
    persistKanbanBoards(if (next.isEmpty()) listOf(defaultKanbanBoard(coreAccounts)) else next)
    if (boardId == activeKanbanBoardId) {
        kanbanColumns = emptyMap()
        loadKanbanBoard(refresh = false)
    }
}

internal fun MeronMobileState.addKanbanColumn(column: KanbanColumnSpec) {
    val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
    if (board.columns.any { kanbanColumnKey(it) == kanbanColumnKey(column) }) return
    persistKanbanBoards(
        kanbanBoards.map {
            if (it.id == board.id) it.copy(columns = it.columns + column) else it
        },
    )
    loadKanbanColumn(column, refresh = true)
}

internal fun MeronMobileState.removeKanbanColumn(column: KanbanColumnSpec) {
    val key = kanbanColumnKey(column)
    persistKanbanBoards(
        kanbanBoards.map {
            if (it.id ==
                activeKanbanBoardId
            ) {
                it.copy(columns = it.columns.filterNot { existing -> kanbanColumnKey(existing) == key })
            } else {
                it
            }
        },
    )
    kanbanColumns = kanbanColumns - key
}

internal fun MeronMobileState.moveKanbanColumn(
    column: KanbanColumnSpec,
    delta: Int,
) {
    persistKanbanBoards(
        kanbanBoards.map { board ->
            if (board.id != activeKanbanBoardId) return@map board
            val columns = board.columns.toMutableList()
            val index = columns.indexOfFirst { kanbanColumnKey(it) == kanbanColumnKey(column) }
            val target = (index + delta).coerceIn(0, columns.lastIndex)
            if (index < 0 || index == target) {
                board
            } else {
                val item = columns.removeAt(index)
                columns.add(target, item)
                board.copy(columns = columns)
            }
        },
    )
}

internal fun MeronMobileState.createFolderForKanban(
    account: AccountSummary,
    name: String,
) {
    val trimmed = name.trim()
    if (trimmed.isBlank()) {
        status = "Folder name is required."
        return
    }
    if (!coreLoaded) {
        status = "Rust core not packaged."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.createFolder(FolderCreateParams(accountId = account.id, name = trimmed))
                loadAccountFolders(client, account)
            }
        }.onSuccess { folders ->
            foldersByAccount = foldersByAccount + (account.id to folders)
            val created = folders.firstOrNull { it.name.equals(trimmed, ignoreCase = true) }?.name ?: trimmed
            addKanbanColumn(KanbanColumnSpec(account.id, created))
            showKanbanCreateFolderDialog = null
            kanbanFolderNameInput = ""
            status = "Folder created"
        }.onFailure {
            status = "Create folder failed: ${it.message}"
        }
    }
}
