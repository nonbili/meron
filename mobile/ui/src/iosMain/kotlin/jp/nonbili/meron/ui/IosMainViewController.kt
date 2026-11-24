@file:OptIn(kotlinx.cinterop.BetaInteropApi::class, kotlinx.cinterop.ExperimentalForeignApi::class)

package jp.nonbili.meron.ui

import androidx.compose.foundation.ComposeFoundationFlags
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.ui.window.ComposeUIViewController
import jp.nonbili.meron.shared.MeronCore
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.cstr
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.usePinned
import platform.AuthenticationServices.ASPresentationAnchor
import platform.AuthenticationServices.ASWebAuthenticationPresentationContextProvidingProtocol
import platform.AuthenticationServices.ASWebAuthenticationSession
import platform.Foundation.NSApplicationSupportDirectory
import platform.Foundation.NSBundle
import platform.Foundation.NSData
import platform.Foundation.NSDate
import platform.Foundation.NSDateFormatter
import platform.Foundation.NSError
import platform.Foundation.NSFileManager
import platform.Foundation.NSNotificationCenter
import platform.Foundation.NSOperationQueue
import platform.Foundation.NSString
import platform.Foundation.NSTemporaryDirectory
import platform.Foundation.NSURL
import platform.Foundation.NSUTF8StringEncoding
import platform.Foundation.NSUserDomainMask
import platform.Foundation.create
import platform.Foundation.dataWithContentsOfURL
import platform.Foundation.stringWithContentsOfFile
import platform.Foundation.writeToFile
import platform.UIKit.UIActivityViewController
import platform.UIKit.UIApplication
import platform.UIKit.UIApplicationOpenSettingsURLString
import platform.UIKit.UIApplicationWillEnterForegroundNotification
import platform.UIKit.UIDocumentPickerDelegateProtocol
import platform.UIKit.UIDocumentPickerViewController
import platform.UIKit.UIModalPresentationFullScreen
import platform.UIKit.UIViewController
import platform.UIKit.UIWindow
import platform.UniformTypeIdentifiers.UTType
import platform.UniformTypeIdentifiers.UTTypeItem
import platform.UserNotifications.UNAuthorizationOptionAlert
import platform.UserNotifications.UNAuthorizationOptionBadge
import platform.UserNotifications.UNAuthorizationOptionSound
import platform.UserNotifications.UNAuthorizationStatusAuthorized
import platform.UserNotifications.UNAuthorizationStatusEphemeral
import platform.UserNotifications.UNAuthorizationStatusNotDetermined
import platform.UserNotifications.UNAuthorizationStatusProvisional
import platform.UserNotifications.UNUserNotificationCenter
import platform.darwin.DISPATCH_QUEUE_PRIORITY_DEFAULT
import platform.darwin.NSObject
import platform.darwin.dispatch_async
import platform.darwin.dispatch_get_global_queue
import platform.darwin.dispatch_get_main_queue
import platform.posix.memcpy

