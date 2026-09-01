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
import jp.nonbili.meron.shared.AllocateIdentityParams
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
import jp.nonbili.meron.shared.SendMailParams
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.SignatureMark
import jp.nonbili.meron.shared.SignaturePlacement
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
import jp.nonbili.meron.shared.bodyWithSignature
import jp.nonbili.meron.shared.bodyWithSwappedSignature
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsInbox
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardHtmlForSend
import jp.nonbili.meron.shared.forwardInlineImages
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.inlineImageToDraftAttachment
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.noSignatureMark
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAllocatedMessageId
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
import jp.nonbili.meron.shared.resolveSignatureHtml
import jp.nonbili.meron.shared.signaturePlainText
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import jp.nonbili.meron.shared.untrustedCertificateProtocol
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi
import kotlin.math.abs

// Only touched from the main thread (all send paths run in the UI scope).
private var localSendSequence = 0L

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
        ?: candidates.firstOrNull { it.accountId == composeFromAccountId }
        ?: candidates.firstOrNull { it.accountId == defaultSendAccountId() }
        ?: candidates.firstOrNull()
}

/**
 * The signature for a draft sent from [accountId], already converted to the
 * plain text the composer edits. Blank when nothing is configured.
 */
private fun MeronMobileState.signatureTextFor(accountId: String): String {
    val account = coreAccounts.firstOrNull { it.id == accountId }
    return signaturePlainText(resolveSignatureHtml(account, appSignatureHtml))
}

/**
 * Seed a fresh draft body with the sending account's signature, remembering what
 * was inserted so a later change of identity can swap it out.
 */
private fun MeronMobileState.seedBodyWithSignature(
    body: String,
    accountId: String,
    placement: SignaturePlacement = SignaturePlacement.BelowText,
): String {
    val signature = signatureTextFor(accountId)
    // The placement is recorded even when the account sends no signature, so a
    // forward that later moves to an account with one still puts it above the
    // quote rather than after it.
    composeSignature = if (signature.isBlank()) noSignatureMark(placement) else SignatureMark(signature, placement)
    return bodyWithSignature(body, signature, placement)
}

/**
 * Move the draft to another send identity, swapping the signature it carries for
 * the new account's. Sending account B's mail under account A's signature is
 * worse than no signature at all; an edited signature is left alone.
 */
internal fun MeronMobileState.changeComposeIdentity(
    accountId: String,
    email: String,
) {
    if (accountId != composeDraftAccountId && composeDraftSaved && composeDraftAccountId.isNotBlank() && composeDraftId.isNotBlank()) {
        val owner = ComposeDraftOwner(composeDraftAccountId, composeDraftId, composeDraftThreadId())
        if (owner !in composeDraftCleanupOwners) composeDraftCleanupOwners = composeDraftCleanupOwners + owner
        composeDraftId = newDraftMessageId(accountId)
        composeDraftSaved = false
        composeDraftAccountId = ""
    }
    composeFromAccountId = accountId
    composeFromEmail = email
    val sessionGeneration = composeSessionGeneration
    val identityGeneration = ++composeIdentityGeneration
    if (!appSignatureLoaded) {
        composeSignaturePending = true
        scope.launch {
            awaitAppSignatureLoaded()
            if (sessionGeneration == composeSessionGeneration && identityGeneration == composeIdentityGeneration) {
                applyComposeIdentitySignature(accountId)
                composeSignaturePending = false
            }
        }
        return
    }
    applyComposeIdentitySignature(accountId)
    composeSignaturePending = false
}

private fun MeronMobileState.applyComposeIdentitySignature(accountId: String) {
    val swapped = bodyWithSwappedSignature(body, composeSignature, signatureTextFor(accountId))
    body = swapped.body
    composeSignature = swapped.tracking
}

internal fun MeronMobileState.clearComposeDraftState() {
    attachments = emptyList()
    composeSignature = null
    to = ""
    cc = ""
    bcc = ""
    subject = ""
    body = ""
    composeFromAccountId = ""
    composeFromEmail = ""
    composeSignaturePending = false
    ++composeIdentityGeneration
    composeDraftId = ""
    composeDraftSaved = false
    composeDraftAccountId = ""
    composeInReplyTo = ""
    composeReferences = ""
    composeForwardHtml = ""
    composeForwardInlineAttachments = emptyList()
    composeSeed = ComposeSeed()
    recipientSuggestionField = ""
    recipientSuggestions = emptyList()
}

/**
 * The composer with the signature this app seeded taken back out. A signature
 * the user has typed into can no longer be identified (see
 * [bodyWithSwappedSignature]), which is the same answer as "this text is theirs
 * now". Mirrors the reply bar's [quickReplyIsBlank].
 */
private fun MeronMobileState.composeBodyWithoutSignature(): String {
    val mark = composeSignature ?: return body
    if (mark.text.isBlank()) return body
    val swapped = bodyWithSwappedSignature(body, mark, "")
    return if (swapped.tracking == null) body else swapped.body
}

/**
 * Whether the composer holds nothing the user put there. A seeded signature does
 * not count as content: it is not something they wrote, and saving on it leaves
 * an empty draft behind for every composer merely opened and closed again.
 */
internal fun MeronMobileState.composeIsBlank(): Boolean = composeBodyWithoutSignature().isBlank() && currentComposeSeed() == composeSeed

/** Records what a freshly opened composer holds, so [composeIsBlank] can tell it apart from the user's own writing. */
internal fun MeronMobileState.rememberComposeSeed() {
    composeSeed = currentComposeSeed()
}

private fun MeronMobileState.currentComposeSeed(): ComposeSeed =
    ComposeSeed(
        to = recipientEntries(to),
        cc = recipientEntries(cc),
        bcc = recipientEntries(bcc),
        subject = subject.trim(),
        attachments = attachments,
    )

private fun recipientEntries(value: String): List<String> {
    val (completed, active) = parseRecipients(value)
    return (completed + active).map { it.trim() }.filter { it.isNotEmpty() }
}

// The draft as the core should receive it: the plain body the composer edits,
// plus — for a forward — the rebuilt HTML alternative and the inline images it
// references.
private fun MeronMobileState.currentComposeDraft(): ComposeDraft {
    val html = forwardHtmlForSend(body.trim(), composeForwardHtml)
    return ComposeDraft(
        to = to.trim(),
        cc = cc.trim(),
        bcc = bcc.trim(),
        subject = subject.trim(),
        body = body.trim(),
        // The inline images exist only to back the quote's cid: refs. If the
        // quote is gone the HTML is empty, and attaching them would ship files
        // nothing references.
        attachments = if (html.isBlank()) attachments else attachments + composeForwardInlineAttachments,
        html = html,
    )
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
    if (composeSendInFlight) return
    if (composeSignaturePending || !appSignatureLoaded) {
        status = "Waiting for signature before sending."
        return
    }
    val identity = selectedComposeIdentity()
    val accountId = identity?.accountId ?: defaultSendAccountId()
    if (accountId.isBlank()) {
        status = "Select or add an account before sending."
        return
    }
    val draft = currentComposeDraft()
    if (!draft.canSend) {
        status = "Complete To, Subject, and Body or Attachments before sending."
        return
    }
    composeSendInFlight = true
    pendingComposeSend = null
    pendingCertificateRetry = null
    val generation = composeSessionGeneration
    val initialDraftOwners = composeDraftOwnerSnapshot()
    status = "Sending..."
    scope.launch {
        composeSaveMutex.withLock {
            val draftOwners =
                if (composeSessionGeneration == generation) {
                    composeDraftOwnerSnapshot()
                } else {
                    initialDraftOwners
                }
            val params =
                runCatching {
                    withContext(ioDispatcher) {
                        val client = MobileMailCommandClient(core)
                        draft.toSendMailParams(accountId = accountId, from = identity?.email.orEmpty()).copy(
                            inReplyTo = composeInReplyTo,
                            references = composeReferences,
                            messageId = allocateCoreMessageId(client = client, accountId = accountId, draft = false),
                        )
                    }
                }.getOrElse {
                    failComposeSend(error = it)
                    return@withLock
                }
            // Remembered so a send refused by a certificate we cannot validate
            // can be sent again as the same message — including its Message-ID —
            // once the user trusts the server.
            val pending = PendingComposeSend(accountId, params, generation, draftOwners)
            pendingComposeSend = pending
            dispatchComposeSend(pending)
        }
    }
}

/**
 * Send a prepared message and reconcile the composer around the result. Split
 * out so a retry sends exactly the message that failed rather than whatever the
 * composer holds by the time the user gets back to it.
 */
private suspend fun MeronMobileState.dispatchComposeSend(
    pending: PendingComposeSend,
) {
    runCatching {
        withContext(ioDispatcher) {
            val client = MobileMailCommandClient(core)
            withManagedGoogleAuth(client, pending.accountId) { client.send(pending.params) }
        }
    }.onSuccess {
        finishComposeSend(pending)
    }.onFailure {
        failComposeSend(pending, it)
    }
}

/** Re-send the message a certificate rejection stopped, after it was trusted. */
internal fun MeronMobileState.retryComposeSend() {
    retryComposeSend(pendingComposeSend ?: return)
}

internal fun MeronMobileState.retryComposeSend(pending: PendingComposeSend) {
    if (composeSendInFlight) return
    pendingComposeSend = pending
    composeSendInFlight = true
    status = "Sending..."
    scope.launch {
        composeSaveMutex.withLock {
            dispatchComposeSend(pending)
        }
    }
}

