import Foundation

struct UnreadWidgetThread: Codable, Equatable, Identifiable {
    let id: String
    let sender: String
    let subject: String
    let date: TimeInterval

    /// Absolute timestamps stay accurate between WidgetKit timeline reloads.
    func dateLabel(relativeTo now: Date, calendar: Calendar = .current, locale: Locale = .current) -> String {
        let date = Date(timeIntervalSince1970: self.date)
        let formatter = DateFormatter()
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.locale = locale
        let today = calendar.isDate(date, inSameDayAs: now)
        formatter.dateStyle = today ? .none : .short
        formatter.timeStyle = today ? .short : .none
        return formatter.string(from: date)
    }

    var url: URL {
        var components = URLComponents()
        components.scheme = "meron"
        components.host = "thread"
        components.queryItems = [URLQueryItem(name: "id", value: id)]
        return components.url!
    }
}

struct UnreadWidgetScope: Codable, Equatable, Identifiable {
    let id: String
    let title: String
    let count: Int
    let threads: [UnreadWidgetThread]
}

struct UnreadWidgetSnapshot: Codable, Equatable {
    let updatedAt: Date
    let scopes: [UnreadWidgetScope]

    func scope(_ id: String?) -> UnreadWidgetScope? {
        scopes.first { $0.id == (id ?? "unified") } ?? scopes.first { $0.id == "unified" }
    }
}

enum UnreadWidgetStore {
    static let group = "group.jp.nonbili.meron"
    static var fileURL: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)?
            .appendingPathComponent("unread-widget.json")
    }

    static func read() -> UnreadWidgetSnapshot? {
        guard let url = fileURL, let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(UnreadWidgetSnapshot.self, from: data)
    }

    static func write(_ snapshot: UnreadWidgetSnapshot) throws {
        guard let url = fileURL else { return }
        try JSONEncoder().encode(snapshot).write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }
}

/// All reads are local. A failed read leaves the last good snapshot intact.
enum UnreadWidgetSnapshotBuilder {
    static func build(now: Date = Date(), invoke: (String, [String: Any]) -> [String: Any]?) -> UnreadWidgetSnapshot? {
        guard let result = invoke("account.list", [:]), let accounts = result["accounts"] as? [[String: Any]] else { return nil }
        var scopes: [UnreadWidgetScope] = []
        var unifiedCount = 0
        for account in accounts {
            guard let id = account["id"] as? String,
                  let result = invoke("mail.folderList", ["account_id": id]),
                  let folders = result["folders"] as? [[String: Any]] else { return nil }
            let inbox = folders.first { ($0["role"] as? String) == "inbox" || ($0["name"] as? String)?.uppercased() == "INBOX" }
            let count = max(0, inbox?["unread"] as? Int ?? 0)
            if account["included_in_unified"] as? Bool != false { unifiedCount += count }
            var threads: [UnreadWidgetThread] = []
            if let folder = inbox?["name"] as? String {
                guard let rows = readThreads(params: ["account_id": id, "folder_id": folder], invoke: invoke) else { return nil }
                threads = rows
            }
            let name = account["display_name"] as? String ?? ""
            scopes.append(UnreadWidgetScope(id: id, title: name.isEmpty ? (account["email"] as? String ?? id) : name, count: count, threads: threads))
        }
        guard let unifiedThreads = readThreads(params: ["account_id": "unified", "folder_id": "inbox", "folder_role": "inbox"], invoke: invoke) else { return nil }
        scopes.insert(UnreadWidgetScope(id: "unified", title: "", count: unifiedCount, threads: unifiedThreads), at: 0)
        return UnreadWidgetSnapshot(updatedAt: now, scopes: scopes)
    }

    private static func readThreads(params: [String: Any], invoke: (String, [String: Any]) -> [String: Any]?) -> [UnreadWidgetThread]? {
        var params = params
        params.merge(["filter": "unread", "refresh": false, "limit": 8]) { _, new in new }
        guard let result = invoke("mail.threadList", params), let rows = result["threads"] as? [[String: Any]] else { return nil }
        var seen = Set<String>()
        return rows.compactMap { row in
            guard let id = row["id"] as? String, !id.isEmpty, seen.insert(id).inserted else { return nil }
            let sender = ["from_name", "from_addr", "from"].compactMap { row[$0] as? String }.first { !$0.isEmpty } ?? ""
            return UnreadWidgetThread(id: id, sender: sender, subject: row["subject"] as? String ?? "", date: (row["date"] as? NSNumber)?.doubleValue ?? (row["date_epoch_seconds"] as? NSNumber)?.doubleValue ?? 0)
        }.sorted { $0.date > $1.date }.prefix(8).map { $0 }
    }
}