@OptIn(ExperimentalFoundationApi::class)
fun MainViewController(
    core: MeronCore,
    coreLoaded: Boolean,
    coreInitJson: String,
    incomingMailtoDraft: jp.nonbili.meron.shared.ComposeDraft? = null,
    incomingMailtoEvents: IncomingMailtoEvents? = null,
    incomingOAuthCallbackUrl: String? = null,
    incomingNotificationThreadTarget: NotificationThreadTarget? = null,
    incomingThreadEvents: IncomingThreadEvents? = null,
    outlookClientId: String = "",
    outlookRedirectUri: String = "",
    googleClientId: String = "",
    googleRedirectUri: String = "",
    coreProtocolVersion: Int = 0,
): UIViewController {
    ComposeFoundationFlags.isNewContextMenuEnabled = true
    val appPrefs = IosAppPreferences("meron_app")
    val kanbanPrefs = IosAppPreferences("meron_kanban")
    val locale = IosLocaleController()
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
    // Built once, outside the composition: MeronApp keys its state on the host,
    // so a new instance per recomposition (a system appearance change, say)
    // would rebuild MeronMobileState and drop everything loaded into it.
    val mobileHost =
        IosMobileHost(
            delegate = host,
            packageName = packageName,
            appVersionName = appVersion,
            coreProtocolVersion = coreProtocolVersion,
        )
    return ComposeUIViewController {
        // An iOS app may not close itself: quitting leaves the store as it was
        // and says so.
        NewerStoreGate(core = core, coreLoaded = coreLoaded, prefs = appPrefs, locale = locale, onQuit = null) {
            MeronApp(
                core = core,
                coreLoaded = coreLoaded,
                prefs = appPrefs,
                kanbanPrefs = kanbanPrefs,
                services = services,
                locale = locale,
                mobileHost = mobileHost,
                coreInitJson = coreInitJson,
                incomingMailtoDraft = incomingMailtoEvents?.draft ?: incomingMailtoDraft,
                onMailtoDraftConsumed = { incomingMailtoEvents?.consume() },
                incomingOAuthCallbackUrl = incomingOAuthCallbackUrl,
                incomingNotificationThreadTarget = incomingThreadEvents?.target ?: incomingNotificationThreadTarget,
                onNotificationThreadTargetConsumed = { incomingThreadEvents?.consume() },
            )
        }
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
        Log.diagnosticSink = { line -> appendIosDiagnosticLog(line) }
        refreshNotificationAuthorization()
        // Re-check when the app returns to the foreground: the user may have
        // toggled notifications in the Settings app.
        NSNotificationCenter.defaultCenter.addObserverForName(
            UIApplicationWillEnterForegroundNotification,
            null,
            NSOperationQueue.mainQueue,
        ) { _ -> refreshNotificationAuthorization() }
    }

    override fun readDiagnosticLog(): String = readSyncDiagnosticLog()

    override fun shareDiagnosticLog() {
        val body =
            readSyncDiagnosticLog().ifBlank {
                "No log entries recorded yet. Try again after the next sync."
            }
        val disclosure =
            "Account emails below are masked to only the first letter and domain (e.g. j***@gmail.com).\n" +
                "Review before sharing.\n\n"
        presentDiagnosticLogShareSheet(disclosure + body)
    }

    override fun shareText(text: String) {
        presentShareSheet(listOf(text))
    }

    override fun pendingCrashReport(): String = readIosCrashMarker()

    override fun clearPendingCrashReport() {
        clearIosCrashMarker()
    }

    override fun noteCoreCrash(summary: String) {
        writeIosCrashMarker(summary)
    }

    override fun notificationsEnabled(): Boolean = notificationsGranted

    override fun requestNotificationPermission() {
        if (authorizationDetermined && !notificationsGranted) {
            // The system dialog only shows once; after a denial the user has to
            // flip the switch in the Settings app.
            val url = NSURL.URLWithString(UIApplicationOpenSettingsURLString) ?: return
            UIApplication.sharedApplication.openURL(url, options = emptyMap<Any?, Any?>(), completionHandler = null)
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
    val path = iosDiagnosticPath("sync-diagnostic.log") ?: return ""
    return NSString.stringWithContentsOfFile(path, encoding = NSUTF8StringEncoding, error = null) ?: ""
}

private val iosEmailRegex = Regex("[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}")

private fun redactIosLogMessage(message: String): String =
    iosEmailRegex.replace(message) { match ->
        val email = match.value
        val at = email.indexOf('@')
        if (at <= 0) "***" else "${email.first()}***${email.substring(at)}"
    }

private fun iosDiagnosticPath(fileName: String): String? {
    val base =
        NSFileManager.defaultManager
            .URLsForDirectory(NSApplicationSupportDirectory, NSUserDomainMask)
            .firstOrNull() as? NSURL ?: return null
    return base
        .URLByAppendingPathComponent("Meron", isDirectory = true)
        ?.URLByAppendingPathComponent(fileName)
        ?.path
}

private fun diagnosticTimestamp(): String {
    val formatter = NSDateFormatter()
    formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
    return formatter.stringFromDate(NSDate())
}

private fun appendIosDiagnosticLog(message: String) {
    val path = iosDiagnosticPath("sync-diagnostic.log") ?: return
    val existing = NSString.stringWithContentsOfFile(path, encoding = NSUTF8StringEncoding, error = null).orEmpty()
    val lines =
        (
            existing.trimEnd().lines().filter { it.isNotEmpty() } +
                "${diagnosticTimestamp()} ${redactIosLogMessage(message)}"
        ).takeLast(500)
    writeIosTextFile(path, lines.joinToString("\n") + "\n")
}

private fun writeIosCrashMarker(summary: String) {
    val path = iosDiagnosticPath("pending-crash.txt") ?: return
    writeIosTextFile(path, "${diagnosticTimestamp()} ${redactIosLogMessage(summary)}\n")
}

private fun readIosCrashMarker(): String {
    val path = iosDiagnosticPath("pending-crash.txt") ?: return ""
    return NSString.stringWithContentsOfFile(path, encoding = NSUTF8StringEncoding, error = null)?.trim().orEmpty()
}

private fun clearIosCrashMarker() {
    val path = iosDiagnosticPath("pending-crash.txt") ?: return
    NSFileManager.defaultManager.removeItemAtPath(path, error = null)
}

private fun writeIosTextFile(
    path: String,
    contents: String,
) {
    val nsContents =
        memScoped {
            NSString.create(cString = contents.cstr.ptr, encoding = NSUTF8StringEncoding)
        } ?: return
    nsContents.writeToFile(
        path,
        atomically = true,
        encoding = NSUTF8StringEncoding,
        error = null,
    )
}

@OptIn(ExperimentalForeignApi::class)
private fun presentDiagnosticLogShareSheet(text: String) = presentShareSheet(listOf(text))

/** The view controller on top, to present a sheet over. */
@OptIn(ExperimentalForeignApi::class)
private fun topViewController(): UIViewController? {
    var presenter =
        (UIApplication.sharedApplication.windows.firstOrNull() as? UIWindow)?.rootViewController ?: return null
    while (true) {
        presenter = presenter.presentedViewController ?: break
    }
    return presenter
}

/** Present the share sheet; [onComplete] hears whether the user finished
 *  an action (saved the file, say) or dismissed it. */
private fun presentShareSheet(
    items: List<Any>,
    onComplete: ((Boolean) -> Unit)? = null,
) {
    val presenter =
        topViewController() ?: run {
            onComplete?.invoke(false)
            return
        }
    val activityController = UIActivityViewController(activityItems = items, applicationActivities = null)
    // Avoids the iPad requirement to set a popover sourceView/sourceRect.
    activityController.modalPresentationStyle = UIModalPresentationFullScreen
    if (onComplete != null) {
        activityController.completionWithItemsHandler = { _, completed, _, _ -> onComplete(completed) }
    }
    presenter.presentViewController(activityController, animated = true, completion = null)
}

/** Write [bytes] to a temporary file to share, or null when that fails. */
@OptIn(ExperimentalForeignApi::class)
private fun stageShareFile(
    bytes: ByteArray,
    fileName: String,
): NSURL? {
    val safeName = fileName.replace('/', '_').ifBlank { "attachment" }
    val dir = NSTemporaryDirectory() + "meron-share/"
    NSFileManager.defaultManager.createDirectoryAtPath(dir, withIntermediateDirectories = true, attributes = null, error = null)
    val path = dir + safeName
    val data =
        bytes.usePinned { pinned ->
            NSData.create(bytes = if (bytes.isNotEmpty()) pinned.addressOf(0) else null, length = bytes.size.toULong())
        }
    if (!data.writeToFile(path, atomically = true)) return null
    return NSURL.fileURLWithPath(path)
}

/** The picked document's bytes, read from the copy the picker made. */
@OptIn(ExperimentalForeignApi::class)
private fun readPickedFile(url: NSURL): PickedFile? {
    val data = NSData.dataWithContentsOfURL(url) ?: return null
    val length = data.length.toInt()
    val bytes = ByteArray(length)
    if (length > 0) bytes.usePinned { memcpy(it.addressOf(0), data.bytes, data.length) }
    return PickedFile(name = url.lastPathComponent ?: "file", bytes = bytes, mimeType = "application/octet-stream")
}

/** The document picker's answer, held while it is on screen (UIKit keeps
 *  only a weak reference to its delegate). */
private class DocumentPickerDelegate(
    private val onPicked: (PickedFile?) -> Unit,
    private val onDone: () -> Unit,
) : NSObject(),
    UIDocumentPickerDelegateProtocol {
    override fun documentPicker(
        controller: UIDocumentPickerViewController,
        didPickDocumentsAtURLs: List<*>,
    ) {
        onDone()
        val url = didPickDocumentsAtURLs.firstOrNull() as? NSURL
        // A large file is read off the main thread, and answered back on it.
        dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT.toLong(), 0u)) {
            val picked = url?.let(::readPickedFile)
            dispatch_async(dispatch_get_main_queue()) { onPicked(picked) }
        }
    }

    override fun documentPickerWasCancelled(controller: UIDocumentPickerViewController) {
        onDone()
        onPicked(null)
    }
}

