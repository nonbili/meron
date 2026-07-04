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
            listener(MeronCoreKt.parseCoreEventEnvelope(eventJson: eventJson))
        }
        _ = meron_core_emit_ready_event()
        return IosCloseableHandle {
            RustCoreBridge.setEventHandler(nil)
        }
    }
}

final class IosMeronCore: MeronCore {
    private let eventStream = IosCoreEventStream()

    func events() -> CoreEventStream {
        eventStream
    }

    func invoke(command: String, payloadJson: String, completionHandler: @escaping (String?, Error?) -> Void) {
        let request = CoreRequest(id: 1, method: command, paramsJson: payloadJson)
        completionHandler(RustCoreBridge.invokeJson(request.toJson()), nil)
    }

    func protocolVersion(completionHandler: @escaping (KotlinInt?, Error?) -> Void) {
        completionHandler(KotlinInt(int: Int32(RustCoreBridge.protocolVersion())), nil)
    }
}

struct IosComposeHost: UIViewControllerRepresentable {
    let core: IosMeronCore
    let coreLoaded: Bool
    let coreInitJson: String
    let incomingMailtoDraft: ComposeDraft?
    let incomingOAuthCallbackUrl: String?
    let incomingNotificationThreadTarget: NotificationThreadTarget?
    let coreProtocolVersion: Int32

    func makeUIViewController(context _: Context) -> UIViewController {
        IosMainViewControllerKt.MainViewController(
            core: core,
            coreLoaded: coreLoaded,
            coreInitJson: coreInitJson,
            incomingMailtoDraft: incomingMailtoDraft,
            incomingOAuthCallbackUrl: incomingOAuthCallbackUrl,
            incomingNotificationThreadTarget: incomingNotificationThreadTarget,
            outlookClientId: Bundle.main.object(forInfoDictionaryKey: "MERON_OUTLOOK_CLIENT_ID") as? String ?? "",
            outlookRedirectUri: Bundle.main.object(forInfoDictionaryKey: "MERON_OUTLOOK_REDIRECT_URI") as? String ?? "",
            googleClientId: Bundle.main.object(forInfoDictionaryKey: "MERON_GOOGLE_CLIENT_ID") as? String ?? "",
            googleRedirectUri: Bundle.main.object(forInfoDictionaryKey: "MERON_GOOGLE_REDIRECT_URI") as? String ?? "",
            coreProtocolVersion: coreProtocolVersion
        )
    }

    func updateUIViewController(_: UIViewController, context _: Context) {}
}
