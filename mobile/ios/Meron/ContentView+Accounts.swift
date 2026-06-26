import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UserNotifications
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    func listAccounts() {
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 10).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = String(localized: "mobile.ios.loadedAccounts")
    }

    func loadStorageUsage(showStatus: Bool = true) {
        storageBusy = true
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.storageUsageRequest(id: 40).toJson())
        storageBusy = false
        if response.contains(#""error""#) {
            if showStatus { accountStatus = String(localized: "mobile.ios.storageUsageFailed") }
            return
        }
        let usage = MobileResponseParsersKt.parseStorageUsageResponse(responseJson: response)
        storageCacheBytes = usage.cacheBytes
        storageDbBytes = usage.dbBytes
        if showStatus { accountStatus = String(localized: "mobile.ios.loadedStorageUsage") }
    }

    func clearStorageCache() {
        if !storageClearConfirming {
            storageClearConfirming = true
            accountStatus = String(localized: "mobile.ios.tapClearCacheAgainToConfirm")
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
            accountStatus = String(localized: "mobile.ios.clearCacheFailed")
            return
        }
        let usage = MobileResponseParsersKt.parseStorageUsageResponse(responseJson: response)
        storageCacheBytes = usage.cacheBytes
        storageDbBytes = usage.dbBytes
        accountStatus = String(localized: "mobile.ios.clearedCachedAttachments")
    }

    func autodiscoverPasswordAccount() {
        let email = accountEmail.trimmingCharacters(in: .whitespacesAndNewlines)
        guard email.contains("@"), !email.hasSuffix("@") else {
            accountStatus = String(localized: "mobile.ios.enterEmailAddressFirst")
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.accountAutodiscoverRequest(id: 57, params: AutodiscoverAccountParams(email: email)).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.settingsLookupFailed")
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
            accountStatus = localizedCatalogString(
                "mobile.ios.settingsFoundUseAppPassword",
                args: ["provider": discovered.providerName.isEmpty ? discovered.appPasswordProvider : discovered.providerName]
            )
        } else if discovered.source == "guess" {
            accountStatus = String(localized: "mobile.ios.settingsGuessedVerifyServers")
        } else if !discovered.providerName.isEmpty {
            accountStatus = localizedCatalogString("mobile.ios.settingsFoundForProvider", args: ["provider": discovered.providerName])
        } else {
            accountStatus = String(localized: "mobile.ios.settingsFound")
        }
        accountAdvancedServerSettingsOpen = true
    }

    func addPasswordAccount() {
        let previousAccountIds = Set(coreAccounts.map(\.id))
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
        accountStatus = failed
            ? String(localized: "mobile.ios.passwordAccountFailed")
            : (reconnectingAccountId.isEmpty ? String(localized: "mobile.ios.passwordAccountAdded") : String(localized: "mobile.ios.passwordAccountReconnected"))
        if failed {
            accountJson = response
            return
        }
        reconnectingAccountId = ""
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 12).toJson())
        finishSuccessfulAccountSetup(accountListResponse: accountJson, preferredEmail: params.email, previousAccountIds: previousAccountIds)
    }

    func addRssAccount() {
        let previousAccountIds = Set(coreAccounts.map(\.id))
        let params = AddRssAccountParams(
            feedUrl: rssFeedUrl.trimmingCharacters(in: .whitespacesAndNewlines),
            displayName: rssDisplayName.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        let request = MobileCommandsKt.accountAddRssRequest(id: 13, params: params).toJson()
        let response = RustCoreBridge.invokeJson(request)
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.rssAccountFailed")
            accountJson = response
            return
        }
        accountStatus = String(localized: "mobile.ios.rssAccountAdded")
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 14).toJson())
        finishSuccessfulAccountSetup(accountListResponse: accountJson, preferredEmail: nil, previousAccountIds: previousAccountIds)
    }

    func addOAuthAccount() {
        let previousAccountIds = Set(coreAccounts.map(\.id))
        let refreshToken = oauthRefreshToken.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !refreshToken.isEmpty else {
            accountStatus = String(localized: "mobile.ios.oauthRefreshTokenRequired")
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
        accountStatus = failed
            ? String(localized: "mobile.ios.oauthAccountFailed")
            : (reconnectingAccountId.isEmpty ? String(localized: "mobile.ios.oauthAccountAdded") : String(localized: "mobile.ios.oauthAccountReconnected"))
        if failed {
            accountJson = response
            return
        }
        reconnectingAccountId = ""
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 24).toJson())
        finishSuccessfulAccountSetup(accountListResponse: accountJson, preferredEmail: params.email, previousAccountIds: previousAccountIds)
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
                params: AccountChatWallpaperParams(
                    accountId: accountId,
                    presetId: draft.wallpaperPresetId.trimmingCharacters(in: .whitespacesAndNewlines),
                    customUrl: draft.wallpaperUrl.trimmingCharacters(in: .whitespacesAndNewlines)
                )
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
                accountStatus = String(localized: "mobile.ios.accountSettingsFailed")
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
                accountStatus = String(localized: "mobile.ios.accountSettingsFailed")
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
                accountStatus = String(localized: "mobile.ios.accountSettingsFailed")
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
                accountStatus = String(localized: "mobile.ios.accountSettingsFailed")
                accountJson = aliasesResponse
                return
            }
        }

        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 50).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = String(localized: "mobile.ios.savedAccountSettings")
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
            accountStatus = String(localized: "mobile.ios.removeAccountFailed")
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
        accountStatus = String(localized: "mobile.ios.removedAccount")
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
                params: AccountReorderParams(accountIds: next.map(\.id))
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.moveAccountFailed")
            accountJson = response
            return
        }
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 59).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = String(localized: "mobile.ios.movedAccount")
    }

    func reconnectAccount(_ account: AccountSummary) {
        guard !MailStateKt.accountSummaryIsRss(account: account) else {
            accountStatus = String(localized: "mobile.ios.rssAccountsDoNotNeedReconnect")
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
        accountAdvancedServerSettingsOpen = true
        if account.provider == "gmail" || account.authType == "gmail_oauth" {
            oauthProvider = "gmail"
        } else if account.provider == "outlook" || account.authType == "outlook_oauth" {
            oauthProvider = "outlook"
        }
        selectedTab = .accounts
        accountStatus = localizedCatalogString("mobile.ios.reconnectAccountFromPrefilledForm", args: ["account": accountLabel(account)])
    }

    func launchOAuthFlow() {
        let clientId = iosResolvedOAuthClientId(provider: oauthProvider, configuredClientId: oauthClientId)
        guard !clientId.isEmpty else {
            accountStatus = String(localized: "mobile.ios.oauthClientIdRequired")
            return
        }
        let redirectUri = iosResolvedOAuthRedirectUri(provider: oauthProvider, configuredRedirectUri: oauthRedirectUri)
        oauthRedirectUri = redirectUri
        oauthState = UUID().uuidString
        oauthVerifier = UUID().uuidString + UUID().uuidString
        let params = OAuthAuthorizationRequest(
            provider: oauthProvider,
            clientId: clientId,
            redirectUri: redirectUri,
            state: oauthState,
            codeChallenge: pkceChallenge(oauthVerifier),
            loginHint: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines)
        )
        guard let url = URL(string: OAuthFlowKt.buildOAuthAuthorizationUrl(request: params)) else {
            accountStatus = String(localized: "mobile.ios.oauthUrlCouldNotBeBuilt")
            return
        }
        saveIosPendingOAuthFlow(
            IosPendingOAuthFlow(
                provider: oauthProvider,
                state: oauthState,
                verifier: oauthVerifier,
                redirectUri: redirectUri,
                email: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        )
        UIApplication.shared.open(url)
        accountStatus = localizedCatalogString("mobile.ios.openedProviderOAuthInBrowser", args: ["provider": oauthProvider])
    }

    func exchangeOAuthCode() {
        let previousAccountIds = Set(coreAccounts.map(\.id))
        let code = oauthAuthorizationCode.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !code.isEmpty else {
            accountStatus = String(localized: "mobile.ios.oauthAuthorizationCodeRequired")
            return
        }
        let clientId = iosResolvedOAuthClientId(provider: oauthProvider, configuredClientId: oauthClientId)
        guard !clientId.isEmpty else {
            accountStatus = String(localized: "mobile.ios.oauthClientIdRequired")
            return
        }
        let redirectUri = iosResolvedOAuthRedirectUri(provider: oauthProvider, configuredRedirectUri: oauthRedirectUri)
        oauthRedirectUri = redirectUri
        let params = ExchangeOAuthCodeParams(
            email: oauthEmail.trimmingCharacters(in: .whitespacesAndNewlines),
            provider: oauthProvider,
            displayName: accountDisplayName.trimmingCharacters(in: .whitespacesAndNewlines),
            senderName: accountSenderName.trimmingCharacters(in: .whitespacesAndNewlines),
            code: code,
            clientId: clientId,
            clientSecret: iosResolvedOAuthClientSecret(provider: oauthProvider, configuredClientSecret: oauthClientSecret),
            redirectUri: redirectUri,
            codeVerifier: oauthVerifier
        )
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.accountExchangeOAuthCodeRequest(id: 25, params: params).toJson())
        if response.contains(#""error""#) {
            accountStatus = String(localized: "mobile.ios.oauthExchangeFailed")
            accountJson = response
            return
        }
        accountStatus = reconnectingAccountId.isEmpty ? String(localized: "mobile.ios.oauthAccountAdded") : String(localized: "mobile.ios.oauthAccountReconnected")
        reconnectingAccountId = ""
        clearIosPendingOAuthFlow()
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 26).toJson())
        finishSuccessfulAccountSetup(accountListResponse: accountJson, preferredEmail: params.email, previousAccountIds: previousAccountIds)
    }

    func handleOAuthCallback(_ url: URL) {
        if let pending = loadIosPendingOAuthFlow() {
            oauthProvider = pending.provider
            oauthState = pending.state
            oauthVerifier = pending.verifier
            oauthRedirectUri = pending.redirectUri
            oauthEmail = pending.email
        }
        accountStatus = applyOAuthCallbackToComposeState(
            rawUrl: url.absoluteString,
            expectedState: oauthState,
            redirectUri: oauthRedirectUri,
            authorizationCode: &oauthAuthorizationCode
        )
        guard !oauthAuthorizationCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !accountStatus.hasPrefix(String(localized: "mobile.ios.oauthCallbackFailed"))
        else {
            return
        }
        selectedTab = .accounts
        accountStatus = String(localized: "accounts.oauth.finishingSignIn")
        clearIosPendingOAuthFlow()
        exchangeOAuthCode()
    }

    func finishSuccessfulAccountSetup(accountListResponse: String, preferredEmail: String?, previousAccountIds: Set<String>) {
        let preferred = updateCoreAccounts(from: accountListResponse, preferredEmail: preferredEmail, previousAccountIds: previousAccountIds)
        guard preferred != nil else { return }
        selectedTab = .mail
        syncSelectedAccount()
    }

    @discardableResult
    func updateCoreAccounts(from response: String, preferredEmail: String? = nil, previousAccountIds: Set<String>? = nil) -> AccountSummary? {
        coreAccounts = MobileResponseParsersKt.parseAccountListResponse(responseJson: response)
        let preferredAccount = iosPreferredAccountAfterAccountList(
            accounts: coreAccounts,
            preferredEmail: preferredEmail,
            previousAccountIds: previousAccountIds
        )
        refreshAccountInboxUnreadCounts()
        if let preferredAccount {
            selectedCoreAccountId = preferredAccount.id
            selectedCoreFolder = iosInboxFolderId
            coreFolders = []
            coreThreads = []
            selectedCoreThread = nil
            coreMessages = []
            mailboxCursor = ""
            mailboxAccountCursors = [:]
        } else if selectedCoreAccountId.isEmpty || (selectedCoreAccountId != iosUnifiedAccountId && !coreAccounts.contains(where: { $0.id == selectedCoreAccountId })) {
            selectedCoreAccountId = coreAccounts.isEmpty ? "" : iosUnifiedAccountId
        }
        if selectedCoreAccountId != iosUnifiedAccountId && hiddenNavigationAccountIds.contains(selectedCoreAccountId) {
            selectedCoreAccountId = iosUnifiedAccountId
        }
        if kanbanSelectedAccountId.isEmpty || !coreAccounts.contains(where: { $0.id == kanbanSelectedAccountId }) {
            kanbanSelectedAccountId = coreAccounts.first?.id ?? ""
        }
        ensureKanbanDefaults()
        syncIosLiveMailPush(showStatus: false)
        return preferredAccount
    }

    func refreshAccountInboxUnreadCounts() {
        var nextCounts: [String: Int] = [:]
        for account in coreAccounts {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.folderListRequest(id: 88, params: FolderListParams(accountId: account.id)).toJson()
            )
            guard !response.contains(#""error""#) else { continue }
            let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
            nextCounts[account.id] = inboxUnreadCount(folders: folders, accountId: account.id)
        }
        accountInboxUnreadCounts = nextCounts
    }

    func refreshNotificationStatus() {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            DispatchQueue.main.async {
                notificationStatus = settings.authorizationStatus == .authorized || settings.authorizationStatus == .provisional
                    ? String(localized: "mobile.accounts.notificationsEnabled")
                    : String(localized: "mobile.accounts.notificationsDisabled")
            }
        }
    }

    func syncIosLiveMailPush(showStatus: Bool) {
        IosLiveMailPush.sync(enabled: liveMailPushEnabled, accounts: coreAccounts) { status in
            liveMailPushStatus = status
            if showStatus {
                accountStatus = status
            }
        }
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
        if !visible, selectedCoreAccountId == accountId {
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
