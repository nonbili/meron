package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.automirrored.filled.Send
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.ComposeDraft
import jp.nonbili.meron.shared.DiscardDraftParams
import jp.nonbili.meron.shared.MessageAttachment
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.ReplyRecipients
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.SignatureMark
import jp.nonbili.meron.shared.SignaturePlacement
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.bodyWithSignature
import jp.nonbili.meron.shared.bodyWithSwappedSignature
import jp.nonbili.meron.shared.buildReplyRecipients
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsInbox
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.untrustedCertificateProtocol
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

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

/** Backs the reply bar's recipient row: the To/Cc the bar would send to as it
 * stands. Both blank when the open thread cannot be replied to, which hides the
 * row. */
internal fun MeronMobileState.quickReplyRecipients(): ReplyRecipients {
    val none = ReplyRecipients("", "")
    val thread = selectedCoreThread ?: return none
    if (threadIdIsRss(thread.id)) return none
    val parent = quickReplyParent() ?: return none
    return buildReplyRecipients(parent)
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
        // Sending cancels this job, but the write it is already making lands on
        // the server regardless. Cancelling between the write and the fold below
        // is what strands the draft: runCatching swallows the CancellationException
        // as a failed save, so the id just written is never published to the bar
        // nor handed over, and the send that cancelled us finds no draft to
        // discard. Once the write is under way, see it through and record it.
        withContext(NonCancellable) { saveQuickReplyDraftLocked(showStatus) }
    }

