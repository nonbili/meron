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
                    Text("Manage providers, background refresh, and core diagnostics.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            Section("Background Refresh") {
                Button {
                    IosBackgroundRefresh.runOnce { summary in
                        backgroundRefreshStatus = summary
                    }
                } label: {
                    Label("Run Background Refresh", systemImage: "arrow.clockwise")
                }
                Button {
                    IosNotificationService.requestAuthorization { granted in
                        notificationStatus = granted ? "Notifications enabled." : "Notifications disabled."
                    }
                } label: {
                    Label("Enable Notifications", systemImage: "bell")
                }
                LabeledContent("Refresh", value: backgroundRefreshStatus)
                LabeledContent("Notifications", value: notificationStatus)
            }

            Section("Appearance") {
                Picker("Theme", selection: $appearanceMode) {
                    ForEach(iosThemeOptions) { option in
                        Text(option.name).tag(option.id)
                    }
                }
                Toggle("Show sender images", isOn: $showSenderImages)
            }

            Section("Navigation") {
                Toggle("Show Unified inbox", isOn: $showUnifiedInbox)
                Toggle("Show Starred tab", isOn: $showStarredTab)
                Toggle("Show unread badges", isOn: $showUnreadBadges)
            }

            Section("Composer") {
                Picker("Send shortcut", selection: $sendShortcutMode) {
                    Text("Cmd/Ctrl+Enter").tag("mod_enter")
                    Text("Enter").tag("enter")
                }
                Text("\(sendShortcutLabel(sendShortcutMode)) sends from hardware keyboards.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section("Kanban") {
                Stepper(
                    value: $kanbanColumnWidth,
                    in: iosKanbanColumnMinWidth ... iosKanbanColumnMaxWidth,
                    step: 20
                ) {
                    LabeledContent("Column width", value: "\(Int(kanbanColumnWidth)) pt")
                }
            }

            Section("Storage") {
                LabeledContent("Cache", value: formattedStorageBytes(storageCacheBytes))
                LabeledContent("Database", value: formattedStorageBytes(storageDbBytes))
                Button {
                    loadStorageUsage(showStatus: true)
                } label: {
                    Label(storageBusy ? "Refreshing" : "Refresh Usage", systemImage: "arrow.clockwise")
                }
                .disabled(storageBusy)

                Button(role: storageClearConfirming ? .destructive : nil) {
                    clearStorageCache()
                } label: {
                    Label(storageClearConfirming ? "Confirm Clear Cache" : "Clear Cache", systemImage: "trash")
                }
                .disabled(storageBusy || (storageCacheBytes ?? 0) == 0)
            }

            Section("About") {
                LabeledContent("Version", value: "\(appVersion) (\(appBuild))")
                LabeledContent("Core protocol", value: "\(rustProtocolVersion)")
                LabeledContent("Shared protocol", value: "\(protocolVersion)")
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
                Section("Configured Accounts") {
                    ForEach(Array(coreAccounts.enumerated()), id: \.element.id) { index, account in
                        DisclosureGroup {
                            HStack {
                                Button {
                                    moveAccount(account, delta: -1)
                                } label: {
                                    Label("Move Up", systemImage: "arrow.up")
                                }
                                .disabled(index == 0)

                                Button {
                                    moveAccount(account, delta: 1)
                                } label: {
                                    Label("Move Down", systemImage: "arrow.down")
                                }
                                .disabled(index == coreAccounts.count - 1)

                                if account.needsReconnect, !MailStateKt.accountSummaryIsRss(account: account) {
                                    Button {
                                        reconnectAccount(account)
                                    } label: {
                                        Label("Reconnect", systemImage: "key")
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

            Section("Password Account") {
                TextField("Email", text: $accountEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Username", text: $accountUsername)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField("Password", text: $accountPassword)
                TextField("Display name", text: $accountDisplayName)
                TextField("Sender name", text: $accountSenderName)
                Button {
                    autodiscoverPasswordAccount()
                } label: {
                    Label("Find Mail Settings", systemImage: "magnifyingglass")
                }
                TextField("IMAP host", text: $imapHost)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("IMAP port", text: $imapPort)
                    .keyboardType(.numberPad)
                TextField("SMTP host", text: $smtpHost)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("SMTP port", text: $smtpPort)
                    .keyboardType(.numberPad)
                Button {
                    addPasswordAccount()
                } label: {
                    Label(reconnectingAccountId.isEmpty ? "Add Password Account" : "Reconnect Password Account", systemImage: "person.badge.plus")
                }
                Button {
                    listAccounts()
                } label: {
                    Label("Reload Accounts", systemImage: "list.bullet")
                }
            }

            Section("OAuth") {
                Picker("Provider", selection: $oauthProvider) {
                    Text("Gmail").tag("gmail")
                    Text("Outlook").tag("outlook")
                }
                .pickerStyle(.segmented)
                TextField("OAuth email", text: $oauthEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("OAuth client ID", text: $oauthClientId)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField("OAuth client secret (optional)", text: $oauthClientSecret)
                TextField("Redirect URI", text: $oauthRedirectUri)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                Button {
                    launchOAuthFlow()
                } label: {
                    Label("Open OAuth in Browser", systemImage: "safari")
                }
                if !oauthAuthorizationCode.isEmpty {
                    LabeledContent("Authorization Code", value: oauthAuthorizationCode)
                }
                Button {
                    exchangeOAuthCode()
                } label: {
                    Label(reconnectingAccountId.isEmpty ? "Exchange Code And Add Account" : "Exchange Code And Reconnect", systemImage: "arrow.triangle.2.circlepath")
                }
                TextField("Access token", text: $oauthAccessToken)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                SecureField("Refresh token", text: $oauthRefreshToken)
                TextField("Token expires at", text: $oauthExpiresAt)
                    .keyboardType(.numberPad)
                Button {
                    addOAuthAccount()
                } label: {
                    Label(reconnectingAccountId.isEmpty ? "Add OAuth Account" : "Reconnect OAuth Account", systemImage: "person.crop.circle.badge.checkmark")
                }
            }

            Section("RSS") {
                TextField("RSS feed URL", text: $rssFeedUrl)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("RSS name", text: $rssDisplayName)
                Button {
                    addRssAccount()
                } label: {
                    Label("Add RSS Account", systemImage: "dot.radiowaves.left.and.right")
                }
            }

            Section("Diagnostics") {
                DisclosureGroup("Core contract") {
                    LabeledContent("Expected protocol", value: "\(protocolVersion)")
                    LabeledContent("Rust protocol", value: "\(rustProtocolVersion)")
                    LabeledContent("Command", value: MobileCommand.shared.ThreadList)
                    DiagnosticText(title: "Init", value: rustInitJson)
                    DiagnosticText(title: "Ping", value: rustPingJson)
                    DiagnosticText(title: "Generated request", value: threadListJson)
                    ForEach(rustReadyEvents, id: \.self) { event in
                        DiagnosticText(title: "Event", value: event)
                    }
                }
                if !accountJson.isEmpty {
                    DisclosureGroup("Last core response") {
                        Text(accountJson)
                            .font(.system(.footnote, design: .monospaced))
                            .textSelection(.enabled)
                    }
                }
                Label("Use registered Gmail/Outlook mobile client IDs and exact redirect URIs.", systemImage: "safari")
            }
        }
        .listStyle(.insetGrouped)
    }
}
