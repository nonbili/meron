import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

struct ContentView: View {
    let protocolVersion = CoreProtocolKt.EXPECTED_PROTOCOL_VERSION
    let rustInitJson: String
    let rustProtocolVersion = RustCoreBridge.protocolVersion()
    let rustPingJson = RustCoreBridge.pingJson()
    let rustReadyEvents = RustCoreBridge.readyEvents()
    let threadListJson: String
    let appVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0.1.0"
    let appBuild = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "1"
    @State var mailtoDraft: ComposeDraft?
    @State var attachments: [DraftAttachment] = []
    @State var quickReplyAttachments: [DraftAttachment] = []
    @State var quickReplyFailure = ""
    @State var quickReplySending = false
    @State var isFileImporterPresented = false
    @State var isInlineImageImporterPresented = false
    @State var isQuickReplyFileImporterPresented = false
    @State var isOpmlImporterPresented = false
    @State var isOpmlExporterPresented = false
    @State var opmlImportAccountId = ""
    @State var isAccountMediaImporterPresented = false
    @State var isKanbanBoardMediaImporterPresented = false
    @State var isAddFeedPresented = false
    @State var addFeedUrl = ""
    @State var accountMediaImportTarget: AccountMediaImportTarget?
    @State var kanbanBoardMediaImportTarget: KanbanBoardMediaImportTarget?
    @State var opmlExportDocument = OpmlDocument()
    @State var attachmentError: String?
    @State var composeInlineAttachmentIds: Set<String> = []
    @State var shareableAttachment: ShareableFile?
    @State var attachmentExportDocument = AttachmentDocument()
    @State var attachmentExportFilename = "attachment"
    @State var isAttachmentExporterPresented = false
    @State var composeNoSubjectConfirming = false
    @State var composeSending = false
    @State var composeReturnTab: IosAppTab = .mail
    @State var imagePreview: IosImagePreview?
    @State var messageReaderTarget: MessageBody?
    @State var backgroundRefreshStatus = String(localized: "mobile.ios.backgroundRefreshScheduledOnLaunch")
    @State var notificationStatus = String(localized: "mobile.ios.notificationsNotRequested")
    @State var notificationOpenObserver: NSObjectProtocol?
    @State var coreEventHandlerInstalled = false
    @State var coreSyncErrorAccountId = ""
    @State var coreSyncErrorMessage = ""
    @State var liveMailPushStatus = ""
    @AppStorage(iosAppearanceModeKey) var appearanceMode = "system"
    @AppStorage(iosShowUnifiedInboxKey) var showUnifiedInbox = true
    @AppStorage(iosShowStarredTabKey) var showStarredTab = true
    @AppStorage(iosShowUnreadBadgesKey) var showUnreadBadges = true
    @AppStorage(iosShowSenderImagesKey) var showSenderImages = false
    @AppStorage(iosSendShortcutKey) var sendShortcutMode = "mod_enter"
    @AppStorage(iosAppLanguageTagKey) var appLanguageTag = ""
    @AppStorage(iosHiddenNavigationAccountsKey) var hiddenNavigationAccountsValue = ""
    @AppStorage(iosKanbanColumnWidthKey) var kanbanColumnWidth = iosKanbanColumnDefaultWidth
    @AppStorage(iosLiveMailPushKey) var liveMailPushEnabled = false
    @State var storageCacheBytes: Int64?
    @State var storageDbBytes: Int64?
    @State var storageBusy = false
    @State var storageClearConfirming = false
    @State var accountEmail = ""
    @State var accountUsername = ""
    @State var accountPassword = ""
    @State var accountDisplayName = ""
    @State var accountSenderName = ""
    @State var imapHost = ""
    @State var imapPort = "993"
    @State var smtpHost = ""
    @State var smtpPort = "465"
    @State var accountAdvancedServerSettingsOpen = false
    @State var oauthProvider = "gmail"
    @State var oauthEmail = ""
    @State var oauthAccessToken = ""
    @State var oauthRefreshToken = ""
    @State var oauthExpiresAt = "0"
    @State var oauthClientId = iosDefaultGoogleOAuthClientId
    @State var oauthClientSecret = ""
    @State var oauthRedirectUri = OAuthFlowKt.defaultOAuthRedirectUri()
    @State var oauthState = UUID().uuidString
    @State var oauthVerifier = UUID().uuidString + UUID().uuidString
    @State var oauthAuthorizationCode = ""
    @State var reconnectingAccountId = ""
    @State var rssFeedUrl = ""
    @State var rssDisplayName = ""
    @State var accountStatus = String(localized: "mobile.ios.notLoaded")
    @State var accountJson = ""
    @State var coreAccounts: [AccountSummary] = []
    @State var accountInboxUnreadCounts: [String: Int] = [:]
    @State var focusedAccountSettingsId = ""
    @State var selectedCoreAccountId = ""
    @State var coreFolders: [FolderSummary] = []
    @State var selectedCoreFolder = "inbox"
    @State var mailSearch = ""
    @State var mailFilter: IosFilterMode = .all
    @State var coreThreads: [ThreadSummary] = []
    @State var selectedMailThreadIds: Set<String> = []
    @State var pendingThreadUndo: IosThreadUndo?
    @State var mailboxCursor = ""
    @State var mailboxAccountCursors: [String: String] = [:]
    @State var isLoadingMoreThreads = false
    @State var starredItems: [StarredItemSummary] = []
    @State var starredSearch = ""
    @State var starredMessageScrollTarget = ""
    @State var selectedCoreThread: ThreadSummary?
    @State var coreMessages: [MessageBody] = []
    @State var revealedRemoteMediaMessageIds: Set<String> = []
    @State var moveThreadTarget: ThreadSummary?
    @State var moveThreadFolders: [FolderSummary] = []
    @State var moveThreadDialogPresented = false
    @State var moveThreadCreateDialogPresented = false
    @State var moveThreadNewFolderName = ""
    @State var moveRssFeedTarget: ThreadSummary?
    @State var moveRssFeedAccounts: [AccountSummary] = []
    @State var moveRssFeedDialogPresented = false
    @State var copyThreadTarget: ThreadSummary?
    @State var copyThreadFolders: [FolderSummary] = []
    @State var copyThreadDialogPresented = false
    @State var deleteThreadConfirmTarget: ThreadSummary?
    @State var deleteThreadConfirmPresented = false
    @State var deleteMessageConfirmTarget: MessageBody?
    @State var deleteMessageConfirmPresented = false
    @State var deleteStarredItemConfirmTarget: StarredItemSummary?
    @State var deleteStarredItemConfirmPresented = false
    @State var deleteKanbanConfirmTarget: IosKanbanDeleteTarget?
    @State var deleteKanbanConfirmPresented = false
    @State var deleteRssFeedConfirmTarget: ThreadSummary?
    @State var deleteRssFeedConfirmPresented = false
    @State var conversationHtmlOverrides: [String: Bool] = [:]
    @State var messageCursor = ""
    @State var isLoadingMoreMessages = false
    @State var threadSearch = ""
    @State var threadSearchPresented = false
    @State var conversationDetailsPresented = false
    @State var activeThreadSearchIndex = 0
    @State var kanbanBoards: [IosKanbanBoardSpec] = loadIosKanbanBoards()
    @State var activeKanbanBoardId = UserDefaults.standard.string(forKey: "ios_kanban_active_board_v1") ?? ""
    @State var kanbanThreadsByColumn: [String: [ThreadSummary]] = [:]
    @State var kanbanStarredThreadIdsByItemId: [String: String] = [:]
    @State var kanbanStatusByColumn: [String: String] = [:]
    @State var kanbanCursorByColumn: [String: String] = [:]
    @State var kanbanAccountCursorsByColumn: [String: [String: String]] = [:]
    @State var kanbanLoadingMoreColumns: Set<String> = []
    @State var kanbanMinimizedColumnIds: Set<String> = []
    @State var kanbanSearch = ""
    @State var kanbanSearchScope = "all"
    @State var kanbanFilter: IosFilterMode = .all
    @State var kanbanNewBoardName = ""
    @State var kanbanBoardAvatarUrl = ""
    @State var kanbanBoardWallpaperPresetId = ""
    @State var kanbanBoardWallpaperUrl = ""
    @State var kanbanCreateFolderName = ""
    @State var kanbanSelectedAccountId = ""
    @State var kanbanSelectedFolderId = "inbox"
    @State var composeTo = "user1@mail.localhost"
    @State var composeFromAccountId = ""
    @State var composeFromEmail = ""
    @State var composeDraftId = ""
    @State var composeDraftSaved = false
    @State var composeInReplyTo = ""
    @State var composeReferences = ""
    @State var composeReplyTo = ""
    @State var composeCc = ""
    @State var composeBcc = ""
    @State var composeCcBccVisible = false
    @State var composeDiscardConfirming = false
    @State var composeSubject = "Hello from Meron iOS"
    @State var composeBody = "This message was sent from the native iOS shell through meron-core."
    @State var composeRichText = false
    @State var composeAutosaveTask: Task<Void, Never>?
    @State var composeLastAutosaveSignature = ""
    @State var quickReplyBody = ""
    @State var recipientSuggestionField = ""
    @State var recipientSuggestions: [ContactSuggestion] = []
    @State var isCommandPalettePresented = false
    @State var commandPaletteQuery = ""
    @State var selectedTab: IosAppTab = .mail
    @FocusState var mailFocusedField: IosMailFocusedField?

