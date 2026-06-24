import CryptoKit
import MeronShared
import SwiftUI
import UIKit
import UniformTypeIdentifiers
import WebKit

struct ShareSheet: UIViewControllerRepresentable {
    let url: URL

    func makeUIViewController(context _: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [url], applicationActivities: nil)
    }

    func updateUIViewController(_: UIActivityViewController, context _: Context) {}
}

struct ImagePreviewSheet: View {
    let preview: IosImagePreview
    @Environment(\.dismiss) var dismiss

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

struct AccountSettingsEditor: View {
    let account: AccountSummary
    let isRss: Bool
    let onSave: (AccountSettingsDraft) -> Void
    let onPickAvatar: () -> Void
    let onPickWallpaper: () -> Void
    let onRemove: () -> Void
    let showInNavigation: Bool

    @State var displayName: String
    @State var senderName: String
    @State var avatarUrl: String
    @State var wallpaperPresetId: String
    @State var loadRemoteImages: Bool
    @State var conversationHtml: Bool
    @State var includedInUnified: Bool
    @State var visibleInNavigation: Bool
    @State var muted: Bool
    @State var paused: Bool
    @State var intervalText: String
    @State var aliasesText: String

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
                    .lineLimit(2 ... 5)
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

struct HtmlMessageWebView: UIViewRepresentable {
    let html: String

    func makeUIView(context _: Context) -> WKWebView {
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

    func updateUIView(_ webView: WKWebView, context _: Context) {
        webView.loadHTMLString(html, baseURL: nil)
    }
}

struct SenderAvatar: View {
    let label: String
    let enabled: Bool
    let size: CGFloat
    @State var urlIndex = 0

