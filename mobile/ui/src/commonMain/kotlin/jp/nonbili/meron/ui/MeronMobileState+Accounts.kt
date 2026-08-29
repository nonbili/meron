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
import jp.nonbili.meron.shared.AccountCertPinParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountMediaFileParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountProxyParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSignatureParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AddRssFeedParams
import jp.nonbili.meron.shared.AppPrefsGetParams
import jp.nonbili.meron.shared.AppPrefsSetParams
import jp.nonbili.meron.shared.AttachmentReadParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.CertificateProtocol
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
import jp.nonbili.meron.shared.ProbeCertParams
import jp.nonbili.meron.shared.ProxyParams
import jp.nonbili.meron.shared.ProxySpec
import jp.nonbili.meron.shared.RemoveRssFeedParams
import jp.nonbili.meron.shared.RssMarkReadParams
import jp.nonbili.meron.shared.RssMarkStarredParams
import jp.nonbili.meron.shared.RssThreadParams
import jp.nonbili.meron.shared.SendIdentity
import jp.nonbili.meron.shared.ServerCertificate
import jp.nonbili.meron.shared.SharedMobileContract
import jp.nonbili.meron.shared.SignatureSpec
import jp.nonbili.meron.shared.StarredItemSummary
import jp.nonbili.meron.shared.StarredItemsParams
import jp.nonbili.meron.shared.StorageUsage
import jp.nonbili.meron.shared.SyncMailParams
import jp.nonbili.meron.shared.SyncRssParams
import jp.nonbili.meron.shared.ThreadActionParams
import jp.nonbili.meron.shared.ThreadListParams
import jp.nonbili.meron.shared.ThreadReadParams
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.UpdateOAuthTokenParams
import jp.nonbili.meron.shared.accountSendIdentities
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.attachmentToDraftAttachment
import jp.nonbili.meron.shared.buildOAuthAuthorizationUrl
import jp.nonbili.meron.shared.coreErrorMessage
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.detectReplyFromIdentity
import jp.nonbili.meron.shared.encodeAppPrefValue
import jp.nonbili.meron.shared.folderIsDrafts
import jp.nonbili.meron.shared.folderIsTrash
import jp.nonbili.meron.shared.formatContactSuggestion
import jp.nonbili.meron.shared.formatSendIdentity
import jp.nonbili.meron.shared.forwardableAttachments
import jp.nonbili.meron.shared.isOAuthCallbackUrl
import jp.nonbili.meron.shared.isOAuthLoginFailure
import jp.nonbili.meron.shared.isPotentialOAuthCallbackUrl
import jp.nonbili.meron.shared.messageEditAsNewDraft
import jp.nonbili.meron.shared.messageForwardDraft
import jp.nonbili.meron.shared.newDraftMessageId
import jp.nonbili.meron.shared.ownAddressList
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAppPrefsResponse
import jp.nonbili.meron.shared.parseAttachmentDataResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseContactSuggestResponse
import jp.nonbili.meron.shared.parseFolderListResponse
import jp.nonbili.meron.shared.parseMailtoUrl
import jp.nonbili.meron.shared.parseMediaFileUrlResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseOpmlImportCountResponse
import jp.nonbili.meron.shared.parseProbeCertResponse
import jp.nonbili.meron.shared.parseProxyResponse
import jp.nonbili.meron.shared.parseStarredItemsPage
import jp.nonbili.meron.shared.parseStarredItemsResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.parseThreadListPage
import jp.nonbili.meron.shared.parseThreadListResponse
import jp.nonbili.meron.shared.parseThreadReadPage
import jp.nonbili.meron.shared.recipientTail
import jp.nonbili.meron.shared.replaceRecipientTail
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.sanitizeRemoteSenders
import jp.nonbili.meron.shared.threadIdIsRss
import jp.nonbili.meron.shared.toReplyMailParams
import jp.nonbili.meron.shared.toSaveDraftParams
import jp.nonbili.meron.shared.toSendMailParams
import jp.nonbili.meron.shared.untrustedCertificateProtocol
import jp.nonbili.meron.shared.withRemoteSender
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlin.math.abs
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

internal fun MeronMobileState.applyAccounts(
    json: String,
    preferEmail: String? = null,
) {
    accountJson = json
    val parsed = parseAccountListResponse(json)
    coreAccounts = parsed
    // Account data is now in state. Mark accounts as loaded so the blocking
    // inbox loader clears even on paths that bypass listAccounts() — e.g. the
    // OAuth exchange after the custom-tab round-trip recreates the state with
    // initialAccountsLoaded=false.
    initialAccountsLoaded = true
    accountsLoading = false
    val previousAccountId = selectedCoreAccountId
    selectedCoreAccountId = preferEmail?.let { wanted -> parsed.firstOrNull { it.email == wanted }?.id }
        ?: selectedCoreAccountId.takeIf { sel -> sel == UNIFIED_ACCOUNT_ID || parsed.any { it.id == sel } }
        ?: UNIFIED_ACCOUNT_ID
    if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID && previousAccountId != UNIFIED_ACCOUNT_ID) {
        selectedCoreFolder = INBOX_FOLDER
    }
    saveLastMailLocation(prefs, selectedCoreAccountId, selectedCoreFolder)
    kanbanBoards = ensureKanbanDefaults(kanbanPrefs, kanbanBoards, parsed)
    if (activeKanbanBoardId.isBlank() || kanbanBoards.none { it.id == activeKanbanBoardId }) {
        activeKanbanBoardId = kanbanBoards.firstOrNull()?.id.orEmpty()
        saveActiveKanbanBoardId(kanbanPrefs, activeKanbanBoardId)
    }
}

private fun findOAuthResultAccount(
    accounts: List<AccountSummary>,
    previousAccountIds: Set<String>,
    provider: String,
    preferredEmail: String,
): AccountSummary? {
    val normalizedProvider = provider.trim().lowercase()
    val preferred = preferredEmail.trim()
    val providerMatches =
        accounts.filter {
            it.provider.equals(normalizedProvider, ignoreCase = true) ||
                it.authType.equals("${normalizedProvider}_oauth", ignoreCase = true)
        }
    return providerMatches.firstOrNull { it.id !in previousAccountIds }
        ?: preferred.takeIf { it.isNotBlank() }?.let { email ->
            accounts.firstOrNull { it.email.equals(email, ignoreCase = true) }
        }
        ?: providerMatches.firstOrNull()
}

internal fun MeronMobileState.listAccounts(): Job? {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        initialAccountsLoaded = true
        return null
    }
    val generation = ++accountLoadGeneration
    accountsLoading = true
    return scope.launch {
        runCatching {
            withContext(ioDispatcher) { MobileMailCommandClient(core).listAccounts() }
        }.onSuccess {
            if (generation != accountLoadGeneration) return@onSuccess
            applyAccounts(it)
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
        }.onFailure {
            if (generation != accountLoadGeneration) return@onFailure
            status = "Account list failed: ${it.message}"
        }
        if (generation == accountLoadGeneration) {
            accountsLoading = false
            initialAccountsLoaded = true
        }
    }
}

/** Read the app-wide proxy from the core store into [MeronMobileState.appProxy]. */
internal fun MeronMobileState.loadAppProxy(): Job? {
    if (!coreLoaded) return null
    val generation = ++proxyLoadGeneration
    return scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).getProxy()
            }
        }.onSuccess {
            if (generation == proxyLoadGeneration) appProxy = parseProxyResponse(it)
        }
    }
}

/**
 * Persist the app-wide proxy. Live IMAP sessions keep their sockets; the change
 * lands as they reconnect, which is why the status line says so rather than
 * implying an instant switch.
 */
internal fun MeronMobileState.saveAppProxy(spec: ProxySpec) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val previous = appProxy
    appProxy = spec
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).setProxy(ProxyParams(spec))
            }
        }.onSuccess {
            status = if (spec.mode == "off") "Proxy disabled" else "Proxy saved"
        }.onFailure {
            appProxy = previous
            status = "Proxy update failed: ${it.message}"
        }
    }
}

