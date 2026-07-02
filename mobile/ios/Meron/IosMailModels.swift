import CryptoKit
import MeronUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

let iosUnifiedAccountId = "unified"
let iosInboxFolderId = "inbox"
let iosStarredFolderId = "starred"
let iosAppearanceModeKey = "ios_appearance_mode_v1"
let iosShowUnifiedInboxKey = "ios_show_unified_inbox_v1"
let iosShowStarredTabKey = "ios_show_starred_tab_v1"
let iosShowUnreadBadgesKey = "ios_show_unread_badges_v1"
let iosShowSenderImagesKey = "ios_show_sender_images_v1"
let iosSendShortcutKey = "ios_send_shortcut_v1"
let iosAppLanguageTagKey = "ios_app_language_tag_v1"
let iosHiddenNavigationAccountsKey = "ios_hidden_navigation_accounts_v1"
let iosKanbanColumnWidthKey = "ios_kanban_column_width_v1"
let iosLiveMailPushKey = "ios_live_mail_push_v1"
let iosPendingOAuthFlowKey = "ios_pending_oauth_flow_v1"
let iosDefaultGoogleOAuthClientId = "247266375404-5cdrkea888lsttjh65fjtp3504h45r2k.apps.googleusercontent.com"
let iosKanbanColumnMinWidth = 240.0
let iosKanbanColumnDefaultWidth = 310.0
let iosKanbanColumnMaxWidth = 520.0

enum IosThreadUndoAction {
    case moveToOriginalFolder
    case markRead(seen: Bool)
    case markStarred(starred: Bool)
}

struct IosThreadUndo {
    let id = UUID()
    let message: String
    let thread: ThreadSummary
    let action: IosThreadUndoAction
    let threadsSnapshot: [ThreadSummary]
    let selectedThreadIdsSnapshot: Set<String>
    let selectedThreadSnapshot: ThreadSummary?
    let messagesSnapshot: [MessageBody]
    let messageCursorSnapshot: String
    let kanbanThreadsSnapshot: [String: [ThreadSummary]]
}

struct IosPendingOAuthFlow: Codable, Equatable {
    let provider: String
    let state: String
    let verifier: String
    let redirectUri: String
    let email: String
}

struct IosThemeOption: Identifiable {
    let id: String
    let nameKey: String
    let dark: Bool?
    let accent: Color

    var name: String {
        String(localized: String.LocalizationValue(nameKey))
    }
}

struct IosAboutSupportLink: Identifiable, Equatable {
    let id: String
    let titleKey: String
    let url: String
    let systemImage: String
}

let iosAboutSupportLinks = [
    IosAboutSupportLink(
        id: "github-sponsors",
        titleKey: "about.githubSponsors",
        url: "https://github.com/sponsors/nonbili",
        systemImage: "heart"
    ),
    IosAboutSupportLink(
        id: "liberapay",
        titleKey: "about.liberapay",
        url: "https://liberapay.com/nonbili",
        systemImage: "heart.circle"
    ),
    IosAboutSupportLink(
        id: "paypal",
        titleKey: "about.paypal",
        url: "https://www.paypal.com/paypalme/nonbili",
        systemImage: "creditcard"
    ),
]

func iosColor(_ rgb: UInt32) -> Color {
    Color(
        red: Double((rgb >> 16) & 0xFF) / 255.0,
        green: Double((rgb >> 8) & 0xFF) / 255.0,
        blue: Double(rgb & 0xFF) / 255.0
    )
}

