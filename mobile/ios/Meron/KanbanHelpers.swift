import Foundation
import MeronUI

func loadIosKanbanBoards() -> [IosKanbanBoardSpec] {
    guard let data = UserDefaults.standard.data(forKey: "ios_kanban_boards_v1"),
          let boards = try? JSONDecoder().decode([IosKanbanBoardSpec].self, from: data)
    else {
        return []
    }
    return boards
}

func visibleKanbanThreadSummaries(
    board: IosKanbanBoardSpec?,
    threadsByColumn: [String: [ThreadSummary]]
) -> [ThreadSummary] {
    guard let board else { return [] }
    var seen: Set<String> = []
    return board.columns.flatMap { column in
        threadsByColumn[column.id] ?? []
    }.filter { thread in
        seen.insert(thread.id).inserted
    }
}

func kanbanThreadSummary(for item: StarredItemSummary) -> ThreadSummary {
    ThreadSummary(
        id: item.id,
        accountId: item.accountId,
        folder: item.folder,
        folderRole: item.folderRole,
        subject: item.subject,
        sender: item.sender,
        preview: item.preview,
        unread: item.unread,
        unreadCount: item.unread ? 1 : 0,
        starred: true,
        hasDraft: false,
        dateEpochSeconds: item.dateEpochSeconds,
        feedUrl: "",
        threadId: item.threadId
    )
}

func starredItemReaderMessage(for item: StarredItemSummary) -> MessageBody {
    MessageBody(
        id: item.id,
        folderId: item.folder,
        from: item.sender,
        to: "",
        cc: "",
        bcc: "",
        subject: item.subject,
        body: item.preview,
        bodyHtml: "",
        dateEpochSeconds: item.dateEpochSeconds,
        fromAddr: item.sender,
        replyTo: "",
        messageId: "",
        inReplyTo: "",
        references: "",
        unread: item.unread,
        outgoing: false,
        starred: true,
        hasAttachments: false,
        bodyMissing: false,
        attachments: [],
        sendStatus: .none
    )
}

func starredKanbanReaderMessage(for thread: ThreadSummary) -> MessageBody {
    MessageBody(
        id: thread.id,
        folderId: thread.folder,
        from: thread.sender,
        to: "",
        cc: "",
        bcc: "",
        subject: thread.subject,
        body: thread.preview,
        bodyHtml: "",
        dateEpochSeconds: thread.dateEpochSeconds,
        fromAddr: thread.sender,
        replyTo: "",
        messageId: "",
        inReplyTo: "",
        references: "",
        unread: thread.unread,
        outgoing: false,
        starred: thread.starred,
        hasAttachments: false,
        bodyMissing: false,
        attachments: [],
        sendStatus: .none
    )
}

enum StarredItemListUpdate {
    case read(seen: Bool)
    case remove
}

func starredItemsAfterAction(
    _ items: [StarredItemSummary],
    itemId: String,
    update: StarredItemListUpdate
) -> [StarredItemSummary] {
    switch update {
    case let .read(seen):
        return items.map { item in
            guard item.id == itemId else { return item }
            return StarredItemSummary(
                id: item.id,
                threadId: item.threadId,
                accountId: item.accountId,
                folder: item.folder,
                folderRole: item.folderRole,
                subject: item.subject,
                sender: item.sender,
                preview: item.preview,
                unread: !seen,
                dateEpochSeconds: item.dateEpochSeconds
            )
        }
    case .remove:
        return items.filter { $0.id != itemId }
    }
}

func sortedStarredItems(_ items: [StarredItemSummary]) -> [StarredItemSummary] {
    items.sorted { lhs, rhs in
        if lhs.dateEpochSeconds != rhs.dateEpochSeconds {
            return lhs.dateEpochSeconds > rhs.dateEpochSeconds
        }
        return lhs.id < rhs.id
    }
}

func starredItemsAfterThreadReadState(
    _ items: [StarredItemSummary],
    threadId: String,
    seen: Bool
) -> [StarredItemSummary] {
    items.map { item in
        guard item.threadId == threadId else { return item }
        return StarredItemSummary(
            id: item.id,
            threadId: item.threadId,
            accountId: item.accountId,
            folder: item.folder,
            folderRole: item.folderRole,
            subject: item.subject,
            sender: item.sender,
            preview: item.preview,
            unread: !seen,
            dateEpochSeconds: item.dateEpochSeconds
        )
    }
}