// How many times a failed app-signature read is retried, and how long between.
private const val APP_SIGNATURE_LOAD_ATTEMPTS = 3
private const val APP_SIGNATURE_RETRY_DELAY_MS = 250L

/**
 * Read the app-wide signature from the core store. It shares the desktop
 * `signature` row rather than a `mobile.*` one, so the two platforms agree after
 * a backup restore.
 */
internal fun MeronMobileState.loadAppSignature(): Job? {
    if (!coreLoaded) {
        // Nothing to read and nothing coming: callers waiting on this (the
        // `mailto:` handler) must not hang, the same as initialAccountsLoaded.
        appSignatureLoaded = true
        appSignatureLoadCompletion.complete(Unit)
        return null
    }
    // A reload (after a backup restore, say) makes the value on hand stale, so
    // compose waits again — and only the newest read may answer, or a slow
    // startup response could land on top of the restored signature.
    val generation = ++appSignatureLoadGeneration
    appSignatureLoaded = false
    appSignatureLoadCompletion.complete(Unit)
    appSignatureLoadCompletion = CompletableDeferred()
    return scope.launch {
        repeat(APP_SIGNATURE_LOAD_ATTEMPTS) { attempt ->
            val response =
                try {
                    withContext(ioDispatcher) {
                        // The core reports failure in the response body rather
                        // than by throwing, so validate before accepting it.
                        requireCoreOk(
                            MobileMailCommandClient(core).getPrefs(AppPrefsGetParams(listOf(APP_SIGNATURE_SETTING_KEY))),
                        )
                    }
                } catch (cancelled: CancellationException) {
                    throw cancelled
                } catch (_: Throwable) {
                    null
                }
            if (generation != appSignatureLoadGeneration) return@launch
            if (response != null) {
                appSignatureHtml = parseAppPrefsResponse(response)[APP_SIGNATURE_SETTING_KEY] as? String ?: ""
                appSignatureLoaded = true
                appSignatureLoadCompletion.complete(Unit)
                return@launch
            }
            // A read can fail transiently on a cold start. Give up eventually so
            // a store that never opens cannot strand a `mailto:` link forever.
            if (attempt + 1 < APP_SIGNATURE_LOAD_ATTEMPTS) {
                delay(APP_SIGNATURE_RETRY_DELAY_MS * (attempt + 1))
            }
        }
        if (generation == appSignatureLoadGeneration) {
            appSignatureLoaded = true
            appSignatureLoadCompletion.complete(Unit)
        }
    }
}

/**
 * Read the app-wide remote-content sender allowlist from the core store. Like
 * the signature it shares the desktop row rather than a `mobile.*` one — and
 * unlike the signature nothing waits on it: a thread read before it lands shows
 * the account's own policy, and the reveal affordance with it.
 */
internal fun MeronMobileState.loadRemoteImageSenders(): Job? {
    if (!coreLoaded) return null
    val generation = ++remoteImageSendersLoadGeneration
    return scope.launch {
        runCatching { withContext(ioDispatcher) { readRemoteImageSenders() } }
            .onSuccess { stored ->
                // A write that landed while this read was in flight bumped the
                // generation: it already read the row itself, so this snapshot
                // is the older one and must not take the edit back off screen.
                if (generation == remoteImageSendersLoadGeneration) remoteImageSenders = stored
            }
    }
}

/** The allowlist as the core store holds it, normalized. Runs on [ioDispatcher]. */
private suspend fun MeronMobileState.readRemoteImageSenders(): List<String> {
    val response =
        requireCoreOk(
            MobileMailCommandClient(core).getPrefs(AppPrefsGetParams(listOf(REMOTE_IMAGE_SENDERS_SETTING_KEY))),
        )
    val stored = parseAppPrefsResponse(response)[REMOTE_IMAGE_SENDERS_SETTING_KEY]
    return sanitizeRemoteSenders((stored as? List<*>).orEmpty().filterIsInstance<String>())
}

/**
 * Allow (or stop allowing) remote content from one sender. Unlike an account's
 * "load remote images" toggle this is additive and app-wide: mail from [addr]
 * loads its remote content in every account.
 *
 * The core resolves the allowlist as it bakes a body, so messages already on
 * screen are re-gated by the reader rather than by a re-read of the thread.
 */
internal fun MeronMobileState.setRemoteImageSender(
    addr: String,
    allowed: Boolean,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val optimistic = withRemoteSender(remoteImageSenders, addr, allowed)
    if (optimistic == remoteImageSenders) return
    // Show the change at once; the write below decides what it really becomes.
    remoteImageSenders = optimistic
    scope.launch {
        // The row holds the whole list, so the edit is applied to what the store
        // actually has rather than to the snapshot this state happened to be
        // showing: a startup read that has not landed yet would otherwise write
        // an allowlist with every stored sender missing from it, and two edits
        // in quick succession would each drop the other's. The lock keeps the
        // read and the write it feeds one step, so those cases serialize.
        remoteImageSendersWrites.withLock {
            runCatching {
                withContext(ioDispatcher) {
                    val updated = withRemoteSender(readRemoteImageSenders(), addr, allowed)
                    requireCoreOk(
                        MobileMailCommandClient(core).setPref(
                            AppPrefsSetParams(REMOTE_IMAGE_SENDERS_SETTING_KEY, encodeAppPrefValue(updated)),
                        ),
                    )
                    updated
                }
            }.onSuccess { updated ->
                // Retire any read still in flight: this write knows the row.
                ++remoteImageSendersLoadGeneration
                remoteImageSenders = updated
            }.onFailure {
                // Undo this edit alone, against the list as it stands now — a
                // blanket restore of the pre-edit snapshot would take back the
                // edits that succeeded in between.
                remoteImageSenders = withRemoteSender(remoteImageSenders, addr, !allowed)
                status = "Remote content update failed: ${it.message}"
            }
        }
    }
}

internal fun MeronMobileState.invalidateBackupReloads() {
    ++accountLoadGeneration
    ++proxyLoadGeneration
    ++appSignatureLoadGeneration
    ++remoteImageSendersLoadGeneration
    accountsLoading = false
    appSignatureLoaded = false
    appSignatureLoadCompletion.complete(Unit)
    appSignatureLoadCompletion = CompletableDeferred()
}

internal suspend fun MeronMobileState.awaitAppSignatureLoaded() {
    while (!appSignatureLoaded) {
        appSignatureLoadCompletion.await()
    }
}

/** Persist the app-wide signature. Drafts already open keep what they carry. */
internal fun MeronMobileState.saveAppSignature(html: String) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val previous = appSignatureHtml
    appSignatureHtml = html
    reseedUntouchedQuickReply()
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).setPref(AppPrefsSetParams(APP_SIGNATURE_SETTING_KEY, encodeAppPrefValue(html)))
            }
        }.onFailure {
            appSignatureHtml = previous
            reseedUntouchedQuickReply()
            status = "Signature update failed: ${it.message}"
        }
    }
}

/** Point one account at the app-wide signature, at none, or at its own. */
internal fun MeronMobileState.saveAccountSignature(
    account: AccountSummary,
    spec: SignatureSpec?,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).setAccountSignature(AccountSignatureParams(account.id, spec))
            }
        }.onSuccess {
            // Joined, not fired and forgotten: the reseed below resolves the
            // signature off coreAccounts, which this refresh is what updates.
            listAccounts()?.join()
            reseedUntouchedQuickReply()
        }.onFailure {
            status = "Signature update failed: ${it.message}"
        }
    }
}

/** Point one account at its own proxy, at the app-wide one, or at none. */
internal fun MeronMobileState.saveAccountProxy(
    account: AccountSummary,
    spec: ProxySpec,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).setAccountProxy(AccountProxyParams(account.id, spec))
            }
        }.onSuccess {
            listAccounts()
            status = "Proxy saved"
        }.onFailure {
            status = "Proxy update failed: ${it.message}"
        }
    }
}

