package jp.nonbili.meron

import android.Manifest
import android.accounts.AccountManager
import android.accounts.OperationCanceledException
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.content.FileProvider
import jp.nonbili.meron.ui.GoogleDeviceAccount
import jp.nonbili.meron.ui.GoogleDeviceAccountResult
import jp.nonbili.meron.ui.ManagedTokenRefresh
import jp.nonbili.meron.ui.MobileHost
import jp.nonbili.meron.ui.PickedFile
import jp.nonbili.meron.ui.PlatformServices
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import java.io.File

class AndroidPlatformServices(
    private val activity: ComponentActivity,
) : PlatformServices {
    private var pendingPick: ((PickedFile?) -> Unit)? = null

    private val openDocument =
        activity.registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            val callback = pendingPick
            pendingPick = null
            callback?.invoke(uri?.toPickedFile())
        }

    override fun openUrl(url: String) {
        activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
    }

    override fun openOAuthUrl(
        url: String,
        callbackScheme: String,
        onCallback: (String) -> Unit,
        onFailure: (String) -> Unit,
    ) {
        runCatching {
            CustomTabsIntent
                .Builder()
                .setShowTitle(true)
                .build()
                .launchUrl(activity, Uri.parse(url))
        }.recoverCatching {
            openUrl(url)
        }.onFailure {
            onFailure(it.message ?: "OAuth browser launch failed")
        }
    }

    override fun copyText(
        label: String,
        value: String,
    ) {
        val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        clipboard.setPrimaryClip(ClipData.newPlainText(label, value))
    }

    override fun copyImage(
        bytes: ByteArray,
        mimeType: String,
        label: String,
    ) {
        shareFile(bytes, "$label.${mimeType.substringAfter('/', "png")}", mimeType)
    }

    override fun shareFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    ) {
        shareBytesViaFileProvider(activity, bytes, fileName, mimeType)
    }

    override fun saveFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    ) {
        shareFile(bytes, fileName, mimeType)
    }

    override fun pickFile(
        mimeTypes: List<String>,
        onPicked: (PickedFile?) -> Unit,
    ) {
        pendingPick = onPicked
        openDocument.launch(mimeTypes.ifEmpty { listOf("*/*") }.toTypedArray())
    }

    override fun pickImage(onPicked: (PickedFile?) -> Unit) {
        pickFile(listOf("image/*"), onPicked)
    }

    private fun Uri.toPickedFile(): PickedFile? =
        runCatching {
            val bytes = activity.contentResolver.openInputStream(this)?.use { it.readBytes() } ?: return null
            PickedFile(
                name = activity.displayNameFor(this),
                bytes = bytes,
                mimeType = activity.contentResolver.getType(this) ?: "application/octet-stream",
            )
        }.getOrNull()
}

private fun shareBytesViaFileProvider(
    activity: ComponentActivity,
    bytes: ByteArray,
    fileName: String,
    mimeType: String,
) {
    val dir = File(activity.cacheDir, "attachments").apply { mkdirs() }
    val file = File(dir, fileName.ifBlank { "meron-file" })
    file.writeBytes(bytes)
    val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
    val intent =
        Intent(Intent.ACTION_SEND)
            .setType(mimeType)
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    activity.startActivity(Intent.createChooser(intent, fileName))
}

internal const val NOTIFICATION_PERMISSION_REQUEST_CODE = 4001

/** One-line explanation of why libmeron_core.so failed to load, with the facts
 *  needed to diagnose a bad install from a screenshot: the loader error, the
 *  device's supported ABIs (we ship arm64-v8a and armeabi-v7a), whether the per-ABI split
 *  APK made it onto the device (lost when the app is shared/restored instead
 *  of installed from Play), and the installer package. Empty once the core is
 *  loaded. */