private suspend fun MeronMobileState.finishComposeSend(pending: PendingComposeSend) {
    val discardOutcome = discardComposeDraftOwners(pending.draftOwners)
    composeDraftCleanupOwners =
        (composeDraftCleanupOwners + discardOutcome.failedOwners)
            .distinctBy { it.accountId to it.draftId }
    if (pendingComposeSend == pending) pendingComposeSend = null
    if (pendingCertificateRetry == PendingCertificateRetry.Compose(pending)) pendingCertificateRetry = null
    composeSendInFlight = false
    if (composeSessionGeneration == pending.composeSessionGeneration) {
        clearComposeDraftState()
        // A retry can land after the user has left the composer to read the banner;
        // only the captured composer still on screen gets closed.
        if (screen == Screen.Compose) closeCompose()
    }
    errorBanner = null
    status = "Message sent"
    syncCoreThreads()
}

private fun MeronMobileState.failComposeSend(
    pending: PendingComposeSend? = pendingComposeSend,
    error: Throwable,
) {
    composeSendInFlight = false
    val message = error.message ?: "Send failed"
    errorBanner = message
    status = "Send failed: $message"
    // Trusting the certificate has to resume the send, not fall back to a
    // sync: the message is still unsent and the composer may be closed by then.
    if (pending != null && untrustedCertificateProtocol(message) != null) {
        pendingCertificateRetry = PendingCertificateRetry.Compose(pending)
    } else if (pending != null && pendingCertificateRetry == PendingCertificateRetry.Compose(pending)) {
        pendingCertificateRetry = null
    }
}

private fun MeronMobileState.composeDraftOwnerSnapshot(): List<ComposeDraftOwner> =
    buildList {
        if (composeDraftSaved && composeDraftAccountId.isNotBlank() && composeDraftId.isNotBlank()) {
            add(ComposeDraftOwner(composeDraftAccountId, composeDraftId, composeDraftThreadId()))
        }
        addAll(composeDraftCleanupOwners)
    }.distinctBy { it.accountId to it.draftId }

/** The thread a compose draft belongs to, blank unless composing from a thread. */
private fun MeronMobileState.composeDraftThreadId(): String = selectedCoreThread?.takeIf { composeReturnScreen == Screen.Thread }?.id.orEmpty()

private data class ComposeDraftDiscardOutcome(
    val clearedThreadIds: Set<String> = emptySet(),
    val failedOwners: List<ComposeDraftOwner> = emptyList(),
)

private suspend fun MeronMobileState.discardComposeDraftOwners(owners: List<ComposeDraftOwner>): ComposeDraftDiscardOutcome {
    val clearedThreadIds = mutableSetOf<String>()
    val failedOwners = mutableListOf<ComposeDraftOwner>()
    owners.forEach { owner ->
        val discarded =
            runCatching {
                withContext(ioDispatcher) {
                    MobileMailCommandClient(core).discardDraft(
                        DiscardDraftParams(accountId = owner.accountId, draftId = owner.draftId),
                    )
                }
            }.isSuccess
        if (discarded) {
            composeDraftCleanupOwners = composeDraftCleanupOwners - owner
            // A discard can also happen on a later retry from saveComposeDraft, so
            // the local copy of the draft is dropped here rather than at the send.
            removeDiscardedDraftFromOpenThread(owner.draftId, owner.threadId)?.let { clearedThreadIds.add(it) }
        } else {
            failedOwners.add(owner)
        }
    }
    return ComposeDraftDiscardOutcome(clearedThreadIds, failedOwners)
}

/**
 * Delete the server-side draft of a composer the user has emptied out, mirroring
 * the reply bar's [discardQuickReplyDraftIfEmpty]. Reports whether anything was
 * deleted.
 */
private suspend fun MeronMobileState.discardEmptiedComposeDraft(generation: Int): Boolean {
    val openDraft =
        ComposeDraftOwner(composeDraftAccountId, composeDraftId, composeDraftThreadId())
            .takeIf { composeDraftSaved && it.accountId.isNotBlank() && it.draftId.isNotBlank() }
    // The composer's own draft is deliberately not queued in
    // composeDraftCleanupOwners: it keeps its id, so a failed discard followed by
    // the user typing again would save under that id and then have the cleanup
    // delete the replacement. A retry comes from the next blank save instead.
    val owners = (listOfNotNull(openDraft) + composeDraftCleanupOwners).distinctBy { it.accountId to it.draftId }
    if (owners.isEmpty()) return false
    val outcome = discardComposeDraftOwners(owners)
    if (outcome.failedOwners.isNotEmpty()) return false
    // A newer composer may have opened while the discard was in flight; its draft
    // id and account are not this session's to clear.
    if (generation != composeSessionGeneration) return false
    composeDraftId = ""
    composeDraftSaved = false
    composeDraftAccountId = ""
    syncCoreThreads(syncFirst = false)
    return true
}

internal fun MeronMobileState.saveComposeDraft() {
    val generation = composeSessionGeneration
    scope.launch(start = CoroutineStart.UNDISPATCHED) {
        saveComposeDraft(showStatus = true, generation = generation)
    }
}

internal fun MeronMobileState.autoSaveComposeDraft() {
    val generation = composeSessionGeneration
    scope.launch(start = CoroutineStart.UNDISPATCHED) {
        saveComposeDraft(showStatus = false, generation = generation)
    }
}

private suspend fun MeronMobileState.saveComposeDraft(
    showStatus: Boolean,
    generation: Int,
    keepObsoleteDraft: Boolean = false,
): Boolean {
    return composeSaveMutex.withLock {
        if (generation != composeSessionGeneration) return@withLock false
        // A send is about to discard the draft; saving now could resurrect it.
        if (composeSendInFlight) return@withLock false
        if (composeSignaturePending || !appSignatureLoaded) {
            if (showStatus) status = "Waiting for signature before saving."
            return@withLock false
        }
        val identityGeneration = composeIdentityGeneration
        val identity = selectedComposeIdentity()
        val accountId = identity?.accountId ?: defaultSendAccountId()
        if (accountId.isBlank()) {
            if (showStatus) status = "Select or add an account before saving."
            return@withLock false
        }
        val draft = currentComposeDraft()
        if (composeIsBlank()) {
            // Everything the user wrote is gone. A copy already on the server has
            // to go with it, or closing and reopening would bring back the text
            // they deliberately erased.
            val discarded = discardEmptiedComposeDraft(generation)
            // Same reason the save path rechecks below: a status line belongs to
            // whichever composer is open now.
            if (generation != composeSessionGeneration) return@withLock false
            if (showStatus) status = if (discarded) "Draft discarded" else "Nothing to save."
            return@withLock false
        }
        val draftId = composeDraftId.ifBlank { newDraftMessageId(accountId) }
        val cleanupOwners = composeDraftCleanupOwners
        val draftThreadId = composeDraftThreadId().ifBlank { null }
        val inReplyTo = composeInReplyTo
        val references = composeReferences
        if (showStatus) status = "Saving draft..."
        var resolvedDraftId = draftId
        var allocatedRemoteDraft = false
        val result =
            runCatching {
                withContext(ioDispatcher) {
                    val client = MobileMailCommandClient(core)
                    resolvedDraftId =
                        if (draftId.startsWith("local-draft-")) {
                            allocateCoreMessageId(client, accountId, draft = true).also { allocatedRemoteDraft = true }
                        } else {
                            draftId
                        }
                    val params =
                        draft
                            .toSaveDraftParams(
                                accountId = accountId,
                                draftId = resolvedDraftId,
                                from = identity?.email.orEmpty(),
                            ).copy(
                                inReplyTo = inReplyTo,
                                references = references,
                            )
                    withManagedGoogleAuth(client, accountId) { client.saveDraft(params) }
                    resolvedDraftId
                }
            }
        val sessionObsolete = generation != composeSessionGeneration
        val identityObsolete = identityGeneration != composeIdentityGeneration
        val obsolete = sessionObsolete || identityObsolete || composeSendInFlight
        if (obsolete) {
            val keepClosingDraft = keepObsoleteDraft && sessionObsolete && result.isSuccess
            if (keepClosingDraft) {
                discardComposeDraftOwners(cleanupOwners)
                draftThreadId?.let { markThreadDraftEverywhere(it) }
                syncCoreThreads(syncFirst = false)
                return@withLock true
            }
            if (allocatedRemoteDraft && resolvedDraftId.isNotBlank()) {
                discardObsoleteComposeDraft(accountId, resolvedDraftId)
            }
            return@withLock false
        }
        result.fold(
            onSuccess = { savedDraftId ->
                composeDraftId = savedDraftId
                composeDraftSaved = true
                composeDraftAccountId = accountId
                discardComposeDraftOwners(cleanupOwners)
                selectedCoreThread?.let { markThreadDraftEverywhere(it.id) }
                if (showStatus) status = "Draft saved"
                syncCoreThreads(syncFirst = false)
                runCatching { reloadCurrentThreadMessages() }
                true
            },
            onFailure = {
                status =
                    if (showStatus) {
                        "Draft save failed: ${it.message}"
                    } else {
                        "Draft autosave failed: ${it.message}"
                    }
                false
            },
        )
    }
}

