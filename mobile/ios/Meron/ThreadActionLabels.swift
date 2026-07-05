import Foundation
import MeronUI
import SwiftUI

func threadDeleteActionLabel(_ thread: ThreadSummary) -> String {
    threadDeleteActionLabel(folder: thread.folder)
}

func threadDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "threads.actions.discardDraft")
    }
    if MailStateKt.folderIsTrash(folder: folder) {
        return String(localized: "threads.actions.deleteForever")
    }
    return String(localized: "threads.actions.moveToTrash")
}

func threadReadToggleActionLabel(_ thread: ThreadSummary) -> String {
    thread.unread ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread")
}

func iosAccountStateLabels(
    needsReconnect: Bool,
    paused: Bool,
    muted: Bool,
    hiddenFromNavigation: Bool
) -> [String] {
    var labels: [String] = []
    if needsReconnect {
        labels.append(String(localized: "mobile.ios.needsReconnect"))
    }
    if paused {
        labels.append(String(localized: "settings.account.pauseAccount"))
    }
    if muted {
        labels.append(String(localized: "settings.account.muteNotifications"))
    }
    if hiddenFromNavigation {
        labels.append(String(localized: "settings.account.hiddenFromNavigation"))
    }
    return labels
}

func kanbanColumnHideActionLabel() -> String {
    String(localized: "kanban.actions.hideColumn")
}

func threadReadUndoSeenTarget(afterMarkingSeen seen: Bool) -> Bool {
    !seen
}

func threadStarUndoTarget(afterSettingStarred starred: Bool) -> Bool {
    !starred
}

func threadDeleteRequiresConfirmation(_ thread: ThreadSummary?) -> Bool {
    guard let thread else { return false }
    return threadDeleteRequiresConfirmation(folder: thread.folder)
}

func threadDeleteRequiresConfirmation(folder: String) -> Bool {
    MailStateKt.folderIsDrafts(folder: folder) || MailStateKt.folderIsTrash(folder: folder)
}

func firstThreadRequiringDeleteConfirmation(_ threads: [ThreadSummary]) -> ThreadSummary? {
    threads.first { threadDeleteRequiresConfirmation($0) }
}

func threadDeleteConfirmationTitle(_ thread: ThreadSummary?) -> String {
    guard let thread else { return String(localized: "buttons.delete") }
    return MailStateKt.folderIsDrafts(folder: thread.folder)
        ? String(localized: "mobile.compose.discardDraftTitle")
        : threadDeleteActionLabel(thread)
}

func threadDeleteConfirmationMessage(_ thread: ThreadSummary) -> String {
    if MailStateKt.folderIsDrafts(folder: thread.folder) {
        return String(localized: "mobile.compose.discardDraftText")
    }
    return thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject
}

func sendShortcutLabel(_ mode: String) -> String {
    mode == "enter" ? "Enter" : "Cmd/Ctrl+Enter"
}

func sendShortcutHintLocalizationKey(_ mode: String) -> String {
    mode == "enter" ? "settings.composer.sendShortcutEnterHint" : "settings.composer.sendShortcutModHint"
}

func sendShortcutHintText(_ mode: String) -> String {
    if mode == "enter" {
        return String(localized: "settings.composer.sendShortcutEnterHint")
    }
    return localizedCatalogString(
        "settings.composer.sendShortcutModHint",
        args: ["shortcut": sendShortcutLabel("mod_enter")]
    )
}

func iosSyncErrorLooksAuthRelated(_ message: String) -> Bool {
    let lower = message.lowercased()
    return ["auth", "login", "credential", "password", "unauthor", "permission", "token", "401", "535"]
        .contains { lower.contains($0) }
}

func sendShortcutMatches(_ press: KeyPress, mode: String) -> Bool {
    guard press.key == .return else { return false }
    if mode == "enter" {
        return !press.modifiers.contains(.shift) &&
            !press.modifiers.contains(.command) &&
            !press.modifiers.contains(.control)
    }
    return !press.modifiers.contains(.shift) &&
        (press.modifiers.contains(.command) || press.modifiers.contains(.control))
}

func quickReplyCanSend(body: String, attachmentCount: Int, sending: Bool) -> Bool {
    !sending && (!body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || attachmentCount > 0)
}

func rssFeedMoveTargetAccounts(accounts: [AccountSummary], sourceAccountId: String) -> [AccountSummary] {
    accounts.filter { account in
        MailStateKt.accountSummaryIsRss(account: account) && account.id != sourceAccountId
    }
}

struct IosSelectionMoveCopyAvailability: Equatable {
    var canMove: Bool
    var canCopy: Bool
}

func iosSelectionMoveCopyAvailability(selectedThreads: [ThreadSummary], rssMoveTargetCount: Int) -> IosSelectionMoveCopyAvailability {
    guard selectedThreads.count == 1, let thread = selectedThreads.first else {
        return IosSelectionMoveCopyAvailability(canMove: false, canCopy: false)
    }
    if MailStateKt.threadIdIsRss(threadId: thread.id) {
        return IosSelectionMoveCopyAvailability(canMove: rssMoveTargetCount > 0, canCopy: false)
    }
    return IosSelectionMoveCopyAvailability(canMove: true, canCopy: true)
}

