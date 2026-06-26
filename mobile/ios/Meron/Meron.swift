import SwiftUI
import MeronUI
import UIKit

@main
struct Meron: App {
    @UIApplicationDelegateAdaptor(IosNotificationDelegate.self) private var notificationDelegate
    @State private var incomingMailtoDraft: ComposeDraft?
    @State private var incomingOAuthCallbackUrl: String?
    @State private var incomingNotificationThreadTarget: NotificationThreadTarget?

    private let core = IosMeronCore()
    private let coreLoaded = true
    private let coreInitJson: String
    private let coreProtocolVersion: Int32

    init() {
        let dataDirectory = IosAppPaths.mobileDataDirectory()
        coreInitJson = RustCoreBridge.initJson(dataDirectory: dataDirectory, dbKey: IosDbKey.get())
        coreProtocolVersion = Int32(RustCoreBridge.protocolVersion())
        IosBackgroundRefresh.register()
        IosBackgroundRefresh.schedule()
    }

    var body: some Scene {
        WindowGroup {
            IosComposeHost(
                core: core,
                coreLoaded: coreLoaded,
                coreInitJson: coreInitJson,
                incomingMailtoDraft: incomingMailtoDraft,
                incomingOAuthCallbackUrl: incomingOAuthCallbackUrl,
                incomingNotificationThreadTarget: incomingNotificationThreadTarget,
                coreProtocolVersion: coreProtocolVersion
            )
            .ignoresSafeArea()
            .onOpenURL(perform: handleOpenUrl)
            .onReceive(NotificationCenter.default.publisher(for: .iosNotificationThreadTargetOpened)) { notification in
                guard let target = notification.object as? IosNotificationThreadTarget else { return }
                incomingNotificationThreadTarget =
                    NotificationThreadTarget(
                        accountId: target.accountId,
                        folder: target.folder,
                        threadKey: target.threadKey,
                        nonce: Int64(Date().timeIntervalSince1970 * 1000)
                    )
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                // Refresh the visible mailbox right away on return from the
                // background, instead of waiting out the remaining poll tick.
                AppForegroundSignal.shared.signal()
            }
        }
    }

    private func handleOpenUrl(_ url: URL) {
        let rawUrl = url.absoluteString
        if OAuthFlowKt.isPotentialOAuthCallbackUrl(rawUrl: rawUrl) {
            incomingOAuthCallbackUrl = rawUrl
            return
        }
        if let draft = MailtoKt.parseMailtoUrl(rawUrl: rawUrl) {
            incomingMailtoDraft = draft
        }
    }
}
