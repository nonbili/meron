package jp.nonbili.meron.ui

import androidx.compose.ui.input.key.key
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.MessageBody
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendStatus
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.bareAddress
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.forwardInlineImages
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.forwardedHtmlQuote
import jp.nonbili.meron.shared.inlineImageToDraftAttachment
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.notificationThreadId
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.rewriteMediaRefsToCid
import jp.nonbili.meron.shared.splitAddressList
import jp.nonbili.meron.shared.threadIdIsRss
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.math.abs

internal fun MeronMobileState.openDraftCompose(
    message: MessageBody,
    thread: ThreadSummary,
    returnScreen: Screen = Screen.Mail,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val generation = ++composeSessionGeneration
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val copied =
                    forwardableAttachments(message).mapNotNull { attachment ->
                        val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                        data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                    }
                // A saved forward keeps its quoted HTML; without re-deriving it
                // here, reopening the draft would silently downgrade it to the
                // plain body on the next send. The reader rewrote the quote's
                // cid: refs to /media/ paths on the way in, so the inline images
                // have to be resolved again too.
                val quote = forwardedHtmlQuote(message.bodyHtml)
                val inlineImages =
                    if (quote.isBlank()) {
                        emptyList()
                    } else {
                        forwardInlineImages(message).mapNotNull { image ->
                            val data =
                                runCatching {
                                    parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(image.attachment.key)))
                                }.getOrNull()
                            data?.takeIf { it.isNotBlank() }?.let { image to it }
                        }
                    }
                val availableImages = inlineImages.map { it.first }
                val rewrittenQuote =
                    forwardInlineImages(message).fold(rewriteMediaRefsToCid(quote, availableImages)) { html, image ->
                        html.replace("/media/${image.attachment.key.trim()}", "")
                    }
                Triple(
                    copied,
                    rewrittenQuote,
                    inlineImages.map { (image, data) -> inlineImageToDraftAttachment(image, data) },
                )
            }
        }.onSuccess { (copiedAttachments, forwardHtml, inlineAttachments) ->
            if (generation != composeSessionGeneration) return@onSuccess
            // Start from a clean composer: a draft opened after a reply would
            // otherwise inherit that reply's threading headers and be sent into
            // the wrong conversation. The draft's own headers are restored just
            // below — a reply draft has to keep threading where it belongs.
            clearComposeDraftState()
            to = message.to
            cc = message.cc
            bcc = message.bcc
            subject = message.subject
            body = message.body
            // A draft written earlier already carries whatever signature it was
            // written with, so the body stays unmanaged: a later change of From
            // must not rewrite part of it, nor append a second signature.
            composeSignature = null
            attachments = copiedAttachments
            composeForwardHtml = forwardHtml
            composeForwardInlineAttachments = inlineAttachments
            composeFromAccountId = thread.accountId
            composeFromEmail = ""
            // A draft with no Message-ID of its own — imported, or written by
            // something that omitted the header — cannot be addressed on the
            // server: a discard would search for a header that isn't there. The
            // composer takes an id of its own and treats it as unsaved, so its
            // first save creates that draft properly instead of claiming one
            // that was never written.
            val openedDraftId = message.messageId.trim().trim('<', '>')
            composeDraftId = openedDraftId.ifBlank { newDraftMessageId(thread.accountId) }
            composeDraftSaved = openedDraftId.isNotBlank()
            composeDraftAccountId = thread.accountId
            composeInReplyTo = message.inReplyTo
            composeReferences = message.references
            composeReturnScreen = returnScreen
            rememberComposeSeed()
            screen = Screen.Compose
            status = "Draft ready"
        }.onFailure {
            if (generation != composeSessionGeneration) return@onFailure
            status = "Draft open failed: ${it.message}"
        }
    }
}

internal fun draftThreadShouldOpenConversation(messages: List<MessageBody>): Boolean =
    messages.any { it.folderId.isNotBlank() && !folderIsDrafts(it.folderId) } ||
        messages.any { folderIsDrafts(it.folderId) && (it.references.isNotBlank() || it.inReplyTo.isNotBlank()) }