private suspend fun MeronMobileState.saveQuickReplyDraftLocked(showStatus: Boolean): Boolean {
    // A send is about to discard the draft; saving now could resurrect it. The
    // reply the user has since started still has to reach the server though, so
    // record that a save was turned away and run it once the send settles.
    if (quickReplySendInFlight) {
        quickReplyAutosaveDeferred = true
        return false
    }
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

/**
 * Run a save that a send in flight turned away. The bar empties on the click, so
 * the user can start their next reply while the previous one is still going out
 * — and that reply's debounce lands inside the send, where saving is refused.
 * Nothing else reschedules it: until it runs, the copy in Drafts still holds the
 * text that was already sent, and the reply on screen is not saved anywhere.
 */
private fun MeronMobileState.runDeferredQuickReplyAutosave() {
    if (!quickReplyAutosaveDeferred) return
    quickReplyAutosaveDeferred = false
    if (quickReplyIsBlank()) return
    autoSaveQuickReplyDraft()
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
    // A send out for this conversation is still holding this draft as the copy
    // it falls back on if it fails. Emptying a *newer* reply out of the bar is
    // not permission to delete the safety net of the one already on its way.
    if (quickReplySendInFlight && quickReplySendThreadId == quickReplyThreadId) return
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
            val normalizedDraftId = draftId.normalizedComposeDraftId()
            val anotherDraftRemains =
                messages.any {
                    it.messageId.normalizedComposeDraftId() != normalizedDraftId && folderIsDrafts(it.folderId)
                }
            // Clearing only the open thread's copy leaves the thread in
            // locallyDraftedThreadIds, and the refresh below stamps the flag
            // straight back onto the row it just fetched — so the badge outlives
            // the draft everywhere the thread is listed.
            if (!anotherDraftRemains) clearThreadDraftEverywhere(thread.backendThreadId())
            syncCoreThreads(syncFirst = false)
            selectedCoreThread = selectedCoreThread?.copy(hasDraft = anotherDraftRemains)
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

/** Escalate the quick reply bar into the full composer, seeded as a reply and
 * carrying over whatever has been typed. `replyAll` seeds the wider recipient
 * list; the bar has no recipient fields to show it in, so reply-all always
 * lands here rather than in the bar. */
internal fun MeronMobileState.openQuickReplyInFullEditor(replyAll: Boolean = false) {
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
            attachments = quickReplyAttachments,
            replyAll = replyAll,
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
    // Which conversation the send belongs to, so a read landing mid-send knows
    // the draft at its tail is this send's and not a reply left unfinished.
    // The bar is empty from the click, so its own emptiness no longer says so.
    quickReplySendThreadId = threadId
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
            attachments = sentAttachments,
        )
    // Empty the visible bar on the click, not when the send settles. The work
    // below — an autosave still holding the lock, then an identity round trip,
    // then the send itself — runs for seconds against a slow mailbox, and
    // leaving the text sitting there reads as a tap that did nothing.
    //
    // Only what the user can see is cleared. The draft id stays on the bar: a
    // failed send keeps that draft as its safety net, and edits made after the
    // failure have to write back into the same copy rather than open a second
    // one beside it. The success path below clears the id once there is nothing
    // left to fall back to.
    val barBodyAtClick = quickReplyBody
    val barSignatureAtClick = quickReplySignature
    quickReplyAttachments = emptyList()
    seedQuickReplySignature()
    // Render the sent bubble optimistically — on the click, before the send
    // round-trip — so replying feels instant. The bubble shows a "Sending…"
    // status until the canonical stored message replaces it on re-fetch; on
    // failure it flips to "Failed" and stays visible so the reply isn't lost.
    // A counter suffix keeps ids unique even for two sends in the same
    // millisecond — message ids key the conversation list, duplicates crash.
    val tempId = "local-send-${currentTimeMillis()}-${localSendSequence++}"
    messages =
        messages +
        MessageBody(
            id = tempId,
            folderId = parent.folderId,
            from = "You",
            fromAddr = replyFrom.ifBlank { account?.email.orEmpty() },
            to = baseParams.to,
            cc = baseParams.cc,
            subject = baseParams.subject,
            body = sentBody,
            references = baseParams.references,
            dateEpochSeconds = currentTimeMillis() / 1000,
            hasAttachments = sentAttachments.isNotEmpty(),
            attachments =
                sentAttachments.map {
                    MessageAttachment(filename = it.displayName, mimeType = it.mimeType, sizeBytes = it.sizeBytes)
                },
            sendStatus = SendStatus.Sending,
        )
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
                        quickReplySendThreadId = ""
                        resolvedOwner?.let { owner ->
                            quickReplyConsumedDraftIds -= owner.draftId.normalizedComposeDraftId()
                        }
                        // Nothing was sent, so take the optimistic bubble back
                        // down rather than leave a reply that does not exist.
                        messages = messages.filterNot { message -> message.id == tempId }
                        // No send was ever dispatched, so there is no pending
                        // reply for Retry to resend. Put the reply back where the
                        // user left it instead: the box they typed it into, ready
                        // to send again. Only if they have not started another.
                        if (quickReplyGeneration == generation) {
                            quickReplyBody = barBodyAtClick
                            quickReplySignature = barSignatureAtClick
                            quickReplyAttachments = sentAttachments
                        }
                        quickReplyFailure = it.message.orEmpty()
                        status = "Send failed: ${it.message}"
                        return@launch
                    }
                val params = baseParams.copy(messageId = outboundMessageId)
                // The bubble went up on the click; only its id was still unknown.
                messages =
                    messages.map { message ->
                        if (message.id == tempId) message.copy(messageId = outboundMessageId) else message
                    }
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
    // The reply is resent from the params it was captured with, so it only has
    // to belong to the conversation on screen. Requiring the bar to be untouched
    // made a failed send unretryable the moment the user typed anything.
    if (selectedCoreThread?.backendThreadId() != pending.threadId) return
    retryQuickReplySend(pending)
}