private suspend fun MeronMobileState.discardObsoleteComposeDraft(
    accountId: String,
    draftId: String,
) {
    runCatching {
        withContext(ioDispatcher) {
            MobileMailCommandClient(core).discardDraft(DiscardDraftParams(accountId = accountId, draftId = draftId))
        }
    }
}

// The message a quick reply answers: the newest incoming message that isn't a
// draft. Keeps mail we sent out of recipient/threading derivation — see
// [sentByUs] for what counts as ours.
internal fun MeronMobileState.quickReplyParent(): MessageBody? {
    val accountId = selectedCoreThread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    val account = coreAccounts.firstOrNull { it.id == accountId }
    val ownAddresses =
        account
            ?.let(::accountSendIdentities)
            ?.map { it.email.trim().lowercase() }
            ?.filter { it.isNotBlank() }
            ?.toSet()
            .orEmpty()

    return messages.lastOrNull { !folderIsDrafts(it.folderId) && !it.sentByUs(ownAddresses) }
        ?: messages.lastOrNull { !folderIsDrafts(it.folderId) }
        ?: messages.lastOrNull()
}

// Whether a loaded message is one we sent, as opposed to one we received. The
// core settles this from the message's own delivery headers whenever it has the
// body cached. Until then `outgoing` — like the address check kept here for rows
// shaped before that flag existed — falls back to matching From against our
// identities, which also fires for a colleague's mail from a shared alias;
// sitting in the inbox vetoes that match. An optimistic send is ours whatever
// folder it claims.
private fun MessageBody.sentByUs(ownAddresses: Set<String>): Boolean {
    if (sendStatus != SendStatus.None) return true
    if (folderIsInbox(folderId)) return false
    return outgoing || ownAddresses.contains(fromAddr.trim().lowercase())
}

// Continue a thread with the configured identity used by its most recent
// outgoing message. Null means there is no such message; blank means primary.
private fun MeronMobileState.detectRecentThreadFrom(account: AccountSummary): String? {
    val identities = accountSendIdentities(account)
    val byEmail = identities.associateBy { it.email.trim().lowercase() }
    val ownAddresses = identities.map { it.email.trim().lowercase() }.filter { it.isNotBlank() }.toSet()
    for (message in messages.asReversed()) {
        if (folderIsDrafts(message.folderId)) continue
        if (!message.sentByUs(ownAddresses)) continue
        val identity = byEmail[message.fromAddr.trim().lowercase()] ?: continue
        return if (identity.email.equals(account.email, ignoreCase = true)) "" else identity.email
    }
    return null
}

// The address the quick reply sends from: the identity picked in the reply bar's
// From row, the identity used by the newest outgoing message, or the alias the
// inbound parent was delivered to. Blank means the account primary, which the
// send and draft paths read as "use the default".
internal fun MeronMobileState.resolveQuickReplyFrom(
    parent: MessageBody,
    account: AccountSummary?,
): String {
    if (account == null) return ""
    if (quickReplyFrom.isNotBlank()) {
        return if (quickReplyFrom.equals(account.email, ignoreCase = true)) "" else quickReplyFrom
    }
    detectRecentThreadFrom(account)?.let { return it }
    return detectReplyFromIdentity(parent, account)
}

// Identities the open thread's quick reply can send as. Empty when there is
// nothing to choose between (no thread, unknown account, or a single address) —
// the reply bar hides its From row rather than stating the obvious.
internal fun MeronMobileState.quickReplyIdentities(): List<SendIdentity> {
    val thread = selectedCoreThread ?: return emptyList()
    val accountId = thread.accountId.ifBlank { defaultSendAccountId() }
    val account = coreAccounts.firstOrNull { it.id == accountId } ?: return emptyList()
    val identities = accountSendIdentities(account)
    return if (identities.size < 2) emptyList() else identities
}

// The identity the reply bar's From row shows as current — the resolved send-as
// address matched back to the pickable list.
internal fun MeronMobileState.selectedQuickReplyIdentity(): SendIdentity? {
    val identities = quickReplyIdentities()
    if (identities.isEmpty()) return null
    val account = coreAccounts.firstOrNull { it.id == identities.first().accountId } ?: return null
    val parent = quickReplyParent()
    val email = (parent?.let { resolveQuickReplyFrom(it, account) } ?: quickReplyFrom).ifBlank { account.email }
    return identities.firstOrNull { it.email.equals(email, ignoreCase = true) } ?: identities.first()
}

/**
 * The account the open thread's quick reply sends from. Blank when there is no
 * thread, or none of the accounts can send.
 */
private fun MeronMobileState.quickReplyAccountId(): String {
    val thread = selectedCoreThread ?: return ""
    if (threadIdIsRss(thread.id)) return ""
    return thread.accountId.ifBlank { defaultSendAccountId() }
}

/**
 * Seed the reply bar with the replying account's signature, as the box the user
 * starts typing into rather than something stapled on at send time — the rule
 * the full composer follows, and what every other mail client shows.
 *
 * Replaces whatever the box holds, so it is only ever called on a fresh quick
 * reply: a thread switch, or the clear after a send or an escalation. The app
 * signature is read back from the core asynchronously, so a box seeded before
 * it lands is seeded again once it does — unless the user has since typed.
 */
internal fun MeronMobileState.seedQuickReplySignature() {
    val threadId = quickReplyThreadId
    val signature = quickReplyAccountId().takeIf { it.isNotBlank() }?.let { signatureTextFor(it) }.orEmpty()
    quickReplyBody = bodyWithSignature("", signature)
    quickReplySignature = if (signature.isBlank()) null else SignatureMark(signature, SignaturePlacement.BelowText)
    if (appSignatureLoaded) return
    scope.launch {
        awaitAppSignatureLoaded()
        if (quickReplyThreadId == threadId && quickReplyDraftId.isBlank() && quickReplyIsBlank()) {
            seedQuickReplySignature()
        }
    }
}

/**
 * Re-seed a reply bar the user has not written in yet, so it isn't left holding
 * a signature that is no longer the one it would send. Called whenever the
 * app-wide or an account's signature is rewritten — the settings screen can be
 * opened and left with the thread still behind it, and the bar is what gets
 * sent. A bar with anything of theirs in it — text, an attachment, a hydrated
 * draft — is never rewritten.
 */
internal fun MeronMobileState.reseedUntouchedQuickReply() {
    if (quickReplyThreadId.isBlank()) return
    if (quickReplyDraftSaved || quickReplyDraftId.isNotBlank()) return
    if (!quickReplyIsBlank()) return
    seedQuickReplySignature()
}

/**
 * The reply bar with the signature this app seeded taken back out, and whether
 * it was still there to take. A signature the user has typed into can no longer
 * be identified (see [bodyWithSwappedSignature]), which is the same answer as
 * "this text is theirs now".
 */
private fun MeronMobileState.quickReplyWithoutSignature(): Pair<String, Boolean> {
    val mark = quickReplySignature ?: return quickReplyBody to false
    if (mark.text.isBlank()) return quickReplyBody to false
    val swapped = bodyWithSwappedSignature(quickReplyBody, mark, "")
    return if (swapped.tracking == null) quickReplyBody to false else swapped.body to true
}

/**
 * Whether the reply bar holds nothing the user put there. A seeded signature
 * does not count as content: it is not something they wrote, and treating it as
 * such would save a draft for every thread they merely open, and let an
 * untouched box be "sent".
 */
internal fun MeronMobileState.quickReplyIsBlank(): Boolean = quickReplyWithoutSignature().first.isBlank() && quickReplyAttachments.isEmpty()

private suspend fun MeronMobileState.saveQuickReplyDraft(showStatus: Boolean): Boolean =
    quickReplySaveMutex.withLock {
        saveQuickReplyDraftLocked(showStatus)
    }