struct IosCommand: Identifiable {
    let id: String
    let label: String
    let keywords: String
    let systemImage: String
    let active: Bool
    let role: ButtonRole?
    let action: () -> Void
}

func iosCommandMatches(_ command: IosCommand, query: String) -> Bool {
    let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    guard !normalized.isEmpty else { return true }
    return [
        command.label,
        command.keywords,
        command.id,
    ].joined(separator: " ").lowercased().contains(normalized)
}

func messagesAfterDeletingMessage(_ messages: [MessageBody], messageId: String) -> [MessageBody] {
    messages.filter { $0.id != messageId }
}

func messageDeleteRequiresConfirmation(folder _: String) -> Bool {
    true
}

func messageDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "chat.actions.discardDraft")
    }
    return String(localized: "chat.actions.deleteMessage")
}

func messageDeleteConfirmationTitle(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "mobile.compose.discardDraftTitle")
    }
    return messageDeleteActionLabel(folder: folder)
}

func messageDeleteConfirmationMessage(_ message: MessageBody, folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "mobile.compose.discardDraftText")
    }
    return message.subject.isEmpty ? String(localized: "threads.noSubject") : message.subject
}

func starredItemDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "chat.actions.discardDraft")
    }
    return String(localized: "chat.actions.deleteMessage")
}

func starredItemDeleteConfirmationMessage(_ item: StarredItemSummary) -> String {
    if MailStateKt.folderIsDrafts(folder: item.folder) {
        return String(localized: "mobile.compose.discardDraftText")
    }
    return item.subject.isEmpty ? String(localized: "threads.noSubject") : item.subject
}

func rssFeedDeleteConfirmationMessage(_ thread: ThreadSummary) -> String {
    let title = thread.subject.isEmpty ? String(localized: "feeds.fallbackName") : thread.subject
    return "\(title)\n\n\(String(localized: "feeds.deleteHint"))"
}

func iosConversationDeleteCommandLabel(_ thread: ThreadSummary) -> String {
    MailStateKt.threadIdIsRss(threadId: thread.id) ? String(localized: "feeds.actions.deleteFeed") : threadDeleteActionLabel(thread)
}

func iosFullReplyShortcutCanOpen(selectedTab: IosAppTab, thread: ThreadSummary?) -> Bool {
    guard selectedTab == .mail, let thread else { return false }
    return !MailStateKt.threadIdIsRss(threadId: thread.id)
}

func iosComposeReturnTab(from selectedTab: IosAppTab) -> IosAppTab {
    switch selectedTab {
    case .kanban, .starred:
        return selectedTab
    case .mail, .compose, .accounts:
        return .mail
    }
}

func iosCommandPaletteAdjacentThreadSource(
    selectedTab: IosAppTab,
    mailThreads: [ThreadSummary],
    kanbanThreads: [ThreadSummary]
) -> [ThreadSummary] {
    selectedTab == .kanban ? kanbanThreads : mailThreads
}

func adjacentThreadSummary(_ threads: [ThreadSummary], currentId: String?, delta: Int) -> ThreadSummary? {
    guard !threads.isEmpty else { return nil }
    guard let currentId,
          let currentIndex = threads.firstIndex(where: { $0.id == currentId })
    else {
        return threads.first
    }
    let nextIndex = min(threads.count - 1, max(0, currentIndex + delta))
    return threads[nextIndex]
}

func iosPreferredAccountAfterAccountList(
    accounts: [AccountSummary],
    preferredEmail: String?,
    previousAccountIds: Set<String>?
) -> AccountSummary? {
    let email = preferredEmail?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() ?? ""
    if !email.isEmpty,
       let account = accounts.first(where: { $0.email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == email })
    {
        return account
    }
    guard let previousAccountIds else { return nil }
    return accounts.first { !previousAccountIds.contains($0.id) }
}

func selectedThreadsMarkReadTarget(_ threads: [ThreadSummary]) -> Bool {
    threads.contains(where: \.unread)
}

func selectedThreadsStarTarget(_ threads: [ThreadSummary]) -> Bool {
    threads.contains { !$0.starred }
}

func firstRssThread(_ threads: [ThreadSummary]) -> ThreadSummary? {
    threads.first { MailStateKt.threadIdIsRss(threadId: $0.id) }
}

func selectedThreadsPartitionedForArchiveOrRemove(_ threads: [ThreadSummary]) -> (mail: [ThreadSummary], rss: [ThreadSummary]) {
    (
        mail: threads.filter { !MailStateKt.threadIdIsRss(threadId: $0.id) },
        rss: threads.filter { MailStateKt.threadIdIsRss(threadId: $0.id) }
    )
}
