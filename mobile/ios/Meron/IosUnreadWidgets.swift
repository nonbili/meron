import Foundation
import MeronUI
import UIKit
import WidgetKit

enum IosUnreadWidgets {
    private static let queue = DispatchQueue(label: "jp.nonbili.meron.widget-snapshot", qos: .utility)
    private static var pending: DispatchWorkItem?
    private static var lastReload = Date.distantPast

    static func commandChangesSnapshot(_ command: String) -> Bool {
        // Treat new account operations conservatively; only known read-only calls skip publication.
        if command.hasPrefix("account.") {
            return !["account.list", "account.autodiscover", "account.probeCert"].contains(command)
        }
        return [
            "mail.sync", "mail.markRead", "mail.markAllRead", "mail.archive", "mail.delete",
            "mail.move", "mail.copy", "mail.emptyFolder", "mail.folderDelete",
            "rss.sync", "rss.markRead", "feed.add", "feed.remove", "feed.move",
            "rss.importOpml", "backup.import", "storage.clearCache",
        ].contains(command)
    }

    static func refresh() {
        queue.async {
            pending?.cancel()
            let work = DispatchWorkItem { publish() }
            pending = work
            queue.asyncAfter(deadline: .now() + 0.5, execute: work)
        }
    }

    /// Keep the app alive for a best-effort publication without blocking UIKit.
    static func refreshAfterBackgrounding() {
        let publication = BackgroundPublication()
        let work = DispatchWorkItem {
            pending?.cancel()
            pending = nil
            publish()
            DispatchQueue.main.async { publication.finish() }
        }
        publication.identifier = UIApplication.shared.beginBackgroundTask(withName: "Unread widgets") {
            work.cancel()
            publication.finish()
        }
        queue.async(execute: work)
    }

    private final class BackgroundPublication {
        /// Only accessed on the main queue, including the expiration handler.
        var identifier: UIBackgroundTaskIdentifier = .invalid

        func finish() {
            guard identifier != .invalid else { return }
            UIApplication.shared.endBackgroundTask(identifier)
            identifier = .invalid
        }
    }

    /// Background tasks publish before reporting completion to iOS.
    static func refreshBeforeCompletion() {
        queue.sync {
            pending?.cancel()
            pending = nil
            publish()
        }
    }

    private static func publish() {
        guard UnreadWidgetStore.fileURL != nil,
              let snapshot = UnreadWidgetSnapshotBuilder.build(invoke: { method, params in
                  let request: [String: Any] = ["id": 93, "method": method, "params": params]
                  guard let data = try? JSONSerialization.data(withJSONObject: request),
                        let json = String(data: data, encoding: .utf8),
                        let responseData = RustCoreBridge.invokeJson(json).data(using: .utf8),
                        let response = try? JSONSerialization.jsonObject(with: responseData) as? [String: Any],
                        response["error"] == nil else { return nil }
                  return response["result"] as? [String: Any]
              }) else { return }
        do {
            let previous = UnreadWidgetStore.read()
            try UnreadWidgetStore.write(snapshot)
            // Refresh timestamps too, but avoid spending reload budget on every read.
            if previous?.scopes != snapshot.scopes || snapshot.updatedAt.timeIntervalSince(lastReload) > 300 {
                lastReload = snapshot.updatedAt
                WidgetCenter.shared.reloadTimelines(ofKind: "MeronUnreadCount")
                WidgetCenter.shared.reloadTimelines(ofKind: "MeronUnreadList")
            }
        } catch {
            IosSyncDiagnosticLog.append("widget snapshot write failed")
        }
    }
}

struct IosWidgetThreadTarget: Equatable {
    let accountId: String
    let folder: String
    let threadKey: String
}

func iosWidgetThreadTarget(_ url: URL) -> IosWidgetThreadTarget? {
    guard url.scheme == "meron", url.host == "thread",
          let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
          let id = components.queryItems?.first(where: { $0.name == "id" })?.value,
          let target = NotificationThreadIdsKt.parseNotificationThreadId(threadId: id) else { return nil }
    return IosWidgetThreadTarget(accountId: target.accountId, folder: target.folder, threadKey: target.threadKey)
}
