import Foundation
import MeronUI

func inboxUnreadCount(folders: [FolderSummary], accountId: String) -> Int {
    folders
        .filter { folder in
            folder.accountId == accountId && folder.name.compare(iosInboxFolderId, options: [.caseInsensitive, .diacriticInsensitive]) == .orderedSame
        }
        .reduce(0) { $0 + Int($1.unread) }
}

func navigationAccountLabel(_ account: AccountSummary, unreadCounts: [String: Int], showUnreadBadges: Bool) -> String {
    let label = account.displayName.isEmpty ? (account.email.isEmpty ? account.id : account.email) : account.displayName
    guard showUnreadBadges, let unread = unreadCounts[account.id], unread > 0 else { return label }
    return "\(label) (\(unread))"
}

func navigationUnifiedInboxLabel(accounts: [AccountSummary], unreadCounts: [String: Int], showUnreadBadges: Bool) -> String {
    let label = String(localized: "kanban.columns.unifiedInbox")
    guard showUnreadBadges else { return label }
    let unread = accounts
        .filter(\.includedInUnified)
        .reduce(0) { $0 + (unreadCounts[$1.id] ?? 0) }
    return unread > 0 ? "\(label) (\(unread))" : label
}
