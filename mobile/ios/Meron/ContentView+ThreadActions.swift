import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func openMessageAttachment(_ attachment: MessageAttachment) {
        if attachment.mimeType.hasPrefix("image/") {
            openImageAttachmentPreview(attachment)
            return
        }
        if !attachment.url.isEmpty, let url = URL(string: attachment.url) {
            UIApplication.shared.open(url)
            return
        }
        openCachedMessageAttachment(attachment)
    }

    func openImageAttachmentPreview(_ attachment: MessageAttachment) {
        if !attachment.key.isEmpty {
            openCachedMessageAttachment(attachment)
            return
        }
        if let data = imageDataUrlPayload(attachment.url), let image = UIImage(data: data) {
            presentImagePreview(attachment: attachment, image: image, data: data)
            return
        }
        guard !attachment.url.isEmpty, let url = URL(string: attachment.url) else {
            accountStatus = String(localized: "mobile.ios.attachmentIsNotCached")
            return
        }
        Task {
            do {
                let (data, _) = try await URLSession.shared.data(from: url)
                guard let image = UIImage(data: data) else {
                    accountStatus = String(localized: "mobile.ios.attachmentDataInvalid")
                    return
                }
                presentImagePreview(attachment: attachment, image: image, data: data)
            } catch {
                accountStatus = "\(String(localized: "mobile.ios.attachmentOpenFailed")): \(error.localizedDescription)"
            }
        }
    }

    func openCachedMessageAttachment(_ attachment: MessageAttachment) {
        guard !attachment.key.isEmpty else {
            accountStatus = String(localized: "mobile.ios.attachmentIsNotCached")
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.attachmentReadRequest(
                id: 64,
                params: AttachmentReadParams(key: attachment.key)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.attachmentOpenFailed")
            accountJson = response
            return
        }
        let base64 = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
        guard let data = Data(base64Encoded: base64) else {
            accountStatus = String(localized: "mobile.ios.attachmentDataInvalid")
            return
        }
        if attachment.mimeType.hasPrefix("image/"), let image = UIImage(data: data) {
            presentImagePreview(attachment: attachment, image: image, data: data)
            return
        }
        presentShareableAttachment(attachment: attachment, data: data)
    }

    func presentImagePreview(attachment: MessageAttachment, image: UIImage, data: Data) {
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("MeronAttachments", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent(safeAttachmentFilename(attachment.filename))
            try data.write(to: url, options: .atomic)
            imagePreview = IosImagePreview(
                title: attachment.filename.isEmpty ? String(localized: "mobile.ios.image") : attachment.filename,
                image: image,
                url: url
            )
        } catch {
            accountStatus = "\(String(localized: "mobile.ios.attachmentOpenFailed")): \(error.localizedDescription)"
        }
    }

    func presentShareableAttachment(attachment: MessageAttachment, data: Data) {
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("MeronAttachments", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent(safeAttachmentFilename(attachment.filename))
            try data.write(to: url, options: .atomic)
            shareableAttachment = ShareableFile(url: url)
            accountStatus = String(localized: "mobile.ios.chooseSaveOrShareAttachment")
        } catch {
            accountStatus = "\(String(localized: "mobile.ios.attachmentOpenFailed")): \(error.localizedDescription)"
        }
    }

    func saveMessageAttachment(_ attachment: MessageAttachment) {
        guard !attachment.key.isEmpty else {
            accountStatus = String(localized: "mobile.ios.attachmentIsNotCached")
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.attachmentReadRequest(
                id: 65,
                params: AttachmentReadParams(key: attachment.key)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.attachmentOpenFailed")
            accountJson = response
            return
        }
        let base64 = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
        guard let data = Data(base64Encoded: base64), !data.isEmpty else {
            accountStatus = String(localized: "mobile.ios.attachmentDataInvalid")
            return
        }
        attachmentExportFilename = safeAttachmentFilename(attachment.filename)
        attachmentExportDocument = AttachmentDocument(data: data)
        isAttachmentExporterPresented = true
    }

    func saveImagePreview(_ preview: IosImagePreview) {
        do {
            let data = try Data(contentsOf: preview.url)
            guard !data.isEmpty else {
                accountStatus = String(localized: "mobile.ios.attachmentDataInvalid")
                return
            }
            attachmentExportFilename = safeAttachmentFilename(preview.title)
            attachmentExportDocument = AttachmentDocument(data: data)
            isAttachmentExporterPresented = true
        } catch {
            accountStatus = "\(String(localized: "mobile.ios.attachmentOpenFailed")): \(error.localizedDescription)"
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
                accountStatus = forward ? String(localized: "mobile.ios.forwardAttachmentCopyFailed") : String(localized: "mobile.ios.editAsNewAttachmentCopyFailed")
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
        composeCcBccVisible = !draft.cc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !draft.bcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        composeSubject = draft.subject
        composeBody = draft.body
        attachments = draft.attachments
        composeFromAccountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        composeFromEmail = ""
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = ""
        composeReferences = ""
        composeReplyTo = ""
        composeRichText = false
        composeReturnTab = iosComposeReturnTab(from: selectedTab)
        selectedTab = .compose
        accountStatus = forward ? String(localized: "mobile.ios.forwardDraftReady") : String(localized: "mobile.ios.copiedMessageIntoCompose")
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
                accountStatus = String(localized: "mobile.ios.draftAttachmentCopyFailed")
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
        composeCcBccVisible = !message.cc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !message.bcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        composeSubject = message.subject
        composeBody = message.body.isEmpty ? messagePlainText(message) : message.body
        attachments = copiedAttachments
        composeFromAccountId = thread.accountId
        composeFromEmail = message.fromAddr
        let cleanedMessageId = message.messageId.trimmingCharacters(in: CharacterSet(charactersIn: "<> \n\t"))
        composeDraftId = cleanedMessageId.isEmpty ? newIosDraftMessageId(accountId: thread.accountId) : cleanedMessageId
        composeDraftSaved = true
        composeInReplyTo = ""
        composeReferences = message.references
        composeReplyTo = message.replyTo
        composeRichText = !message.bodyHtml.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        composeReturnTab = iosComposeReturnTab(from: selectedTab)
        selectedTab = .compose
        accountStatus = String(localized: "mobile.ios.draftReady")
    }

    func archiveThread(_ thread: ThreadSummary) {
        let undo = threadUndo(message: String(localized: "mobile.ios.archiveComplete"), thread: thread)
        let params = ThreadActionParams(threadId: thread.id, folderId: nil, messageIds: [])
        if runThreadAction(
            requestJson: MobileCommandsKt.archiveThreadRequest(id: 17, params: params).toJson(),
            successStatus: String(localized: "mobile.ios.archiveComplete"),
            undo: undo
        ) {
            removeCoreThreadCaches(threadId: thread.id)
            removeKanbanThreadCaches(threadId: thread.id)
        }
    }

    func deleteThread(_ thread: ThreadSummary) {
        guard !threadDeleteRequiresConfirmation(thread) else {
            deleteThreadConfirmTarget = thread
            deleteThreadConfirmPresented = true
            return
        }
        performDeleteThread(thread)
    }

    func confirmDeleteThread(_ thread: ThreadSummary) {
        deleteThreadConfirmTarget = nil
        deleteThreadConfirmPresented = false
        performDeleteThread(thread)
    }

    func performDeleteThread(_ thread: ThreadSummary) {
        let undo = threadDeleteSupportsUndo(thread) ? threadUndo(message: threadDeleteSuccessStatus(thread), thread: thread) : nil
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        if runThreadAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 18, params: params).toJson(),
            successStatus: threadDeleteSuccessStatus(thread),
            undo: undo
        ) {
            removeCoreThreadCaches(threadId: thread.id)
            removeKanbanThreadCaches(threadId: thread.id)
        }
    }

    func presentMoveThreadDialog(_ thread: ThreadSummary) {
        guard !isRssThread(thread) else {
            accountStatus = String(localized: "mobile.ios.rssFeedsMoveFromKanban")
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 65, params: FolderListParams(accountId: thread.accountId)).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.folderListFailed")
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
        let undo = threadUndo(message: String(localized: "mobile.ios.moveComplete"), thread: thread)
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.moveThreadRequest(
                id: 66,
                params: MoveThreadParams(threadId: thread.id, targetFolderId: folder)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.moveFailed")
            accountJson = response
            return
        }
        pendingThreadUndo = undo
        removeCoreThreadCaches(threadId: thread.id)
        removeKanbanThreadCaches(threadId: thread.id)
        accountStatus = String(localized: "mobile.ios.moveComplete")
    }

    func rssFeedMoveTargets(for thread: ThreadSummary) -> [AccountSummary] {
        guard isRssThread(thread) else { return [] }
        return rssFeedMoveTargetAccounts(accounts: coreAccounts, sourceAccountId: thread.accountId)
    }

    func presentMoveRssFeedDialog(_ thread: ThreadSummary) {
        guard isRssThread(thread) else {
            accountStatus = String(localized: "mobile.ios.rssFeedsCanOnlyMoveToRss")
            return
        }
        let targets = rssFeedMoveTargets(for: thread)
        guard !targets.isEmpty else {
            accountStatus = String(localized: "folders.noOtherAvailable")
            return
        }
        moveRssFeedAccounts = targets
        moveRssFeedTarget = thread
        moveRssFeedDialogPresented = true
    }

    func moveRssFeed(_ thread: ThreadSummary, toAccount account: AccountSummary) {
        guard isRssThread(thread), MailStateKt.accountSummaryIsRss(account: account) else {
            accountStatus = String(localized: "mobile.ios.rssFeedsCanOnlyMoveToRss")
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.feedMoveRequest(
                id: 87,
                params: MoveRssFeedParams(threadId: thread.id, targetAccountId: account.id)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.moveFailed")
            accountJson = response
            return
        }
        moveRssFeedDialogPresented = false
        moveRssFeedTarget = nil
        moveRssFeedAccounts = []
        removeCoreThreadCaches(threadId: thread.id)
        removeKanbanThreadCaches(threadId: thread.id)
        if selectedCoreAccountId == thread.accountId {
            loadCoreFoldersAndThreads(accountId: thread.accountId, requestedFolder: selectedCoreFolder)
        }
        refreshAccountInboxUnreadCounts()
        accountStatus = String(localized: "mobile.ios.moveComplete")
    }

    func presentCopyThreadDialog(_ thread: ThreadSummary) {
        guard !isRssThread(thread) else {
            accountStatus = String(localized: "mobile.ios.rssFeedsCannotCopyToMail")
            return
        }
        var targets: [FolderSummary] = []
        for account in coreAccounts where !MailStateKt.accountSummaryIsRss(account: account) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.folderListRequest(id: 85, params: FolderListParams(accountId: account.id)).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = String(localized: "mobile.ios.folderListFailed")
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
            accountStatus = String(localized: "mobile.ios.copyFailed")
            accountJson = response
            return
        }
        copyThreadDialogPresented = false
        copyThreadTarget = nil
        accountStatus = String(localized: "mobile.ios.copyComplete")
    }

    func createFolderAndMoveThread(_ thread: ThreadSummary, name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            accountStatus = String(localized: "folders.nameRequired")
            return
        }
        let createResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderCreateRequest(
                id: 67,
                params: FolderCreateParams(accountId: thread.accountId, name: trimmed)
            ).toJson()
        )
        if createResponse.contains(#""error""#) {
            accountStatus = String(localized: "folders.createFailed")
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
        deleteRssFeedConfirmTarget = thread
        deleteRssFeedConfirmPresented = true
    }

    func confirmRemoveRssFeed(_ thread: ThreadSummary) {
        deleteRssFeedConfirmTarget = nil
        deleteRssFeedConfirmPresented = false
        performRemoveRssFeed(thread)
    }

    func performRemoveRssFeed(_ thread: ThreadSummary) {
        let params = RemoveRssFeedParams(threadId: thread.id)
        if runThreadAction(
            requestJson: MobileCommandsKt.feedRemoveRequest(id: 18, params: params).toJson(),
            successStatus: String(localized: "mobile.ios.removedFeed")
        ) {
            removeCoreThreadCaches(threadId: thread.id)
            removeKanbanThreadCaches(threadId: thread.id)
        }
    }

    func markThreadRead(_ thread: ThreadSummary, seen: Bool) {
        let undo = threadUndo(
            message: seen ? String(localized: "mobile.ios.markedRead") : String(localized: "mobile.ios.markedUnread"),
            thread: thread,
            action: .markRead(seen: threadReadUndoSeenTarget(afterMarkingSeen: seen))
        )
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkReadParams(threadId: thread.id, seen: seen, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkReadRequest(id: 19, params: params).toJson()
        } else {
            let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [])
            requestJson = MobileCommandsKt.markReadRequest(id: 19, params: params).toJson()
        }
        if runThreadAction(requestJson: requestJson, successStatus: seen ? String(localized: "mobile.ios.markedRead") : String(localized: "mobile.ios.markedUnread"), undo: undo) {
            let unread = !seen
            selectedCoreThread = selectedCoreThread?.id == thread.id ? selectedCoreThread?.withUnread(unread) : selectedCoreThread
            coreMessages = selectedCoreThread?.id == thread.id ? messagesAfterThreadReadState(coreMessages, unread: unread) : coreMessages
            updateKanbanThreadCaches(threadId: thread.id, unread: unread)
            starredItems = starredItemsAfterThreadReadState(starredItems, threadId: thread.id, seen: seen)
            updateUnifiedStarredKanbanThreadRead(threadId: thread.id, seen: seen)
        }
    }

    func markThreadStarred(_ thread: ThreadSummary, starred: Bool) {
        let undo = threadUndo(
            message: starred ? String(localized: "mobile.ios.starred") : String(localized: "mobile.ios.unstarred"),
            thread: thread,
            action: .markStarred(starred: threadStarUndoTarget(afterSettingStarred: starred))
        )
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkStarredParams(threadId: thread.id, starred: starred, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkStarredRequest(id: 20, params: params).toJson()
        } else {
            let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [])
            requestJson = MobileCommandsKt.markStarredRequest(id: 20, params: params).toJson()
        }
        if runThreadAction(requestJson: requestJson, successStatus: starred ? String(localized: "mobile.ios.starred") : String(localized: "mobile.ios.unstarred"), undo: undo) {
            selectedCoreThread = selectedCoreThread?.id == thread.id ? selectedCoreThread?.withFlags(starred: starred) : selectedCoreThread
            coreMessages = selectedCoreThread?.id == thread.id ? messagesAfterThreadStarredState(coreMessages, starred: starred) : coreMessages
            updateKanbanThreadCaches(threadId: thread.id, starred: starred)
            let messages = selectedCoreThread?.id == thread.id ? coreMessages : []
            starredItems = starredItemsAfterThreadStarred(starredItems, thread: thread, messages: messages, starred: starred)
            updateUnifiedStarredKanbanThreadStarred(threadId: thread.id, starred: starred)
        }
    }

    func toggleMessageRead(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let seen = message.unread
        let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.markReadRequest(id: 87, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.messageUpdateFailed")
            accountJson = response
            return
        }
        coreMessages = coreMessages.map { $0.id == message.id ? $0.withFlags(unread: !seen) : $0 }
        let updatedUnread = coreMessages.contains(where: \.unread)
        selectedCoreThread = selectedCoreThread?.withUnread(updatedUnread)
        coreThreads = coreThreads.map { $0.id == thread.id ? $0.withUnread(updatedUnread) : $0 }
        updateKanbanThreadCaches(threadId: thread.id, unread: updatedUnread)
        starredItems = starredItemsAfterAction(starredItems, itemId: message.id, update: .read(seen: seen))
        updateUnifiedStarredKanbanItems(itemId: message.id, update: .read(seen: seen))
        accountStatus = seen ? String(localized: "mobile.ios.markedRead") : String(localized: "mobile.ios.markedUnread")
    }

    func toggleMessageStarred(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let starred = !message.starred
        let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.markStarredRequest(id: 88, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.starFailed")
            accountJson = response
            return
        }
        coreMessages = coreMessages.map { $0.id == message.id ? $0.withFlags(starred: starred) : $0 }
        let updatedStarred = coreMessages.contains(where: \.starred)
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
        updateKanbanThreadCaches(threadId: thread.id, starred: updatedStarred)
        starredItems = starredItemsAfterMessageStarred(starredItems, message: message, thread: thread, starred: starred)
        if !starred {
            updateUnifiedStarredKanbanItems(itemId: message.id, update: .remove)
        }
        accountStatus = starred ? String(localized: "mobile.ios.starred") : String(localized: "mobile.ios.unstarred")
    }

    func deleteMessage(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        guard !messageDeleteRequiresConfirmation(folder: thread.folder) else {
            deleteMessageConfirmTarget = message
            deleteMessageConfirmPresented = true
            return
        }
        performDeleteMessage(message, in: thread)
    }

    func confirmDeleteMessage(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        deleteMessageConfirmTarget = nil
        deleteMessageConfirmPresented = false
        performDeleteMessage(message, in: thread)
    }

    func performDeleteMessage(_ message: MessageBody, in thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.deleteThreadRequest(id: 89, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.deleteFailed")
            accountJson = response
            return
        }
        let remainingMessages = messagesAfterDeletingMessage(coreMessages, messageId: message.id)
        coreMessages = remainingMessages
        if remainingMessages.isEmpty {
            coreThreads.removeAll { $0.id == thread.id }
            selectedMailThreadIds.remove(thread.id)
            selectedCoreThread = nil
            messageCursor = ""
            revealedRemoteMediaMessageIds.removeAll()
            removeKanbanThreadCaches(threadId: thread.id)
        } else {
            let updatedUnread = remainingMessages.contains(where: \.unread)
            let updatedStarred = remainingMessages.contains(where: \.starred)
            selectedCoreThread = selectedCoreThread?.withFlags(unread: updatedUnread, starred: updatedStarred)
            coreThreads = coreThreads.map { current in
                current.id == thread.id ? current.withFlags(unread: updatedUnread, starred: updatedStarred) : current
            }
            updateKanbanThreadCaches(threadId: thread.id, unread: updatedUnread, starred: updatedStarred)
        }
        starredItems = starredItemsAfterAction(starredItems, itemId: message.id, update: .remove)
        updateUnifiedStarredKanbanItems(itemId: message.id, update: .remove)
        accountStatus = String(localized: "mobile.ios.deleteComplete")
    }

    func updateKanbanThreadCaches(threadId: String, unread: Bool? = nil, starred: Bool? = nil) {
        for (columnId, threads) in kanbanThreadsByColumn {
            kanbanThreadsByColumn[columnId] = kanbanThreadsAfterThreadFlagUpdate(
                threads,
                threadId: threadId,
                unread: unread,
                starred: starred
            )
        }
    }

    func updateCoreThreadCaches(threadId: String, unread: Bool? = nil, starred: Bool? = nil) {
        coreThreads = kanbanThreadsAfterThreadFlagUpdate(
            coreThreads,
            threadId: threadId,
            unread: unread,
            starred: starred
        )
        if selectedCoreThread?.id == threadId {
            selectedCoreThread = selectedCoreThread?.withFlags(unread: unread, starred: starred)
            if let unread {
                coreMessages = messagesAfterThreadReadState(coreMessages, unread: unread)
            }
            if let starred {
                coreMessages = messagesAfterThreadStarredState(coreMessages, starred: starred)
            }
        }
    }

    func removeCoreThreadCaches(threadId: String) {
        coreThreads.removeAll { $0.id == threadId }
        selectedMailThreadIds = selectedThreadIdsAfterRemovingThread(selectedMailThreadIds, threadId: threadId)
        if selectedCoreThread?.id == threadId {
            selectedCoreThread = nil
            coreMessages = []
            messageCursor = ""
            revealedRemoteMediaMessageIds.removeAll()
            threadSearch = ""
            activeThreadSearchIndex = 0
        }
    }

    func removeKanbanThreadCaches(threadId: String) {
        for (columnId, threads) in kanbanThreadsByColumn {
            kanbanThreadsByColumn[columnId] = kanbanThreadsAfterRemovingThread(threads, threadId: threadId)
        }
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
        runStarredItemAction(
            requestJson: requestJson,
            successStatus: seen ? String(localized: "mobile.ios.markedRead") : String(localized: "mobile.ios.markedUnread"),
            itemId: item.id,
            update: .read(seen: seen)
        )
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
        runStarredItemAction(
            requestJson: requestJson,
            successStatus: String(localized: "mobile.ios.unstarred"),
            itemId: item.id,
            update: .remove
        )
    }

    func deleteStarredMailItem(_ item: StarredItemSummary) {
        guard !isRssStarredItem(item) else {
            accountStatus = String(localized: "mobile.ios.rssItemsCannotBeDeleted")
            return
        }
        guard !messageDeleteRequiresConfirmation(folder: item.folder) else {
            deleteStarredItemConfirmTarget = item
            deleteStarredItemConfirmPresented = true
            return
        }
        performDeleteStarredMailItem(item)
    }

    func confirmDeleteStarredMailItem(_ item: StarredItemSummary) {
        deleteStarredItemConfirmTarget = nil
        deleteStarredItemConfirmPresented = false
        performDeleteStarredMailItem(item)
    }

    func performDeleteStarredMailItem(_ item: StarredItemSummary) {
        let params = ThreadActionParams(threadId: item.threadId, folderId: item.folder, messageIds: [item.id])
        runStarredItemAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 43, params: params).toJson(),
            successStatus: threadDeleteSuccessStatus(folder: item.folder),
            itemId: item.id,
            update: .remove
        )
    }

    func runStarredItemAction(
        requestJson: String,
        successStatus: String,
        itemId: String,
        update: StarredItemListUpdate
    ) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.starredItemActionFailed")
            accountJson = response
            return
        }
        accountStatus = successStatus
        starredItems = starredItemsAfterAction(starredItems, itemId: itemId, update: update)
        updateUnifiedStarredKanbanItems(itemId: itemId, update: update)
    }

    func updateUnifiedStarredKanbanItems(itemId: String, update: StarredItemListUpdate) {
        for column in kanbanBoards.flatMap(\.columns) where isUnifiedStarredKanbanColumn(column) {
            let current = kanbanThreadsByColumn[column.id] ?? []
            let next = starredKanbanThreadsAfterAction(current, itemId: itemId, update: update)
            if next.map(\.id) != current.map(\.id) || next.contains(where: { nextThread in
                current.first(where: { $0.id == nextThread.id })?.unread != nextThread.unread
            }) {
                kanbanThreadsByColumn[column.id] = next
            }
        }
        if case .remove = update {
            kanbanStarredThreadIdsByItemId[itemId] = nil
        }
    }

    func updateUnifiedStarredKanbanThreadRead(threadId: String, seen: Bool) {
        for column in kanbanBoards.flatMap(\.columns) where isUnifiedStarredKanbanColumn(column) {
            let current = kanbanThreadsByColumn[column.id] ?? []
            let next = starredKanbanThreadsAfterThreadReadState(
                current,
                threadId: threadId,
                threadIdsByItemId: kanbanStarredThreadIdsByItemId,
                seen: seen
            )
            if next.contains(where: { nextThread in
                current.first(where: { $0.id == nextThread.id })?.unread != nextThread.unread
            }) {
                kanbanThreadsByColumn[column.id] = next
            }
        }
    }

    func updateUnifiedStarredKanbanThreadStarred(threadId: String, starred: Bool) {
        for column in kanbanBoards.flatMap(\.columns) where isUnifiedStarredKanbanColumn(column) {
            let current = kanbanThreadsByColumn[column.id] ?? []
            let next = starredKanbanThreadsAfterThreadStarred(
                current,
                threadId: threadId,
                threadIdsByItemId: kanbanStarredThreadIdsByItemId,
                starred: starred
            )
            if next.map(\.id) != current.map(\.id) {
                kanbanThreadsByColumn[column.id] = next
            }
        }
        if !starred {
            kanbanStarredThreadIdsByItemId = kanbanStarredThreadIdsByItemId.filter { $0.value != threadId }
        }
    }

    @discardableResult
    func runThreadAction(requestJson: String, successStatus: String, undo: IosThreadUndo? = nil) -> Bool {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.threadActionFailed")
            accountJson = response
            return false
        }
        pendingThreadUndo = undo
        accountStatus = successStatus
        if !selectedCoreAccountId.isEmpty {
            loadCoreFoldersAndThreads(accountId: selectedCoreAccountId, requestedFolder: selectedCoreFolder)
        }
        return true
    }

    func threadUndo(message: String, thread: ThreadSummary, action: IosThreadUndoAction = .moveToOriginalFolder) -> IosThreadUndo {
        IosThreadUndo(
            message: message,
            thread: thread,
            action: action,
            threadsSnapshot: coreThreads,
            selectedThreadIdsSnapshot: selectedMailThreadIds,
            selectedThreadSnapshot: selectedCoreThread,
            messagesSnapshot: coreMessages,
            messageCursorSnapshot: messageCursor,
            kanbanThreadsSnapshot: kanbanThreadsByColumn
        )
    }

    func threadDeleteSupportsUndo(_ thread: ThreadSummary) -> Bool {
        !MailStateKt.folderIsDrafts(folder: thread.folder) && !MailStateKt.folderIsTrash(folder: thread.folder)
    }

    func undoPendingThreadAction() {
        guard let undo = pendingThreadUndo else { return }
        pendingThreadUndo = nil
        let response = RustCoreBridge.invokeJson(undoThreadActionRequest(undo).toJson())
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.undoFailed")
            accountJson = response
            return
        }
        coreThreads = undo.threadsSnapshot
        selectedMailThreadIds = undo.selectedThreadIdsSnapshot
        selectedCoreThread = undo.selectedThreadSnapshot
        coreMessages = undo.messagesSnapshot
        messageCursor = undo.messageCursorSnapshot
        kanbanThreadsByColumn = undo.kanbanThreadsSnapshot
        accountStatus = String(localized: "mobile.ios.threadRestored")
    }

    func undoThreadActionRequest(_ undo: IosThreadUndo) -> CoreRequest {
        switch undo.action {
        case .moveToOriginalFolder:
            return MobileCommandsKt.moveThreadRequest(
                id: 90,
                params: MoveThreadParams(threadId: undo.thread.id, targetFolderId: undo.thread.folder)
            )
        case let .markRead(seen):
            if isRssThread(undo.thread) {
                return MobileCommandsKt.rssMarkReadRequest(
                    id: 90,
                    params: RssMarkReadParams(threadId: undo.thread.id, seen: seen, itemKeys: [])
                )
            }
            return MobileCommandsKt.markReadRequest(
                id: 90,
                params: MarkReadParams(threadId: undo.thread.id, seen: seen, messageIds: [])
            )
        case let .markStarred(starred):
            if isRssThread(undo.thread) {
                return MobileCommandsKt.rssMarkStarredRequest(
                    id: 90,
                    params: RssMarkStarredParams(threadId: undo.thread.id, starred: starred, itemKeys: [])
                )
            }
            return MobileCommandsKt.markStarredRequest(
                id: 90,
                params: MarkStarredParams(threadId: undo.thread.id, starred: starred, messageIds: [])
            )
        }
    }

    var activeKanbanBoard: IosKanbanBoardSpec? {
        kanbanBoards.first(where: { $0.id == activeKanbanBoardId }) ?? kanbanBoards.first
    }

    func activeKanbanBoardIndex() -> Int {
        guard !kanbanBoards.isEmpty else { return -1 }
        if let index = kanbanBoards.firstIndex(where: { $0.id == activeKanbanBoardId }) {
            return index
        }
        return 0
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

    func selectKanbanBoard(_ boardId: String, loadBoard: Bool = true) {
        activeKanbanBoardId = boardId
        kanbanMinimizedColumnIds = []
        kanbanSearchScope = "all"
        UserDefaults.standard.set(boardId, forKey: "ios_kanban_active_board_v1")
        kanbanNewBoardName = activeKanbanBoard?.name ?? ""
        kanbanBoardAvatarUrl = activeKanbanBoard?.avatarUrl ?? ""
        kanbanBoardWallpaperPresetId = activeKanbanBoard?.wallpaperPresetId ?? ""
        kanbanBoardWallpaperUrl = activeKanbanBoard?.wallpaperUrl ?? ""
        if loadBoard {
            loadActiveKanbanBoard(refresh: false)
        }
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
            let board = IosKanbanBoardSpec(id: UUID().uuidString, name: String(localized: "kanban.board.defaultName"), columns: columns)
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
        let board = IosKanbanBoardSpec(
            id: UUID().uuidString,
            name: localizedCatalogString("mobile.ios.boardNumberName", args: ["number": kanbanBoards.count + 1]),
            columns: initialColumns
        )
        kanbanBoards.append(board)
        activeKanbanBoardId = board.id
        kanbanNewBoardName = board.name
        kanbanBoardAvatarUrl = ""
        kanbanBoardWallpaperPresetId = ""
        kanbanBoardWallpaperUrl = ""
        kanbanMinimizedColumnIds = []
        kanbanSearchScope = "all"
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

    func moveActiveKanbanBoard(delta: Int) {
        let index = activeKanbanBoardIndex()
        guard index >= 0 else { return }
        let target = min(max(index + delta, 0), max(kanbanBoards.count - 1, 0))
        guard index != target else { return }

        let board = kanbanBoards.remove(at: index)
        kanbanBoards.insert(board, at: target)
        activeKanbanBoardId = board.id
        persistKanbanBoards()
        accountStatus = String(localized: "mobile.ios.movedBoard")
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
        accountStatus = String(localized: "mobile.ios.savedBoardStyle")
    }

    func importKanbanBoardMedia(from url: URL) {
        guard let target = kanbanBoardMediaImportTarget else {
            return
        }
        kanbanBoardMediaImportTarget = nil
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let mediaUrl = try copyKanbanBoardMedia(from: url, boardId: target.boardId)
            kanbanBoards = kanbanBoards.map { board in
                if board.id != target.boardId { return board }
                var next = board
                if target.isWallpaper {
                    next.wallpaperPresetId = nil
                    next.wallpaperUrl = mediaUrl
                    kanbanBoardWallpaperPresetId = ""
                    kanbanBoardWallpaperUrl = mediaUrl
                } else {
                    next.avatarUrl = mediaUrl
                    kanbanBoardAvatarUrl = mediaUrl
                }
                return next
            }
            persistKanbanBoards()
            accountStatus = target.isWallpaper ? String(localized: "mobile.ios.updatedBoardWallpaper") : String(localized: "mobile.ios.updatedBoardAvatar")
        } catch {
            accountStatus = "\(String(localized: "mobile.ios.boardMediaImportFailed")): \(error.localizedDescription)"
        }
    }

    func copyKanbanBoardMedia(from url: URL, boardId: String) throws -> String {
        let data = try Data(contentsOf: url)
        let sanitizedBoardId = boardId
            .replacingOccurrences(of: "/", with: "-")
            .replacingOccurrences(of: ":", with: "-")
        let extensionPart = url.pathExtension.isEmpty ? "img" : url.pathExtension
        let directory = URL(fileURLWithPath: IosAppPaths.mobileDataDirectory())
            .appendingPathComponent("BoardMedia", isDirectory: true)
            .appendingPathComponent(sanitizedBoardId, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let destination = directory.appendingPathComponent("\(UUID().uuidString).\(extensionPart)")
        try data.write(to: destination, options: .atomic)
        return destination.path
    }

    func deleteActiveKanbanBoard() {
        guard !kanbanBoards.isEmpty else { return }
        kanbanBoards.removeAll { $0.id == activeKanbanBoardId }
        if kanbanBoards.isEmpty {
            activeKanbanBoardId = ""
            kanbanThreadsByColumn = [:]
            kanbanStatusByColumn = [:]
        }
        kanbanMinimizedColumnIds = []
        kanbanSearchScope = "all"
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
        kanbanMinimizedColumnIds.remove(column.id)
        if kanbanSearchScope == column.id {
            kanbanSearchScope = "all"
        }
        persistKanbanBoards()
    }

    func isUnifiedStarredKanbanColumn(_ column: IosKanbanColumnSpec) -> Bool {
        column.accountId == iosUnifiedAccountId && column.folderId.caseInsensitiveCompare(iosStarredFolderId) == .orderedSame
    }

    func kanbanThreadId(for thread: ThreadSummary, in column: IosKanbanColumnSpec) -> String {
        isUnifiedStarredKanbanColumn(column)
            ? kanbanStarredItemActionTarget(for: thread, threadIdsByItemId: kanbanStarredThreadIdsByItemId).threadId
            : thread.id
    }

    func kanbanActionThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) -> ThreadSummary {
        let threadId = kanbanThreadId(for: thread, in: column)
        guard threadId != thread.id else { return thread }
        return ThreadSummary(
            id: threadId,
            accountId: thread.accountId,
            folder: thread.folder,
            subject: thread.subject,
            sender: thread.sender,
            preview: thread.preview,
            unread: thread.unread,
            starred: thread.starred,
            dateEpochSeconds: thread.dateEpochSeconds,
            feedUrl: thread.feedUrl
        )
    }

    func isKanbanThreadRss(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) -> Bool {
        MailStateKt.threadIdIsRss(threadId: kanbanThreadId(for: thread, in: column))
    }

    func openKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if isUnifiedStarredKanbanColumn(column) {
            selectedCoreAccountId = thread.accountId
            selectedCoreFolder = thread.folder
            selectedTab = .mail
            if isKanbanThreadRss(thread, in: column) {
                starredMessageScrollTarget = ""
                messageReaderTarget = starredKanbanReaderMessage(for: thread)
                return
            }
            readThread(kanbanActionThread(thread, in: column))
            DispatchQueue.main.async {
                starredMessageScrollTarget = thread.id
            }
            return
        }
        readThread(kanbanActionThread(thread, in: column))
    }

    func moveKanbanColumn(_ column: IosKanbanColumnSpec, delta: Int) {
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            guard let index = board.columns.firstIndex(where: { $0.id == column.id }) else { return board }
            let target = min(max(index + delta, 0), max(board.columns.count - 1, 0))
            guard index != target else { return board }

            var next = board
            var columns = board.columns
            let item = columns.remove(at: index)
            columns.insert(item, at: target)
            next.columns = columns
            return next
        }
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
            accountStatus = String(localized: "mobile.ios.chooseAccountAndFolderName")
            return
        }
        let request = MobileCommandsKt.folderCreateRequest(
            id: 32,
            params: FolderCreateParams(accountId: accountId, name: name)
        ).toJson()
        let response = RustCoreBridge.invokeJson(request)
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.createFolderFailed")
            accountJson = response
            return
        }
        kanbanCreateFolderName = ""
        loadFoldersForKanbanAccount(accountId)
        addKanbanColumn(accountId: accountId, folderId: name)
        accountStatus = String(localized: "mobile.ios.folderCreated")
    }

    func loadActiveKanbanBoard(refresh: Bool) {
        activeKanbanBoard?.columns.forEach { loadKanbanColumn($0, refresh: refresh) }
    }

    func unifiedKanbanAccounts(for cursors: [String: String]? = nil) -> [AccountSummary] {
        let accounts = coreAccounts.filter(\.includedInUnified)
        guard let cursors else { return accounts }
        return accounts.filter { !(cursors[$0.id] ?? "").isEmpty }
    }

    func syncKanbanAccount(_ account: AccountSummary, folderId: String) -> Bool {
        let response: String = if MailStateKt.accountSummaryIsRss(account: account) {
            RustCoreBridge.invokeJson(
                MobileCommandsKt.syncRssRequest(id: 33, params: SyncRssParams(accountId: account.id)).toJson()
            )
        } else {
            RustCoreBridge.invokeJson(
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
        kanbanStatusByColumn[column.id] = refresh ? String(localized: "mobile.ios.refreshing") : String(localized: "common.loading")
        let trimmedQuery = kanbanSearchQuery(for: column)

        if isUnifiedStarredKanbanColumn(column) {
            let response = RustCoreBridge.invokeJson(MobileCommandsKt.starredItemsRequest(id: 86).toJson())
            if response.contains(#""error""#) {
                kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.starredLoadFailed")
                accountJson = response
                return
            }
            let items = MobileResponseParsersKt.parseStarredItemsResponse(responseJson: response)
            let filtered = kanbanStarredItemsMatching(items, query: trimmedQuery)
            kanbanStarredThreadIdsByItemId.merge(Dictionary(uniqueKeysWithValues: filtered.map { ($0.id, $0.threadId) })) { _, next in next }
            kanbanThreadsByColumn[column.id] = filtered.map(kanbanThreadSummary)
            kanbanCursorByColumn[column.id] = ""
            kanbanAccountCursorsByColumn[column.id] = [:]
            kanbanStatusByColumn[column.id] = localizedCatalogString(
                "mobile.ios.kanbanFilteredItemCount",
                args: ["count": filtered.count, "filter": kanbanFilter.label.lowercased()]
            )
            return
        }

        if column.accountId == iosUnifiedAccountId {
            let accounts = unifiedKanbanAccounts()
            if refresh {
                for account in accounts {
                    if !syncKanbanAccount(account, folderId: iosInboxFolderId) {
                        kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.syncFailed")
                        return
                    }
                }
            }

            var threads: [ThreadSummary] = []
            var nextCursors: [String: String] = [:]
            for account in accounts {
                guard let page = threadListPage(accountId: account.id, folderId: iosInboxFolderId, query: trimmedQuery, beforeCursor: nil) else {
                    kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.loadFailed")
                    return
                }
                threads.append(contentsOf: page.threads)
                if trimmedQuery.isEmpty, !page.nextCursor.isEmpty {
                    nextCursors[account.id] = page.nextCursor
                }
            }
            kanbanThreadsByColumn[column.id] = threads.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
            kanbanCursorByColumn[column.id] = ""
            kanbanAccountCursorsByColumn[column.id] = nextCursors
            kanbanStatusByColumn[column.id] = localizedCatalogString(
                "mobile.ios.kanbanFilteredItemCount",
                args: ["count": threads.count, "filter": kanbanFilter.label.lowercased()]
            )
            return
        }

        let account = coreAccounts.first(where: { $0.id == column.accountId })
        if refresh, let account {
            if !syncKanbanAccount(account, folderId: column.folderId) {
                kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.syncFailed")
                return
            }
        }

        guard let page = threadListPage(accountId: column.accountId, folderId: column.folderId, query: trimmedQuery, beforeCursor: nil) else {
            kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.loadFailed")
            return
        }
        kanbanThreadsByColumn[column.id] = page.threads
        kanbanCursorByColumn[column.id] = trimmedQuery.isEmpty ? page.nextCursor : ""
        kanbanAccountCursorsByColumn[column.id] = [:]
        kanbanStatusByColumn[column.id] = localizedCatalogString(
            "mobile.ios.kanbanFilteredItemCount",
            args: ["count": page.threads.count, "filter": kanbanFilter.label.lowercased()]
        )
    }

    func loadMoreKanbanColumn(_ column: IosKanbanColumnSpec) {
        guard kanbanSearchQuery(for: column).isEmpty else { return }
        guard !isUnifiedStarredKanbanColumn(column) else { return }
        if column.accountId == iosUnifiedAccountId {
            let cursors = kanbanAccountCursorsByColumn[column.id] ?? [:]
            guard !cursors.isEmpty, !kanbanLoadingMoreColumns.contains(column.id) else { return }
            kanbanLoadingMoreColumns.insert(column.id)
            var threads: [ThreadSummary] = []
            var nextCursors: [String: String] = [:]
            for account in unifiedKanbanAccounts(for: cursors) {
                guard let page = threadListPage(accountId: account.id, folderId: iosInboxFolderId, query: "", beforeCursor: cursors[account.id]) else {
                    kanbanLoadingMoreColumns.remove(column.id)
                    kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.loadOlderFailed")
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
            kanbanStatusByColumn[column.id] = appended.isEmpty
                ? String(localized: "mobile.ios.noOlderItems")
                : localizedCatalogString("mobile.ios.kanbanItemCount", args: ["count": existing.count + appended.count])
            return
        }

        guard let cursor = kanbanCursorByColumn[column.id],
              !cursor.isEmpty,
              !kanbanLoadingMoreColumns.contains(column.id) else { return }
        kanbanLoadingMoreColumns.insert(column.id)
        guard let page = threadListPage(accountId: column.accountId, folderId: column.folderId, query: "", beforeCursor: cursor) else {
            kanbanLoadingMoreColumns.remove(column.id)
            kanbanStatusByColumn[column.id] = String(localized: "mobile.ios.loadOlderFailed")
            return
        }
        kanbanLoadingMoreColumns.remove(column.id)
        let existing = kanbanThreadsByColumn[column.id] ?? []
        let existingIds = Set(existing.map(\.id))
        let appended = page.threads.filter { !existingIds.contains($0.id) }
        kanbanThreadsByColumn[column.id] = (existing + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        kanbanCursorByColumn[column.id] = page.nextCursor
        kanbanStatusByColumn[column.id] = appended.isEmpty
            ? String(localized: "mobile.ios.noOlderItems")
            : localizedCatalogString("mobile.ios.kanbanItemCount", args: ["count": existing.count + appended.count])
    }

    func markKanbanColumnAllRead(_ column: IosKanbanColumnSpec) {
        let unreadThreads = (kanbanThreadsByColumn[column.id] ?? []).filter(\.unread)
        guard !unreadThreads.isEmpty else {
            accountStatus = String(localized: "mobile.ios.noUnreadCards")
            return
        }

        var failedResponse = ""
        if isUnifiedStarredKanbanColumn(column) {
            for thread in unreadThreads {
                let target = kanbanStarredItemActionTarget(for: thread, threadIdsByItemId: kanbanStarredThreadIdsByItemId)
                let request = target.isRss
                    ? MobileCommandsKt.rssMarkReadRequest(
                        id: 49,
                        params: RssMarkReadParams(threadId: target.threadId, seen: true, itemKeys: [target.itemId])
                    ).toJson()
                    : MobileCommandsKt.markReadRequest(
                        id: 49,
                        params: MarkReadParams(threadId: target.threadId, seen: true, messageIds: [target.itemId])
                    ).toJson()
                let response = RustCoreBridge.invokeJson(request)
                if response.contains(#""error""#) {
                    failedResponse = response
                    break
                }
            }
        } else if column.accountId == iosUnifiedAccountId {
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
                  !MailStateKt.accountSummaryIsRss(account: account)
        {
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
            accountStatus = String(localized: "mobile.ios.kanbanMarkAllReadFailed")
            accountJson = failedResponse
            return
        }

        let unreadIds = Set(unreadThreads.map(\.id))
        for (columnId, threads) in kanbanThreadsByColumn {
            kanbanThreadsByColumn[columnId] = threadsAfterMarkingThreadIdsRead(threads, threadIds: unreadIds)
        }
        if isUnifiedStarredKanbanColumn(column) {
            for itemId in unreadIds {
                starredItems = starredItemsAfterAction(starredItems, itemId: itemId, update: .read(seen: true))
            }
        }
        coreThreads = threadsAfterMarkingThreadIdsRead(coreThreads, threadIds: unreadIds)
        if let selectedCoreThread, unreadIds.contains(selectedCoreThread.id) {
            self.selectedCoreThread = selectedCoreThread.withUnread(false)
            coreMessages = messagesAfterThreadReadState(coreMessages, unread: false)
        }
        accountStatus = String(
            format: String(localized: "mobile.ios.markedKanbanCardsRead"),
            unreadThreads.count
        )
    }

    func archiveOrRemoveKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let actionThread = kanbanActionThread(thread, in: column)
        if isRssThread(actionThread) {
            removeRssFeed(actionThread)
        } else {
            let request = MobileCommandsKt.archiveThreadRequest(
                id: 35,
                params: ThreadActionParams(threadId: actionThread.id, folderId: nil, messageIds: [])
            ).toJson()
            if runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: String(localized: "mobile.ios.archiveComplete")) {
                removeCoreThreadCaches(threadId: actionThread.id)
                removeKanbanThreadCaches(threadId: actionThread.id)
            }
        }
    }

    func deleteKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if kanbanDeleteRequiresConfirmation(thread: thread, in: column) {
            deleteKanbanConfirmTarget = IosKanbanDeleteTarget(thread: thread, column: column)
            deleteKanbanConfirmPresented = true
            return
        }
        performDeleteKanbanThread(thread, in: column)
    }

    func confirmDeleteKanbanThread(_ target: IosKanbanDeleteTarget) {
        deleteKanbanConfirmTarget = nil
        deleteKanbanConfirmPresented = false
        performDeleteKanbanThread(target.thread, in: target.column)
    }

    func performDeleteKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if isUnifiedStarredKanbanColumn(column) {
            guard !MailStateKt.threadIdIsRss(threadId: kanbanThreadId(for: thread, in: column)) else {
                accountStatus = String(localized: "mobile.ios.rssItemsCannotBeDeleted")
                return
            }
            let request = MobileCommandsKt.deleteThreadRequest(
                id: 36,
                params: ThreadActionParams(threadId: kanbanThreadId(for: thread, in: column), folderId: thread.folder, messageIds: [thread.id])
            ).toJson()
            if runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: String(localized: "mobile.ios.deleteComplete")) {
                starredItems = starredItemsAfterAction(starredItems, itemId: thread.id, update: .remove)
                kanbanStarredThreadIdsByItemId[thread.id] = nil
            }
            return
        }
        let request = MobileCommandsKt.deleteThreadRequest(
            id: 36,
            params: ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        ).toJson()
        if runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: threadDeleteSuccessStatus(thread)) {
            removeCoreThreadCaches(threadId: thread.id)
            removeKanbanThreadCaches(threadId: thread.id)
        }
    }

    func markKanbanThreadRead(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if isUnifiedStarredKanbanColumn(column) {
            let threadId = kanbanThreadId(for: thread, in: column)
            let request = MailStateKt.threadIdIsRss(threadId: threadId)
                ? MobileCommandsKt.rssMarkReadRequest(id: 37, params: RssMarkReadParams(threadId: threadId, seen: thread.unread, itemKeys: [thread.id])).toJson()
                : MobileCommandsKt.markReadRequest(id: 37, params: MarkReadParams(threadId: threadId, seen: thread.unread, messageIds: [thread.id])).toJson()
            if runKanbanThreadAction(
                requestJson: request,
                sourceColumn: column,
                successStatus: thread.unread ? String(localized: "mobile.ios.markedRead") : String(localized: "mobile.ios.markedUnread"),
                remove: false
            ) {
                starredItems = starredItemsAfterAction(starredItems, itemId: thread.id, update: .read(seen: thread.unread))
            }
            return
        }
        let request = isRssThread(thread)
            ? MobileCommandsKt.rssMarkReadRequest(id: 37, params: RssMarkReadParams(threadId: thread.id, seen: thread.unread, itemKeys: [])).toJson()
            : MobileCommandsKt.markReadRequest(id: 37, params: MarkReadParams(threadId: thread.id, seen: thread.unread, messageIds: [])).toJson()
        if runKanbanThreadAction(
            requestJson: request,
            sourceColumn: column,
            successStatus: thread.unread ? String(localized: "mobile.ios.markedRead") : String(localized: "mobile.ios.markedUnread"),
            remove: false
        ) {
            let unread = !thread.unread
            updateCoreThreadCaches(threadId: thread.id, unread: unread)
            updateKanbanThreadCaches(threadId: thread.id, unread: unread)
        }
    }

    func markKanbanThreadStarred(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if isUnifiedStarredKanbanColumn(column) {
            let threadId = kanbanThreadId(for: thread, in: column)
            let request = MailStateKt.threadIdIsRss(threadId: threadId)
                ? MobileCommandsKt.rssMarkStarredRequest(id: 38, params: RssMarkStarredParams(threadId: threadId, starred: !thread.starred, itemKeys: [thread.id])).toJson()
                : MobileCommandsKt.markStarredRequest(id: 38, params: MarkStarredParams(threadId: threadId, starred: !thread.starred, messageIds: [thread.id])).toJson()
            if runKanbanThreadAction(
                requestJson: request,
                sourceColumn: column,
                successStatus: thread.starred ? String(localized: "mobile.ios.unstarred") : String(localized: "mobile.ios.starred"),
                remove: thread.starred
            ), thread.starred {
                starredItems = starredItemsAfterAction(starredItems, itemId: thread.id, update: .remove)
                kanbanStarredThreadIdsByItemId[thread.id] = nil
            }
            return
        }
        let request = isRssThread(thread)
            ? MobileCommandsKt.rssMarkStarredRequest(id: 38, params: RssMarkStarredParams(threadId: thread.id, starred: !thread.starred, itemKeys: [])).toJson()
            : MobileCommandsKt.markStarredRequest(id: 38, params: MarkStarredParams(threadId: thread.id, starred: !thread.starred, messageIds: [])).toJson()
        if runKanbanThreadAction(
            requestJson: request,
            sourceColumn: column,
            successStatus: thread.starred ? String(localized: "mobile.ios.unstarred") : String(localized: "mobile.ios.starred"),
            remove: false
        ) {
            let starred = !thread.starred
            updateCoreThreadCaches(threadId: thread.id, starred: starred)
            updateKanbanThreadCaches(threadId: thread.id, starred: starred)
        }
    }

    func moveKanbanThread(_ thread: ThreadSummary, from source: IosKanbanColumnSpec, to target: IosKanbanColumnSpec) {
        let actionThread = kanbanActionThread(thread, in: source)
        let validation = iosKanbanMoveValidation(
            threadIsRss: isRssThread(actionThread),
            targetAccount: coreAccounts.first { $0.id == target.accountId }
        )
        guard validation == .allowed else {
            accountStatus = iosKanbanMoveValidationStatus(validation)
            return
        }
        let request: String = if isRssThread(actionThread) {
            MobileCommandsKt.feedMoveRequest(
                id: 39,
                params: MoveRssFeedParams(threadId: actionThread.id, targetAccountId: target.accountId)
            ).toJson()
        } else {
            MobileCommandsKt.moveThreadRequest(
                id: 39,
                params: MoveThreadParams(threadId: actionThread.id, targetFolderId: target.folderId)
            ).toJson()
        }
        if runKanbanThreadAction(requestJson: request, sourceColumn: source, successStatus: String(localized: "mobile.ios.moveComplete")) {
            removeCoreThreadCaches(threadId: actionThread.id)
            removeKanbanThreadCaches(threadId: actionThread.id)
            loadKanbanColumn(target, refresh: false)
        }
    }

    func kanbanMoveTargets(for thread: ThreadSummary, from source: IosKanbanColumnSpec, in board: IosKanbanBoardSpec) -> [IosKanbanColumnSpec] {
        let threadIsRss = isKanbanThreadRss(thread, in: source)
        return board.columns.filter { target in
            guard target.id != source.id, target.accountId != iosUnifiedAccountId else {
                return false
            }
            guard let targetAccount = coreAccounts.first(where: { $0.id == target.accountId }) else {
                return false
            }
            guard MailStateKt.accountSummaryIsRss(account: targetAccount) == threadIsRss else {
                return false
            }
            return !(target.accountId == thread.accountId && target.folderId.caseInsensitiveCompare(thread.folder) == .orderedSame)
        }
    }

    @discardableResult
    func runKanbanThreadAction(
        requestJson: String,
        sourceColumn: IosKanbanColumnSpec,
        successStatus: String,
        remove: Bool = true
    ) -> Bool {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.kanbanActionFailed")
            accountJson = response
            return false
        }
        accountStatus = successStatus
        if remove {
            loadKanbanColumn(sourceColumn, refresh: false)
        } else {
            loadKanbanColumn(sourceColumn, refresh: false)
        }
        return true
    }

    func accountLabel(_ account: AccountSummary) -> String {
        if !account.displayName.isEmpty { return account.displayName }
        if !account.email.isEmpty { return account.email }
        return account.id
    }

    func kanbanColumnTitle(_ column: IosKanbanColumnSpec) -> String {
        if isUnifiedStarredKanbanColumn(column) {
            return String(localized: "kanban.columns.unifiedStarred")
        }
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

    func kanbanSearchQuery(for column: IosKanbanColumnSpec) -> String {
        let query = kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return "" }
        let scope = kanbanSearchScope.isEmpty ? "all" : kanbanSearchScope
        return scope == "all" || scope == column.id ? query : ""
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
            return String(localized: "threads.actions.discardDraft")
        }
        if MailStateKt.folderIsTrash(folder: folder) {
            return String(localized: "threads.actions.deleteForever")
        }
        return String(localized: "threads.actions.moveToTrash")
    }

    func threadDeleteSuccessStatus(_ thread: ThreadSummary) -> String {
        threadDeleteSuccessStatus(folder: thread.folder)
    }

    func threadDeleteSuccessStatus(folder: String) -> String {
        if MailStateKt.folderIsDrafts(folder: folder) {
            return String(localized: "mobile.ios.draftDiscarded")
        }
        if MailStateKt.folderIsTrash(folder: folder) {
            return String(localized: "mobile.ios.threadDeleted")
        }
        return String(localized: "mobile.ios.threadMovedToTrash")
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
            scheduleComposeAutosave()
        } catch {
            attachmentError = error.localizedDescription
        }
    }

    func addInlineImageAttachment(from url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "image/*"
            guard type.lowercased().hasPrefix("image/") else {
                attachmentError = String(localized: "mobile.ios.attachmentDataInvalid")
                return
            }
            let attachment = DraftAttachment(
                id: UUID().uuidString,
                displayName: url.lastPathComponent.isEmpty ? "inline-image" : url.lastPathComponent,
                mimeType: type,
                sizeBytes: Int64(data.count),
                dataBase64: data.base64EncodedString()
            )
            attachments.append(attachment)
            composeInlineAttachmentIds.insert(attachment.id)
            composeRichText = true
            appendComposeMarkdownSnippet("![\(attachment.displayName)](cid:\(composeInlineContentId(for: attachment)))")
            attachmentError = nil
            scheduleComposeAutosave()
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
                accountStatus = String(localized: "mobile.ios.mediaImportFailed")
                accountJson = uploadResponse
                return
            }
            let mediaUrl = MobileResponseParsersKt.parseMediaFileUrlResponse(responseJson: uploadResponse)
            if mediaUrl.isEmpty {
                accountStatus = String(localized: "mobile.ios.mediaImportFailedEmptyUrl")
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
                accountStatus = String(localized: "mobile.ios.mediaImportFailed")
                accountJson = setResponse
                return
            }
            accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 57).toJson())
            updateCoreAccounts(from: accountJson)
            accountStatus = target.isWallpaper ? String(localized: "mobile.ios.updatedChatWallpaper") : String(localized: "mobile.ios.updatedAvatar")
        } catch {
            accountStatus = "\(String(localized: "mobile.ios.mediaImportFailed")): \(error.localizedDescription)"
        }
    }
}
