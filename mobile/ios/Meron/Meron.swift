import MeronUI
import SwiftUI
import UIKit

@main
struct Meron: App {
    /// Hosted unit tests configure their own temporary core profiles. Do not
    /// start the real account Engine or UI against those process-global bridges.
    private static var isRunningTests: Bool {
        ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil
    }

    @UIApplicationDelegateAdaptor(IosNotificationDelegate.self) private var notificationDelegate
    private let incomingMailtoEvents = IncomingMailtoEvents()
    @State private var incomingOAuthCallbackUrl: String?
    private let incomingThreadEvents = IncomingThreadEvents()

    private let core = IosMeronCore()
    private static let engineLifecycleQueue = DispatchQueue(label: "jp.nonbili.meron.engine-lifecycle")

    private let coreLoaded: Bool
    private let coreInitJson: String
    private let coreProtocolVersion: Int32
    private let startupError: String?

    init() {
        if Self.isRunningTests {
            coreLoaded = true
            startupError = nil
            coreInitJson = "{}"
            coreProtocolVersion = Int32(RustCoreBridge.protocolVersion())
            return
        }
        let dataDirectory = IosAppPaths.mobileDataDirectory()
        coreProtocolVersion = Int32(RustCoreBridge.protocolVersion())
        do {
            let key = try IosDbKey.get()
            coreInitJson = RustCoreBridge.configureJson(dataDirectory: dataDirectory, dbKey: key)
            coreLoaded = true
            startupError = nil
        } catch {
            let data = try! JSONSerialization.data(withJSONObject: ["error": ["message": error.localizedDescription]])
            coreInitJson = String(decoding: data, as: UTF8.self)
            coreLoaded = false
            startupError = error.localizedDescription
            return
        }
        IosBackgroundRefresh.register()
        IosBackgroundRefresh.schedule()
        // Host the shared Engine for this (foreground) launch: warm session pool
        // + foreground IMAP IDLE. Paused on background, resumed on return.
        Self.engineLifecycle("engine.foreground")
        IosUnreadWidgets.refresh()
        IosReminders.refresh()
    }

    /// Drive the core's foreground Engine lifecycle (warm pool + IDLE) from the
    /// app's foreground/background transitions.
    /// Serial and off the main thread: the first call waits out any store
    /// upgrade, and iOS kills an app that blocks its launch for long.
    private static func engineLifecycle(_ method: String) {
        let request = #"{"id":92,"method":"\#(method)","params":{}}"#
        var task = UIBackgroundTaskIdentifier.invalid
        task = UIApplication.shared.beginBackgroundTask(withName: "Engine lifecycle") {
            UIApplication.shared.endBackgroundTask(task)
            task = .invalid
        }
        engineLifecycleQueue.async {
            _ = RustCoreBridge.invokeJson(request)
            DispatchQueue.main.async {
                guard task != .invalid else { return }
                UIApplication.shared.endBackgroundTask(task)
                task = .invalid
            }
        }
    }

    var body: some Scene {
        WindowGroup {
            if Self.isRunningTests {
                Color.clear
            } else if let startupError {
                VStack(spacing: 16) {
                    Text("Unable to open database").font(.headline)
                    Text(startupError)
                    Text("Your local data has not been reset.").font(.footnote)
                }
                .padding()
            } else {
                IosComposeHost(
                    core: core,
                    coreLoaded: coreLoaded,
                    coreInitJson: coreInitJson,
                    incomingMailtoEvents: incomingMailtoEvents,
                    incomingOAuthCallbackUrl: incomingOAuthCallbackUrl,
                    incomingThreadEvents: incomingThreadEvents,
                    coreProtocolVersion: coreProtocolVersion
                )
                .ignoresSafeArea()
                .onOpenURL(perform: handleOpenUrl)
                .onReceive(NotificationCenter.default.publisher(for: .iosNotificationThreadTargetOpened)) { notification in
                    // The payload rides in userInfo, keyed exactly as a notification's
                    // own userInfo is, so one parser reads both.
                    guard let target = iosNotificationThreadTarget(userInfo: notification.userInfo ?? [:])
                    else { return }
                    incomingThreadEvents.offer(target: NotificationThreadTarget(
                        accountId: target.accountId,
                        folder: target.folder,
                        threadKey: target.threadKey,
                        nonce: Int64(Date().timeIntervalSince1970 * 1000)
                    ))
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                    // Refresh the visible mailbox right away on return from the
                    // background, instead of waiting out the remaining poll tick.
                    AppForegroundSignal.shared.signal()
                    IosUnreadWidgets.refresh()
                    IosReminders.refresh()
                    Self.engineLifecycle("engine.foreground")
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.significantTimeChangeNotification)) { _ in
                    IosReminders.refresh()
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.didEnterBackgroundNotification)) { _ in
                    // Park the Engine: stop foreground IDLE and drop warm sockets the
                    // OS would freeze anyway.
                    Self.engineLifecycle("engine.background")
                    IosUnreadWidgets.refreshAfterBackgrounding()
                }
            }
        }
    }

    private func handleOpenUrl(_ url: URL) {
        if let target = iosWidgetThreadTarget(url) {
            incomingThreadEvents.offer(target: NotificationThreadTarget(
                accountId: target.accountId, folder: target.folder, threadKey: target.threadKey,
                nonce: Int64(Date().timeIntervalSince1970 * 1000)
            ))
            return
        }
        let rawUrl = url.absoluteString
        if OAuthFlowKt.isPotentialOAuthCallbackUrl(rawUrl: rawUrl) {
            incomingOAuthCallbackUrl = rawUrl
            return
        }
        if let draft = MailtoKt.parseMailtoUrl(rawUrl: rawUrl) {
            incomingMailtoEvents.offer(draft: draft)
        }
    }
}
