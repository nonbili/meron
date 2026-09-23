package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.outlined.Drafts
import androidx.compose.ui.input.key.key
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AllocateIdentityParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.ContactSuggestParams
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.SignatureMark
import jp.nonbili.meron.shared.SignaturePlacement
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.bodyWithSignature
import jp.nonbili.meron.shared.bodyWithSwappedSignature
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.forwardHtmlForSend
import jp.nonbili.meron.shared.forwardInlineImages
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.inlineImageToDraftAttachment
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.noSignatureMark
import jp.nonbili.meron.shared.parseAllocatedMessageId
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.replyAllAddsRecipients
import jp.nonbili.meron.shared.resolveSignatureHtml
import jp.nonbili.meron.shared.signaturePlainText
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import jp.nonbili.meron.shared.untrustedCertificateProtocol
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

// Only touched from the main thread (all send paths run in the UI scope).
internal var localSendSequence = 0L

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
internal fun MeronMobileState.signatureTextFor(accountId: String): String {
    val account = coreAccounts.firstOrNull { it.id == accountId }
    return signaturePlainText(resolveSignatureHtml(account, appSignatureHtml))
}

/**
 * Seed a fresh draft body with the sending account's signature, remembering what
 * was inserted so a later change of identity can swap it out.
 */
internal fun MeronMobileState.seedBodyWithSignature(
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
    // The message went out, so a draft that survived its discard is stale. Left
    // to the next save's cleanup it lingered for as long as no other draft was
    // saved — and, in a thread, could hydrate the reply bar with the sent text.
    composeDraftCleanupOwners = composeDraftCleanupOwners - discardOutcome.failedOwners.toSet()
    discardOutcome.failedOwners.forEach { retrySentDraftDiscard(it) }
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

/** Whether the open conversation's reply target has other recipients, so
 * reply-all would reach someone the reply bar does not. */
internal fun MeronMobileState.canReplyAllToThread(): Boolean {
    val thread = selectedCoreThread ?: return false
    if (threadIdIsRss(thread.id)) return false
    val parent = quickReplyParent() ?: return false
    return replyAllAddsRecipients(parent)
}

/** The same question for one message, for its own menu. */
internal fun MeronMobileState.canReplyAllToMessage(message: MessageBody): Boolean = replyAllAddsRecipients(message)

/** Reply-all to one message, rather than to the conversation's reply target:
 * the message menu acts on the message it belongs to. Opens the full composer —
 * the recipients are the point of the action, and only the composer shows them.
 * The reply bar and any draft it holds are left untouched. */
internal fun MeronMobileState.replyAllToMessage(message: MessageBody) {
    val thread = selectedCoreThread
    val accountId = thread?.accountId?.ifBlank { defaultSendAccountId() }.orEmpty()
    if (accountId.isBlank() || thread == null) {
        status = "Open a mail thread before replying."
        return
    }
    if (threadIdIsRss(thread.id)) {
        status = "RSS items do not support replies."
        return
    }
    val account = coreAccounts.firstOrNull { it.id == accountId }
    // The bar's From override belongs to the bar's own reply; this one sends
    // from whichever identity this message was addressed to.
    val replyFrom = account?.let { detectReplyFromIdentity(message, it) }.orEmpty()
    val params =
        message.toReplyMailParams(
            accountId = accountId,
            body = "",
            from = replyFrom,
            replyAll = true,
        )
    val generation = ++composeSessionGeneration
    val open: MeronMobileState.() -> Unit = open@{
        if (generation != composeSessionGeneration) return@open
        // A reply of its own, not a continuation of whatever the composer last
        // held: threading headers and leftover forward state go first.
        clearComposeDraftState()
        to = params.to
        cc = params.cc
        bcc = params.bcc
        subject = params.subject
        body = seedBodyWithSignature(params.body, accountId)
        composeFromAccountId = accountId
        composeFromEmail = replyFrom
        composeInReplyTo = params.inReplyTo
        composeReferences = params.references
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

internal suspend fun allocateCoreMessageId(
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
