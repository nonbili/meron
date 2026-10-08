package jp.nonbili.meron.ui

import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Settings
import jp.nonbili.meron.shared.AccountAliasParams
import jp.nonbili.meron.shared.AccountAliasesParams
import jp.nonbili.meron.shared.AccountAvatarParams
import jp.nonbili.meron.shared.AccountChatWallpaperParams
import jp.nonbili.meron.shared.AccountFlagParams
import jp.nonbili.meron.shared.AccountIdParams
import jp.nonbili.meron.shared.AccountNameParams
import jp.nonbili.meron.shared.AccountProxyParams
import jp.nonbili.meron.shared.AccountReorderParams
import jp.nonbili.meron.shared.AccountRssSyncIntervalParams
import jp.nonbili.meron.shared.AccountSignatureParams
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.AddPasswordAccountParams
import jp.nonbili.meron.shared.AddRssAccountParams
import jp.nonbili.meron.shared.AppPrefsGetParams
import jp.nonbili.meron.shared.AppPrefsSetParams
import jp.nonbili.meron.shared.AutodiscoverAccountParams
import jp.nonbili.meron.shared.CertificateProtocol
import jp.nonbili.meron.shared.ExportOpmlParams
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.ProxyParams
import jp.nonbili.meron.shared.ProxySpec
import jp.nonbili.meron.shared.SignatureSpec
import jp.nonbili.meron.shared.accountSummaryIsRss
import jp.nonbili.meron.shared.encodeAppPrefValue
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseAppPrefsResponse
import jp.nonbili.meron.shared.parseAutodiscoverResponse
import jp.nonbili.meron.shared.parseOpmlExportResponse
import jp.nonbili.meron.shared.parseProxyResponse
import jp.nonbili.meron.shared.parseStorageUsageResponse
import jp.nonbili.meron.shared.requireCoreOk
import jp.nonbili.meron.shared.sanitizeRemoteSenders
import jp.nonbili.meron.shared.untrustedCertificateProtocol
import jp.nonbili.meron.shared.withRemoteSender
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/**
 * Core reported that it refused to use this account's sign-in (missing, or
 * rejected by the provider). Take the account's reconnect state from a fresh
 * account list, so the drawer and the reconnect banner show it now — and so a
 * report that was overtaken by a reconnect, about credentials already replaced,
 * changes nothing. Watchers repeat the report while they retry; only an account
 * not yet flagged is looked up.
 */
internal fun MeronMobileState.refreshAccountReconnectState(accountId: String) {
    if (!coreLoaded || coreAccounts.none { it.id == accountId && !it.needsReconnect }) return
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { parseAccountListResponse(MobileMailCommandClient(core).listAccounts()) }
        }.onSuccess { fresh ->
            if (fresh.any { it.id == accountId && it.needsReconnect }) {
                coreAccounts = coreAccounts.map { if (it.id == accountId) it.copy(needsReconnect = true) else it }
            }
        }
    }
}

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
    if (selectedCoreAccountId != previousAccountId) {
        selectedCoreFolder = loadMailFolderForAccount(prefs, selectedCoreAccountId)
    }
    saveLastMailLocation(prefs, selectedCoreAccountId, selectedCoreFolder)
    kanbanBoards = ensureKanbanDefaults(kanbanPrefs, kanbanBoards, parsed)
    if (activeKanbanBoardId.isBlank() || kanbanBoards.none { it.id == activeKanbanBoardId }) {
        activeKanbanBoardId = kanbanBoards.firstOrNull()?.id.orEmpty()
        saveActiveKanbanBoardId(kanbanPrefs, activeKanbanBoardId)
    }
}

internal fun findOAuthResultAccount(
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
internal fun MeronMobileState.addPasswordAccount(params: AddPasswordAccountParams) {
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
            selectedCoreThread = null
            messages = emptyList()
            coreThreads = emptyList()
            applyAccounts(it)
            clearMailFolderForAccount(prefs, account.id)
            status = "Removed account"
            syncCoreThreads(syncFirst = false, successStatus = "Removed account")
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