/**
 * Save an existing account's servers. The password is deliberately absent from
 * the request unless the user typed a new one: the core reads an omitted
 * `password` as "keep the stored credential", so changing a port never costs
 * the account its login.
 *
 * A certificate the server cannot prove is offered for pinning here rather than
 * dead-ending, exactly as the sync and send paths do — but against the *edited*
 * endpoint, since the save failed and the stored account still names the old one.
 */
internal fun MeronMobileState.saveAccountServerSettings(
    account: AccountSummary,
    draft: ServerSettingsDraft,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val params =
        AddPasswordAccountParams(
            email = account.email,
            displayName = account.displayName,
            senderName = account.senderName,
            imapHost = draft.imapHost,
            imapPort = draft.imapPort,
            smtpHost = draft.smtpHost,
            smtpPort = draft.smtpPort,
            username = draft.username.ifBlank { account.email },
            password = draft.password,
            tls = draft.imapSecurity == MailSecurity.TLS,
            starttls = draft.imapSecurity == MailSecurity.STARTTLS,
            smtpTls = draft.smtpSecurity == MailSecurity.TLS,
            smtpStarttls = draft.smtpSecurity == MailSecurity.STARTTLS,
        )
    status = "Saving server settings..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.addPasswordAccount(params)
                client.listAccounts()
            }
        }.onSuccess {
            applyAccounts(it, preferEmail = account.email)
            errorBanner = null
            syncError = null
            status = "Server settings saved"
            syncCoreThreads(accountOverride = account.id, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure { failure ->
            val message = failure.message ?: "Server settings save failed"
            if (untrustedCertificateProtocol(message) != null) {
                showTypedServerCertificate(
                    accountId = account.id,
                    imapHost = draft.imapHost,
                    imapPort = draft.imapPort,
                    imapSecurity = draft.imapSecurity,
                    smtpHost = draft.smtpHost,
                    smtpPort = draft.smtpPort,
                    smtpSecurity = draft.smtpSecurity,
                    proxy = account.proxy,
                    retry = PendingCertificateRetry.ServerSettings(account.id, draft),
                    message = message,
                )
            } else {
                errorBanner = message
                status = "Server settings save failed: $message"
            }
        }
    }
}

/**
 * The certificate prompt for a save whose servers are not stored yet — a new
 * account, or an edit that failed before it could be written. Same flow as
 * [showServerCertificate], but probing the endpoint the user just typed rather
 * than the account's recorded one, which is stale or absent in both cases.
 *
 * [message] is the failure carrying the core's marker; it names which of the
 * two servers refused. It is restored as the banner if the probe itself fails,
 * so a dead end still explains itself.
 */
private fun MeronMobileState.showTypedServerCertificate(
    accountId: String,
    imapHost: String,
    imapPort: Int,
    imapSecurity: MailSecurity,
    smtpHost: String,
    smtpPort: Int,
    smtpSecurity: MailSecurity,
    proxy: ProxySpec,
    retry: PendingCertificateRetry,
    message: String,
) {
    val protocol = untrustedCertificateProtocol(message) ?: return
    val smtp = protocol == CertificateProtocol.SMTP
    val host = if (smtp) smtpHost else imapHost
    if (host.isBlank()) return
    val port = if (smtp) smtpPort else imapPort
    val starttls = (if (smtp) smtpSecurity else imapSecurity) == MailSecurity.STARTTLS
    certPromptBusy = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                parseProbeCertResponse(
                    requireCoreOk(
                        MobileMailCommandClient(core).probeCert(
                            ProbeCertParams(
                                host = host,
                                port = port,
                                protocol = protocol.wire,
                                starttls = starttls,
                                proxy = proxy,
                            ),
                        ),
                    ),
                )
            }
        }.onSuccess { certificate: ServerCertificate? ->
            if (certificate == null) {
                errorBanner = message
                status = "Could not read the server's certificate"
            } else {
                certPrompt =
                    MobileCertPrompt(
                        accountId = accountId,
                        host = host,
                        port = port,
                        protocol = protocol,
                        certificate = certificate,
                        retry = retry,
                    )
            }
        }.onFailure {
            errorBanner = message
            status = "Could not read the server's certificate: ${it.message}"
        }
        certPromptBusy = false
    }
}

/**
 * Fetch the certificate the failing server presented and put it in front of the
 * user. A server whose certificate cannot be validated — a local Proton Mail
 * Bridge serves a self-signed CA certificate as its leaf — is unreachable until
 * that exact certificate is pinned, so the alternative to this prompt is an
 * account that can never sync.
 *
 * [message] is the failure that carries the core's marker; it names which of
 * the two servers refused.
 */
internal fun MeronMobileState.showServerCertificate(
    accountId: String,
    message: String,
    retry: PendingCertificateRetry? = null,
) {
    val protocol = untrustedCertificateProtocol(message) ?: return
    val resolvedAccountId = certificateErrorAccountId(accountId, retry) ?: return
    val account = coreAccounts.find { it.id == resolvedAccountId } ?: return
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val host = if (protocol == CertificateProtocol.SMTP) account.smtpHost else account.imapHost
    if (host.isBlank()) return
    val port =
        when {
            protocol == CertificateProtocol.SMTP -> account.smtpPort.takeIf { it > 0 } ?: 465
            else -> account.imapPort.takeIf { it > 0 } ?: 993
        }
    val starttls = if (protocol == CertificateProtocol.SMTP) account.smtpStarttls else account.starttls
    certPromptBusy = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                parseProbeCertResponse(
                    requireCoreOk(
                        MobileMailCommandClient(core).probeCert(
                            ProbeCertParams(
                                host = host,
                                port = port,
                                protocol = protocol.wire,
                                starttls = starttls,
                                proxy = account.proxy,
                            ),
                        ),
                    ),
                )
            }
        }.onSuccess { certificate: ServerCertificate? ->
            if (certificate == null) {
                status = "Could not read the server's certificate"
            } else {
                certPrompt =
                    MobileCertPrompt(
                        accountId = resolvedAccountId,
                        host = host,
                        port = port,
                        protocol = protocol,
                        certificate = certificate,
                        retry = retry,
                    )
            }
        }.onFailure {
            status = "Could not read the server's certificate: ${it.message}"
        }
        certPromptBusy = false
    }
}

/**
 * Pin the certificate the user accepted and retry the sync. Only the server the
 * prompt was about is pinned: the other one keeps whatever it had.
 */
internal fun MeronMobileState.trustPromptedCertificate() {
    val prompt = certPrompt ?: return
    val fingerprint = prompt.certificate.fingerprint
    // Pinning normally writes to the account's row and lets the retry read it
    // back. An account that does not exist yet has no row, so its pin travels
    // on the request that creates it instead.
    val addRetry = prompt.retry as? PendingCertificateRetry.AddAccount
    if (addRetry != null) {
        certPrompt = null
        errorBanner = null
        if (pendingCertificateRetry == prompt.retry) pendingCertificateRetry = null
        status = "Trusted ${prompt.host}"
        val smtp = prompt.protocol == CertificateProtocol.SMTP
        addPasswordAccount(
            addRetry.params.copy(
                certPin = if (smtp) addRetry.params.certPin else fingerprint,
                smtpCertPin = if (smtp) fingerprint else addRetry.params.smtpCertPin,
            ),
        )
        return
    }
    certPromptBusy = true
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                requireCoreOk(
                    MobileMailCommandClient(core).setAccountCertPin(
                        AccountCertPinParams(
                            accountId = prompt.accountId,
                            certPin = fingerprint.takeIf { prompt.protocol == CertificateProtocol.IMAP },
                            smtpCertPin = fingerprint.takeIf { prompt.protocol == CertificateProtocol.SMTP },
                        ),
                    ),
                )
            }
        }.onSuccess {
            certPrompt = null
            syncError = null
            errorBanner = null
            status = "Trusted ${prompt.host}"
            // Resume what the certificate blocked — an unsent message stays
            // unsent unless its send is the thing that runs again.
            if (pendingCertificateRetry == prompt.retry) pendingCertificateRetry = null
            when (val retry = prompt.retry) {
                is PendingCertificateRetry.Compose -> {
                    retryComposeSend(retry.pending)
                }

                is PendingCertificateRetry.QuickReply -> {
                    retryQuickReplySend(retry.pending)
                }

                is PendingCertificateRetry.ServerSettings -> {
                    val account = coreAccounts.find { it.id == retry.accountId }
                    if (account == null) syncCoreThreads() else saveAccountServerSettings(account, retry.draft)
                }

                // Handled by the early return above: an account that does not
                // exist yet never reaches the pin-then-retry path.
                is PendingCertificateRetry.AddAccount -> {
                    Unit
                }

                null -> {
                    syncCoreThreads()
                }
            }
        }.onFailure {
            status = "Could not save the certificate: ${it.message}"
        }
        certPromptBusy = false
    }
}

