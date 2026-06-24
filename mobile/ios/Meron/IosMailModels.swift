import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

let iosUnifiedAccountId = "unified"
let iosInboxFolderId = "inbox"
let iosAppearanceModeKey = "ios_appearance_mode_v1"
let iosShowUnifiedInboxKey = "ios_show_unified_inbox_v1"
let iosShowStarredTabKey = "ios_show_starred_tab_v1"
let iosShowUnreadBadgesKey = "ios_show_unread_badges_v1"
let iosShowSenderImagesKey = "ios_show_sender_images_v1"
let iosSendShortcutKey = "ios_send_shortcut_v1"
let iosHiddenNavigationAccountsKey = "ios_hidden_navigation_accounts_v1"
let iosKanbanColumnWidthKey = "ios_kanban_column_width_v1"
let iosKanbanColumnMinWidth = 240.0
let iosKanbanColumnDefaultWidth = 310.0
let iosKanbanColumnMaxWidth = 520.0

struct IosThemeOption: Identifiable {
    let id: String
    let name: String
    let dark: Bool?
    let accent: Color
}

func iosColor(_ rgb: UInt32) -> Color {
    Color(
        red: Double((rgb >> 16) & 0xFF) / 255.0,
        green: Double((rgb >> 8) & 0xFF) / 255.0,
        blue: Double(rgb & 0xFF) / 255.0
    )
}

let iosThemeOptions: [IosThemeOption] = [
    IosThemeOption(id: "system", name: "System", dark: nil, accent: iosColor(0x4F46E5)),
    IosThemeOption(id: "indigo", name: "Indigo", dark: false, accent: iosColor(0x4F46E5)),
    IosThemeOption(id: "indigo-dark", name: "Indigo Dark", dark: true, accent: iosColor(0x6366F1)),
    IosThemeOption(id: "light", name: "Meron Light", dark: false, accent: iosColor(0x0E7A58)),
    IosThemeOption(id: "dark", name: "Meron Dark", dark: true, accent: iosColor(0x36B489)),
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

struct IosKanbanColumnSpec: Codable, Identifiable, Hashable {
    let id: String
    var accountId: String
    var folderId: String

    init(accountId: String, folderId: String) {
        self.accountId = accountId
        self.folderId = folderId
        id = "\(accountId)::\(folderId)"
    }
}

struct IosKanbanBoardSpec: Codable, Identifiable, Hashable {
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

    enum CodingKeys: String, CodingKey {
        case id
        case name
        case columns
        case avatarUrl
        case wallpaper
    }

    enum WallpaperKeys: String, CodingKey {
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

enum IosFilterMode: String, CaseIterable, Identifiable {
    case all
    case unread
    case starred

    var id: String {
        rawValue
    }

    var label: String {
        switch self {
        case .all: "All"
        case .unread: "Unread"
        case .starred: "Starred"
        }
    }
}

enum IosAppTab {
    case mail
    case compose
    case starred
    case kanban
    case accounts
}

extension String {
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

extension ThreadSummary {
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

func identityKey(_ identity: SendIdentity) -> String {
    "\(identity.accountId)|\(identity.email)"
}

struct OpmlDocument: FileDocument {
    static var readableContentTypes: [UTType] {
        [.xml]
    }

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

    func fileWrapper(configuration _: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

extension FolderSummary {
    var accountIdAndName: String {
        "\(accountId)::\(name)"
    }
}

extension MessageBody {
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

struct AccountSettingsDraft {
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

struct AccountMediaImportTarget {
    let accountId: String
    let isWallpaper: Bool
}

struct ShareableFile: Identifiable {
    let id = UUID()
    let url: URL
}

struct IosImagePreview: Identifiable {
    let id = UUID()
    let title: String
    let image: UIImage
    let url: URL
}

struct ConversationParticipant: Identifiable, Hashable {
    var id: String {
        email
    }

    let name: String
    let email: String
    let count: Int
    let isSelf: Bool
}
