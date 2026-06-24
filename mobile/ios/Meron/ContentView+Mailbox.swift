import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func syncSelectedAccount() {
        let accountId = selectedMailboxAccountId()
        guard !accountId.isEmpty else {
            accountStatus = "No account selected."
            return
        }
        if accountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: true)
            return
        }

        let syncResponse: String
        let requestedFolder = selectedCoreFolder.isEmpty ? "inbox" : selectedCoreFolder
        if selectedAccountIsRss(accountId) {
            let syncParams = SyncRssParams(accountId: accountId)
            syncResponse = RustCoreBridge.invokeJson(MobileCommandsKt.syncRssRequest(id: 15, params: syncParams).toJson())
        } else {
            let syncParams = SyncMailParams(accountId: accountId, folderId: requestedFolder, limit: 50, folders: true)
            syncResponse = RustCoreBridge.invokeJson(MobileCommandsKt.syncMailRequest(id: 15, params: syncParams).toJson())
        }
        if syncResponse.contains(#""error""#) {
            accountStatus = "Core sync failed."
            accountJson = syncResponse
            return
        }

        loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: requestedFolder)
    }

    func searchSelectedMailbox() {
        let accountId = selectedMailboxAccountId()
        guard !accountId.isEmpty else {
            accountStatus = "No account selected."
            return
        }
        if accountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
            return
        }
        loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
    }

    func addFeedToSelectedRssAccount() {
        let accountId = selectedCoreAccountId
        let feedUrl = addFeedUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !accountId.isEmpty, selectedAccountIsRss(accountId) else {
            accountStatus = "Select an RSS account first."
            return
        }
        guard !feedUrl.isEmpty else {
            accountStatus = "Feed URL is required."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.feedAddRequest(
                id: 44,
                params: AddRssFeedParams(accountId: accountId, feedUrl: feedUrl)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Add feed failed."
            accountJson = response
            return
        }
        addFeedUrl = ""
        accountStatus = "Feed added."
        syncSelectedAccount()
    }

    func importOpml(from url: URL) {
        let accountId = selectedCoreAccountId.isEmpty ? (coreAccounts.first?.id ?? "") : selectedCoreAccountId
        guard !accountId.isEmpty, selectedAccountIsRss(accountId) else {
            accountStatus = "Select an RSS account first."
            return
        }
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }
        do {
            let opml = try String(contentsOf: url, encoding: .utf8)
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.feedImportOpmlRequest(
                    id: 45,
                    params: ImportOpmlParams(accountId: accountId, opml: opml)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "OPML import failed."
                accountJson = response
                return
            }
            let imported = MobileResponseParsersKt.parseOpmlImportCountResponse(responseJson: response)
            accountStatus = imported == 0 ? "No new feeds imported." : "Imported \(imported) feed(s)."
            syncSelectedAccount()
        } catch {
            accountStatus = "OPML file read failed: \(error.localizedDescription)"
        }
    }

    func exportSelectedAccountOpml() {
        let accountId = selectedCoreAccountId.isEmpty ? (coreAccounts.first?.id ?? "") : selectedCoreAccountId
        guard !accountId.isEmpty, selectedAccountIsRss(accountId) else {
            accountStatus = "Select an RSS account first."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.feedExportOpmlRequest(
                id: 46,
                params: ExportOpmlParams(accountId: accountId)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "OPML export failed."
            accountJson = response
            return
        }
        let opml = MobileResponseParsersKt.parseOpmlExportResponse(responseJson: response)
        guard !opml.isEmpty else {
            accountStatus = "No OPML content to export."
            return
        }
        opmlExportDocument = OpmlDocument(text: opml)
        isOpmlExporterPresented = true
    }

    func markSelectedMailboxAllRead() {
        let accountId = selectedMailboxAccountId()
        guard !accountId.isEmpty else {
            accountStatus = "No account selected."
            return
        }
        let unreadThreads = coreThreads.filter { $0.unread }
        guard !unreadThreads.isEmpty else {
            accountStatus = "No unread messages."
            return
        }

        var failedResponse = ""
        if accountId == iosUnifiedAccountId {
            for (threadAccountId, threads) in Dictionary(grouping: unreadThreads, by: \.accountId) {
                if let account = coreAccounts.first(where: { $0.id == threadAccountId }),
                   MailStateKt.accountSummaryIsRss(account: account) {
                    for thread in threads {
                        let request = MobileCommandsKt.rssMarkReadRequest(
                            id: 44,
                            params: RssMarkReadParams(threadId: thread.id, seen: true, itemKeys: [])
                        ).toJson()
                        let response = RustCoreBridge.invokeJson(request)
                        if response.contains(#""error""#) {
                            failedResponse = response
                            break
                        }
                    }
                } else {
                    let request = MobileCommandsKt.markAllReadRequest(
                        id: 44,
                        params: MarkAllReadParams(accountId: threadAccountId, folderId: "inbox")
                    ).toJson()
                    failedResponse = RustCoreBridge.invokeJson(request)
                }
                if failedResponse.contains(#""error""#) {
                    break
                }
            }
        } else if selectedAccountIsRss(accountId) {
            for thread in unreadThreads {
                let request = MobileCommandsKt.rssMarkReadRequest(
                    id: 44,
                    params: RssMarkReadParams(threadId: thread.id, seen: true, itemKeys: [])
                ).toJson()
                let response = RustCoreBridge.invokeJson(request)
                if response.contains(#""error""#) {
                    failedResponse = response
                    break
                }
            }
        } else {
            let request = MobileCommandsKt.markAllReadRequest(
                id: 44,
                params: MarkAllReadParams(accountId: accountId, folderId: selectedCoreFolder.isEmpty ? "inbox" : selectedCoreFolder)
            ).toJson()
            failedResponse = RustCoreBridge.invokeJson(request)
        }

        if failedResponse.contains(#""error""#) {
            accountStatus = "Mark all read failed."
            accountJson = failedResponse
            return
        }

        coreThreads = coreThreads.map { thread in
            thread.unread ? ThreadSummary(
                id: thread.id,
                accountId: thread.accountId,
                folder: thread.folder,
                subject: thread.subject,
                sender: thread.sender,
                preview: thread.preview,
                unread: false,
                starred: thread.starred,
                dateEpochSeconds: thread.dateEpochSeconds,
                feedUrl: thread.feedUrl
            ) : thread
        }
        accountStatus = "Marked \(unreadThreads.count) unread item(s) read."
        if accountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
    }

}
