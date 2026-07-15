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
import jp.nonbili.meron.shared.UpdateOAuthTokenParams
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
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
    selectedCoreAccountId = preferEmail?.let { wanted -> parsed.firstOrNull { it.email == wanted }?.id }
        ?: selectedCoreAccountId.takeIf { sel -> sel == UNIFIED_ACCOUNT_ID || parsed.any { it.id == sel } }
        ?: UNIFIED_ACCOUNT_ID
    if (selectedCoreAccountId == UNIFIED_ACCOUNT_ID) {
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

internal fun MeronMobileState.listAccounts() {
    if (!coreLoaded) {
        status = "Rust core not packaged."
        initialAccountsLoaded = true
        return
    }
    scope.launch {
        accountsLoading = true
        runCatching {
            withContext(ioDispatcher) { MobileMailCommandClient(core).listAccounts() }
        }.onSuccess {
            applyAccounts(it)
            mobileHost.syncLiveMailPush(liveMailPushEnabled)
        }.onFailure {
            status = "Account list failed: ${it.message}"
        }
        accountsLoading = false
        initialAccountsLoaded = true
    }
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
        return
    }
    val params =
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
            tls = true,
        )
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
            screen = Screen.Mail
            errorBanner = null
            status = "Added ${params.email}"
            syncCoreThreads(accountOverride = selectedCoreAccountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure {
            errorBanner = it.message ?: "Add account failed"
            status = "Add account failed: ${it.message}"
        }
    }
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
        status = "Rust core not packaged."
        return
    }
    status = "Finding mail settings..."
    scope.launch {
        runCatching {
            withContext(ioDispatcher) {
                val client = MobileMailCommandClient(core)
                parseAutodiscoverResponse(client.autodiscoverAccount(AutodiscoverAccountParams(emailValue)))
            }
        }.onSuccess { discovered ->
            if (discovered.imapHost.isNotBlank()) host = discovered.imapHost
            if (discovered.imapPort > 0) imapPort = discovered.imapPort.toString()
            if (discovered.smtpHost.isNotBlank()) smtpHost = discovered.smtpHost
            if (discovered.smtpPort > 0) smtpPort = discovered.smtpPort.toString()
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
            passwordServerSettingsOpen = true
            status = "Settings lookup failed: ${it.message}"
        }
    }
}

internal fun MeronMobileState.addRssAccount() {
    if (rssAccountAdding) return
    if (!coreLoaded) {
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
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
        status = "Rust core not packaged."
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
 * meron-core before a sync. No-op for browser-flow / non-managed accounts.
 * Failures are swallowed so a stale token still attempts the sync.
 */
internal suspend fun MeronMobileState.ensureManagedGoogleToken(
    client: MobileMailCommandClient,
    accountId: String,
) {
    when (val refresh = mobileHost.refreshManagedGoogleToken(accountId)) {
        ManagedTokenRefresh.NotNeeded -> {
            Unit
        }

        is ManagedTokenRefresh.Refreshed -> {
            runCatching {
                client.updateOAuthToken(
                    UpdateOAuthTokenParams(
                        accountId = accountId,
                        accessToken = refresh.accessToken,
                        tokenExpiresAt = refresh.expiresAtEpochSeconds,
                    ),
                )
            }.onSuccess {
                mobileHost.recordManagedGoogleExpiry(accountId, refresh.expiresAtEpochSeconds)
                if (googleReauthAccountId == accountId) googleReauthAccountId = null
            }
        }

        ManagedTokenRefresh.Failed -> {
            // OS could not silently mint a token (e.g. consent revoked).
            googleReauthAccountId = accountId
            errorBanner = "Google sign-in expired. Reconnect the account on this device."
        }

        ManagedTokenRefresh.TransientError -> {
            // Network hiccup while minting — not a reconnect case. Attempt the
            // sync with the stored token; it may still be valid.
            Unit
        }
    }
}

internal fun MeronMobileState.exchangeOAuthCode() {
    if (!coreLoaded) {
        status = "Rust core not packaged."
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
            ensureManagedGoogleToken(client, account.id)
            Log.i("MailLoad", "loadAccountInbox sync mail account=${account.id} folder=$requestedFolder")
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
    val threadsJson =
        client.listThreads(
            ThreadListParams(
                accountId = account.id,
                folderId = folder,
                query = query.trim(),
                filter = filter.protocolValue(),
                beforeCursor = beforeCursor,
            ),
        )
    val page = parseThreadListPage(threadsJson)
    Log.i(
        "MailLoad",
        "loadAccountInbox threads account=${account.id} folder=$folder count=${page.threads.size} cursor=${page.nextCursor.isNotBlank()}",
    )
    return MailboxLoadResult(
        folders = folders,
        folder = folder,
        threads = page.threads,
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
