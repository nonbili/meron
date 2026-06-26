package jp.nonbili.meron.ui

import androidx.compose.ui.window.ComposeUIViewController
import jp.nonbili.meron.shared.MeronCore
import platform.AuthenticationServices.ASPresentationAnchor
import platform.AuthenticationServices.ASWebAuthenticationPresentationContextProvidingProtocol
import platform.AuthenticationServices.ASWebAuthenticationSession
import platform.Foundation.NSBundle
import platform.Foundation.NSError
import platform.Foundation.NSURL
import platform.darwin.NSObject
import platform.UIKit.UIApplication
import platform.UIKit.UIViewController
import platform.UIKit.UIWindow

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
) : MobileHost by delegate

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