internal fun MeronMobileState.retryQuickReplySend(pending: PendingQuickReplySend) {
    if (quickReplySendInFlight) return
    pendingQuickReplySend = pending
    quickReplySendInFlight = true
    // A retry is a send: it owns this conversation's draft for as long as it is
    // out, exactly as the first attempt did. Without this the guards that read
    // the pair see a blank thread and stand down for the whole retry.
    quickReplySendThreadId = pending.threadId
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
                // An empty box is not "the user's next reply". Typing one and
                // clearing it again moves the generation on while leaving the id
                // claimed, and treating that as kept is what left the draft in
                // Drafts still holding the reply that already went out.
                !quickReplyIsBlank() &&
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
            if (consumedDraftDiscarded) {
                removeDiscardedDraftFromOpenThread(owner.draftId, owner.threadId)
                // The bar can still be claiming the copy just deleted — an empty
                // box that was typed in and cleared keeps its id. Leaving the
                // claim would point the next save at a draft that is gone.
                if (quickReplyDraftId.normalizedComposeDraftId() == owner.draftId.normalizedComposeDraftId()) {
                    quickReplyDraftId = ""
                    quickReplyDraftSaved = false
                }
            }
        }
        if (pendingQuickReplySend == pending) pendingQuickReplySend = null
        if (pendingCertificateRetry == PendingCertificateRetry.QuickReply(pending)) pendingCertificateRetry = null
        quickReplySendInFlight = false
        quickReplySendThreadId = ""
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
        refreshKanbanColumnsHoldingThread(pending.threadId)
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
        // Only now stop hiding it. Released any earlier, the refreshes above can
        // paint the server's pre-discard draft row beside the reply it was sent
        // as, leaving both cards on screen until the next read drops one.
        val undiscardedDraft = pending.draftOwner?.takeIf { !barKeptTheDraft && !consumedDraftDiscarded }
        if (undiscardedDraft != null) {
            // The reply went out, so the copy left behind is stale rather than
            // a safety net: released now, it could hydrate the sent text back
            // into the bar. It stays hidden until a retry removes it.
            retrySentDraftDiscard(undiscardedDraft)
        } else if (consumedDraftId.isNotBlank()) {
            quickReplyConsumedDraftIds -= consumedDraftId
        }
        runDeferredQuickReplyAutosave()
    }.onFailure {
        // A failed send leaves the draft as the safety net it was written to be.
        if (consumedDraftId.isNotBlank()) quickReplyConsumedDraftIds -= consumedDraftId
        quickReplySendInFlight = false
        quickReplySendThreadId = ""
        val message = it.message ?: "Send failed"
        status = "Reply failed: $message"
        // The bubble belongs to this send, not to whoever holds the bar now.
        // Gating it on the editor left it reading "Sending…" for good once the
        // user started their next reply — the one state it must never show is a
        // send that is no longer happening.
        messages = messages.map { if (it.id == pending.tempMessageId) it.copy(sendStatus = SendStatus.Failed) else it }
        // Retry resends the params this send captured, so the conversation being
        // open is enough to offer it; the bar having moved on is not a reason to
        // hide the only way back to a reply that never went out.
        if (selectedCoreThread?.backendThreadId() == pending.threadId) {
            quickReplyFailure = message
        }
        if (untrustedCertificateProtocol(message) != null) {
            errorBanner = message
            pendingCertificateRetry = PendingCertificateRetry.QuickReply(pending)
        } else if (pendingCertificateRetry == PendingCertificateRetry.QuickReply(pending)) {
            pendingCertificateRetry = null
        }
        runDeferredQuickReplyAutosave()
    }
}

/**
 * Keep trying to discard the draft of a message that already went out, after
 * the discard that followed the send failed. Until one succeeds the draft stays
 * among [MeronMobileState.quickReplyConsumedDraftIds]: hidden from the
 * conversation and never hydrated into the reply bar, where it would put text
 * that was already sent one tap from going out twice.
 */
internal fun MeronMobileState.retrySentDraftDiscard(owner: ComposeDraftOwner) {
    val id = owner.draftId.normalizedComposeDraftId()
    if (id.isBlank()) return
    quickReplyConsumedDraftIds += id
    scope.launch {
        for (delayMs in sentDraftDiscardRetryDelaysMs) {
            delay(delayMs)
            // Reopened in the full composer meanwhile: it is the user's again.
            if (composeDraftId.normalizedComposeDraftId() == id) {
                quickReplyConsumedDraftIds -= id
                return@launch
            }
            val discarded =
                runCatching {
                    withContext(ioDispatcher) {
                        MobileMailCommandClient(core).discardDraft(
                            DiscardDraftParams(accountId = owner.accountId, draftId = owner.draftId),
                        )
                    }
                }.isSuccess
            if (discarded) {
                quickReplyConsumedDraftIds -= id
                removeDiscardedDraftFromOpenThread(owner.draftId, owner.threadId)
                return@launch
            }
        }
        // Still hidden, so it cannot come back into the reply bar this session;
        // the Drafts folder is where the user can remove it.
        status = "Could not discard draft"
    }
}

private fun MeronMobileState.quickReplyEditorOwns(pending: PendingQuickReplySend): Boolean =
    selectedCoreThread?.backendThreadId() == pending.threadId &&
        quickReplyThreadId == pending.threadId &&
        quickReplyGeneration == pending.quickReplyGeneration
