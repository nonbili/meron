package jp.nonbili.meron

import android.Manifest
import android.accounts.AccountManager
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
        val uri = writeCacheFile(bytes, fileName)
        val intent =
            Intent(Intent.ACTION_SEND)
                .setType(mimeType)
                .putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        activity.startActivity(Intent.createChooser(intent, fileName))
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

    private fun writeCacheFile(
        bytes: ByteArray,
        fileName: String,
    ): Uri {
        val dir = File(activity.cacheDir, "attachments").apply { mkdirs() }
        val file = File(dir, fileName.ifBlank { "meron-file" })
        file.writeBytes(bytes)
        return FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
    }
}

class AndroidMobileHost(
    private val activity: ComponentActivity,
) : MobileHost {
    private companion object {
        const val GOOGLE_AUTH_LOG_TAG = "Meron.GoogleAuth"
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var pendingGoogleAccountResult: ((GoogleDeviceAccount?) -> Unit)? = null
    private val googleAccountPicker =
        activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            val callback = pendingGoogleAccountResult
            pendingGoogleAccountResult = null
            val accountName = result.data?.getStringExtra(AccountManager.KEY_ACCOUNT_NAME)
            if (result.resultCode != Activity.RESULT_OK || accountName.isNullOrBlank()) {
                lastGoogleDeviceAuthError = "Google account picker did not return an account."
                Log.i(
                    GOOGLE_AUTH_LOG_TAG,
                    "Google account picker returned no account; resultCode=${result.resultCode} accountPresent=${!accountName.isNullOrBlank()}",
                )
                callback?.invoke(null)
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
                    callback?.invoke(it)
                }.onFailure {
                    lastGoogleDeviceAuthError =
                        it.message?.takeIf { message -> message.isNotBlank() }
                            ?: it::class.simpleName
                            ?: "Google AccountManager token request failed."
                    Log.w(GOOGLE_AUTH_LOG_TAG, "Google AccountManager token request failed; falling back to browser", it)
                    callback?.invoke(null)
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

    override fun notificationsEnabled(): Boolean =
        Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == android.content.pm.PackageManager.PERMISSION_GRANTED

    override fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 4001)
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

    override fun connectGoogleDeviceAccount(onResult: (GoogleDeviceAccount?) -> Unit) {
        lastGoogleDeviceAuthError = ""
        Log.i(GOOGLE_AUTH_LOG_TAG, "Launching Google account picker")
        pendingGoogleAccountResult = onResult
        googleAccountPicker.launch(GoogleAccountManagerAuth.chooseAccountIntent())
    }

    override suspend fun refreshManagedGoogleToken(accountId: String): ManagedTokenRefresh =
        when (val refresh = GoogleAccountManagerAuth.mintIfNeeded(activity, accountId)) {
            GoogleAccountManagerAuth.TokenRefresh.NotNeeded -> {
                ManagedTokenRefresh.NotNeeded
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
        }

    override fun recordManagedGoogleExpiry(
        accountId: String,
        expiresAtEpochSeconds: Long,
    ) {
        GoogleAccountManagerAuth.recordExpiry(activity, accountId, expiresAtEpochSeconds)
    }
}
