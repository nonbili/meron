import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func loadCoreFoldersAndThreads(accountId: String, requestedFolder: String? = nil) {
        if accountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
            return
        }
        let folderResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 14, params: FolderListParams(accountId: accountId)).toJson()
        )
        let parsedFolders = MobileResponseParsersKt.parseFolderListResponse(responseJson: folderResponse)
        coreFolders = parsedFolders
        accountInboxUnreadCounts[accountId] = inboxUnreadCount(folders: parsedFolders, accountId: accountId)
        let fallbackFolder = parsedFolders.first?.name ?? "inbox"
        let folderId = parsedFolders.contains(where: { $0.name == (requestedFolder ?? selectedCoreFolder) })
            ? (requestedFolder ?? selectedCoreFolder)
            : fallbackFolder
        selectedCoreFolder = folderId
        loadCoreThreads(accountId: accountId, folderId: folderId)
    }

    func unifiedMailboxAccounts(for cursors: [String: String]? = nil) -> [AccountSummary] {
        let accounts = coreAccounts.filter(\.includedInUnified)
        guard let cursors else { return accounts }
        return accounts.filter { !(cursors[$0.id] ?? "").isEmpty }
    }

    func mailThreadListPage(accountId: String, folderId: String, beforeCursor: String?) -> ThreadListPage? {
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: folderId,
            query: mailSearch.trimmingCharacters(in: .whitespacesAndNewlines),
            filter: mailFilter.rawValue,
            beforeCursor: beforeCursor,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 16, params: threadParams).toJson())
        accountJson = threadResponse
        if threadResponse.contains(#""error""#) {
            return nil
        }
        return MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
    }

    func loadUnifiedInbox(syncFirst: Bool) {
        let accounts = unifiedMailboxAccounts()
        guard !accounts.isEmpty else {
            coreFolders = []
            coreThreads = []
            selectedCoreThread = nil
            coreMessages = []
            mailboxCursor = ""
            mailboxAccountCursors = [:]
            selectedCoreFolder = "inbox"
            accountStatus = String(localized: "mobile.ios.noAccountsInUnifiedInbox")
            return
        }

        if syncFirst {
            for account in accounts {
                if !syncKanbanAccount(account, folderId: iosInboxFolderId) {
                    accountStatus = String(localized: "mobile.ios.coreSyncFailed")
                    return
                }
            }
            refreshAccountInboxUnreadCounts()
        }

        var threads: [ThreadSummary] = []
        var nextCursors: [String: String] = [:]
        for account in accounts {
            guard let page = mailThreadListPage(accountId: account.id, folderId: iosInboxFolderId, beforeCursor: nil) else {
                accountStatus = String(localized: "mobile.ios.unifiedInboxLoadFailed")
                return
            }
            threads.append(contentsOf: page.threads)
            if mailSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, !page.nextCursor.isEmpty {
                nextCursors[account.id] = page.nextCursor
            }
        }

        coreFolders = []
        selectedCoreFolder = iosInboxFolderId
        coreThreads = threads.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxCursor = ""
        mailboxAccountCursors = nextCursors
        if let selectedCoreThread, !coreThreads.contains(where: { $0.id == selectedCoreThread.id }) {
            self.selectedCoreThread = nil
            coreMessages = []
        }
        accountStatus = localizedCatalogString(
            "mobile.ios.loadedFilteredThreadCountFromUnifiedInbox",
            args: ["count": coreThreads.count, "filter": mailFilter.label.lowercased()]
        )
    }

    func loadCoreThreads(accountId: String, folderId: String) {
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: folderId,
            query: mailSearch.trimmingCharacters(in: .whitespacesAndNewlines),
            filter: mailFilter.rawValue,
            beforeCursor: nil,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 16, params: threadParams).toJson())
        let page = MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
        coreThreads = page.threads
        mailboxCursor = page.nextCursor
        mailboxAccountCursors = [:]
        accountJson = threadResponse
        if let selectedCoreThread, !coreThreads.contains(where: { $0.id == selectedCoreThread.id }) {
            self.selectedCoreThread = nil
            coreMessages = []
        }
        accountStatus = localizedCatalogString(
            "mobile.ios.loadedFilteredThreadCountFromFolder",
            args: ["count": coreThreads.count, "filter": mailFilter.label.lowercased(), "folder": folderId]
        )
    }

    func loadMoreCoreThreads() {
        if selectedMailboxAccountId() == iosUnifiedAccountId {
            loadMoreUnifiedInbox()
            return
        }
        guard !mailboxCursor.isEmpty, !isLoadingMoreThreads else { return }
        let accountId = selectedMailboxAccountId()
        guard !accountId.isEmpty else { return }
        isLoadingMoreThreads = true
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: selectedCoreFolder.isEmpty ? "inbox" : selectedCoreFolder,
            query: mailSearch.trimmingCharacters(in: .whitespacesAndNewlines),
            filter: mailFilter.rawValue,
            beforeCursor: mailboxCursor,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 46, params: threadParams).toJson())
        isLoadingMoreThreads = false
        accountJson = threadResponse
        if threadResponse.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.loadOlderFailed")
            return
        }
        let page = MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
        let existingIds = Set(coreThreads.map(\.id))
        let appended = page.threads.filter { !existingIds.contains($0.id) }
        coreThreads = (coreThreads + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxCursor = page.nextCursor
        accountStatus = appended.isEmpty
            ? String(localized: "mobile.ios.noOlderMessages")
            : localizedCatalogString("mobile.ios.loadedOlderMessageCount", args: ["count": appended.count])
    }

    func loadMoreUnifiedInbox() {
        guard !mailboxAccountCursors.isEmpty, !isLoadingMoreThreads else { return }
        let cursors = mailboxAccountCursors
        let accounts = unifiedMailboxAccounts(for: cursors)
        guard !accounts.isEmpty else { return }
        isLoadingMoreThreads = true
        var threads: [ThreadSummary] = []
        var nextCursors: [String: String] = [:]
        for account in accounts {
            guard let page = mailThreadListPage(accountId: account.id, folderId: iosInboxFolderId, beforeCursor: cursors[account.id]) else {
                isLoadingMoreThreads = false
                accountStatus = String(localized: "mobile.ios.loadOlderFailed")
                return
            }
            threads.append(contentsOf: page.threads)
            if !page.nextCursor.isEmpty {
                nextCursors[account.id] = page.nextCursor
            }
        }
        isLoadingMoreThreads = false
        let existingIds = Set(coreThreads.map(\.id))
        let appended = threads.filter { !existingIds.contains($0.id) }
        coreThreads = (coreThreads + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxAccountCursors = nextCursors
        mailboxCursor = ""
        accountStatus = appended.isEmpty
            ? String(localized: "mobile.ios.noOlderMessages")
            : localizedCatalogString("mobile.ios.loadedOlderMessageCount", args: ["count": appended.count])
    }

    func loadStarredItems() {
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.starredItemsRequest(id: 40).toJson())
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.starredLoadFailed")
            accountJson = response
            return
        }
        starredItems = MobileResponseParsersKt.parseStarredItemsResponse(responseJson: response)
        accountJson = response
        accountStatus = localizedCatalogString("mobile.ios.loadedStarredItemCount", args: ["count": starredItems.count])
    }

    func readThread(_ thread: ThreadSummary) {
        selectedCoreThread = thread
        messageCursor = ""
        isLoadingMoreMessages = false
        threadSearch = ""
        threadSearchPresented = false
        conversationDetailsPresented = false
        activeThreadSearchIndex = 0
        let response: String
        if isRssThread(thread) {
            let params = RssThreadParams(threadId: thread.id, beforeCursor: nil, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.rssThreadRequest(id: 21, params: params).toJson())
        } else {
            let params = ThreadReadParams(threadId: thread.id, beforeCursor: nil, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.threadReadRequest(id: 21, params: params).toJson())
        }
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.threadReadFailed")
            accountJson = response
            return
        }
        let page = MobileResponseParsersKt.parseThreadReadPage(responseJson: response)
        coreMessages = page.messages
        messageCursor = page.nextCursor
        accountJson = response
        accountStatus = localizedCatalogString("mobile.ios.loadedMessageCount", args: ["count": coreMessages.count])
        if !isRssThread(thread),
           MailStateKt.folderIsDrafts(folder: thread.folder),
           let draftMessage = coreMessages.last
        {
            openDraftCompose(draftMessage, thread: thread)
        }
    }

    func openNotificationThread(_ target: IosNotificationThreadTarget) {
        mailSearch = ""
        mailFilter = .all
        selectedTab = .mail
        selectedCoreAccountId = target.accountId
        selectedCoreFolder = target.folder
        selectedMailThreadIds = []
        let accounts: [AccountSummary]
        if coreAccounts.isEmpty {
            let response = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 92).toJson())
            if response.contains(#""error""#) {
                accountStatus = String(localized: "mobile.ios.accountLoadFailed")
                accountJson = response
                return
            }
            accounts = MobileResponseParsersKt.parseAccountListResponse(responseJson: response)
            coreAccounts = accounts
        } else {
            accounts = coreAccounts
        }
        guard let account = accounts.first(where: { $0.id == target.accountId }) else {
            accountStatus = String(localized: "mobile.ios.noAccountSelected")
            return
        }

        loadCoreFoldersAndThreads(accountId: target.accountId, requestedFolder: target.folder)
        let expectedThreadId = iosNotificationThreadId(target)
        if let thread = coreThreads.first(where: { $0.id == expectedThreadId }) {
            readThread(thread)
            return
        }
        if syncKanbanAccount(account, folderId: target.folder) {
            loadCoreFoldersAndThreads(accountId: target.accountId, requestedFolder: target.folder)
        }
        guard let thread = coreThreads.first(where: { $0.id == expectedThreadId }) else {
            accountStatus = String(localized: "mobile.ios.threadReadFailed")
            return
        }
        readThread(thread)
    }

    func handleOpenedNotification(_ notification: Notification) {
        if let target = iosNotificationThreadTarget(userInfo: notification.userInfo ?? [:]) {
            openNotificationThread(target)
        }
    }

    func installNotificationOpenObserver() {
        guard notificationOpenObserver == nil else { return }
        notificationOpenObserver = NotificationCenter.default.addObserver(
            forName: .iosNotificationThreadTargetOpened,
            object: nil,
            queue: .main
        ) { notification in
            handleOpenedNotification(notification)
        }
    }

    func installCoreEventHandler() {
        guard !coreEventHandlerInstalled else { return }
        coreEventHandlerInstalled = true
        RustCoreBridge.setEventHandler { eventJson in
            handleCoreEvent(eventJson)
        }
    }

    func handleCoreEvent(_ eventJson: String) {
        guard let data = eventJson.data(using: .utf8),
              let envelope = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let event = envelope["event"] as? String
        else {
            return
        }
        switch event {
        case "mail.newMessages":
            let detail = envelope["detail"] as? [String: Any] ?? [:]
            clearCoreSyncError(accountId: detail["account"] as? String)
            if !liveMailPushEnabled, detail["muted"] as? Bool != true {
                IosNotificationService.notifyNewMail(
                    accountName: detail["accountName"] as? String ?? detail["account"] as? String ?? "",
                    from: detail["from"] as? String ?? "",
                    subject: detail["subject"] as? String ?? "",
                    count: detail["count"] as? Int ?? 1,
                    accountId: detail["account"] as? String ?? "",
                    folder: detail["folder"] as? String ?? "",
                    threadKey: detail["threadKey"] as? String ?? ""
                )
            }
            refreshCurrentMailboxFromCoreEvent()
        case "mail.synced":
            let detail = envelope["detail"] as? [String: Any] ?? [:]
            clearCoreSyncError(accountId: detail["account"] as? String)
            if detail["folders"] as? Bool == true {
                refreshAccountInboxUnreadCounts()
            }
            refreshCurrentMailboxFromCoreEvent()
        case "mail.syncError":
            let detail = envelope["detail"] as? [String: Any] ?? [:]
            coreSyncErrorAccountId = detail["account"] as? String ?? ""
            coreSyncErrorMessage = detail["message"] as? String ?? String(localized: "mobile.ios.syncFailed")
            accountStatus = coreSyncErrorMessage
        default:
            break
        }
    }

    func refreshCurrentMailboxFromCoreEvent() {
        guard !coreAccounts.isEmpty else { return }
        let accountId = selectedMailboxAccountId()
        guard !accountId.isEmpty else { return }
        if accountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
        if selectedTab == .starred {
            loadStarredItems()
        } else if selectedTab == .kanban {
            loadActiveKanbanBoard(refresh: false)
        }
    }

    func clearCoreSyncError(accountId: String?) {
        if !coreSyncErrorAccountId.isEmpty, let accountId, coreSyncErrorAccountId != accountId {
            return
        }
        coreSyncErrorAccountId = ""
        coreSyncErrorMessage = ""
    }

    func handleCoreSyncErrorAction() {
        if iosSyncErrorLooksAuthRelated(coreSyncErrorMessage),
           let account = coreAccounts.first(where: { $0.id == coreSyncErrorAccountId })
                ?? selectedMailboxAccountForReconnect()
        {
            reconnectAccount(account)
            return
        }
        syncSelectedAccount()
    }

    func selectedMailboxAccountForReconnect() -> AccountSummary? {
        let accountId = selectedMailboxAccountId()
        guard accountId != iosUnifiedAccountId else { return nil }
        return coreAccounts.first { $0.id == accountId }
    }

    func loadMoreThreadMessages() {
        guard let thread = selectedCoreThread,
              !messageCursor.isEmpty,
              !isLoadingMoreMessages else { return }
        let cursor = messageCursor
        isLoadingMoreMessages = true
        let response: String
        if isRssThread(thread) {
            let params = RssThreadParams(threadId: thread.id, beforeCursor: cursor, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.rssThreadRequest(id: 47, params: params).toJson())
        } else {
            let params = ThreadReadParams(threadId: thread.id, beforeCursor: cursor, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.threadReadRequest(id: 47, params: params).toJson())
        }
        isLoadingMoreMessages = false
        accountJson = response
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.loadOlderMessagesFailed")
            return
        }
        let page = MobileResponseParsersKt.parseThreadReadPage(responseJson: response)
        let existingIds = Set(coreMessages.map(\.id))
        let older = page.messages.filter { !existingIds.contains($0.id) }
        coreMessages = (older + coreMessages).sorted { $0.dateEpochSeconds < $1.dateEpochSeconds }
        messageCursor = page.nextCursor
        accountStatus = older.isEmpty
            ? String(localized: "mobile.ios.noOlderMessagesInThread")
            : localizedCatalogString("mobile.ios.loadedOlderMessageCount", args: ["count": older.count])
    }

    func readStarredItem(_ item: StarredItemSummary) {
        starredMessageScrollTarget = ""
        selectedCoreAccountId = item.accountId
        selectedCoreFolder = item.folder
        if isRssStarredItem(item) {
            selectedTab = .mail
            messageReaderTarget = starredItemReaderMessage(for: item)
            return
        }
        readThread(
            ThreadSummary(
                id: item.threadId,
                accountId: item.accountId,
                folder: item.folder,
                subject: item.subject,
                sender: item.sender,
                preview: item.preview,
                unread: item.unread,
                starred: true,
                dateEpochSeconds: item.dateEpochSeconds,
                feedUrl: ""
            )
        )
        selectedTab = .mail
        DispatchQueue.main.async {
            starredMessageScrollTarget = item.id
        }
    }
}