private suspend fun MeronMobileState.saveQuickReplyDraftLocked(showStatus: Boolean): Boolean {
    // A send is about to discard the draft; saving now could resurrect it.
    if (quickReplySendInFlight) return false
    val thread = selectedCoreThread
    val generation = quickReplyGeneration
    val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    val parent = quickReplyParent()
    if (accountId.isBlank() || thread == null || parent == null) {
        if (showStatus) status = "Open a mail thread before saving a reply draft."
        return false
    }
    if (quickReplyIsBlank()) {
        if (showStatus) status = "Nothing to save."
        return false
    }
    val account = coreAccounts.firstOrNull { it.id == accountId }
    val replyFrom = resolveQuickReplyFrom(parent, account)
    val replyParams =
        parent.toReplyMailParams(
            accountId = accountId,
            body = quickReplyBody.trim(),
            from = replyFrom,
            ownAddresses = ownAddressList(coreAccounts),
            attachments = quickReplyAttachments,
        )
    val draftId = quickReplyDraftId.ifBlank { newDraftMessageId(accountId) }
    quickReplyInReplyTo = replyParams.inReplyTo
    quickReplyReferences = replyParams.references
    val draft = ComposeDraft(replyParams.to, replyParams.cc, "", replyParams.subject, quickReplyBody.trim(), quickReplyAttachments)
    if (showStatus) status = "Saving draft..."
    return runCatching {
        withContext(ioDispatcher) {
            val client = MobileMailCommandClient(core)
            val resolvedDraftId =
                if (draftId.startsWith("local-draft-")) allocateCoreMessageId(client, accountId, draft = true) else draftId
            val params =
                draft
                    .toSaveDraftParams(
                        accountId = accountId,
                        draftId = resolvedDraftId,
                        from = replyFrom,
                    ).copy(
                        inReplyTo = replyParams.inReplyTo,
                        references = replyParams.references,
                    )
            withManagedGoogleAuth(client, accountId) { client.saveDraft(params) }
            resolvedDraftId
        }
    }.fold(
        onSuccess = { savedDraftId ->
            val threadId = thread.backendThreadId()
            val sameEditor = selectedCoreThread?.backendThreadId() == threadId && quickReplyThreadId == threadId
            val newlyAllocated = draftId.startsWith("local-draft-")
            if (sameEditor && newlyAllocated && quickReplyDraftId.isBlank()) {
                // Publish the allocated id even if the text changed or a send is
                // waiting on the save lock. The next save reuses it, and the
                // waiting send can capture it as the draft it must discard.
                quickReplyDraftId = savedDraftId
                quickReplyDraftSaved = true
                markThreadDraftEverywhere(threadId)
                true
            } else if (quickReplyGeneration != generation || !sameEditor || quickReplySendInFlight) {
                // The save still belongs to the editor that started it. Keep its
                // remote draft, but do not let its completion mutate another
                // thread or a newer version of this reply.
                //
                // A send is waiting on this save's lock and cannot read the id
                // off the bar — the bar has moved on, or never held it — so hand
                // it over directly. Without this the copy just written is the one
                // left in Drafts beside the reply that send is about to deliver.
                if (quickReplySendInFlight) quickReplySendDraftHandover = threadId to savedDraftId
                true
            } else {
                quickReplyDraftId = savedDraftId
                quickReplyDraftSaved = true
                markThreadDraftEverywhere(threadId)
                if (showStatus) status = "Draft saved"
                true
            }
        },
        onFailure = {
            if (showStatus) status = "Draft save failed: ${it.message}"
            false
        },
    )
}

// Hides the draft hydrated into quick reply wherever it sits in the loaded
// conversation. An optimistic sent bubble is appended after it, so tail-only
// matching would reveal the draft again while sending.
internal fun MeronMobileState.visibleThreadMessages(): List<MessageBody> {
    // Drafts a send has taken over are hidden too, until their discard comes
    // back: navigating away clears the bar's id, so reopening the conversation
    // mid-send would otherwise show the draft beside the reply it was sent as.
    val hidden = quickReplyConsumedDraftIds.toMutableSet()
    if (quickReplyDraftId.isNotBlank()) hidden += quickReplyDraftId.normalizedComposeDraftId()
    if (hidden.isEmpty()) return messages
    return messages.filterNot {
        folderIsDrafts(it.folderId) && it.messageId.normalizedComposeDraftId() in hidden
    }
}

internal fun MeronMobileState.autoSaveQuickReplyDraft() {
    scope.launch {
        saveQuickReplyDraft(showStatus = false)
    }
}

// Flushes any pending debounced autosave immediately — used when navigating
// away from the thread screen, mirroring closeCompose()'s autosave-on-close
// for the full composer, so the last few keystrokes aren't lost to the
// debounce window.
internal fun MeronMobileState.flushQuickReplyAutosave() {
    quickReplyAutosaveJob?.cancel()
    quickReplyAutosaveJob = null
    if (quickReplyIsBlank()) {
        discardQuickReplyDraftIfEmpty()
    } else {
        autoSaveQuickReplyDraft()
    }
}

internal fun MeronMobileState.discardQuickReplyDraftIfEmpty() {
    if (!quickReplyIsBlank()) return
    val draftId = quickReplyDraftId.takeIf { quickReplyDraftSaved } ?: return
    val thread = selectedCoreThread ?: return
    val accountId = thread.accountId.ifBlank { defaultSendAccountId() }
    quickReplyDraftId = ""
    quickReplyDraftSaved = false
    quickReplyInReplyTo = ""
    quickReplyReferences = ""
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                withManagedGoogleAuth(client, accountId) {
                    client.discardDraft(
                        DiscardDraftParams(accountId = accountId, draftId = draftId),
                    )
                }
            }
        }.onSuccess {
            syncCoreThreads(syncFirst = false)
            val normalizedDraftId = draftId.normalizedComposeDraftId()
            selectedCoreThread =
                selectedCoreThread?.copy(
                    hasDraft =
                        messages.any {
                            it.messageId.normalizedComposeDraftId() != normalizedDraftId && folderIsDrafts(it.folderId)
                        },
                )
        }
    }
}

internal fun MeronMobileState.onQuickReplyBodyChange(value: String) {
    quickReplyBody = value
    ++quickReplyGeneration
    quickReplyFailure = ""
    quickReplyAutosaveJob?.cancel()
    quickReplyAutosaveJob =
        scope.launch {
            delay(1200)
            if (quickReplyIsBlank()) {
                discardQuickReplyDraftIfEmpty()
            } else {
                saveQuickReplyDraft(showStatus = false)
            }
        }
}

internal fun MeronMobileState.openQuickReplyInFullEditor() {
    val thread = selectedCoreThread
    val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    val parent = quickReplyParent()
    if (accountId.isBlank() || thread == null || parent == null) {
        status = "Open a mail thread before replying."
        return
    }
    if (threadIdIsRss(thread.id)) {
        status = "RSS items do not support replies."
        return
    }
    ++quickReplyGeneration
    val replyFrom = resolveQuickReplyFrom(parent, coreAccounts.firstOrNull { it.id == accountId })
    // The seeded signature is handed over stripped, so the full composer inserts
    // and tracks its own copy — the account is the same, so this is the identical
    // text, now swappable if the draft later changes identity. When it can't be
    // found the user has written into it: it stays in the body as theirs, and the
    // composer must not add a second.
    val (carriedBody, signatureStripped) = quickReplyWithoutSignature()
    val carriesEditedSignature = !signatureStripped && quickReplySignature?.text?.isNotBlank() == true
    val params =
        parent.toReplyMailParams(
            accountId = accountId,
            body = carriedBody.trim(),
            from = replyFrom,
            ownAddresses = ownAddressList(coreAccounts),
            attachments = quickReplyAttachments,
        )
    val generation = ++composeSessionGeneration
    val open: MeronMobileState.() -> Unit = open@{
        if (generation != composeSessionGeneration) return@open
        composeSignaturePending = false
        ++composeIdentityGeneration
        to = params.to
        cc = params.cc
        bcc = params.bcc
        subject = params.subject
        body =
            if (carriesEditedSignature) {
                composeSignature = null
                params.body
            } else {
                seedBodyWithSignature(params.body, accountId)
            }
        attachments = quickReplyAttachments
        // A quick reply is plain text; nothing carries over from an earlier forward.
        composeForwardHtml = ""
        composeForwardInlineAttachments = emptyList()
        composeFromAccountId = accountId
        composeFromEmail = replyFrom
        // Hand off any draft already saved for this quick reply so continuing in the
        // full editor keeps editing the same server-side draft instead of creating a
        // duplicate one.
        composeDraftId = quickReplyDraftId
        composeDraftSaved = quickReplyDraftSaved
        composeDraftAccountId = if (quickReplyDraftSaved) accountId else ""
        composeInReplyTo = params.inReplyTo
        composeReferences = params.references
        quickReplyAutosaveJob?.cancel()
        quickReplyAttachments = emptyList()
        quickReplyFailure = ""
        quickReplyDraftId = ""
        quickReplyDraftSaved = false
        quickReplyInReplyTo = ""
        quickReplyReferences = ""
        quickReplyFrom = ""
        // The thread is still behind the composer, so the bar the user comes
        // back to is a fresh quick reply — signature and all.
        seedQuickReplySignature()
        composeReturnScreen = Screen.Thread
        rememberComposeSeed()
        screen = Screen.Compose
        status = ""
    }
    if (appSignatureLoaded) {
        open()
    } else {
        scope.launch {
            awaitAppSignatureLoaded()
            open()
        }
    }
}