let iosThemeOptions: [IosThemeOption] = [
    IosThemeOption(id: "system", nameKey: "settings.language.system", dark: nil, accent: iosColor(0x4F46E5)),
    IosThemeOption(id: "indigo", nameKey: "mobile.ios.themeIndigo", dark: false, accent: iosColor(0x4F46E5)),
    IosThemeOption(id: "indigo-dark", nameKey: "mobile.ios.themeIndigoDark", dark: true, accent: iosColor(0x6366F1)),
    IosThemeOption(id: "light", nameKey: "mobile.ios.themeMeronLight", dark: false, accent: iosColor(0x0E7A58)),
    IosThemeOption(id: "dark", nameKey: "mobile.ios.themeMeronDark", dark: true, accent: iosColor(0x36B489)),
    IosThemeOption(id: "mist", nameKey: "mobile.ios.themeMist", dark: false, accent: iosColor(0x0EA5B7)),
    IosThemeOption(id: "paper", nameKey: "mobile.ios.themePaper", dark: false, accent: iosColor(0x64748B)),
    IosThemeOption(id: "dawn", nameKey: "mobile.ios.themeDawn", dark: false, accent: iosColor(0xC06C84)),
    IosThemeOption(id: "honey", nameKey: "mobile.ios.themeHoney", dark: false, accent: iosColor(0xB07C10)),
    IosThemeOption(id: "lilac", nameKey: "mobile.ios.themeLilac", dark: false, accent: iosColor(0x7A5BC4)),
    IosThemeOption(id: "graphite", nameKey: "mobile.ios.themeGraphite", dark: true, accent: iosColor(0x8B9BB4)),
    IosThemeOption(id: "midnight", nameKey: "mobile.ios.themeMidnight", dark: true, accent: iosColor(0x38BDF8)),
    IosThemeOption(id: "forest", nameKey: "mobile.ios.themeForest", dark: true, accent: iosColor(0x7CCF9B)),
    IosThemeOption(id: "plum", nameKey: "mobile.ios.themePlum", dark: true, accent: iosColor(0xB48AE0)),
    IosThemeOption(id: "ember", nameKey: "mobile.ios.themeEmber", dark: true, accent: iosColor(0xE1854C)),
]

struct IosAppLanguage: Identifiable, Hashable {
    let id: String
    let nativeName: String
}

struct IosWallpaperPreset: Identifiable, Hashable {
    let id: String
    let nameKey: String

    var name: String {
        String(localized: String.LocalizationValue(nameKey))
    }
}

let iosWallpaperPresets: [IosWallpaperPreset] = [
    IosWallpaperPreset(id: "", nameKey: "mobile.ios.wallpaperDefault"),
    IosWallpaperPreset(id: "plain", nameKey: "wallpaper.plain"),
    IosWallpaperPreset(id: "doodle", nameKey: "mobile.ios.wallpaperDoodle"),
    IosWallpaperPreset(id: "dots", nameKey: "mobile.ios.wallpaperLinearDots"),
    IosWallpaperPreset(id: "grid", nameKey: "mobile.ios.wallpaperClassicGrid"),
    IosWallpaperPreset(id: "stripes", nameKey: "mobile.ios.wallpaperDiagonalStripes"),
    IosWallpaperPreset(id: "hexagon", nameKey: "mobile.ios.wallpaperHexagonGrid"),
    IosWallpaperPreset(id: "isometric", nameKey: "mobile.ios.wallpaperIsometricCubes"),
    IosWallpaperPreset(id: "waves", nameKey: "mobile.ios.wallpaperFlowingWaves"),
    IosWallpaperPreset(id: "nordic", nameKey: "mobile.ios.wallpaperNordicPattern"),
    IosWallpaperPreset(id: "topography", nameKey: "mobile.ios.wallpaperTopography"),
    IosWallpaperPreset(id: "constellation", nameKey: "mobile.ios.wallpaperConstellation"),
    IosWallpaperPreset(id: "aurora", nameKey: "mobile.ios.wallpaperAurora"),
    IosWallpaperPreset(id: "nebula", nameKey: "mobile.ios.wallpaperNebula"),
    IosWallpaperPreset(id: "sunset", nameKey: "mobile.ios.wallpaperSunsetGlow"),
    IosWallpaperPreset(id: "forest", nameKey: "mobile.ios.wallpaperForestMist"),
    IosWallpaperPreset(id: "desert", nameKey: "mobile.ios.wallpaperDesertDunes"),
    IosWallpaperPreset(id: "ocean", nameKey: "mobile.ios.wallpaperTranquilOcean"),
    IosWallpaperPreset(id: "mountain", nameKey: "mobile.ios.wallpaperMountainRange"),
    IosWallpaperPreset(id: "breeze", nameKey: "mobile.ios.wallpaperSoftBreeze"),
    IosWallpaperPreset(id: "galaxy", nameKey: "mobile.ios.wallpaperSpiralGalaxy"),
    IosWallpaperPreset(id: "shapes", nameKey: "mobile.ios.wallpaperAbstractShapes"),
    IosWallpaperPreset(id: "sakura", nameKey: "mobile.ios.wallpaperSakuraWatercolor"),
    IosWallpaperPreset(id: "vintage", nameKey: "mobile.ios.wallpaperVintageParchment"),
    IosWallpaperPreset(id: "raindrops", nameKey: "mobile.ios.wallpaperRaindrops"),
    IosWallpaperPreset(id: "marble", nameKey: "mobile.ios.wallpaperSleekMarble"),
    IosWallpaperPreset(id: "cyberpunk", nameKey: "mobile.ios.wallpaperCyberpunkGrid"),
    IosWallpaperPreset(id: "matrix", nameKey: "mobile.ios.wallpaperDigitalMatrix"),
    IosWallpaperPreset(id: "autumn", nameKey: "mobile.ios.wallpaperAutumnLeaves"),
    IosWallpaperPreset(id: "nightsky", nameKey: "mobile.ios.wallpaperCelestialNight"),
]

