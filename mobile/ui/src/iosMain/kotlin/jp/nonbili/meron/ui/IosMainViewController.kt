package jp.nonbili.meron.ui

import androidx.compose.foundation.ComposeFoundationFlags
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.ui.window.ComposeUIViewController
import jp.nonbili.meron.shared.MeronCore
import kotlinx.cinterop.ExperimentalForeignApi
import platform.AuthenticationServices.ASPresentationAnchor
import platform.AuthenticationServices.ASWebAuthenticationPresentationContextProvidingProtocol
import platform.AuthenticationServices.ASWebAuthenticationSession
import platform.Foundation.NSApplicationSupportDirectory
import platform.Foundation.NSBundle
import platform.Foundation.NSError
import platform.Foundation.NSFileManager
import platform.Foundation.NSNotificationCenter
import platform.Foundation.NSOperationQueue
import platform.Foundation.NSString
import platform.Foundation.NSURL
import platform.Foundation.NSUTF8StringEncoding
import platform.Foundation.NSUserDomainMask
import platform.Foundation.stringWithContentsOfFile
import platform.UIKit.UIActivityViewController
import platform.UIKit.UIApplication
import platform.UIKit.UIApplicationOpenSettingsURLString
import platform.UIKit.UIApplicationWillEnterForegroundNotification
import platform.UIKit.UIModalPresentationFullScreen
import platform.UIKit.UIViewController
import platform.UIKit.UIWindow
import platform.UserNotifications.UNAuthorizationOptionAlert
import platform.UserNotifications.UNAuthorizationOptionBadge
import platform.UserNotifications.UNAuthorizationOptionSound
import platform.UserNotifications.UNAuthorizationStatusAuthorized
import platform.UserNotifications.UNAuthorizationStatusEphemeral
import platform.UserNotifications.UNAuthorizationStatusNotDetermined
import platform.UserNotifications.UNAuthorizationStatusProvisional
import platform.UserNotifications.UNUserNotificationCenter
import platform.darwin.NSObject

@OptIn(ExperimentalFoundationApi::class)
fun MainViewController(
    core: MeronCore,
    coreLoaded: Boolean,
    coreInitJson: String,
    incomingMailtoDraft: jp.nonbili.meron.shared.ComposeDraft? = null,
    incomingOAuthCallbackUrl: String? = null,
    incomingNotificationThreadTarget: NotificationThreadTarget? = null,
    outlookClientId: String = "",
    outlookRedirectUri: String = "",
    googleClientId: String = "",
    googleRedirectUri: String = "",
    coreProtocolVersion: Int = 0,
): UIViewController {
    ComposeFoundationFlags.isNewContextMenuEnabled = true
    val appPrefs = IosAppPreferences("meron_app")
    val kanbanPrefs = IosAppPreferences("meron_kanban")
    val locale = IosLocaleController(appPrefs)
    val host =
        DefaultMobileHost(
            outlookClientId = outlookClientId,
            outlookRedirectUri = outlookRedirectUri,
            googleClientId = googleClientId,
            googleRedirectUri = googleRedirectUri,
        )
    val bundle = NSBundle.mainBundle
    val appVersion = (bundle.objectForInfoDictionaryKey("CFBundleShortVersionString") as? String).orEmpty()
    val packageName = bundle.bundleIdentifier.orEmpty()
    val services = IosPlatformServices()
    return ComposeUIViewController {
        MeronApp(
            core = core,
            coreLoaded = coreLoaded,
            prefs = appPrefs,
            kanbanPrefs = kanbanPrefs,
            services = services,
            locale = locale,
            mobileHost =
                IosMobileHost(
                    delegate = host,
                    packageName = packageName,
                    appVersionName = appVersion,
                    coreProtocolVersion = coreProtocolVersion,
                ),
            coreInitJson = coreInitJson,
            incomingMailtoDraft = incomingMailtoDraft,
            incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
            incomingNotificationThreadTarget = incomingNotificationThreadTarget,
        )
    }
}

