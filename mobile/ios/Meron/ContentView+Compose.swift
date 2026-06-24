import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func defaultSendAccountId() -> String {
        if !selectedCoreAccountId.isEmpty,
           let account = coreAccounts.first(where: { $0.id == selectedCoreAccountId }),
           !MailStateKt.accountSummaryIsRss(account: account) {
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
        composeTo = ""
        composeCc = ""
        composeBcc = ""
        composeSubject = ""
        composeBody = ""
        attachments = []
        composeFromAccountId = ""
        composeFromEmail = ""
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = ""
        composeReferences = ""
        recipientSuggestionField = ""
        recipientSuggestions = []
        mailtoDraft = nil
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
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty else {
            accountStatus = "Select or add an account before sending."
            return
        }
        guard !composeTo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !composeSubject.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              (!composeBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !attachments.isEmpty) else {
            accountStatus = "Complete To, Subject, and Body or Attachments before sending."
            return
        }
        let attachmentInputs = attachments.map {
            MobileAttachmentInput(filename: $0.displayName, mime: $0.mimeType, data: $0.dataBase64, inlineId: "")
        }
        let params = SendMailParams(
            accountId: accountId,
            to: composeTo.trimmingCharacters(in: .whitespacesAndNewlines),
            subject: composeSubject.trimmingCharacters(in: .whitespacesAndNewlines),
            body: composeBody.trimmingCharacters(in: .whitespacesAndNewlines),
            from: identity?.email ?? "",
            cc: composeCc.trimmingCharacters(in: .whitespacesAndNewlines),
            bcc: composeBcc.trimmingCharacters(in: .whitespacesAndNewlines),
            replyTo: "",
            html: "",
            inReplyTo: composeInReplyTo,
            references: composeReferences,
            messageId: "",
            attachments: attachmentInputs
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.sendMailRequest(id: 22, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = "Core send failed."
            accountJson = response
            return
        }
        if composeDraftSaved && !composeDraftId.isEmpty {
            _ = RustCoreBridge.invokeJson(
                MobileCommandsKt.discardDraftRequest(
                    id: 62,
                    params: DiscardDraftParams(accountId: accountId, draftId: composeDraftId)
                ).toJson()
            )
        }
        clearComposeDraftState()
        accountStatus = "Sent through core."
        if selectedMailboxAccountId() == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
    }

    func saveComposeDraft() {
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty else {
            accountStatus = "Select or add an account before saving."
            return
        }
        let hasContent = !composeTo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !composeCc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !composeBcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !composeSubject.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !composeBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
            !attachments.isEmpty
        guard hasContent else {
            accountStatus = "Nothing to save."
            return
        }
        if composeDraftId.isEmpty {
            composeDraftId = newIosDraftMessageId(accountId: accountId)
        }
        let attachmentInputs = attachments.map {
            MobileAttachmentInput(filename: $0.displayName, mime: $0.mimeType, data: $0.dataBase64, inlineId: "")
        }
        let params = SaveDraftParams(
            accountId: accountId,
            draftId: composeDraftId,
            to: composeTo.trimmingCharacters(in: .whitespacesAndNewlines),
            subject: composeSubject.trimmingCharacters(in: .whitespacesAndNewlines),
            body: composeBody.trimmingCharacters(in: .whitespacesAndNewlines),
            from: identity?.email ?? "",
            cc: composeCc.trimmingCharacters(in: .whitespacesAndNewlines),
            bcc: composeBcc.trimmingCharacters(in: .whitespacesAndNewlines),
            replyTo: "",
            html: "",
            inReplyTo: composeInReplyTo,
            references: composeReferences,
            attachments: attachmentInputs
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.saveDraftRequest(id: 61, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = "Draft save failed."
            accountJson = response
            return
        }
        composeDraftSaved = true
        accountStatus = "Draft saved."
        if selectedMailboxAccountId() == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
    }

    func discardComposeDraft() {
        let identity = selectedComposeIdentity()
        let accountId = identity?.accountId ?? defaultSendAccountId()
        if composeDraftSaved && !composeDraftId.isEmpty && accountId.isEmpty {
            accountStatus = "Select or add an account before discarding."
            return
        }
        if composeDraftSaved && !composeDraftId.isEmpty {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.discardDraftRequest(
                    id: 63,
                    params: DiscardDraftParams(accountId: accountId, draftId: composeDraftId)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "Draft discard failed."
                accountJson = response
                return
            }
        }
        clearComposeDraftState()
        accountStatus = "Draft discarded."
        if selectedMailboxAccountId() == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else if !accountId.isEmpty {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
    }

    func sendQuickReply() {
        let accountId = selectedCoreThread?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty, let selectedCoreThread, let parent = coreMessages.last else {
            accountStatus = "Open a mail thread before replying."
            return
        }
        guard !isRssThread(selectedCoreThread) else {
            accountStatus = "RSS threads do not support replies."
            return
        }
        let body = quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !body.isEmpty || !quickReplyAttachments.isEmpty else {
            accountStatus = "Write a reply or attach a file before sending."
            return
        }

        quickReplyFailure = ""
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.sendMailRequest(
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
        )
        if response.contains(#""error""#) {
            accountStatus = "Quick reply failed."
            quickReplyFailure = "Reply failed. Your draft is still here."
            accountJson = response
            return
        }
        quickReplyBody = ""
        quickReplyAttachments = []
        quickReplyFailure = ""
        accountStatus = "Quick reply sent."
    }

    func openQuickReplyInFullEditor() {
        let accountId = selectedCoreThread?.accountId ?? defaultSendAccountId()
        guard !accountId.isEmpty, let selectedCoreThread, let parent = coreMessages.last else {
            accountStatus = "Open a mail thread before replying."
            return
        }
        guard !isRssThread(selectedCoreThread) else {
            accountStatus = "RSS threads do not support replies."
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
        composeSubject = params.subject
        composeBody = params.body
        attachments = quickReplyAttachments
        composeFromAccountId = accountId
        composeFromEmail = params.from
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = params.inReplyTo
        composeReferences = params.references
        quickReplyBody = ""
        quickReplyAttachments = []
        quickReplyFailure = ""
        selectedTab = .compose
        accountStatus = "Reply opened in full editor."
    }

}
