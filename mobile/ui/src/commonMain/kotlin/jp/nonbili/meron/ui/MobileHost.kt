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

/** Final outcome of platform Google account sign-in. */
sealed interface GoogleDeviceAccountResult {
    data class Connected(
        val account: GoogleDeviceAccount,
    ) : GoogleDeviceAccountResult

    data object Cancelled : GoogleDeviceAccountResult

    data class Failed(
        val message: String,
    ) : GoogleDeviceAccountResult
}

/** Result of silently refreshing a managed Google access token. */
sealed interface ManagedTokenRefresh {
    data object NotNeeded : ManagedTokenRefresh

    /** The token already pushed into core is comfortably before its recorded
     *  expiry; minting was skipped. */
    data object StillFresh : ManagedTokenRefresh

    data class Refreshed(
        val accessToken: String,
        val expiresAtEpochSeconds: Long,
    ) : ManagedTokenRefresh

    data object Failed : ManagedTokenRefresh

    /** Minting hit a transient network error; the stored token may still work. */
    data object TransientError : ManagedTokenRefresh
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

    /** Export the on-device background-sync diagnostic log via the platform
     *  share sheet, so a user can send it to support without adb. No-op where
     *  no diagnostic log is kept. */
    fun shareDiagnosticLog() {}

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

    /** Last system Google sign-in failure, if the host returned a failure. */
    val lastGoogleDeviceAuthError: String
        get() = ""

    /** Run the full system Google sign-in (pick account, mint token, fetch name).
     *  [onResult] gets the connected account, user cancellation, or failure. No-op
     *  where unsupported. */
    fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccountResult) -> Unit)

    /** Silently mint a fresh access token for a managed Google account before a
     *  server-touching command. Unless [force], the host may skip minting and
     *  return [ManagedTokenRefresh.StillFresh] while the last pushed token is
     *  comfortably before expiry; [force] mints regardless — used after the
     *  server rejected the stored token. */
    suspend fun refreshManagedGoogleToken(
        accountId: String,
        force: Boolean = false,
    ): ManagedTokenRefresh

    /** Persist the latest known expiry for a managed Google account. */
    fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    )
}

/** Fallback host: no system Google sign-in; reports notifications as enabled so
 *  permission-gated UI stays hidden unless the platform host overrides it (the
 *  iOS host layers real UNUserNotificationCenter wiring on top of this). */
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

    override fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccountResult) -> Unit) = onResult(GoogleDeviceAccountResult.Cancelled)

    override suspend fun refreshManagedGoogleToken(
        accountId: String,
        force: Boolean,
    ): ManagedTokenRefresh = ManagedTokenRefresh.NotNeeded

    override fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    ) {}
}