private class IosMobileHost(
    private val delegate: MobileHost,
    override val packageName: String,
    override val appVersionName: String,
    override val coreProtocolVersion: Int,
) : MobileHost by delegate {
    // UNUserNotificationCenter reports authorization asynchronously, but the
    // shared UI reads it synchronously — cache the last known status and nudge
    // the composition through NotificationPermissionSignal when it changes.
    private var notificationsGranted = false
    private var authorizationDetermined = false

    init {
        refreshNotificationAuthorization()
        // Re-check when the app returns to the foreground: the user may have
        // toggled notifications in the Settings app.
        NSNotificationCenter.defaultCenter.addObserverForName(
            UIApplicationWillEnterForegroundNotification,
            null,
            NSOperationQueue.mainQueue,
        ) { _ -> refreshNotificationAuthorization() }
    }

    override fun shareDiagnosticLog() {
        val body =
            readSyncDiagnosticLog().ifBlank {
                "No background sync activity recorded yet. Enable diagnostic logging in Settings and try again after the next sync."
            }
        val disclosure =
            "Account emails below are masked to only the first letter and domain (e.g. j***@gmail.com).\n" +
                "Review before sharing.\n\n"
        presentDiagnosticLogShareSheet(disclosure + body)
    }

    override fun notificationsEnabled(): Boolean = notificationsGranted

    override fun requestNotificationPermission() {
        if (authorizationDetermined && !notificationsGranted) {
            // The system dialog only shows once; after a denial the user has to
            // flip the switch in the Settings app.
            val url = NSURL.URLWithString(UIApplicationOpenSettingsURLString) ?: return
            UIApplication.sharedApplication.openURL(url)
            return
        }
        UNUserNotificationCenter.currentNotificationCenter().requestAuthorizationWithOptions(
            UNAuthorizationOptionAlert or UNAuthorizationOptionBadge or UNAuthorizationOptionSound,
        ) { granted, _ ->
            notificationsGranted = granted
            authorizationDetermined = true
            NotificationPermissionSignal.signal()
        }
    }

    private fun refreshNotificationAuthorization() {
        UNUserNotificationCenter.currentNotificationCenter().getNotificationSettingsWithCompletionHandler { settings ->
            val status = settings?.authorizationStatus ?: return@getNotificationSettingsWithCompletionHandler
            val granted =
                status == UNAuthorizationStatusAuthorized ||
                    status == UNAuthorizationStatusProvisional ||
                    status == UNAuthorizationStatusEphemeral
            val changed = granted != notificationsGranted
            notificationsGranted = granted
            authorizationDetermined = status != UNAuthorizationStatusNotDetermined
            if (changed) NotificationPermissionSignal.signal()
        }
    }
}

// Mirrors IosAppPaths.mobileDataDirectory() + IosSyncDiagnosticLog's file name
// on the Swift side (both read/write the same app-support-directory file).
@OptIn(ExperimentalForeignApi::class)
private fun readSyncDiagnosticLog(): String {
    val base =
        NSFileManager.defaultManager
            .URLsForDirectory(NSApplicationSupportDirectory, NSUserDomainMask)
            .firstOrNull() as? NSURL ?: return ""
    val path =
        base
            .URLByAppendingPathComponent("Meron", isDirectory = true)
            ?.URLByAppendingPathComponent("sync-diagnostic.log")
            ?.path ?: return ""
    return NSString.stringWithContentsOfFile(path, encoding = NSUTF8StringEncoding, error = null) ?: ""
}

@OptIn(ExperimentalForeignApi::class)
private fun presentDiagnosticLogShareSheet(text: String) {
    val rootViewController =
        (UIApplication.sharedApplication.windows.firstOrNull() as? UIWindow)?.rootViewController ?: return
    var presenter = rootViewController
    while (true) {
        presenter = presenter.presentedViewController ?: break
    }
    val activityController = UIActivityViewController(activityItems = listOf(text), applicationActivities = null)
    // Avoids the iPad requirement to set a popover sourceView/sourceRect.
    activityController.modalPresentationStyle = UIModalPresentationFullScreen
    presenter.presentViewController(activityController, animated = true, completion = null)
}

private class IosPlatformServices : PlatformServices {
    private val authPresentationContext = IosAuthPresentationContext()
    private var authSession: ASWebAuthenticationSession? = null

    override fun openUrl(url: String) {
        val nsUrl = NSURL.URLWithString(url) ?: return
        UIApplication.sharedApplication.openURL(nsUrl)
    }

    override fun openOAuthUrl(
        url: String,
        callbackScheme: String,
        onCallback: (String) -> Unit,
        onFailure: (String) -> Unit,
    ) {
        val nsUrl =
            NSURL.URLWithString(url)
                ?: return onFailure("OAuth URL could not be built.")
        val session =
            ASWebAuthenticationSession(
                uRL = nsUrl,
                callbackURLScheme = callbackScheme.ifBlank { null },
            ) { callbackUrl: NSURL?, error: NSError? ->
                authSession = null
                when {
                    callbackUrl != null -> onCallback(callbackUrl.absoluteString.orEmpty())
                    error != null -> onFailure(error.localizedDescription)
                    else -> onFailure("OAuth sign-in was cancelled.")
                }
            }
        session.presentationContextProvider = authPresentationContext
        session.prefersEphemeralWebBrowserSession = false
        authSession = session
        if (!session.start()) {
            authSession = null
            onFailure("OAuth browser launch failed.")
        }
    }

    override fun copyText(
        label: String,
        value: String,
    ) {
        platform.UIKit.UIPasteboard.generalPasteboard.string = value
    }

    override fun copyImage(
        bytes: ByteArray,
        mimeType: String,
        label: String,
    ) {
        shareFile(bytes, "$label.${mimeType.substringAfter('/', "bin")}", mimeType)
    }

    override fun shareFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    ) {
        // TODO: present UIActivityViewController from the Swift host. This keeps
        // the shared UI callable on iOS while the native host owns presentation.
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
        onPicked(null)
    }

    override fun pickImage(onPicked: (PickedFile?) -> Unit) {
        onPicked(null)
    }
}

private class IosAuthPresentationContext :
    NSObject(),
    ASWebAuthenticationPresentationContextProvidingProtocol {
    override fun presentationAnchorForWebAuthenticationSession(session: ASWebAuthenticationSession): ASPresentationAnchor {
        val windows = UIApplication.sharedApplication.windows
        return (windows.firstOrNull() as? UIWindow) ?: UIWindow()
    }
}