internal fun MeronMobileState.dismissCertificatePrompt() {
    certPrompt = null
}

internal fun MeronMobileState.loadStorageUsage(showStatus: Boolean = false) {
    if (!coreLoaded) return
    scope.launch {
        storageBusy = true
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).storageUsage()
            }
        }.onSuccess {
            storageUsage = parseStorageUsageResponse(it)
            if (showStatus) status = "Loaded storage usage"
        }.onFailure {
            if (showStatus) status = "Storage usage failed: ${it.message}"
        }
        storageBusy = false
    }
}

internal fun MeronMobileState.clearStorageCache() {
    if (!storageClearConfirming) {
        storageClearConfirming = true
        status = "Tap clear cache again to confirm."
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    storageClearConfirming = false
    scope.launch {
        storageBusy = true
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).clearStorageCache()
            }
        }.onSuccess {
            storageUsage = parseStorageUsageResponse(it)
            status = "Cleared cached attachments"
        }.onFailure {
            status = "Clear cache failed: ${it.message}"
        }
        storageBusy = false
    }
}

internal fun MeronMobileState.addPasswordAccount() {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    addPasswordAccount(
        AddPasswordAccountParams(
            email = email.trim(),
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            imapHost = host.trim(),
            imapPort = imapPort.trim().toIntOrNull() ?: 993,
            smtpHost = smtpHost.trim(),
            smtpPort = smtpPort.trim().toIntOrNull() ?: 465,
            username = username.trim().ifBlank { email.trim() },
            password = password,
            tls = imapSecurity == MailSecurity.TLS,
            starttls = imapSecurity == MailSecurity.STARTTLS,
            smtpTls = smtpSecurity == MailSecurity.TLS,
            smtpStarttls = smtpSecurity == MailSecurity.STARTTLS,
        ),
    )
}

/**
 * Create the account described by [params].
 *
 * A server whose certificate cannot be validated — a local Proton Mail Bridge
 * serves a self-signed CA certificate as its leaf — is offered for pinning
 * instead of dead-ending on the handshake error, which is what setup used to do:
 * the banner named a TLS failure the user had no way to act on, so such an
 * account simply could not be added on this device. Accepting re-enters here
 * with the pin attached, since there is no stored account to write it to yet.
 */
private fun MeronMobileState.addPasswordAccount(params: AddPasswordAccountParams) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    status = "Adding password account..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.addPasswordAccount(params)
                client.listAccounts()
            }
        }.onSuccess {
            applyAccounts(it, preferEmail = params.email)
            resetPasswordAccountForm()
            screen = Screen.Mail
            errorBanner = null
            certPrompt = null
            status = "Added ${params.email}"
            syncCoreThreads(accountOverride = selectedCoreAccountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure { failure ->
            val message = failure.message ?: "Add account failed"
            if (untrustedCertificateProtocol(message) != null &&
                shouldPromptForNewCertificate(params, message)
            ) {
                showTypedServerCertificate(
                    accountId = mailAccountId(params.email),
                    imapHost = params.imapHost,
                    imapPort = params.imapPort,
                    imapSecurity = mailSecurityOf(params.tls, params.starttls ?: false),
                    smtpHost = params.smtpHost,
                    smtpPort = params.smtpPort,
                    smtpSecurity = mailSecurityOf(params.smtpTls ?: true, params.smtpStarttls ?: false),
                    proxy = ProxySpec.followApp,
                    retry = PendingCertificateRetry.AddAccount(mailAccountId(params.email), params),
                    message = message,
                )
            } else {
                errorBanner = message
                status = "Add account failed: $message"
            }
        }
    }
}

/**
 * The id the core mints for a mail address, so a prompt raised before the
 * account exists still names the account it will become. Mirrors `accountID`
 * in the bridge: the normalized address, used verbatim.
 */
internal fun mailAccountId(email: String): String = email.trim().lowercase()

/**
 * Whether a certificate failure is worth prompting about, or is the *same*
 * refusal we already pinned for. Retrying a pin that did not help would
 * otherwise loop the prompt forever.
 */
private fun shouldPromptForNewCertificate(
    params: AddPasswordAccountParams,
    message: String,
): Boolean =
    when (untrustedCertificateProtocol(message)) {
        CertificateProtocol.SMTP -> params.smtpCertPin == null
        else -> params.certPin == null
    }

// Clear the password setup form when its account is added and whenever a fresh
// setup starts, so an account never inherits the previous server, ports and
// security modes -- a sticky "touched" flag would otherwise keep autodiscovery
// and port edits from correcting a hand-picked mode.
internal fun MeronMobileState.resetPasswordAccountForm() {
    email = ""
    username = ""
    password = ""
    displayName = ""
    senderName = ""
    host = ""
    hostTouched = false
    imapPort = "993"
    imapPortTouched = false
    imapSecurity = MailSecurity.TLS
    imapSecurityTouched = false
    smtpHost = ""
    smtpHostTouched = false
    smtpPort = "465"
    smtpPortTouched = false
    smtpSecurity = MailSecurity.TLS
    smtpSecurityTouched = false
    lastAutodiscoverEmail = ""
    passwordAutodiscoverGeneration += 1
    passwordServerSettingsOpen = false
}

internal fun MeronMobileState.autodiscoverPasswordAccount(auto: Boolean = false) {
    val emailValue = email.trim()
    if (!emailValue.contains('@') || emailValue.endsWith('@')) {
        // Don't nag while the user is still typing the address.
        if (!auto) status = "Enter an email address first."
        return
    }
    // The on-blur trigger fires whenever focus leaves the email field; skip the
    // lookup unless the address actually changed since the last attempt.
    if (auto && emailValue.equals(lastAutodiscoverEmail, ignoreCase = true)) return
    lastAutodiscoverEmail = emailValue
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    passwordAutodiscoverGeneration += 1
    val requestGeneration = passwordAutodiscoverGeneration
    status = "Finding mail settings..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                parseAutodiscoverResponse(client.autodiscoverAccount(AutodiscoverAccountParams(emailValue)))
            }
        }.onSuccess { discovered ->
            if (
                requestGeneration != passwordAutodiscoverGeneration ||
                !email.trim().equals(emailValue, ignoreCase = true)
            ) {
                return@onSuccess
            }
            val discoveredImapPort = discovered.imapPort.takeIf { discovered.imapHost.isNotBlank() } ?: 0
            val discoveredSmtpPort = discovered.smtpPort.takeIf { discovered.smtpHost.isNotBlank() } ?: 0
            val imapSelection =
                mailServerSelectionAfterDiscovery(
                    imapPort,
                    imapSecurity,
                    imapSecurityTouched,
                    hostTouched,
                    imapPortTouched,
                    discoveredImapPort,
                    preserveUserSettings = auto,
                )
            val smtpSelection =
                mailServerSelectionAfterDiscovery(
                    smtpPort,
                    smtpSecurity,
                    smtpSecurityTouched,
                    smtpHostTouched,
                    smtpPortTouched,
                    discoveredSmtpPort,
                    preserveUserSettings = auto,
                )
            if ((!auto || !hostTouched) && discovered.imapHost.isNotBlank()) {
                host = discovered.imapHost
                if (!auto) hostTouched = false
            }
            imapPort = imapSelection.port
            imapSecurity = imapSelection.security
            if (!auto && discoveredServerIsComplete(discovered.imapHost, discovered.imapPort)) {
                imapPortTouched = false
                imapSecurityTouched = false
            }
            if ((!auto || !smtpHostTouched) && discovered.smtpHost.isNotBlank()) {
                smtpHost = discovered.smtpHost
                if (!auto) smtpHostTouched = false
            }
            smtpPort = smtpSelection.port
            smtpSecurity = smtpSelection.security
            if (!auto && discoveredServerIsComplete(discovered.smtpHost, discovered.smtpPort)) {
                smtpPortTouched = false
                smtpSecurityTouched = false
            }
            if (discovered.username.isNotBlank()) username = discovered.username
            status =
                when {
                    discovered.appPasswordProvider.isNotBlank() -> {
                        "${discovered.providerName.ifBlank {
                            discovered.appPasswordProvider
                        }} settings found. Use an app password."
                    }

                    discovered.source == "guess" -> {
                        passwordServerSettingsOpen = true
                        "Settings guessed. Verify the servers before adding."
                    }

                    else -> {
                        "Settings found${discovered.providerName.takeIf { it.isNotBlank() }?.let { " for $it" }.orEmpty()}."
                    }
                }
        }.onFailure {
            if (
                requestGeneration != passwordAutodiscoverGeneration ||
                !email.trim().equals(emailValue, ignoreCase = true)
            ) {
                return@onFailure
            }
            passwordServerSettingsOpen = true
            status = "Settings lookup failed: ${it.message}"
        }
    }
}