func starredItemsAfterMessageStarred(
    _ items: [StarredItemSummary],
    message: MessageBody,
    thread: ThreadSummary,
    starred: Bool
) -> [StarredItemSummary] {
    if !starred {
        return starredItemsAfterAction(items, itemId: message.id, update: .remove)
    }
    let item = StarredItemSummary(
        id: message.id,
        threadId: thread.id,
        accountId: thread.accountId,
        folder: thread.folder,
        folderRole: thread.folderRole,
        subject: message.subject,
        sender: message.from.isEmpty ? thread.sender : message.from,
        preview: messagePlainText(message),
        unread: message.unread,
        dateEpochSeconds: message.dateEpochSeconds
    )
    let withoutExisting = items.filter { $0.id != message.id }
    return sortedStarredItems(withoutExisting + [item])
}

func starredItemsAfterThreadStarred(
    _ items: [StarredItemSummary],
    thread: ThreadSummary,
    messages: [MessageBody],
    starred: Bool
) -> [StarredItemSummary] {
    if !starred {
        return items.filter { $0.threadId != thread.id }
    }
    let messageItems = messages.map { message in
        StarredItemSummary(
            id: message.id,
            threadId: thread.id,
            accountId: thread.accountId,
            folder: thread.folder,
            folderRole: thread.folderRole,
            subject: message.subject,
            sender: message.from.isEmpty ? thread.sender : message.from,
            preview: messagePlainText(message),
            unread: message.unread,
            dateEpochSeconds: message.dateEpochSeconds
        )
    }
    let messageIds = Set(messageItems.map(\.id))
    return sortedStarredItems(items.filter { !messageIds.contains($0.id) } + messageItems)
}

func starredKanbanThreadsAfterAction(
    _ threads: [ThreadSummary],
    itemId: String,
    update: StarredItemListUpdate
) -> [ThreadSummary] {
    switch update {
    case let .read(seen):
        return threads.map { thread in
            thread.id == itemId ? thread.withUnread(!seen) : thread
        }
    case .remove:
        return threads.filter { $0.id != itemId }
    }
}

func starredKanbanThreadsAfterThreadReadState(
    _ threads: [ThreadSummary],
    threadId: String,
    threadIdsByItemId: [String: String],
    seen: Bool
) -> [ThreadSummary] {
    threads.map { thread in
        let mappedThreadId = threadIdsByItemId[thread.id] ?? thread.id
        return mappedThreadId == threadId ? thread.withUnread(!seen) : thread
    }
}

func starredKanbanThreadsAfterThreadStarred(
    _ threads: [ThreadSummary],
    threadId: String,
    threadIdsByItemId: [String: String],
    starred: Bool
) -> [ThreadSummary] {
    guard !starred else { return threads }
    return threads.filter { thread in
        let mappedThreadId = threadIdsByItemId[thread.id] ?? thread.id
        return mappedThreadId != threadId
    }
}

func kanbanThreadsAfterThreadFlagUpdate(
    _ threads: [ThreadSummary],
    threadId: String,
    unread: Bool? = nil,
    starred: Bool? = nil
) -> [ThreadSummary] {
    threads.map { thread in
        thread.id == threadId ? thread.withFlags(unread: unread, starred: starred) : thread
    }
}

func kanbanThreadsAfterRemovingThread(_ threads: [ThreadSummary], threadId: String) -> [ThreadSummary] {
    threads.filter { $0.id != threadId }
}

func selectedThreadIdsAfterRemovingThread(_ selectedIds: Set<String>, threadId: String) -> Set<String> {
    var next = selectedIds
    next.remove(threadId)
    return next
}

func threadsAfterMarkingThreadIdsRead(_ threads: [ThreadSummary], threadIds: Set<String>) -> [ThreadSummary] {
    threads.map { thread in
        threadIds.contains(thread.id) ? thread.withUnread(false) : thread
    }
}