internal fun MeronMobileState.readCoreThread(
    thread: ThreadSummary,
    sourceFolder: String = thread.folder,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val backendThreadId = thread.backendThreadId()
    val readToken = activeThreadReadToken + 1
    activeThreadReadToken = readToken
    val returnScreen = if (screen == Screen.Kanban) screen else Screen.Mail
    val readsDraftThread =
        !threadIdIsRss(backendThreadId) &&
            (thread.folderRole == "drafts" || (thread.folderRole == "folder" && (folderIsDrafts(sourceFolder) || folderIsDrafts(thread.folder))))
    val selectedThread = if (sourceFolder.isNotBlank() && sourceFolder != thread.folder) thread.copy(folder = sourceFolder) else thread
    selectedCoreThread = selectedThread
    messages = emptyList()
    messageCursor = ""
    loadingMoreMessages = false
    previousTopScreen = returnScreen
    if (quickReplyThreadId != backendThreadId) {
        ++quickReplyGeneration
        quickReplyAutosaveJob?.cancel()
        quickReplyAttachments = emptyList()
        quickReplyFailure = ""
        quickReplyDraftId = ""
        quickReplyDraftSaved = false
        quickReplyInReplyTo = ""
        quickReplyReferences = ""
        quickReplyFrom = ""
        quickReplyThreadId = backendThreadId
        // Starts the new thread's bar on the replying account's signature rather
        // than blank — the bar is what gets sent, so it shows what will go out.
        // Set after quickReplyThreadId, which the late-signature re-seed keys on.
        seedQuickReplySignature()
    }
    if (!readsDraftThread) {
        screen = Screen.Thread
    }
    // Nothing is marked read on open: messages are marked incrementally as the
    // user scrolls past them (see the scroll-driven marking in ThreadUi),
    // mirroring desktop.
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (threadIdIsRss(backendThreadId)) {
                    client.readRssThread(RssThreadParams(threadId = backendThreadId))
                } else {
                    withManagedGoogleAuth(client, selectedThread.accountId) {
                        client.readThread(ThreadReadParams(threadId = backendThreadId))
                    }
                }
            }
        }.onSuccess {
            // Drop any superseded read, including an older request for this
            // same conversation after the user backed out and reopened it.
            if (activeThreadReadToken != readToken || selectedCoreThread?.backendThreadId() != backendThreadId) {
                return@onSuccess
            }
            val page = parseThreadReadPage(it)
            messages = mergeLocalSendMessages(messages, page.messages)
            messageCursor = page.nextCursor
            hydrateQuickReplyFromTailDraft(backendThreadId, messages)
            if (readsDraftThread) {
                val draftMessage =
                    page.messages.lastOrNull { message -> folderIsDrafts(message.folderId) }
                        ?: page.messages.lastOrNull()
                if (draftThreadShouldOpenConversation(page.messages)) {
                    screen = Screen.Thread
                } else {
                    draftMessage?.let { message ->
                        openDraftCompose(message, selectedThread, returnScreen = returnScreen)
                    }
                }
            }
        }.onFailure {
            if (activeThreadReadToken != readToken) return@onFailure
            status = "Could not open message: ${it.message}"
        }
    }
}

// Pre-fills the quick-reply bar from an already-saved draft reply sitting at
// the tail of the conversation, so the user can continue and send it inline
// instead of being forced into the full editor. No-op when the tail message
// isn't a draft, or is already the one loaded (e.g. re-entrant calls from
// loadMoreThreadMessages).
internal fun MeronMobileState.hydrateQuickReplyFromTailDraft(
    threadBackendId: String,
    mergedMessages: List<MessageBody>,
) {
    if (quickReplyThreadId != threadBackendId) return
    val tail = mergedMessages.lastOrNull() ?: return
    if (!folderIsDrafts(tail.folderId)) return
    val normalizedTailId = tail.messageId.normalizedComposeDraftId()
    // A draft a send already consumed, whose discard has not come back yet: it
    // holds the text we just sent, not a reply left unfinished.
    if (normalizedTailId.isNotBlank() && normalizedTailId in quickReplyConsumedDraftIds) return
    // A send still out for this conversation owns whatever draft sits at the
    // tail — the copy it is about to discard, or the one its autosave was
    // mid-write on when the click cancelled it. The bar empties on the click
    // now, so a full bar no longer stands guard over that window.
    if (quickReplySendInFlight && quickReplySendThreadId == threadBackendId) return
    if (quickReplyDraftId.isNotBlank() && quickReplyDraftId.normalizedComposeDraftId() == normalizedTailId) return
    // Only a draft we can address on the server. A row synced from its envelope
    // carries no Message-ID yet — and no body either — so taking it would put an
    // empty bar in front of the user calling itself their saved draft, under an
    // id that names nothing: the next save would append a second copy and leave
    // this one stranded. Reading the thread back-fills the header.
    val tailDraftId = tail.messageId.trim().trim('<', '>')
    if (tailDraftId.isBlank()) return
    // Only into a bar that is free. Hydration fills an empty reply from a saved
    // draft; it is not entitled to replace a reply the user began before this
    // read came back, nor to drop the id that reply is already saved under.
    if (!quickReplyIsBlank() || quickReplyDraftId.isNotBlank()) return
    quickReplyBody = tail.body
    ++quickReplyGeneration
    quickReplyDraftId = tailDraftId
    quickReplyDraftSaved = true
    quickReplyInReplyTo = tail.inReplyTo
    quickReplyReferences = tail.references
    quickReplyFrom = tail.fromAddr
    quickReplyFailure = ""
    // The saved body already carries whatever signature it was written with, so
    // none of it is this app's to strip, re-seed, or discount as "not content".
    quickReplySignature = null
    if (!tail.hasAttachments) {
        quickReplyAttachments = emptyList()
        return
    }
    scope.launch {
        val copied =
            runCatching {
                withContext(ioDispatcher) {
                    val client = MobileMailCommandClient(core)
                    forwardableAttachments(tail).mapNotNull { attachment ->
                        val data = parseAttachmentDataResponse(client.readAttachment(AttachmentReadParams(attachment.key)))
                        data.takeIf { it.isNotBlank() }?.let { attachmentToDraftAttachment(attachment, it) }
                    }
                }
            }.getOrElse { emptyList() }
        if (quickReplyThreadId == threadBackendId && quickReplyDraftId.normalizedComposeDraftId() == normalizedTailId) {
            quickReplyAttachments = copied
            ++quickReplyGeneration
        }
    }
}