internal fun mailSecurityForPort(port: Int): MailSecurity =
    when (port) {
        25, 143, 587 -> MailSecurity.STARTTLS
        3143, 3587 -> MailSecurity.NONE
        else -> MailSecurity.TLS
    }

internal fun mailSecurityAfterPortEdit(
    current: MailSecurity,
    touched: Boolean,
    portText: String,
): MailSecurity {
    if (touched) return current
    val port = portText.toIntOrNull()?.takeIf { it > 0 } ?: return current
    return mailSecurityForPort(port)
}

internal data class MailServerSelection(
    val port: String,
    val security: MailSecurity,
)

internal fun discoveredServerIsComplete(
    host: String,
    port: Int,
): Boolean = host.isNotBlank() && port > 0

internal fun mailServerSelectionAfterDiscovery(
    currentPort: String,
    currentSecurity: MailSecurity,
    securityTouched: Boolean,
    hostTouched: Boolean,
    portTouched: Boolean,
    discoveredPort: Int,
    preserveUserSettings: Boolean = true,
): MailServerSelection =
    if ((preserveUserSettings && (hostTouched || portTouched)) || discoveredPort <= 0) {
        MailServerSelection(currentPort, currentSecurity)
    } else {
        MailServerSelection(
            discoveredPort.toString(),
            if (preserveUserSettings && securityTouched) currentSecurity else mailSecurityForPort(discoveredPort),
        )
    }

internal fun MeronMobileState.addRssAccount() {
    if (rssAccountAdding) return
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    rssAccountAdding = true
    status = "Adding RSS account..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.addRssAccount(AddRssAccountParams(feedUrl = rssFeedUrl.trim(), displayName = rssDisplayName.trim()))
                client.listAccounts()
            }
        }.onSuccess { json ->
            rssAccountAdding = false
            val parsedNew = parseAccountListResponse(json)
            val oldIds = coreAccounts.map { it.id }.toSet()
            val newRssAccount = parsedNew.firstOrNull { it.id !in oldIds && accountSummaryIsRss(it) }
            if (newRssAccount != null) {
                // Switch away from Unified before publishing the refreshed account
                // list, so effects observing coreAccounts load the new RSS mailbox.
                selectCoreMailbox(newRssAccount.id, INBOX_FOLDER)
            }
            applyAccounts(json)
            rssDisplayName = ""
            rssFeedUrl = ""
            screen = Screen.Mail
            status = "RSS account added"
            // account.addRss already fetched and stored the starter feed's items,
            // so re-fetching here would be a redundant (and slow) network round-trip.
            syncCoreThreads(
                accountOverride = selectedCoreAccountId,
                folderOverride = INBOX_FOLDER,
                syncFirst = false,
                successStatus = "RSS account added",
            )
        }.onFailure {
            rssAccountAdding = false
            status = "Add RSS failed: ${it.message}"
        }
    }
}

internal fun nextRssAccountDisplayName(accounts: List<AccountSummary>): String {
    val names =
        accounts
            .filter(::accountSummaryIsRss)
            .map {
                it.displayName
                    .ifBlank { it.email }
                    .trim()
                    .lowercase()
            }.toSet()
    var suffix = 0
    while (true) {
        val candidate = if (suffix == 0) "RSS" else "RSS$suffix"
        if (candidate.lowercase() !in names) return candidate
        suffix += 1
    }
}

internal fun MeronMobileState.exportOpmlForSelectedAccount() {
    val accountId = selectedCoreAccountId
    if (accountId == UNIFIED_ACCOUNT_ID || accountId.isBlank()) {
        status = "Select an RSS account first."
        return
    }
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                MobileMailCommandClient(core).exportOpml(ExportOpmlParams(accountId = accountId))
            }
        }.onSuccess {
            val opml = parseOpmlExportResponse(it)
            if (opml.isBlank()) {
                status = "No OPML content to export."
            } else {
                pendingOpmlExport = opml
                launchOpmlExport("meron-feeds.opml")
            }
        }.onFailure {
            status = "OPML export failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.saveAccountSettings(
    account: AccountSummary,
    displayName: String,
    senderName: String,
    avatarUrl: String,
    wallpaperPresetId: String,
    loadRemoteImages: Boolean,
    conversationHtml: Boolean,
    includedInUnified: Boolean,
    muted: Boolean,
    paused: Boolean,
    rssSyncIntervalMinutes: Int,
    aliasesText: String,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val aliases =
        aliasesText
            .lineSequence()
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .map { line ->
                val parts = line.split(",", limit = 2).map { it.trim() }
                AccountAliasParams(email = parts[0], name = parts.getOrElse(1) { "" })
            }.filter { it.email.isNotBlank() }
            .toList()
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.setAccountName(AccountNameParams(account.id, displayName.trim()))
                client.setAccountAvatar(AccountAvatarParams(account.id, avatarUrl.trim()))
                client.setAccountChatWallpaper(AccountChatWallpaperParams(account.id, presetId = wallpaperPresetId.trim()))
                if (!accountSummaryIsRss(account)) {
                    client.setAccountSenderName(AccountNameParams(account.id, senderName.trim()))
                    client.setAccountAliases(AccountAliasesParams(account.id, aliases))
                }
                client.setAccountImages(AccountFlagParams(account.id, loadRemoteImages))
                client.setAccountConversationHtml(AccountFlagParams(account.id, conversationHtml))
                client.setAccountUnified(AccountFlagParams(account.id, includedInUnified))
                client.setAccountMuted(AccountFlagParams(account.id, muted))
                client.setAccountPaused(AccountFlagParams(account.id, paused))
                if (accountSummaryIsRss(account)) {
                    client.setAccountRssSyncInterval(AccountRssSyncIntervalParams(account.id, rssSyncIntervalMinutes.coerceIn(5, 1440)))
                }
                client.listAccounts()
            }
        }.onSuccess {
            applyAccounts(it)
            accountSettingsTargetId = null
            status = "Saved account settings"
        }.onFailure {
            status = "Account settings failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.setAccountNavigationVisible(
    account: AccountSummary,
    visible: Boolean,
) {
    hiddenNavigationAccountIds =
        if (visible) {
            hiddenNavigationAccountIds - account.id
        } else {
            hiddenNavigationAccountIds + account.id
        }
    saveAppStringSet(prefs, HIDDEN_NAV_ACCOUNTS_PREF, hiddenNavigationAccountIds)
    if (!visible && selectedCoreAccountId == account.id) {
        selectedCoreAccountId = UNIFIED_ACCOUNT_ID
        selectedCoreFolder = INBOX_FOLDER
        selectedCoreThread = null
        messages = emptyList()
        coreThreads = emptyList()
        mailboxCursor = ""
        mailboxAccountCursors = emptyMap()
    }
}