internal fun MeronMobileState.discardComposeDraft() {
    ++composeSessionGeneration
    val identity = selectedComposeIdentity()
    val accountId = composeDraftAccountId.ifBlank { identity?.accountId ?: defaultSendAccountId() }
    val draftId = composeDraftId.takeIf { composeDraftSaved }
    val draftOwners =
        buildList {
            if (!draftId.isNullOrBlank() && accountId.isNotBlank()) add(ComposeDraftOwner(accountId, draftId, composeDraftThreadId()))
            addAll(composeDraftCleanupOwners)
        }.distinctBy { it.accountId to it.draftId }
    composeDraftCleanupOwners = draftOwners
    val returnScreen = composeReturnScreen
    val thread = selectedCoreThread
    val draftThread = thread?.takeIf { folderIsDrafts(it.folder) }
    if (!draftId.isNullOrBlank() && accountId.isBlank()) {
        status = "Select or add an account before discarding."
        return
    }
    val previousMessages = messages
    val previousThread = selectedCoreThread
    val previousThreads = coreThreads
    val previousKanbanColumns = kanbanColumns
    val optimisticallyClearedThreadId = removeDiscardedDraftFromOpenThread(draftId, composeDraftThreadId())
    if (draftThread != null) {
        removeThreadEverywhere(draftThread.id)
        locallyDiscardedThreadIds = locallyDiscardedThreadIds + draftThread.id
    }
    clearComposeDraftState()
    screen = returnScreen
    status = "Discarding draft..."
    scope.launch(start = CoroutineStart.UNDISPATCHED) {
        composeSaveMutex.withLock {
            val outcome = discardComposeDraftOwners(draftOwners)
            if (outcome.failedOwners.isEmpty()) {
                status = "Draft discarded"
                syncCoreThreads(syncFirst = true)
                if (thread != null) {
                    refreshKanbanColumnsForMailEvent(accountId, thread.folder, refresh = true)
                }
                if (draftThread == null) {
                    runCatching { reloadCurrentThreadMessages() }
                }
            } else {
                val openDraftFailed = draftOwners.firstOrNull { it.draftId == draftId } in outcome.failedOwners
                // Roll back only the drafts that survived: one owner failing must not
                // bring back a draft another owner really did delete. The open
                // draft's own thread is restored only when its draft is the one that
                // stayed behind.
                if (openDraftFailed) {
                    restoreUndiscardedDraftMessages(previousMessages, outcome.failedOwners)
                    selectedCoreThread = previousThread
                    coreThreads = previousThreads
                    kanbanColumns = previousKanbanColumns
                    if (draftThread != null) {
                        locallyDiscardedThreadIds = locallyDiscardedThreadIds - draftThread.id
                    }
                }
                // The snapshots above predate the per-owner clearing, and whole-map
                // restores would also drop a marker set meanwhile by an autosave for
                // another thread, so the flags are re-applied one thread at a time.
                outcome.clearedThreadIds.forEach { clearThreadDraftEverywhere(it) }
                outcome.failedOwners.forEach { markThreadDraftEverywhere(it.threadId) }
                if (openDraftFailed) {
                    optimisticallyClearedThreadId?.let { markThreadDraftEverywhere(it) }
                }
                status = "Draft discard failed: one or more drafts could not be discarded"
            }
        }
    }
}

/**
 * Drop a discarded draft from the open thread, returning the id of the thread
 * whose draft marker was cleared, or null when nothing was cleared.
 * [draftThreadId] is the thread the caller knows the draft belongs to.
 */
internal fun MeronMobileState.removeDiscardedDraftFromOpenThread(
    draftId: String?,
    draftThreadId: String = "",
): String? {
    val normalizedDraftId = draftId?.normalizedComposeDraftId().orEmpty()
    if (normalizedDraftId.isBlank()) return null
    val remaining =
        messages.filterNot { message ->
            message.id == "local-draft-$normalizedDraftId" ||
                message.messageId.normalizedComposeDraftId() == normalizedDraftId
        }
    val foundInOpenThread = remaining.size != messages.size
    // Finding the draft in the open thread proves it lived there; otherwise only
    // the caller knows. Absence proves nothing on its own: a quick reply saves a
    // draft without adding a message locally, the list is empty while a thread
    // loads, and a cleanup owner can belong to a thread that is no longer open.
    val threadId = if (foundInOpenThread) selectedCoreThread?.id.orEmpty() else draftThreadId
    if (threadId.isBlank()) return null
    if (foundInOpenThread) messages = remaining
    // Another draft still sitting in the thread keeps the marker on.
    if (threadId == selectedCoreThread?.id && remaining.any { folderIsDrafts(it.folderId) }) {
        selectedCoreThread = selectedCoreThread?.copy(hasDraft = true)
        return null
    }
    clearThreadDraftEverywhere(threadId)
    return threadId
}

/**
 * Puts back the messages of drafts a failed discard left behind, at roughly
 * their old place, keeping whatever arrived while the discard was in flight.
 */
private fun MeronMobileState.restoreUndiscardedDraftMessages(
    previousMessages: List<MessageBody>,
    owners: List<ComposeDraftOwner>,
) {
    val draftIds = owners.map { it.draftId.normalizedComposeDraftId() }.toSet()
    val presentIds = messages.map { it.id }.toSet()
    val restored = messages.toMutableList()
    previousMessages.forEachIndexed { index, message ->
        if (message.id in presentIds) return@forEachIndexed
        val belongsToOwner =
            draftIds.any { draftId ->
                message.id == "local-draft-$draftId" || message.messageId.normalizedComposeDraftId() == draftId
            }
        if (belongsToOwner) restored.add(index.coerceAtMost(restored.size), message)
    }
    messages = restored
}

internal fun String.normalizedComposeDraftId(): String = trim().trim('<', '>').lowercase()

internal fun MeronMobileState.sendQuickReply() {
    if (quickReplySendInFlight) return
    val thread = selectedCoreThread
    val threadId = thread?.backendThreadId().orEmpty()
    val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    val parent = quickReplyParent()
    val sentBody = quickReplyBody.trim()
    val sentAttachments = quickReplyAttachments
    if (accountId.isBlank() || thread == null || parent == null) {
        status = "Open a mail thread before replying."
        return
    }
    if (threadIdIsRss(thread.id)) {
        status = "RSS items do not support replies."
        return
    }
    // A box holding only the seeded signature is not a reply worth sending.
    if (quickReplyIsBlank()) {
        status = "Write a reply or attach a file before sending."
        return
    }
    quickReplyFailure = ""
    quickReplyAutosaveJob?.cancel()
    quickReplySendInFlight = true
    pendingCertificateRetry = null
    pendingQuickReplySend = null
    val generation = quickReplyGeneration
    // The bar's draft as it stood at the click. Read here rather than in the
    // coroutine below, which starts a turn later: a tap that leaves the
    // conversation in between would clear the bar, and this send would have no
    // owner to discard.
    val claimedDraftOwner =
        quickReplyDraftId
            .takeIf { quickReplyDraftSaved && it.isNotBlank() }
            ?.let { ComposeDraftOwner(accountId, it, threadId) }
    // Hold it from the click too. The allocation below is long enough to leave
    // the conversation and come back, and the draft rehydrated in between holds
    // the text being sent — indistinguishable, by the time the send settles,
    // from a newer reply the user has started.
    claimedDraftOwner?.let { quickReplyConsumedDraftIds += it.draftId.normalizedComposeDraftId() }
    val account = coreAccounts.firstOrNull { it.id == accountId }
    val replyFrom = resolveQuickReplyFrom(parent, account)
    val baseParams =
        parent.toReplyMailParams(
            accountId = accountId,
            body = sentBody,
            from = replyFrom,
            ownAddresses = ownAddressList(coreAccounts),
            attachments = sentAttachments,
        )
    // Render the sent bubble optimistically — before the send round-trip — so
    // replying feels instant. The bubble shows a "Sending…" status until the
    // canonical stored message replaces it on re-fetch; on failure it flips to
    // "Failed" and stays visible so the reply isn't lost. The reply-bar text
    // itself is left populated until the send actually succeeds, so a failed
    // send can be retried with its real content instead of resending blank.
    // A counter suffix keeps ids unique even for two sends in the same
    // millisecond — message ids key the conversation list, duplicates crash.
    val tempId = "local-send-${currentTimeMillis()}-${localSendSequence++}"
    scope.launch {
        val pending =
            quickReplySaveMutex.withLock {
                // Settle who owns the draft this reply consumed *before* the
                // identity allocation below: that is a round trip, and until the
                // owner is resolved and held, reopening the conversation can
                // hydrate the very draft being sent, and a failed allocation can
                // leave a handover behind for some later send to act on.
                //
                // Any autosave from before the click has finished by now — it
                // held this same lock — so whatever it handed over is here.
                val resolvedOwner =
                    claimedDraftOwner
                        ?: quickReplyDraftId
                            .takeIf { quickReplyDraftSaved && it.isNotBlank() && quickReplyThreadId == threadId }
                            ?.let { ComposeDraftOwner(accountId, it, threadId) }
                        ?: quickReplySendDraftHandover
                            ?.takeIf { it.first == threadId }
                            ?.let { ComposeDraftOwner(accountId, it.second, threadId) }
                quickReplySendDraftHandover = null
                resolvedOwner?.let { quickReplyConsumedDraftIds += it.draftId.normalizedComposeDraftId() }
                val outboundMessageId =
                    runCatching {
                        withContext(ioDispatcher) { allocateCoreMessageId(MobileMailCommandClient(core), accountId, draft = false) }
                    }.getOrElse {
                        quickReplySendInFlight = false
                        resolvedOwner?.let { owner ->
                            quickReplyConsumedDraftIds -= owner.draftId.normalizedComposeDraftId()
                        }
                        quickReplyFailure = it.message.orEmpty()
                        status = "Send failed: ${it.message}"
                        return@launch
                    }
                val params = baseParams.copy(messageId = outboundMessageId)
                val optimistic =
                    MessageBody(
                        id = tempId,
                        folderId = parent.folderId,
                        from = "You",
                        fromAddr = replyFrom.ifBlank { account?.email.orEmpty() },
                        to = params.to,
                        cc = params.cc,
                        subject = params.subject,
                        body = sentBody,
                        messageId = outboundMessageId,
                        references = params.references,
                        dateEpochSeconds = currentTimeMillis() / 1000,
                        hasAttachments = sentAttachments.isNotEmpty(),
                        attachments =
                            sentAttachments.map {
                                MessageAttachment(filename = it.displayName, mimeType = it.mimeType, sizeBytes = it.sizeBytes)
                            },
                        sendStatus = SendStatus.Sending,
                    )
                messages = messages + optimistic
                PendingQuickReplySend(
                    accountId = accountId,
                    params = params,
                    tempMessageId = tempId,
                    threadId = threadId,
                    draftOwner = resolvedOwner,
                    quickReplyGeneration = generation,
                )
            }
        pendingQuickReplySend = pending
        status = "Sending reply..."
        dispatchQuickReplySend(pending)
    }
}