internal fun ThreadSummary.backendThreadId(): String = threadId.ifBlank { id }

/** Item keys are present only when a starred RSS row represents one article. */
internal fun ThreadSummary.rssItemKeys(): List<String> = listOf(id).takeIf { threadIdIsRss(backendThreadId()) && threadId.isNotBlank() && threadId != id }.orEmpty()

// Re-read the currently open thread and replace its message list with the
// canonical copy from the core. Used after sending a quick reply so the stored
// sent message replaces the optimistic one. Runs on ioDispatcher; guards against
// the user having switched threads while the read was in flight.
internal suspend fun MeronMobileState.reloadCurrentThreadMessages() {
    val thread = selectedCoreThread ?: return
    if (!coreLoaded) return
    val response =
        withContext(ioDispatcher) {
            val client = MobileMailCommandClient(core)
            if (threadIdIsRss(thread.id)) {
                client.readRssThread(RssThreadParams(threadId = thread.id))
            } else {
                withManagedGoogleAuth(client, thread.accountId) {
                    client.readThread(ThreadReadParams(threadId = thread.id))
                }
            }
        }
    if (selectedCoreThread?.id != thread.id) return
    val page = parseThreadReadPage(response)
    messages = mergeLocalSendMessages(messages, page.messages)
    messageCursor = page.nextCursor
}

// Retry loading bodies for the open thread. Re-reading is enough: the core
// re-attempts the on-demand IMAP fetch for any message without a cached body.
internal fun MeronMobileState.retryOpenThreadLoad() {
    scope.launch {
        runCatching { reloadCurrentThreadMessages() }
            .onFailure { status = "Could not open message: ${it.message}" }
    }
}

internal fun mergeLocalSendMessages(
    current: List<MessageBody>,
    refreshed: List<MessageBody>,
): List<MessageBody> {
    val refreshedIds = refreshed.map { it.id }.toSet()
    val refreshedMessageIds =
        refreshed
            .mapNotNull { it.messageId.normalizedMessageId().takeIf(String::isNotBlank) }
            .toSet()
    val unresolved =
        current.filter { message ->
            val localSend = message.id.startsWith("local-send-")
            val localDraft = message.id.startsWith("local-draft-")
            if (!localSend && !localDraft && message.sendStatus == SendStatus.None) return@filter false
            if (message.id in refreshedIds) return@filter false
            val messageId = message.messageId.normalizedMessageId()
            messageId.isBlank() || messageId !in refreshedMessageIds
        }
    // Fallback candidates: outgoing, non-draft rows this read newly revealed. A
    // message we were already showing before the send cannot be its server
    // copy, and a draft — even one holding this very reply — is not a sent copy.
    val knownIds = current.map { it.id }.toSet()
    val candidates = refreshed.filter { it.outgoing && it.id !in knownIds && !folderIsDrafts(it.folderId) }
    val paired = pairLocalSendsWithServerCopies(unresolved.filter { it.id.startsWith("local-send-") }, candidates)
    val local = unresolved.filter { it.id !in paired }
    if (local.isEmpty()) return refreshed
    return (refreshed + local).sortedBy { it.dateEpochSeconds }
}

