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
    @State var isFileImporterPresented = false
    @State var isQuickReplyFileImporterPresented = false
    @State var isOpmlImporterPresented = false
    @State var isOpmlExporterPresented = false
    @State var isAccountMediaImporterPresented = false
    @State var isAddFeedPresented = false
    @State var addFeedUrl = ""
    @State var accountMediaImportTarget: AccountMediaImportTarget?
    @State var opmlExportDocument = OpmlDocument()
    @State var attachmentError: String?
    @State var shareableAttachment: ShareableFile?
    @State var imagePreview: IosImagePreview?
    @State var backgroundRefreshStatus = "Scheduled on app launch."
    @State var notificationStatus = "Not requested."
    @AppStorage(iosAppearanceModeKey) var appearanceMode = "system"
    @AppStorage(iosShowUnifiedInboxKey) var showUnifiedInbox = true
    @AppStorage(iosShowStarredTabKey) var showStarredTab = true
    @AppStorage(iosShowUnreadBadgesKey) var showUnreadBadges = true
    @AppStorage(iosShowSenderImagesKey) var showSenderImages = false
    @AppStorage(iosSendShortcutKey) var sendShortcutMode = "mod_enter"
    @AppStorage(iosHiddenNavigationAccountsKey) var hiddenNavigationAccountsValue = ""
    @AppStorage(iosKanbanColumnWidthKey) var kanbanColumnWidth = iosKanbanColumnDefaultWidth
    @State var storageCacheBytes: Int64?
    @State var storageDbBytes: Int64?
    @State var storageBusy = false
    @State var storageClearConfirming = false
    @State var accountEmail = "user1@mail.localhost"
    @State var accountUsername = ""
    @State var accountPassword = "user1password"
    @State var accountDisplayName = "Local Test"
    @State var accountSenderName = "Local Test"
    @State var imapHost = "10.0.2.2"
    @State var imapPort = "993"
    @State var smtpHost = "10.0.2.2"
    @State var smtpPort = "465"
    @State var oauthProvider = "gmail"
    @State var oauthEmail = "me@gmail.com"
    @State var oauthAccessToken = ""
    @State var oauthRefreshToken = ""
    @State var oauthExpiresAt = "0"
    @State var oauthClientId = ""
    @State var oauthClientSecret = ""
    @State var oauthRedirectUri = OAuthFlowKt.defaultOAuthRedirectUri()
    @State var oauthState = UUID().uuidString
    @State var oauthVerifier = UUID().uuidString + UUID().uuidString
    @State var oauthAuthorizationCode = ""
    @State var reconnectingAccountId = ""
    @State var rssFeedUrl = "https://example.com/feed.xml"
    @State var rssDisplayName = "Example Feed"
    @State var accountStatus = "Not loaded."
    @State var accountJson = ""
    @State var coreAccounts: [AccountSummary] = []
    @State var selectedCoreAccountId = ""
    @State var coreFolders: [FolderSummary] = []
    @State var selectedCoreFolder = "inbox"
    @State var mailSearch = ""
    @State var mailFilter: IosFilterMode = .all
    @State var coreThreads: [ThreadSummary] = []
    @State var mailboxCursor = ""
    @State var mailboxAccountCursors: [String: String] = [:]
    @State var isLoadingMoreThreads = false
    @State var starredItems: [StarredItemSummary] = []
    @State var selectedCoreThread: ThreadSummary?
    @State var coreMessages: [MessageBody] = []
    @State var moveThreadTarget: ThreadSummary?
    @State var moveThreadFolders: [FolderSummary] = []
    @State var moveThreadDialogPresented = false
    @State var moveThreadCreateDialogPresented = false
    @State var moveThreadNewFolderName = ""
    @State var copyThreadTarget: ThreadSummary?
    @State var copyThreadFolders: [FolderSummary] = []
    @State var copyThreadDialogPresented = false
    @State var conversationHtmlOverrides: [String: Bool] = [:]
    @State var messageCursor = ""
    @State var isLoadingMoreMessages = false
    @State var threadSearch = ""
    @State var activeThreadSearchIndex = 0
    @State var kanbanBoards: [IosKanbanBoardSpec] = loadIosKanbanBoards()
    @State var activeKanbanBoardId = UserDefaults.standard.string(forKey: "ios_kanban_active_board_v1") ?? ""
    @State var kanbanThreadsByColumn: [String: [ThreadSummary]] = [:]
    @State var kanbanStatusByColumn: [String: String] = [:]
    @State var kanbanCursorByColumn: [String: String] = [:]
    @State var kanbanAccountCursorsByColumn: [String: [String: String]] = [:]
    @State var kanbanLoadingMoreColumns: Set<String> = []
    @State var kanbanSearch = ""
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
    @State var composeCc = ""
    @State var composeBcc = ""
    @State var composeSubject = "Hello from Meron iOS"
    @State var composeBody = "This message was sent from the native iOS shell through meron-core."
    @State var quickReplyBody = ""
    @State var recipientSuggestionField = ""
    @State var recipientSuggestions: [ContactSuggestion] = []
    @State var selectedTab: IosAppTab = .mail

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
            }
            .tabItem { Label("Mail", systemImage: "tray") }
            .badge(showUnreadBadges ? coreThreads.filter(\.unread).count : 0)
            .tag(IosAppTab.mail)

            NavigationStack {
                composeView
                    .navigationTitle("Compose")
                    .navigationBarTitleDisplayMode(.inline)
            }
            .tabItem { Label("Compose", systemImage: "square.and.pencil") }
            .tag(IosAppTab.compose)

            if showStarredTab {
                NavigationStack {
                    starredView
                        .navigationTitle("Starred")
                        .navigationBarTitleDisplayMode(.inline)
                }
                .tabItem { Label("Starred", systemImage: "star") }
                .badge(showUnreadBadges ? starredItems.filter(\.unread).count : 0)
                .tag(IosAppTab.starred)
            }

            NavigationStack {
                kanbanView
                    .navigationTitle("Kanban")
                    .navigationBarTitleDisplayMode(.inline)
            }
            .tabItem { Label("Kanban", systemImage: "rectangle.3.group") }
            .tag(IosAppTab.kanban)

            NavigationStack {
                accountsView
                    .navigationTitle("Accounts")
                    .navigationBarTitleDisplayMode(.inline)
            }
            .tabItem { Label("Accounts", systemImage: "person.crop.circle") }
            .tag(IosAppTab.accounts)
        }
        .onChange(of: showStarredTab) { _, visible in
            if !visible, selectedTab == .starred {
                selectedTab = .mail
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
        .onOpenURL { url in
            if OAuthFlowKt.isOAuthCallbackUrl(rawUrl: url.absoluteString, redirectUri: oauthRedirectUri) {
                handleOAuthCallback(url)
            } else {
                mailtoDraft = MailtoKt.parseMailtoUrl(rawUrl: url.absoluteString)
                if let draft = mailtoDraft {
                    composeDraftId = ""
                    composeDraftSaved = false
                    applyMailtoDraftToCompose(
                        draft,
                        to: &composeTo,
                        cc: &composeCc,
                        bcc: &composeBcc,
                        subject: &composeSubject,
                        body: &composeBody
                    )
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
        .fileImporter(isPresented: $isQuickReplyFileImporterPresented, allowedContentTypes: [.item]) { result in
            switch result {
            case let .success(url):
                addQuickReplyAttachment(from: url)
            case let .failure(error):
                attachmentError = error.localizedDescription
            }
        }
        .fileImporter(isPresented: $isOpmlImporterPresented, allowedContentTypes: [.xml, .item]) { result in
            switch result {
            case let .success(url):
                importOpml(from: url)
            case let .failure(error):
                accountStatus = "OPML import failed: \(error.localizedDescription)"
            }
        }
        .fileImporter(isPresented: $isAccountMediaImporterPresented, allowedContentTypes: [.image]) { result in
            switch result {
            case let .success(url):
                importAccountMedia(from: url)
            case let .failure(error):
                accountStatus = "Media import failed: \(error.localizedDescription)"
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
                accountStatus = "Exported OPML."
            case let .failure(error):
                accountStatus = "OPML export failed: \(error.localizedDescription)"
            }
        }
        .sheet(item: $shareableAttachment) { item in
            ShareSheet(url: item.url)
        }
        .sheet(item: $imagePreview) { preview in
            ImagePreviewSheet(preview: preview)
        }
        .confirmationDialog("Move conversation", isPresented: $moveThreadDialogPresented, presenting: moveThreadTarget) { thread in
            Button("New Folder...") {
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
            Button("Cancel", role: .cancel) {}
        } message: { thread in
            Text(thread.subject.isEmpty ? "(no subject)" : thread.subject)
        }
        .alert("New Folder", isPresented: $moveThreadCreateDialogPresented, presenting: moveThreadTarget) { thread in
            TextField("Folder name", text: $moveThreadNewFolderName)
            Button("Create & Move") {
                createFolderAndMoveThread(thread, name: moveThreadNewFolderName)
            }
            Button("Cancel", role: .cancel) {}
        } message: { thread in
            Text(thread.subject.isEmpty ? "(no subject)" : thread.subject)
        }
        .confirmationDialog("Copy conversation", isPresented: $copyThreadDialogPresented, presenting: copyThreadTarget) { thread in
            ForEach(copyThreadFolders, id: \.accountIdAndName) { folder in
                Button(copyTargetLabel(folder)) {
                    copyThread(thread, toFolder: folder)
                }
            }
            Button("Cancel", role: .cancel) {}
        } message: { thread in
            Text(thread.subject.isEmpty ? "(no subject)" : thread.subject)
        }
        .alert("Add Feed", isPresented: $isAddFeedPresented) {
            TextField("Feed URL", text: $addFeedUrl)
            Button("Add") {
                addFeedToSelectedRssAccount()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Add a feed to the selected RSS account.")
        }
        .onAppear {
            ensureKanbanDefaults()
            loadStorageUsage(showStatus: false)
        }
    }
}
