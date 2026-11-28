import MeronUI
import NaturalLanguage
import SwiftUI
import Translation

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

struct IosComposeWindow: View {
    @StateObject private var translation = IosMailTranslationCoordinator()
    let host: (IosMailTranslationCoordinator) -> IosComposeHost

    init(host: @escaping (IosMailTranslationCoordinator) -> IosComposeHost) {
        self.host = host
    }

    var body: some View {
        host(translation)
            .background {
                if #available(iOS 18.0, *) {
                    IosMailTranslationHost(coordinator: translation)
                }
            }
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
    let translationBridge: IosMailTranslationCoordinator

    private var availableTranslationBridge: IosMailTranslationBridge? {
        if #available(iOS 18.0, *) { return translationBridge }
        return nil
    }

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
            coreProtocolVersion: coreProtocolVersion,
            translationBridge: availableTranslationBridge
        )
    }

    func updateUIViewController(_: UIViewController, context _: Context) {}
}

final class IosMailTranslationCoordinator: ObservableObject, IosMailTranslationBridge {
    struct Request {
        let id: UUID
        let text: String
        let targetLanguage: String
        let completion: (MailTranslation?) -> Void
    }

    @Published private(set) var request: Request?

    func languages(completion: @escaping ([MailTranslationLanguage]) -> Void) {
        guard #available(iOS 18.0, *) else {
            completion([])
            return
        }
        Task { @MainActor in
            let languages = await LanguageAvailability().supportedLanguages
            completion(languages.map { language in
                // Apple's minimal Chinese identifiers use regions. Match the app's script-based locales.
                let tag = if language.languageCode?.identifier == "zh", let script = language.script?.identifier {
                    "zh-\(script)"
                } else {
                    language.minimalIdentifier
                }
                let name = Locale(identifier: tag).localizedString(forIdentifier: tag) ?? tag
                return MailTranslationLanguage(tag: tag, name: name)
            })
        }
    }

    func translate(
        text: String,
        targetLanguage: String,
        completion: @escaping (MailTranslation?) -> Void
    ) -> CloseableHandle {
        let id = UUID()
        DispatchQueue.main.async {
            let previous = self.request
            if let source = self.sourceMatchingTarget(text: text, targetLanguage: targetLanguage) {
                self.request = nil
                completion(MailTranslation(text: text, sourceLanguage: source))
            } else {
                self.request = Request(id: id, text: text, targetLanguage: targetLanguage, completion: completion)
            }
            previous?.completion(nil)
        }
        return IosCloseableHandle { [weak self] in
            DispatchQueue.main.async {
                guard self?.request?.id == id else { return }
                self?.request = nil
            }
        }
    }

    private func sourceMatchingTarget(text: String, targetLanguage: String) -> String? {
        let recognizer = NLLanguageRecognizer()
        recognizer.processString(text)
        guard let (tag, confidence) = recognizer.languageHypotheses(withMaximum: 1).first,
              confidence >= 0.5
        else { return nil }
        let source = Locale.Language(identifier: tag.rawValue)
        let target = Locale.Language(identifier: targetLanguage)
        guard source.languageCode == target.languageCode, source.script == target.script else { return nil }
        return tag.rawValue
    }

    @available(iOS 18.0, *)
    @MainActor
    func translate(_ request: Request, using session: TranslationSession) async {
        guard self.request?.id == request.id else { return }
        let result: MailTranslation?
        do {
            let response = try await session.translate(request.text)
            result = MailTranslation(text: response.targetText, sourceLanguage: response.sourceLanguage.minimalIdentifier)
        } catch {
            result = nil
        }
        // The dialog may have closed or switched target language during the download.
        guard self.request?.id == request.id else { return }
        self.request = nil
        request.completion(result)
    }
}

@available(iOS 18.0, *)
struct IosMailTranslationHost: View {
    @ObservedObject var coordinator: IosMailTranslationCoordinator

    var body: some View {
        if let request = coordinator.request {
            Color.clear
                .frame(width: 1, height: 1)
                .accessibilityHidden(true)
                .allowsHitTesting(false)
                .translationTask(
                    source: nil,
                    target: Locale.Language(identifier: request.targetLanguage)
                ) { session in
                    await coordinator.translate(request, using: session)
                }
                .id(request.id)
        }
    }
}
