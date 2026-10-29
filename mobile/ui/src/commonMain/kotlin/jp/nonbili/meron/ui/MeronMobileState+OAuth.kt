package jp.nonbili.meron.ui

import androidx.compose.material.icons.filled.Add
import jp.nonbili.meron.shared.AddOAuthAccountParams
import jp.nonbili.meron.shared.ExchangeOAuthCodeParams
import jp.nonbili.meron.shared.MobileMailCommandClient
import jp.nonbili.meron.shared.OAuthAuthorizeUrlParams
import jp.nonbili.meron.shared.UpdateOAuthTokenParams
import jp.nonbili.meron.shared.coreErrorMessage
import jp.nonbili.meron.shared.defaultOAuthRedirectUri
import jp.nonbili.meron.shared.isOAuthLoginFailure
import jp.nonbili.meron.shared.parseAccountListResponse
import jp.nonbili.meron.shared.parseOAuthCallbackUrlForRedirect
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlin.uuid.ExperimentalUuidApi
import kotlin.uuid.Uuid

internal fun MeronMobileState.addOAuthAccount() {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val refreshToken = oauthRefreshToken.trim()
    if (refreshToken.isBlank()) {
        status = trs("mobile.status.oauthRefreshTokenRequired")
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
    status = trs("mobile.status.addingProviderAccount", mapOf("provider" to oauthProvider.replaceFirstChar { it.uppercase() }))
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
            status = trs("mobile.status.accountAdded", mapOf("account" to params.email))
            syncCoreThreads(accountOverride = selectedCoreAccountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure {
            errorBanner = it.message ?: trs("mobile.status.addAccountFailed")
            status = trsError("mobile.status.addAccountFailed", it)
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
                status = trs("mobile.status.googleSignInCancelled")
            }

            is GoogleDeviceAccountResult.Failed -> {
                val deviceAuthError = account.message.ifBlank { mobileHost.lastGoogleDeviceAuthError }
                if (mobileHost.googleRedirectUri.isBlank()) {
                    status =
                        listOf(
                            deviceAuthError,
                            trs("mobile.status.googleRedirectMissing"),
                        ).filter { it.isNotBlank() }.joinToString(" ")
                    return@connectGoogleDeviceAccount
                }
                status =
                    listOf(
                        deviceAuthError,
                        trs("mobile.status.openingGoogleSignIn"),
                    ).filter { it.isNotBlank() }.joinToString(" ")
                launchOAuthFlow()
            }
        }
    }
}

private fun MeronMobileState.addGoogleDeviceAccount(account: GoogleDeviceAccount) {
    status = trs("mobile.status.connectingAccount", mapOf("account" to account.email))
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
            status = trs("mobile.status.accountConnected", mapOf("account" to account.email))
            // Fetch the inbox immediately instead of waiting for a manual sync.
            syncCoreThreads(accountOverride = accountId, folderOverride = INBOX_FOLDER, syncFirst = true)
        }.onFailure {
            errorBanner = it.message ?: trs("mobile.status.googleSignInFailed")
            status = trsError("mobile.status.googleSignInFailed", it)
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
            errorBanner = trs("mobile.status.googleSignInExpired")
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
        status = trs("mobile.status.oauthCodeRequired")
        return
    }
    val clientId = bakedOAuthClientId()
    if (clientId.isBlank()) {
        status = trs("mobile.status.oauthClientIdRequired")
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
    status = trs("mobile.status.exchangingOAuthCode")
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
            status = connectedAccount?.email?.takeIf { it.isNotBlank() }?.let { trs("mobile.status.accountConnected", mapOf("account" to it)) }
                ?: if (params.email.isBlank()) trs("mobile.status.accountConnectedUnnamed") else trs("mobile.status.accountConnected", mapOf("account" to params.email))
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
            errorBanner = it.message ?: trs("mobile.status.oauthExchangeFailed")
            status = trsError("mobile.status.oauthExchangeFailed", it)
        }
    }
}

@OptIn(ExperimentalUuidApi::class)
internal fun MeronMobileState.launchOAuthFlow() {
    if (!coreLoaded) {
        status = coreUnavailableMessage
        return
    }
    val clientId = bakedOAuthClientId()
    if (clientId.isBlank()) {
        status = trs("mobile.status.oauthClientIdRequired")
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
    val request =
        OAuthAuthorizeUrlParams(
            provider = oauthProvider,
            clientId = clientId,
            redirectUri = redirectUri,
            state = oauthState,
            codeChallenge = pkceChallenge(oauthVerifier),
            loginHint = oauthEmail.trim(),
        )
    scope.launch {
        runCatching {
            withContext(ioDispatcher) { MobileMailCommandClient(core).oauthAuthorizeUrl(request) }
        }.onSuccess { url ->
            status = trs("mobile.status.openedProviderSignIn", mapOf("provider" to oauthProvider.replaceFirstChar { it.uppercase() }))
            services.openOAuthUrl(
                url = url,
                callbackScheme = redirectUri.substringBefore(':', missingDelimiterValue = ""),
                onCallback = ::handleOAuthCallback,
                onFailure = { message -> status = withReason(trs("mobile.android.oauthBrowserLaunchFailed"), message) { key, args -> trs(key, args) } },
            )
        }.onFailure {
            if (it is CancellationException) throw it
            status = trsError("mobile.android.oauthBrowserLaunchFailed", it)
        }
    }
}

internal fun MeronMobileState.handleOAuthCallback(rawUrl: String) {
    Log.i("Meron.OAuth", "callback received length=${rawUrl.length}")
    val pending = loadPendingOAuthFlow(prefs)
    if (pending == null || pending.state.isBlank() || pending.redirectUri.isBlank()) {
        Log.w("Meron.OAuth", "callback ignored without a pending flow")
        return
    }
    Log.i(
        "Meron.OAuth",
        "pending flow provider=${pending.provider} redirectUri=${pending.redirectUri} emailPresent=${pending.email.isNotBlank()}",
    )
    oauthProvider = pending.provider
    oauthState = pending.state
    oauthVerifier = pending.verifier
    oauthRedirectUri = pending.redirectUri
    oauthEmail = pending.email
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
            status = trs("accounts.oauth.finishingSignIn")
            clearPendingOAuthFlow(prefs)
            exchangeOAuthCode()
        } else {
            Log.w("Meron.OAuth", "callback did not match redirectUri=$oauthRedirectUri")
            status = trs("mobile.status.oauthCallbackMismatch")
        }
    }.onFailure {
        Log.w("Meron.OAuth", "callback parse failed: ${it.message}", it)
        status = trsError("mobile.status.oauthCallbackFailed", it)
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