// How far the server's Date header may sit from the moment we rendered the
// bubble and still be the same message — enough for a slow submission plus
// modest clock skew, short enough not to swallow a genuinely later reply.
private const val SENT_COPY_MATCH_WINDOW_SECONDS = 600L

// Match optimistic bubbles to the server's copies of them when the Message-ID
// we generated did not come back. Proton Bridge replaces that id with one of
// its own (`@protonmail.internalid`), so identity has to come from the
// envelope: same sender, same subject, same recipients, and a send time close
// to when we rendered the bubble.
//
// Two replies into one thread share every one of those fields, so pairing is
// decided globally rather than by first match: every plausible pair is ranked
// by whether the content matches and then by how far apart the two times are,
// and pairs are taken best-first. That keeps a copy arriving out of order from
// claiming the wrong bubble — which would hide one reply and show the other
// twice — while still settling on time alone when a server reflows the body it
// stored and no content match exists.
//
// Returns the ids of the bubbles that found a copy.
private fun pairLocalSendsWithServerCopies(
    locals: List<MessageBody>,
    candidates: List<MessageBody>,
): Set<String> {
    val ranked =
        locals
            .flatMap { local ->
                candidates
                    .filter { candidate -> isPlausibleSentCopy(local, candidate) }
                    .map { candidate ->
                        Triple(
                            local.id to candidate.id,
                            if (local.contentSignature() == candidate.contentSignature()) 0 else 1,
                            abs(candidate.dateEpochSeconds - local.dateEpochSeconds),
                        )
                    }
            }.sortedWith(compareBy({ it.second }, { it.third }))

    val pairedLocals = mutableSetOf<String>()
    val claimed = mutableSetOf<String>()
    for ((pair, _, _) in ranked) {
        val (localId, candidateId) = pair
        if (localId in pairedLocals || candidateId in claimed) continue
        pairedLocals += localId
        claimed += candidateId
    }
    return pairedLocals
}

// The envelope test every pair must clear before ranking.
private fun isPlausibleSentCopy(
    local: MessageBody,
    candidate: MessageBody,
): Boolean {
    if (bareAddress(candidate.fromAddr).lowercase() != bareAddress(local.fromAddr).lowercase()) return false
    if (candidate.subject.trim() != local.subject.trim()) return false
    if (candidate.recipientKey() != local.recipientKey()) return false
    return abs(candidate.dateEpochSeconds - local.dateEpochSeconds) <= SENT_COPY_MATCH_WINDOW_SECONDS
}

// What distinguishes two replies that share an envelope: what they say and what
// they carry. Whitespace-insensitive, since a server may rewrap the body it
// stored — a mismatch demotes a pair rather than rejecting it.
private fun MessageBody.contentSignature(): String {
    val normalizedBody =
        body
            .split(whitespaceRun)
            .filter { it.isNotBlank() }
            .joinToString(" ")
            .lowercase()
    val files = attachments.map { it.filename.trim().lowercase() }.sorted().joinToString("|")
    return "$files\u0000$normalizedBody"
}

private val whitespaceRun = Regex("\\s+")

// Order-independent set of the bare To/Cc addresses, for envelope comparison.
private fun MessageBody.recipientKey(): String =
    (splitAddressList(to) + splitAddressList(cc))
        .map { bareAddress(it).lowercase() }
        .filter { it.isNotBlank() }
        .sorted()
        .joinToString(",")

private fun String.normalizedMessageId(): String = trim().trim('<', '>').lowercase()

// Re-read the open thread on a push/sync event so live IDLE updates (new mail,
// or our own sent copy) appear in the conversation, not just the thread list.
// Mirrors desktop's refreshOpenThread: skip when the event is for a different
// account than the open thread (it may differ from the selected mailbox account
// in unified / kanban / starred views).
internal suspend fun MeronMobileState.refreshOpenThreadFor(eventAccount: String) {
    val open = selectedCoreThread ?: return
    if (eventAccount.isNotBlank() && open.accountId.isNotBlank() && open.accountId != eventAccount) {
        return
    }
    runCatching { reloadCurrentThreadMessages() }
}

/** Opens the conversation a tapped notification names, layered over whatever
 *  the user was looking at: the mailbox behind it keeps its account, folder,
 *  search and filter, so backing out returns to the Mail or Kanban view the tap
 *  interrupted rather than to the notification's own folder. The thread is
 *  opened directly by composite id without searching the mailbox. */