internal fun MeronMobileState.retryQuickReplySend() {
    val pending = pendingQuickReplySend ?: return
    if (!quickReplyEditorOwns(pending)) return
    retryQuickReplySend(pending)
}

internal fun MeronMobileState.retryQuickReplySend(pending: PendingQuickReplySend) {
    if (quickReplySendInFlight) return
    pendingQuickReplySend = pending
    quickReplySendInFlight = true
    quickReplyFailure = ""
    messages = messages.map { if (it.id == pending.tempMessageId) it.copy(sendStatus = SendStatus.Sending) else it }
    status = "Sending reply..."
    scope.launch { dispatchQuickReplySend(pending) }
}

private suspend fun MeronMobileState.dispatchQuickReplySend(pending: PendingQuickReplySend) {
    // Claim the draft this reply consumed for as long as the send is settling,
    // so reopening the conversation before the discard returns cannot hydrate
    // the sent text back into the reply bar.
    val consumedDraftId =
        pending.draftOwner
            ?.draftId
            ?.normalizedComposeDraftId()
            .orEmpty()
    if (consumedDraftId.isNotBlank()) quickReplyConsumedDraftIds += consumedDraftId
    runCatching {
        withContext(ioDispatcher) {
            val client = MobileMailCommandClient(core)
            withManagedGoogleAuth(client, pending.accountId) { client.send(pending.params) }
        }
    }.onSuccess {
        val sameEditorGeneration = quickReplyGeneration == pending.quickReplyGeneration
        // The bar has carried on writing into this same draft — it moved on but
        // kept the id — so it holds the user's next reply now, not the text that
        // just went out, and the autosave following this send will write theirs
        // over it. That is the one case where the draft stays.
        val barKeptTheDraft =
            !sameEditorGeneration &&
                quickReplyThreadId == pending.draftOwner?.threadId &&
                quickReplyDraftId.normalizedComposeDraftId() == pending.draftOwner?.draftId?.normalizedComposeDraftId()
        // Everything else — the bar untouched, or moved to another thread or
        // another draft — leaves nobody pointing at the consumed copy, and not
        // discarding it is what strands it in Drafts beside the sent reply.
        var consumedDraftDiscarded = false
        pending.draftOwner?.takeIf { !barKeptTheDraft }?.let { owner ->
            consumedDraftDiscarded =
                runCatching {
                    withContext(ioDispatcher) {
                        MobileMailCommandClient(core).discardDraft(
                            DiscardDraftParams(accountId = owner.accountId, draftId = owner.draftId),
                        )
                    }
                }.isSuccess
            if (consumedDraftDiscarded) removeDiscardedDraftFromOpenThread(owner.draftId, owner.threadId)
        }
        if (consumedDraftId.isNotBlank()) quickReplyConsumedDraftIds -= consumedDraftId
        if (pendingQuickReplySend == pending) pendingQuickReplySend = null
        if (pendingCertificateRetry == PendingCertificateRetry.QuickReply(pending)) pendingCertificateRetry = null
        quickReplySendInFlight = false
        if (sameEditorGeneration) {
            quickReplyFailure = ""
            quickReplyAttachments = emptyList()
            quickReplyDraftId = ""
            quickReplyDraftSaved = false
            quickReplyInReplyTo = ""
            quickReplyReferences = ""
            ++quickReplyGeneration
            seedQuickReplySignature()
        }
        status = "Reply sent"
        val threadStillOpen = selectedCoreThread?.backendThreadId() == pending.threadId
        if (threadStillOpen) {
            messages = messages.map { if (it.id == pending.tempMessageId) it.copy(sendStatus = SendStatus.None) else it }
        }
        errorBanner = null
        syncCoreThreads(syncFirst = false)
        if (threadStillOpen) {
            runCatching { reloadCurrentThreadMessages() }.onSuccess {
                // The read can race the server-side discard and return its stale
                // pre-discard row. The discard already succeeded, so reconcile
                // that row once more after applying the refreshed conversation.
                pending.draftOwner?.takeIf { !barKeptTheDraft && consumedDraftDiscarded }?.let { owner ->
                    removeDiscardedDraftFromOpenThread(owner.draftId, owner.threadId)
                }
            }
        }
    }.onFailure {
        // A failed send leaves the draft as the safety net it was written to be.
        if (consumedDraftId.isNotBlank()) quickReplyConsumedDraftIds -= consumedDraftId
        quickReplySendInFlight = false
        val message = it.message ?: "Send failed"
        status = "Reply failed: $message"
        if (quickReplyEditorOwns(pending)) {
            quickReplyFailure = message
            messages = messages.map { if (it.id == pending.tempMessageId) it.copy(sendStatus = SendStatus.Failed) else it }
        }
        if (untrustedCertificateProtocol(message) != null) {
            errorBanner = message
            pendingCertificateRetry = PendingCertificateRetry.QuickReply(pending)
        } else if (pendingCertificateRetry == PendingCertificateRetry.QuickReply(pending)) {
            pendingCertificateRetry = null
        }
    }
}

private fun MeronMobileState.quickReplyEditorOwns(pending: PendingQuickReplySend): Boolean =
    selectedCoreThread?.backendThreadId() == pending.threadId &&
        quickReplyThreadId == pending.threadId &&
        quickReplyGeneration == pending.quickReplyGeneration

private suspend fun allocateCoreMessageId(
    client: MobileMailCommandClient,
    accountId: String,
    draft: Boolean,
): String {
    val id = parseAllocatedMessageId(client.allocateIdentity(AllocateIdentityParams(accountId, draft)))
    require(id.isNotBlank()) { "Core did not allocate a message identity" }
    return id
}

private suspend fun MeronMobileState.readAttachmentData(
    client: MobileMailCommandClient,
    accountId: String,
    key: String,
): String {
    val response =
        withManagedGoogleAuth(client, accountId) {
            client.readAttachment(AttachmentReadParams(key))
        }
    return parseAttachmentDataResponse(response)
}

