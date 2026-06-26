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
                Toggle(String(localized: "settings.liveMailPush"), isOn: $liveMailPushEnabled)
                Text(String(localized: "settings.liveMailPushHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button {
                    IosBackgroundRefresh.runOnce { summary in
                        backgroundRefreshStatus = summary
                    }
                } label: {
                    Label(String(localized: "settings.refreshBackground"), systemImage: "arrow.clockwise")
                }
                Text(String(localized: "settings.refreshBackgroundHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button {
                    IosNotificationService.requestAuthorization { granted in
                        notificationStatus = granted ? String(localized: "mobile.accounts.notificationsEnabled") : String(localized: "mobile.accounts.notificationsDisabled")
                    }
                } label: {
                    Label(String(localized: "mobile.accounts.enableNotifications"), systemImage: "bell")
                }
                Text(String(localized: "settings.notificationsEnableHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                LabeledContent(String(localized: "mobile.accounts.refresh"), value: backgroundRefreshStatus)
                if !liveMailPushStatus.isEmpty {
                    LabeledContent(String(localized: "settings.liveMailPush"), value: liveMailPushStatus)
                }
                LabeledContent(String(localized: "mobile.accounts.notifications"), value: notificationStatus)
            }

            Section(String(localized: "settings.pages.appearance")) {
                Picker(String(localized: "common.theme"), selection: $appearanceMode) {
                    ForEach(iosThemeOptions) { option in
                        Text(option.name).tag(option.id)
                    }
                }
                Picker(String(localized: "settings.language.label"), selection: Binding(
                    get: { iosNormalizedAppLanguageTag(appLanguageTag) },
                    set: { appLanguageTag = iosNormalizedAppLanguageTag($0) }
                )) {
                    Text(String(localized: "settings.language.system")).tag("")
                    ForEach(iosSupportedAppLanguages) { language in
                        Text(language.nativeName).tag(language.id)
                    }
                }
                Text(String(localized: "settings.language.hint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Toggle(String(localized: "settings.appearance.showSenderImages"), isOn: $showSenderImages)
                Text(String(localized: "settings.appearance.showSenderImagesHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section(String(localized: "mobile.accounts.navigation")) {
                Toggle(String(localized: "settings.sideNav.showUnifiedInbox"), isOn: $showUnifiedInbox)
                Text(String(localized: "settings.navigationDrawerHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Toggle(String(localized: "mobile.accounts.showStarredTab"), isOn: $showStarredTab)
                Text(String(localized: "settings.navigationDrawerHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Toggle(String(localized: "mobile.accounts.showUnreadBadges"), isOn: $showUnreadBadges)
                Text(String(localized: "settings.appearance.showUnreadAccountBadgeHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section(String(localized: "settings.sections.composer")) {
                Picker(String(localized: "settings.composer.sendMessageWith"), selection: $sendShortcutMode) {
                    Text("Cmd/Ctrl+Enter").tag("mod_enter")
                    Text("Enter").tag("enter")
                }
                Text(sendShortcutHintText(sendShortcutMode))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section(String(localized: "shortcuts.title")) {
                iosShortcutRow(keys: "Cmd/Ctrl+K", label: String(localized: "palette.label"))
                iosShortcutRow(keys: "Cmd/Ctrl+Shift+?", label: String(localized: "shortcuts.title"))
                iosShortcutRow(keys: "Cmd/Ctrl+N", label: String(localized: "composer.actions.newMessage"))
                iosShortcutRow(keys: "Cmd/Ctrl+Enter", label: String(localized: "buttons.send"))
                iosShortcutRow(keys: "Cmd/Ctrl+Shift+R", label: String(localized: "mobile.mail.syncMailbox"))
                iosShortcutRow(keys: "Cmd/Ctrl+,", label: String(localized: "mobile.tabs.accounts"))
                iosShortcutRow(keys: "Cmd/Ctrl+Shift+V", label: "\(String(localized: "mobile.tabs.mail")) / \(String(localized: "mobile.tabs.kanban"))")
                iosShortcutRow(keys: "Cmd/Ctrl+W", label: String(localized: "buttons.close"))
                iosShortcutRow(keys: "Cmd/Ctrl+1-9", label: "Go to: \(String(localized: "kanban.columns.unifiedInbox")) / \(String(localized: "kanban.board.label")) / \(String(localized: "mobile.ios.account"))")
                iosShortcutRow(keys: "J", label: String(localized: "shortcuts.nextThread"))
                iosShortcutRow(keys: "K", label: String(localized: "shortcuts.previousThread"))
                iosShortcutRow(keys: "Arrow Down", label: String(localized: "shortcuts.nextThread"))
                iosShortcutRow(keys: "Arrow Up", label: String(localized: "shortcuts.previousThread"))
                iosShortcutRow(keys: "E", label: String(localized: "threads.actions.archiveThread"))
                iosShortcutRow(keys: "S", label: String(localized: "chat.star"))
                iosShortcutRow(keys: "U", label: String(localized: "threads.actions.markAsUnread"))
                iosShortcutRow(keys: "R", label: String(localized: "shortcuts.focusQuickReply"))
                iosShortcutRow(keys: "Cmd/Ctrl+E", label: String(localized: "composer.actions.openFullEditor"))
                iosShortcutRow(keys: "I", label: String(localized: "chat.conversationDetails"))
                iosShortcutRow(keys: "Delete", label: String(localized: "buttons.delete"))
                iosShortcutRow(keys: "Shift+#", label: String(localized: "buttons.delete"))
                iosShortcutRow(keys: "Cmd/Ctrl+F", label: String(localized: "chat.searchThread"))
                iosShortcutRow(keys: "Cmd/Ctrl+Shift+F", label: String(localized: "shortcuts.focusMailboxSearch"))
            }

            Section(String(localized: "settings.sections.kanban")) {
                Stepper(
                    value: $kanbanColumnWidth,
                    in: iosKanbanColumnMinWidth ... iosKanbanColumnMaxWidth,
                    step: 20
                ) {
                    LabeledContent(String(localized: "settings.kanban.columnWidth"), value: "\(Int(kanbanColumnWidth)) pt")
                }

                if kanbanBoards.isEmpty {
                    Text(String(localized: "mobile.ios.loadAccountsToCreateBoard"))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                } else {
                    Picker(String(localized: "kanban.board.label"), selection: $activeKanbanBoardId) {
                        ForEach(kanbanBoards) { board in
                            Text(board.name).tag(board.id)
                        }
                    }
                    .onChange(of: activeKanbanBoardId) { _, next in
                        selectKanbanBoard(next)
                    }
                    HStack {
                        let boardIndex = activeKanbanBoardIndex()
                        Button {
                            moveActiveKanbanBoard(delta: -1)
                        } label: {
                            Label(String(localized: "mobile.accounts.moveUp"), systemImage: "arrow.up")
                        }
                        .disabled(boardIndex <= 0)

                        Button {
                            moveActiveKanbanBoard(delta: 1)
                        } label: {
                            Label(String(localized: "mobile.accounts.moveDown"), systemImage: "arrow.down")
                        }
                        .disabled(boardIndex < 0 || boardIndex >= kanbanBoards.count - 1)
                    }

                    HStack {
                        KanbanBoardStylePreview(board: activeKanbanBoard)
                        TextField(String(localized: "kanban.board.name"), text: $kanbanNewBoardName)
                    }

                    TextField(String(localized: "mobile.ios.boardImageUrl"), text: $kanbanBoardAvatarUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button {
                        if let board = activeKanbanBoard {
                            kanbanBoardMediaImportTarget = KanbanBoardMediaImportTarget(boardId: board.id, isWallpaper: false)
                            isKanbanBoardMediaImporterPresented = true
                        }
                    } label: {
                        Label(String(localized: "kanban.board.chooseImage"), systemImage: "photo")
                    }

                    IosWallpaperPreview(
                        presetId: kanbanBoardWallpaperPresetId,
                        customUrl: kanbanBoardWallpaperUrl
                    )
                    IosWallpaperPresetPicker(selected: Binding(
                        get: { kanbanBoardWallpaperPresetId },
                        set: { presetId in
                            kanbanBoardWallpaperPresetId = presetId
                            kanbanBoardWallpaperUrl = ""
                        }
                    ))
                    TextField(String(localized: "mobile.ios.wallpaperImageUrl"), text: $kanbanBoardWallpaperUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                    Button {
                        if let board = activeKanbanBoard {
                            kanbanBoardMediaImportTarget = KanbanBoardMediaImportTarget(boardId: board.id, isWallpaper: true)
                            isKanbanBoardMediaImporterPresented = true
                        }
                    } label: {
                        Label(String(localized: "settings.chooseWallpaperImage"), systemImage: "photo.on.rectangle")
                    }

                    Button {
                        renameActiveKanbanBoard()
                        updateActiveKanbanBoardAppearance()
                    } label: {
                        Label(String(localized: "buttons.save"), systemImage: "checkmark.circle")
                    }

                    Button(role: .destructive) {
                        deleteActiveKanbanBoard()
                    } label: {
                        Label(String(localized: "kanban.board.delete"), systemImage: "trash")
                    }
                }

                Button {
                    createKanbanBoard()
                } label: {
                    Label(String(localized: "settings.kanban.newBoard"), systemImage: "plus.rectangle.on.rectangle")
                }
            }

            Section(String(localized: "settings.sections.storage")) {
                Text(String(localized: "settings.storage.usageHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
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
                Text(String(localized: "settings.storage.clearCachedAttachmentsOnly"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Section(String(localized: "about.title")) {
                LabeledContent(String(localized: "mobile.accounts.version"), value: "\(appVersion) (\(appBuild))")
                LabeledContent(String(localized: "mobile.accounts.coreProtocol"), value: "\(rustProtocolVersion)")
                LabeledContent(String(localized: "mobile.accounts.sharedProtocol"), value: "\(protocolVersion)")
                Text(String(localized: "about.supportDevelopment"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                ForEach(iosAboutSupportLinks) { supportLink in
                    if let url = URL(string: supportLink.url) {
                        Link(destination: url) {
                            Label(
                                String(localized: String.LocalizationValue(supportLink.titleKey)),
                                systemImage: supportLink.systemImage
                            )
                        }
                    }
                }
            }

            accountSettingsSection(
                title: String(localized: "settings.sections.mailAccounts"),
                accounts: coreAccounts.filter { !MailStateKt.accountSummaryIsRss(account: $0) },
                emptyText: String(localized: "settings.sections.noMailAccounts")
            )

            accountSettingsSection(
                title: String(localized: "settings.sections.feedAccounts"),
                accounts: coreAccounts.filter { MailStateKt.accountSummaryIsRss(account: $0) },
                emptyText: String(localized: "settings.sections.noFeedAccounts")
            )

            Section(String(localized: "mobile.accounts.passwordAccount")) {
                Text(String(localized: "accounts.providers.customDescription"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextField(String(localized: "accounts.fields.emailAddress"), text: $accountEmail)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onSubmit {
                        autodiscoverPasswordAccount()
                    }
                TextField(String(localized: "settings.account.senderName"), text: $accountSenderName)
                SecureField(String(localized: "accounts.fields.password"), text: $accountPassword)
                Button {
                    autodiscoverPasswordAccount()
                } label: {
                    Label(String(localized: "mobile.accounts.findMailSettings"), systemImage: "magnifyingglass")
                }
                DisclosureGroup(isExpanded: $accountAdvancedServerSettingsOpen) {
                    TextField(String(localized: "settings.account.displayName"), text: $accountDisplayName)
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
                    TextField(String(localized: "accounts.fields.username"), text: $accountUsername)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                } label: {
                    Text(String(localized: "accounts.advancedServerSettings"))
                }
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

            Section(String(localized: "accounts.setup.oauthTab")) {
                Picker(String(localized: "mobile.accounts.provider"), selection: $oauthProvider) {
                    Text("Gmail").tag("gmail")
                    Text("Outlook").tag("outlook")
                }
                .pickerStyle(.segmented)
                Text(String(localized: String.LocalizationValue(oauthProvider == "outlook" ? "accounts.providers.outlookDescription" : "accounts.providers.gmailDescription")))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button {
                    launchOAuthFlow()
                } label: {
                    Label(String(localized: "mobile.accounts.openOAuthInBrowser"), systemImage: "safari")
                }
                if !oauthAuthorizationCode.isEmpty {
                    LabeledContent(String(localized: "mobile.accounts.authorizationCode"), value: oauthAuthorizationCode)
                    Button {
                        exchangeOAuthCode()
                    } label: {
                        Label(reconnectingAccountId.isEmpty ? String(localized: "mobile.accounts.exchangeCodeAndAddAccount") : String(localized: "mobile.accounts.exchangeCodeAndReconnect"), systemImage: "arrow.triangle.2.circlepath")
                    }
                }
            }

            Section(String(localized: "accounts.setup.rssTab")) {
                Text(String(localized: "accounts.setup.feedAccountHint"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
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

    func iosShortcutRow(keys: String, label: String) -> some View {
        HStack(spacing: 12) {
            Text(label)
            Spacer(minLength: 12)
            Text(keys)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .padding(.horizontal, 7)
                .padding(.vertical, 3)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
        }
    }

    @ViewBuilder
    func accountSettingsSection(title: String, accounts: [AccountSummary], emptyText: String) -> some View {
        Section(title) {
            if accounts.isEmpty {
                Text(emptyText)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(accounts, id: \.id) { account in
                    let index = coreAccounts.firstIndex(where: { $0.id == account.id }) ?? 0
                    DisclosureGroup(isExpanded: Binding(
                        get: { focusedAccountSettingsId == account.id },
                        set: { expanded in
                            focusedAccountSettingsId = expanded ? account.id : ""
                        }
                    )) {
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
                            onImportOpml: {
                                opmlImportAccountId = account.id
                                isOpmlImporterPresented = true
                            },
                            onExportOpml: {
                                exportSelectedAccountOpml(accountId: account.id)
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
                            let stateLabels = iosAccountStateLabels(
                                needsReconnect: account.needsReconnect,
                                paused: account.paused,
                                muted: account.muted,
                                hiddenFromNavigation: hiddenNavigationAccountIds.contains(account.id)
                            )
                            if !stateLabels.isEmpty {
                                HStack(spacing: 6) {
                                    ForEach(stateLabels, id: \.self) { label in
                                        Text(label)
                                            .font(.caption2.weight(.semibold))
                                            .foregroundStyle(.secondary)
                                            .padding(.horizontal, 6)
                                            .padding(.vertical, 2)
                                            .background(Color(.tertiarySystemGroupedBackground))
                                            .clipShape(Capsule())
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