internal fun describeCoreLoadFailure(context: Context): String {
    if (MeronCoreNative.isLoaded()) return ""
    val error = MeronCoreNative.loadError().ifBlank { "libmeron_core.so not found" }
    val abis = Build.SUPPORTED_ABIS.joinToString(", ").ifBlank { "unknown" }
    val splits =
        context.applicationInfo.splitSourceDirs
            ?.joinToString(", ") { File(it).name }
            ?.ifBlank { null } ?: "none"
    val installer =
        runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                context.packageManager.getInstallSourceInfo(context.packageName).installingPackageName
            } else {
                @Suppress("DEPRECATION")
                context.packageManager.getInstallerPackageName(context.packageName)
            }
        }.getOrNull() ?: "unknown"
    return "$error (device ABIs: $abis; APK splits: $splits; installer: $installer)"
}

class AndroidMobileHost(
    private val activity: ComponentActivity,
) : MobileHost {
    private companion object {
        const val GOOGLE_AUTH_LOG_TAG = "Meron.GoogleAuth"

        fun isGoogleAuthCancelled(error: Throwable): Boolean = error is OperationCanceledException || error.cause is OperationCanceledException
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var pendingGoogleAccountResult: ((GoogleDeviceAccountResult) -> Unit)? = null
    private val googleAccountPicker =
        activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            val callback = pendingGoogleAccountResult
            pendingGoogleAccountResult = null
            val accountName = result.data?.getStringExtra(AccountManager.KEY_ACCOUNT_NAME)
            if (result.resultCode == Activity.RESULT_CANCELED) {
                lastGoogleDeviceAuthError = ""
                Log.i(GOOGLE_AUTH_LOG_TAG, "Google account picker cancelled")
                callback?.invoke(GoogleDeviceAccountResult.Cancelled)
                return@registerForActivityResult
            }
            if (result.resultCode != Activity.RESULT_OK || accountName.isNullOrBlank()) {
                val message = "Google account picker did not return an account."
                lastGoogleDeviceAuthError = message
                Log.i(
                    GOOGLE_AUTH_LOG_TAG,
                    "Google account picker returned no account; resultCode=${result.resultCode} accountPresent=${!accountName.isNullOrBlank()}",
                )
                callback?.invoke(GoogleDeviceAccountResult.Failed(message))
                return@registerForActivityResult
            }
            Log.i(GOOGLE_AUTH_LOG_TAG, "Google account picker selected account; requesting token")
            lastGoogleDeviceAuthError = ""
            scope.launch {
                runCatching {
                    val token = GoogleAccountManagerAuth.interactiveToken(activity, accountName)
                    Log.i(GOOGLE_AUTH_LOG_TAG, "Google AccountManager token request succeeded")
                    val profile = GoogleAccountManagerAuth.fetchUserProfile(token)
                    val expiresAt = System.currentTimeMillis() / 1000L + GoogleAccountManagerAuth.TOKEN_LIFETIME_SECONDS
                    GoogleAccountManagerAuth.register(activity, accountName.trim().lowercase(), accountName)
                    GoogleDeviceAccount(
                        email = accountName,
                        displayName = profile.name,
                        avatarUrl = profile.pictureUrl,
                        accessToken = token,
                        expiresAtEpochSeconds = expiresAt,
                    )
                }.onSuccess {
                    callback?.invoke(GoogleDeviceAccountResult.Connected(it))
                }.onFailure {
                    if (isGoogleAuthCancelled(it)) {
                        lastGoogleDeviceAuthError = ""
                        Log.i(GOOGLE_AUTH_LOG_TAG, "Google AccountManager token request cancelled")
                        callback?.invoke(GoogleDeviceAccountResult.Cancelled)
                        return@onFailure
                    }
                    lastGoogleDeviceAuthError =
                        it.message?.takeIf { message -> message.isNotBlank() }
                            ?: it::class.simpleName
                            ?: "Google AccountManager token request failed."
                    Log.w(GOOGLE_AUTH_LOG_TAG, "Google AccountManager token request failed; falling back to browser", it)
                    AndroidSyncDiagnosticLog.appendRedacted(
                        activity,
                        "$GOOGLE_AUTH_LOG_TAG token request failed; falling back to browser: ${it.message}",
                    )
                    callback?.invoke(GoogleDeviceAccountResult.Failed(lastGoogleDeviceAuthError))
                }
            }
        }

    override val outlookClientId: String = BuildConfig.MERON_OUTLOOK_CLIENT_ID
    override val outlookRedirectUri: String = BuildConfig.MERON_OUTLOOK_REDIRECT_URI
    override val googleClientId: String = BuildConfig.MERON_GOOGLE_CLIENT_ID
    override val googleRedirectUri: String = BuildConfig.MERON_GOOGLE_REDIRECT_URI
    override val googleTokenUrl: String = BuildConfig.MERON_GOOGLE_TOKEN_URL
    override val supportsGoogleDeviceAuth: Boolean = true
    override var lastGoogleDeviceAuthError: String = ""
        private set
    override val packageName: String = activity.packageName
    override val appVersionName: String =
        activity.packageManager
            .getPackageInfo(activity.packageName, 0)
            .versionName
            .orEmpty()
    override val coreProtocolVersion: Int = MeronCoreNative.protocolVersion()

    override val coreLoadDiagnostics: String = describeCoreLoadFailure(activity)

    override fun notificationsEnabled(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == android.content.pm.PackageManager.PERMISSION_GRANTED

    override fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), NOTIFICATION_PERMISSION_REQUEST_CODE)
        }
    }

    override fun syncLiveMailPush(enabled: Boolean) {
        AndroidMailPushService.sync(activity)
    }

    override fun syncBackgroundRefresh(enabled: Boolean) {
        AndroidBackgroundSyncScheduler.sync(activity, enabled)
    }

    override fun runBackgroundRefreshOnce() {
        AndroidBackgroundSyncScheduler.runOnce(activity)
    }

    override fun readDiagnosticLog(): String = AndroidSyncDiagnosticLog.read(activity)

    override fun shareDiagnosticLog() {
        val body =
            AndroidSyncDiagnosticLog.read(activity).ifBlank {
                "No log entries recorded yet. Try again after the next sync."
            }
        val disclosure =
            "Account emails below are masked to only the first letter and domain (e.g. j***@gmail.com).\n" +
                "Review before sharing.\n\n"
        shareBytesViaFileProvider(activity, (disclosure + body).toByteArray(), "meron-sync-log.txt", "text/plain")
    }

    override fun notifyNewMail(
        accountName: String,
        from: String,
        subject: String,
        count: Int,
        accountId: String,
        folder: String,
        threadKey: String,
    ) {
        AndroidNotificationService.notifyNewMail(activity, accountName, from, subject, count, accountId, folder, threadKey)
    }

    override fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccountResult) -> Unit) {
        lastGoogleDeviceAuthError = ""
        Log.i(GOOGLE_AUTH_LOG_TAG, "Launching Google account picker")
        pendingGoogleAccountResult = onResult
        googleAccountPicker.launch(GoogleAccountManagerAuth.chooseAccountIntent())
    }

    override suspend fun refreshManagedGoogleToken(
        accountId: String,
        force: Boolean,
    ): ManagedTokenRefresh =
        when (val refresh = GoogleAccountManagerAuth.mintIfNeeded(activity, accountId, skipIfFresh = !force)) {
            GoogleAccountManagerAuth.TokenRefresh.NotNeeded -> {
                ManagedTokenRefresh.NotNeeded
            }

            GoogleAccountManagerAuth.TokenRefresh.StillFresh -> {
                ManagedTokenRefresh.StillFresh
            }

            is GoogleAccountManagerAuth.TokenRefresh.Refreshed -> {
                ManagedTokenRefresh.Refreshed(
                    accessToken = refresh.token,
                    expiresAtEpochSeconds = refresh.expiresAt,
                )
            }

            GoogleAccountManagerAuth.TokenRefresh.Failed -> {
                ManagedTokenRefresh.Failed
            }

            GoogleAccountManagerAuth.TokenRefresh.TransientError -> {
                ManagedTokenRefresh.TransientError
            }
        }

    override fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    ) {
        GoogleAccountManagerAuth.recordExpiry(activity, accountId, expiresAtEpochSeconds)
    }
}