internal fun MeronMobileState.removeAccount(account: AccountSummary) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.removeAccount(AccountIdParams(account.id))
                client.listAccounts()
            }
        }.onSuccess {
            hiddenNavigationAccountIds = hiddenNavigationAccountIds - account.id
            saveAppStringSet(prefs, HIDDEN_NAV_ACCOUNTS_PREF, hiddenNavigationAccountIds)
            selectedCoreAccountId = UNIFIED_ACCOUNT_ID
            selectedCoreFolder = INBOX_FOLDER
            selectedCoreThread = null
            messages = emptyList()
            coreThreads = emptyList()
            applyAccounts(it)
            status = "Removed account"
        }.onFailure {
            status = "Remove account failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.moveAccount(
    account: AccountSummary,
    delta: Int,
) {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val oldIndex = coreAccounts.indexOfFirst { it.id == account.id }
    val newIndex = (oldIndex + delta).coerceIn(0, coreAccounts.lastIndex)
    if (oldIndex < 0 || oldIndex == newIndex) return
    val next = coreAccounts.toMutableList()
    val moved = next.removeAt(oldIndex)
    next.add(newIndex, moved)
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.reorderAccounts(AccountReorderParams(next.map { it.id }))
                client.listAccounts()
            }
        }.onSuccess {
            applyAccounts(it, preferEmail = account.email.ifBlank { account.id })
            status = "Moved account"
        }.onFailure {
            status = "Move account failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.addOAuthAccount() {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val refreshToken = oauthRefreshToken.trim()
    if (refreshToken.isBlank()) {
        status = "OAuth refresh token is required."
        return
    }
    val params =
        AddOAuthAccountParams(
            email = oauthEmail.trim(),
            provider = oauthProvider,
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            accessToken = oauthAccessToken.trim(),
            refreshToken = refreshToken,
            tokenExpiresAt = oauthExpiresAt.trim().toLongOrNull() ?: 0,
        )
    status = "Adding ${oauthProvider.replaceFirstChar { it.uppercase() }} account..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.addOAuthAccount(params)
                client.listAccounts()
            }
        }.onSuccess {
            applyAccounts(it, preferEmail = params.email)
            screen = Screen.Mail
            errorBanner = null
            status = "Added ${params.email}"
            syncCoreThreads(accountOverride = selectedCoreAccountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure {
            errorBanner = it.message ?: "Add OAuth failed"
            status = "Add OAuth failed: ${it.message}"
        }
    }
}

/**
 * Gmail via the platform's system Google account. The host runs the full system
 * flow (pick account, mint token, read profile name) and returns the result.
 */
internal fun MeronMobileState.connectGoogleDeviceAccount() {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    if (!mobileHost.supportsGoogleDeviceAuth) {
        launchOAuthFlow()
        return
    }
    mobileHost.connectGoogleDeviceAccount { account ->
        when (account) {
            is GoogleDeviceAccountResult.Connected -> {
                addGoogleDeviceAccount(account.account)
            }

            GoogleDeviceAccountResult.Cancelled -> {
                status = "Google sign-in cancelled."
            }

            is GoogleDeviceAccountResult.Failed -> {
                val deviceAuthError = account.message.ifBlank { mobileHost.lastGoogleDeviceAuthError }
                if (mobileHost.googleRedirectUri.isBlank()) {
                    status =
                        listOf(
                            deviceAuthError,
                            "Google browser sign-in requires a configured HTTPS redirect URI.",
                        ).filter { it.isNotBlank() }.joinToString(" ")
                    return@connectGoogleDeviceAccount
                }
                status =
                    listOf(
                        deviceAuthError,
                        "Opening Google sign-in in browser...",
                    ).filter { it.isNotBlank() }.joinToString(" ")
                launchOAuthFlow()
            }
        }
    }
}

private fun MeronMobileState.addGoogleDeviceAccount(account: GoogleDeviceAccount) {
    status = "Connecting ${account.email}..."
    scope.launch {
        runCatching {
            val params =
                AddOAuthAccountParams(
                    email = account.email,
                    provider = "gmail",
                    displayName = account.displayName,
                    senderName = account.displayName,
                    username = account.email,
                    avatarUrl = account.avatarUrl,
                    // No refresh token: the host re-mints access tokens.
                    accessToken = account.accessToken,
                    refreshToken = "",
                    tokenExpiresAt = account.expiresAtEpochSeconds,
                )
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.addOAuthAccount(params)
                client.listAccounts()
            }
        }.onSuccess { accounts ->
            // meron-core keys accounts by lower-cased email.
            val accountId = account.email.trim().lowercase()
            mobileHost.recordManagedGoogleExpiry(accountId, account.expiresAtEpochSeconds)
            if (googleReauthAccountId == accountId) googleReauthAccountId = null
            applyAccounts(accounts, preferEmail = account.email)
            screen = Screen.Mail
            errorBanner = null
            status = "Connected ${account.email}"
            // Fetch the inbox immediately instead of waiting for a manual sync.
            syncCoreThreads(accountOverride = accountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure {
            errorBanner = it.message ?: "Google sign-in failed"
            status = "Google sign-in failed: ${it.message}"
        }
    }
}

/**
 * For host-managed Gmail accounts, mint a fresh access token and push it into
 * meron-core before a server-touching command. No-op for browser-flow /
 * non-managed accounts, and — unless [force] — while the last pushed token is
 * comfortably before expiry. Returns true only when a fresh token was pushed
 * into core. Failures are swallowed so a stale token still attempts the
 * command.
 */
internal suspend fun MeronMobileState.ensureManagedGoogleToken(
    client: MobileMailCommandClient,
    accountId: String,
    force: Boolean = false,
): Boolean {
    when (val refresh = mobileHost.refreshManagedGoogleToken(accountId, force)) {
        ManagedTokenRefresh.NotNeeded, ManagedTokenRefresh.StillFresh -> {
            Unit
        }

        is ManagedTokenRefresh.Refreshed -> {
            val pushed =
                runCatching {
                    client.updateOAuthToken(
                        UpdateOAuthTokenParams(
                            accountId = accountId,
                            accessToken = refresh.accessToken,
                            tokenExpiresAt = refresh.expiresAtEpochSeconds,
                        ),
                    )
                }
            if (pushed.isSuccess) {
                mobileHost.recordManagedGoogleExpiry(accountId, refresh.expiresAtEpochSeconds)
                if (googleReauthAccountId == accountId) googleReauthAccountId = null
                return true
            }
        }

        ManagedTokenRefresh.Failed -> {
            // OS could not silently mint a token (e.g. consent revoked).
            googleReauthAccountId = accountId
            errorBanner = "Google sign-in expired. Reconnect the account on this device."
        }

        ManagedTokenRefresh.TransientError -> {
            // Network hiccup while minting — not a reconnect case. Attempt the
            // command with the stored token; it may still be valid.
            Unit
        }
    }
    return false
}

/**
 * Run a server-touching mail command with managed-Gmail token upkeep: refresh
 * the pushed token first when it is near expiry, and if the server still
 * rejects our OAuth credentials (token revoked mid-session, or core state that
 * drifted from the host's expiry record), force-mint a fresh token and retry
 * once. Non-managed accounts run [action] unchanged. Handles both failure
 * shapes: hosts whose core invoke throws on an error payload, and ones that
 * return the payload for the caller to inspect.
 */
internal suspend fun MeronMobileState.withManagedGoogleAuth(
    client: MobileMailCommandClient,
    accountId: String,
    action: suspend () -> String,
): String {
    if (accountId.isBlank()) return action()
    ensureManagedGoogleToken(client, accountId)
    val first = runCatching { action() }
    (first.exceptionOrNull() as? CancellationException)?.let { throw it }
    val errorMessage = first.exceptionOrNull()?.message ?: first.getOrNull()?.let(::coreErrorMessage)
    if (!isOAuthLoginFailure(errorMessage)) return first.getOrThrow()
    if (!ensureManagedGoogleToken(client, accountId, force = true)) return first.getOrThrow()
    return action()
}

internal fun MeronMobileState.exchangeOAuthCode() {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val code = oauthAuthorizationCode.trim()
    if (code.isBlank()) {
        status = "OAuth authorization code is required."
        return
    }
    val clientId = bakedOAuthClientId()
    if (clientId.isBlank()) {
        status = "OAuth client ID is required."
        return
    }
    val params =
        ExchangeOAuthCodeParams(
            email = oauthEmail.trim(),
            provider = oauthProvider,
            displayName = displayName.trim(),
            senderName = senderName.trim(),
            code = code,
            clientId = clientId,
            clientSecret = "",
            redirectUri = oauthRedirectUri.trim(),
            codeVerifier = oauthVerifier,
            tokenUrl = if (oauthProvider == "gmail") mobileHost.googleTokenUrl else "",
        )
    Log.i(
        "Meron.OAuth",
        "exchange start provider=${params.provider} emailPresent=${params.email.isNotBlank()} " +
            "clientIdPresent=${params.clientId.isNotBlank()} redirectUri=${params.redirectUri} " +
            "tokenUrlPresent=${params.tokenUrl.isNotBlank()} codeLength=${params.code.length} " +
            "verifierPresent=${params.codeVerifier.isNotBlank()}",
    )
    status = "Exchanging OAuth code..."
    val previousAccountIds = coreAccounts.map { it.id }.toSet()
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                client.exchangeOAuthCode(params)
                client.listAccounts()
            }
        }.onSuccess { accountsJson ->
            val parsedAccounts = parseAccountListResponse(accountsJson)
            val connectedAccount =
                findOAuthResultAccount(
                    accounts = parsedAccounts,
                    previousAccountIds = previousAccountIds,
                    provider = params.provider,
                    preferredEmail = params.email,
                )
            applyAccounts(accountsJson, preferEmail = connectedAccount?.email ?: params.email.ifBlank { null })
            connectedAccount?.let { selectedCoreAccountId = it.id }
            screen = Screen.Mail
            errorBanner = null
            status = connectedAccount?.email?.takeIf { it.isNotBlank() }?.let { "Connected $it" }
                ?: if (params.email.isBlank()) "Connected account" else "Connected ${params.email}"
            val syncAccountId = connectedAccount?.id ?: selectedCoreAccountId
            Log.i(
                "Meron.OAuth",
                "exchange success provider=${params.provider} selectedAccount=$syncAccountId " +
                    "connectedEmailPresent=${connectedAccount?.email?.isNotBlank() == true}",
            )
            syncCoreThreads(accountOverride = syncAccountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure {
            Log.w("Meron.OAuth", "exchange failed provider=${params.provider}: ${it.message}", it)
            oauthAuthorizationCode = ""
            errorBanner = it.message ?: "OAuth exchange failed"
            status = "OAuth exchange failed: ${it.message}"
        }
    }
}

