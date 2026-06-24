import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

extension ContentView {
    var accountsView: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 6) {
                    Text(accountStatus)
                        .font(.headline)
                    Text(String(localized: "mobile.accounts.manageSubtitle"))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            Section(String(localized: "mobile.accounts.backgroundRefresh")) {
                Button {
                    IosBackgroundRefresh.runOnce { summary in
                        backgroundRefreshStatus = summary
                    }
                } label: {
                    Label(String(localized: "mobile.accounts.runBackgroundRefresh"), systemImage: "arrow.clockwise")
                }
                Button {
                    IosNotificationService.requestAuthorization { granted in
                        notificationStatus = granted ? String(localized: "mobile.accounts.notificationsEnabled") : String(localized: "mobile.accounts.notificationsDisabled")
                    }
                } label: {
                    Label(String(localized: "mobile.accounts.enableNotifications"), systemImage: "bell")
                }
                LabeledContent(String(localized: "mobile.accounts.refresh"), value: backgroundRefreshStatus)
                LabeledContent(String(localized: "mobile.accounts.notifications"), value: notificationStatus)
            }

            Section(String(localized: "settings.pages.appearance")) {
                Picker(String(localized: "common.theme"), selection: $appearanceMode) {
                    ForEach(iosThemeOptions) { option in
                        Text(option.name).tag(option.id)
                    }
                }
                Toggle(String(localized: "settings.appearance.showSenderImages"), isOn: $showSenderImages)
            }

            Section(String(localized: "mobile.accounts.navigation")) {
                Toggle(String(localized: "settings.sideNav.showUnifiedInbox"), isOn: $showUnifiedInbox)
                Toggle(String(localized: "mobile.accounts.showStarredTab"), isOn: $showStarredTab)
                Toggle(String(localized: "mobile.accounts.showUnreadBadges"), isOn: $showUnreadBadges)
            }

            Section(String(localized: "settings.sections.composer")) {
                Picker(String(localized: "settings.composer.sendMessageWith"), selection: $sendShortcutMode) {
                    Text("Cmd/Ctrl+Enter").tag("mod_enter")
                    Text("Enter").tag("enter")
                }
                Text(String(localized: "mobile.accounts.sendShortcutHardware").replacingOccurrences(of: "{shortcut}", with: sendShortcutLabel(sendShortcutMode)))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section(String(localized: "settings.sections.kanban")) {
                Stepper(
                    value: $kanbanColumnWidth,
                    in: iosKanbanColumnMinWidth ... iosKanbanColumnMaxWidth,
                    step: 20
                ) {
                    LabeledContent(String(localized: "settings.kanban.columnWidth"), value: "\(Int(kanbanColumnWidth)) pt")
                }
            }

            Section(String(localized: "settings.sections.storage")) {
                LabeledContent(String(localized: "settings.storage.cacheLabel"), value: formattedStorageBytes(storageCacheBytes))
                LabeledContent(String(localized: "settings.storage.databaseLabel"), value: formattedStorageBytes(storageDbBytes))
                Button {
                    loadStorageUsage(showStatus: true)
                } label: {
                    Label(storageBusy ? String(localized: "common.loading") : String(localized: "mobile.accounts.refreshUsage"), systemImage: "arrow.clockwise")
                }
                .disabled(storageBusy)

                Button(role: storageClearConfirming ? .destructive : nil) {
                    clearStorageCache()
                } label: {
                    Label(storageClearConfirming ? String(localized: "mobile.accounts.confirmClearCache") : String(localized: "settings.storage.clearTitle"), systemImage: "trash")
                }
                .disabled(storageBusy || (storageCacheBytes ?? 0) == 0)
            }

            Section(String(localized: "about.title")) {
                LabeledContent(String(localized: "mobile.accounts.version"), value: "\(appVersion) (\(appBuild))")
                LabeledContent(String(localized: "mobile.accounts.coreProtocol"), value: "\(rustProtocolVersion)")
                LabeledContent(String(localized: "mobile.accounts.sharedProtocol"), value: "\(protocolVersion)")
                if let sponsorsUrl = URL(string: "https://github.com/sponsors/nonbili") {
                    Link(destination: sponsorsUrl) {
                        Label("GitHub Sponsors", systemImage: "heart")
                    }
                }
                if let liberapayUrl = URL(string: "https://liberapay.com/nonbili") {
                    Link(destination: liberapayUrl) {
                        Label("Liberapay", systemImage: "heart.circle")
                    }
                }
                if let paypalUrl = URL(string: "https://www.paypal.com/paypalme/nonbili") {
                    Link(destination: paypalUrl) {
                        Label("PayPal", systemImage: "creditcard")
                    }
                }
            }

            if !coreAccounts.isEmpty {
                Section(String(localized: "mobile.accounts.configuredAccounts")) {
                    ForEach(Array(coreAccounts.enumerated()), id: \.element.id) { index, account in
                        DisclosureGroup {
                            HStack {
                                Button {
                                    moveAccount(account, delta: -1)
                                } label: {
                                    Label(String(localized: "mobile.accounts.moveUp"), systemImage: "arrow.up")
                                }
                                .disabled(index == 0)

                                Button {
                                    moveAccount(account, delta: 1)
                                } label: {
                                    Label(String(localized: "mobile.accounts.moveDown"), systemImage: "arrow.down")
                                }
                                .disabled(index == coreAccounts.count - 1)

                                if account.needsReconnect, !MailStateKt.accountSummaryIsRss(account: account) {
                                    Button {
                                        reconnectAccount(account)
                                    } label: {
                                        Label(String(localized: "settings.account.reconnectButton"), systemImage: "key")
                                    }
                                }
                            }
                            AccountSettingsEditor(
                                account: account,
                                isRss: MailStateKt.accountSummaryIsRss(account: account),
                                onSave: { draft in
                                    saveAccountSettings(account: account, draft: draft)
                                },
                                onPickAvatar: {
                                    accountMediaImportTarget = AccountMediaImportTarget(accountId: account.id, isWallpaper: false)
                                    isAccountMediaImporterPresented = true
                                },
                                onPickWallpaper: {
                                    accountMediaImportTarget = AccountMediaImportTarget(accountId: account.id, isWallpaper: true)
                                    isAccountMediaImporterPresented = true
                                },
                                onRemove: {
                                    removeAccount(account)
                                },
                                showInNavigation: !hiddenNavigationAccountIds.contains(account.id)
                            )
                        } label: {
                            VStack(alignment: .leading, spacing: 2) {
                                Label {
                                    Text(accountLabel(account))
                                } icon: {
                                    if account.needsReconnect {
                                        Image(systemName: "exclamationmark.triangle.fill")
                                            .foregroundStyle(.orange)
                                    }
                                }
                                Text(account.email.isEmpty ? account.id : account.email)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }

            Section(String(localized: "mobile.accounts.passwordAccount")) {
                TextField(String(localized: "accounts.fields.emailAddress"), text: $accountEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField(String(localized: "accounts.fields.username"), text: $accountUsername)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField(String(localized: "accounts.fields.password"), text: $accountPassword)
                TextField(String(localized: "settings.account.displayName"), text: $accountDisplayName)
                TextField(String(localized: "settings.account.senderName"), text: $accountSenderName)
                Button {
                    autodiscoverPasswordAccount()
                } label: {
                    Label(String(localized: "mobile.accounts.findMailSettings"), systemImage: "magnifyingglass")
                }
                TextField(String(localized: "accounts.fields.imapHost"), text: $imapHost)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField(String(localized: "accounts.fields.port"), text: $imapPort)
                    .keyboardType(.numberPad)
                TextField(String(localized: "accounts.fields.smtpHost"), text: $smtpHost)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField(String(localized: "accounts.fields.port"), text: $smtpPort)
                    .keyboardType(.numberPad)
                Button {
                    addPasswordAccount()
                } label: {
                    Label(reconnectingAccountId.isEmpty ? String(localized: "mobile.accounts.addPasswordAccount") : String(localized: "mobile.accounts.reconnectPasswordAccount"), systemImage: "person.badge.plus")
                }
                Button {
                    listAccounts()
                } label: {
                    Label(String(localized: "mobile.accounts.reloadAccounts"), systemImage: "list.bullet")
                }
            }

            Section("OAuth") {
                Picker(String(localized: "mobile.accounts.provider"), selection: $oauthProvider) {
                    Text("Gmail").tag("gmail")
                    Text("Outlook").tag("outlook")
                }
                .pickerStyle(.segmented)
                TextField(String(localized: "mobile.accounts.oauthEmail"), text: $oauthEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField(String(localized: "mobile.accounts.oauthClientId"), text: $oauthClientId)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField(String(localized: "mobile.accounts.oauthClientSecretOptional"), text: $oauthClientSecret)
                TextField(String(localized: "mobile.accounts.redirectUri"), text: $oauthRedirectUri)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                Button {
                    launchOAuthFlow()
                } label: {
                    Label(String(localized: "mobile.accounts.openOAuthInBrowser"), systemImage: "safari")
                }
                if !oauthAuthorizationCode.isEmpty {
                    LabeledContent(String(localized: "mobile.accounts.authorizationCode"), value: oauthAuthorizationCode)
                }
                Button {
                    exchangeOAuthCode()
                } label: {
                    Label(reconnectingAccountId.isEmpty ? String(localized: "mobile.accounts.exchangeCodeAndAddAccount") : String(localized: "mobile.accounts.exchangeCodeAndReconnect"), systemImage: "arrow.triangle.2.circlepath")
                }
                TextField(String(localized: "mobile.accounts.accessToken"), text: $oauthAccessToken)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField(String(localized: "mobile.accounts.refreshToken"), text: $oauthRefreshToken)
                TextField(String(localized: "mobile.accounts.tokenExpiresAt"), text: $oauthExpiresAt)
                    .keyboardType(.numberPad)
                Button {
                    addOAuthAccount()
                } label: {
                    Label(reconnectingAccountId.isEmpty ? String(localized: "mobile.accounts.addOAuthAccount") : String(localized: "mobile.accounts.reconnectOAuthAccount"), systemImage: "person.crop.circle.badge.checkmark")
                }
            }

            Section("RSS") {
                TextField(String(localized: "mobile.accounts.rssFeedUrl"), text: $rssFeedUrl)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField(String(localized: "mobile.accounts.rssName"), text: $rssDisplayName)
                Button {
                    addRssAccount()
                } label: {
                    Label(String(localized: "mobile.accounts.addRssAccount"), systemImage: "dot.radiowaves.left.and.right")
                }
            }

            Section(String(localized: "mobile.accounts.diagnostics")) {
                DisclosureGroup(String(localized: "mobile.accounts.coreContract")) {
                    LabeledContent(String(localized: "mobile.accounts.expectedProtocol"), value: "\(protocolVersion)")
                    LabeledContent(String(localized: "mobile.accounts.rustProtocol"), value: "\(rustProtocolVersion)")
                    LabeledContent(String(localized: "mobile.accounts.command"), value: MobileCommand.shared.ThreadList)
                    DiagnosticText(title: "Init", value: rustInitJson)
                    DiagnosticText(title: "Ping", value: rustPingJson)
                    DiagnosticText(title: String(localized: "mobile.accounts.generatedRequest"), value: threadListJson)
                    ForEach(rustReadyEvents, id: \.self) { event in
                        DiagnosticText(title: "Event", value: event)
                    }
                }
                if !accountJson.isEmpty {
                    DisclosureGroup(String(localized: "mobile.accounts.lastCoreResponse")) {
                        Text(accountJson)
                            .font(.system(.footnote, design: .monospaced))
                            .textSelection(.enabled)
                    }
                }
                Label(String(localized: "mobile.accounts.oauthClientHint"), systemImage: "safari")
            }
        }
        .listStyle(.insetGrouped)
    }
}