internal fun MeronMobileState.openMessageCompose(
    message: MessageBody,
    forward: Boolean,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val generation = ++composeSessionGeneration
    scope.launch {
        awaitAppSignatureLoaded()
        if (generation != composeSessionGeneration) return@launch
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val accountId = selectedCoreThread?.accountId.orEmpty()
                val copiedAttachments =
                    forwardableAttachments(message).mapNotNull { attachment ->
                        readAttachmentData(client, accountId, attachment.key).takeIf { it.isNotBlank() }?.let {
                            attachmentToDraftAttachment(attachment, it)
                        }
                    }
                // Only a forward quotes the original's HTML, so only a forward
                // needs its inline images re-attached. Images whose bytes can't
                // be read are left out of the quote's cid: rewrite rather than
                // being referenced with no part behind them.
                val inlineImages =
                    if (forward) {
                        forwardInlineImages(message).mapNotNull { image ->
                            runCatching {
                                readAttachmentData(client, accountId, image.attachment.key)
                            }.getOrNull()
                                ?.takeIf { it.isNotBlank() }
                                ?.let { image to it }
                        }
                    } else {
                        emptyList()
                    }
                val draft =
                    if (forward) {
                        messageForwardDraft(
                            message = message,
                            attachments = copiedAttachments,
                            dateLabel = formatMessageFullTimestamp(message.dateEpochSeconds),
                            inlineImages = inlineImages.map { it.first },
                        )
                    } else {
                        messageEditAsNewDraft(message, copiedAttachments)
                    }
                draft to inlineImages.map { (image, data) -> inlineImageToDraftAttachment(image, data) }
            }
        }.onSuccess { (draft, inlineAttachments) ->
            if (generation != composeSessionGeneration) return@onSuccess
            // A forward and a copy are both new conversations: everything the
            // previous draft left behind goes, threading headers included, or
            // they would thread themselves under the last reply's parent.
            clearComposeDraftState()
            to = draft.to
            cc = draft.cc
            bcc = draft.bcc
            subject = draft.subject
            // A forward's body is the quote, so the signature goes above it. A
            // copied message ("edit as new") already carries the signature it
            // was written with, and must not collect a second one.
            body =
                if (forward) {
                    seedBodyWithSignature(
                        draft.body,
                        selectedCoreThread?.accountId.orEmpty().ifBlank { defaultSendAccountId() },
                        SignaturePlacement.AboveQuote,
                    )
                } else {
                    composeSignature = null
                    draft.body
                }
            attachments = draft.attachments
            composeForwardHtml = draft.html
            composeForwardInlineAttachments = inlineAttachments
            composeFromAccountId = selectedCoreThread?.accountId ?: selectedCoreAccountId.takeIf { it != UNIFIED_ACCOUNT_ID }.orEmpty()
            composeFromEmail = ""
            composeDraftId = ""
            composeDraftSaved = false
            composeDraftAccountId = ""
            composeReturnScreen = Screen.Thread
            rememberComposeSeed()
            screen = Screen.Compose
            status = if (forward) "Forward draft ready" else "Copied message into compose"
        }.onFailure {
            if (generation != composeSessionGeneration) return@onFailure
            status = if (forward) "Forward failed: ${it.message}" else "Edit as new failed: ${it.message}"
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
internal suspend fun MeronMobileState.readAttachmentBytes(attachment: MessageAttachment): ByteArray {
    val client = MobileMailCommandClient(core)
    val response =
        withManagedGoogleAuth(client, selectedCoreThread?.accountId.orEmpty()) {
            client.readAttachment(AttachmentReadParams(attachment.key))
        }
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
        status = coreUnavailableMessage
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
        status = coreUnavailableMessage
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val bytes = readAttachmentBytes(attachment)
                val image = if (attachment.mimeType.startsWith("image/")) decodeImageBitmap(bytes) else null
                bytes to image
            }
        }.onSuccess { (bytes, image) ->
            if (attachment.mimeType.startsWith("image/")) {
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

internal fun MeronMobileState.shareImageAttachment(attachment: MessageAttachment) {
    if (attachment.url.isNotBlank()) {
        services.openUrl(attachment.url)
        return
    }
    if (attachment.key.isBlank()) {
        status = "Attachment is not cached."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { readAttachmentBytes(attachment) }
        }.onSuccess { bytes ->
            services.shareFile(
                bytes,
                safeAttachmentFilename(attachment.filename),
                attachment.mimeType.ifBlank { "image/*" },
            )
        }.onFailure {
            status = "Image share failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.copyImageAttachment(attachment: MessageAttachment) {
    if (attachment.key.isBlank()) {
        status = "Attachment is not cached."
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { readAttachmentBytes(attachment) }
        }.onSuccess { bytes ->
            services.copyImage(bytes, attachment.mimeType.ifBlank { "image/*" }, attachment.filename.ifBlank { "Image" })
            status = "Image copied."
        }.onFailure {
            status = "Image copy failed: ${it.message}"
        }
    }
}

/**
 * Open a compose screen addressed to one person (the "message this participant"
 * action in a thread), signature included like any other new message.
 */
internal fun MeronMobileState.openComposeTo(
    email: String,
    accountId: String,
) {
    openSignatureCompose {
        clearComposeDraftState()
        composeFromAccountId = accountId
        composeFromEmail = ""
        to = email
        body = seedBodyWithSignature("", accountId.ifBlank { defaultSendAccountId() })
        composeReturnScreen = Screen.Thread
        rememberComposeSeed()
        screen = Screen.Compose
    }
}

/**
 * Open a compose screen from a `mailto:` link. The link's own body counts as
 * text the user asked for, so the signature goes below it.
 */
internal fun MeronMobileState.openMailtoCompose(draft: ComposeDraft) {
    openMailtoCompose(draft, onOpened = {})
}

internal fun MeronMobileState.openMailtoCompose(
    draft: ComposeDraft,
    onOpened: () -> Unit,
) {
    openSignatureCompose {
        clearComposeDraftState()
        to = draft.to
        cc = draft.cc
        bcc = draft.bcc
        subject = draft.subject
        attachments = draft.attachments
        body = seedBodyWithSignature(draft.body, defaultSendAccountId())
        composeReturnScreen = if (screen == Screen.Kanban) screen else Screen.Mail
        rememberComposeSeed()
        screen = Screen.Compose
        onOpened()
    }
}

internal fun MeronMobileState.openCompose() {
    // Everything the previous draft left behind goes, the sender and threading
    // headers included: a fresh message that kept `composeInReplyTo` would
    // thread itself into the conversation the last reply belonged to, and one
    // that kept `composeFromAccountId` would send from an account other than
    // the one whose signature it is about to be seeded with.
    openSignatureCompose {
        clearComposeDraftState()
        body = seedBodyWithSignature("", defaultSendAccountId())
        composeReturnScreen = if (screen == Screen.Kanban) screen else Screen.Mail
        rememberComposeSeed()
        screen = Screen.Compose
    }
}

private fun MeronMobileState.openSignatureCompose(open: MeronMobileState.() -> Unit) {
    val generation = ++composeSessionGeneration
    if (appSignatureLoaded) {
        if (generation == composeSessionGeneration) open()
        return
    }
    scope.launch {
        awaitAppSignatureLoaded()
        if (generation == composeSessionGeneration) open()
    }
}

internal fun MeronMobileState.closeCompose() {
    val generation = composeSessionGeneration
    val returnScreen = composeReturnScreen
    showLocalDraftInOpenThread()
    screen = returnScreen
    // Start immediately so the closing session is snapshotted before another
    // compose can open. A later session suppresses state updates but keeps the
    // remote copy written for the composer that was just closed.
    scope.launch(start = CoroutineStart.UNDISPATCHED) {
        saveComposeDraft(showStatus = false, generation = generation, keepObsoleteDraft = true)
    }
}

private fun MeronMobileState.showLocalDraftInOpenThread() {
    val thread = selectedCoreThread ?: return
    if (composeReturnScreen != Screen.Thread) return
    val draft = ComposeDraft(to.trim(), cc.trim(), bcc.trim(), subject.trim(), body.trim(), attachments)
    if (composeIsBlank()) return
    val accountId = selectedComposeIdentity()?.accountId ?: thread.accountId.ifBlank { defaultSendAccountId() }
    if (accountId.isBlank()) return
    val draftId = composeDraftId.ifBlank { newDraftMessageId(accountId) }
    composeDraftId = draftId
    val normalizedDraftId = draftId.trim().trim('<', '>').lowercase()
    val localDraft =
        MessageBody(
            id = "local-draft-$normalizedDraftId",
            folderId = "Drafts",
            from = selectedComposeIdentity()?.email.orEmpty(),
            to = draft.to,
            cc = draft.cc,
            bcc = draft.bcc,
            subject = draft.subject,
            body = draft.body,
            dateEpochSeconds = currentTimeMillis() / 1000,
            fromAddr = selectedComposeIdentity()?.email.orEmpty(),
            messageId = draftId,
            references = composeReferences,
            outgoing = true,
            hasAttachments = draft.attachments.isNotEmpty(),
        )
    messages =
        (
            messages.filterNot {
                it.id == localDraft.id || it.messageId
                    .trim()
                    .trim('<', '>')
                    .lowercase() == normalizedDraftId
            } + localDraft
        ).sortedBy { it.dateEpochSeconds }
    selectedCoreThread = thread.copy(hasDraft = true)
    markThreadDraftEverywhere(thread.id)
}

internal fun MeronMobileState.markThreadDraftEverywhere(threadId: String) {
    if (threadId.isBlank()) return
    val canonicalId =
        (listOfNotNull(selectedCoreThread).asSequence() + coreThreads.asSequence())
            .firstOrNull { it.id == threadId || it.threadId == threadId }
            ?.backendThreadId()
            ?: threadId
    locallyDraftedThreadIds = locallyDraftedThreadIds + canonicalId
    coreThreads = threadsWithDraftFlag(coreThreads, setOf(canonicalId))
    selectedCoreThread =
        selectedCoreThread?.let { thread ->
            if (thread.id == threadId || thread.threadId == threadId) thread.copy(hasDraft = true) else thread
        }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = threadsWithDraftFlag(state.threads, setOf(threadId)))
        }
    mailboxCache =
        mailboxCache.mapValues { (_, cached) ->
            cached.copy(threads = threadsWithDraftFlag(cached.threads, setOf(threadId)))
        }
}

internal fun MeronMobileState.clearThreadDraftEverywhere(threadId: String) {
    if (threadId.isBlank()) return
    val matchingIds =
        buildSet {
            add(threadId)
            (
                listOfNotNull(selectedCoreThread).asSequence() +
                    coreThreads.asSequence() +
                    kanbanColumns.values.asSequence().flatMap { it.threads.asSequence() } +
                    mailboxCache.values.asSequence().flatMap { it.threads.asSequence() }
            ).filter { it.id == threadId || it.threadId == threadId }
                .forEach {
                    add(it.id)
                    if (it.threadId.isNotBlank()) add(it.threadId)
                }
        }
    locallyDraftedThreadIds = locallyDraftedThreadIds - matchingIds
    coreThreads = threadsWithoutDraftFlag(coreThreads, threadId)
    selectedCoreThread =
        selectedCoreThread?.let { thread ->
            if (thread.id == threadId || thread.threadId == threadId) thread.copy(hasDraft = false) else thread
        }
    kanbanColumns =
        kanbanColumns.mapValues { (_, state) ->
            state.copy(threads = threadsWithoutDraftFlag(state.threads, threadId))
        }
    mailboxCache =
        mailboxCache.mapValues { (_, cached) ->
            cached.copy(threads = threadsWithoutDraftFlag(cached.threads, threadId))
        }
}

private fun threadsWithoutDraftFlag(
    threads: List<ThreadSummary>,
    threadId: String,
): List<ThreadSummary> =
    threads.map { thread ->
        if (thread.id == threadId || thread.threadId == threadId) thread.copy(hasDraft = false) else thread
    }

internal fun MeronMobileState.withLocalDraftFlags(threads: List<ThreadSummary>): List<ThreadSummary> = threadsWithDraftFlag(threads, locallyDraftedThreadIds)