@OptIn(ExperimentalUuidApi::class)
internal fun MeronMobileState.launchOAuthFlow() {
    val clientId = bakedOAuthClientId()
    if (clientId.isBlank()) {
        status = "OAuth client ID is required."
        return
    }
    val redirectUri = resolvedOAuthRedirectUri()
    oauthRedirectUri = redirectUri
    oauthState = Uuid.random().toString()
    oauthVerifier = Uuid.random().toString() + Uuid.random().toString()
    savePendingOAuthFlow(
        prefs,
        PendingOAuthFlow(
            provider = oauthProvider,
            state = oauthState,
            verifier = oauthVerifier,
            redirectUri = redirectUri,
            email = oauthEmail.trim(),
        ),
    )
    val url =
        buildOAuthAuthorizationUrl(
            OAuthAuthorizationRequest(
                provider = oauthProvider,
                clientId = clientId,
                redirectUri = redirectUri,
                state = oauthState,
                codeChallenge = pkceChallenge(oauthVerifier),
                loginHint = oauthEmail.trim(),
            ),
        )
    status = "Opened ${oauthProvider.replaceFirstChar { it.uppercase() }} sign-in"
    services.openOAuthUrl(
        url = url,
        callbackScheme = redirectUri.substringBefore(':', missingDelimiterValue = ""),
        onCallback = ::handleOAuthCallback,
        onFailure = { message -> status = "OAuth browser launch failed: $message" },
    )
}

internal fun MeronMobileState.handleOAuthCallback(rawUrl: String) {
    Log.i("Meron.OAuth", "callback received length=${rawUrl.length}")
    loadPendingOAuthFlow(prefs)?.let { pending ->
        Log.i(
            "Meron.OAuth",
            "pending flow provider=${pending.provider} redirectUri=${pending.redirectUri} emailPresent=${pending.email.isNotBlank()}",
        )
        oauthProvider = pending.provider
        oauthState = pending.state
        oauthVerifier = pending.verifier
        oauthRedirectUri = pending.redirectUri
        oauthEmail = pending.email
    }
    runCatching {
        parseOAuthCallbackUrlForRedirect(
            rawUrl = rawUrl,
            expectedState = oauthState,
            redirectUri = oauthRedirectUri.trim(),
        )
    }.onSuccess { result ->
        if (result != null) {
            Log.i("Meron.OAuth", "callback parsed provider=$oauthProvider codeLength=${result.code.length}")
            oauthAuthorizationCode = result.code
            addSection = 0
            passwordServerSettingsOpen = false
            screen = Screen.AddAccount
            status = "Finishing ${oauthProvider.replaceFirstChar { it.uppercase() }} sign-in..."
            clearPendingOAuthFlow(prefs)
            exchangeOAuthCode()
        } else {
            Log.w("Meron.OAuth", "callback did not match redirectUri=$oauthRedirectUri")
            status = "OAuth callback did not match expected redirect URI."
        }
    }.onFailure {
        Log.w("Meron.OAuth", "callback parse failed: ${it.message}", it)
        status = "OAuth callback failed: ${it.message}"
    }
}

private fun MeronMobileState.bakedOAuthClientId(): String =
    when (oauthProvider) {
        "outlook" -> mobileHost.outlookClientId
        "gmail" -> mobileHost.googleClientId
        else -> ""
    }.trim()

private fun MeronMobileState.resolvedOAuthRedirectUri(): String =
    when (oauthProvider) {
        "outlook" -> mobileHost.outlookRedirectUri
        "gmail" -> mobileHost.googleRedirectUri.ifBlank { oauthRedirectUri.ifBlank { defaultOAuthRedirectUri() } }
        else -> oauthRedirectUri.ifBlank { defaultOAuthRedirectUri() }
    }.trim()