func messagesAfterThreadReadState(_ messages: [MessageBody], unread: Bool) -> [MessageBody] {
    messages.map { $0.withFlags(unread: unread) }
}

func messagesAfterThreadStarredState(_ messages: [MessageBody], starred: Bool) -> [MessageBody] {
    messages.map { $0.withFlags(starred: starred) }
}

struct KanbanStarredItemActionTarget: Equatable {
    let itemId: String
    let threadId: String
    let isRss: Bool
}

func kanbanStarredItemActionTarget(
    for thread: ThreadSummary,
    threadIdsByItemId: [String: String]
) -> KanbanStarredItemActionTarget {
    let threadId = threadIdsByItemId[thread.id] ?? thread.id
    return KanbanStarredItemActionTarget(
        itemId: thread.id,
        threadId: threadId,
        isRss: MailStateKt.threadIdIsRss(threadId: threadId)
    )
}

func kanbanDeleteIsStarredMessage(column: IosKanbanColumnSpec) -> Bool {
    column.accountId == iosUnifiedAccountId && column.folderId.caseInsensitiveCompare(iosStarredFolderId) == .orderedSame
}

func kanbanDeleteRequiresConfirmation(thread: ThreadSummary, in column: IosKanbanColumnSpec) -> Bool {
    kanbanDeleteIsStarredMessage(column: column)
        ? messageDeleteRequiresConfirmation(folder: thread.folder)
        : threadDeleteRequiresConfirmation(folder: thread.folder)
}

func kanbanDeleteActionLabel(_ target: IosKanbanDeleteTarget?) -> String {
    guard let target else { return String(localized: "buttons.delete") }
    return kanbanDeleteIsStarredMessage(column: target.column)
        ? messageDeleteActionLabel(folder: target.thread.folder)
        : threadDeleteActionLabel(target.thread)
}

func kanbanDeleteConfirmationTitle(_ target: IosKanbanDeleteTarget?) -> String {
    guard let target else { return String(localized: "buttons.delete") }
    return kanbanDeleteIsStarredMessage(column: target.column)
        ? messageDeleteConfirmationTitle(folder: target.thread.folder)
        : threadDeleteConfirmationTitle(target.thread)
}

func kanbanDeleteConfirmationMessage(_ target: IosKanbanDeleteTarget) -> String {
    if kanbanDeleteIsStarredMessage(column: target.column) {
        if MailStateKt.folderIsDrafts(folder: target.thread.folder) {
            return String(localized: "mobile.compose.discardDraftText")
        }
        return target.thread.subject.isEmpty ? String(localized: "threads.noSubject") : target.thread.subject
    }
    return threadDeleteConfirmationMessage(target.thread)
}

enum IosKanbanMoveValidation: Equatable {
    case allowed
    case missingTargetAccount
    case rssFeedToMailAccount
    case mailThreadToRssAccount
}

func iosKanbanMoveValidation(threadIsRss: Bool, targetAccount: AccountSummary?) -> IosKanbanMoveValidation {
    guard let targetAccount else { return .missingTargetAccount }
    let targetIsRss = MailStateKt.accountSummaryIsRss(account: targetAccount)
    if threadIsRss && !targetIsRss {
        return .rssFeedToMailAccount
    }
    if !threadIsRss && targetIsRss {
        return .mailThreadToRssAccount
    }
    return .allowed
}

func iosKanbanMoveValidationStatus(_ validation: IosKanbanMoveValidation) -> String {
    switch validation {
    case .allowed:
        return ""
    case .missingTargetAccount:
        return String(localized: "mobile.ios.moveFailed")
    case .rssFeedToMailAccount:
        return String(localized: "mobile.ios.rssFeedsCanOnlyMoveToRss")
    case .mailThreadToRssAccount:
        return String(localized: "mobile.ios.mailThreadsCannotMoveIntoRss")
    }
}

func kanbanStarredItemsMatching(_ items: [StarredItemSummary], query: String) -> [StarredItemSummary] {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        return items.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
    }
    let needle = trimmed.lowercased()
    return items
        .filter { item in
            [
                item.subject,
                item.sender,
                item.preview,
                item.accountId,
                item.folder,
            ].contains { $0.localizedCaseInsensitiveContains(needle) }
        }
        .sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
}
