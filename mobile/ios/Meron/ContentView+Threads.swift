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
            accountStatus = "No accounts are included in Unified inbox."
            return
        }

        if syncFirst {
            for account in accounts {
                if !syncKanbanAccount(account, folderId: iosInboxFolderId) {
                    accountStatus = "Core sync failed."
                    return
                }
            }
        }

        var threads: [ThreadSummary] = []
        var nextCursors: [String: String] = [:]
        for account in accounts {
            guard let page = mailThreadListPage(accountId: account.id, folderId: iosInboxFolderId, beforeCursor: nil) else {
                accountStatus = "Unified inbox load failed."
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
        accountStatus = "Loaded \(coreThreads.count) \(mailFilter.label.lowercased()) thread(s) from Unified inbox."
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
        accountStatus = "Loaded \(coreThreads.count) \(mailFilter.label.lowercased()) thread(s) from \(folderId)."
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
            accountStatus = "Load older failed."
            return
        }
        let page = MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
        let existingIds = Set(coreThreads.map(\.id))
        let appended = page.threads.filter { !existingIds.contains($0.id) }
        coreThreads = (coreThreads + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxCursor = page.nextCursor
        accountStatus = appended.isEmpty ? "No older messages." : "Loaded \(appended.count) older message(s)."
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
                accountStatus = "Load older failed."
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
        accountStatus = appended.isEmpty ? "No older messages." : "Loaded \(appended.count) older message(s)."
    }

    func loadStarredItems() {
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.starredItemsRequest(id: 40).toJson())
        if response.contains(#""error""#) {
            accountStatus = "Starred load failed."
            accountJson = response
            return
        }
        starredItems = MobileResponseParsersKt.parseStarredItemsResponse(responseJson: response)
        accountJson = response
        accountStatus = "Loaded \(starredItems.count) starred item(s)."
    }

    func readThread(_ thread: ThreadSummary) {
        selectedCoreThread = thread
        messageCursor = ""
        isLoadingMoreMessages = false
        threadSearch = ""
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
            accountStatus = "Thread read failed."
            accountJson = response
            return
        }
        let page = MobileResponseParsersKt.parseThreadReadPage(responseJson: response)
        coreMessages = page.messages
        messageCursor = page.nextCursor
        accountJson = response
        accountStatus = "Loaded \(coreMessages.count) message(s)."
        if !isRssThread(thread),
           MailStateKt.folderIsDrafts(folder: thread.folder),
           let draftMessage = coreMessages.last
        {
            openDraftCompose(draftMessage, thread: thread)
        }
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
            accountStatus = "Load older messages failed."
            return
        }
        let page = MobileResponseParsersKt.parseThreadReadPage(responseJson: response)
        let existingIds = Set(coreMessages.map(\.id))
        let older = page.messages.filter { !existingIds.contains($0.id) }
        coreMessages = (older + coreMessages).sorted { $0.dateEpochSeconds < $1.dateEpochSeconds }
        messageCursor = page.nextCursor
        accountStatus = older.isEmpty ? "No older messages in this thread." : "Loaded \(older.count) older message(s)."
    }

    func readStarredItem(_ item: StarredItemSummary) {
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
    }
}