// A just-discarded draft can briefly reappear in a server refetch: some IMAP
// providers (Gmail included) don't guarantee an expunge on one connection is
// visible to a concurrent read session immediately. Keep hiding a discarded
// thread until a fetch actually confirms it's gone, then stop tracking it.
internal fun MeronMobileState.withoutLocallyDiscardedThreads(threads: List<ThreadSummary>): List<ThreadSummary> {
    if (locallyDiscardedThreadIds.isEmpty()) return threads
    locallyDiscardedThreadIds =
        locallyDiscardedThreadIds.filter { id -> threads.any { it.id == id } }.toSet()
    if (locallyDiscardedThreadIds.isEmpty()) return threads
    return threads.filterNot { it.id in locallyDiscardedThreadIds }
}

internal fun threadsWithDraftFlag(
    threads: List<ThreadSummary>,
    threadId: String,
): List<ThreadSummary> = threadsWithDraftFlag(threads, setOf(threadId))

internal fun threadsWithDraftFlag(
    threads: List<ThreadSummary>,
    threadIds: Set<String>,
): List<ThreadSummary> {
    if (threadIds.isEmpty()) return threads
    var changed = false
    val updated =
        threads.map { thread ->
            if (thread.id !in threadIds && thread.threadId !in threadIds) {
                thread
            } else if (thread.hasDraft) {
                thread
            } else {
                changed = true
                thread.copy(hasDraft = true)
            }
        }
    return if (changed) updated else threads
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
            resetPasswordAccountForm()
            displayName = account.displayName
            senderName = account.senderName
            email = account.email
            username = account.email
            password = ""
            // The prefilled address is already the account's own; don't let the
            // on-blur lookup re-run and rewrite the form around it.
            lastAutodiscoverEmail = account.email
            if (account.imapHost.isNotBlank()) host = account.imapHost
            hostTouched = host.isNotBlank()
            if (account.imapPort > 0) imapPort = account.imapPort.toString()
            imapPortTouched = account.imapPort > 0
            imapSecurity =
                when {
                    account.starttls -> MailSecurity.STARTTLS
                    account.tls -> MailSecurity.TLS
                    else -> MailSecurity.NONE
                }
            imapSecurityTouched =
                account.imapPort > 0 && imapSecurity != mailSecurityForPort(account.imapPort)
            if (account.smtpHost.isNotBlank()) smtpHost = account.smtpHost
            smtpHostTouched = smtpHost.isNotBlank()
            if (account.smtpPort > 0) smtpPort = account.smtpPort.toString()
            smtpPortTouched = account.smtpPort > 0
            smtpSecurity =
                when {
                    account.smtpStarttls -> MailSecurity.STARTTLS
                    account.smtpTls -> MailSecurity.TLS
                    else -> MailSecurity.NONE
                }
            smtpSecurityTouched =
                account.smtpPort > 0 && smtpSecurity != mailSecurityForPort(account.smtpPort)
            addSection = 1
            passwordServerSettingsOpen = true
        }
    }
    errorBanner = null
    previousTopScreen = if (screen == Screen.Kanban) screen else Screen.Mail
    screen = Screen.AddAccount
}

/** Append a board holding [columns] and make it the active one. */
private fun MeronMobileState.addKanbanBoard(columns: List<KanbanColumnSpec>): KanbanBoardSpec {
    val board =
        defaultKanbanBoard(coreAccounts).copy(
            name = "Kanban board ${kanbanBoards.size + 1}",
            columns = columns,
        )
    persistKanbanBoards(kanbanBoards + board)
    activeKanbanBoardId = board.id
    saveActiveKanbanBoardId(kanbanPrefs, board.id)
    return board
}

internal fun MeronMobileState.createKanbanBoard(): String {
    val board = addKanbanBoard(defaultKanbanBoard(coreAccounts).columns)
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
    // Deleting the last board leaves no board at all; the kanban screen and the
    // drawer both render that empty state, and reseeding a default here would
    // make the delete look like it did nothing.
    val wasActive = boardId == activeKanbanBoardId
    persistKanbanBoards(kanbanBoards.filterNot { it.id == boardId })
    // persistKanbanBoards has already moved the selection off the deleted board,
    // so drop the cached columns and load whatever it landed on (if anything).
    if (wasActive) {
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

/**
 * Replace the active board's columns with [columns] (the selection from the add-column
 * dialog), preserving the relative order of existing columns and appending new ones.
 * Loads any newly added column and drops cached data for removed ones. With no board
 * left at all, the selection creates one.
 */
internal fun MeronMobileState.applyKanbanColumns(columns: List<KanbanColumnSpec>) {
    val deduped = columns.distinctBy(::kanbanColumnKey)
    // Every board can be deleted, and the empty kanban screen still offers "Add
    // column", so a selection made with no board left has to bring one with it.
    val active = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId }
    if (active == null && deduped.isEmpty()) return
    val board = active ?: addKanbanBoard(emptyList())
    val nextKeys = deduped.map(::kanbanColumnKey).toSet()
    val existingKeys = board.columns.map(::kanbanColumnKey).toSet()
    if (nextKeys == existingKeys) return
    // Keep existing columns in their current order, then append newly selected ones.
    val ordered =
        board.columns.filter { kanbanColumnKey(it) in nextKeys } +
            deduped.filter { kanbanColumnKey(it) !in existingKeys }
    persistKanbanBoards(
        kanbanBoards.map { if (it.id == board.id) it.copy(columns = ordered) else it },
    )
    (existingKeys - nextKeys).forEach { kanbanColumns = kanbanColumns - it }
    ordered
        .filter { kanbanColumnKey(it) !in existingKeys }
        .forEach { loadKanbanColumn(it, refresh = true) }
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

/**
 * Drop every column showing a folder, on all boards. Used once the folder is gone
 * from the server: a column left behind would only fail to load.
 */
internal fun MeronMobileState.removeKanbanColumnsForFolder(
    accountId: String,
    folderId: String,
) {
    val key = kanbanColumnKey(KanbanColumnSpec(accountId, folderId))
    persistKanbanBoards(
        kanbanBoards.map { board ->
            board.copy(columns = board.columns.filterNot { kanbanColumnKey(it) == key })
        },
    )
    kanbanColumns = kanbanColumns - key
}

/**
 * Point an existing column at another folder of the same account, keeping its slot
 * on the board. Does nothing when the folder is unchanged or already has its own
 * column here — the board must not end up with duplicates.
 */
internal fun MeronMobileState.switchKanbanColumnFolder(
    column: KanbanColumnSpec,
    folderId: String,
) {
    if (folderId.isBlank() || kanbanFolderIdsEqual(folderId, column.folderId)) return
    val board = kanbanBoards.firstOrNull { it.id == activeKanbanBoardId } ?: return
    val fromKey = kanbanColumnKey(column)
    val target = KanbanColumnSpec(column.accountId, folderId)
    val toKey = kanbanColumnKey(target)
    if (board.columns.none { kanbanColumnKey(it) == fromKey }) return
    if (board.columns.any { it.accountId == target.accountId && kanbanFolderIdsEqual(it.folderId, target.folderId) }) return
    persistKanbanBoards(
        kanbanBoards.map { existing ->
            if (existing.id != board.id) {
                existing
            } else {
                existing.copy(columns = existing.columns.map { if (kanbanColumnKey(it) == fromKey) target else it })
            }
        },
    )
    if (kanbanSearchScope == fromKey) persistKanbanSearchScope(toKey)
    // Keep the old folder's cached page only while another board still shows it.
    if (kanbanBoards.none { it.columns.any { existing -> kanbanColumnKey(existing) == fromKey } }) {
        kanbanColumns = kanbanColumns - fromKey
    }
    loadKanbanColumn(target, refresh = true)
}

/**
 * Fetch an account's folder list for a folder picker (a kanban column's or the
 * mail list's) when only the bootstrap inbox is cached, so the picker isn't
 * limited to what a sync happened to surface.
 */
internal fun MeronMobileState.ensureAccountFolders(accountId: String) {
    if (!coreLoaded || accountId == UNIFIED_ACCOUNT_ID) return
    val account = coreAccounts.firstOrNull { it.id == accountId } ?: return
    if (accountSummaryIsRss(account)) return
    if (foldersByAccount[accountId].orEmpty().size > 1) return
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                withManagedGoogleAuth(client, account.id) {
                    client.sync(
                        SyncMailParams(
                            accountId = account.id,
                            folderId = INBOX_FOLDER,
                            limit = 1,
                            folders = true,
                            deferTail = true,
                        ),
                    )
                }
                loadAccountFolders(client, account)
            }
        }.onSuccess { folders ->
            if (folders.isNotEmpty()) foldersByAccount = foldersByAccount + (accountId to folders)
        }
    }
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
        status = coreUnavailableMessage
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                withManagedGoogleAuth(client, account.id) {
                    client.createFolder(FolderCreateParams(accountId = account.id, name = trimmed))
                }
                loadAccountFolders(client, account)
            }
        }.onSuccess { folders ->
            foldersByAccount = foldersByAccount + (account.id to folders)
            val created = folders.folderCreatedAs(trimmed)?.name ?: trimmed
            addKanbanColumn(KanbanColumnSpec(account.id, created))
            showKanbanCreateFolderDialog = null
            kanbanFolderNameInput = ""
            status = "Folder created"
        }.onFailure {
            status = "Create folder failed: ${it.message}"
        }
    }
}
