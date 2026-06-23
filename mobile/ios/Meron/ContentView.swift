import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

private let iosUnifiedAccountId = "unified"
private let iosInboxFolderId = "inbox"
let iosAppearanceModeKey = "ios_appearance_mode_v1"
private let iosShowUnifiedInboxKey = "ios_show_unified_inbox_v1"
private let iosShowStarredTabKey = "ios_show_starred_tab_v1"
private let iosShowUnreadBadgesKey = "ios_show_unread_badges_v1"
private let iosShowSenderImagesKey = "ios_show_sender_images_v1"
private let iosSendShortcutKey = "ios_send_shortcut_v1"
private let iosHiddenNavigationAccountsKey = "ios_hidden_navigation_accounts_v1"
private let iosKanbanColumnWidthKey = "ios_kanban_column_width_v1"
private let iosKanbanColumnMinWidth = 240.0
private let iosKanbanColumnDefaultWidth = 310.0
private let iosKanbanColumnMaxWidth = 520.0

struct IosThemeOption: Identifiable {
    let id: String
    let name: String
    let dark: Bool?
    let accent: Color
}

func iosColor(_ rgb: UInt32) -> Color {
    Color(
        red: Double((rgb >> 16) & 0xff) / 255.0,
        green: Double((rgb >> 8) & 0xff) / 255.0,
        blue: Double(rgb & 0xff) / 255.0
    )
}

let iosThemeOptions: [IosThemeOption] = [
    IosThemeOption(id: "system", name: "System", dark: nil, accent: iosColor(0x4F46E5)),
    IosThemeOption(id: "indigo", name: "Indigo", dark: false, accent: iosColor(0x4F46E5)),
    IosThemeOption(id: "indigo-dark", name: "Indigo Dark", dark: true, accent: iosColor(0x6366F1)),
    IosThemeOption(id: "light", name: "Meron Light", dark: false, accent: iosColor(0x2563EB)),
    IosThemeOption(id: "dark", name: "Meron Dark", dark: true, accent: iosColor(0x60A5FA)),
    IosThemeOption(id: "mist", name: "Mist", dark: false, accent: iosColor(0x0EA5B7)),
    IosThemeOption(id: "paper", name: "Paper", dark: false, accent: iosColor(0x64748B)),
    IosThemeOption(id: "dawn", name: "Dawn", dark: false, accent: iosColor(0xC06C84)),
    IosThemeOption(id: "honey", name: "Honey", dark: false, accent: iosColor(0xB07C10)),
    IosThemeOption(id: "lilac", name: "Lilac", dark: false, accent: iosColor(0x7A5BC4)),
    IosThemeOption(id: "graphite", name: "Graphite", dark: true, accent: iosColor(0x8B9BB4)),
    IosThemeOption(id: "midnight", name: "Midnight", dark: true, accent: iosColor(0x38BDF8)),
    IosThemeOption(id: "forest", name: "Forest", dark: true, accent: iosColor(0x7CCF9B)),
    IosThemeOption(id: "plum", name: "Plum", dark: true, accent: iosColor(0xB48AE0)),
    IosThemeOption(id: "ember", name: "Ember", dark: true, accent: iosColor(0xE1854C)),
]

func iosThemeOption(_ mode: String) -> IosThemeOption {
    iosThemeOptions.first { $0.id == mode } ?? iosThemeOptions[0]
}

private struct IosKanbanColumnSpec: Codable, Identifiable, Hashable {
    let id: String
    var accountId: String
    var folderId: String

    init(accountId: String, folderId: String) {
        self.accountId = accountId
        self.folderId = folderId
        self.id = "\(accountId)::\(folderId)"
    }
}

private struct IosKanbanBoardSpec: Codable, Identifiable, Hashable {
    var id: String
    var name: String
    var columns: [IosKanbanColumnSpec]
    var avatarUrl: String?
    var wallpaperPresetId: String?
    var wallpaperUrl: String?

    init(
        id: String,
        name: String,
        columns: [IosKanbanColumnSpec],
        avatarUrl: String? = nil,
        wallpaperPresetId: String? = nil,
        wallpaperUrl: String? = nil
    ) {
        self.id = id
        self.name = name
        self.columns = columns
        self.avatarUrl = avatarUrl
        self.wallpaperPresetId = wallpaperPresetId
        self.wallpaperUrl = wallpaperUrl
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case name
        case columns
        case avatarUrl
        case wallpaper
    }

    private enum WallpaperKeys: String, CodingKey {
        case kind
        case presetId
        case url
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        name = (try? container.decode(String.self, forKey: .name)) ?? "Kanban board"
        columns = (try? container.decode([IosKanbanColumnSpec].self, forKey: .columns)) ?? []
        avatarUrl = try? container.decode(String.self, forKey: .avatarUrl)
        if let wallpaper = try? container.nestedContainer(keyedBy: WallpaperKeys.self, forKey: .wallpaper) {
            wallpaperPresetId = try? wallpaper.decode(String.self, forKey: .presetId)
            wallpaperUrl = try? wallpaper.decode(String.self, forKey: .url)
        } else {
            wallpaperPresetId = nil
            wallpaperUrl = nil
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(id, forKey: .id)
        try container.encode(name, forKey: .name)
        try container.encode(columns, forKey: .columns)
        if let avatarUrl, !avatarUrl.isEmpty {
            try container.encode(avatarUrl, forKey: .avatarUrl)
        }
        if let wallpaperUrl, !wallpaperUrl.isEmpty {
            var wallpaper = container.nestedContainer(keyedBy: WallpaperKeys.self, forKey: .wallpaper)
            try wallpaper.encode("custom", forKey: .kind)
            try wallpaper.encode(wallpaperUrl, forKey: .url)
        } else if let wallpaperPresetId, !wallpaperPresetId.isEmpty {
            var wallpaper = container.nestedContainer(keyedBy: WallpaperKeys.self, forKey: .wallpaper)
            try wallpaper.encode("preset", forKey: .kind)
            try wallpaper.encode(wallpaperPresetId, forKey: .presetId)
        }
    }
}

private enum IosFilterMode: String, CaseIterable, Identifiable {
    case all
    case unread
    case starred

    var id: String { rawValue }

    var label: String {
        switch self {
        case .all: return "All"
        case .unread: return "Unread"
        case .starred: return "Starred"
        }
    }
}

private enum IosAppTab {
    case mail
    case compose
    case starred
    case kanban
    case accounts
}

private extension String {
    var nilIfBlank: String? {
        isEmpty ? nil : self
    }

