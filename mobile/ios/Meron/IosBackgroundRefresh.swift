import BackgroundTasks
import Foundation
import MeronUI
import UIKit
import UserNotifications

extension Notification.Name {
    static let iosNotificationThreadTargetOpened = Notification.Name("iosNotificationThreadTargetOpened")
}

struct IosNotificationThreadTarget: Equatable {
    let accountId: String
    let folder: String
    let threadKey: String
    let subject: String
}

enum IosNotificationPayloadKey {
    static let accountId = "accountId"
    static let folder = "folder"
    static let threadKey = "threadKey"
    static let subject = "subject"
}

func iosNotificationThreadTarget(userInfo: [AnyHashable: Any]) -> IosNotificationThreadTarget? {
    let accountId = (userInfo[IosNotificationPayloadKey.accountId] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let folder = (userInfo[IosNotificationPayloadKey.folder] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let threadKey = (userInfo[IosNotificationPayloadKey.threadKey] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let subject = userInfo[IosNotificationPayloadKey.subject] as? String ?? ""
    guard !accountId.isEmpty, !folder.isEmpty, !threadKey.isEmpty else { return nil }
    return IosNotificationThreadTarget(accountId: accountId, folder: folder, threadKey: threadKey, subject: subject)
}

func iosNotificationThreadId(_ target: IosNotificationThreadTarget) -> String {
    if target.accountId.contains(":") {
        return "\(target.accountId)#rss#\(target.threadKey)"
    }
    let folder = target.folder.caseInsensitiveCompare("inbox") == .orderedSame ? "INBOX" : target.folder
    if target.threadKey.hasPrefix("uid:") {
        return "\(target.accountId)#\(folder)#\(String(target.threadKey.dropFirst(4)))"
    }
    let compoundKey: String
    if target.threadKey.hasPrefix("gmthrid:") {
        compoundKey = target.threadKey
    } else {
        compoundKey = target.threadKey + "#" + iosThreadGroupingSubject(target.subject)
    }
    let encoded = Data(compoundKey.utf8)
        .base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
    return "\(target.accountId)#\(folder)#t.\(encoded)"
}

func iosThreadGroupingSubject(_ subject: String) -> String {
    var value = subject.trimmingCharacters(in: .whitespacesAndNewlines)
    let prefixPattern = #"(?i)^(?:\[[^\]]+\]\s*)*(?:re|fw|fwd|aw|sv|vs|rv|res|tr|antw|wg|答复|回复|转发)(?:\[[0-9]+\]|\([0-9]+\))?[:：]\s*"#
    let tagPattern = #"^\s*\[[^\]]*\]\s*"#
    while true {
        if let range = value.range(of: prefixPattern, options: .regularExpression), range.lowerBound == value.startIndex {
            value = String(value[range.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
            continue
        }
        if let range = value.range(of: tagPattern, options: .regularExpression), range.lowerBound == value.startIndex {
            value = String(value[range.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
            continue
        }
        return value
    }
}

final class IosNotificationDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        return true
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if let target = iosNotificationThreadTarget(userInfo: response.notification.request.content.userInfo) {
            NotificationCenter.default.post(
                name: .iosNotificationThreadTargetOpened,
                object: nil,
                userInfo: [
                    IosNotificationPayloadKey.accountId: target.accountId,
                    IosNotificationPayloadKey.folder: target.folder,
                    IosNotificationPayloadKey.threadKey: target.threadKey,
                    IosNotificationPayloadKey.subject: target.subject,
                ]
            )
        }
        completionHandler()
    }
}

enum IosNotificationService {
    static func requestAuthorization(completion: @escaping (Bool) -> Void) {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound]) { granted, _ in
            DispatchQueue.main.async {
                completion(granted)
            }
        }
    }

    static func refreshNotificationContent(_ body: String) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = String(localized: "mobile.ios.refreshCompleteTitle")
        content.body = body
        content.sound = .default
        return content
    }

    static func notifyRefreshComplete(_ body: String) {
        let request = UNNotificationRequest(
            identifier: "meron-refresh-complete",
            content: refreshNotificationContent(body),
            trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }

    static func notifyNewMailContentForTesting(
        accountName: String,
        from: String,
        subject: String,
        count: Int,
        accountId: String = "",
        folder: String = "",
        threadKey: String = ""
    ) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = accountName.isEmpty ? String(localized: "mobile.ios.newMailTitle") : accountName
        let sender = from.trimmingCharacters(in: .whitespacesAndNewlines)
        let messageSubject = subject.trimmingCharacters(in: .whitespacesAndNewlines)
        if count > 1 {
            content.body = localizedCatalogString("mobile.ios.newMailCountBody", args: ["count": count])
        } else if !sender.isEmpty, !messageSubject.isEmpty {
            content.body = "\(sender): \(messageSubject)"
        } else {
            content.body = messageSubject.isEmpty ? String(localized: "mobile.ios.newMailBodyFallback") : messageSubject
        }
        content.sound = .default
        if !accountId.isEmpty, !folder.isEmpty, !threadKey.isEmpty {
            content.userInfo = [
                IosNotificationPayloadKey.accountId: accountId,
                IosNotificationPayloadKey.folder: folder,
                IosNotificationPayloadKey.threadKey: threadKey,
                IosNotificationPayloadKey.subject: subject,
            ]
        }
        return content
    }

    static func notifyNewMail(
        accountName: String,
        from: String,
        subject: String,
        count: Int,
        accountId: String = "",
        folder: String = "",
        threadKey: String = ""
    ) {
        let content = notifyNewMailContentForTesting(
            accountName: accountName,
            from: from,
            subject: subject,
            count: count,
            accountId: accountId,
            folder: folder,
            threadKey: threadKey
        )
        let request = UNNotificationRequest(
            identifier: "meron-new-mail-\(UUID().uuidString)",
            content: content,
            trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }
}

enum IosLiveMailPush {
    private static let folderId = "inbox"
    private static var watched: Set<String> = []

    static func sync(enabled: Bool, accounts: [AccountSummary], status: @escaping (String) -> Void) {
        if !enabled {
            stopAll()
            status(String(localized: "mobile.ios.liveMailPushDisabled"))
            return
        }

        let active = accounts.filter { account in
            !MailStateKt.accountSummaryIsRss(account: account) && !account.paused && !account.needsReconnect
        }
        let wanted = Set(active.map { watchKey(accountId: $0.id) })
        for key in watched.subtracting(wanted) {
            let accountId = accountId(from: key)
            stopWatch(accountId: accountId)
            watched.remove(key)
        }
        for account in active {
            let key = watchKey(accountId: account.id)
            if watched.insert(key).inserted {
                startWatch(accountId: account.id)
            }
        }
        status(localizedCatalogString("mobile.ios.liveMailPushWatching", args: ["count": watched.count]))
    }

    static func stopAll() {
        for key in watched {
            stopWatch(accountId: accountId(from: key))
        }
        watched.removeAll()
    }

    private static func startWatch(accountId: String) {
        _ = RustCoreBridge.invokeJson(watchRequest(method: "watch.start", accountId: accountId))
    }

    private static func stopWatch(accountId: String) {
        _ = RustCoreBridge.invokeJson(watchRequest(method: "watch.stop", accountId: accountId))
    }

    private static func watchRequest(method: String, accountId: String) -> String {
        let request: [String: Any] = [
            "id": 91,
            "method": method,
            "params": [
                "account_id": accountId,
                "folder": folderId,
            ],
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: request),
              let json = String(data: data, encoding: .utf8)
        else {
            return #"{"id":91,"method":"watch.stop","params":{"account_id":"","folder":"inbox"}}"#
        }
        return json
    }

    private static func watchKey(accountId: String) -> String {
        "\(accountId)\n\(folderId)"
    }

    private static func accountId(from key: String) -> String {
        key.components(separatedBy: "\n").first ?? ""
    }
}

enum IosBackgroundRefresh {
    static let taskIdentifier = "jp.nonbili.meron.refresh"

    @discardableResult
    static func register() -> Bool {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: taskIdentifier, using: nil) { task in
            guard let refreshTask = task as? BGAppRefreshTask else {
                task.setTaskCompleted(success: false)
                return
            }
            handle(task: refreshTask)
        }
    }

    static func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: taskIdentifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        do {
            try BGTaskScheduler.shared.submit(request)
        } catch {
            // The system can reject scheduling on simulators, disabled background refresh,
            // or missing entitlements. Manual foreground refresh remains available.
        }
    }

    static func runOnce(completion: @escaping (String) -> Void) {
        DispatchQueue.global(qos: .background).async {
            let result = refreshNow()
            if result.refreshed > 0 {
                IosNotificationService.notifyRefreshComplete(result.summary)
            }
            DispatchQueue.main.async {
                completion(result.summary)
            }
        }
    }

    private static func handle(task: BGAppRefreshTask) {
        schedule()
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1

        let operation = BlockOperation {
            let result = refreshNow()
            if result.refreshed > 0 {
                IosNotificationService.notifyRefreshComplete(result.summary)
            }
            task.setTaskCompleted(success: true)
        }

        task.expirationHandler = {
            queue.cancelAllOperations()
            task.setTaskCompleted(success: false)
        }
        queue.addOperation(operation)
    }

    private static func refreshNow() -> RefreshResult {
        _ = RustCoreBridge.initJson(dataDirectory: IosAppPaths.mobileDataDirectory(), dbKey: IosDbKey.get())
        let accountList = invoke(id: 1, method: "account.list", params: [:])
        guard let accounts = accountList["result"] as? [String: Any],
              let accountRows = accounts["accounts"] as? [[String: Any]]
        else {
            return RefreshResult(refreshed: 0, skipped: 0, failed: 1)
        }

        var refreshed = 0
        var skipped = 0
        var failed = 0
        for (index, account) in accountRows.enumerated() {
            let accountId = account["id"] as? String ?? ""
            let paused = account["paused"] as? Bool == true
            let needsReconnect = account["needs_reconnect"] as? Bool == true
            guard let request = iosRefreshSyncRequest(
                accountId: accountId,
                engine: account["engine"] as? String ?? "",
                provider: account["provider"] as? String ?? "",
                authType: account["auth_type"] as? String ?? "",
                paused: paused,
                needsReconnect: needsReconnect,
                id: Int64(index + 2)
            ) else {
                skipped += 1
                continue
            }

            let response = invoke(
                id: request.id,
                method: request.method,
                params: request.params
            )
            if response["error"] == nil {
                refreshed += 1
            } else {
                failed += 1
            }
        }

        return RefreshResult(refreshed: refreshed, skipped: skipped, failed: failed)
    }

    private static func invoke(id: Int64, method: String, params: [String: Any]) -> [String: Any] {
        let request: [String: Any] = [
            "id": id,
            "method": method,
            "params": params,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: request),
              let json = String(data: data, encoding: .utf8)
        else {
            return ["error": ["message": "failed to encode request"]]
        }
        let response = RustCoreBridge.invokeJson(json)
        guard let responseData = response.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: responseData),
              let dictionary = object as? [String: Any]
        else {
            return ["error": ["message": "failed to decode response"]]
        }
        return dictionary
    }

    private struct RefreshResult {
        let refreshed: Int
        let skipped: Int
        let failed: Int

        var summary: String {
            BackgroundRefreshKt.backgroundRefreshSummary(
                refreshed: Int32(refreshed),
                skipped: Int32(skipped),
                failed: Int32(failed)
            )
        }
    }
}

struct IosRefreshSyncRequest {
    let id: Int64
    let method: String
    let params: [String: Any]
}

func iosRefreshSyncRequest(
    accountId: String,
    engine: String,
    provider: String,
    authType: String,
    paused: Bool,
    needsReconnect: Bool,
    id: Int64
) -> IosRefreshSyncRequest? {
    guard BackgroundRefreshKt.shouldBackgroundRefreshAccount(
        accountId: accountId,
        paused: paused,
        needsReconnect: needsReconnect
    ) else {
        return nil
    }

    let usesRssProtocol = BackgroundRefreshKt.backgroundRefreshUsesRssProtocol(
        engine: engine,
        provider: provider,
        authType: authType
    )
    return IosRefreshSyncRequest(
        id: id,
        method: usesRssProtocol ? "rss.sync" : "mail.sync",
        params: usesRssProtocol
            ? ["account_id": accountId]
            : [
                "account_id": accountId,
                "folder_id": "inbox",
                "limit": 50,
                "folders": true,
            ]
    )
}
