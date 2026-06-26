package jp.nonbili.meron.ui

/** Final data for a Google account connected through the platform's system
 *  sign-in (Android AccountManager). */
data class GoogleDeviceAccount(
    val email: String,
    val displayName: String,
    val accessToken: String,
    val expiresAtEpochSeconds: Long,
)

/** Result of silently refreshing a managed Google access token. */
sealed interface ManagedTokenRefresh {
    data object NotNeeded : ManagedTokenRefresh

    data class Refreshed(
        val accessToken: String,
        val expiresAtEpochSeconds: Long,
    ) : ManagedTokenRefresh

    data object Failed : ManagedTokenRefresh
}

/** Host-provided platform capabilities the shared UI/state needs beyond
 *  [PlatformServices]: notifications, live push, background refresh, build config,
 *  and the Android-only Google device-account OAuth. iOS supplies no-op/false
 *  defaults where a capability does not apply. */
interface MobileHost {
    fun notificationsEnabled(): Boolean

    fun requestNotificationPermission()

    fun syncLiveMailPush(enabled: Boolean)

    fun runBackgroundRefreshOnce()

    /** Outlook OAuth client id baked into the build (empty when unset). */
    val outlookClientId: String

    /** Outlook OAuth redirect URI baked into the build (empty when unset). */
    val outlookRedirectUri: String

    /** Whether the platform offers system Google account sign-in (Android only). */
    val supportsGoogleDeviceAuth: Boolean

    /** Run the full system Google sign-in (pick account, mint token, fetch name).
     *  [onResult] gets the connected account, or null on cancel/failure. No-op
     *  where unsupported. */
    fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccount?) -> Unit)

    /** Silently mint a fresh access token for a managed Google account before sync. */
    suspend fun refreshManagedGoogleToken(accountId: String): ManagedTokenRefresh

    /** Persist the latest known expiry for a managed Google account. */
    fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    )
}

/** iOS default: no system Google sign-in; notifications/config wired by the host app. */
open class DefaultMobileHost(
    override val outlookClientId: String = "",
    override val outlookRedirectUri: String = "",
) : MobileHost {
    override fun notificationsEnabled(): Boolean = true

    override fun requestNotificationPermission() {}

    override fun syncLiveMailPush(enabled: Boolean) {}

    override fun runBackgroundRefreshOnce() {}

    override val supportsGoogleDeviceAuth: Boolean = false

    override fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccount?) -> Unit) = onResult(null)

    override suspend fun refreshManagedGoogleToken(accountId: String): ManagedTokenRefresh = ManagedTokenRefresh.NotNeeded

    override fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    ) {}
}