internal suspend fun MeronMobileState.loadAccountInbox(
    client: MobileMailCommandClient,
    account: AccountSummary,
    requestedFolder: String,
    query: String = mailSearch,
    filter: FilterMode = mailFilter,
    syncFirst: Boolean = true,
    beforeCursor: String? = null,
    syncLimit: Int = MAILBOX_SYNC_LIMIT,
    listLimit: Int = MAILBOX_PAGE_SIZE,
    refreshSearch: Boolean = true,
): MailboxLoadResult {
    // When syncFirst is false we read whatever the local (encrypted) store
    // already has — used on startup so the inbox shows instantly without a
    // server round-trip. Pull-to-sync / "Sync now" still fetch from server.
    Log.i(
        "MailLoad",
        "loadAccountInbox start account=${account.id} requestedFolder=$requestedFolder syncFirst=$syncFirst beforeCursor=${beforeCursor?.isNotBlank() == true} query=${query.isNotBlank()} filter=${filter.protocolValue()}",
    )
    if (syncFirst) {
        if (accountSummaryIsRss(account)) {
            Log.i("MailLoad", "loadAccountInbox sync rss account=${account.id}")
            client.syncRss(SyncRssParams(accountId = account.id))
        } else {
            Log.i("MailLoad", "loadAccountInbox sync mail account=${account.id} folder=$requestedFolder")
            withManagedGoogleAuth(client, account.id) {
                client.sync(
                    SyncMailParams(
                        accountId = account.id,
                        folderId = requestedFolder,
                        limit = syncLimit,
                        folders = true,
                        deferTail = true,
                    ),
                )
            }
        }
    }
    val foldersJson = client.listFolders(FolderListParams(accountId = account.id))
    val folders = parseFolderListResponse(foldersJson)
    // Server folder names are case-sensitive ("INBOX"), but the default
    // request uses "inbox"; match case-insensitively and fall back to a real
    // inbox before the first folder.
    val folder =
        folders.firstOrNull { it.name.equals(requestedFolder, ignoreCase = true) }?.name
            ?: folders.firstOrNull { it.name.equals(INBOX_FOLDER, ignoreCase = true) }?.name
            ?: folders.firstOrNull()?.name
            ?: requestedFolder
    Log.i("MailLoad", "loadAccountInbox folders account=${account.id} count=${folders.size} resolvedFolder=$folder")
    if (!accountSummaryIsRss(account) && query.isNotBlank() && refreshSearch) {
        // The refreshed search is a live IMAP operation even when syncFirst is
        // false. Keep token upkeep out of the cache-first request so local
        // matches can paint without waiting for the network.
        ensureManagedGoogleToken(client, account.id)
    }
    val threadsJson =
        client.listThreads(
            ThreadListParams(
                accountId = account.id,
                folderId = folder,
                query = query.trim(),
                filter = filter.protocolValue(),
                beforeCursor = beforeCursor,
                refresh = refreshSearch,
                // Paging forward always fetches a single page; only a reload of
                // the whole mailbox re-requests the depth already on screen.
                limit = if (beforeCursor == null) listLimit else MAILBOX_PAGE_SIZE,
            ),
        )
    val page = parseThreadListPage(threadsJson)
    val foldersWithPageUnread =
        page.folderUnread?.let { unread ->
            folders.map { item ->
                if (item.name.equals(folder, ignoreCase = folder.equals(INBOX_FOLDER, ignoreCase = true))) {
                    item.copy(unread = unread)
                } else {
                    item
                }
            }
        } ?: folders
    Log.i(
        "MailLoad",
        "loadAccountInbox threads account=${account.id} folder=$folder count=${page.threads.size} cursor=${page.nextCursor.isNotBlank()}",
    )
    return MailboxLoadResult(
        folders = foldersWithPageUnread,
        folder = folder,
        threads = page.threads,
        unreadCount = page.folderUnread,
        nextCursor = page.nextCursor,
        folderSynced = page.folderSynced,
    )
}

internal suspend fun MeronMobileState.loadUnifiedInbox(
    client: MobileMailCommandClient,
    accounts: List<AccountSummary>,
    query: String = mailSearch,
    filter: FilterMode = mailFilter,
    syncFirst: Boolean = true,
    beforeCursor: String? = null,
    syncLimit: Int = MAILBOX_SYNC_LIMIT,
    listLimit: Int = MAILBOX_PAGE_SIZE,
    refreshSearch: Boolean = true,
    /**
     * Which special-use folder the view is on. The unified view addresses
     * mailboxes by role: the core resolves each account's own Sent/Archive/…
     * and leaves out accounts whose server has none.
     */
    folderRole: String = INBOX_FOLDER,
): MailboxLoadResult {
    val role = unifiedFolderRole(folderRole)
    if (syncFirst) {
        accounts.forEach { account ->
            withSyncAccountContext(account.id) {
                if (accountSummaryIsRss(account)) {
                    if (role == INBOX_FOLDER) {
                        client.syncRss(SyncRssParams(accountId = account.id))
                    }
                } else {
                    withManagedGoogleAuth(client, account.id) {
                        var accountFolders =
                            parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
                        var targetFolder = unifiedAccountFolder(accountFolders, role)
                        // A cold cache cannot resolve a provider-specific Sent /
                        // Archive name. Refresh folder metadata through Inbox,
                        // then resolve the role again before syncing its mailbox.
                        if (targetFolder == null) {
                            client.sync(
                                SyncMailParams(
                                    accountId = account.id,
                                    folderId = INBOX_FOLDER,
                                    limit = syncLimit,
                                    folders = true,
                                    deferTail = true,
                                ),
                            )
                            accountFolders =
                                parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
                            targetFolder = unifiedAccountFolder(accountFolders, role)
                        }
                        if (targetFolder != null) {
                            client.sync(
                                SyncMailParams(
                                    accountId = account.id,
                                    folderId = targetFolder,
                                    limit = syncLimit,
                                    folders = true,
                                    deferTail = true,
                                ),
                            )
                        } else {
                            "{}"
                        }
                    }
                }
            }
        }
    }
    val folders =
        accounts.flatMap { account ->
            withSyncAccountContext(account.id) {
                parseFolderListResponse(client.listFolders(FolderListParams(accountId = account.id)))
            }
        }
    if (query.isNotBlank() && refreshSearch) {
        // Unified live search fans out inside one core call and falls back per
        // account, so refresh all managed mail tokens before issuing it.
        accounts.filterNot(::accountSummaryIsRss).forEach { account ->
            ensureManagedGoogleToken(client, account.id)
        }
    }
    val page =
        parseThreadListPage(
            client.listThreads(
                ThreadListParams(
                    accountId = UNIFIED_ACCOUNT_ID,
                    folderId = role,
                    folderRole = role,
                    query = query.trim(),
                    filter = filter.protocolValue(),
                    beforeCursor = beforeCursor,
                    refresh = refreshSearch,
                    // The core fans this out per account, so the limit is a
                    // per-account depth here too — matching how load-more appends
                    // one page from each account.
                    limit = if (beforeCursor == null) listLimit else MAILBOX_PAGE_SIZE,
                ),
            ),
        )
    return MailboxLoadResult(
        folders = folders,
        folder = role,
        threads = page.threads,
        // Only the inbox rolls up into the drawer's unread badge; the other
        // unified folders have no badge to keep in sync.
        unreadCount = page.folderUnread.takeIf { role == INBOX_FOLDER },
        nextCursor = page.nextCursor,
        folderSynced = page.folderSynced,
    )
}

/**
 * The unified starred listing. Unlike the other unified folders this is not a
 * mailbox any account owns: the core returns the starred *items* themselves —
 * single messages and feed entries across every account — which the list shows
 * as rows the same way a thread is shown.
 */
internal suspend fun MeronMobileState.loadUnifiedStarred(
    client: MobileMailCommandClient,
    query: String = mailSearch,
    filter: FilterMode = mailFilter,
    beforeCursor: String? = null,
    limit: Int = MAILBOX_PAGE_SIZE,
): MailboxLoadResult {
    val page =
        parseStarredItemsPage(
            client.listStarredItems(
                StarredItemsParams(
                    query = query.trim(),
                    filter = filter.protocolValue(),
                    limit = limit,
                    beforeCursor = beforeCursor,
                ),
            ),
        )
    val rows = page.items.map { it.toThreadSummary() }
    return MailboxLoadResult(
        folders = emptyList(),
        folder = STARRED_FOLDER,
        threads = rows,
        nextCursor = page.nextCursor,
    )
}

internal suspend fun MeronMobileState.loadAccountFolders(
    client: MobileMailCommandClient,
    account: AccountSummary,
): List<FolderSummary> {
    val foldersJson = client.listFolders(FolderListParams(accountId = account.id))
    return parseFolderListResponse(foldersJson)
}
