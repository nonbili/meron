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
    /// The core store rows the notification announced, as `[folder, message
    /// id]` pairs; see [IosNewMailReconciler].
    static let storeMessages = "storeMessages"
}

func iosNotificationThreadTarget(userInfo: [AnyHashable: Any]) -> IosNotificationThreadTarget? {
    let accountId = (userInfo[IosNotificationPayloadKey.accountId] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let folder = (userInfo[IosNotificationPayloadKey.folder] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let threadKey = (userInfo[IosNotificationPayloadKey.threadKey] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let subject = userInfo[IosNotificationPayloadKey.subject] as? String ?? ""
    guard !accountId.isEmpty, !folder.isEmpty, !threadKey.isEmpty else { return nil }
    return IosNotificationThreadTarget(accountId: accountId, folder: folder, threadKey: threadKey, subject: subject)
}

/// The payload a tapped notification is republished with, keyed the same way a
/// notification's own userInfo is so [iosNotificationThreadTarget] reads both.
func iosNotificationThreadTargetUserInfo(_ target: IosNotificationThreadTarget) -> [String: Any] {
    [
        IosNotificationPayloadKey.accountId: target.accountId,
        IosNotificationPayloadKey.folder: target.folder,
        IosNotificationPayloadKey.threadKey: target.threadKey,
        IosNotificationPayloadKey.subject: target.subject,
    ]
}

func iosNotificationThreadId(_ target: IosNotificationThreadTarget) -> String {
    // A feed is a thread, keyed by its subscription rather than by folder. Core
    // mints RSS account ids as "rss-<uuid>", the same prefix the desktop bridge
    // and the Android client route on.
    if target.accountId.hasPrefix("rss-") {
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
        _: UIApplication,
        didFinishLaunchingWithOptions _: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        UNUserNotificationCenter.current().delegate = self
        return true
    }

    func userNotificationCenter(
        _: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if let target = iosNotificationThreadTarget(userInfo: response.notification.request.content.userInfo) {
            NotificationCenter.default.post(
                name: .iosNotificationThreadTargetOpened,
                object: nil,
                userInfo: iosNotificationThreadTargetUserInfo(target)
            )
        }
        IosReminders.refresh()
        completionHandler()
    }

    func userNotificationCenter(
        _: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        let isReminder = notification.request.identifier.hasPrefix("meron-reminder:")
        if isReminder { IosReminders.refresh() }
        completionHandler(isReminder ? [.banner, .sound] : [])
    }
}

enum IosNotificationService {
    static func requestAuthorization(completion: @escaping (Bool) -> Void) {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .badge, .sound]) { granted, _ in
            DispatchQueue.main.async {
                if granted { IosReminders.refresh() }
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
        threadKey: String = "",
        storeMessages: [IosStoreMessage] = []
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
        var userInfo: [String: Any] = [:]
        if !accountId.isEmpty, !folder.isEmpty, !threadKey.isEmpty {
            userInfo[IosNotificationPayloadKey.folder] = folder
            userInfo[IosNotificationPayloadKey.threadKey] = threadKey
            userInfo[IosNotificationPayloadKey.subject] = subject
        }
        if !accountId.isEmpty, !storeMessages.isEmpty {
            userInfo[IosNotificationPayloadKey.storeMessages] = storeMessages.map { [$0.folder, $0.messageId] }
        }
        if !userInfo.isEmpty {
            userInfo[IosNotificationPayloadKey.accountId] = accountId
            content.userInfo = userInfo
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
        threadKey: String = "",
        storeMessages: [IosStoreMessage] = []
    ) {
        let content = notifyNewMailContentForTesting(
            accountName: accountName,
            from: from,
            subject: subject,
            count: count,
            accountId: accountId,
            folder: folder,
            threadKey: threadKey,
            storeMessages: storeMessages
        )
        let request = UNNotificationRequest(
            identifier: "\(iosNewMailNotificationPrefix)\(UUID().uuidString)",
            content: content,
            trigger: nil
        )
        UNUserNotificationCenter.current().add(request)
    }
}

let iosNewMailNotificationPrefix = "meron-new-mail-"

/// A core store row: mail by folder and UID, a feed entry by its subscription
/// and entry key — what `mail.stillUnread` looks up.
struct IosStoreMessage: Hashable {
    let folder: String
    let messageId: String
}

struct IosStoreRow: Hashable {
    let accountId: String
    let message: IosStoreMessage
}

/// A delivered new-mail notification and the messages it announced.
struct IosTrackedNotification: Equatable {
    let identifier: String
    let accountId: String
    let messages: [IosStoreMessage]
}

/// Read a delivered notification back, or nil for one that is not new mail or
/// predates `storeMessages` — those are left to the user.
func iosTrackedNotification(identifier: String, userInfo: [AnyHashable: Any]) -> IosTrackedNotification? {
    guard identifier.hasPrefix(iosNewMailNotificationPrefix),
          let accountId = userInfo[IosNotificationPayloadKey.accountId] as? String, !accountId.isEmpty,
          let pairs = userInfo[IosNotificationPayloadKey.storeMessages] as? [[String]]
    else { return nil }
    let messages = pairs.compactMap { pair in
        pair.count == 2 ? IosStoreMessage(folder: pair[0], messageId: pair[1]) : nil
    }
    guard !messages.isEmpty else { return nil }
    return IosTrackedNotification(identifier: identifier, accountId: accountId, messages: messages)
}

/// The notifications none of whose messages is still unread. One notification
/// covers a whole batch, so it stays while any arrival in it is unread.
func iosStaleNotificationIds(_ tracked: [IosTrackedNotification], stillUnread: Set<IosStoreRow>) -> [String] {
    tracked
        .filter { notification in
            !notification.messages.contains { stillUnread.contains(IosStoreRow(accountId: notification.accountId, message: $0)) }
        }
        .map(\.identifier)
}

/// Commands after which a message may no longer be unread, or no longer there:
/// a sync that brought in another device's changes, or the app's own read,
/// move and delete writes. Matches the Android client's list.
func iosCommandMayStaleNotifications(_ command: String) -> Bool {
    [
        "mail.sync", "mail.markRead", "mail.markAllRead", "mail.archive", "mail.delete",
        "mail.move", "mail.emptyFolder", "mail.folderDelete", "account.remove", "rss.sync", "rss.markRead",
        "feed.remove",
    ].contains(command)
}

/// Withdraws new-mail notifications whose mail is no longer unread, the way
/// Gmail does: read, archived or deleted here, or — once a sync has brought the
/// change in — on another device. The delivered notifications are compared
/// against core's store rather than tracking each action.
enum IosNewMailReconciler {
    private static let queue = DispatchQueue(label: "jp.nonbili.meron.new-mail-reconcile", qos: .utility)
    private static var pending: DispatchWorkItem?

    /// Coalesces a burst of syncs and writes into one pass.
    static func request() {
        queue.async {
            pending?.cancel()
            let work = DispatchWorkItem { reconcile() }
            pending = work
            queue.asyncAfter(deadline: .now() + 0.5, execute: work)
        }
    }

    /// Background tasks reconcile before reporting completion to iOS.
    static func reconcileNow() {
        queue.sync {
            pending?.cancel()
            pending = nil
            reconcile()
        }
    }

    private static func reconcile() {
        let center = UNUserNotificationCenter.current()
        let delivered = DispatchSemaphore(value: 0)
        var tracked: [IosTrackedNotification] = []
        center.getDeliveredNotifications { notifications in
            tracked = notifications.compactMap {
                iosTrackedNotification(identifier: $0.request.identifier, userInfo: $0.request.content.userInfo)
            }
            delivered.signal()
        }
        guard delivered.wait(timeout: .now() + 5) == .success, !tracked.isEmpty,
              let stillUnread = fetchStillUnread(tracked)
        else { return }
        let stale = iosStaleNotificationIds(tracked, stillUnread: stillUnread)
        if !stale.isEmpty {
            center.removeDeliveredNotifications(withIdentifiers: stale)
        }
    }

    /// The rows core still holds unread, or nil when it could not say — in
    /// which case nothing is withdrawn.
    private static func fetchStillUnread(_ tracked: [IosTrackedNotification]) -> Set<IosStoreRow>? {
        let messages = tracked.flatMap { notification in
            notification.messages.map {
                ["account_id": notification.accountId, "folder_id": $0.folder, "message_id": $0.messageId]
            }
        }
        let request: [String: Any] = ["id": 94, "method": "mail.stillUnread", "params": ["messages": messages]]
        guard let data = try? JSONSerialization.data(withJSONObject: request),
              let json = String(data: data, encoding: .utf8),
              let responseData = RustCoreBridge.invokeJson(json).data(using: .utf8),
              let response = try? JSONSerialization.jsonObject(with: responseData) as? [String: Any],
              let result = response["result"] as? [String: Any],
              let unread = result["unread"] as? [[String: Any]]
        else { return nil }
        return Set(unread.map {
            IosStoreRow(
                accountId: $0["account_id"] as? String ?? "",
                message: IosStoreMessage(folder: $0["folder_id"] as? String ?? "", messageId: $0["message_id"] as? String ?? "")
            )
        })
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

    // Mirrors IosAppPreferences("meron_app") + POLL_INTERVAL_MINUTES_PREF so the
    // best-effort background task honors the user's chosen poll interval.
    private static let pollIntervalDefaultsKey = "meron_app.poll_interval_minutes_v1"
    private static let backgroundSyncDefaultsKey = "meron_app.background_sync_enabled_v1"

    private static var backgroundSyncEnabled: Bool {
        let defaults = UserDefaults.standard
        if defaults.object(forKey: backgroundSyncDefaultsKey) == nil {
            return true
        }
        return defaults.bool(forKey: backgroundSyncDefaultsKey)
    }

    static func schedule() {
        guard backgroundSyncEnabled else {
            BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: taskIdentifier)
            return
        }
        let storedMinutes = Int32(UserDefaults.standard.integer(forKey: pollIntervalDefaultsKey))
        let request = BGAppRefreshTaskRequest(identifier: taskIdentifier)
        request.earliestBeginDate = Date(
            timeIntervalSinceNow: BackgroundRefreshKt.backgroundRefreshDelaySeconds(pollIntervalMinutes: storedMinutes)
        )
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
            IosUnreadWidgets.refreshBeforeCompletion()
            IosNewMailReconciler.reconcileNow()
            if result.refreshed > 0 {
                IosNotificationService.notifyRefreshComplete(result.summary)
            }
            DispatchQueue.main.async {
                completion(result.summary)
            }
        }
    }

    private static func handle(task: BGAppRefreshTask) {
        guard backgroundSyncEnabled else {
            task.setTaskCompleted(success: true)
            return
        }
        schedule()
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1

        let operation = BlockOperation {
            let result = refreshNow()
            IosUnreadWidgets.refreshBeforeCompletion()
            // Mail read or removed on another device came in with this sync.
            IosNewMailReconciler.reconcileNow()
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
        defer { IosReminders.refresh() }
        let accountList = invoke(id: 1, method: "account.list", params: [:])
        guard let accounts = accountList["result"] as? [String: Any],
              let accountRows = accounts["accounts"] as? [[String: Any]]
        else {
            IosSyncDiagnosticLog.append("account.list failed")
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
            if let error = response["error"] as? [String: Any] {
                let message = error["message"] as? String ?? ""
                IosSyncDiagnosticLog.append(
                    "\(request.method) failed for account \(IosSyncLogRedaction.accountLabel(account)): "
                        + IosSyncLogRedaction.redactMessage(message)
                )
                failed += 1
            } else {
                refreshed += 1
                // Arrivals ride back on the sync response: this runs with no live
                // event listener, and a feed refresh raises no event at all.
                notifyArrivals(response)
                if request.method == "mail.sync" {
                    syncNotifyFolders(account: account, accountId: accountId, id: request.id)
                }
            }
        }

        IosSyncDiagnosticLog.append(
            "background refresh done: refreshed=\(refreshed) skipped=\(skipped) failed=\(failed)"
        )
        return RefreshResult(refreshed: refreshed, skipped: skipped, failed: failed)
    }

    private static func notifyArrivals(_ response: [String: Any]) {
        guard let result = response["result"] as? [String: Any],
              let detail = result["new_messages"] as? [String: Any],
              let batch = iosNewMailBatch(detail)
        else { return }
        IosNotificationService.notifyNewMail(
            accountName: batch.accountName,
            from: batch.from,
            subject: batch.subject,
            count: batch.count,
            accountId: batch.accountId,
            folder: batch.folder,
            threadKey: batch.threadKey,
            storeMessages: batch.storeMessages
        )
    }

    /// Sync the folders the user opted in to notifications, after the account's
    /// inbox sync refreshed the folder list. A failure is logged without failing
    /// the account, whose inbox did refresh.
    private static func syncNotifyFolders(account: [String: Any], accountId: String, id: Int64) {
        let list = invoke(id: id, method: "mail.folderList", params: ["account_id": accountId])
        guard let result = list["result"] as? [String: Any],
              let folders = result["folders"] as? [[String: Any]]
        else { return }
        for folder in folders {
            guard folder["notify"] as? Bool == true,
                  let name = folder["name"] as? String,
                  name.caseInsensitiveCompare("inbox") != .orderedSame
            else { continue }
            let response = invoke(
                id: id,
                method: "mail.sync",
                params: ["account_id": accountId, "folder_id": name, "limit": 50, "folders": false]
            )
            if let error = response["error"] as? [String: Any] {
                let message = error["message"] as? String ?? ""
                IosSyncDiagnosticLog.append(
                    "mail.sync of an opted-in folder failed for account \(IosSyncLogRedaction.accountLabel(account)): "
                        + IosSyncLogRedaction.redactMessage(message)
                )
            } else {
                notifyArrivals(response)
            }
        }
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
            localizedCatalogString(
                BackgroundRefreshKt.backgroundRefreshSummaryKey(
                    refreshed: Int32(refreshed),
                    skipped: Int32(skipped),
                    failed: Int32(failed)
                ),
                args: ["refreshed": refreshed, "skipped": skipped, "failed": failed]
            )
        }
    }
}

/// What a `mail.newMessages` detail says a notification should show. Both
/// `mail.sync` and `rss.sync` return one under `new_messages` when a refresh
/// found arrivals; a feed's "sender" is the feed title and its thread key is the
/// subscription.
struct IosNewMailBatch: Equatable {
    let accountId: String
    let accountName: String
    let folder: String
    let from: String
    let subject: String
    let count: Int
    let threadKey: String
    /// Every arrival the notification announces, so it can be withdrawn once
    /// they are all read; empty when the detail does not list them all.
    var storeMessages: [IosStoreMessage] = []
}

/// Read a `new_messages` detail into a batch worth notifying about, or nil when
/// there is nothing to post: a muted account (it still syncs, it just stays
/// quiet) or a payload naming no arrivals.
func iosNewMailBatch(_ detail: [String: Any]) -> IosNewMailBatch? {
    if detail["muted"] as? Bool == true {
        return nil
    }
    let count = detail["count"] as? Int ?? 0
    guard count > 0 else { return nil }
    return IosNewMailBatch(
        accountId: detail["account"] as? String ?? "",
        accountName: detail["accountName"] as? String ?? "",
        folder: detail["folder"] as? String ?? "",
        from: detail["from"] as? String ?? "",
        subject: detail["subject"] as? String ?? "",
        count: count,
        threadKey: detail["threadKey"] as? String ?? "",
        storeMessages: iosStoreMessages(detail, count: count)
    )
}

/// The store rows a detail's `messages` name: mail by the batch folder and
/// UID, a feed entry by its subscription (the entry's thread key) and entry
/// key. Empty unless every arrival is listed and identifiable — a notification
/// standing partly for unlisted mail must not be withdrawn on the rest.
func iosStoreMessages(_ detail: [String: Any], count: Int) -> [IosStoreMessage] {
    let accountId = detail["account"] as? String ?? ""
    let folder = detail["folder"] as? String ?? ""
    let entries = detail["messages"] as? [[String: Any]] ?? []
    let messages: [IosStoreMessage] = entries.compactMap { entry in
        if accountId.hasPrefix("rss-") {
            let subscription = entry["threadKey"] as? String ?? ""
            let itemKey = entry["itemKey"] as? String ?? ""
            return subscription.isEmpty || itemKey.isEmpty ? nil : IosStoreMessage(folder: subscription, messageId: itemKey)
        }
        let uid = (entry["uid"] as? NSNumber)?.int64Value ?? 0
        return uid > 0 && !folder.isEmpty ? IosStoreMessage(folder: folder, messageId: String(uid)) : nil
    }
    return messages.count == count && messages.count == entries.count ? messages : []
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

/// Publish the next 64 local reminders to iOS, which delivers them even after
/// the process is suspended. Reconcile only our requests, leaving mail intact.
/// A serial queue plus generation check prevents an older plan winning a race
/// with an edit made while the notification center is answering.
enum IosReminders {
    private static let queue = DispatchQueue(label: "jp.nonbili.meron.reminders")
    private static var generation = 0
    private static let prefix = "meron-reminder:"
    private static let historyKey = "meron.reminders.scheduled"
    private static let enabledKey = "meron.reminders.enabled"
    private static var scheduled = UserDefaults.standard.dictionary(forKey: historyKey) as? [String: Double] ?? [:]
    private static var lastFailureLogAt: TimeInterval = 0
    private static var wantedIDs = Set<String>()

    static func allowsScheduling(_ status: UNAuthorizationStatus) -> Bool {
        switch status {
        case .authorized, .provisional, .ephemeral: return true
        default: return false
        }
    }

    struct Selection {
        let requests: [[String: Any]]
        let wanted: Set<String>
        let history: [String: Double]
    }

    /// Preserve due requests already accepted by iOS, suppress replay, and
    /// reserve their slots before adding new requests to the 64-request window.
    static func selectRequests(
        reminders: [[String: Any]],
        history: [String: Double],
        pending: Set<String>,
        now: TimeInterval
    ) -> Selection {
        // Keep an extra day beyond core's look-back to cover fractional seconds
        // and the delay between fetching a plan and reconciling native requests.
        let history = history.filter { $0.value >= now - 2 * 86_400 }
        var retained = Set<String>()
        for reminder in reminders {
            guard let key = reminder["key"] as? String,
                  let at = (reminder["at"] as? NSNumber)?.doubleValue else { continue }
            let id = prefix + key
            if at <= now && history[key] != nil && pending.contains(id) {
                retained.insert(id)
            }
        }
        let requests = Array(reminders.filter {
            guard let at = ($0["at"] as? NSNumber)?.doubleValue,
                  let key = $0["key"] as? String else { return false }
            return at > now || history[key] == nil
        }.prefix(max(0, 64 - retained.count)))
        let wanted = retained.union(requests.compactMap { ($0["key"] as? String).map { prefix + $0 } })
        // A canceled request never becomes a delivery receipt. In particular,
        // forgetting a future canceled trigger prevents its later acknowledgement
        // from suppressing the replacement reminder for the same occurrence.
        let keptHistory = history.filter { key, at in
            wanted.contains(prefix + key) || (at <= now && !pending.contains(prefix + key))
        }
        return Selection(requests: requests, wanted: wanted, history: keptHistory)
    }

    static func refresh() {
        queue.async {
            generation += 1
            let revision = generation
            let center = UNUserNotificationCenter.current()
            center.getNotificationSettings { settings in
                queue.async {
                    guard generation == revision else { return }
                    let enabled = allowsScheduling(settings.authorizationStatus)
                    let wasEnabled = UserDefaults.standard.bool(forKey: enabledKey)
                    if !enabled {
                        UserDefaults.standard.set(false, forKey: enabledKey)
                        wantedIDs.removeAll()
                        center.getPendingNotificationRequests { pending in
                            queue.async {
                                guard generation == revision else { return }
                                center.removePendingNotificationRequests(withIdentifiers: pending.map(\.identifier).filter { $0.hasPrefix(prefix) })
                            }
                        }
                        return
                    }
                    if !wasEnabled {
                        guard invoke("reminders.skipPast")?["ok"] as? Bool == true else { return }
                        UserDefaults.standard.set(true, forKey: enabledKey)
                    }
                    reconcile(center: center, revision: revision)
                }
            }
        }
    }

    private static func invoke(_ method: String, params: [String: Any] = [:]) -> [String: Any]? {
        let request: [String: Any] = ["id": 94, "method": method, "params": params]
        guard let data = try? JSONSerialization.data(withJSONObject: request),
              let json = String(data: data, encoding: .utf8) else { return nil }
        let raw = RustCoreBridge.invokeJson(json)
        guard let response = raw.data(using: .utf8),
              let envelope = try? JSONSerialization.jsonObject(with: response) as? [String: Any] else { return nil }
        return envelope["result"] as? [String: Any]
    }

    private static func logFailure() {
        let now = Date().timeIntervalSince1970
        if now - lastFailureLogAt >= 60 {
            lastFailureLogAt = now
            IosSyncDiagnosticLog.append("Unable to schedule reminders")
        }
    }

    private static func reconcile(center: UNUserNotificationCenter, revision: Int) {
        center.getPendingNotificationRequests { pending in
            queue.async {
                guard generation == revision else { return }
                let now = Date().timeIntervalSince1970
                let pendingIDs = Set(pending.map(\.identifier))
                // Accepted requests no longer pending have already run. Persist
                // their receipts in one batch so they cannot crowd catch-up
                // items out of core's next plan or replay after a restart.
                let finished = scheduled.compactMap { key, at in
                    at <= now && !pendingIDs.contains(prefix + key) ? key : nil
                }
                if !finished.isEmpty {
                    guard invoke("reminders.acknowledge", params: ["keys": finished])?["ok"] as? Bool == true else {
                        logFailure()
                        return
                    }
                    for key in finished { scheduled.removeValue(forKey: key) }
                }
                guard let result = invoke("reminders.list"),
                      let reminders = result["reminders"] as? [[String: Any]] else {
                    logFailure()
                    return
                }
                let selection = selectRequests(reminders: reminders, history: scheduled, pending: pendingIDs, now: Date().timeIntervalSince1970)
                scheduled = selection.history
                wantedIDs = selection.wanted
                UserDefaults.standard.set(scheduled, forKey: historyKey)
                let stale = pendingIDs.filter { $0.hasPrefix(prefix) && !selection.wanted.contains($0) }
                center.removePendingNotificationRequests(withIdentifiers: Array(stale))
                for reminder in selection.requests {
                    guard let key = reminder["key"] as? String,
                          let at = reminder["at"] as? NSNumber else { continue }
                    // The app may have been suspended since this plan was read.
                    // Leave an already accepted due request to iOS instead of
                    // replacing it with a fresh one-second alert.
                    if at.doubleValue <= Date().timeIntervalSince1970 && scheduled[key] != nil { continue }
                    let content = UNMutableNotificationContent()
                    content.title = reminder["title"] as? String ?? ""
                    let kind = reminder["kind"] as? String ?? "calendar"
                    let label = kind == "tasks" ? String(localized: "tasks.title") : String(localized: "calendar.title")
                    let location = reminder["body"] as? String ?? ""
                    content.body = location.isEmpty ? label : label + " · " + location
                    content.sound = .default
                    let trigger = UNTimeIntervalNotificationTrigger(timeInterval: max(1, at.doubleValue - Date().timeIntervalSince1970), repeats: false)
                    center.add(UNNotificationRequest(identifier: prefix + key, content: content, trigger: trigger)) { error in
                        queue.async {
                            guard wantedIDs.contains(prefix + key) else {
                                if error == nil { center.removePendingNotificationRequests(withIdentifiers: [prefix + key]) }
                                return
                            }
                            if error == nil {
                                if scheduled[key] != at.doubleValue {
                                    scheduled[key] = at.doubleValue
                                    UserDefaults.standard.set(scheduled, forKey: historyKey)
                                }
                            } else {
                                logFailure()
                            }
                        }
                    }
                }
            }
        }
    }
}