private class IosPlatformServices : PlatformServices {
    private val authPresentationContext = IosAuthPresentationContext()
    private var authSession: ASWebAuthenticationSession? = null

    override fun openUrl(url: String) {
        val nsUrl = NSURL.URLWithString(url) ?: return
        // The options form: since iOS 18 the deprecated one-argument openURL(_:)
        // opens nothing and returns false, which left every link in a message
        // dead to the tap.
        UIApplication.sharedApplication.openURL(nsUrl, options = emptyMap<Any?, Any?>(), completionHandler = null)
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
        val url = stageShareFile(bytes, fileName) ?: return
        presentShareSheet(listOf(url))
    }

    override fun saveFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
    ) {
        shareFile(bytes, fileName, mimeType)
    }

    /** Saving goes through the share sheet ("Save to Files"): done only
     *  once the user finished there, and failed when the file could not be
     *  staged. */
    override fun saveFile(
        bytes: ByteArray,
        fileName: String,
        mimeType: String,
        onComplete: (Result<Boolean>) -> Unit,
    ) {
        val url =
            stageShareFile(bytes, fileName) ?: run {
                onComplete(Result.failure(IllegalStateException("Could not write $fileName")))
                return
            }
        presentShareSheet(listOf(url)) { completed -> onComplete(Result.success(completed)) }
    }

    private var documentPickerDelegate: DocumentPickerDelegate? = null

    override fun pickFile(
        mimeTypes: List<String>,
        onPicked: (PickedFile?) -> Unit,
    ) {
        val presenter =
            topViewController() ?: run {
                onPicked(null)
                return
            }
        // The asked-for types, or anything for a wildcard.
        val types =
            if (mimeTypes.isEmpty() || mimeTypes.any { it.endsWith("/*") }) {
                listOf(UTTypeItem)
            } else {
                mimeTypes.mapNotNull { UTType.typeWithMIMEType(it) }.ifEmpty { listOf(UTTypeItem) }
            }
        val picker = UIDocumentPickerViewController(forOpeningContentTypes = types, asCopy = true)
        val delegate = DocumentPickerDelegate(onPicked) { documentPickerDelegate = null }
        documentPickerDelegate = delegate
        picker.delegate = delegate
        presenter.presentViewController(picker, animated = true, completion = null)
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
