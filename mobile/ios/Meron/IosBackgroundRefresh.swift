import BackgroundTasks
import Foundation
import MeronShared
import UserNotifications

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
        content.title = "Meron refresh complete"
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