let iosSupportedAppLanguages: [IosAppLanguage] = [
    IosAppLanguage(id: "ar", nativeName: "العربية"),
    IosAppLanguage(id: "de", nativeName: "Deutsch"),
    IosAppLanguage(id: "el", nativeName: "Ελληνικά"),
    IosAppLanguage(id: "en", nativeName: "English"),
    IosAppLanguage(id: "es", nativeName: "Español"),
    IosAppLanguage(id: "et", nativeName: "Eesti"),
    IosAppLanguage(id: "fr", nativeName: "Français"),
    IosAppLanguage(id: "it", nativeName: "Italiano"),
    IosAppLanguage(id: "ja", nativeName: "日本語"),
    IosAppLanguage(id: "ko", nativeName: "한국어"),
    IosAppLanguage(id: "lv", nativeName: "Latviešu"),
    IosAppLanguage(id: "pl", nativeName: "Polski"),
    IosAppLanguage(id: "pt", nativeName: "Português"),
    IosAppLanguage(id: "pt-BR", nativeName: "Português (Brasil)"),
    IosAppLanguage(id: "sv", nativeName: "Svenska"),
    IosAppLanguage(id: "tr", nativeName: "Türkçe"),
    IosAppLanguage(id: "vi", nativeName: "Tiếng Việt"),
    IosAppLanguage(id: "zh-Hans", nativeName: "简体中文"),
    IosAppLanguage(id: "zh-Hant", nativeName: "繁體中文"),
]

func iosNormalizedAppLanguageTag(_ tag: String) -> String {
    iosSupportedAppLanguages.contains { $0.id == tag } ? tag : ""
}

func iosAppLanguageDisplayName(_ tag: String) -> String {
    if tag.isEmpty {
        return String(localized: "settings.language.system")
    }
    return iosSupportedAppLanguages.first { $0.id == tag }?.nativeName ?? tag
}

func iosAppLocale(_ tag: String) -> Locale {
    let normalized = iosNormalizedAppLanguageTag(tag)
    return normalized.isEmpty ? .current : Locale(identifier: normalized)
}

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

struct IosKanbanDeleteTarget {
    let thread: ThreadSummary
    let column: IosKanbanColumnSpec
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
        name = (try? container.decode(String.self, forKey: .name)) ?? String(localized: "kanban.board.defaultName")
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
        case .all: String(localized: "filters.all")
        case .unread: String(localized: "filters.unread")
        case .starred: String(localized: "filters.starred")
        }
    }
}

enum IosMailFocusedField: Hashable {
    case surface
    case mailboxSearch
    case threadSearch
    case quickReply
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
            feedUrl: feedUrl,
            threadId: threadId
        )
    }

    func withFlags(unread nextUnread: Bool? = nil, starred nextStarred: Bool? = nil) -> ThreadSummary {
        ThreadSummary(
            id: id,
            accountId: accountId,
            folder: folder,
            subject: subject,
            sender: sender,
            preview: preview,
            unread: nextUnread ?? unread,
            starred: nextStarred ?? starred,
            dateEpochSeconds: dateEpochSeconds,
            feedUrl: feedUrl,
            threadId: threadId
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

struct AttachmentDocument: FileDocument {
    static var readableContentTypes: [UTType] {
        [.data]
    }

    var data: Data

    init(data: Data = Data()) {
        self.data = data
    }

    init(configuration: ReadConfiguration) throws {
        data = configuration.file.regularFileContents ?? Data()
    }

    func fileWrapper(configuration _: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
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
            folderId: folderId,
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
            bodyMissing: bodyMissing,
            attachments: attachments,
            sendStatus: sendStatus
        )
    }
}

struct AccountSettingsDraft {
    var displayName: String
    var senderName: String
    var avatarUrl: String
    var wallpaperPresetId: String
    var wallpaperUrl: String
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

struct KanbanBoardMediaImportTarget {
    let boardId: String
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
