import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func listAccounts() {
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 10).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = "Loaded accounts."
    }

    func loadStorageUsage(showStatus: Bool = true) {
        storageBusy = true
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.storageUsageRequest(id: 40).toJson())
        storageBusy = false
        if response.contains(#""error""#) {
            if showStatus { accountStatus = "Storage usage failed." }
            return
        }
        let usage = MobileResponseParsersKt.parseStorageUsageResponse(responseJson: response)
        storageCacheBytes = usage.cacheBytes
        storageDbBytes = usage.dbBytes
        if showStatus { accountStatus = "Loaded storage usage." }
    }

    func clearStorageCache() {
        if !storageClearConfirming {
            storageClearConfirming = true
            accountStatus = "Tap clear cache again to confirm."
            DispatchQueue.main.asyncAfter(deadline: .now() + 4) {
                storageClearConfirming = false
            }
            return
        }
        storageClearConfirming = false
        storageBusy = true
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.storageClearCacheRequest(id: 41).toJson())
        storageBusy = false
        if response.contains(#""error""#) {
            accountStatus = "Clear cache failed."
            return
        }
        let usage = MobileResponseParsersKt.parseStorageUsageResponse(responseJson: response)
        storageCacheBytes = usage.cacheBytes
        storageDbBytes = usage.dbBytes
        accountStatus = "Cleared cached attachments."
    }

    func autodiscoverPasswordAccount() {
        let email = accountEmail.trimmingCharacters(in: .whitespacesAndNewlines)
        guard email.contains("@"), !email.hasSuffix("@") else {
            accountStatus = "Enter an email address first."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.accountAutodiscoverRequest(id: 57, params: AutodiscoverAccountParams(email: email)).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Settings lookup failed."
            accountJson = response
            return
        }
        let discovered = MobileResponseParsersKt.parseAutodiscoverResponse(responseJson: response)
        if !discovered.imapHost.isEmpty {
            imapHost = discovered.imapHost
        }
        if discovered.imapPort > 0 {
            imapPort = "\(discovered.imapPort)"
        }
        if !discovered.smtpHost.isEmpty {
            smtpHost = discovered.smtpHost
        }
        if discovered.smtpPort > 0 {
            smtpPort = "\(discovered.smtpPort)"
        }
        if !discovered.username.isEmpty {
            accountUsername = discovered.username
        }
        if !discovered.appPasswordProvider.isEmpty {
            accountStatus = "\(discovered.providerName.isEmpty ? discovered.appPasswordProvider : discovered.providerName) settings found. Use an app password."
        } else if discovered.source == "guess" {
            accountStatus = "Settings guessed. Verify the servers before adding."
        } else if !discovered.providerName.isEmpty {
            accountStatus = "Settings found for \(discovered.providerName)."
        } else {
            accountStatus = "Settings found."
        }
    }

    func addPasswordAccount() {
        let params = AddPasswordAccountParams(
            email: accountEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            imapHost: imapHost.trimmingCharacters(in: .whitespacesAndNewlines),
            imapPort: Int32(imapPort) ?? 993,
            smtpHost: smtpHost.trimmingCharacters(in: .whitespacesAndNewlines),
            smtpPort: Int32(smtpPort) ?? 465,
            username: accountUsername.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                ? accountEmail.trimmingCharacters(in: .whitespacesAndNewlines)
                : accountUsername.trimmingCharacters(in: .whitespacesAndNewlines),
            password: accountPassword,
            tls: true
        )
        let request = MobileCommandsKt.accountAddPasswordRequest(id: 11, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        let failed = response.contains(#""error""#)
        accountStatus = failed ? "Password account failed." : (reconnectingAccountId.isEmpty ? "Password account added." : "Password account reconnected.")
        if failed {
            accountJson = response
            return
        }
        reconnectingAccountId = ""
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 12).toJson())
        updateCoreAccounts(from: accountJson)
    }

    func addRssAccount() {
        let params = AddRssAccountParams(
            feedUrl: rssFeedUrl.trimmingCharacters(in: .whitespacesAndNewlines),
            displayName: rssDisplayName.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        let request = MobileCommandsKt.accountAddRssRequest(id: 13, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        accountStatus = response.contains(#""error""#) ? "RSS account failed." : "RSS account added."
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 14).toJson())
        updateCoreAccounts(from: accountJson)
    }

    func addOAuthAccount() {
        let refreshToken = oauthRefreshToken.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !refreshToken.isEmpty else {
            accountStatus = "OAuth refresh token is required."
            return
        }
        let params = AddOAuthAccountParams(
            email: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            provider: oauthProvider,
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            username: "",
            avatarUrl: "",
            accessToken: oauthAccessToken.trimmingCharacters(in: .whitespacesAndNewlines),
            refreshToken: refreshToken,
            tokenExpiresAt: Int64(oauthExpiresAt) ?? 0,
            imapHost: "",
            imapPort: nil,
            smtpHost: "",
            smtpPort: nil
        )
        let request = MobileCommandsKt.accountAddOAuthRequest(id: 23, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        let failed = response.contains(#""error""#)
        accountStatus = failed ? "OAuth account failed." : (reconnectingAccountId.isEmpty ? "OAuth account added." : "OAuth account reconnected.")
        if failed {
            accountJson = response
            return
        }
        reconnectingAccountId = ""
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 24).toJson())
        updateCoreAccounts(from: accountJson)
    }

    func saveAccountSettings(account: AccountSummary, draft: AccountSettingsDraft) {
        let accountId = account.id
        setAccountNavigationVisible(accountId: accountId, visible: draft.showInNavigation)
        let isRss = MailStateKt.accountSummaryIsRss(account: account)
        let aliases = draft.aliasesText
            .split(separator: "\n")
            .compactMap { line -> AccountAliasParams? in
                let parts = line.split(separator: ",", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                guard let email = parts.first, !email.isEmpty else { return nil }
                return AccountAliasParams(email: email, name: parts.count > 1 ? parts[1] : "")
            }

        let requests = [
            MobileCommandsKt.accountSetNameRequest(
                id: 41,
                params: AccountNameParams(accountId: accountId, name: draft.displayName.trimmingCharacters(in: .whitespacesAndNewlines))
            ).toJson(),
            MobileCommandsKt.accountSetAvatarRequest(
                id: 53,
                params: AccountAvatarParams(accountId: accountId, avatarUrl: draft.avatarUrl.trimmingCharacters(in: .whitespacesAndNewlines))
            ).toJson(),
            MobileCommandsKt.accountSetChatWallpaperRequest(
                id: 54,
                params: AccountChatWallpaperParams(accountId: accountId, presetId: draft.wallpaperPresetId.trimmingCharacters(in: .whitespacesAndNewlines), customUrl: "")
            ).toJson(),
            MobileCommandsKt.accountSetImagesRequest(
                id: 42,
                params: AccountFlagParams(accountId: accountId, enabled: draft.loadRemoteImages)
            ).toJson(),
            MobileCommandsKt.accountSetConversationHtmlRequest(
                id: 43,
                params: AccountFlagParams(accountId: accountId, enabled: draft.conversationHtml)
            ).toJson(),
            MobileCommandsKt.accountSetUnifiedRequest(
                id: 44,
                params: AccountFlagParams(accountId: accountId, enabled: draft.includedInUnified)
            ).toJson(),
            MobileCommandsKt.accountSetMutedRequest(
                id: 45,
                params: AccountFlagParams(accountId: accountId, enabled: draft.muted)
            ).toJson(),
            MobileCommandsKt.accountSetPausedRequest(
                id: 46,
                params: AccountFlagParams(accountId: accountId, enabled: draft.paused)
            ).toJson(),
        ]
        for request in requests {
            let response = RustCoreBridge.invokeJson(request)
            if response.contains(#""error""#) {
                accountStatus = "Account settings failed."
                accountJson = response
                return
            }
        }
        if isRss {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.accountSetRssSyncIntervalRequest(
                    id: 47,
                    params: AccountRssSyncIntervalParams(accountId: accountId, minutes: Int32(draft.rssSyncIntervalMinutes))
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "Account settings failed."
                accountJson = response
                return
            }
        } else {
            let senderResponse = RustCoreBridge.invokeJson(
                MobileCommandsKt.accountSetSenderNameRequest(
                    id: 48,
                    params: AccountNameParams(accountId: accountId, name: draft.senderName.trimmingCharacters(in: .whitespacesAndNewlines))
                ).toJson()
            )
            if senderResponse.contains(#""error""#) {
                accountStatus = "Account settings failed."
                accountJson = senderResponse
                return
            }
            let aliasesResponse = RustCoreBridge.invokeJson(
                MobileCommandsKt.accountSetAliasesRequest(
                    id: 49,
                    params: AccountAliasesParams(accountId: accountId, aliases: aliases)
                ).toJson()
            )
            if aliasesResponse.contains(#""error""#) {
                accountStatus = "Account settings failed."
                accountJson = aliasesResponse
                return
            }
        }

        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 50).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = "Saved account settings."
        if selectedCoreAccountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
        } else if selectedCoreAccountId == accountId {
            loadCoreFoldersAndThreads(accountId: accountId, requestedFolder: selectedCoreFolder)
        }
    }

    func removeAccount(_ account: AccountSummary) {
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.accountRemoveRequest(
                id: 51,
                params: AccountIdParams(accountId: account.id)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Remove account failed."
            accountJson = response
            return
        }
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 52).toJson())
        updateCoreAccounts(from: accountJson)
        setAccountNavigationVisible(accountId: account.id, visible: true)
        if selectedCoreAccountId == account.id {
            selectedCoreAccountId = coreAccounts.isEmpty ? "" : iosUnifiedAccountId
            selectedCoreFolder = "inbox"
            coreThreads = []
            coreMessages = []
            selectedCoreThread = nil
            mailboxCursor = ""
            mailboxAccountCursors = [:]
        }
        accountStatus = "Removed account."
    }

    func moveAccount(_ account: AccountSummary, delta: Int) {
        guard let oldIndex = coreAccounts.firstIndex(where: { $0.id == account.id }) else {
            return
        }
        let newIndex = min(max(oldIndex + delta, 0), coreAccounts.count - 1)
        guard oldIndex != newIndex else {
            return
        }
        var next = coreAccounts
        let moved = next.remove(at: oldIndex)
        next.insert(moved, at: newIndex)
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.accountReorderRequest(
                id: 58,
                params: AccountReorderParams(accountIds: next.map { $0.id })
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Move account failed."
            accountJson = response
            return
        }
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 59).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = "Moved account."
    }

    func reconnectAccount(_ account: AccountSummary) {
        guard !MailStateKt.accountSummaryIsRss(account: account) else {
            accountStatus = "RSS accounts do not need reconnect."
            return
        }
        reconnectingAccountId = account.id
        accountEmail = account.email
        accountUsername = account.email
        accountPassword = ""
        accountDisplayName = account.displayName
        accountSenderName = account.senderName
        if !account.imapHost.isEmpty {
            imapHost = account.imapHost
        }
        if account.imapPort > 0 {
            imapPort = "\(account.imapPort)"
        }
        if !account.smtpHost.isEmpty {
            smtpHost = account.smtpHost
        }
        if account.smtpPort > 0 {
            smtpPort = "\(account.smtpPort)"
        }
        oauthEmail = account.email
        oauthAuthorizationCode = ""
        oauthAccessToken = ""
        oauthRefreshToken = ""
        oauthExpiresAt = "0"
        if account.provider == "gmail" || account.authType == "gmail_oauth" {
            oauthProvider = "gmail"
        } else if account.provider == "outlook" || account.authType == "outlook_oauth" {
            oauthProvider = "outlook"
        }
        selectedTab = .accounts
        accountStatus = "Reconnect \(accountLabel(account)) from the prefilled account form."
    }

    func launchOAuthFlow() {
        let clientId = oauthClientId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clientId.isEmpty else {
            accountStatus = "OAuth client ID is required."
            return
        }
        oauthState = UUID().uuidString
        oauthVerifier = UUID().uuidString + UUID().uuidString
        let params = OAuthAuthorizationRequest(
            provider: oauthProvider,
            clientId: clientId,
            redirectUri: oauthRedirectUri.trimmingCharacters(in: .whitespacesAndNewlines),
            state: oauthState,
            codeChallenge: pkceChallenge(oauthVerifier),
            loginHint: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        guard let url = URL(string: OAuthFlowKt.buildOAuthAuthorizationUrl(request: params)) else {
            accountStatus = "OAuth URL could not be built."
            return
        }
        UIApplication.shared.open(url)
        accountStatus = "Opened \(oauthProvider) OAuth in browser."
    }

    func exchangeOAuthCode() {
        let code = oauthAuthorizationCode.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else {
            accountStatus = "OAuth authorization code is required."
            return
        }
        let clientId = oauthClientId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clientId.isEmpty else {
            accountStatus = "OAuth client ID is required."
            return
        }
        let params = ExchangeOAuthCodeParams(
            email: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            provider: oauthProvider,
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            code: code,
            clientId: clientId,
            clientSecret: oauthClientSecret.trimmingCharacters(in: .whitespacesAndNewlines),
            redirectUri: oauthRedirectUri.trimmingCharacters(in: .whitespacesAndNewlines),
            codeVerifier: oauthVerifier
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.accountExchangeOAuthCodeRequest(id: 25, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = "OAuth exchange failed."
            accountJson = response
            return
        }
        accountStatus = reconnectingAccountId.isEmpty ? "OAuth account added." : "OAuth account reconnected."
        reconnectingAccountId = ""
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 26).toJson())
        updateCoreAccounts(from: accountJson)
    }

    func handleOAuthCallback(_ url: URL) {
        accountStatus = applyOAuthCallbackToComposeState(
            rawUrl: url.absoluteString,
            expectedState: oauthState,
            redirectUri: oauthRedirectUri,
            authorizationCode: &oauthAuthorizationCode
        )
    }

    func updateCoreAccounts(from response: String) {
        coreAccounts = MobileResponseParsersKt.parseAccountListResponse(responseJson: response)
        if selectedCoreAccountId.isEmpty || (selectedCoreAccountId != iosUnifiedAccountId && !coreAccounts.contains(where: { $0.id == selectedCoreAccountId })) {
            selectedCoreAccountId = coreAccounts.isEmpty ? "" : iosUnifiedAccountId
        }
        if selectedCoreAccountId != iosUnifiedAccountId && hiddenNavigationAccountIds.contains(selectedCoreAccountId) {
            selectedCoreAccountId = iosUnifiedAccountId
        }
        if kanbanSelectedAccountId.isEmpty || !coreAccounts.contains(where: { $0.id == kanbanSelectedAccountId }) {
            kanbanSelectedAccountId = coreAccounts.first?.id ?? ""
        }
        ensureKanbanDefaults()
    }

    func selectedMailboxAccountId() -> String {
        if selectedCoreAccountId.isEmpty {
            return coreAccounts.isEmpty ? "" : iosUnifiedAccountId
        }
        return selectedCoreAccountId
    }

    func setAccountNavigationVisible(accountId: String, visible: Bool) {
        var hidden = hiddenNavigationAccountIds
        if visible {
            hidden.remove(accountId)
        } else {
            hidden.insert(accountId)
        }
        hiddenNavigationAccountsValue = hidden.sorted().joined(separator: "\n")
        if !visible && selectedCoreAccountId == accountId {
            selectedCoreAccountId = iosUnifiedAccountId
            selectedCoreFolder = iosInboxFolderId
            coreFolders = []
            coreThreads = []
            selectedCoreThread = nil
            coreMessages = []
            mailboxCursor = ""
            mailboxAccountCursors = [:]
            loadUnifiedInbox(syncFirst: false)
        }
    }

}
