import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

let iosQuickReplyDraftsKey = "ios_quick_reply_drafts_v1"

extension ContentView {
    func loadQuickReplyDrafts() -> [String: String] {
        guard let data = UserDefaults.standard.data(forKey: iosQuickReplyDraftsKey),
              let drafts = try? JSONDecoder().decode([String: String].self, from: data)
        else {
            return [:]
        }
        return drafts
    }

    func saveQuickReplyDrafts(_ drafts: [String: String]) {
        guard let data = try? JSONEncoder().encode(drafts) else { return }
        UserDefaults.standard.set(data, forKey: iosQuickReplyDraftsKey)
    }

    func readQuickReplyDraft(threadId: String) -> String {
        guard !threadId.isEmpty else { return "" }
        return loadQuickReplyDrafts()[threadId] ?? ""
    }

    func persistQuickReplyDraft(threadId: String, body: String) {
        guard !threadId.isEmpty else { return }
        var drafts = loadQuickReplyDrafts()
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            drafts.removeValue(forKey: threadId)
        } else {
            drafts[threadId] = body
        }
        saveQuickReplyDrafts(drafts)
    }

    func persistSelectedQuickReplyDraft() {
        persistQuickReplyDraft(threadId: selectedCoreThread?.id ?? "", body: quickReplyBody)
    }

    func defaultSendAccountId() -> String {
        if !selectedCoreAccountId.isEmpty,
           let account = coreAccounts.first(where: { $0.id == selectedCoreAccountId }),
           !MailStateKt.accountSummaryIsRss(account: account)
        {
            return selectedCoreAccountId
        }
        return coreAccounts.first(where: { !MailStateKt.accountSummaryIsRss(account: $0) })?.id ?? ""
    }

    func newIosDraftMessageId(accountId: String) -> String {
        let domain = accountId.split(separator: "@", maxSplits: 1).last.map(String.init) ?? "meron"
        let normalizedDomain = domain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "meron" : domain
        return "meron-draft-\(UUID().uuidString.lowercased())@\(normalizedDomain)"
    }

    func composeIdentityCandidates() -> [SendIdentity] {
        coreAccounts
            .filter { !MailStateKt.accountSummaryIsRss(account: $0) && !$0.needsReconnect }
            .flatMap { MailStateKt.accountSendIdentities(account: $0) }
    }

    func selectedComposeIdentity() -> SendIdentity? {
        let candidates = composeIdentityCandidates()
        if let selected = candidates.first(where: { $0.accountId == composeFromAccountId && $0.email == composeFromEmail }) {
            return selected
        }
        let defaultAccountId = defaultSendAccountId()
        return candidates.first(where: { $0.accountId == defaultAccountId }) ?? candidates.first
    }

    func clearComposeDraftState() {
        composeAutosaveTask?.cancel()
        composeAutosaveTask = nil
        composeLastAutosaveSignature = ""
        composeTo = ""
        composeCc = ""
        composeBcc = ""
        composeSubject = ""
        composeBody = ""
        attachments = []
        composeInlineAttachmentIds = []
        composeFromAccountId = ""
        composeFromEmail = ""
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = ""
        composeReferences = ""
        composeReplyTo = ""
        composeCcBccVisible = false
        composeDiscardConfirming = false
        composeNoSubjectConfirming = false
        composeSending = false
        composeRichText = false
        recipientSuggestionField = ""
        recipientSuggestions = []
        mailtoDraft = nil
    }

    var composeHasContent: Bool {
        composeDraftHasContent(
            to: composeTo,
            cc: composeCc,
            bcc: composeBcc,
            subject: composeSubject,
            body: composeBody,
            attachments: attachments
        )
    }

    var composeCanSend: Bool {
        composeDraftCanSubmit(
            identityAvailable: selectedComposeIdentity() != nil,
            to: composeTo,
            subject: composeSubject,
            body: composeBody,
            attachments: attachments,
            sending: composeSending
        )
    }

    func currentComposeAutosaveSignature() -> String {
        let identity = selectedComposeIdentity()
        return composeDraftAutosaveSignature(
            accountId: identity?.accountId ?? defaultSendAccountId(),
            fromEmail: identity?.email ?? "",
            to: composeTo,
            cc: composeCc,
            bcc: composeBcc,
            subject: composeSubject,
            body: composeBody,
            rich: composeRichText,
            replyTo: composeReplyTo,
            inReplyTo: composeInReplyTo,
            references: composeReferences,
            inlineAttachmentIds: composeInlineAttachmentIds,
            attachments: attachments
        )
    }

    func scheduleComposeAutosave() {
        composeAutosaveTask?.cancel()
        guard composeHasContent else {
            composeAutosaveTask = nil
            composeLastAutosaveSignature = ""
            return
        }
        let signature = currentComposeAutosaveSignature()
        guard signature != composeLastAutosaveSignature else {
            composeAutosaveTask = nil
            return
        }
        composeAutosaveTask = Task {
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard currentComposeAutosaveSignature() == signature else { return }
                _ = saveComposeDraft(manual: false, refreshMailbox: false)
            }
        }
    }

    var shouldShowComposeCcBcc: Bool {
        composeCcBccVisible ||
            !composeCc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !composeBcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    func appendComposeMarkdownSnippet(_ snippet: String) {
        let trimmed = composeBody.trimmingCharacters(in: .whitespacesAndNewlines)
        composeRichText = true
        composeBody = trimmed.isEmpty ? snippet : "\(composeBody)\n\n\(snippet)"
    }

    func composeInlineContentId(for attachment: DraftAttachment) -> String {
        "ios-\(attachment.id.replacingOccurrences(of: "@", with: "-"))@meron"
    }

    func composeAttachmentInputs(html: String) -> [MobileAttachmentInput] {
        attachments.compactMap { attachment in
            let inlineId = !html.isEmpty && composeInlineAttachmentIds.contains(attachment.id)
                ? composeInlineContentId(for: attachment)
                : ""
            if !inlineId.isEmpty, !html.contains("cid:\(inlineId)") {
                return nil
            }
            return MobileAttachmentInput(
                filename: attachment.displayName,
                mime: attachment.mimeType,
                data: attachment.dataBase64,
                inlineId: inlineId
            )
        }
    }

    func requestDiscardComposeDraft() {
        guard !composeSending else { return }
        if composeHasContent {
            composeDiscardConfirming = true
        } else {
            discardComposeDraft()
        }
    }

    func closeComposeSurface() {
        guard !composeSending else { return }
        if composeHasContent {
            guard saveComposeDraft(manual: true, refreshMailbox: false) else {
                return
            }
        }
        finishComposeAndReturn()
    }

    func finishComposeAndReturn() {
        clearComposeDraftState()
        let destination = composeReturnTab == .starred && !showStarredTab ? IosAppTab.mail : composeReturnTab
        composeReturnTab = .mail
        selectedTab = destination
        if destination == .mail {
            focusMailShortcutSurfaceIfIdle()
        }
    }

    func loadRecipientSuggestions(field: String, value: String) {
        let accountId = defaultSendAccountId()
        recipientSuggestionField = field
        guard !accountId.isEmpty else {
            recipientSuggestions = []
            return
        }
        let request = MobileCommandsKt.contactSuggestRequest(
            id: 60,
            params: ContactSuggestParams(
                accountId: accountId,
                query: MailStateKt.recipientTail(value: value),
                limit: 6
            )
        ).toJson()
        let response = RustCoreBridge.invokeJson(request)
        if response.contains(#""error""#) {
            recipientSuggestions = []
            return
        }
        recipientSuggestions = MobileResponseParsersKt.parseContactSuggestResponse(responseJson: response)
    }

    func acceptRecipientSuggestion(field: String, contact: ContactSuggestion) {
        switch field {
        case "to":
            composeTo = MailStateKt.replaceRecipientTail(value: composeTo, contact: contact)
        case "cc":
            composeCc = MailStateKt.replaceRecipientTail(value: composeCc, contact: contact)
        case "bcc":
            composeBcc = MailStateKt.replaceRecipientTail(value: composeBcc, contact: contact)
        default:
            break
        }
        recipientSuggestions = []
    }

    func sendCoreMail() {
        guard !composeSending else { return }
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty else {
            accountStatus = String(localized: "mobile.ios.selectAccountBeforeSending")
            return
        }
        guard composeDraftCanSend(to: composeTo, subject: composeSubject, body: composeBody, attachments: attachments) else {
            accountStatus = String(localized: "mobile.ios.completeMessageBeforeSending")
            return
        }
        if composeDraftNeedsNoSubjectConfirmation(subject: composeSubject) {
            composeNoSubjectConfirming = true
            return
        }
        performSendCoreMail(accountId: accountId, identity: identity)
    }

    func confirmSendCoreMailWithoutSubject() {
        guard !composeSending else { return }
        composeNoSubjectConfirming = false
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty else {
            accountStatus = String(localized: "mobile.ios.selectAccountBeforeSending")
            return
        }
        guard composeDraftCanSend(to: composeTo, subject: composeSubject, body: composeBody, attachments: attachments) else {
            accountStatus = String(localized: "mobile.ios.completeMessageBeforeSending")
            return
        }
        performSendCoreMail(accountId: accountId, identity: identity)
    }

    private func performSendCoreMail(accountId: String, identity: SendIdentity?) {
        let trimmedBody = composeBody.trimmingCharacters(in: .whitespacesAndNewlines)
        let html = composeRichText ? composeBodyAsSimpleHtml(trimmedBody) : ""
        let attachmentInputs = composeAttachmentInputs(html: html)
        let request = MobileCommandsKt.sendMailRequest(
            id: 22,
            params: SendMailParams(
                accountId: accountId,
                to: composeTo.trimmingCharacters(in: .whitespacesAndNewlines),
                subject: composeSubject.trimmingCharacters(in: .whitespacesAndNewlines),
                body: trimmedBody,
                from: identity?.email ?? "",
                cc: composeCc.trimmingCharacters(in: .whitespacesAndNewlines),
                bcc: composeBcc.trimmingCharacters(in: .whitespacesAndNewlines),
                replyTo: composeReplyTo.trimmingCharacters(in: .whitespacesAndNewlines),
                html: html,
                inReplyTo: composeInReplyTo,
                references: composeReferences,
                messageId: "",
                attachments: attachmentInputs
            )
        ).toJson()
        let savedDraftId = composeDraftSaved ? composeDraftId : ""
        let selectedFolder = selectedCoreFolder
        let refreshUnified = selectedMailboxAccountId() == iosUnifiedAccountId
        composeAutosaveTask?.cancel()
        composeAutosaveTask = nil
        composeSending = true
        accountStatus = String(localized: "composer.status.sending")
        Task {
            let response = await Task.detached {
                let sendResponse = RustCoreBridge.invokeJson(request)
                if !sendResponse.contains(#""error""#), !savedDraftId.isEmpty {
                    _ = RustCoreBridge.invokeJson(
                        MobileCommandsKt.discardDraftRequest(
                            id: 62,
                            params: DiscardDraftParams(accountId: accountId, draftId: savedDraftId)
                        ).toJson()
                    )
                }
                return sendResponse
            }.value
            composeSending = false
            handleComposeSendResponse(response, accountId: accountId, selectedFolder: selectedFolder, refreshUnified: refreshUnified)
        }
    }

    func handleComposeSendResponse(_ response: String, accountId: String, selectedFolder: String, refreshUnified: Bool) {
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.coreSendFailed")
            accountJson = response
            return
        }
        finishComposeAndReturn()
        accountStatus = String(localized: "mobile.ios.sentThroughCore")
        if refreshUnified {
            loadUnifiedInbox(syncFirst: false)
        } else {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedFolder)
        }
    }

    @discardableResult
    func saveComposeDraft(manual: Bool = true, refreshMailbox: Bool = true) -> Bool {
        guard !composeSending else { return false }
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty else {
            if manual {
                accountStatus = String(localized: "mobile.ios.selectAccountBeforeSaving")
            }
            return false
        }
        let hasContent = composeDraftHasContent(
            to: composeTo,
            cc: composeCc,
            bcc: composeBcc,
            subject: composeSubject,
            body: composeBody,
            attachments: attachments
        )
        guard hasContent else {
            if manual {
                accountStatus = String(localized: "mobile.ios.nothingToSave")
            }
            return false
        }
        if composeDraftId.isEmpty {
            composeDraftId = newIosDraftMessageId(accountId: accountId)
        }
        let trimmedBody = composeBody.trimmingCharacters(in: .whitespacesAndNewlines)
        let html = composeRichText ? composeBodyAsSimpleHtml(trimmedBody) : ""
        let attachmentInputs = composeAttachmentInputs(html: html)
        let params = SaveDraftParams(
            accountId: accountId,
            draftId: composeDraftId,
            to: composeTo.trimmingCharacters(in: .whitespacesAndNewlines),
            subject: composeSubject.trimmingCharacters(in: .whitespacesAndNewlines),
            body: trimmedBody,
            from: identity?.email ?? "",
            cc: composeCc.trimmingCharacters(in: .whitespacesAndNewlines),
            bcc: composeBcc.trimmingCharacters(in: .whitespacesAndNewlines),
            replyTo: composeReplyTo.trimmingCharacters(in: .whitespacesAndNewlines),
            html: html,
            inReplyTo: composeInReplyTo,
            references: composeReferences,
            attachments: attachmentInputs
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.saveDraftRequest(id: 61, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = manual
                ? String(localized: "mobile.ios.draftSaveFailed")
                : String(localized: "composer.status.draftAutosaveFailed")
            accountJson = response
            return false
        }
        composeDraftSaved = true
        composeLastAutosaveSignature = currentComposeAutosaveSignature()
        if manual {
            accountStatus = String(localized: "mobile.ios.draftSaved")
        }
        if refreshMailbox {
            if selectedMailboxAccountId() == iosUnifiedAccountId {
                loadUnifiedInbox(syncFirst: false)
            } else {
                loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
            }
        }
        return true
    }

    func discardComposeDraft() {
        guard !composeSending else { return }
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        if composeDraftSaved, !composeDraftId.isEmpty, accountId.isEmpty {
            accountStatus = String(localized: "mobile.ios.selectAccountBeforeDiscarding")
            return
        }
        if composeDraftSaved, !composeDraftId.isEmpty {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.discardDraftRequest(
                    id: 63,
                    params: DiscardDraftParams(accountId: accountId, draftId: composeDraftId)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = String(localized: "mobile.ios.draftDiscardFailed")
                accountJson = response
                return
            }
        }
        finishComposeAndReturn()
        accountStatus = String(localized: "mobile.ios.draftDiscarded")
        if selectedMailboxAccountId() == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else if !accountId.isEmpty {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
    }

    func sendQuickReply() {
        guard !quickReplySending else { return }
        let accountId = selectedCoreThread?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty, let selectedCoreThread, let parent = coreMessages.last else {
            accountStatus = String(localized: "mobile.ios.openMailThreadBeforeReplying")
            return
        }
        guard !isRssThread(selectedCoreThread) else {
            accountStatus = String(localized: "mobile.ios.rssThreadsDoNotSupportReplies")
            return
        }
        let body = quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty || !quickReplyAttachments.isEmpty else {
            accountStatus = String(localized: "mobile.ios.writeReplyBeforeSending")
            return
        }

        quickReplyFailure = ""
        quickReplySending = true
        accountStatus = String(localized: "composer.status.sending")
        let threadId = selectedCoreThread.id
        let request = MobileCommandsKt.sendMailRequest(
            id: 27,
            params: parent.toReplyMailParams(
                accountId: accountId,
                body: body,
                from: coreAccounts.first(where: { $0.id == accountId })
                    .map { MailStateKt.detectReplyFromIdentity(message: parent, account: $0) } ?? "",
                ownAddresses: MailStateKt.ownAddressList(accounts: coreAccounts),
                attachments: quickReplyAttachments
            )
        ).toJson()
        Task {
            let response = await Task.detached {
                RustCoreBridge.invokeJson(request)
            }.value
            quickReplySending = false
            handleQuickReplySendResponse(response, threadId: threadId)
        }
    }

    func handleQuickReplySendResponse(_ response: String, threadId: String) {
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.quickReplyFailed")
            quickReplyFailure = String(localized: "reply.failedDraftKept")
            accountJson = response
            return
        }
        persistQuickReplyDraft(threadId: threadId, body: "")
        quickReplyBody = ""
        quickReplyAttachments = []
        quickReplyFailure = ""
        accountStatus = String(localized: "mobile.ios.quickReplySent")
    }

    func openQuickReplyInFullEditor() {
        let accountId = selectedCoreThread?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty, let selectedCoreThread, let parent = coreMessages.last else {
            accountStatus = String(localized: "mobile.ios.openMailThreadBeforeReplying")
            return
        }
        guard !isRssThread(selectedCoreThread) else {
            accountStatus = String(localized: "mobile.ios.rssThreadsDoNotSupportReplies")
            return
        }
        let params = parent.toReplyMailParams(
            accountId: accountId,
            body: quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines),
            from: coreAccounts.first(where: { $0.id == accountId })
                .map { MailStateKt.detectReplyFromIdentity(message: parent, account: $0) } ?? "",
            ownAddresses: MailStateKt.ownAddressList(accounts: coreAccounts),
            attachments: quickReplyAttachments
        )
        composeTo = params.to
        composeCc = params.cc
        composeBcc = params.bcc
        composeCcBccVisible = !params.cc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !params.bcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        composeSubject = params.subject
        composeBody = params.body
        attachments = quickReplyAttachments
        composeFromAccountId = accountId
        composeFromEmail = params.from
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = params.inReplyTo
        composeReferences = params.references
        composeReplyTo = params.replyTo
        composeRichText = false
        persistQuickReplyDraft(threadId: selectedCoreThread.id, body: "")
        quickReplyBody = ""
        quickReplyAttachments = []
        quickReplyFailure = ""
        composeReturnTab = iosComposeReturnTab(from: selectedTab)
        selectedTab = .compose
        accountStatus = String(localized: "mobile.ios.replyOpenedInFullEditor")
    }
}