internal fun MeronMobileState.openNotificationThread(target: NotificationThreadTarget) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    if (target.threadKey.isBlank()) {
        openNotificationMailbox(target)
        return
    }
    openLinkedThread(notificationThreadId(target.accountId, target.folder, target.threadKey), target.accountId)
}

/** A group summary names an account and folder but no conversation, so this one
 *  does navigate the mailbox. Hiding an account from the side nav only drops it
 *  from the drawer list; its mail is still reachable, including from here. */
private fun MeronMobileState.openNotificationMailbox(target: NotificationThreadTarget) {
    mailSearch = ""
    mailFilter = FilterMode.All
    mailAttachmentsOnly = false
    selectedCoreAccountId = target.accountId
    selectedCoreFolder = target.folder
    syncing = true
    scope.launchThreadListRead(threadRemovalGuard) { read ->
        val folderReadVersion = folderReadGuard.version
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                val accounts =
                    coreAccounts.takeIf { accounts -> accounts.any { it.id == target.accountId } }
                        ?: parseAccountListResponse(client.listAccounts())
                val account =
                    accounts.firstOrNull { it.id == target.accountId }
                        ?: error("Account not found: ${target.accountId}")
                val result =
                    loadAccountInbox(
                        client = client,
                        account = account,
                        requestedFolder = target.folder,
                        query = "",
                        filter = FilterMode.All,
                        syncFirst = false,
                    )
                accounts to result
            }
        }.onSuccess { (accounts, result) ->
            if (coreAccounts.isEmpty()) {
                coreAccounts = accounts
            }
            coreFolders = reconcileFolderUnread(result.folders, folderReadVersion)
            if (result.folders.isNotEmpty()) {
                foldersByAccount = foldersByAccount + reconcileFolderUnread(result.folders, folderReadVersion).groupBy { it.accountId }
            }
            selectedCoreFolder = result.folder
            coreThreads = withLocalDraftFlags(read.filter(result.threads))
            visibleMailboxKey = mailboxCacheKey(target.accountId, result.folder, "", FilterMode.All)
            mailboxCursor = result.nextCursor
            mailboxAccountCursors = result.accountCursors
            mailboxPageDepth = MAILBOX_PAGE_SIZE
            syncing = false
            initialThreadsLoaded = true
            selectedMailThreadIds = emptySet()
            // Loading the mailbox is not enough on its own: nothing else on this
            // path moves the app off the screen it was on, so the tap would land
            // behind Settings, Kanban, Compose, or a thread left open.
            selectedCoreThread = null
            previousTopScreen = Screen.Mail
            screen = Screen.Mail
        }.onFailure {
            syncing = false
            status = "Could not open notification: ${it.message}"
        }
    }
}

internal fun MeronMobileState.loadMoreThreadMessages() {
    val thread = selectedCoreThread ?: return
    if (!coreLoaded || messageCursor.isBlank() || loadingMoreMessages) return
    val cursor = messageCursor
    loadingMoreMessages = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                if (threadIdIsRss(thread.id)) {
                    client.readRssThread(RssThreadParams(threadId = thread.id, beforeCursor = cursor))
                } else {
                    withManagedGoogleAuth(client, thread.accountId) {
                        client.readThread(ThreadReadParams(threadId = thread.id, beforeCursor = cursor))
                    }
                }
            }
        }.onSuccess {
            val page = parseThreadReadPage(it)
            val existingIds = messages.map { message -> message.id }.toSet()
            val older = page.messages.filterNot { message -> message.id in existingIds }
            messages = (older + messages).sortedBy { message -> message.dateEpochSeconds }
            messageCursor = page.nextCursor
            loadingMoreMessages = false
            if (older.isEmpty()) status = "No older messages in this thread."
        }.onFailure {
            loadingMoreMessages = false
            status = "Could not load older messages: ${it.message}"
        }
    }
}

// Fetch a separate snapshot: printing must not change scroll position or the
// open conversation, and must include messages outside the currently loaded page.
internal suspend fun MeronMobileState.loadThreadForPrinting(): List<MessageBody> {
    val thread = checkNotNull(selectedCoreThread)
    check(coreLoaded)
    val threadId = thread.backendThreadId()
    return withContext(ioDispatcher) {
        val client = MobileMailCommandClient(core)
        loadPrintThread { cursor ->
            parseThreadReadPage(
                withManagedGoogleAuth(client, thread.accountId) {
                    client.readThread(ThreadReadParams(threadId = threadId, forPrint = true, beforeCursor = cursor))
                },
            )
        }
    }
}