    var body: some View {
        let urls = enabled ? senderImageUrls(label) : []
        Group {
            if urls.indices.contains(urlIndex) {
                AsyncImage(url: urls[urlIndex]) { phase in
                    switch phase {
                    case let .success(image):
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

    var initialsAvatar: some View {
        ZStack {
            Circle()
                .fill(avatarColor(for: label))
            Text(avatarInitials(label))
                .font(.system(size: size * 0.34, weight: .semibold))
                .foregroundStyle(.white)
        }
    }
}

func senderImageUrls(_ label: String) -> [URL] {
    guard let email = extractEmail(label),
          let domain = email.split(separator: "@").last,
          !domain.isEmpty
    else {
        return []
    }
    let hash = md5Hex(email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())
    return [
        URL(string: "https://www.gravatar.com/avatar/\(hash)?s=96&d=404"),
        URL(string: "https://www.google.com/s2/favicons?domain=\(domain)&sz=96"),
    ].compactMap { $0 }
}

func extractEmail(_ value: String) -> String? {
    let pattern = #"[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}"#
    return value.range(of: pattern, options: [.regularExpression, .caseInsensitive]).map { String(value[$0]) }
}

func md5Hex(_ value: String) -> String {
    Insecure.MD5.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
}

func threadDeleteActionLabel(_ thread: ThreadSummary) -> String {
    threadDeleteActionLabel(folder: thread.folder)
}

func threadDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return "Discard Draft"
    }
    if MailStateKt.folderIsTrash(folder: folder) {
        return "Delete Forever"
    }
    return "Move to Trash"
}

func avatarInitials(_ value: String) -> String {
    let parts = value
        .replacingOccurrences(of: "<", with: " ")
        .replacingOccurrences(of: ">", with: " ")
        .split { $0.isWhitespace || $0 == "@" || $0 == "." }
    let letters = parts.prefix(2).compactMap(\.first).map { String($0).uppercased() }
    return letters.isEmpty ? "?" : letters.joined()
}

func avatarColor(for value: String) -> Color {
    let palette: [Color] = [.blue, .teal, .indigo, .purple, .pink, .green, .orange]
    return palette[abs(value.hashValue) % palette.count]
}

func sendShortcutLabel(_ mode: String) -> String {
    mode == "enter" ? "Enter" : "Cmd/Ctrl+Enter"
}

func sendShortcutMatches(_ press: KeyPress, mode: String) -> Bool {
    guard press.key == .return else { return false }
    if mode == "enter" {
        return !press.modifiers.contains(.shift) &&
            !press.modifiers.contains(.command) &&
            !press.modifiers.contains(.control)
    }
    return !press.modifiers.contains(.shift) &&
        (press.modifiers.contains(.command) || press.modifiers.contains(.control))
}

struct ThreadRow<Actions: View>: View {
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

struct ConversationMessageRow: View {
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

struct ConversationDetailsDisclosure: View {
    let participants: [ConversationParticipant]
    let attachments: [MessageAttachment]
    let onCopyEmail: (String) -> Void
    let onComposeTo: (ConversationParticipant) -> Void
    let onOpenAttachment: (MessageAttachment) -> Void

    var mediaCount: Int {
        attachments.filter { $0.mimeType.hasPrefix("image/") || $0.mimeType.hasPrefix("video/") }.count
    }

    var fileCount: Int {
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

    func initials(for value: String) -> String {
        let words = value
            .split(whereSeparator: { $0.isWhitespace || $0 == "@" || $0 == "." })
            .prefix(2)
            .compactMap(\.first)
        let text = String(words).uppercased()
        return text.isEmpty ? "?" : text
    }
}

struct ConversationDetailStat: View {
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

struct StarredItemRow: View {
    let item: StarredItemSummary
    let showSenderImages: Bool
    let onOpen: () -> Void
    let onToggleRead: () -> Void
    let onUnstar: () -> Void
    let onDelete: () -> Void

    var isRssItem: Bool {
        MailStateKt.threadIdIsRss(threadId: item.threadId)
    }

    var deleteLabel: String {
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

struct KanbanColumnView: View {
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

struct KanbanBoardStylePreview: View {
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

    var avatar: some View {
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

    var wallpaperColor: Color {
        guard let value = board?.wallpaperUrl ?? board?.wallpaperPresetId, !value.isEmpty else {
            return Color(.tertiarySystemGroupedBackground)
        }
        return color(for: value).opacity(0.22)
    }

    var avatarColor: Color {
        color(for: board?.avatarUrl ?? board?.name ?? "Kanban")
    }

    func color(for value: String) -> Color {
        let palette: [Color] = [.blue, .teal, .indigo, .purple, .pink, .green, .orange]
        let index = abs(value.hashValue) % palette.count
        return palette[index]
    }
}

struct KanbanThreadCard: View {
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

func loadIosKanbanBoards() -> [IosKanbanBoardSpec] {
    guard let data = UserDefaults.standard.data(forKey: "ios_kanban_boards_v1"),
          let boards = try? JSONDecoder().decode([IosKanbanBoardSpec].self, from: data)
    else {
        return []
    }
    return boards
}

func relativeTime(_ epochSeconds: Int64) -> String {
    guard epochSeconds > 0 else { return "" }
    let formatter = RelativeDateTimeFormatter()
    formatter.unitsStyle = .abbreviated
    return formatter.localizedString(for: Date(timeIntervalSince1970: TimeInterval(epochSeconds)), relativeTo: Date())
}

func attachmentDetail(_ attachment: MessageAttachment) -> String {
    let size = attachment.sizeBytes > 0 ? ByteCountFormatter.string(fromByteCount: attachment.sizeBytes, countStyle: .file) : ""
    return [attachment.mimeType, size].filter { !$0.isEmpty }.joined(separator: " · ")
}

func attachmentActionHint(_ attachment: MessageAttachment) -> String {
    if attachment.url.isEmpty, attachment.key.isEmpty {
        return "Not cached"
    }
    if attachment.url.isEmpty, !attachment.mimeType.hasPrefix("image/") {
        return "Share or save to Files"
    }
    if attachment.mimeType.hasPrefix("image/") {
        return "Preview, copy, share, or save"
    }
    return "Open externally"
}

func attachmentActionIcon(_ attachment: MessageAttachment) -> String {
    if attachment.mimeType.hasPrefix("image/") {
        return "photo"
    }
    if !attachment.url.isEmpty {
        return "arrow.up.forward.app"
    }
    return "square.and.arrow.up"
}

func messagePlainText(_ message: MessageBody) -> String {
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

func safeAttachmentFilename(_ name: String) -> String {
    let fallback = name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? "attachment" : name
    let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: ".-_"))
    let cleaned = fallback.unicodeScalars.map { scalar in
        allowed.contains(scalar) ? Character(scalar) : "_"
    }
    let value = String(cleaned).trimmingCharacters(in: CharacterSet(charactersIn: "._"))
    return value.isEmpty ? "attachment" : value
}

func formattedStorageBytes(_ bytes: Int64?) -> String {
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

struct DiagnosticText: View {
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

func pkceChallenge(_ verifier: String) -> String {
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