    init() {
        rustInitJson = RustCoreBridge.initJson(dataDirectory: IosAppPaths.mobileDataDirectory(), dbKey: IosDbKey.get())
        let params = ThreadListParams(
            accountId: "mobile-demo",
            folderId: "inbox",
            query: "",
            filter: "all",
            beforeCursor: nil,
            refresh: false
        )
        threadListJson = MobileCommandsKt.threadListRequest(id: 1, params: params).toJson()
    }

    var body: some View {
        TabView(selection: $selectedTab) {
            NavigationStack {
                mailView
                    .navigationTitle(selectedCoreFolder.isEmpty ? "Inbox" : selectedCoreFolder.capitalized)
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { iosCommandPaletteToolbar }
            }
            .tabItem { Label(String(localized: "mobile.tabs.mail"), systemImage: "tray") }
            .badge(showUnreadBadges ? coreThreads.filter(\.unread).count : 0)
            .tag(IosAppTab.mail)

            NavigationStack {
                composeView
                    .navigationTitle(String(localized: "mobile.tabs.compose"))
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { iosCommandPaletteToolbar }
            }
            .tabItem { Label(String(localized: "mobile.tabs.compose"), systemImage: "square.and.pencil") }
            .tag(IosAppTab.compose)

            if showStarredTab {
                NavigationStack {
                    starredView
                        .navigationTitle(String(localized: "mobile.tabs.starred"))
                        .navigationBarTitleDisplayMode(.inline)
                        .toolbar { iosCommandPaletteToolbar }
                }
                .tabItem { Label(String(localized: "mobile.tabs.starred"), systemImage: "star") }
                .badge(showUnreadBadges ? starredItems.filter(\.unread).count : 0)
                .tag(IosAppTab.starred)
            }

            NavigationStack {
                kanbanView
                    .navigationTitle(String(localized: "mobile.tabs.kanban"))
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { iosCommandPaletteToolbar }
            }
            .tabItem { Label(String(localized: "mobile.tabs.kanban"), systemImage: "rectangle.3.group") }
            .tag(IosAppTab.kanban)

            NavigationStack {
                accountsView
                    .navigationTitle(String(localized: "mobile.tabs.accounts"))
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar { iosCommandPaletteToolbar }
            }
            .tabItem { Label(String(localized: "mobile.tabs.accounts"), systemImage: "person.crop.circle") }
            .tag(IosAppTab.accounts)
        }
        .focusable()
        .onKeyPress(characters: CharacterSet(charactersIn: "123456789nerv,wfk?"), phases: [.down]) { press in
            handleGlobalShortcut(press)
        }
        .onChange(of: showStarredTab) { _, visible in
            if !visible, selectedTab == .starred {
                selectedTab = .mail
            }
        }
        .sheet(isPresented: $isCommandPalettePresented) {
            IosCommandPaletteSheet(
                commands: iosCommandPaletteCommands,
                query: $commandPaletteQuery
            ) { command in
                isCommandPalettePresented = false
                command.action()
            }
        }
        .sheet(
            isPresented: Binding(
                get: { messageReaderTarget != nil },
                set: { visible in
                    if !visible {
                        messageReaderTarget = nil
                    }
                }
            )
        ) {
            if let messageReaderTarget {
                IosMessageReaderSheet(
                    message: messageReaderTarget,
                    preferHtml: shouldRenderHtml(messageReaderTarget),
                    allowRemoteImages: selectedConversationAllowsRemoteImages || revealedRemoteMediaMessageIds.contains(messageReaderTarget.id),
                    onOpenAttachment: openMessageAttachment,
                    onSaveAttachment: saveMessageAttachment,
                    onCopy: { label, value in
                        UIPasteboard.general.string = value
                        accountStatus = String(format: String(localized: "mobile.ios.copiedValue"), label.lowercased())
                    }
                )
            }
        }
        .onChange(of: showUnifiedInbox) { _, visible in
            if !visible, selectedCoreAccountId == iosUnifiedAccountId {
                selectedCoreAccountId = visibleNavigationAccounts.first?.id ?? ""
                selectedCoreFolder = iosInboxFolderId
                coreFolders = []
                coreThreads = []
                selectedCoreThread = nil
                coreMessages = []
                mailboxCursor = ""
                mailboxAccountCursors = [:]
            }
        }
        .onChange(of: selectedCoreThread?.id ?? "") { previousThreadId, nextThreadId in
            persistQuickReplyDraft(threadId: previousThreadId, body: quickReplyBody)
            quickReplyBody = readQuickReplyDraft(threadId: nextThreadId)
            quickReplyFailure = ""
        }
        .onOpenURL { url in
            let rawUrl = url.absoluteString
            let pendingRedirectUri = loadIosPendingOAuthFlow()?.redirectUri
            if OAuthFlowKt.isOAuthCallbackUrl(rawUrl: rawUrl, redirectUri: pendingRedirectUri ?? oauthRedirectUri) ||
                OAuthFlowKt.isPotentialOAuthCallbackUrl(rawUrl: rawUrl)
            {
                handleOAuthCallback(url)
            } else {
                mailtoDraft = MailtoKt.parseMailtoUrl(rawUrl: rawUrl)
                if let draft = mailtoDraft {
                    composeDraftId = ""
                    composeDraftSaved = false
                    composeReplyTo = ""
                    composeRichText = false
                    applyMailtoDraftToCompose(
                        draft,
                        to: &composeTo,
                        cc: &composeCc,
                        bcc: &composeBcc,
                        subject: &composeSubject,
                        body: &composeBody
                    )
                    composeCcBccVisible = !composeCc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
                        !composeBcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                    composeReturnTab = .mail
                    selectedTab = .compose
                }
            }
        }
        .fileImporter(isPresented: $isFileImporterPresented, allowedContentTypes: [.item]) { result in
            switch result {
            case let .success(url):
                addAttachment(from: url)
            case let .failure(error):
                attachmentError = error.localizedDescription
            }
        }
        .fileImporter(isPresented: $isInlineImageImporterPresented, allowedContentTypes: [.image]) { result in
            switch result {
            case let .success(url):
                addInlineImageAttachment(from: url)
            case let .failure(error):
                attachmentError = error.localizedDescription
            }
        }
        .fileImporter(isPresented: $isQuickReplyFileImporterPresented, allowedContentTypes: [.item]) { result in
            switch result {
            case let .success(url):
                addQuickReplyAttachment(from: url)
            case let .failure(error):
                attachmentError = error.localizedDescription
            }
        }
        .fileImporter(isPresented: $isOpmlImporterPresented, allowedContentTypes: [.xml, .item]) { result in
            let accountId = opmlImportAccountId
            opmlImportAccountId = ""
            switch result {
            case let .success(url):
                importOpml(from: url, accountId: accountId)
            case let .failure(error):
                accountStatus = "\(String(localized: "mobile.ios.opmlImportFailed")): \(error.localizedDescription)"
            }
        }
        .fileImporter(isPresented: $isAccountMediaImporterPresented, allowedContentTypes: [.image]) { result in
            switch result {
            case let .success(url):
                importAccountMedia(from: url)
            case let .failure(error):
                accountStatus = "\(String(localized: "mobile.ios.mediaImportFailed")): \(error.localizedDescription)"
            }
        }
        .fileImporter(isPresented: $isKanbanBoardMediaImporterPresented, allowedContentTypes: [.image]) { result in
            switch result {
            case let .success(url):
                importKanbanBoardMedia(from: url)
            case let .failure(error):
                accountStatus = "\(String(localized: "mobile.ios.boardMediaImportFailed")): \(error.localizedDescription)"
            }
        }
        .fileExporter(
            isPresented: $isOpmlExporterPresented,
            document: opmlExportDocument,
            contentType: .xml,
            defaultFilename: "meron-feeds.opml"
        ) { result in
            switch result {
            case .success:
                accountStatus = String(localized: "mobile.ios.exportedOpml")
            case let .failure(error):
                accountStatus = "\(String(localized: "mobile.ios.opmlExportFailed")): \(error.localizedDescription)"
            }
        }
        .sheet(item: $shareableAttachment) { item in
            ShareSheet(url: item.url)
        }
        .fileExporter(
            isPresented: $isAttachmentExporterPresented,
            document: attachmentExportDocument,
            contentType: .data,
            defaultFilename: attachmentExportFilename
        ) { result in
            switch result {
            case .success:
                accountStatus = String(localized: "buttons.save")
            case let .failure(error):
                accountStatus = "\(String(localized: "mobile.ios.attachmentOpenFailed")): \(error.localizedDescription)"
            }
        }
        .sheet(item: $imagePreview) { preview in
            ImagePreviewSheet(preview: preview) {
                saveImagePreview(preview)
            }
        }
        .confirmationDialog(String(localized: "threads.moveConversation"), isPresented: $moveThreadDialogPresented, presenting: moveThreadTarget) { thread in
            Button(String(localized: "mobile.ios.newFolderEllipsis")) {
                moveThreadNewFolderName = ""
                DispatchQueue.main.async {
                    moveThreadCreateDialogPresented = true
                }
            }
            ForEach(moveThreadFolders, id: \.name) { folder in
                Button(folder.name) {
                    moveThread(thread, toFolder: folder.name)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        } message: { thread in
            Text(thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject)
        }
        .alert(String(localized: "mobile.ios.newFolder"), isPresented: $moveThreadCreateDialogPresented, presenting: moveThreadTarget) { thread in
            TextField(String(localized: "folders.namePlaceholder"), text: $moveThreadNewFolderName)
            Button(String(localized: "mobile.ios.createAndMove")) {
                createFolderAndMoveThread(thread, name: moveThreadNewFolderName)
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        } message: { thread in
            Text(thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject)
        }
        .confirmationDialog(String(localized: "threads.actions.moveTo"), isPresented: $moveRssFeedDialogPresented) {
            ForEach(moveRssFeedAccounts, id: \.id) { account in
                Button(accountLabel(account)) {
                    if let thread = moveRssFeedTarget {
                        moveRssFeed(thread, toAccount: account)
                    }
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        }
        .confirmationDialog(String(localized: "threads.copyConversation"), isPresented: $copyThreadDialogPresented, presenting: copyThreadTarget) { thread in
            ForEach(copyThreadFolders, id: \.accountIdAndName) { folder in
                Button(copyTargetLabel(folder)) {
                    copyThread(thread, toFolder: folder)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        } message: { thread in
            Text(thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject)
        }
        .alert(
            threadDeleteConfirmationTitle(deleteThreadConfirmTarget),
            isPresented: $deleteThreadConfirmPresented
        ) {
            Button(deleteThreadConfirmTarget.map(threadDeleteActionLabel) ?? String(localized: "buttons.delete"), role: .destructive) {
                if let deleteThreadConfirmTarget {
                    confirmDeleteThread(deleteThreadConfirmTarget)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {
                deleteThreadConfirmTarget = nil
            }
        } message: {
            if let deleteThreadConfirmTarget {
                Text(threadDeleteConfirmationMessage(deleteThreadConfirmTarget))
            }
        }
        .alert(
            messageDeleteConfirmationTitle(folder: selectedCoreThread?.folder ?? ""),
            isPresented: $deleteMessageConfirmPresented
        ) {
            Button(messageDeleteActionLabel(folder: selectedCoreThread?.folder ?? ""), role: .destructive) {
                if let deleteMessageConfirmTarget {
                    confirmDeleteMessage(deleteMessageConfirmTarget)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {
                deleteMessageConfirmTarget = nil
            }
        } message: {
            if let deleteMessageConfirmTarget {
                Text(messageDeleteConfirmationMessage(deleteMessageConfirmTarget, folder: selectedCoreThread?.folder ?? ""))
            }
        }
        .alert(
            messageDeleteConfirmationTitle(folder: deleteStarredItemConfirmTarget?.folder ?? ""),
            isPresented: $deleteStarredItemConfirmPresented
        ) {
            Button(messageDeleteActionLabel(folder: deleteStarredItemConfirmTarget?.folder ?? ""), role: .destructive) {
                if let deleteStarredItemConfirmTarget {
                    confirmDeleteStarredMailItem(deleteStarredItemConfirmTarget)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {
                deleteStarredItemConfirmTarget = nil
            }
        } message: {
            if let deleteStarredItemConfirmTarget {
                Text(starredItemDeleteConfirmationMessage(deleteStarredItemConfirmTarget))
            }
        }
        .alert(
            kanbanDeleteConfirmationTitle(deleteKanbanConfirmTarget),
            isPresented: $deleteKanbanConfirmPresented
        ) {
            Button(kanbanDeleteActionLabel(deleteKanbanConfirmTarget), role: .destructive) {
                if let deleteKanbanConfirmTarget {
                    confirmDeleteKanbanThread(deleteKanbanConfirmTarget)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {
                deleteKanbanConfirmTarget = nil
            }
        } message: {
            if let deleteKanbanConfirmTarget {
                Text(kanbanDeleteConfirmationMessage(deleteKanbanConfirmTarget))
            }
        }
        .alert(
            String(localized: "feeds.actions.deleteFeed"),
            isPresented: $deleteRssFeedConfirmPresented
        ) {
            Button(String(localized: "feeds.actions.deleteFeed"), role: .destructive) {
                if let deleteRssFeedConfirmTarget {
                    confirmRemoveRssFeed(deleteRssFeedConfirmTarget)
                }
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {
                deleteRssFeedConfirmTarget = nil
            }
        } message: {
            if let deleteRssFeedConfirmTarget {
                Text(rssFeedDeleteConfirmationMessage(deleteRssFeedConfirmTarget))
            }
        }
        .alert(String(localized: "feeds.actions.addFeed"), isPresented: $isAddFeedPresented) {
            TextField(String(localized: "feeds.url"), text: $addFeedUrl)
            Button(String(localized: "feeds.actions.addFeed")) {
                addFeedToSelectedRssAccount()
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        } message: {
            Text(String(localized: "empty.addFeedToStart"))
        }
        .alert("No subject", isPresented: $composeNoSubjectConfirming) {
            Button(String(localized: "buttons.send")) {
                confirmSendCoreMailWithoutSubject()
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        } message: {
            Text("Send this message without a subject?")
        }
        .onAppear {
            installNotificationOpenObserver()
            installCoreEventHandler()
            ensureKanbanDefaults()
            loadStorageUsage(showStatus: false)
            refreshNotificationStatus()
            syncIosLiveMailPush(showStatus: false)
        }
        .onChange(of: liveMailPushEnabled) { _, _ in
            syncIosLiveMailPush(showStatus: true)
        }
    }
}
