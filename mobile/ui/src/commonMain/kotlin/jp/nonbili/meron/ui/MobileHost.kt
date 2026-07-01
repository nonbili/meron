package jp.nonbili.meron.ui

/** Final data for a Google account connected through the platform's system
 *  sign-in (Android AccountManager). */
data class GoogleDeviceAccount(
    val email: String,
    val displayName: String,
    val avatarUrl: String = "",
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

    /** Whether the platform can keep mail fresh while suspended via a real
     *  background mechanism (Android foreground service / WorkManager). When
     *  false (iOS), the UI hides live push & manual background refresh and offers
     *  a foreground poll interval instead. */
    val supportsBackgroundPush: Boolean
        get() = true

    fun syncLiveMailPush(enabled: Boolean)

    fun syncBackgroundRefresh(enabled: Boolean)

    fun runBackgroundRefreshOnce()

    fun notifyNewMail(
        accountName: String,
        from: String,
        subject: String,
        count: Int,
        accountId: String,
        folder: String,
        threadKey: String,
    )

    /** Outlook OAuth client id baked into the build (empty when unset). */
    val outlookClientId: String

    /** Outlook OAuth redirect URI baked into the build (empty when unset). */
    val outlookRedirectUri: String

    /** Google OAuth client id baked into the build (empty when unset). */
    val googleClientId: String

    /** Google OAuth redirect URI baked into the build (empty when unset). */
    val googleRedirectUri: String

    /** Server-side Google OAuth token exchange endpoint (empty to call Google directly). */
    val googleTokenUrl: String

    val packageName: String

    val appVersionName: String

    val coreProtocolVersion: Int

    /** Whether the platform offers system Google account sign-in (Android only). */
    val supportsGoogleDeviceAuth: Boolean

    /** Last system Google sign-in failure, if the host returned no account. */
    val lastGoogleDeviceAuthError: String
        get() = ""

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
    override val googleClientId: String = "",
    override val googleRedirectUri: String = "",
    override val googleTokenUrl: String = "",
) : MobileHost {
    override val supportsBackgroundPush: Boolean = false

    override fun notificationsEnabled(): Boolean = true

    override fun requestNotificationPermission() {}

    override fun syncLiveMailPush(enabled: Boolean) {}

    override fun syncBackgroundRefresh(enabled: Boolean) {}

    override fun runBackgroundRefreshOnce() {}

    override fun notifyNewMail(
        accountName: String,
        from: String,
        subject: String,
        count: Int,
        accountId: String,
        folder: String,
        threadKey: String,
    ) {}

    override val supportsGoogleDeviceAuth: Boolean = false

    override val packageName: String = ""

    override val appVersionName: String = ""

    override val coreProtocolVersion: Int = 0

    override fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccount?) -> Unit) = onResult(null)

    override suspend fun refreshManagedGoogleToken(accountId: String): ManagedTokenRefresh = ManagedTokenRefresh.NotNeeded

    override fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    ) {}
}
