import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func openMessageAttachment(_ attachment: MessageAttachment) {
        if !attachment.url.isEmpty, let url = URL(string: attachment.url) {
            UIApplication.shared.open(url)
            return
        }
        guard !attachment.key.isEmpty else {
            accountStatus = "Attachment is not cached."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.attachmentReadRequest(
                id: 64,
                params: AttachmentReadParams(key: attachment.key)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Attachment open failed."
            accountJson = response
            return
        }
        let base64 = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
        guard let data = Data(base64Encoded: base64) else {
            accountStatus = "Attachment data is invalid."
            return
        }
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("MeronAttachments", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent(safeAttachmentFilename(attachment.filename))
            try data.write(to: url, options: .atomic)
            if attachment.mimeType.hasPrefix("image/"), let image = UIImage(data: data) {
                imagePreview = IosImagePreview(
                    title: attachment.filename.isEmpty ? "Image" : attachment.filename,
                    image: image,
                    url: url
                )
                return
            }
            shareableAttachment = ShareableFile(url: url)
            accountStatus = "Choose Save to Files or share this attachment."
        } catch {
            accountStatus = "Attachment open failed: \(error.localizedDescription)"
        }
    }

    func openMessageCompose(_ message: MessageBody, forward: Bool) {
        var copiedAttachments: [DraftAttachment] = []
        for attachment in MailStateKt.forwardableAttachments(message: message) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.attachmentReadRequest(
                    id: 58,
                    params: AttachmentReadParams(key: attachment.key)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = forward ? "Forward attachment copy failed." : "Edit-as-new attachment copy failed."
                accountJson = response
                return
            }
            let data = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
            if !data.isEmpty {
                copiedAttachments.append(
                    MailStateKt.attachmentToDraftAttachment(attachment: attachment, dataBase64: data)
                )
            }
        }
        let draft = forward
            ? MailStateKt.messageForwardDraft(message: message, attachments: copiedAttachments)
            : MailStateKt.messageEditAsNewDraft(message: message, attachments: copiedAttachments)
        composeTo = draft.to
        composeCc = draft.cc
        composeBcc = draft.bcc
        composeSubject = draft.subject
        composeBody = draft.body
        attachments = draft.attachments
        composeFromAccountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        composeFromEmail = ""
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = ""
        composeReferences = ""
        selectedTab = .compose
        accountStatus = forward ? "Forward draft ready." : "Copied message into compose."
    }

    func openDraftCompose(_ message: MessageBody, thread: ThreadSummary) {
        var copiedAttachments: [DraftAttachment] = []
        for attachment in MailStateKt.forwardableAttachments(message: message) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.attachmentReadRequest(
                    id: 63,
                    params: AttachmentReadParams(key: attachment.key)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "Draft attachment copy failed."
                accountJson = response
                return
            }
            let data = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
            if !data.isEmpty {
                copiedAttachments.append(MailStateKt.attachmentToDraftAttachment(attachment: attachment, dataBase64: data))
            }
        }
        composeTo = message.to
        composeCc = message.cc
        composeBcc = message.bcc
        composeSubject = message.subject
        composeBody = message.body
        attachments = copiedAttachments
        composeFromAccountId = thread.accountId
        composeFromEmail = ""
        let cleanedMessageId = message.messageId.trimmingCharacters(in: CharacterSet(charactersIn: "<> \n\t"))
        composeDraftId = cleanedMessageId.isEmpty ? newIosDraftMessageId(accountId: thread.accountId) : cleanedMessageId
        composeDraftSaved = true
        composeInReplyTo = ""
        composeReferences = ""
        selectedTab = .compose
        accountStatus = "Draft ready."
    }

    func archiveThread(_ thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: nil, messageIds: [])
        runThreadAction(
            requestJson: MobileCommandsKt.archiveThreadRequest(id: 17, params: params).toJson(),
            successStatus: "Archive complete."
        )
    }

    func deleteThread(_ thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        runThreadAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 18, params: params).toJson(),
            successStatus: threadDeleteSuccessStatus(thread)
        )
    }

    func presentMoveThreadDialog(_ thread: ThreadSummary) {
        guard !isRssThread(thread) else {
            accountStatus = "RSS feeds move between RSS accounts from Kanban."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 65, params: FolderListParams(accountId: thread.accountId)).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Folder list failed."
            accountJson = response
            return
        }
        let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
            .filter { $0.name.caseInsensitiveCompare(thread.folder) != .orderedSame }
        moveThreadFolders = folders
        moveThreadTarget = thread
        moveThreadDialogPresented = true
    }

    func moveThread(_ thread: ThreadSummary, toFolder folder: String) {
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.moveThreadRequest(
                id: 66,
                params: MoveThreadParams(threadId: thread.id, targetFolderId: folder)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Move failed."
            accountJson = response
            return
        }
        coreThreads.removeAll { $0.id == thread.id }
        if selectedCoreThread?.id == thread.id {
            selectedCoreThread = nil
            coreMessages = []
            messageCursor = ""
            threadSearch = ""
            activeThreadSearchIndex = 0
        }
        accountStatus = "Move complete."
    }

    func presentCopyThreadDialog(_ thread: ThreadSummary) {
        guard !isRssThread(thread) else {
            accountStatus = "RSS feeds can't be copied to mail folders."
            return
        }
        var targets: [FolderSummary] = []
        for account in coreAccounts where !MailStateKt.accountSummaryIsRss(account: account) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.folderListRequest(id: 85, params: FolderListParams(accountId: account.id)).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "Folder list failed."
                accountJson = response
                return
            }
            targets.append(contentsOf: MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
                .filter { folder in
                    !(folder.accountId == thread.accountId && folder.name.caseInsensitiveCompare(thread.folder) == .orderedSame)
                })
        }
        copyThreadFolders = targets
        copyThreadTarget = thread
        copyThreadDialogPresented = true
    }

    func copyTargetLabel(_ folder: FolderSummary) -> String {
        let account = coreAccounts.first(where: { $0.id == folder.accountId })
        let accountLabel = account?.displayName.isEmpty == false ? account?.displayName : account?.email
        guard let accountLabel, !accountLabel.isEmpty else {
            return folder.name
        }
        return "\(accountLabel) / \(folder.name)"
    }

    func copyThread(_ thread: ThreadSummary, toFolder folder: FolderSummary) {
        let targetAccountId = folder.accountId.isEmpty ? thread.accountId : folder.accountId
        let params = CopyThreadParams(
            threadId: thread.id,
            targetAccountId: targetAccountId,
            targetFolderId: folder.name
        )
        let response = RustCoreBridge.invokeJson(
            CoreRequest(id: 86, method: "mail.copy", paramsJson: params.toJson()).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Copy failed."
            accountJson = response
            return
        }
        copyThreadDialogPresented = false
        copyThreadTarget = nil
        accountStatus = "Copy complete."
    }

    func createFolderAndMoveThread(_ thread: ThreadSummary, name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            accountStatus = "Folder name is required."
            return
        }
        let createResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderCreateRequest(
                id: 67,
                params: FolderCreateParams(accountId: thread.accountId, name: trimmed)
            ).toJson()
        )
        if createResponse.contains(#""error""#) {
            accountStatus = "Create folder failed."
            accountJson = createResponse
            return
        }

        let listResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 68, params: FolderListParams(accountId: thread.accountId)).toJson()
        )
        let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: listResponse)
        if selectedCoreAccountId == thread.accountId {
            coreFolders = folders
        }
        moveThreadFolders = folders.filter { $0.name.caseInsensitiveCompare(thread.folder) != .orderedSame }
        let createdName = folders.first { $0.name.caseInsensitiveCompare(trimmed) == .orderedSame }?.name ?? trimmed
        moveThreadNewFolderName = ""
        moveThreadDialogPresented = false
        moveThreadCreateDialogPresented = false
        moveThread(thread, toFolder: createdName)
    }

    func removeRssFeed(_ thread: ThreadSummary) {
        let params = RemoveRssFeedParams(threadId: thread.id)
        runThreadAction(
            requestJson: MobileCommandsKt.feedRemoveRequest(id: 18, params: params).toJson(),
            successStatus: "Removed feed."
        )
    }

    func markThreadRead(_ thread: ThreadSummary, seen: Bool) {
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkReadParams(threadId: thread.id, seen: seen, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkReadRequest(id: 19, params: params).toJson()
        } else {
            let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [])
            requestJson = MobileCommandsKt.markReadRequest(id: 19, params: params).toJson()
        }
        runThreadAction(requestJson: requestJson, successStatus: seen ? "Marked read." : "Marked unread.")
    }

    func markThreadStarred(_ thread: ThreadSummary, starred: Bool) {
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkStarredParams(threadId: thread.id, starred: starred, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkStarredRequest(id: 20, params: params).toJson()
        } else {
            let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [])
            requestJson = MobileCommandsKt.markStarredRequest(id: 20, params: params).toJson()
        }
        runThreadAction(requestJson: requestJson, successStatus: starred ? "Starred." : "Unstarred.")
    }

    func toggleMessageRead(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let seen = message.unread
        let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.markReadRequest(id: 87, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Message update failed."
            accountJson = response
            return
        }
        coreMessages = coreMessages.map { $0.id == message.id ? $0.withFlags(unread: !seen) : $0 }
        let updatedUnread = coreMessages.contains(where: { $0.unread })
        selectedCoreThread = selectedCoreThread?.withUnread(updatedUnread)
        coreThreads = coreThreads.map { $0.id == thread.id ? $0.withUnread(updatedUnread) : $0 }
        accountStatus = seen ? "Marked read." : "Marked unread."
    }

    func toggleMessageStarred(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let starred = !message.starred
        let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.markStarredRequest(id: 88, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Star failed."
            accountJson = response
            return
        }
        coreMessages = coreMessages.map { $0.id == message.id ? $0.withFlags(starred: starred) : $0 }
        let updatedStarred = coreMessages.contains(where: { $0.starred })
        selectedCoreThread = selectedCoreThread.map { thread in
            ThreadSummary(
                id: thread.id,
                accountId: thread.accountId,
                folder: thread.folder,
                subject: thread.subject,
                sender: thread.sender,
                preview: thread.preview,
                unread: thread.unread,
                starred: updatedStarred,
                dateEpochSeconds: thread.dateEpochSeconds,
                feedUrl: thread.feedUrl
            )
        }
        coreThreads = coreThreads.map { thread in
            thread.id == selectedCoreThread?.id ? ThreadSummary(
                id: thread.id,
                accountId: thread.accountId,
                folder: thread.folder,
                subject: thread.subject,
                sender: thread.sender,
                preview: thread.preview,
                unread: thread.unread,
                starred: updatedStarred,
                dateEpochSeconds: thread.dateEpochSeconds,
                feedUrl: thread.feedUrl
            ) : thread
        }
        accountStatus = starred ? "Starred." : "Unstarred."
    }

    func deleteMessage(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.deleteThreadRequest(id: 89, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Delete failed."
            accountJson = response
            return
        }
        coreMessages.removeAll { $0.id == message.id }
        accountStatus = "Delete complete."
    }

    func markStarredItemRead(_ item: StarredItemSummary, seen: Bool) {
        let requestJson: String
        if isRssStarredItem(item) {
            let params = RssMarkReadParams(threadId: item.threadId, seen: seen, itemKeys: [item.id])
            requestJson = MobileCommandsKt.rssMarkReadRequest(id: 41, params: params).toJson()
        } else {
            let params = MarkReadParams(threadId: item.threadId, seen: seen, messageIds: [item.id])
            requestJson = MobileCommandsKt.markReadRequest(id: 41, params: params).toJson()
        }
        runStarredItemAction(requestJson: requestJson, successStatus: seen ? "Marked read." : "Marked unread.")
    }

    func unstarStarredItem(_ item: StarredItemSummary) {
        let requestJson: String
        if isRssStarredItem(item) {
            let params = RssMarkStarredParams(threadId: item.threadId, starred: false, itemKeys: [item.id])
            requestJson = MobileCommandsKt.rssMarkStarredRequest(id: 42, params: params).toJson()
        } else {
            let params = MarkStarredParams(threadId: item.threadId, starred: false, messageIds: [item.id])
            requestJson = MobileCommandsKt.markStarredRequest(id: 42, params: params).toJson()
        }
        runStarredItemAction(requestJson: requestJson, successStatus: "Unstarred.")
    }

    func deleteStarredMailItem(_ item: StarredItemSummary) {
        guard !isRssStarredItem(item) else {
            accountStatus = "RSS items cannot be deleted."
            return
        }
        let params = ThreadActionParams(threadId: item.threadId, folderId: item.folder, messageIds: [item.id])
        runStarredItemAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 43, params: params).toJson(),
            successStatus: threadDeleteSuccessStatus(folder: item.folder)
        )
    }

    func runStarredItemAction(requestJson: String, successStatus: String) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Starred item action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        loadStarredItems()
    }

    func runThreadAction(requestJson: String, successStatus: String) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Thread action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        if !selectedCoreAccountId.isEmpty {
            loadCoreFoldersAndThreads(accountId: selectedCoreAccountId, requestedFolder: selectedCoreFolder)
        }
    }

    var activeKanbanBoard: IosKanbanBoardSpec? {
        kanbanBoards.first(where: { $0.id == activeKanbanBoardId }) ?? kanbanBoards.first
    }

    func persistKanbanBoards() {
        if let data = try? JSONEncoder().encode(kanbanBoards) {
            UserDefaults.standard.set(data, forKey: "ios_kanban_boards_v1")
        }
        if activeKanbanBoardId.isEmpty || !kanbanBoards.contains(where: { $0.id == activeKanbanBoardId }) {
            activeKanbanBoardId = kanbanBoards.first?.id ?? ""
        }
        UserDefaults.standard.set(activeKanbanBoardId, forKey: "ios_kanban_active_board_v1")
    }

    func ensureKanbanDefaults() {
        guard !coreAccounts.isEmpty else { return }
        if kanbanSelectedAccountId.isEmpty {
            kanbanSelectedAccountId = coreAccounts.first?.id ?? ""
        }
        if kanbanBoards.isEmpty {
            var seen = Set<String>()
            let columns = ([IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)] + coreAccounts.map { IosKanbanColumnSpec(accountId: $0.id, folderId: iosInboxFolderId) })
                .filter { seen.insert($0.id).inserted }
            let board = IosKanbanBoardSpec(id: UUID().uuidString, name: "Kanban board", columns: columns)
            kanbanBoards = [board]
            activeKanbanBoardId = board.id
            persistKanbanBoards()
        } else if let first = kanbanBoards.first {
            var existing = Set(first.columns.map(\.id))
            var columns = first.columns
            let unified = IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
            if existing.insert(unified.id).inserted {
                columns.insert(unified, at: 0)
            }
            for account in coreAccounts {
                let column = IosKanbanColumnSpec(accountId: account.id, folderId: iosInboxFolderId)
                if existing.insert(column.id).inserted {
                    columns.append(column)
                }
            }
            if columns != first.columns {
                kanbanBoards = kanbanBoards.map { board in
                    if board.id != first.id { return board }
                    var next = board
                    next.columns = columns
                    return next
                }
                persistKanbanBoards()
            }
        }
        if kanbanNewBoardName.isEmpty {
            kanbanNewBoardName = activeKanbanBoard?.name ?? ""
        }
        if kanbanBoardAvatarUrl.isEmpty {
            kanbanBoardAvatarUrl = activeKanbanBoard?.avatarUrl ?? ""
        }
        if kanbanBoardWallpaperPresetId.isEmpty {
            kanbanBoardWallpaperPresetId = activeKanbanBoard?.wallpaperPresetId ?? ""
        }
        if kanbanBoardWallpaperUrl.isEmpty {
            kanbanBoardWallpaperUrl = activeKanbanBoard?.wallpaperUrl ?? ""
        }
    }

    func createKanbanBoard() {
        let initialColumns = [IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)]
        let board = IosKanbanBoardSpec(id: UUID().uuidString, name: "Board \(kanbanBoards.count + 1)", columns: initialColumns)
        kanbanBoards.append(board)
        activeKanbanBoardId = board.id
        kanbanNewBoardName = board.name
        kanbanBoardAvatarUrl = ""
        kanbanBoardWallpaperPresetId = ""
        kanbanBoardWallpaperUrl = ""
        persistKanbanBoards()
        loadActiveKanbanBoard(refresh: false)
    }

    func renameActiveKanbanBoard() {
        let name = kanbanNewBoardName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return }
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            var next = board
            next.name = name
            return next
        }
        persistKanbanBoards()
    }

    func updateActiveKanbanBoardAppearance() {
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            var next = board
            next.avatarUrl = kanbanBoardAvatarUrl.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
            next.wallpaperPresetId = kanbanBoardWallpaperPresetId.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
            next.wallpaperUrl = kanbanBoardWallpaperUrl.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
            return next
        }
        persistKanbanBoards()
        accountStatus = "Saved board style."
    }

    func deleteActiveKanbanBoard() {
        guard !kanbanBoards.isEmpty else { return }
        kanbanBoards.removeAll { $0.id == activeKanbanBoardId }
        if kanbanBoards.isEmpty {
            activeKanbanBoardId = ""
            kanbanThreadsByColumn = [:]
            kanbanStatusByColumn = [:]
        }
        persistKanbanBoards()
    }

    func addKanbanColumn(accountId: String, folderId: String) {
        guard !accountId.isEmpty, !folderId.isEmpty, let board = activeKanbanBoard else { return }
        let column = IosKanbanColumnSpec(accountId: accountId, folderId: folderId)
        guard !board.columns.contains(column) else { return }
        kanbanBoards = kanbanBoards.map {
            if $0.id != board.id { return $0 }
            var next = $0
            next.columns.append(column)
            return next
        }
        persistKanbanBoards()
        loadKanbanColumn(column, refresh: true)
    }

    func removeKanbanColumn(_ column: IosKanbanColumnSpec) {
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            var next = board
            next.columns = board.columns.filter { $0.id != column.id }
            return next
        }
        kanbanThreadsByColumn[column.id] = nil
        kanbanStatusByColumn[column.id] = nil
        kanbanCursorByColumn[column.id] = nil
        kanbanAccountCursorsByColumn[column.id] = nil
        kanbanLoadingMoreColumns.remove(column.id)
        persistKanbanBoards()
    }

    func loadFoldersForKanbanAccount(_ accountId: String) {
        guard !accountId.isEmpty else { return }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 31, params: FolderListParams(accountId: accountId)).toJson()
        )
        let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
        coreFolders = folders
        kanbanSelectedFolderId = folders.first?.name ?? "inbox"
    }

    func createFolderAndColumn() {
        let accountId = kanbanSelectedAccountId
        let name = kanbanCreateFolderName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !accountId.isEmpty, !name.isEmpty else {
            accountStatus = "Choose an account and folder name."
            return
        }
        let request = MobileCommandsKt.folderCreateRequest(
            id: 32,
            params: FolderCreateParams(accountId: accountId, name: name)
        ).toJson()
        let response = RustCoreBridge.invokeJson(request)
        if response.contains(#""error""#) {
            accountStatus = "Create folder failed."
            accountJson = response
            return
        }
        kanbanCreateFolderName = ""
        loadFoldersForKanbanAccount(accountId)
        addKanbanColumn(accountId: accountId, folderId: name)
        accountStatus = "Folder created."
    }

    func loadActiveKanbanBoard(refresh: Bool) {
        activeKanbanBoard?.columns.forEach { loadKanbanColumn($0, refresh: refresh) }
    }

    func unifiedKanbanAccounts(for cursors: [String: String]? = nil) -> [AccountSummary] {
        let accounts = coreAccounts.filter { $0.includedInUnified }
        guard let cursors else { return accounts }
        return accounts.filter { !(cursors[$0.id] ?? "").isEmpty }
    }

    func syncKanbanAccount(_ account: AccountSummary, folderId: String) -> Bool {
        let response: String
        if MailStateKt.accountSummaryIsRss(account: account) {
            response = RustCoreBridge.invokeJson(
                MobileCommandsKt.syncRssRequest(id: 33, params: SyncRssParams(accountId: account.id)).toJson()
            )
        } else {
            response = RustCoreBridge.invokeJson(
                MobileCommandsKt.syncMailRequest(
                    id: 33,
                    params: SyncMailParams(accountId: account.id, folderId: folderId, limit: 50, folders: true)
                ).toJson()
            )
        }
        if response.contains(#""error""#) {
            accountJson = response
            return false
        }
        return true
    }

    func threadListPage(accountId: String, folderId: String, query: String, beforeCursor: String?) -> ThreadListPage? {
        let threadResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.threadListRequest(
                id: 34,
                params: ThreadListParams(
                    accountId: accountId,
                    folderId: folderId,
                    query: query,
                    filter: kanbanFilter.rawValue,
                    beforeCursor: beforeCursor,
                    refresh: false
                )
            ).toJson()
        )
        if threadResponse.contains(#""error""#) {
            accountJson = threadResponse
            return nil
        }
        return MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
    }

    func loadKanbanColumn(_ column: IosKanbanColumnSpec, refresh: Bool) {
        guard !column.accountId.isEmpty else { return }
        kanbanStatusByColumn[column.id] = refresh ? "Refreshing..." : "Loading..."
        let trimmedQuery = kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines)

        if column.accountId == iosUnifiedAccountId {
            let accounts = unifiedKanbanAccounts()
            if refresh {
                for account in accounts {
                    if !syncKanbanAccount(account, folderId: iosInboxFolderId) {
                        kanbanStatusByColumn[column.id] = "Sync failed"
                        return
                    }
                }
            }

            var threads: [ThreadSummary] = []
            var nextCursors: [String: String] = [:]
            for account in accounts {
                guard let page = threadListPage(accountId: account.id, folderId: iosInboxFolderId, query: trimmedQuery, beforeCursor: nil) else {
                    kanbanStatusByColumn[column.id] = "Load failed"
                    return
                }
                threads.append(contentsOf: page.threads)
                if trimmedQuery.isEmpty && !page.nextCursor.isEmpty {
                    nextCursors[account.id] = page.nextCursor
                }
            }
            kanbanThreadsByColumn[column.id] = threads.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
            kanbanCursorByColumn[column.id] = ""
            kanbanAccountCursorsByColumn[column.id] = nextCursors
            kanbanStatusByColumn[column.id] = "\(threads.count) \(kanbanFilter.label.lowercased()) item(s)"
            return
        }

        let account = coreAccounts.first(where: { $0.id == column.accountId })
        if refresh, let account {
            if !syncKanbanAccount(account, folderId: column.folderId) {
                kanbanStatusByColumn[column.id] = "Sync failed"
                return
            }
        }

        guard let page = threadListPage(accountId: column.accountId, folderId: column.folderId, query: trimmedQuery, beforeCursor: nil) else {
            kanbanStatusByColumn[column.id] = "Load failed"
            return
        }
        kanbanThreadsByColumn[column.id] = page.threads
        kanbanCursorByColumn[column.id] = trimmedQuery.isEmpty ? page.nextCursor : ""
        kanbanAccountCursorsByColumn[column.id] = [:]
        kanbanStatusByColumn[column.id] = "\(page.threads.count) \(kanbanFilter.label.lowercased()) item(s)"
    }

    func loadMoreKanbanColumn(_ column: IosKanbanColumnSpec) {
        guard kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        if column.accountId == iosUnifiedAccountId {
            let cursors = kanbanAccountCursorsByColumn[column.id] ?? [:]
            guard !cursors.isEmpty, !kanbanLoadingMoreColumns.contains(column.id) else { return }
            kanbanLoadingMoreColumns.insert(column.id)
            var threads: [ThreadSummary] = []
            var nextCursors: [String: String] = [:]
            for account in unifiedKanbanAccounts(for: cursors) {
                guard let page = threadListPage(accountId: account.id, folderId: iosInboxFolderId, query: "", beforeCursor: cursors[account.id]) else {
                    kanbanLoadingMoreColumns.remove(column.id)
                    kanbanStatusByColumn[column.id] = "Load older failed"
                    return
                }
                threads.append(contentsOf: page.threads)
                if !page.nextCursor.isEmpty {
                    nextCursors[account.id] = page.nextCursor
                }
            }
            kanbanLoadingMoreColumns.remove(column.id)
            let existing = kanbanThreadsByColumn[column.id] ?? []
            let existingIds = Set(existing.map(\.id))
            let appended = threads.filter { !existingIds.contains($0.id) }
            kanbanThreadsByColumn[column.id] = (existing + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
            kanbanAccountCursorsByColumn[column.id] = nextCursors
            kanbanStatusByColumn[column.id] = appended.isEmpty ? "No older items" : "\(existing.count + appended.count) item(s)"
            return
        }

        guard let cursor = kanbanCursorByColumn[column.id],
              !cursor.isEmpty,
              !kanbanLoadingMoreColumns.contains(column.id) else { return }
        kanbanLoadingMoreColumns.insert(column.id)
        guard let page = threadListPage(accountId: column.accountId, folderId: column.folderId, query: "", beforeCursor: cursor) else {
            kanbanLoadingMoreColumns.remove(column.id)
            kanbanStatusByColumn[column.id] = "Load older failed"
            return
        }
        kanbanLoadingMoreColumns.remove(column.id)
        let existing = kanbanThreadsByColumn[column.id] ?? []
        let existingIds = Set(existing.map(\.id))
        let appended = page.threads.filter { !existingIds.contains($0.id) }
        kanbanThreadsByColumn[column.id] = (existing + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        kanbanCursorByColumn[column.id] = page.nextCursor
        kanbanStatusByColumn[column.id] = appended.isEmpty ? "No older items" : "\(existing.count + appended.count) item(s)"
    }

    func markKanbanColumnAllRead(_ column: IosKanbanColumnSpec) {
        let unreadThreads = (kanbanThreadsByColumn[column.id] ?? []).filter { $0.unread }
        guard !unreadThreads.isEmpty else {
            accountStatus = "No unread cards."
            return
        }

        var failedResponse = ""
        if column.accountId == iosUnifiedAccountId {
            let accountById = Dictionary(uniqueKeysWithValues: coreAccounts.map { ($0.id, $0) })
            let mailAccountIds = Set(unreadThreads
                .map(\.accountId)
                .filter { accountId in
                    accountById[accountId].map { !MailStateKt.accountSummaryIsRss(account: $0) } ?? true
                })
            for accountId in mailAccountIds {
                let request = MobileCommandsKt.markAllReadRequest(
                    id: 49,
                    params: MarkAllReadParams(accountId: accountId, folderId: iosInboxFolderId)
                ).toJson()
                let response = RustCoreBridge.invokeJson(request)
                if response.contains(#""error""#) {
                    failedResponse = response
                    break
                }
            }
        } else if let account = coreAccounts.first(where: { $0.id == column.accountId }),
                  !MailStateKt.accountSummaryIsRss(account: account) {
            let request = MobileCommandsKt.markAllReadRequest(
                id: 49,
                params: MarkAllReadParams(accountId: column.accountId, folderId: column.folderId)
            ).toJson()
            failedResponse = RustCoreBridge.invokeJson(request)
        }

        if failedResponse.isEmpty {
            for thread in unreadThreads where isRssThread(thread) {
                let request = MobileCommandsKt.rssMarkReadRequest(
                    id: 49,
                    params: RssMarkReadParams(threadId: thread.id, seen: true, itemKeys: [])
                ).toJson()
                let response = RustCoreBridge.invokeJson(request)
                if response.contains(#""error""#) {
                    failedResponse = response
                    break
                }
            }
        }

        if failedResponse.contains(#""error""#) {
            accountStatus = "Kanban mark all read failed."
            accountJson = failedResponse
            return
        }

        let unreadIds = Set(unreadThreads.map(\.id))
        kanbanThreadsByColumn[column.id] = (kanbanThreadsByColumn[column.id] ?? []).map { thread in
            unreadIds.contains(thread.id) ? thread.withUnread(false) : thread
        }
        coreThreads = coreThreads.map { thread in
            unreadIds.contains(thread.id) ? thread.withUnread(false) : thread
        }
        accountStatus = "Marked \(unreadThreads.count) Kanban card(s) read."
    }

    func archiveOrRemoveKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if isRssThread(thread) {
            let request = MobileCommandsKt.feedRemoveRequest(id: 35, params: RemoveRssFeedParams(threadId: thread.id)).toJson()
            runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: "Removed feed.")
        } else {
            let request = MobileCommandsKt.archiveThreadRequest(
                id: 35,
                params: ThreadActionParams(threadId: thread.id, folderId: nil, messageIds: [])
            ).toJson()
            runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: "Archive complete.")
        }
    }

    func deleteKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let request = MobileCommandsKt.deleteThreadRequest(
            id: 36,
            params: ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        ).toJson()
        runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: threadDeleteSuccessStatus(thread))
    }

    func markKanbanThreadRead(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let request = isRssThread(thread)
            ? MobileCommandsKt.rssMarkReadRequest(id: 37, params: RssMarkReadParams(threadId: thread.id, seen: thread.unread, itemKeys: [])).toJson()
            : MobileCommandsKt.markReadRequest(id: 37, params: MarkReadParams(threadId: thread.id, seen: thread.unread, messageIds: [])).toJson()
        runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: thread.unread ? "Marked read." : "Marked unread.", remove: false)
    }

    func markKanbanThreadStarred(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let request = isRssThread(thread)
            ? MobileCommandsKt.rssMarkStarredRequest(id: 38, params: RssMarkStarredParams(threadId: thread.id, starred: !thread.starred, itemKeys: [])).toJson()
            : MobileCommandsKt.markStarredRequest(id: 38, params: MarkStarredParams(threadId: thread.id, starred: !thread.starred, messageIds: [])).toJson()
        runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: thread.starred ? "Unstarred." : "Starred.", remove: false)
    }

    func moveKanbanThread(_ thread: ThreadSummary, from source: IosKanbanColumnSpec, to target: IosKanbanColumnSpec) {
        let request: String
        if isRssThread(thread) {
            request = MobileCommandsKt.feedMoveRequest(
                id: 39,
                params: MoveRssFeedParams(threadId: thread.id, targetAccountId: target.accountId)
            ).toJson()
        } else {
            request = MobileCommandsKt.moveThreadRequest(
                id: 39,
                params: MoveThreadParams(threadId: thread.id, targetFolderId: target.folderId)
            ).toJson()
        }
        runKanbanThreadAction(requestJson: request, sourceColumn: source, successStatus: "Move complete.")
        loadKanbanColumn(target, refresh: false)
    }

    func runKanbanThreadAction(
        requestJson: String,
        sourceColumn: IosKanbanColumnSpec,
        successStatus: String,
        remove: Bool = true
    ) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Kanban action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        if remove {
            loadKanbanColumn(sourceColumn, refresh: false)
        } else {
            loadKanbanColumn(sourceColumn, refresh: false)
        }
    }

    func accountLabel(_ account: AccountSummary) -> String {
        if !account.displayName.isEmpty { return account.displayName }
        if !account.email.isEmpty { return account.email }
        return account.id
    }

    func kanbanColumnTitle(_ column: IosKanbanColumnSpec) -> String {
        if column.accountId == iosUnifiedAccountId {
            return column.folderId == iosInboxFolderId ? "Unified Inbox" : "Unified / \(column.folderId)"
        }
        let account = coreAccounts.first(where: { $0.id == column.accountId }).map(accountLabel) ?? column.accountId
        return "\(account) / \(column.folderId)"
    }

    func selectedKanbanAccountIsRss() -> Bool {
        guard let account = coreAccounts.first(where: { $0.id == kanbanSelectedAccountId }) else { return false }
        return MailStateKt.accountSummaryIsRss(account: account)
    }

    func selectedAccountIsRss(_ accountId: String) -> Bool {
        guard let account = coreAccounts.first(where: { $0.id == accountId }) else {
            return false
        }
        return MailStateKt.accountSummaryIsRss(account: account)
    }

    func isRssThread(_ thread: ThreadSummary) -> Bool {
        MailStateKt.threadIdIsRss(threadId: thread.id)
    }

    func threadDeleteActionLabel(_ thread: ThreadSummary) -> String {
        threadDeleteActionLabel(folder: thread.folder)
    }

    func threadDeleteActionLabel(folder: String) -> String {
        if MailStateKt.folderIsDrafts(folder: folder) {
            return "Discard Draft"
        }
        if MailStateKt.folderIsTrash(folder: folder) {
            return "Delete Forever"
        }
        return "Move to Trash"
    }

    func threadDeleteSuccessStatus(_ thread: ThreadSummary) -> String {
        threadDeleteSuccessStatus(folder: thread.folder)
    }

    func threadDeleteSuccessStatus(folder: String) -> String {
        if MailStateKt.folderIsDrafts(folder: folder) {
            return "Draft discarded."
        }
        if MailStateKt.folderIsTrash(folder: folder) {
            return "Thread deleted."
        }
        return "Thread moved to Trash."
    }

    func shouldRenderHtml(_ message: MessageBody) -> Bool {
        guard !message.bodyHtml.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return false
        }
        let accountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        return conversationHtmlOverrides[accountId] ?? coreAccounts.first(where: { $0.id == accountId })?.conversationHtml ?? true
    }

    func currentConversationPrefersHtml() -> Bool {
        let accountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        return conversationHtmlOverrides[accountId] ?? coreAccounts.first(where: { $0.id == accountId })?.conversationHtml ?? true
    }

    func setCurrentConversationPrefersHtml(_ preferHtml: Bool) {
        let accountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        guard !accountId.isEmpty else { return }
        conversationHtmlOverrides[accountId] = preferHtml
    }

    func isRssStarredItem(_ item: StarredItemSummary) -> Bool {
        MailStateKt.threadIdIsRss(threadId: item.threadId)
    }

    func addAttachment(from url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            attachments.append(
                DraftAttachment(
                    id: url.absoluteString,
                    displayName: url.lastPathComponent,
                    mimeType: type,
                    sizeBytes: Int64(data.count),
                    dataBase64: data.base64EncodedString()
                )
            )
            attachmentError = nil
        } catch {
            attachmentError = error.localizedDescription
        }
    }

    func addQuickReplyAttachment(from url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            quickReplyAttachments.append(
                DraftAttachment(
                    id: UUID().uuidString,
                    displayName: url.lastPathComponent,
                    mimeType: type,
                    sizeBytes: Int64(data.count),
                    dataBase64: data.base64EncodedString()
                )
            )
            attachmentError = nil
            quickReplyFailure = ""
        } catch {
            attachmentError = error.localizedDescription
        }
    }

    func importAccountMedia(from url: URL) {
        guard let target = accountMediaImportTarget else {
            return
        }
        accountMediaImportTarget = nil
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            let params = AccountMediaFileParams(
                accountId: target.accountId,
                filename: url.lastPathComponent,
                mime: type,
                data: data.base64EncodedString()
            )
            let uploadRequest = target.isWallpaper
                ? MobileCommandsKt.accountWriteChatWallpaperFileRequest(id: 55, params: params).toJson()
                : MobileCommandsKt.accountWriteAvatarFileRequest(id: 55, params: params).toJson()
            let uploadResponse = RustCoreBridge.invokeJson(uploadRequest)
            if uploadResponse.contains(#""error""#) {
                accountStatus = "Media import failed."
                accountJson = uploadResponse
                return
            }
            let mediaUrl = MobileResponseParsersKt.parseMediaFileUrlResponse(responseJson: uploadResponse)
            if mediaUrl.isEmpty {
                accountStatus = "Media import failed: empty URL."
                return
            }
            let setRequest = target.isWallpaper
                ? MobileCommandsKt.accountSetChatWallpaperRequest(
                    id: 56,
                    params: AccountChatWallpaperParams(accountId: target.accountId, presetId: "", customUrl: mediaUrl)
                ).toJson()
                : MobileCommandsKt.accountSetAvatarRequest(
                    id: 56,
                    params: AccountAvatarParams(accountId: target.accountId, avatarUrl: mediaUrl)
                ).toJson()
            let setResponse = RustCoreBridge.invokeJson(setRequest)
            if setResponse.contains(#""error""#) {
                accountStatus = "Media import failed."
                accountJson = setResponse
                return
            }
            accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 57).toJson())
            updateCoreAccounts(from: accountJson)
            accountStatus = target.isWallpaper ? "Updated chat wallpaper." : "Updated avatar."
        } catch {
            accountStatus = "Media import failed: \(error.localizedDescription)"
        }
    }
}
