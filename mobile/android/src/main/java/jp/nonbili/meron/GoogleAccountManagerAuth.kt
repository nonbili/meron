package jp.nonbili.meron

import android.accounts.Account
import android.accounts.AccountManager
import android.app.Activity
import android.content.Context
import android.os.Bundle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * Gmail auth via the on-device Google account (Android [AccountManager]).
 *
 * Unlike the browser PKCE flow, no refresh token is stored: the OS holds the
 * long-lived credential and silently mints short-lived access tokens. meron-core
 * keeps only the access token; we re-mint a fresh one before each sync with
 * [silentToken] and push it in via `account.updateOAuthToken`.
 *
 * A small local registry (SharedPreferences) records which meron account ids are
 * AccountManager-managed and the device account name to mint from, since a
 * browser-flow Gmail account looks identical (`gmail_oauth`) to meron-core.
 */
object GoogleAccountManagerAuth {
    const val ACCOUNT_TYPE = "com.google"

    /**
     * Full-mailbox IMAP/SMTP scope plus basic profile, in AccountManager
     * "oauth2:" token-type form. The profile scope lets us read the account
     * holder's display name from the userinfo endpoint with the same token.
     */
    const val TOKEN_TYPE =
        "oauth2:https://www.googleapis.com/auth/userinfo.profile https://mail.google.com/"

    private const val PREFS = "google_account_manager"

    /** Assumed access-token lifetime (Google access tokens last ~1h). */
    const val TOKEN_LIFETIME_SECONDS = 3600L

    private fun key(accountId: String) = "managed.$accountId"

    private fun expiryKey(accountId: String) = "expiry.$accountId"

    /** Outcome of [mintIfNeeded]. */
    sealed class TokenRefresh {
        /** Not a managed account, so nothing to do. */
        data object NotNeeded : TokenRefresh()

        /** A fresh token was minted; push it into meron-core, then [recordExpiry]. */
        data class Refreshed(
            val token: String,
            val expiresAt: Long,
        ) : TokenRefresh()

        /** Managed, but the OS could not mint a token — user must reconnect. */
        data object Failed : TokenRefresh()
    }

    data class GoogleUserProfile(
        val name: String = "",
        val pictureUrl: String = "",
    )

    /** Intent that lets the user pick a `com.google` account on the device. */
    fun chooseAccountIntent() =
        AccountManager.newChooseAccountIntent(
            null,
            null,
            arrayOf(ACCOUNT_TYPE),
            null,
            null,
            null,
            null,
        )

    /** Record that [accountId] is refreshed via the device account [accountName]. */
    fun register(
        context: Context,
        accountId: String,
        accountName: String,
    ) {
        prefs(context).edit().putString(key(accountId), accountName).apply()
    }

    fun unregister(
        context: Context,
        accountId: String,
    ) {
        prefs(context).edit().remove(key(accountId)).apply()
    }

    /** Device account name for a managed account, or null if not managed here. */
    fun managedAccountName(
        context: Context,
        accountId: String,
    ): String? = prefs(context).getString(key(accountId), null)

    /** Record the assumed expiry of the token currently stored in meron-core. */
    fun recordExpiry(
        context: Context,
        accountId: String,
        expiresAt: Long,
    ) {
        prefs(context).edit().putLong(expiryKey(accountId), expiresAt).apply()
    }

    /**
     * Mint a fresh access token for managed accounts before sync. Returns
     * [TokenRefresh.NotNeeded] only for non-managed accounts.
     *
     * The local expiry is advisory UI state; meron-core has the authoritative
     * token and can drift from this preference after reconnects or restores.
     * Re-minting here keeps platform-managed Gmail from falling back to core's
     * browser OAuth refresh path.
     */
    suspend fun mintIfNeeded(
        context: Context,
        accountId: String,
    ): TokenRefresh {
        val deviceAccount = managedAccountName(context, accountId) ?: return TokenRefresh.NotNeeded
        val token = silentToken(context, deviceAccount) ?: return TokenRefresh.Failed
        val now = System.currentTimeMillis() / 1000L
        return TokenRefresh.Refreshed(token, now + TOKEN_LIFETIME_SECONDS)
    }

    /**
     * Interactive token request, used at setup. Shows the consent dialog the
     * first time the user grants this app Gmail access. Throws on failure.
     */
    suspend fun interactiveToken(
        activity: Activity,
        accountName: String,
    ): String {
        val account = Account(accountName, ACCOUNT_TYPE)
        return suspendCancellableCoroutine { cont ->
            AccountManager.get(activity).getAuthToken(
                account,
                TOKEN_TYPE,
                Bundle(),
                activity,
                { future ->
                    try {
                        val token =
                            future.result.getString(AccountManager.KEY_AUTHTOKEN)
                                ?: throw IllegalStateException("Android returned no token")
                        cont.resume(token)
                    } catch (ex: Throwable) {
                        cont.resumeWithException(ex)
                    }
                },
                null,
            )
        }
    }

    /**
     * Silent token request, used before sync. Invalidates the cached token and
     * mints a fresh one. Returns null when user interaction is required (e.g.
     * consent revoked) — the caller should then surface "needs reconnect".
     */
    suspend fun silentToken(
        context: Context,
        accountName: String,
    ): String? =
        withContext(Dispatchers.IO) {
            val am = AccountManager.get(context)
            val account = Account(accountName, ACCOUNT_TYPE)
            val cached = am.blockingGetAuthToken(account, TOKEN_TYPE, true)
            if (cached != null) {
                am.invalidateAuthToken(ACCOUNT_TYPE, cached)
            }
            am.blockingGetAuthToken(account, TOKEN_TYPE, true)
        }

    /**
     * Read the account holder's display name and avatar from Google's userinfo
     * endpoint using a profile-scoped access token. Returns empty fields on any
     * failure — the caller falls back to generated account UI.
     */
    suspend fun fetchUserProfile(accessToken: String): GoogleUserProfile =
        withContext(Dispatchers.IO) {
            runCatching {
                val connection =
                    (
                        URL("https://www.googleapis.com/oauth2/v3/userinfo").openConnection()
                            as HttpURLConnection
                    ).apply {
                        setRequestProperty("Authorization", "Bearer $accessToken")
                        connectTimeout = 15_000
                        readTimeout = 15_000
                    }
                val body = connection.inputStream.bufferedReader().use { it.readText() }
                val json = JSONObject(body)
                GoogleUserProfile(
                    name = json.optString("name").trim(),
                    pictureUrl = json.optString("picture").trim(),
                )
            }.getOrDefault(GoogleUserProfile())
        }

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
