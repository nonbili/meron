import MeronUI
import SwiftUI

private final class IosCloseableHandle: CloseableHandle {
    private let onClose: () -> Void

    init(_ onClose: @escaping () -> Void) {
        self.onClose = onClose
    }

    func close() {
        onClose()
    }
}

private final class IosCoreEventStream: CoreEventStream {
    func subscribe(listener: @escaping (CoreEvent) -> Void) -> CloseableHandle {
        RustCoreBridge.setEventHandler { eventJson in
            let event = MeronCoreKt.parseCoreEventEnvelope(eventJson: eventJson)
            listener(event)
            if ["ready", "calendar.changed", "tasks.changed", "mail.synced"].contains(event.name) {
                IosReminders.refresh()
            }
            if ["ready", "mail.synced", "mail.newMessages"].contains(event.name) {
                IosUnreadWidgets.refresh()
            }
            // A foreground IDLE watch syncs without going through a command.
            if ["mail.synced", "mail.newMessages"].contains(event.name) {
                IosNewMailReconciler.request()
            }
        }
        _ = meron_core_emit_ready_event()
        return IosCloseableHandle {
            RustCoreBridge.setEventHandler(nil)
        }
    }
}

func iosCoreResponseError(_ response: String) -> NSError? {
    guard let data = response.data(using: .utf8),
          let envelope = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let error = envelope["error"] as? [String: Any],
          let message = error["message"] as? String else { return nil }
    return NSError(domain: "MeronCore", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
}

final class IosMeronCore: MeronCore {
    private let eventStream = IosCoreEventStream()

    func events() -> CoreEventStream {
        eventStream
    }

    func invoke(command: String, payloadJson: String, completionHandler: @escaping (String?, Error?) -> Void) {
        let request = CoreRequest(id: 1, method: command, paramsJson: payloadJson)
        let response = RustCoreBridge.invokeJson(request.toJson())
        if let error = iosCoreResponseError(response) {
            completionHandler(nil, error)
            return
        }
        completionHandler(response, nil)
        if IosUnreadWidgets.commandChangesSnapshot(command) {
            IosUnreadWidgets.refresh()
        }
        if iosCommandMayStaleNotifications(command) {
            IosNewMailReconciler.request()
        }
    }

    func protocolVersion(completionHandler: @escaping (KotlinInt?, Error?) -> Void) {
        completionHandler(KotlinInt(int: Int32(RustCoreBridge.protocolVersion())), nil)
    }
}

struct IosComposeHost: UIViewControllerRepresentable {
    let core: IosMeronCore
    let coreLoaded: Bool
    let coreInitJson: String
    let incomingMailtoEvents: IncomingMailtoEvents
    let incomingOAuthCallbackUrl: String?
    let incomingThreadEvents: IncomingThreadEvents
    let coreProtocolVersion: Int32

    func makeUIViewController(context _: Context) -> UIViewController {
        IosMainViewControllerKt.MainViewController(
            core: core,
            coreLoaded: coreLoaded,
            coreInitJson: coreInitJson,
            incomingMailtoDraft: nil,
            incomingMailtoEvents: incomingMailtoEvents,
            incomingOAuthCallbackUrl: incomingOAuthCallbackUrl,
            incomingNotificationThreadTarget: nil,
            incomingThreadEvents: incomingThreadEvents,
            outlookClientId: Bundle.main.object(forInfoDictionaryKey: "MERON_OUTLOOK_CLIENT_ID") as? String ?? "",
            outlookRedirectUri: Bundle.main.object(forInfoDictionaryKey: "MERON_OUTLOOK_REDIRECT_URI") as? String ?? "",
            googleClientId: Bundle.main.object(forInfoDictionaryKey: "MERON_GOOGLE_CLIENT_ID") as? String ?? "",
            googleRedirectUri: Bundle.main.object(forInfoDictionaryKey: "MERON_GOOGLE_REDIRECT_URI") as? String ?? "",
            coreProtocolVersion: coreProtocolVersion
        )
    }

    func updateUIViewController(_: UIViewController, context _: Context) {}
}