    var imageURL: URL? {
        if hasPrefix("/") {
            return URL(fileURLWithPath: self)
        }
        return URL(string: self)
    }
}

private extension ThreadSummary {
    func withUnread(_ unread: Bool) -> ThreadSummary {
        ThreadSummary(
            id: id,
            accountId: accountId,
            folder: folder,
            subject: subject,
            sender: sender,
            preview: preview,
            unread: unread,
            starred: starred,
            dateEpochSeconds: dateEpochSeconds,
            feedUrl: feedUrl
        )
    }
}

private func identityKey(_ identity: SendIdentity) -> String {
    "\(identity.accountId)|\(identity.email)"
}

private struct OpmlDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.xml] }

    var text: String

    init(text: String = "") {
        self.text = text
    }

    init(configuration: ReadConfiguration) throws {
        if let data = configuration.file.regularFileContents {
            text = String(decoding: data, as: UTF8.self)
        } else {
            text = ""
        }
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

private extension FolderSummary {
    var accountIdAndName: String {
        "\(accountId)::\(name)"
    }
}

private extension MessageBody {
    func withFlags(unread nextUnread: Bool? = nil, starred nextStarred: Bool? = nil) -> MessageBody {
        MessageBody(
            id: id,
            from: from,
            to: to,
            cc: cc,
            bcc: bcc,
            subject: subject,
            body: body,
            bodyHtml: bodyHtml,
            dateEpochSeconds: dateEpochSeconds,
            fromAddr: fromAddr,
            replyTo: replyTo,
            messageId: messageId,
            references: references,
            unread: nextUnread ?? unread,
            starred: nextStarred ?? starred,
            hasAttachments: hasAttachments,
            attachments: attachments
        )
    }
}

private struct AccountSettingsDraft {
    var displayName: String
    var senderName: String
    var avatarUrl: String
    var wallpaperPresetId: String
    var loadRemoteImages: Bool
    var conversationHtml: Bool
    var includedInUnified: Bool
    var showInNavigation: Bool
    var muted: Bool
    var paused: Bool
    var rssSyncIntervalMinutes: Int
    var aliasesText: String
}

private struct AccountMediaImportTarget {
    let accountId: String
    let isWallpaper: Bool
}

private struct ShareableFile: Identifiable {
    let id = UUID()
    let url: URL
}

private struct IosImagePreview: Identifiable {
    let id = UUID()
    let title: String
    let image: UIImage
    let url: URL
}

private struct ConversationParticipant: Identifiable, Hashable {
    var id: String { email }
    let name: String
    let email: String
    let count: Int
    let isSelf: Bool
}

private struct ShareSheet: UIViewControllerRepresentable {
    let url: URL

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [url], applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

private struct ImagePreviewSheet: View {
    let preview: IosImagePreview
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ZStack {
                Color.black.ignoresSafeArea()
                Image(uiImage: preview.image)
                    .resizable()
                    .scaledToFit()
                    .padding()
            }
            .navigationTitle(preview.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    ShareLink(item: preview.url) {
                        Label("Share", systemImage: "square.and.arrow.up")
                    }
                    Spacer()
                    Button {
                        UIPasteboard.general.image = preview.image
                    } label: {
                        Label("Copy", systemImage: "doc.on.doc")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

private struct AccountSettingsEditor: View {
    let account: AccountSummary
    let isRss: Bool
    let onSave: (AccountSettingsDraft) -> Void
    let onPickAvatar: () -> Void
    let onPickWallpaper: () -> Void
    let onRemove: () -> Void
    let showInNavigation: Bool

    @State private var displayName: String
    @State private var senderName: String
    @State private var avatarUrl: String
    @State private var wallpaperPresetId: String
    @State private var loadRemoteImages: Bool
    @State private var conversationHtml: Bool
    @State private var includedInUnified: Bool
    @State private var visibleInNavigation: Bool
    @State private var muted: Bool
    @State private var paused: Bool
    @State private var intervalText: String
    @State private var aliasesText: String

    init(
        account: AccountSummary,
        isRss: Bool,
        onSave: @escaping (AccountSettingsDraft) -> Void,
        onPickAvatar: @escaping () -> Void,
        onPickWallpaper: @escaping () -> Void,
        onRemove: @escaping () -> Void,
        showInNavigation: Bool
    ) {
        self.account = account
        self.isRss = isRss
        self.onSave = onSave
        self.onPickAvatar = onPickAvatar
        self.onPickWallpaper = onPickWallpaper
        self.onRemove = onRemove
        self.showInNavigation = showInNavigation
        _displayName = State(initialValue: account.displayName)
        _senderName = State(initialValue: account.senderName)
        _avatarUrl = State(initialValue: account.avatarUrl)
        _wallpaperPresetId = State(initialValue: account.chatWallpaperPresetId)
        _loadRemoteImages = State(initialValue: account.loadRemoteImages || isRss)
        _conversationHtml = State(initialValue: account.conversationHtml)
        _includedInUnified = State(initialValue: account.includedInUnified)
        _visibleInNavigation = State(initialValue: showInNavigation)
        _muted = State(initialValue: account.muted)
        _paused = State(initialValue: account.paused)
        _intervalText = State(initialValue: "\(account.rssSyncIntervalMinutes)")
        _aliasesText = State(initialValue: account.aliases.map { alias in
            alias.name.isEmpty ? alias.email : "\(alias.email), \(alias.name)"
        }.joined(separator: "\n"))
    }

    var body: some View {
        Group {
            TextField(isRss ? "Feed group name" : "Account name", text: $displayName)
            if !isRss {
                TextField("Sender name", text: $senderName)
            }
            TextField("Avatar URL", text: $avatarUrl)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button {
                onPickAvatar()
            } label: {
                Label("Choose Avatar Image", systemImage: "photo")
            }
            TextField("Chat wallpaper preset", text: $wallpaperPresetId)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button {
                onPickWallpaper()
            } label: {
                Label("Choose Wallpaper Image", systemImage: "photo.on.rectangle")
            }
            Toggle("Show in unified inbox", isOn: $includedInUnified)
            Toggle("Show in navigation", isOn: $visibleInNavigation)
            Toggle("Mute notifications", isOn: $muted)
            Toggle("Pause automatic sync", isOn: $paused)
            Toggle("Load remote images", isOn: $loadRemoteImages)
            Toggle("Render HTML messages", isOn: $conversationHtml)
            if isRss {
                TextField("RSS sync interval minutes", text: $intervalText)
                    .keyboardType(.numberPad)
            } else {
                TextField("Aliases, one per line: email, optional name", text: $aliasesText, axis: .vertical)
                    .lineLimit(2...5)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }
            Button {
                let minutes = min(1440, max(5, Int(intervalText) ?? Int(account.rssSyncIntervalMinutes)))
                onSave(
                    AccountSettingsDraft(
                        displayName: displayName,
                        senderName: senderName,
                        avatarUrl: avatarUrl,
                        wallpaperPresetId: wallpaperPresetId,
                        loadRemoteImages: loadRemoteImages,
                        conversationHtml: conversationHtml,
                        includedInUnified: includedInUnified,
                        showInNavigation: visibleInNavigation,
                        muted: muted,
                        paused: paused,
                        rssSyncIntervalMinutes: minutes,
                        aliasesText: aliasesText
                    )
                )
            } label: {
                Label("Save Settings", systemImage: "checkmark.circle")
            }
            Button(role: .destructive) {
                onRemove()
            } label: {
                Label("Remove Account", systemImage: "trash")
            }
        }
    }
}

struct ContentView: View {
    private let protocolVersion = CoreProtocolKt.EXPECTED_PROTOCOL_VERSION
    private let rustInitJson: String
    private let rustProtocolVersion = RustCoreBridge.protocolVersion()
    private let rustPingJson = RustCoreBridge.pingJson()
    private let rustReadyEvents = RustCoreBridge.readyEvents()
    private let threadListJson: String
    private let appVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0.1.0"
    private let appBuild = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "1"
    @State private var mailtoDraft: ComposeDraft?
    @State private var attachments: [DraftAttachment] = []
    @State private var quickReplyAttachments: [DraftAttachment] = []
    @State private var quickReplyFailure = ""
    @State private var isFileImporterPresented = false
    @State private var isQuickReplyFileImporterPresented = false
    @State private var isOpmlImporterPresented = false
    @State private var isOpmlExporterPresented = false
    @State private var isAccountMediaImporterPresented = false
    @State private var isAddFeedPresented = false
    @State private var addFeedUrl = ""
    @State private var accountMediaImportTarget: AccountMediaImportTarget?
    @State private var opmlExportDocument = OpmlDocument()
    @State private var attachmentError: String?
    @State private var shareableAttachment: ShareableFile?
    @State private var imagePreview: IosImagePreview?
    @State private var backgroundRefreshStatus = "Scheduled on app launch."
    @State private var notificationStatus = "Not requested."
    @AppStorage(iosAppearanceModeKey) private var appearanceMode = "system"
    @AppStorage(iosShowUnifiedInboxKey) private var showUnifiedInbox = true
    @AppStorage(iosShowStarredTabKey) private var showStarredTab = true
    @AppStorage(iosShowUnreadBadgesKey) private var showUnreadBadges = true
    @AppStorage(iosShowSenderImagesKey) private var showSenderImages = false
    @AppStorage(iosSendShortcutKey) private var sendShortcutMode = "mod_enter"
    @AppStorage(iosHiddenNavigationAccountsKey) private var hiddenNavigationAccountsValue = ""
    @AppStorage(iosKanbanColumnWidthKey) private var kanbanColumnWidth = iosKanbanColumnDefaultWidth
    @State private var storageCacheBytes: Int64?
    @State private var storageDbBytes: Int64?
    @State private var storageBusy = false
    @State private var storageClearConfirming = false
    @State private var accountEmail = "user1@mail.localhost"
    @State private var accountUsername = ""
    @State private var accountPassword = "user1password"
    @State private var accountDisplayName = "Local Test"
    @State private var accountSenderName = "Local Test"
    @State private var imapHost = "10.0.2.2"
    @State private var imapPort = "993"
    @State private var smtpHost = "10.0.2.2"
    @State private var smtpPort = "465"
    @State private var oauthProvider = "gmail"
    @State private var oauthEmail = "me@gmail.com"
    @State private var oauthAccessToken = ""
    @State private var oauthRefreshToken = ""
    @State private var oauthExpiresAt = "0"
    @State private var oauthClientId = ""
    @State private var oauthClientSecret = ""
    @State private var oauthRedirectUri = OAuthFlowKt.defaultOAuthRedirectUri()
    @State private var oauthState = UUID().uuidString
    @State private var oauthVerifier = UUID().uuidString + UUID().uuidString
    @State private var oauthAuthorizationCode = ""
    @State private var reconnectingAccountId = ""
    @State private var rssFeedUrl = "https://example.com/feed.xml"
    @State private var rssDisplayName = "Example Feed"
    @State private var accountStatus = "Not loaded."
    @State private var accountJson = ""
    @State private var coreAccounts: [AccountSummary] = []
    @State private var selectedCoreAccountId = ""
    @State private var coreFolders: [FolderSummary] = []
    @State private var selectedCoreFolder = "inbox"
    @State private var mailSearch = ""
    @State private var mailFilter: IosFilterMode = .all
    @State private var coreThreads: [ThreadSummary] = []
    @State private var mailboxCursor = ""
    @State private var mailboxAccountCursors: [String: String] = [:]
    @State private var isLoadingMoreThreads = false
    @State private var starredItems: [StarredItemSummary] = []
    @State private var selectedCoreThread: ThreadSummary?
    @State private var coreMessages: [MessageBody] = []
    @State private var moveThreadTarget: ThreadSummary?
    @State private var moveThreadFolders: [FolderSummary] = []
    @State private var moveThreadDialogPresented = false
    @State private var moveThreadCreateDialogPresented = false
    @State private var moveThreadNewFolderName = ""
    @State private var copyThreadTarget: ThreadSummary?
    @State private var copyThreadFolders: [FolderSummary] = []
    @State private var copyThreadDialogPresented = false
    @State private var conversationHtmlOverrides: [String: Bool] = [:]
    @State private var messageCursor = ""
    @State private var isLoadingMoreMessages = false
    @State private var threadSearch = ""
    @State private var activeThreadSearchIndex = 0
    @State private var kanbanBoards: [IosKanbanBoardSpec] = loadIosKanbanBoards()
    @State private var activeKanbanBoardId = UserDefaults.standard.string(forKey: "ios_kanban_active_board_v1") ?? ""
    @State private var kanbanThreadsByColumn: [String: [ThreadSummary]] = [:]
    @State private var kanbanStatusByColumn: [String: String] = [:]
    @State private var kanbanCursorByColumn: [String: String] = [:]
    @State private var kanbanAccountCursorsByColumn: [String: [String: String]] = [:]
    @State private var kanbanLoadingMoreColumns: Set<String> = []
    @State private var kanbanSearch = ""
    @State private var kanbanFilter: IosFilterMode = .all
    @State private var kanbanNewBoardName = ""
    @State private var kanbanBoardAvatarUrl = ""
    @State private var kanbanBoardWallpaperPresetId = ""
    @State private var kanbanBoardWallpaperUrl = ""
    @State private var kanbanCreateFolderName = ""
    @State private var kanbanSelectedAccountId = ""
    @State private var kanbanSelectedFolderId = "inbox"
    @State private var composeTo = "user1@mail.localhost"
    @State private var composeFromAccountId = ""
    @State private var composeFromEmail = ""
    @State private var composeDraftId = ""
    @State private var composeDraftSaved = false
    @State private var composeInReplyTo = ""
    @State private var composeReferences = ""
    @State private var composeCc = ""
    @State private var composeBcc = ""
    @State private var composeSubject = "Hello from Meron iOS"
    @State private var composeBody = "This message was sent from the native iOS shell through meron-core."
    @State private var quickReplyBody = ""
    @State private var recipientSuggestionField = ""
    @State private var recipientSuggestions: [ContactSuggestion] = []
    @State private var selectedTab: IosAppTab = .mail

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
            if !visible && selectedTab == .starred {
                selectedTab = .mail
            }
        }
        .onChange(of: showUnifiedInbox) { _, visible in
            if !visible && selectedCoreAccountId == iosUnifiedAccountId {
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
            case .success(let url):
                addAttachment(from: url)
            case .failure(let error):
                attachmentError = error.localizedDescription
            }
        }
        .fileImporter(isPresented: $isQuickReplyFileImporterPresented, allowedContentTypes: [.item]) { result in
            switch result {
            case .success(let url):
                addQuickReplyAttachment(from: url)
            case .failure(let error):
                attachmentError = error.localizedDescription
            }
        }
        .fileImporter(isPresented: $isOpmlImporterPresented, allowedContentTypes: [.xml, .item]) { result in
            switch result {
            case .success(let url):
                importOpml(from: url)
            case .failure(let error):
                accountStatus = "OPML import failed: \(error.localizedDescription)"
            }
        }
        .fileImporter(isPresented: $isAccountMediaImporterPresented, allowedContentTypes: [.image]) { result in
            switch result {
            case .success(let url):
                importAccountMedia(from: url)
            case .failure(let error):
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
            case .failure(let error):
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

    private var mailView: some View {
        List {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Text(coreAccounts.isEmpty ? "Connect your mail" : accountStatus)
                        .font(.headline)
                    Text(coreAccounts.isEmpty ? "Add an account from the Accounts tab to load your inbox." : "Read, triage, and reply from the selected mailbox.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }

            if coreAccounts.isEmpty {
                Section {
                    Label("No accounts configured", systemImage: "tray")
                        .foregroundStyle(.secondary)
                }
            } else {
                if !mailReconnectAccounts.isEmpty {
                    Section("Needs Reconnect") {
                        ForEach(mailReconnectAccounts, id: \.id) { account in
                            VStack(alignment: .leading, spacing: 8) {
                                Label("\(accountLabel(account)) needs credentials", systemImage: "key")
                                    .font(.headline)
                                Text("Update the password or OAuth sign-in before syncing this account.")
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                Button {
                                    reconnectAccount(account)
                                } label: {
                                    Label("Reconnect", systemImage: "arrow.triangle.2.circlepath")
                                }
                            }
                        }
                    }
                }

                Section("Mailbox") {
                    Picker("Account", selection: $selectedCoreAccountId) {
                        if showUnifiedInbox {
                            Text("Unified inbox")
                                .tag(iosUnifiedAccountId)
                        }
                        ForEach(visibleNavigationAccounts, id: \.id) { account in
                            Text(account.displayName.isEmpty ? (account.email.isEmpty ? account.id : account.email) : account.displayName)
                                .tag(account.id)
                        }
                    }
                    .onChange(of: selectedCoreAccountId) { _, _ in
                        selectedCoreFolder = "inbox"
                        coreFolders = []
                        coreThreads = []
                        selectedCoreThread = nil
                        coreMessages = []
                        mailboxCursor = ""
                        mailboxAccountCursors = [:]
                    }

                    if selectedCoreAccountId != iosUnifiedAccountId && !coreFolders.isEmpty {
                        Picker("Folder", selection: $selectedCoreFolder) {
                            ForEach(coreFolders, id: \.name) { folder in
                                Text(folder.unread > 0 ? "\(folder.name) (\(folder.unread))" : folder.name)
                                    .tag(folder.name)
                            }
                        }
                        .onChange(of: selectedCoreFolder) { _, _ in
                            selectedCoreThread = nil
                            coreMessages = []
                        }
                    }

                    TextField("Search mail", text: $mailSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.search)
                        .onSubmit {
                            searchSelectedMailbox()
                        }

                    Picker("Filter", selection: $mailFilter) {
                        ForEach(IosFilterMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: mailFilter) { _, _ in
                        searchSelectedMailbox()
                    }

                    Button {
                        syncSelectedAccount()
                    } label: {
                        Label("Sync Mailbox", systemImage: "arrow.clockwise")
                    }
                    Button {
                        searchSelectedMailbox()
                    } label: {
                        Label("Search Cached Mail", systemImage: "magnifyingglass")
                    }
                    if selectedAccountIsRss(selectedCoreAccountId) {
                        Button {
                            addFeedUrl = ""
                            isAddFeedPresented = true
                        } label: {
                            Label("Add Feed", systemImage: "plus")
                        }
                        Button {
                            isOpmlImporterPresented = true
                        } label: {
                            Label("Import OPML", systemImage: "square.and.arrow.down")
                        }
                        Button {
                            exportSelectedAccountOpml()
                        } label: {
                            Label("Export OPML", systemImage: "square.and.arrow.up")
                        }
                    }
                    if coreThreads.contains(where: { $0.unread }) {
                        Button {
                            markSelectedMailboxAllRead()
                        } label: {
                            Label("Mark All Read", systemImage: "envelope.open")
                        }
                    }
                }
            }

            Section("Inbox") {
                if coreThreads.isEmpty {
                    Text(mailSearch.isEmpty && mailFilter == .all ? "Sync the selected mailbox to load cached threads." : "No messages match the current search and filter.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(coreThreads, id: \.id) { thread in
                        ThreadRow(thread: thread, showSenderImages: showSenderImages) {
                            readThread(thread)
                        } actions: {
                            Button(thread.unread ? "Mark Read" : "Mark Unread") {
                                markThreadRead(thread, seen: thread.unread)
                            }
                            Button(thread.starred ? "Unstar" : "Star") {
                                markThreadStarred(thread, starred: !thread.starred)
                            }
                            if isRssThread(thread) {
                                if !thread.feedUrl.isEmpty {
                                    Button("Copy Feed URL") {
                                        UIPasteboard.general.string = thread.feedUrl
                                        accountStatus = "Copied feed URL."
                                    }
                                }
                                Button("Remove Feed", role: .destructive) {
                                    removeRssFeed(thread)
                                }
                            } else {
                                Button("Archive") {
                                    archiveThread(thread)
                                }
                                Button("Move To") {
                                    presentMoveThreadDialog(thread)
                                }
                                Button("Copy To") {
                                    presentCopyThreadDialog(thread)
                                }
                                Button(threadDeleteActionLabel(thread), role: .destructive) {
                                    deleteThread(thread)
                                }
                            }
                        }
                    }
                    if !mailboxCursor.isEmpty || !mailboxAccountCursors.isEmpty || isLoadingMoreThreads {
                        Button {
                            loadMoreCoreThreads()
                        } label: {
                            if isLoadingMoreThreads {
                                Label("Loading Older", systemImage: "hourglass")
                            } else {
                                Label("Load Older", systemImage: "chevron.down")
                            }
                        }
                        .disabled(isLoadingMoreThreads)
                    }
                }
            }

            conversationSection
        }
        .listStyle(.insetGrouped)
    }

    private var hiddenNavigationAccountIds: Set<String> {
        Set(hiddenNavigationAccountsValue
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty })
    }

    private var visibleNavigationAccounts: [AccountSummary] {
        let hidden = hiddenNavigationAccountIds
        return coreAccounts.filter { !hidden.contains($0.id) }
    }

    private var mailReconnectAccounts: [AccountSummary] {
        let selectedId = selectedMailboxAccountId()
        let accounts = coreAccounts.filter { $0.needsReconnect && !MailStateKt.accountSummaryIsRss(account: $0) }
        if selectedId == iosUnifiedAccountId || selectedId.isEmpty {
            return accounts
        }
        return accounts.filter { $0.id == selectedId }
    }

    private var conversationSection: some View {
        ScrollViewReader { proxy in
            Section("Conversation") {
                if let selectedCoreThread {
                    Text(selectedCoreThread.subject.isEmpty ? selectedCoreThread.id : selectedCoreThread.subject)
                        .font(.headline)
                    if !isRssThread(selectedCoreThread) {
                        Button {
                            presentMoveThreadDialog(selectedCoreThread)
                        } label: {
                            Label("Move to Folder", systemImage: "folder")
                        }
                        Button {
                            presentCopyThreadDialog(selectedCoreThread)
                        } label: {
                            Label("Copy to Folder", systemImage: "doc.on.doc")
                        }
                    }
                }

                if coreMessages.isEmpty {
                    Text("Open a thread to read cached messages.")
                        .foregroundStyle(.secondary)
                } else {
                    Picker("View", selection: Binding(
                        get: { currentConversationPrefersHtml() },
                        set: { setCurrentConversationPrefersHtml($0) }
                    )) {
                        Text("HTML").tag(true)
                        Text("Plain Text").tag(false)
                    }
                    .pickerStyle(.segmented)
                    if currentConversationPrefersHtml() && !normalizedThreadSearch.isEmpty {
                        Text("Search uses plain text.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    ConversationDetailsDisclosure(
                        participants: conversationParticipants,
                        attachments: conversationAttachments,
                        onCopyEmail: { email in
                            UIPasteboard.general.string = email
                            accountStatus = "Copied email address."
                        },
                        onComposeTo: { person in
                            composeFromAccountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
                            composeFromEmail = ""
                            composeTo = person.email
                            composeCc = ""
                            composeBcc = ""
                            composeSubject = ""
                            composeBody = ""
                            attachments = []
                            composeDraftId = ""
                            composeDraftSaved = false
                            selectedTab = .compose
                        },
                        onOpenAttachment: openMessageAttachment
                    )
                    threadSearchControls
                    if !messageCursor.isEmpty || isLoadingMoreMessages {
                        Button {
                            loadMoreThreadMessages()
                        } label: {
                            if isLoadingMoreMessages {
                                Label("Loading Older Messages", systemImage: "hourglass")
                            } else {
                                Label("Load Older Messages", systemImage: "chevron.up")
                            }
                        }
                        .disabled(isLoadingMoreMessages)
                    }
                    ForEach(coreMessages, id: \.id) { message in
                        let activeMatch = message.id == activeThreadSearchId
                        ConversationMessageRow(
                            message: message,
                            activeSearchMatch: activeMatch,
                            renderHtml: shouldRenderHtml(message) && normalizedThreadSearch.isEmpty,
                            canComposeFromMessage: selectedCoreThread.map { !isRssThread($0) } ?? false,
                            onOpenAttachment: openMessageAttachment,
                            onCopy: { label, value in
                                UIPasteboard.general.string = value
                                accountStatus = "Copied \(label.lowercased())."
                            },
                            onForward: { openMessageCompose(message, forward: true) },
                            onEditAsNew: { openMessageCompose(message, forward: false) },
                            onToggleRead: { toggleMessageRead(message) },
                            onToggleStarred: { toggleMessageStarred(message) },
                            onDelete: { deleteMessage(message) }
                        )
                        .id(message.id)
                    }

                    if let selectedCoreThread, !isRssThread(selectedCoreThread) {
                        quickReplySection
                    }
                }
            }
            .onChange(of: activeThreadSearchId) { _, id in
                guard !id.isEmpty else { return }
                withAnimation {
                    proxy.scrollTo(id, anchor: .center)
                }
            }
        }
    }

    private var threadSearchControls: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Search conversation", text: $threadSearch)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .onChange(of: threadSearch) { _, _ in
                    activeThreadSearchIndex = 0
                }
            HStack {
                Text(threadSearchMatchLabel)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Button("Previous") {
                    goToThreadSearchMatch(-1)
                }
                .disabled(threadSearchMatches.isEmpty)
                Button("Next") {
                    goToThreadSearchMatch(1)
                }
                .disabled(threadSearchMatches.isEmpty)
                if !threadSearch.isEmpty {
                    Button {
                        threadSearch = ""
                        activeThreadSearchIndex = 0
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                    }
                    .buttonStyle(.borderless)
                }
            }
        }
    }

    private var quickReplySection: some View {
        Group {
            if !quickReplyAttachments.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack {
                        ForEach(quickReplyAttachments, id: \.id) { attachment in
                            Button {
                                quickReplyAttachments.removeAll { $0.id == attachment.id }
                                quickReplyFailure = ""
                            } label: {
                                Label(attachment.displayName, systemImage: "xmark.circle")
                                    .lineLimit(1)
                            }
                            .buttonStyle(.bordered)
                        }
                    }
                }
            }
            TextEditor(text: $quickReplyBody)
                .frame(minHeight: 90)
                .onChange(of: quickReplyBody) { _, _ in
                    quickReplyFailure = ""
                }
                .onKeyPress(keys: [.return]) { press in
                    if sendShortcutMatches(press, mode: sendShortcutMode) &&
                        (!quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !quickReplyAttachments.isEmpty) {
                        sendQuickReply()
                        return .handled
                    }
                    return .ignored
                }
            if !quickReplyFailure.isEmpty {
                HStack {
                    Label(quickReplyFailure, systemImage: "exclamationmark.circle")
                        .font(.caption)
                        .foregroundStyle(.red)
                    Spacer()
                    Button("Retry") {
                        sendQuickReply()
                    }
                    .disabled(quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && quickReplyAttachments.isEmpty)
                }
            }
            Button {
                isQuickReplyFileImporterPresented = true
            } label: {
                Label("Attach File", systemImage: "paperclip")
            }
            Button {
                openQuickReplyInFullEditor()
            } label: {
                Label("Open Full Editor", systemImage: "arrow.up.left.and.arrow.down.right")
            }
            Button {
                sendQuickReply()
            } label: {
                Label("Send Reply", systemImage: "arrowshape.turn.up.left")
            }
            .disabled(quickReplyBody.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && quickReplyAttachments.isEmpty)
        }
    }

    private var normalizedThreadSearch: String {
        threadSearch.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    }

    private var threadSearchMatches: [String] {
        guard !normalizedThreadSearch.isEmpty else { return [] }
        return coreMessages
            .filter { conversationMessageSearchText($0).contains(normalizedThreadSearch) }
            .map(\.id)
    }

    private var activeThreadSearchId: String {
        guard !threadSearchMatches.isEmpty else { return "" }
        return threadSearchMatches[min(activeThreadSearchIndex, threadSearchMatches.count - 1)]
    }

    private var threadSearchMatchLabel: String {
        guard !normalizedThreadSearch.isEmpty else { return "" }
        return "\(threadSearchMatches.isEmpty ? 0 : min(activeThreadSearchIndex + 1, threadSearchMatches.count))/\(threadSearchMatches.count)"
    }

    private func goToThreadSearchMatch(_ delta: Int) {
        let matches = threadSearchMatches
        guard !matches.isEmpty else { return }
        let next = activeThreadSearchIndex + delta
        if next < 0 {
            activeThreadSearchIndex = matches.count - 1
        } else if next >= matches.count {
            activeThreadSearchIndex = 0
        } else {
            activeThreadSearchIndex = next
        }
    }

    private func conversationMessageSearchText(_ message: MessageBody) -> String {
        [
            message.subject,
            message.from,
            message.fromAddr,
            message.to,
            message.cc,
            message.body,
            message.bodyHtml.replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression),
            message.attachments.map(\.filename).joined(separator: " ")
        ].joined(separator: " ").lowercased()
    }

    private var conversationAttachments: [MessageAttachment] {
        Array(coreMessages.flatMap(\.attachments).reversed())
    }

    private var conversationParticipants: [ConversationParticipant] {
        guard selectedCoreThread.map({ !isRssThread($0) }) ?? false else { return [] }
        let ownEmail = selectedCoreThread
            .flatMap { thread in coreAccounts.first(where: { $0.id == thread.accountId })?.email }
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() } ?? ""
        var order: [String] = []
        var rows: [String: (name: String, email: String, count: Int, isSelf: Bool)] = [:]

        func add(name: String, email: String) {
            let normalized = email
                .trimmingCharacters(in: CharacterSet(charactersIn: " <>;,"))
                .lowercased()
            guard !normalized.isEmpty, normalized.contains("@") else { return }
            if var existing = rows[normalized] {
                existing.count += 1
                if (existing.name.isEmpty || existing.name == existing.email),
                   !name.isEmpty,
                   name != email {
                    existing.name = name
                }
                rows[normalized] = existing
            } else {
                order.append(normalized)
                rows[normalized] = (
                    name: (!name.isEmpty && name != email) ? name : normalized,
                    email: normalized,
                    count: 1,
                    isSelf: normalized == ownEmail
                )
            }
        }

        for message in coreMessages {
            add(name: message.from, email: message.fromAddr.isEmpty ? message.from : message.fromAddr)
            parseAddressList(message.to).forEach { add(name: $0.name, email: $0.email) }
            parseAddressList(message.cc).forEach { add(name: $0.name, email: $0.email) }
        }

        return order.compactMap { rows[$0] }
            .map { ConversationParticipant(name: $0.name, email: $0.email, count: $0.count, isSelf: $0.isSelf) }
            .sorted {
                if $0.isSelf != $1.isSelf { return !$0.isSelf }
                return $0.count > $1.count
            }
    }

    private func parseAddressList(_ value: String) -> [(name: String, email: String)] {
        value
            .split(whereSeparator: { $0 == "," || $0 == ";" })
            .compactMap { raw -> (name: String, email: String)? in
                let entry = raw.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !entry.isEmpty else { return nil }
                if let open = entry.lastIndex(of: "<"), let close = entry.lastIndex(of: ">"), open < close {
                    let name = entry[..<open]
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                    let email = entry[entry.index(after: open)..<close].trimmingCharacters(in: .whitespacesAndNewlines)
                    return (name, email)
                }
                return (entry, entry)
            }
    }

    private var composeView: some View {
        List {
            Section {
                let identities = composeIdentityCandidates()
                if identities.count > 1 {
                    Picker("From", selection: Binding(
                        get: { selectedComposeIdentity().map(identityKey) ?? "" },
                        set: { key in
                            let parts = key.split(separator: "|", maxSplits: 1).map(String.init)
                            if parts.count == 2 {
                                composeFromAccountId = parts[0]
                                composeFromEmail = parts[1]
                            }
                        }
                    )) {
                        ForEach(Array(identities.enumerated()), id: \.offset) { _, identity in
                            Text(MailStateKt.formatSendIdentity(identity: identity))
                                .tag(identityKey(identity))
                        }
                    }
                }
                TextField("To", text: $composeTo)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeTo) { _, value in loadRecipientSuggestions(field: "to", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "to", value: composeTo) }
                contactSuggestionRows(field: "to")
                TextField("Cc", text: $composeCc)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeCc) { _, value in loadRecipientSuggestions(field: "cc", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "cc", value: composeCc) }
                contactSuggestionRows(field: "cc")
                TextField("Bcc", text: $composeBcc)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onChange(of: composeBcc) { _, value in loadRecipientSuggestions(field: "bcc", value: value) }
                    .onTapGesture { loadRecipientSuggestions(field: "bcc", value: composeBcc) }
                contactSuggestionRows(field: "bcc")
                TextField("Subject", text: $composeSubject)
                TextEditor(text: $composeBody)
                    .frame(minHeight: 220)
                    .onKeyPress(keys: [.return]) { press in
                        if sendShortcutMatches(press, mode: sendShortcutMode) {
                            sendCoreMail()
                            return .handled
                        }
                        return .ignored
                    }
            }

            Section("Attachments") {
                Button {
                    isFileImporterPresented = true
                } label: {
                    Label("Attach File", systemImage: "paperclip")
                }
                if let attachmentError {
                    Text(attachmentError)
                        .foregroundStyle(.red)
                }
                if attachments.isEmpty {
                    Text("No attachments selected.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(attachments, id: \.id) { attachment in
                        VStack(alignment: .leading) {
                            Text(attachment.displayName)
                            Text("\(attachment.sizeBytes) bytes")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    Button("Clear Attachments", role: .destructive) {
                        attachments = []
                    }
                }
            }

            Section {
                if let draft = mailtoDraft {
                    Text("Loaded mailto draft for \(draft.to)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Button {
                    saveComposeDraft()
                } label: {
                    Label("Save Draft", systemImage: "tray.and.arrow.down")
                }
                Button(role: .destructive) {
                    discardComposeDraft()
                } label: {
                    Label("Discard Draft", systemImage: "trash")
                }
                Button {
                    sendCoreMail()
                } label: {
                    Label("Send Message", systemImage: "paperplane.fill")
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    @ViewBuilder
    private func contactSuggestionRows(field: String) -> some View {
        if recipientSuggestionField == field && !recipientSuggestions.isEmpty {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack {
                    ForEach(recipientSuggestions, id: \.addr) { contact in
                        Button {
                            acceptRecipientSuggestion(field: field, contact: contact)
                        } label: {
                            Text(MailStateKt.formatContactSuggestion(contact: contact))
                                .lineLimit(1)
                        }
                        .buttonStyle(.bordered)
                    }
                }
            }
        }
    }

    private var starredView: some View {
        List {
            Section {
                Button {
                    loadStarredItems()
                } label: {
                    Label("Refresh Starred", systemImage: "arrow.clockwise")
                }
            }

            Section("Items") {
                if starredItems.isEmpty {
                    Text("Star messages or feed items to collect them here.")
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(starredItems, id: \.id) { item in
                        StarredItemRow(item: item, showSenderImages: showSenderImages) {
                            readStarredItem(item)
                        } onToggleRead: {
                            markStarredItemRead(item, seen: item.unread)
                        } onUnstar: {
                            unstarStarredItem(item)
                        } onDelete: {
                            deleteStarredMailItem(item)
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .onAppear {
            if starredItems.isEmpty {
                loadStarredItems()
            }
        }
    }

    private var kanbanView: some View {
        List {
            Section {
                if kanbanBoards.isEmpty {
                    Text("Load accounts to create a board.")
                        .foregroundStyle(.secondary)
                } else {
                    Picker("Board", selection: $activeKanbanBoardId) {
                        ForEach(kanbanBoards) { board in
                            Text(board.name).tag(board.id)
                        }
                    }
                    .onChange(of: activeKanbanBoardId) { _, next in
                        UserDefaults.standard.set(next, forKey: "ios_kanban_active_board_v1")
                        kanbanNewBoardName = activeKanbanBoard?.name ?? ""
                        kanbanBoardAvatarUrl = activeKanbanBoard?.avatarUrl ?? ""
                        kanbanBoardWallpaperPresetId = activeKanbanBoard?.wallpaperPresetId ?? ""
                        kanbanBoardWallpaperUrl = activeKanbanBoard?.wallpaperUrl ?? ""
                        loadActiveKanbanBoard(refresh: false)
                    }

                    HStack {
                        KanbanBoardStylePreview(board: activeKanbanBoard)

                        TextField("Board name", text: $kanbanNewBoardName)
                        Button {
                            renameActiveKanbanBoard()
                        } label: {
                            Label("Rename", systemImage: "pencil")
                        }
                    }

                    TextField("Board image URL", text: $kanbanBoardAvatarUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    TextField("Wallpaper preset", text: $kanbanBoardWallpaperPresetId)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    TextField("Wallpaper image URL", text: $kanbanBoardWallpaperUrl)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    Button {
                        updateActiveKanbanBoardAppearance()
                    } label: {
                        Label("Save Board Style", systemImage: "photo")
                    }
                }

                Button {
                    createKanbanBoard()
                } label: {
                    Label("New Board", systemImage: "plus.rectangle.on.rectangle")
                }

                Button(role: .destructive) {
                    deleteActiveKanbanBoard()
                } label: {
                    Label("Delete Board", systemImage: "trash")
                }
                .disabled(kanbanBoards.isEmpty)
            }

            Section("Columns") {
                if coreAccounts.isEmpty {
                    Button {
                        listAccounts()
                    } label: {
                        Label("Load Accounts", systemImage: "person.crop.circle.badge")
                    }
                } else {
                    Button {
                        addKanbanColumn(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
                    } label: {
                        Label("Add Unified Inbox", systemImage: "tray.2")
                    }

                    Picker("Account", selection: $kanbanSelectedAccountId) {
                        ForEach(coreAccounts, id: \.id) { account in
                            Text(accountLabel(account)).tag(account.id)
                        }
                    }
                    .onChange(of: kanbanSelectedAccountId) { _, accountId in
                        loadFoldersForKanbanAccount(accountId)
                    }

                    Picker("Folder", selection: $kanbanSelectedFolderId) {
                        ForEach(coreFolders, id: \.name) { folder in
                            Text(folder.name).tag(folder.name)
                        }
                    }

                    Button {
                        addKanbanColumn(accountId: kanbanSelectedAccountId, folderId: kanbanSelectedFolderId)
                    } label: {
                        Label("Add Column", systemImage: "rectangle.badge.plus")
                    }

                    HStack {
                        TextField("New folder", text: $kanbanCreateFolderName)
                        Button {
                            createFolderAndColumn()
                        } label: {
                            Label("Create", systemImage: "folder.badge.plus")
                        }
                    }
                    .disabled(selectedKanbanAccountIsRss())
                }
            }

            Section("Board") {
                if let board = activeKanbanBoard {
                    TextField("Search board", text: $kanbanSearch)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.search)
                        .onSubmit {
                            loadActiveKanbanBoard(refresh: false)
                        }

                    Picker("Filter", selection: $kanbanFilter) {
                        ForEach(IosFilterMode.allCases) { mode in
                            Text(mode.label).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: kanbanFilter) { _, _ in
                        loadActiveKanbanBoard(refresh: false)
                    }

                    HStack {
                        Button {
                            loadActiveKanbanBoard(refresh: false)
                        } label: {
                            Label("Search Board", systemImage: "magnifyingglass")
                        }
                        if !kanbanSearch.isEmpty {
                            Button {
                                kanbanSearch = ""
                                loadActiveKanbanBoard(refresh: false)
                            } label: {
                                Label("Clear", systemImage: "xmark.circle")
                            }
                        }
                    }

                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(board.columns) { column in
                                KanbanColumnView(
                                    title: kanbanColumnTitle(column),
                                    status: kanbanStatusByColumn[column.id] ?? "",
                                    threads: kanbanThreadsByColumn[column.id] ?? [],
                                    canLoadMore: kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && ((column.accountId == iosUnifiedAccountId && !(kanbanAccountCursorsByColumn[column.id] ?? [:]).isEmpty) || (column.accountId != iosUnifiedAccountId && !(kanbanCursorByColumn[column.id] ?? "").isEmpty)),
                                    isLoadingMore: kanbanLoadingMoreColumns.contains(column.id),
                                    moveTargets: board.columns.filter { $0.id != column.id && $0.accountId != iosUnifiedAccountId },
                                    targetTitle: { kanbanColumnTitle($0) },
                                    onRefresh: { loadKanbanColumn(column, refresh: true) },
                                    onLoadMore: { loadMoreKanbanColumn(column) },
                                    onMarkAllRead: { markKanbanColumnAllRead(column) },
                                    onRemoveColumn: { removeKanbanColumn(column) },
                                    onOpen: { readThread($0) },
                                    onArchive: { archiveOrRemoveKanbanThread($0, in: column) },
                                    onDelete: { deleteKanbanThread($0, in: column) },
                                    onToggleRead: { markKanbanThreadRead($0, in: column) },
                                    onToggleStar: { markKanbanThreadStarred($0, in: column) },
                                    onMove: { thread, target in moveKanbanThread(thread, from: column, to: target) },
                                    showSenderImages: showSenderImages,
                                    columnWidth: CGFloat(kanbanColumnWidth)
                                )
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    Button {
                        loadActiveKanbanBoard(refresh: true)
                    } label: {
                        Label("Refresh Board", systemImage: "arrow.clockwise")
                    }
                } else {
                    Text("Create a board and add folder columns.")
                        .foregroundStyle(.secondary)
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    private var accountsView: some View {
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
                    in: iosKanbanColumnMinWidth...iosKanbanColumnMaxWidth,
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

                                if account.needsReconnect && !MailStateKt.accountSummaryIsRss(account: account) {
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

    private func listAccounts() {
        accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 10).toJson())
        updateCoreAccounts(from: accountJson)
        accountStatus = "Loaded accounts."
    }

    private func loadStorageUsage(showStatus: Bool = true) {
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

    private func clearStorageCache() {
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

    private func autodiscoverPasswordAccount() {
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

    private func addPasswordAccount() {
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

    private func addRssAccount() {
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

    private func addOAuthAccount() {
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

    private func saveAccountSettings(account: AccountSummary, draft: AccountSettingsDraft) {
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

    private func removeAccount(_ account: AccountSummary) {
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

    private func moveAccount(_ account: AccountSummary, delta: Int) {
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

    private func reconnectAccount(_ account: AccountSummary) {
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

    private func launchOAuthFlow() {
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

    private func exchangeOAuthCode() {
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

    private func handleOAuthCallback(_ url: URL) {
        accountStatus = applyOAuthCallbackToComposeState(
            rawUrl: url.absoluteString,
            expectedState: oauthState,
            redirectUri: oauthRedirectUri,
            authorizationCode: &oauthAuthorizationCode
        )
    }

    private func updateCoreAccounts(from response: String) {
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

    private func selectedMailboxAccountId() -> String {
        if selectedCoreAccountId.isEmpty {
            return coreAccounts.isEmpty ? "" : iosUnifiedAccountId
        }
        return selectedCoreAccountId
    }

    private func setAccountNavigationVisible(accountId: String, visible: Bool) {
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

    private func syncSelectedAccount() {
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

    private func searchSelectedMailbox() {
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

    private func addFeedToSelectedRssAccount() {
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

    private func importOpml(from url: URL) {
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

    private func exportSelectedAccountOpml() {
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

    private func markSelectedMailboxAllRead() {
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

    private func loadCoreFoldersAndThreads(accountId: String, requestedFolder: String? = nil) {
        if accountId == iosUnifiedAccountId {
            loadUnifiedInbox(syncFirst: false)
            return
        }
        let folderResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 14, params: FolderListParams(accountId: accountId)).toJson()
        )
        let parsedFolders = MobileResponseParsersKt.parseFolderListResponse(responseJson: folderResponse)
        coreFolders = parsedFolders
        let fallbackFolder = parsedFolders.first?.name ?? "inbox"
        let folderId = parsedFolders.contains(where: { $0.name == (requestedFolder ?? selectedCoreFolder) })
            ? (requestedFolder ?? selectedCoreFolder)
            : fallbackFolder
        selectedCoreFolder = folderId
        loadCoreThreads(accountId: accountId, folderId: folderId)
    }

    private func unifiedMailboxAccounts(for cursors: [String: String]? = nil) -> [AccountSummary] {
        let accounts = coreAccounts.filter { $0.includedInUnified }
        guard let cursors else { return accounts }
        return accounts.filter { !(cursors[$0.id] ?? "").isEmpty }
    }

    private func mailThreadListPage(accountId: String, folderId: String, beforeCursor: String?) -> ThreadListPage? {
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: folderId,
            query: mailSearch.trimmingCharacters(in: .whitespacesAndNewlines),
            filter: mailFilter.rawValue,
            beforeCursor: beforeCursor,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 16, params: threadParams).toJson())
        accountJson = threadResponse
        if threadResponse.contains(#""error""#) {
            return nil
        }
        return MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
    }

    private func loadUnifiedInbox(syncFirst: Bool) {
        let accounts = unifiedMailboxAccounts()
        guard !accounts.isEmpty else {
            coreFolders = []
            coreThreads = []
            selectedCoreThread = nil
            coreMessages = []
            mailboxCursor = ""
            mailboxAccountCursors = [:]
            selectedCoreFolder = "inbox"
            accountStatus = "No accounts are included in Unified inbox."
            return
        }

        if syncFirst {
            for account in accounts {
                if !syncKanbanAccount(account, folderId: iosInboxFolderId) {
                    accountStatus = "Core sync failed."
                    return
                }
            }
        }

        var threads: [ThreadSummary] = []
        var nextCursors: [String: String] = [:]
        for account in accounts {
            guard let page = mailThreadListPage(accountId: account.id, folderId: iosInboxFolderId, beforeCursor: nil) else {
                accountStatus = "Unified inbox load failed."
                return
            }
            threads.append(contentsOf: page.threads)
            if mailSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !page.nextCursor.isEmpty {
                nextCursors[account.id] = page.nextCursor
            }
        }

        coreFolders = []
        selectedCoreFolder = iosInboxFolderId
        coreThreads = threads.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxCursor = ""
        mailboxAccountCursors = nextCursors
        if let selectedCoreThread, !coreThreads.contains(where: { $0.id == selectedCoreThread.id }) {
            self.selectedCoreThread = nil
            coreMessages = []
        }
        accountStatus = "Loaded \(coreThreads.count) \(mailFilter.label.lowercased()) thread(s) from Unified inbox."
    }

    private func loadCoreThreads(accountId: String, folderId: String) {
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: folderId,
            query: mailSearch.trimmingCharacters(in: .whitespacesAndNewlines),
            filter: mailFilter.rawValue,
            beforeCursor: nil,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 16, params: threadParams).toJson())
        let page = MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
        coreThreads = page.threads
        mailboxCursor = page.nextCursor
        mailboxAccountCursors = [:]
        accountJson = threadResponse
        if let selectedCoreThread, !coreThreads.contains(where: { $0.id == selectedCoreThread.id }) {
            self.selectedCoreThread = nil
            coreMessages = []
        }
        accountStatus = "Loaded \(coreThreads.count) \(mailFilter.label.lowercased()) thread(s) from \(folderId)."
    }

    private func loadMoreCoreThreads() {
        if selectedMailboxAccountId() == iosUnifiedAccountId {
            loadMoreUnifiedInbox()
            return
        }
        guard !mailboxCursor.isEmpty, !isLoadingMoreThreads else { return }
        let accountId = selectedMailboxAccountId()
        guard !accountId.isEmpty else { return }
        isLoadingMoreThreads = true
        let threadParams = ThreadListParams(
            accountId: accountId,
            folderId: selectedCoreFolder.isEmpty ? "inbox" : selectedCoreFolder,
            query: mailSearch.trimmingCharacters(in: .whitespacesAndNewlines),
            filter: mailFilter.rawValue,
            beforeCursor: mailboxCursor,
            refresh: false
        )
        let threadResponse = RustCoreBridge.invokeJson(MobileCommandsKt.threadListRequest(id: 46, params: threadParams).toJson())
        isLoadingMoreThreads = false
        accountJson = threadResponse
        if threadResponse.contains(#""error""#) {
            accountStatus = "Load older failed."
            return
        }
        let page = MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
        let existingIds = Set(coreThreads.map(\.id))
        let appended = page.threads.filter { !existingIds.contains($0.id) }
        coreThreads = (coreThreads + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxCursor = page.nextCursor
        accountStatus = appended.isEmpty ? "No older messages." : "Loaded \(appended.count) older message(s)."
    }

    private func loadMoreUnifiedInbox() {
        guard !mailboxAccountCursors.isEmpty, !isLoadingMoreThreads else { return }
        let cursors = mailboxAccountCursors
        let accounts = unifiedMailboxAccounts(for: cursors)
        guard !accounts.isEmpty else { return }
        isLoadingMoreThreads = true
        var threads: [ThreadSummary] = []
        var nextCursors: [String: String] = [:]
        for account in accounts {
            guard let page = mailThreadListPage(accountId: account.id, folderId: iosInboxFolderId, beforeCursor: cursors[account.id]) else {
                isLoadingMoreThreads = false
                accountStatus = "Load older failed."
                return
            }
            threads.append(contentsOf: page.threads)
            if !page.nextCursor.isEmpty {
                nextCursors[account.id] = page.nextCursor
            }
        }
        isLoadingMoreThreads = false
        let existingIds = Set(coreThreads.map(\.id))
        let appended = threads.filter { !existingIds.contains($0.id) }
        coreThreads = (coreThreads + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        mailboxAccountCursors = nextCursors
        mailboxCursor = ""
        accountStatus = appended.isEmpty ? "No older messages." : "Loaded \(appended.count) older message(s)."
    }

    private func loadStarredItems() {
        let response = RustCoreBridge.invokeJson(MobileCommandsKt.starredItemsRequest(id: 40).toJson())
        if response.contains(#""error""#) {
            accountStatus = "Starred load failed."
            accountJson = response
            return
        }
        starredItems = MobileResponseParsersKt.parseStarredItemsResponse(responseJson: response)
        accountJson = response
        accountStatus = "Loaded \(starredItems.count) starred item(s)."
    }

    private func readThread(_ thread: ThreadSummary) {
        selectedCoreThread = thread
        messageCursor = ""
        isLoadingMoreMessages = false
        threadSearch = ""
        activeThreadSearchIndex = 0
        let response: String
        if isRssThread(thread) {
            let params = RssThreadParams(threadId: thread.id, beforeCursor: nil, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.rssThreadRequest(id: 21, params: params).toJson())
        } else {
            let params = ThreadReadParams(threadId: thread.id, beforeCursor: nil, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.threadReadRequest(id: 21, params: params).toJson())
        }
        if response.contains(#""error""#) {
            accountStatus = "Thread read failed."
            accountJson = response
            return
        }
        let page = MobileResponseParsersKt.parseThreadReadPage(responseJson: response)
        coreMessages = page.messages
        messageCursor = page.nextCursor
        accountJson = response
        accountStatus = "Loaded \(coreMessages.count) message(s)."
        if !isRssThread(thread),
           MailStateKt.folderIsDrafts(folder: thread.folder),
           let draftMessage = coreMessages.last {
            openDraftCompose(draftMessage, thread: thread)
        }
    }

    private func loadMoreThreadMessages() {
        guard let thread = selectedCoreThread,
              !messageCursor.isEmpty,
              !isLoadingMoreMessages else { return }
        let cursor = messageCursor
        isLoadingMoreMessages = true
        let response: String
        if isRssThread(thread) {
            let params = RssThreadParams(threadId: thread.id, beforeCursor: cursor, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.rssThreadRequest(id: 47, params: params).toJson())
        } else {
            let params = ThreadReadParams(threadId: thread.id, beforeCursor: cursor, limit: nil)
            response = RustCoreBridge.invokeJson(MobileCommandsKt.threadReadRequest(id: 47, params: params).toJson())
        }
        isLoadingMoreMessages = false
        accountJson = response
        if response.contains(#""error""#) {
            accountStatus = "Load older messages failed."
            return
        }
        let page = MobileResponseParsersKt.parseThreadReadPage(responseJson: response)
        let existingIds = Set(coreMessages.map(\.id))
        let older = page.messages.filter { !existingIds.contains($0.id) }
        coreMessages = (older + coreMessages).sorted { $0.dateEpochSeconds < $1.dateEpochSeconds }
        messageCursor = page.nextCursor
        accountStatus = older.isEmpty ? "No older messages in this thread." : "Loaded \(older.count) older message(s)."
    }

    private func readStarredItem(_ item: StarredItemSummary) {
        readThread(
            ThreadSummary(
                id: item.threadId,
                accountId: item.accountId,
                folder: item.folder,
                subject: item.subject,
                sender: item.sender,
                preview: item.preview,
                unread: item.unread,
                starred: true,
                dateEpochSeconds: item.dateEpochSeconds,
                feedUrl: ""
            )
        )
    }

    private func defaultSendAccountId() -> String {
        if !selectedCoreAccountId.isEmpty,
           let account = coreAccounts.first(where: { $0.id == selectedCoreAccountId }),
           !MailStateKt.accountSummaryIsRss(account: account) {
            return selectedCoreAccountId
        }
        return coreAccounts.first(where: { !MailStateKt.accountSummaryIsRss(account: $0) })?.id ?? ""
    }

    private func newIosDraftMessageId(accountId: String) -> String {
        let domain = accountId.split(separator: "@", maxSplits: 1).last.map(String.init) ?? "meron"
        let normalizedDomain = domain.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "meron" : domain
        return "meron-draft-\(UUID().uuidString.lowercased())@\(normalizedDomain)"
    }

    private func composeIdentityCandidates() -> [SendIdentity] {
        coreAccounts
            .filter { !MailStateKt.accountSummaryIsRss(account: $0) && !$0.needsReconnect }
            .flatMap { MailStateKt.accountSendIdentities(account: $0) }
    }

    private func selectedComposeIdentity() -> SendIdentity? {
        let candidates = composeIdentityCandidates()
        if let selected = candidates.first(where: { $0.accountId == composeFromAccountId && $0.email == composeFromEmail }) {
            return selected
        }
        let defaultAccountId = defaultSendAccountId()
        return candidates.first(where: { $0.accountId == defaultAccountId }) ?? candidates.first
    }

    private func clearComposeDraftState() {
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

    private func loadRecipientSuggestions(field: String, value: String) {
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

    private func acceptRecipientSuggestion(field: String, contact: ContactSuggestion) {
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

    private func sendCoreMail() {
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

    private func saveComposeDraft() {
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

    private func discardComposeDraft() {
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

    private func sendQuickReply() {
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

    private func openQuickReplyInFullEditor() {
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

    private func openMessageAttachment(_ attachment: MessageAttachment) {
        if !attachment.url.isEmpty, let url = URL(string: attachment.url) {
            UIApplication.shared.open(url)
            return
        }
        guard !attachment.key.isEmpty else {
            accountStatus = "Attachment is not cached."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.attachmentReadRequest(
                id: 64,
                params: AttachmentReadParams(key: attachment.key)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Attachment open failed."
            accountJson = response
            return
        }
        let base64 = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
        guard let data = Data(base64Encoded: base64) else {
            accountStatus = "Attachment data is invalid."
            return
        }
        do {
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("MeronAttachments", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent(safeAttachmentFilename(attachment.filename))
            try data.write(to: url, options: .atomic)
            if attachment.mimeType.hasPrefix("image/"), let image = UIImage(data: data) {
                imagePreview = IosImagePreview(
                    title: attachment.filename.isEmpty ? "Image" : attachment.filename,
                    image: image,
                    url: url
                )
                return
            }
            shareableAttachment = ShareableFile(url: url)
            accountStatus = "Choose Save to Files or share this attachment."
        } catch {
            accountStatus = "Attachment open failed: \(error.localizedDescription)"
        }
    }

    private func openMessageCompose(_ message: MessageBody, forward: Bool) {
        var copiedAttachments: [DraftAttachment] = []
        for attachment in MailStateKt.forwardableAttachments(message: message) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.attachmentReadRequest(
                    id: 58,
                    params: AttachmentReadParams(key: attachment.key)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = forward ? "Forward attachment copy failed." : "Edit-as-new attachment copy failed."
                accountJson = response
                return
            }
            let data = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
            if !data.isEmpty {
                copiedAttachments.append(
                    MailStateKt.attachmentToDraftAttachment(attachment: attachment, dataBase64: data)
                )
            }
        }
        let draft = forward
            ? MailStateKt.messageForwardDraft(message: message, attachments: copiedAttachments)
            : MailStateKt.messageEditAsNewDraft(message: message, attachments: copiedAttachments)
        composeTo = draft.to
        composeCc = draft.cc
        composeBcc = draft.bcc
        composeSubject = draft.subject
        composeBody = draft.body
        attachments = draft.attachments
        composeFromAccountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        composeFromEmail = ""
        composeDraftId = ""
        composeDraftSaved = false
        composeInReplyTo = ""
        composeReferences = ""
        selectedTab = .compose
        accountStatus = forward ? "Forward draft ready." : "Copied message into compose."
    }

    private func openDraftCompose(_ message: MessageBody, thread: ThreadSummary) {
        var copiedAttachments: [DraftAttachment] = []
        for attachment in MailStateKt.forwardableAttachments(message: message) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.attachmentReadRequest(
                    id: 63,
                    params: AttachmentReadParams(key: attachment.key)
                ).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "Draft attachment copy failed."
                accountJson = response
                return
            }
            let data = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
            if !data.isEmpty {
                copiedAttachments.append(MailStateKt.attachmentToDraftAttachment(attachment: attachment, dataBase64: data))
            }
        }
        composeTo = message.to
        composeCc = message.cc
        composeBcc = message.bcc
        composeSubject = message.subject
        composeBody = message.body
        attachments = copiedAttachments
        composeFromAccountId = thread.accountId
        composeFromEmail = ""
        let cleanedMessageId = message.messageId.trimmingCharacters(in: CharacterSet(charactersIn: "<> \n\t"))
        composeDraftId = cleanedMessageId.isEmpty ? newIosDraftMessageId(accountId: thread.accountId) : cleanedMessageId
        composeDraftSaved = true
        composeInReplyTo = ""
        composeReferences = ""
        selectedTab = .compose
        accountStatus = "Draft ready."
    }

    private func archiveThread(_ thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: nil, messageIds: [])
        runThreadAction(
            requestJson: MobileCommandsKt.archiveThreadRequest(id: 17, params: params).toJson(),
            successStatus: "Archive complete."
        )
    }

    private func deleteThread(_ thread: ThreadSummary) {
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        runThreadAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 18, params: params).toJson(),
            successStatus: threadDeleteSuccessStatus(thread)
        )
    }

    private func presentMoveThreadDialog(_ thread: ThreadSummary) {
        guard !isRssThread(thread) else {
            accountStatus = "RSS feeds move between RSS accounts from Kanban."
            return
        }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 65, params: FolderListParams(accountId: thread.accountId)).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Folder list failed."
            accountJson = response
            return
        }
        let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
            .filter { $0.name.caseInsensitiveCompare(thread.folder) != .orderedSame }
        moveThreadFolders = folders
        moveThreadTarget = thread
        moveThreadDialogPresented = true
    }

    private func moveThread(_ thread: ThreadSummary, toFolder folder: String) {
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.moveThreadRequest(
                id: 66,
                params: MoveThreadParams(threadId: thread.id, targetFolderId: folder)
            ).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Move failed."
            accountJson = response
            return
        }
        coreThreads.removeAll { $0.id == thread.id }
        if selectedCoreThread?.id == thread.id {
            selectedCoreThread = nil
            coreMessages = []
            messageCursor = ""
            threadSearch = ""
            activeThreadSearchIndex = 0
        }
        accountStatus = "Move complete."
    }

    private func presentCopyThreadDialog(_ thread: ThreadSummary) {
        guard !isRssThread(thread) else {
            accountStatus = "RSS feeds can't be copied to mail folders."
            return
        }
        var targets: [FolderSummary] = []
        for account in coreAccounts where !MailStateKt.accountSummaryIsRss(account: account) {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.folderListRequest(id: 85, params: FolderListParams(accountId: account.id)).toJson()
            )
            if response.contains(#""error""#) {
                accountStatus = "Folder list failed."
                accountJson = response
                return
            }
            targets.append(contentsOf: MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
                .filter { folder in
                    !(folder.accountId == thread.accountId && folder.name.caseInsensitiveCompare(thread.folder) == .orderedSame)
                })
        }
        copyThreadFolders = targets
        copyThreadTarget = thread
        copyThreadDialogPresented = true
    }

    private func copyTargetLabel(_ folder: FolderSummary) -> String {
        let account = coreAccounts.first(where: { $0.id == folder.accountId })
        let accountLabel = account?.displayName.isEmpty == false ? account?.displayName : account?.email
        guard let accountLabel, !accountLabel.isEmpty else {
            return folder.name
        }
        return "\(accountLabel) / \(folder.name)"
    }

    private func copyThread(_ thread: ThreadSummary, toFolder folder: FolderSummary) {
        let targetAccountId = folder.accountId.isEmpty ? thread.accountId : folder.accountId
        let params = CopyThreadParams(
            threadId: thread.id,
            targetAccountId: targetAccountId,
            targetFolderId: folder.name
        )
        let response = RustCoreBridge.invokeJson(
            CoreRequest(id: 86, method: "mail.copy", paramsJson: params.toJson()).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Copy failed."
            accountJson = response
            return
        }
        copyThreadDialogPresented = false
        copyThreadTarget = nil
        accountStatus = "Copy complete."
    }

    private func createFolderAndMoveThread(_ thread: ThreadSummary, name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            accountStatus = "Folder name is required."
            return
        }
        let createResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderCreateRequest(
                id: 67,
                params: FolderCreateParams(accountId: thread.accountId, name: trimmed)
            ).toJson()
        )
        if createResponse.contains(#""error""#) {
            accountStatus = "Create folder failed."
            accountJson = createResponse
            return
        }

        let listResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 68, params: FolderListParams(accountId: thread.accountId)).toJson()
        )
        let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: listResponse)
        if selectedCoreAccountId == thread.accountId {
            coreFolders = folders
        }
        moveThreadFolders = folders.filter { $0.name.caseInsensitiveCompare(thread.folder) != .orderedSame }
        let createdName = folders.first { $0.name.caseInsensitiveCompare(trimmed) == .orderedSame }?.name ?? trimmed
        moveThreadNewFolderName = ""
        moveThreadDialogPresented = false
        moveThreadCreateDialogPresented = false
        moveThread(thread, toFolder: createdName)
    }

    private func removeRssFeed(_ thread: ThreadSummary) {
        let params = RemoveRssFeedParams(threadId: thread.id)
        runThreadAction(
            requestJson: MobileCommandsKt.feedRemoveRequest(id: 18, params: params).toJson(),
            successStatus: "Removed feed."
        )
    }

    private func markThreadRead(_ thread: ThreadSummary, seen: Bool) {
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkReadParams(threadId: thread.id, seen: seen, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkReadRequest(id: 19, params: params).toJson()
        } else {
            let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [])
            requestJson = MobileCommandsKt.markReadRequest(id: 19, params: params).toJson()
        }
        runThreadAction(requestJson: requestJson, successStatus: seen ? "Marked read." : "Marked unread.")
    }

    private func markThreadStarred(_ thread: ThreadSummary, starred: Bool) {
        let requestJson: String
        if isRssThread(thread) {
            let params = RssMarkStarredParams(threadId: thread.id, starred: starred, itemKeys: [])
            requestJson = MobileCommandsKt.rssMarkStarredRequest(id: 20, params: params).toJson()
        } else {
            let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [])
            requestJson = MobileCommandsKt.markStarredRequest(id: 20, params: params).toJson()
        }
        runThreadAction(requestJson: requestJson, successStatus: starred ? "Starred." : "Unstarred.")
    }

    private func toggleMessageRead(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let seen = message.unread
        let params = MarkReadParams(threadId: thread.id, seen: seen, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.markReadRequest(id: 87, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Message update failed."
            accountJson = response
            return
        }
        coreMessages = coreMessages.map { $0.id == message.id ? $0.withFlags(unread: !seen) : $0 }
        let updatedUnread = coreMessages.contains(where: { $0.unread })
        selectedCoreThread = selectedCoreThread?.withUnread(updatedUnread)
        coreThreads = coreThreads.map { $0.id == thread.id ? $0.withUnread(updatedUnread) : $0 }
        accountStatus = seen ? "Marked read." : "Marked unread."
    }

    private func toggleMessageStarred(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let starred = !message.starred
        let params = MarkStarredParams(threadId: thread.id, starred: starred, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.markStarredRequest(id: 88, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Star failed."
            accountJson = response
            return
        }
        coreMessages = coreMessages.map { $0.id == message.id ? $0.withFlags(starred: starred) : $0 }
        let updatedStarred = coreMessages.contains(where: { $0.starred })
        selectedCoreThread = selectedCoreThread.map { thread in
            ThreadSummary(
                id: thread.id,
                accountId: thread.accountId,
                folder: thread.folder,
                subject: thread.subject,
                sender: thread.sender,
                preview: thread.preview,
                unread: thread.unread,
                starred: updatedStarred,
                dateEpochSeconds: thread.dateEpochSeconds,
                feedUrl: thread.feedUrl
            )
        }
        coreThreads = coreThreads.map { thread in
            thread.id == selectedCoreThread?.id ? ThreadSummary(
                id: thread.id,
                accountId: thread.accountId,
                folder: thread.folder,
                subject: thread.subject,
                sender: thread.sender,
                preview: thread.preview,
                unread: thread.unread,
                starred: updatedStarred,
                dateEpochSeconds: thread.dateEpochSeconds,
                feedUrl: thread.feedUrl
            ) : thread
        }
        accountStatus = starred ? "Starred." : "Unstarred."
    }

    private func deleteMessage(_ message: MessageBody) {
        guard let thread = selectedCoreThread else { return }
        let params = ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [message.id])
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.deleteThreadRequest(id: 89, params: params).toJson()
        )
        if response.contains(#""error""#) {
            accountStatus = "Delete failed."
            accountJson = response
            return
        }
        coreMessages.removeAll { $0.id == message.id }
        accountStatus = "Delete complete."
    }

    private func markStarredItemRead(_ item: StarredItemSummary, seen: Bool) {
        let requestJson: String
        if isRssStarredItem(item) {
            let params = RssMarkReadParams(threadId: item.threadId, seen: seen, itemKeys: [item.id])
            requestJson = MobileCommandsKt.rssMarkReadRequest(id: 41, params: params).toJson()
        } else {
            let params = MarkReadParams(threadId: item.threadId, seen: seen, messageIds: [item.id])
            requestJson = MobileCommandsKt.markReadRequest(id: 41, params: params).toJson()
        }
        runStarredItemAction(requestJson: requestJson, successStatus: seen ? "Marked read." : "Marked unread.")
    }

    private func unstarStarredItem(_ item: StarredItemSummary) {
        let requestJson: String
        if isRssStarredItem(item) {
            let params = RssMarkStarredParams(threadId: item.threadId, starred: false, itemKeys: [item.id])
            requestJson = MobileCommandsKt.rssMarkStarredRequest(id: 42, params: params).toJson()
        } else {
            let params = MarkStarredParams(threadId: item.threadId, starred: false, messageIds: [item.id])
            requestJson = MobileCommandsKt.markStarredRequest(id: 42, params: params).toJson()
        }
        runStarredItemAction(requestJson: requestJson, successStatus: "Unstarred.")
    }

    private func deleteStarredMailItem(_ item: StarredItemSummary) {
        guard !isRssStarredItem(item) else {
            accountStatus = "RSS items cannot be deleted."
            return
        }
        let params = ThreadActionParams(threadId: item.threadId, folderId: item.folder, messageIds: [item.id])
        runStarredItemAction(
            requestJson: MobileCommandsKt.deleteThreadRequest(id: 43, params: params).toJson(),
            successStatus: threadDeleteSuccessStatus(folder: item.folder)
        )
    }

    private func runStarredItemAction(requestJson: String, successStatus: String) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Starred item action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        loadStarredItems()
    }

    private func runThreadAction(requestJson: String, successStatus: String) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Thread action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        if !selectedCoreAccountId.isEmpty {
            loadCoreFoldersAndThreads(accountId: selectedCoreAccountId, requestedFolder: selectedCoreFolder)
        }
    }

    private var activeKanbanBoard: IosKanbanBoardSpec? {
        kanbanBoards.first(where: { $0.id == activeKanbanBoardId }) ?? kanbanBoards.first
    }

    private func persistKanbanBoards() {
        if let data = try? JSONEncoder().encode(kanbanBoards) {
            UserDefaults.standard.set(data, forKey: "ios_kanban_boards_v1")
        }
        if activeKanbanBoardId.isEmpty || !kanbanBoards.contains(where: { $0.id == activeKanbanBoardId }) {
            activeKanbanBoardId = kanbanBoards.first?.id ?? ""
        }
        UserDefaults.standard.set(activeKanbanBoardId, forKey: "ios_kanban_active_board_v1")
    }

    private func ensureKanbanDefaults() {
        guard !coreAccounts.isEmpty else { return }
        if kanbanSelectedAccountId.isEmpty {
            kanbanSelectedAccountId = coreAccounts.first?.id ?? ""
        }
        if kanbanBoards.isEmpty {
            var seen = Set<String>()
            let columns = ([IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)] + coreAccounts.map { IosKanbanColumnSpec(accountId: $0.id, folderId: iosInboxFolderId) })
                .filter { seen.insert($0.id).inserted }
            let board = IosKanbanBoardSpec(id: UUID().uuidString, name: "Kanban board", columns: columns)
            kanbanBoards = [board]
            activeKanbanBoardId = board.id
            persistKanbanBoards()
        } else if let first = kanbanBoards.first {
            var existing = Set(first.columns.map(\.id))
            var columns = first.columns
            let unified = IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
            if existing.insert(unified.id).inserted {
                columns.insert(unified, at: 0)
            }
            for account in coreAccounts {
                let column = IosKanbanColumnSpec(accountId: account.id, folderId: iosInboxFolderId)
                if existing.insert(column.id).inserted {
                    columns.append(column)
                }
            }
            if columns != first.columns {
                kanbanBoards = kanbanBoards.map { board in
                    if board.id != first.id { return board }
                    var next = board
                    next.columns = columns
                    return next
                }
                persistKanbanBoards()
            }
        }
        if kanbanNewBoardName.isEmpty {
            kanbanNewBoardName = activeKanbanBoard?.name ?? ""
        }
        if kanbanBoardAvatarUrl.isEmpty {
            kanbanBoardAvatarUrl = activeKanbanBoard?.avatarUrl ?? ""
        }
        if kanbanBoardWallpaperPresetId.isEmpty {
            kanbanBoardWallpaperPresetId = activeKanbanBoard?.wallpaperPresetId ?? ""
        }
        if kanbanBoardWallpaperUrl.isEmpty {
            kanbanBoardWallpaperUrl = activeKanbanBoard?.wallpaperUrl ?? ""
        }
    }

    private func createKanbanBoard() {
        let initialColumns = [IosKanbanColumnSpec(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)]
        let board = IosKanbanBoardSpec(id: UUID().uuidString, name: "Board \(kanbanBoards.count + 1)", columns: initialColumns)
        kanbanBoards.append(board)
        activeKanbanBoardId = board.id
        kanbanNewBoardName = board.name
        kanbanBoardAvatarUrl = ""
        kanbanBoardWallpaperPresetId = ""
        kanbanBoardWallpaperUrl = ""
        persistKanbanBoards()
        loadActiveKanbanBoard(refresh: false)
    }

    private func renameActiveKanbanBoard() {
        let name = kanbanNewBoardName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else { return }
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            var next = board
            next.name = name
            return next
        }
        persistKanbanBoards()
    }

    private func updateActiveKanbanBoardAppearance() {
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            var next = board
            next.avatarUrl = kanbanBoardAvatarUrl.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
            next.wallpaperPresetId = kanbanBoardWallpaperPresetId.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
            next.wallpaperUrl = kanbanBoardWallpaperUrl.trimmingCharacters(in: .whitespacesAndNewlines).nilIfBlank
            return next
        }
        persistKanbanBoards()
        accountStatus = "Saved board style."
    }

    private func deleteActiveKanbanBoard() {
        guard !kanbanBoards.isEmpty else { return }
        kanbanBoards.removeAll { $0.id == activeKanbanBoardId }
        if kanbanBoards.isEmpty {
            activeKanbanBoardId = ""
            kanbanThreadsByColumn = [:]
            kanbanStatusByColumn = [:]
        }
        persistKanbanBoards()
    }

    private func addKanbanColumn(accountId: String, folderId: String) {
        guard !accountId.isEmpty, !folderId.isEmpty, let board = activeKanbanBoard else { return }
        let column = IosKanbanColumnSpec(accountId: accountId, folderId: folderId)
        guard !board.columns.contains(column) else { return }
        kanbanBoards = kanbanBoards.map {
            if $0.id != board.id { return $0 }
            var next = $0
            next.columns.append(column)
            return next
        }
        persistKanbanBoards()
        loadKanbanColumn(column, refresh: true)
    }

    private func removeKanbanColumn(_ column: IosKanbanColumnSpec) {
        kanbanBoards = kanbanBoards.map { board in
            if board.id != activeKanbanBoardId { return board }
            var next = board
            next.columns = board.columns.filter { $0.id != column.id }
            return next
        }
        kanbanThreadsByColumn[column.id] = nil
        kanbanStatusByColumn[column.id] = nil
        kanbanCursorByColumn[column.id] = nil
        kanbanAccountCursorsByColumn[column.id] = nil
        kanbanLoadingMoreColumns.remove(column.id)
        persistKanbanBoards()
    }

    private func loadFoldersForKanbanAccount(_ accountId: String) {
        guard !accountId.isEmpty else { return }
        let response = RustCoreBridge.invokeJson(
            MobileCommandsKt.folderListRequest(id: 31, params: FolderListParams(accountId: accountId)).toJson()
        )
        let folders = MobileResponseParsersKt.parseFolderListResponse(responseJson: response)
        coreFolders = folders
        kanbanSelectedFolderId = folders.first?.name ?? "inbox"
    }

    private func createFolderAndColumn() {
        let accountId = kanbanSelectedAccountId
        let name = kanbanCreateFolderName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !accountId.isEmpty, !name.isEmpty else {
            accountStatus = "Choose an account and folder name."
            return
        }
        let request = MobileCommandsKt.folderCreateRequest(
            id: 32,
            params: FolderCreateParams(accountId: accountId, name: name)
        ).toJson()
        let response = RustCoreBridge.invokeJson(request)
        if response.contains(#""error""#) {
            accountStatus = "Create folder failed."
            accountJson = response
            return
        }
        kanbanCreateFolderName = ""
        loadFoldersForKanbanAccount(accountId)
        addKanbanColumn(accountId: accountId, folderId: name)
        accountStatus = "Folder created."
    }

    private func loadActiveKanbanBoard(refresh: Bool) {
        activeKanbanBoard?.columns.forEach { loadKanbanColumn($0, refresh: refresh) }
    }

    private func unifiedKanbanAccounts(for cursors: [String: String]? = nil) -> [AccountSummary] {
        let accounts = coreAccounts.filter { $0.includedInUnified }
        guard let cursors else { return accounts }
        return accounts.filter { !(cursors[$0.id] ?? "").isEmpty }
    }

    private func syncKanbanAccount(_ account: AccountSummary, folderId: String) -> Bool {
        let response: String
        if MailStateKt.accountSummaryIsRss(account: account) {
            response = RustCoreBridge.invokeJson(
                MobileCommandsKt.syncRssRequest(id: 33, params: SyncRssParams(accountId: account.id)).toJson()
            )
        } else {
            response = RustCoreBridge.invokeJson(
                MobileCommandsKt.syncMailRequest(
                    id: 33,
                    params: SyncMailParams(accountId: account.id, folderId: folderId, limit: 50, folders: true)
                ).toJson()
            )
        }
        if response.contains(#""error""#) {
            accountJson = response
            return false
        }
        return true
    }

    private func threadListPage(accountId: String, folderId: String, query: String, beforeCursor: String?) -> ThreadListPage? {
        let threadResponse = RustCoreBridge.invokeJson(
            MobileCommandsKt.threadListRequest(
                id: 34,
                params: ThreadListParams(
                    accountId: accountId,
                    folderId: folderId,
                    query: query,
                    filter: kanbanFilter.rawValue,
                    beforeCursor: beforeCursor,
                    refresh: false
                )
            ).toJson()
        )
        if threadResponse.contains(#""error""#) {
            accountJson = threadResponse
            return nil
        }
        return MobileResponseParsersKt.parseThreadListPage(responseJson: threadResponse)
    }

    private func loadKanbanColumn(_ column: IosKanbanColumnSpec, refresh: Bool) {
        guard !column.accountId.isEmpty else { return }
        kanbanStatusByColumn[column.id] = refresh ? "Refreshing..." : "Loading..."
        let trimmedQuery = kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines)

        if column.accountId == iosUnifiedAccountId {
            let accounts = unifiedKanbanAccounts()
            if refresh {
                for account in accounts {
                    if !syncKanbanAccount(account, folderId: iosInboxFolderId) {
                        kanbanStatusByColumn[column.id] = "Sync failed"
                        return
                    }
                }
            }

            var threads: [ThreadSummary] = []
            var nextCursors: [String: String] = [:]
            for account in accounts {
                guard let page = threadListPage(accountId: account.id, folderId: iosInboxFolderId, query: trimmedQuery, beforeCursor: nil) else {
                    kanbanStatusByColumn[column.id] = "Load failed"
                    return
                }
                threads.append(contentsOf: page.threads)
                if trimmedQuery.isEmpty && !page.nextCursor.isEmpty {
                    nextCursors[account.id] = page.nextCursor
                }
            }
            kanbanThreadsByColumn[column.id] = threads.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
            kanbanCursorByColumn[column.id] = ""
            kanbanAccountCursorsByColumn[column.id] = nextCursors
            kanbanStatusByColumn[column.id] = "\(threads.count) \(kanbanFilter.label.lowercased()) item(s)"
            return
        }

        let account = coreAccounts.first(where: { $0.id == column.accountId })
        if refresh, let account {
            if !syncKanbanAccount(account, folderId: column.folderId) {
                kanbanStatusByColumn[column.id] = "Sync failed"
                return
            }
        }

        guard let page = threadListPage(accountId: column.accountId, folderId: column.folderId, query: trimmedQuery, beforeCursor: nil) else {
            kanbanStatusByColumn[column.id] = "Load failed"
            return
        }
        kanbanThreadsByColumn[column.id] = page.threads
        kanbanCursorByColumn[column.id] = trimmedQuery.isEmpty ? page.nextCursor : ""
        kanbanAccountCursorsByColumn[column.id] = [:]
        kanbanStatusByColumn[column.id] = "\(page.threads.count) \(kanbanFilter.label.lowercased()) item(s)"
    }

    private func loadMoreKanbanColumn(_ column: IosKanbanColumnSpec) {
        guard kanbanSearch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        if column.accountId == iosUnifiedAccountId {
            let cursors = kanbanAccountCursorsByColumn[column.id] ?? [:]
            guard !cursors.isEmpty, !kanbanLoadingMoreColumns.contains(column.id) else { return }
            kanbanLoadingMoreColumns.insert(column.id)
            var threads: [ThreadSummary] = []
            var nextCursors: [String: String] = [:]
            for account in unifiedKanbanAccounts(for: cursors) {
                guard let page = threadListPage(accountId: account.id, folderId: iosInboxFolderId, query: "", beforeCursor: cursors[account.id]) else {
                    kanbanLoadingMoreColumns.remove(column.id)
                    kanbanStatusByColumn[column.id] = "Load older failed"
                    return
                }
                threads.append(contentsOf: page.threads)
                if !page.nextCursor.isEmpty {
                    nextCursors[account.id] = page.nextCursor
                }
            }
            kanbanLoadingMoreColumns.remove(column.id)
            let existing = kanbanThreadsByColumn[column.id] ?? []
            let existingIds = Set(existing.map(\.id))
            let appended = threads.filter { !existingIds.contains($0.id) }
            kanbanThreadsByColumn[column.id] = (existing + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
            kanbanAccountCursorsByColumn[column.id] = nextCursors
            kanbanStatusByColumn[column.id] = appended.isEmpty ? "No older items" : "\(existing.count + appended.count) item(s)"
            return
        }

        guard let cursor = kanbanCursorByColumn[column.id],
              !cursor.isEmpty,
              !kanbanLoadingMoreColumns.contains(column.id) else { return }
        kanbanLoadingMoreColumns.insert(column.id)
        guard let page = threadListPage(accountId: column.accountId, folderId: column.folderId, query: "", beforeCursor: cursor) else {
            kanbanLoadingMoreColumns.remove(column.id)
            kanbanStatusByColumn[column.id] = "Load older failed"
            return
        }
        kanbanLoadingMoreColumns.remove(column.id)
        let existing = kanbanThreadsByColumn[column.id] ?? []
        let existingIds = Set(existing.map(\.id))
        let appended = page.threads.filter { !existingIds.contains($0.id) }
        kanbanThreadsByColumn[column.id] = (existing + appended).sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
        kanbanCursorByColumn[column.id] = page.nextCursor
        kanbanStatusByColumn[column.id] = appended.isEmpty ? "No older items" : "\(existing.count + appended.count) item(s)"
    }

    private func markKanbanColumnAllRead(_ column: IosKanbanColumnSpec) {
        let unreadThreads = (kanbanThreadsByColumn[column.id] ?? []).filter { $0.unread }
        guard !unreadThreads.isEmpty else {
            accountStatus = "No unread cards."
            return
        }

        var failedResponse = ""
        if column.accountId == iosUnifiedAccountId {
            let accountById = Dictionary(uniqueKeysWithValues: coreAccounts.map { ($0.id, $0) })
            let mailAccountIds = Set(unreadThreads
                .map(\.accountId)
                .filter { accountId in
                    accountById[accountId].map { !MailStateKt.accountSummaryIsRss(account: $0) } ?? true
                })
            for accountId in mailAccountIds {
                let request = MobileCommandsKt.markAllReadRequest(
                    id: 49,
                    params: MarkAllReadParams(accountId: accountId, folderId: iosInboxFolderId)
                ).toJson()
                let response = RustCoreBridge.invokeJson(request)
                if response.contains(#""error""#) {
                    failedResponse = response
                    break
                }
            }
        } else if let account = coreAccounts.first(where: { $0.id == column.accountId }),
                  !MailStateKt.accountSummaryIsRss(account: account) {
            let request = MobileCommandsKt.markAllReadRequest(
                id: 49,
                params: MarkAllReadParams(accountId: column.accountId, folderId: column.folderId)
            ).toJson()
            failedResponse = RustCoreBridge.invokeJson(request)
        }

        if failedResponse.isEmpty {
            for thread in unreadThreads where isRssThread(thread) {
                let request = MobileCommandsKt.rssMarkReadRequest(
                    id: 49,
                    params: RssMarkReadParams(threadId: thread.id, seen: true, itemKeys: [])
                ).toJson()
                let response = RustCoreBridge.invokeJson(request)
                if response.contains(#""error""#) {
                    failedResponse = response
                    break
                }
            }
        }

        if failedResponse.contains(#""error""#) {
            accountStatus = "Kanban mark all read failed."
            accountJson = failedResponse
            return
        }

        let unreadIds = Set(unreadThreads.map(\.id))
        kanbanThreadsByColumn[column.id] = (kanbanThreadsByColumn[column.id] ?? []).map { thread in
            unreadIds.contains(thread.id) ? thread.withUnread(false) : thread
        }
        coreThreads = coreThreads.map { thread in
            unreadIds.contains(thread.id) ? thread.withUnread(false) : thread
        }
        accountStatus = "Marked \(unreadThreads.count) Kanban card(s) read."
    }

    private func archiveOrRemoveKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        if isRssThread(thread) {
            let request = MobileCommandsKt.feedRemoveRequest(id: 35, params: RemoveRssFeedParams(threadId: thread.id)).toJson()
            runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: "Removed feed.")
        } else {
            let request = MobileCommandsKt.archiveThreadRequest(
                id: 35,
                params: ThreadActionParams(threadId: thread.id, folderId: nil, messageIds: [])
            ).toJson()
            runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: "Archive complete.")
        }
    }

    private func deleteKanbanThread(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let request = MobileCommandsKt.deleteThreadRequest(
            id: 36,
            params: ThreadActionParams(threadId: thread.id, folderId: thread.folder, messageIds: [])
        ).toJson()
        runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: threadDeleteSuccessStatus(thread))
    }

    private func markKanbanThreadRead(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let request = isRssThread(thread)
            ? MobileCommandsKt.rssMarkReadRequest(id: 37, params: RssMarkReadParams(threadId: thread.id, seen: thread.unread, itemKeys: [])).toJson()
            : MobileCommandsKt.markReadRequest(id: 37, params: MarkReadParams(threadId: thread.id, seen: thread.unread, messageIds: [])).toJson()
        runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: thread.unread ? "Marked read." : "Marked unread.", remove: false)
    }

    private func markKanbanThreadStarred(_ thread: ThreadSummary, in column: IosKanbanColumnSpec) {
        let request = isRssThread(thread)
            ? MobileCommandsKt.rssMarkStarredRequest(id: 38, params: RssMarkStarredParams(threadId: thread.id, starred: !thread.starred, itemKeys: [])).toJson()
            : MobileCommandsKt.markStarredRequest(id: 38, params: MarkStarredParams(threadId: thread.id, starred: !thread.starred, messageIds: [])).toJson()
        runKanbanThreadAction(requestJson: request, sourceColumn: column, successStatus: thread.starred ? "Unstarred." : "Starred.", remove: false)
    }

    private func moveKanbanThread(_ thread: ThreadSummary, from source: IosKanbanColumnSpec, to target: IosKanbanColumnSpec) {
        let request: String
        if isRssThread(thread) {
            request = MobileCommandsKt.feedMoveRequest(
                id: 39,
                params: MoveRssFeedParams(threadId: thread.id, targetAccountId: target.accountId)
            ).toJson()
        } else {
            request = MobileCommandsKt.moveThreadRequest(
                id: 39,
                params: MoveThreadParams(threadId: thread.id, targetFolderId: target.folderId)
            ).toJson()
        }
        runKanbanThreadAction(requestJson: request, sourceColumn: source, successStatus: "Move complete.")
        loadKanbanColumn(target, refresh: false)
    }

    private func runKanbanThreadAction(
        requestJson: String,
        sourceColumn: IosKanbanColumnSpec,
        successStatus: String,
        remove: Bool = true
    ) {
        let response = RustCoreBridge.invokeJson(requestJson)
        if response.contains(#""error""#) {
            accountStatus = "Kanban action failed."
            accountJson = response
            return
        }
        accountStatus = successStatus
        if remove {
            loadKanbanColumn(sourceColumn, refresh: false)
        } else {
            loadKanbanColumn(sourceColumn, refresh: false)
        }
    }

    private func accountLabel(_ account: AccountSummary) -> String {
        if !account.displayName.isEmpty { return account.displayName }
        if !account.email.isEmpty { return account.email }
        return account.id
    }

    private func kanbanColumnTitle(_ column: IosKanbanColumnSpec) -> String {
        if column.accountId == iosUnifiedAccountId {
            return column.folderId == iosInboxFolderId ? "Unified Inbox" : "Unified / \(column.folderId)"
        }
        let account = coreAccounts.first(where: { $0.id == column.accountId }).map(accountLabel) ?? column.accountId
        return "\(account) / \(column.folderId)"
    }

    private func selectedKanbanAccountIsRss() -> Bool {
        guard let account = coreAccounts.first(where: { $0.id == kanbanSelectedAccountId }) else { return false }
        return MailStateKt.accountSummaryIsRss(account: account)
    }

    private func selectedAccountIsRss(_ accountId: String) -> Bool {
        guard let account = coreAccounts.first(where: { $0.id == accountId }) else {
            return false
        }
        return MailStateKt.accountSummaryIsRss(account: account)
    }

    private func isRssThread(_ thread: ThreadSummary) -> Bool {
        MailStateKt.threadIdIsRss(threadId: thread.id)
    }

    private func threadDeleteActionLabel(_ thread: ThreadSummary) -> String {
        threadDeleteActionLabel(folder: thread.folder)
    }

    private func threadDeleteActionLabel(folder: String) -> String {
        if MailStateKt.folderIsDrafts(folder: folder) {
            return "Discard Draft"
        }
        if MailStateKt.folderIsTrash(folder: folder) {
            return "Delete Forever"
        }
        return "Move to Trash"
    }

    private func threadDeleteSuccessStatus(_ thread: ThreadSummary) -> String {
        threadDeleteSuccessStatus(folder: thread.folder)
    }

    private func threadDeleteSuccessStatus(folder: String) -> String {
        if MailStateKt.folderIsDrafts(folder: folder) {
            return "Draft discarded."
        }
        if MailStateKt.folderIsTrash(folder: folder) {
            return "Thread deleted."
        }
        return "Thread moved to Trash."
    }

    private func shouldRenderHtml(_ message: MessageBody) -> Bool {
        guard !message.bodyHtml.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return false
        }
        let accountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        return conversationHtmlOverrides[accountId] ?? coreAccounts.first(where: { $0.id == accountId })?.conversationHtml ?? true
    }

    private func currentConversationPrefersHtml() -> Bool {
        let accountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        return conversationHtmlOverrides[accountId] ?? coreAccounts.first(where: { $0.id == accountId })?.conversationHtml ?? true
    }

    private func setCurrentConversationPrefersHtml(_ preferHtml: Bool) {
        let accountId = selectedCoreThread?.accountId ?? selectedCoreAccountId
        guard !accountId.isEmpty else { return }
        conversationHtmlOverrides[accountId] = preferHtml
    }

    private func isRssStarredItem(_ item: StarredItemSummary) -> Bool {
        MailStateKt.threadIdIsRss(threadId: item.threadId)
    }

    private func addAttachment(from url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            attachments.append(
                DraftAttachment(
                    id: url.absoluteString,
                    displayName: url.lastPathComponent,
                    mimeType: type,
                    sizeBytes: Int64(data.count),
                    dataBase64: data.base64EncodedString()
                )
            )
            attachmentError = nil
        } catch {
            attachmentError = error.localizedDescription
        }
    }

    private func addQuickReplyAttachment(from url: URL) {
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            quickReplyAttachments.append(
                DraftAttachment(
                    id: UUID().uuidString,
                    displayName: url.lastPathComponent,
                    mimeType: type,
                    sizeBytes: Int64(data.count),
                    dataBase64: data.base64EncodedString()
                )
            )
            attachmentError = nil
            quickReplyFailure = ""
        } catch {
            attachmentError = error.localizedDescription
        }
    }

    private func importAccountMedia(from url: URL) {
        guard let target = accountMediaImportTarget else {
            return
        }
        accountMediaImportTarget = nil
        let scoped = url.startAccessingSecurityScopedResource()
        defer {
            if scoped {
                url.stopAccessingSecurityScopedResource()
            }
        }

        do {
            let data = try Data(contentsOf: url)
            let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
            let params = AccountMediaFileParams(
                accountId: target.accountId,
                filename: url.lastPathComponent,
                mime: type,
                data: data.base64EncodedString()
            )
            let uploadRequest = target.isWallpaper
                ? MobileCommandsKt.accountWriteChatWallpaperFileRequest(id: 55, params: params).toJson()
                : MobileCommandsKt.accountWriteAvatarFileRequest(id: 55, params: params).toJson()
            let uploadResponse = RustCoreBridge.invokeJson(uploadRequest)
            if uploadResponse.contains(#""error""#) {
                accountStatus = "Media import failed."
                accountJson = uploadResponse
                return
            }
            let mediaUrl = MobileResponseParsersKt.parseMediaFileUrlResponse(responseJson: uploadResponse)
            if mediaUrl.isEmpty {
                accountStatus = "Media import failed: empty URL."
                return
            }
            let setRequest = target.isWallpaper
                ? MobileCommandsKt.accountSetChatWallpaperRequest(
                    id: 56,
                    params: AccountChatWallpaperParams(accountId: target.accountId, presetId: "", customUrl: mediaUrl)
                ).toJson()
                : MobileCommandsKt.accountSetAvatarRequest(
                    id: 56,
                    params: AccountAvatarParams(accountId: target.accountId, avatarUrl: mediaUrl)
                ).toJson()
            let setResponse = RustCoreBridge.invokeJson(setRequest)
            if setResponse.contains(#""error""#) {
                accountStatus = "Media import failed."
                accountJson = setResponse
                return
            }
            accountJson = RustCoreBridge.invokeJson(MobileCommandsKt.accountListRequest(id: 57).toJson())
            updateCoreAccounts(from: accountJson)
            accountStatus = target.isWallpaper ? "Updated chat wallpaper." : "Updated avatar."
        } catch {
            accountStatus = "Media import failed: \(error.localizedDescription)"
        }
    }
}

private struct HtmlMessageWebView: UIViewRepresentable {
    let html: String

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        let preferences = WKWebpagePreferences()
        preferences.allowsContentJavaScript = false
        configuration.defaultWebpagePreferences = preferences
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.isOpaque = false
        view.backgroundColor = .clear
        view.scrollView.backgroundColor = .clear
        return view
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        webView.loadHTMLString(html, baseURL: nil)
    }
}

private struct SenderAvatar: View {
    let label: String
    let enabled: Bool
    let size: CGFloat
    @State private var urlIndex = 0

    var body: some View {
        let urls = enabled ? senderImageUrls(label) : []
        Group {
            if urls.indices.contains(urlIndex) {
                AsyncImage(url: urls[urlIndex]) { phase in
                    switch phase {
                    case .success(let image):
                        image
                            .resizable()
                            .scaledToFill()
                    case .failure:
                        if urls.indices.contains(urlIndex + 1) {
                            initialsAvatar
                                .onAppear {
                                    urlIndex += 1
                                }
                        } else {
                            initialsAvatar
                        }
                    default:
                        initialsAvatar
                    }
                }
            } else {
                initialsAvatar
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .onChange(of: label) { _, _ in urlIndex = 0 }
        .onChange(of: enabled) { _, _ in urlIndex = 0 }
    }

    private var initialsAvatar: some View {
        ZStack {
            Circle()
                .fill(avatarColor(for: label))
            Text(avatarInitials(label))
                .font(.system(size: size * 0.34, weight: .semibold))
                .foregroundStyle(.white)
        }
    }
}

private func senderImageUrls(_ label: String) -> [URL] {
    guard let email = extractEmail(label),
          let domain = email.split(separator: "@").last,
          !domain.isEmpty else {
        return []
    }
    let hash = md5Hex(email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    return [
        URL(string: "https://www.gravatar.com/avatar/\(hash)?s=96&d=404"),
        URL(string: "https://www.google.com/s2/favicons?domain=\(domain)&sz=96")
    ].compactMap { $0 }
}

private func extractEmail(_ value: String) -> String? {
    let pattern = #"[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}"#
    return value.range(of: pattern, options: [.regularExpression, .caseInsensitive]).map { String(value[$0]) }
}

private func md5Hex(_ value: String) -> String {
    Insecure.MD5.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
}

private func threadDeleteActionLabel(_ thread: ThreadSummary) -> String {
    threadDeleteActionLabel(folder: thread.folder)
}

private func threadDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return "Discard Draft"
    }
    if MailStateKt.folderIsTrash(folder: folder) {
        return "Delete Forever"
    }
    return "Move to Trash"
}

private func avatarInitials(_ value: String) -> String {
    let parts = value
        .replacingOccurrences(of: "<", with: " ")
        .replacingOccurrences(of: ">", with: " ")
        .split { $0.isWhitespace || $0 == "@" || $0 == "." }
    let letters = parts.prefix(2).compactMap { $0.first }.map { String($0).uppercased() }
    return letters.isEmpty ? "?" : letters.joined()
}

private func avatarColor(for value: String) -> Color {
    let palette: [Color] = [.blue, .teal, .indigo, .purple, .pink, .green, .orange]
    return palette[abs(value.hashValue) % palette.count]
}

private func sendShortcutLabel(_ mode: String) -> String {
    mode == "enter" ? "Enter" : "Cmd/Ctrl+Enter"
}

private func sendShortcutMatches(_ press: KeyPress, mode: String) -> Bool {
    guard press.key == .return else { return false }
    if mode == "enter" {
        return !press.modifiers.contains(.shift) &&
            !press.modifiers.contains(.command) &&
            !press.modifiers.contains(.control)
    }
    return !press.modifiers.contains(.shift) &&
        (press.modifiers.contains(.command) || press.modifiers.contains(.control))
}

private struct ThreadRow<Actions: View>: View {
    let thread: ThreadSummary
    let showSenderImages: Bool
    let onOpen: () -> Void
    @ViewBuilder let actions: () -> Actions

    var body: some View {
        Button(action: onOpen) {
            HStack(alignment: .top, spacing: 10) {
                SenderAvatar(label: thread.sender.isEmpty ? thread.accountId : thread.sender, enabled: showSenderImages, size: 40)
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Text(thread.sender.isEmpty ? thread.accountId : thread.sender)
                            .font(.subheadline.weight(thread.unread ? .semibold : .regular))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        Spacer()
                        if thread.unread {
                            Text("Unread")
                                .font(.caption2.weight(.bold))
                                .foregroundStyle(.tint)
                        }
                        if thread.starred {
                            Image(systemName: "star.fill")
                                .font(.caption)
                                .foregroundStyle(.yellow)
                        }
                    }
                    Text(thread.subject.isEmpty ? "(no subject)" : thread.subject)
                        .font(.headline)
                        .foregroundStyle(.primary)
                        .lineLimit(2)
                    if !thread.preview.isEmpty {
                        Text(thread.preview)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                    }
                    Text(thread.folder)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            actions()
        }
    }
}

private struct ConversationMessageRow: View {
    let message: MessageBody
    let activeSearchMatch: Bool
    let renderHtml: Bool
    let canComposeFromMessage: Bool
    let onOpenAttachment: (MessageAttachment) -> Void
    let onCopy: (String, String) -> Void
    let onForward: () -> Void
    let onEditAsNew: () -> Void
    let onToggleRead: () -> Void
    let onToggleStarred: () -> Void
    let onDelete: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(message.subject.isEmpty ? "(no subject)" : message.subject)
                .font(.headline)
            Text(message.from)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            if renderHtml {
                HtmlMessageWebView(html: message.bodyHtml)
                    .frame(minHeight: 220)
            } else {
                Text(message.body.isEmpty ? "(no content)" : message.body)
                    .font(.body)
                    .foregroundStyle(message.body.isEmpty ? .secondary : .primary)
                    .textSelection(.enabled)
            }
            if !message.attachments.isEmpty {
                ForEach(Array(message.attachments.enumerated()), id: \.offset) { _, attachment in
                    Button {
                        onOpenAttachment(attachment)
                    } label: {
                        Label {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(attachment.filename.isEmpty ? "Attachment" : attachment.filename)
                                    .lineLimit(1)
                                Text(attachmentDetail(attachment))
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                                Text(attachmentActionHint(attachment))
                                    .font(.caption2)
                                    .foregroundStyle(.tint)
                                    .lineLimit(1)
                            }
                        } icon: {
                            Image(systemName: attachmentActionIcon(attachment))
                        }
                    }
                    .buttonStyle(.bordered)
                }
            }
            Menu {
                Button {
                    onCopy("Message text", messagePlainText(message))
                } label: {
                    Label("Copy Message Text", systemImage: "doc.on.doc")
                }
                Button {
                    onCopy("Subject", message.subject.isEmpty ? "(no subject)" : message.subject)
                } label: {
                    Label("Copy Subject", systemImage: "text.quote")
                }
                if !message.messageId.isEmpty {
                    Button {
                        onCopy("Message ID", message.messageId)
                    } label: {
                        Label("Copy Message ID", systemImage: "number")
                    }
                }
                if canComposeFromMessage {
                    Divider()
                    Button {
                        onToggleRead()
                    } label: {
                        Label(message.unread ? "Mark Read" : "Mark Unread", systemImage: message.unread ? "envelope.open" : "envelope.badge")
                    }
                    Button {
                        onToggleStarred()
                    } label: {
                        Label(message.starred ? "Unstar" : "Star", systemImage: message.starred ? "star.slash" : "star")
                    }
                    Button {
                        onForward()
                    } label: {
                        Label("Forward", systemImage: "arrowshape.turn.up.forward")
                    }
                    Button {
                        onEditAsNew()
                    } label: {
                        Label("Edit as New", systemImage: "doc.on.doc")
                    }
                    Button(role: .destructive) {
                        onDelete()
                    } label: {
                        Label("Delete Message", systemImage: "trash")
                    }
                }
            } label: {
                Label("More", systemImage: "ellipsis.circle")
                    .font(.caption)
            }
        }
        .padding(.vertical, 4)
        .padding(.horizontal, activeSearchMatch ? 8 : 0)
        .background(activeSearchMatch ? Color.yellow.opacity(0.18) : Color.clear)
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(activeSearchMatch ? Color.yellow.opacity(0.75) : Color.clear, lineWidth: 2)
        )
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct ConversationDetailsDisclosure: View {
    let participants: [ConversationParticipant]
    let attachments: [MessageAttachment]
    let onCopyEmail: (String) -> Void
    let onComposeTo: (ConversationParticipant) -> Void
    let onOpenAttachment: (MessageAttachment) -> Void

    private var mediaCount: Int {
        attachments.filter { $0.mimeType.hasPrefix("image/") || $0.mimeType.hasPrefix("video/") }.count
    }

    private var fileCount: Int {
        attachments.count - mediaCount
    }

    var body: some View {
        DisclosureGroup {
            HStack {
                ConversationDetailStat(label: "People", value: "\(participants.count)")
                ConversationDetailStat(label: "Media", value: "\(mediaCount)")
                ConversationDetailStat(label: "Files", value: "\(fileCount)")
            }
            .padding(.vertical, 4)

            if participants.isEmpty {
                Text("No conversation participants.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(participants) { person in
                    HStack(spacing: 10) {
                        Circle()
                            .fill(Color.accentColor.opacity(0.18))
                            .frame(width: 34, height: 34)
                            .overlay(
                                Text(initials(for: person.name))
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(.tint)
                            )
                        VStack(alignment: .leading, spacing: 2) {
                            Text(person.name.isEmpty ? person.email : person.name)
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(1)
                            Text("\(person.email) · \(person.count)\(person.isSelf ? " · you" : "")")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                        Spacer()
                        Menu {
                            Button("Copy Email") {
                                onCopyEmail(person.email)
                            }
                            if !person.isSelf {
                                Button("Compose") {
                                    onComposeTo(person)
                                }
                            }
                        } label: {
                            Image(systemName: "ellipsis.circle")
                        }
                    }
                    .padding(.vertical, 3)
                }
            }

            Divider()

            if attachments.isEmpty {
                Text("No shared files in loaded messages.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(Array(attachments.enumerated()), id: \.offset) { _, attachment in
                    Button {
                        onOpenAttachment(attachment)
                    } label: {
                        Label {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(attachment.filename.isEmpty ? "Attachment" : attachment.filename)
                                    .lineLimit(1)
                                Text(attachmentDetail(attachment))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                                Text(attachmentActionHint(attachment))
                                    .font(.caption2)
                                    .foregroundStyle(.tint)
                                    .lineLimit(1)
                            }
                        } icon: {
                            Image(systemName: attachmentActionIcon(attachment))
                        }
                    }
                    .buttonStyle(.plain)
                    .padding(.vertical, 3)
                }
            }
        } label: {
            Label("Details", systemImage: "info.circle")
        }
    }

    private func initials(for value: String) -> String {
        let words = value
            .split(whereSeparator: { $0.isWhitespace || $0 == "@" || $0 == "." })
            .prefix(2)
            .compactMap(\.first)
        let text = String(words).uppercased()
        return text.isEmpty ? "?" : text
    }
}

private struct ConversationDetailStat: View {
    let label: String
    let value: String

    var body: some View {
        VStack(spacing: 2) {
            Text(value)
                .font(.headline)
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct StarredItemRow: View {
    let item: StarredItemSummary
    let showSenderImages: Bool
    let onOpen: () -> Void
    let onToggleRead: () -> Void
    let onUnstar: () -> Void
    let onDelete: () -> Void

    private var isRssItem: Bool {
        MailStateKt.threadIdIsRss(threadId: item.threadId)
    }

    private var deleteLabel: String {
        threadDeleteActionLabel(folder: item.folder)
    }

    var body: some View {
        Button(action: onOpen) {
            HStack(alignment: .top, spacing: 10) {
                SenderAvatar(label: item.sender.isEmpty ? item.accountId : item.sender, enabled: showSenderImages, size: 38)
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Image(systemName: "star.fill")
                            .font(.caption)
                            .foregroundStyle(.yellow)
                        Text(item.sender.isEmpty ? item.accountId : item.sender)
                            .font(.subheadline.weight(item.unread ? .semibold : .regular))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        Spacer()
                        Menu {
                            Button(item.unread ? "Mark Read" : "Mark Unread", action: onToggleRead)
                            Button("Unstar", action: onUnstar)
                            if !isRssItem {
                                Button(deleteLabel, role: .destructive, action: onDelete)
                            }
                        } label: {
                            Image(systemName: "ellipsis.circle")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.borderless)
                        Text(relativeTime(item.dateEpochSeconds))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                    Text(item.subject.isEmpty ? "(no subject)" : item.subject)
                        .font(.headline)
                        .foregroundStyle(.primary)
                        .lineLimit(2)
                    if !item.preview.isEmpty {
                        Text(item.preview)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(2)
                    }
                    Text([item.accountId, item.folder].filter { !$0.isEmpty }.joined(separator: " / "))
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            if !isRssItem {
                Button(deleteLabel, role: .destructive, action: onDelete)
            }
            Button("Unstar", action: onUnstar)
                .tint(.orange)
            Button(item.unread ? "Read" : "Unread", action: onToggleRead)
                .tint(.blue)
        }
    }
}

private struct KanbanColumnView: View {
    let title: String
    let status: String
    let threads: [ThreadSummary]
    let canLoadMore: Bool
    let isLoadingMore: Bool
    let moveTargets: [IosKanbanColumnSpec]
    let targetTitle: (IosKanbanColumnSpec) -> String
    let onRefresh: () -> Void
    let onLoadMore: () -> Void
    let onMarkAllRead: () -> Void
    let onRemoveColumn: () -> Void
    let onOpen: (ThreadSummary) -> Void
    let onArchive: (ThreadSummary) -> Void
    let onDelete: (ThreadSummary) -> Void
    let onToggleRead: (ThreadSummary) -> Void
    let onToggleStar: (ThreadSummary) -> Void
    let onMove: (ThreadSummary, IosKanbanColumnSpec) -> Void
    let showSenderImages: Bool
    let columnWidth: CGFloat

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.headline)
                        .lineLimit(2)
                    if !status.isEmpty {
                        Text(status)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
                Menu {
                    Button("Refresh", action: onRefresh)
                    Button("Mark All Read", action: onMarkAllRead)
                        .disabled(!threads.contains { $0.unread })
                    Button("Remove Column", role: .destructive, action: onRemoveColumn)
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }

            if threads.isEmpty {
                Text("No cached items.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 12)
            } else {
                ForEach(threads, id: \.id) { thread in
                    KanbanThreadCard(
                        thread: thread,
                        moveTargets: moveTargets,
                        targetTitle: targetTitle,
                        showSenderImages: showSenderImages,
                        onOpen: { onOpen(thread) },
                        onArchive: { onArchive(thread) },
                        onDelete: { onDelete(thread) },
                        onToggleRead: { onToggleRead(thread) },
                        onToggleStar: { onToggleStar(thread) },
                        onMove: { target in onMove(thread, target) }
                    )
                }
                if canLoadMore || isLoadingMore {
                    Button {
                        onLoadMore()
                    } label: {
                        if isLoadingMore {
                            Label("Loading Older", systemImage: "hourglass")
                        } else {
                            Label("Load Older", systemImage: "chevron.down")
                        }
                    }
                    .disabled(isLoadingMore)
                    .buttonStyle(.bordered)
                }
            }
        }
        .padding(12)
        .frame(width: columnWidth, alignment: .topLeading)
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private struct KanbanBoardStylePreview: View {
    let board: IosKanbanBoardSpec?

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            RoundedRectangle(cornerRadius: 8)
                .fill(wallpaperColor)
                .frame(width: 48, height: 48)
                .overlay {
                    if let url = board?.wallpaperUrl?.imageURL {
                        AsyncImage(url: url) { image in
                            image
                                .resizable()
                                .scaledToFill()
                        } placeholder: {
                            Image(systemName: "rectangle.3.group")
                                .foregroundStyle(.secondary)
                        }
                    } else {
                        Image(systemName: "rectangle.3.group")
                            .foregroundStyle(.secondary)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 8))

            avatar
                .frame(width: 24, height: 24)
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color(.systemBackground), lineWidth: 2))
                .offset(x: 4, y: 4)
        }
        .frame(width: 54, height: 54)
    }

    private var avatar: some View {
        Group {
            if let url = board?.avatarUrl?.imageURL {
                AsyncImage(url: url) { image in
                    image
                        .resizable()
                        .scaledToFill()
                } placeholder: {
                    Image(systemName: "photo")
                        .font(.caption)
                        .foregroundStyle(.white)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(avatarColor)
                }
            } else {
                Image(systemName: "rectangle.3.group")
                    .font(.caption)
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(avatarColor)
            }
        }
    }

    private var wallpaperColor: Color {
        guard let value = board?.wallpaperUrl ?? board?.wallpaperPresetId, !value.isEmpty else {
            return Color(.tertiarySystemGroupedBackground)
        }
        return color(for: value).opacity(0.22)
    }

    private var avatarColor: Color {
        color(for: board?.avatarUrl ?? board?.name ?? "Kanban")
    }

    private func color(for value: String) -> Color {
        let palette: [Color] = [.blue, .teal, .indigo, .purple, .pink, .green, .orange]
        let index = abs(value.hashValue) % palette.count
        return palette[index]
    }
}

private struct KanbanThreadCard: View {
    let thread: ThreadSummary
    let moveTargets: [IosKanbanColumnSpec]
    let targetTitle: (IosKanbanColumnSpec) -> String
    let showSenderImages: Bool
    let onOpen: () -> Void
    let onArchive: () -> Void
    let onDelete: () -> Void
    let onToggleRead: () -> Void
    let onToggleStar: () -> Void
    let onMove: (IosKanbanColumnSpec) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button(action: onOpen) {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        SenderAvatar(label: thread.sender.isEmpty ? thread.accountId : thread.sender, enabled: showSenderImages, size: 26)
                        Text(thread.sender.isEmpty ? thread.accountId : thread.sender)
                            .font(.caption.weight(thread.unread ? .bold : .regular))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        Spacer()
                        if thread.starred {
                            Image(systemName: "star.fill")
                                .font(.caption)
                                .foregroundStyle(.yellow)
                        }
                    }
                    Text(thread.subject.isEmpty ? "(no subject)" : thread.subject)
                        .font(.subheadline.weight(thread.unread ? .semibold : .regular))
                        .foregroundStyle(.primary)
                        .lineLimit(3)
                    if !thread.preview.isEmpty {
                        Text(thread.preview)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(3)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)

            HStack {
                Button(action: onToggleRead) {
                    Image(systemName: thread.unread ? "envelope.open" : "envelope.badge")
                }
                Button(action: onToggleStar) {
                    Image(systemName: thread.starred ? "star.slash" : "star")
                }
                Menu {
                    Button(thread.id.hasPrefix("rss:") ? "Remove Feed" : "Archive", action: onArchive)
                    Button(threadDeleteActionLabel(thread), role: .destructive, action: onDelete)
                    if !moveTargets.isEmpty {
                        Divider()
                        ForEach(moveTargets) { target in
                            Button(targetTitle(target)) {
                                onMove(target)
                            }
                        }
                    }
                } label: {
                    Image(systemName: "ellipsis")
                }
                Spacer()
            }
            .buttonStyle(.borderless)
        }
        .padding(10)
        .background(Color(.systemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}

private func loadIosKanbanBoards() -> [IosKanbanBoardSpec] {
    guard let data = UserDefaults.standard.data(forKey: "ios_kanban_boards_v1"),
          let boards = try? JSONDecoder().decode([IosKanbanBoardSpec].self, from: data) else {
        return []
    }
    return boards
}

private func relativeTime(_ epochSeconds: Int64) -> String {
    guard epochSeconds > 0 else { return "" }
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .abbreviated
    return formatter.localizedString(for: Date(timeIntervalSince1970: TimeInterval(epochSeconds)), relativeTo: Date())
}

private func attachmentDetail(_ attachment: MessageAttachment) -> String {
    let size = attachment.sizeBytes > 0 ? ByteCountFormatter.string(fromByteCount: attachment.sizeBytes, countStyle: .file) : ""
    return [attachment.mimeType, size].filter { !$0.isEmpty }.joined(separator: " · ")
}

private func attachmentActionHint(_ attachment: MessageAttachment) -> String {
    if attachment.url.isEmpty && attachment.key.isEmpty {
        return "Not cached"
    }
    if attachment.url.isEmpty && !attachment.mimeType.hasPrefix("image/") {
        return "Share or save to Files"
    }
    if attachment.mimeType.hasPrefix("image/") {
        return "Preview, copy, share, or save"
    }
    return "Open externally"
}

private func attachmentActionIcon(_ attachment: MessageAttachment) -> String {
    if attachment.mimeType.hasPrefix("image/") {
        return "photo"
    }
    if !attachment.url.isEmpty {
        return "arrow.up.forward.app"
    }
    return "square.and.arrow.up"
}

private func messagePlainText(_ message: MessageBody) -> String {
    if !message.body.isEmpty {
        return message.body
    }
    let text = message.bodyHtml
        .replacingOccurrences(of: "<br\\s*/?>", with: "\n", options: .regularExpression)
        .replacingOccurrences(of: "</p>", with: "\n", options: [.regularExpression, .caseInsensitive])
        .replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression)
        .replacingOccurrences(of: "[ \\t]+", with: " ", options: .regularExpression)
        .trimmingCharacters(in: .whitespacesAndNewlines)
    return text.isEmpty ? "(no content)" : text
}

private func safeAttachmentFilename(_ name: String) -> String {
    let fallback = name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "attachment" : name
    let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: ".-_"))
    let cleaned = fallback.unicodeScalars.map { scalar in
        allowed.contains(scalar) ? Character(scalar) : "_"
    }
    let value = String(cleaned).trimmingCharacters(in: CharacterSet(charactersIn: "._"))
    return value.isEmpty ? "attachment" : value
}

private func formattedStorageBytes(_ bytes: Int64?) -> String {
    guard let bytes else { return "Not loaded" }
    return ByteCountFormatter.string(fromByteCount: bytes, countStyle: .file)
}

func iosPreferredColorScheme(_ mode: String) -> ColorScheme? {
    guard let dark = iosThemeOption(mode).dark else { return nil }
    return dark ? .dark : .light
}

func iosThemeTint(_ mode: String) -> Color {
    iosThemeOption(mode).accent
}

private struct DiagnosticText: View {
    let title: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(value)
                .font(.system(.footnote, design: .monospaced))
                .textSelection(.enabled)
        }
    }
}

func applyMailtoDraftToCompose(
    _ draft: ComposeDraft,
    to: inout String,
    cc: inout String,
    bcc: inout String,
    subject: inout String,
    body: inout String
) {
    to = draft.to
    cc = draft.cc
    bcc = draft.bcc
    subject = draft.subject
    body = draft.body
}

func applyOAuthCallbackToComposeState(
    rawUrl: String,
    expectedState: String,
    redirectUri: String = OAuthFlowKt.defaultOAuthRedirectUri(),
    authorizationCode: inout String
) -> String {
    if let callback = OAuthFlowKt.parseOAuthCallbackUrlForRedirectOrNull(
        rawUrl: rawUrl,
        expectedState: expectedState,
        redirectUri: redirectUri
    ) {
        authorizationCode = callback.code
        return "OAuth authorization code received; use Exchange Code And Add Account."
    }

    let error = OAuthFlowKt.oauthCallbackValidationErrorForRedirect(
        rawUrl: rawUrl,
        expectedState: expectedState,
        redirectUri: redirectUri
    ) ?? "OAuth callback failed."
    return "OAuth callback failed: \(error)"
}

private func pkceChallenge(_ verifier: String) -> String {
    let digest = SHA256.hash(data: Data(verifier.utf8))
    return Data(digest)
        .base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}

#Preview {
    ContentView()
}
