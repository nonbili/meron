import CryptoKit
import MeronShared
import AVKit
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
    let onSave: () -> Void
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
                        Label(String(localized: "common.share"), systemImage: "square.and.arrow.up")
                    }
                    Spacer()
                    Button {
                        UIPasteboard.general.image = preview.image
                    } label: {
                        Label(String(localized: "chat.actions.copyImage"), systemImage: "doc.on.doc")
                    }
                    Spacer()
                    Button {
                        onSave()
                    } label: {
                        Label(String(localized: "chat.actions.saveImage"), systemImage: "square.and.arrow.down")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button(String(localized: "buttons.done")) {
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
    let onImportOpml: () -> Void
    let onExportOpml: () -> Void
    let showInNavigation: Bool

    @State var displayName: String
    @State var senderName: String
    @State var avatarUrl: String
    @State var wallpaperPresetId: String
    @State var wallpaperUrl: String
    @State var loadRemoteImages: Bool
    @State var conversationHtml: Bool
    @State var includedInUnified: Bool
    @State var visibleInNavigation: Bool
    @State var muted: Bool
    @State var paused: Bool
    @State var intervalText: String
    @State var aliasEntries: [IosAliasDraftEntry]
    @State var confirmRemove = false

    init(
        account: AccountSummary,
        isRss: Bool,
        onSave: @escaping (AccountSettingsDraft) -> Void,
        onPickAvatar: @escaping () -> Void,
        onPickWallpaper: @escaping () -> Void,
        onRemove: @escaping () -> Void,
        onImportOpml: @escaping () -> Void = {},
        onExportOpml: @escaping () -> Void = {},
        showInNavigation: Bool
    ) {
        self.account = account
        self.isRss = isRss
        self.onSave = onSave
        self.onPickAvatar = onPickAvatar
        self.onPickWallpaper = onPickWallpaper
        self.onRemove = onRemove
        self.onImportOpml = onImportOpml
        self.onExportOpml = onExportOpml
        self.showInNavigation = showInNavigation
        _displayName = State(initialValue: account.displayName)
        _senderName = State(initialValue: account.senderName)
        _avatarUrl = State(initialValue: account.avatarUrl)
        _wallpaperPresetId = State(initialValue: account.chatWallpaperPresetId)
        _wallpaperUrl = State(initialValue: account.chatWallpaperKind == "custom" ? account.chatWallpaperUrl : "")
        _loadRemoteImages = State(initialValue: account.loadRemoteImages || isRss)
        _conversationHtml = State(initialValue: account.conversationHtml)
        _includedInUnified = State(initialValue: account.includedInUnified)
        _visibleInNavigation = State(initialValue: showInNavigation)
        _muted = State(initialValue: account.muted)
        _paused = State(initialValue: account.paused)
        _intervalText = State(initialValue: "\(account.rssSyncIntervalMinutes)")
        _aliasEntries = State(initialValue: account.aliases.map { alias in
            IosAliasDraftEntry(email: alias.email, name: alias.name)
        })
    }

    var body: some View {
        Group {
            IosAccountEditorHeader(
                displayName: displayName,
                accountEmail: account.email,
                accountId: account.id,
                avatarUrl: avatarUrl,
                isRss: isRss,
                onPickAvatar: onPickAvatar
            )
            TextField(
                isRss ? String(localized: "settings.account.feedGroupName") : String(localized: "settings.account.displayName"),
                text: $displayName
            )
            if !isRss {
                TextField(String(localized: "settings.account.senderName"), text: $senderName)
            }
            TextField(String(localized: "mobile.ios.avatarUrl"), text: $avatarUrl)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button {
                onPickAvatar()
            } label: {
                Label(String(localized: "settings.account.chooseAvatarImage"), systemImage: "photo")
            }
            IosWallpaperPreview(
                presetId: wallpaperPresetId,
                customUrl: wallpaperUrl
            )
            IosWallpaperPresetPicker(selected: Binding(
                get: { wallpaperPresetId },
                set: { presetId in
                    wallpaperPresetId = presetId
                    wallpaperUrl = ""
                }
            ))
            TextField(String(localized: "mobile.ios.wallpaperImageUrl"), text: Binding(
                get: { wallpaperUrl },
                set: { value in
                    wallpaperUrl = value
                    if !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        wallpaperPresetId = ""
                    }
                }
            ))
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button {
                onPickWallpaper()
            } label: {
                Label(String(localized: "settings.chooseWallpaperImage"), systemImage: "photo.on.rectangle")
            }
            Text(String(localized: "settings.account.chatBackgroundHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
            Toggle(String(localized: "settings.account.showInUnifiedInbox"), isOn: $includedInUnified)
            Text(String(localized: "settings.account.showInUnifiedInboxHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
            Toggle(String(localized: "settings.account.showInSideNav"), isOn: $visibleInNavigation)
            Text(String(localized: "settings.account.showInSideNavHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
            Toggle(String(localized: "settings.account.muteNotifications"), isOn: $muted)
            Text(String(localized: "settings.account.muteNotificationsHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
            Toggle(String(localized: "settings.account.pauseAccount"), isOn: $paused)
            Text(String(localized: "settings.account.pauseAccountHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
            Toggle(String(localized: "settings.account.loadRemoteImages"), isOn: $loadRemoteImages)
            Text(String(localized: "settings.account.loadRemoteImagesHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
            Toggle(String(localized: "settings.account.renderHtmlMessages"), isOn: $conversationHtml)
            if isRss {
                TextField(String(localized: "settings.account.syncIntervalMinutes"), text: $intervalText)
                    .keyboardType(.numberPad)
                Text(String(localized: "settings.account.syncIntervalRange"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Section(String(localized: "settings.sections.subscriptions")) {
                    Text(String(localized: "settings.feeds.opmlHint"))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Button {
                        onImportOpml()
                    } label: {
                        Label(String(localized: "common.import"), systemImage: "square.and.arrow.down")
                    }
                    Button {
                        onExportOpml()
                    } label: {
                        Label(String(localized: "common.export"), systemImage: "square.and.arrow.up")
                    }
                }
            } else {
                Section(String(localized: "settings.account.aliases")) {
                    Text(String(localized: "settings.account.aliasesHint"))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    ForEach($aliasEntries) { $entry in
                        IosAliasEditorRow(
                            entry: $entry,
                            onRemove: {
                                aliasEntries.removeAll { $0.id == entry.id }
                            }
                        )
                    }
                    Button {
                        aliasEntries.append(IosAliasDraftEntry(email: "", name: ""))
                    } label: {
                        Label(String(localized: "settings.account.addAlias"), systemImage: "plus.circle")
                    }
                }
            }
            Button {
                persistAccountSettings()
            } label: {
                Label(String(localized: "buttons.save"), systemImage: "checkmark.circle")
            }
            Button(role: .destructive) {
                confirmRemove = true
            } label: {
                Label(String(localized: "settings.account.removeAccount"), systemImage: "trash")
            }
            Text(String(localized: "settings.account.deleteCachedMailHint"))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .alert(String(localized: "settings.account.removeAccountTitle"), isPresented: $confirmRemove) {
            Button(String(localized: "settings.account.removeAccount"), role: .destructive) {
                confirmRemove = false
                onRemove()
            }
            Button(String(localized: "buttons.cancel"), role: .cancel) {}
        } message: {
            Text(accountRemovalMessage)
        }
        .onChange(of: displayName) { _, _ in persistAccountSettings() }
        .onChange(of: senderName) { _, _ in persistAccountSettings() }
        .onChange(of: avatarUrl) { _, _ in persistAccountSettings() }
        .onChange(of: wallpaperPresetId) { _, _ in persistAccountSettings() }
        .onChange(of: wallpaperUrl) { _, _ in persistAccountSettings() }
        .onChange(of: account.chatWallpaperUrl) { _, url in
            wallpaperUrl = account.chatWallpaperKind == "custom" ? url : ""
        }
        .onChange(of: account.chatWallpaperPresetId) { _, presetId in
            wallpaperPresetId = presetId
        }
        .onChange(of: loadRemoteImages) { _, _ in persistAccountSettings() }
        .onChange(of: conversationHtml) { _, _ in persistAccountSettings() }
        .onChange(of: includedInUnified) { _, _ in persistAccountSettings() }
        .onChange(of: visibleInNavigation) { _, _ in persistAccountSettings() }
        .onChange(of: muted) { _, _ in persistAccountSettings() }
        .onChange(of: paused) { _, _ in persistAccountSettings() }
        .onChange(of: aliasEntries) { _, _ in persistAccountSettings() }
        .onChange(of: intervalText) { _, value in
            let filtered = String(value.filter(\.isNumber).prefix(4))
            if filtered != value {
                intervalText = filtered
                return
            }
            persistAccountSettings()
        }
    }

    var accountRemovalLabel: String {
        account.email.isEmpty ? account.id : account.email
    }

    var accountRemovalMessage: String {
        String(localized: "settings.account.removeAccountText")
            .replacingOccurrences(of: "%1$s", with: accountRemovalLabel)
    }

    var accountSettingsDraft: AccountSettingsDraft {
        let minutes = min(1440, max(5, Int(intervalText) ?? Int(account.rssSyncIntervalMinutes)))
        return AccountSettingsDraft(
            displayName: displayName,
            senderName: senderName,
            avatarUrl: avatarUrl,
            wallpaperPresetId: wallpaperPresetId,
            wallpaperUrl: wallpaperUrl,
            loadRemoteImages: loadRemoteImages,
            conversationHtml: conversationHtml,
            includedInUnified: includedInUnified,
            showInNavigation: visibleInNavigation,
            muted: muted,
            paused: paused,
            rssSyncIntervalMinutes: minutes,
            aliasesText: aliasesText
        )
    }

    func persistAccountSettings() {
        onSave(accountSettingsDraft)
    }

    var aliasesText: String {
        aliasEntries
            .filter { !$0.email.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
            .map { entry in
                let email = entry.email.trimmingCharacters(in: .whitespacesAndNewlines)
                let name = entry.name.trimmingCharacters(in: .whitespacesAndNewlines)
                return name.isEmpty ? email : "\(email), \(name)"
            }
            .joined(separator: "\n")
    }
}

struct IosAliasDraftEntry: Identifiable, Equatable {
    let id = UUID()
    var email: String
    var name: String
}

struct IosAliasEditorRow: View {
    @Binding var entry: IosAliasDraftEntry
    let onRemove: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 8) {
            VStack(spacing: 8) {
                TextField(String(localized: "accounts.fields.emailAddress"), text: $entry.email)
                    .keyboardType(.emailAddress)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField(String(localized: "accounts.fields.displayNameMeronOnly"), text: $entry.name)
                    .textInputAutocapitalization(.words)
            }
            Button(role: .destructive, action: onRemove) {
                Image(systemName: "xmark.circle")
                    .imageScale(.large)
            }
            .accessibilityLabel(String(localized: "settings.account.removeAlias"))
        }
    }
}

struct IosAccountEditorHeader: View {
    let displayName: String
    let accountEmail: String
    let accountId: String
    let avatarUrl: String
    let isRss: Bool
    let onPickAvatar: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 14) {
            IosAccountAvatarPreview(label: title, avatarUrl: avatarUrl, size: 58)
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.headline)
                    .lineLimit(1)
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Text(isRss ? String(localized: "settings.sections.feedAccounts") : String(localized: "mobile.accounts.passwordAccount"))
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .textCase(.uppercase)
            }
            Spacer(minLength: 8)
            Button {
                onPickAvatar()
            } label: {
                Label(String(localized: "common.change"), systemImage: "photo")
                    .labelStyle(.iconOnly)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(String(localized: "settings.account.changeAvatar"))
        }
        .padding(.vertical, 6)
    }

    var title: String {
        displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? subtitle : displayName
    }

    var subtitle: String {
        accountEmail.isEmpty ? accountId : accountEmail
    }
}

struct IosAccountAvatarPreview: View {
    let label: String
    let avatarUrl: String
    let size: CGFloat

    var body: some View {
        Group {
            if let url = avatarUrl.imageURL {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case let .success(image):
                        image
                            .resizable()
                            .scaledToFill()
                    default:
                        fallback
                    }
                }
            } else {
                fallback
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(Circle().stroke(Color(.separator).opacity(0.35), lineWidth: 1))
        .accessibilityHidden(true)
    }

    var fallback: some View {
        ZStack {
            Circle()
                .fill(avatarColor(for: label))
            Text(avatarInitials(label))
                .font(.system(size: size * 0.34, weight: .semibold))
                .foregroundStyle(.white)
        }
    }
}

struct IosWallpaperPresetPicker: View {
    @Binding var selected: String

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(iosWallpaperPresets) { preset in
                    Button {
                        selected = preset.id
                    } label: {
                        Text(preset.name)
                            .font(.caption)
                            .lineLimit(1)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(selected == preset.id ? Color.accentColor.opacity(0.16) : Color(.tertiarySystemGroupedBackground))
                            .foregroundStyle(selected == preset.id ? Color.accentColor : Color.primary)
                            .clipShape(Capsule())
                            .overlay {
                                Capsule()
                                    .stroke(selected == preset.id ? Color.accentColor : Color.clear, lineWidth: 1)
                            }
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.vertical, 2)
        }
    }
}

struct IosWallpaperPreview: View {
    let presetId: String
    let customUrl: String

    var body: some View {
        ZStack {
            IosWallpaperBackground(presetId: presetId, customUrl: customUrl)
            VStack(spacing: 8) {
                previewBubble(String(localized: "mobile.ios.wallpaperPreviewIncoming"), incoming: true)
                previewBubble(String(localized: "mobile.ios.wallpaperPreviewOutgoing"), incoming: false)
            }
            .padding(16)
        }
        .frame(height: 160)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay {
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color(.separator).opacity(0.3), lineWidth: 1)
        }
    }

    func previewBubble(_ text: String, incoming: Bool) -> some View {
        HStack {
            if !incoming {
                Spacer(minLength: 44)
            }
            Text(text)
                .font(.caption)
                .foregroundStyle(incoming ? Color.primary : Color.white)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(incoming ? Color(.systemBackground).opacity(0.92) : Color.accentColor.opacity(0.9))
                .clipShape(RoundedRectangle(cornerRadius: 14))
            if incoming {
                Spacer(minLength: 44)
            }
        }
    }
}

struct IosWallpaperBackground: View {
    let presetId: String
    let customUrl: String

    var body: some View {
        ZStack {
            wallpaperFill
            if customUrl.isEmpty {
                wallpaperPattern
            }
            if let url = customUrl.imageURL {
                AsyncImage(url: url) { image in
                    image
                        .resizable()
                        .scaledToFill()
                } placeholder: {
                    ProgressView()
                }
            }
        }
        .clipped()
    }

    @ViewBuilder
    var wallpaperFill: some View {
        switch presetId {
        case "aurora", "nebula", "galaxy", "nightsky":
            LinearGradient(colors: [iosColor(0x111827), iosColor(0x4F46E5), iosColor(0x14B8A6)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "sunset", "autumn":
            LinearGradient(colors: [iosColor(0xF97316), iosColor(0xFBCFE8), iosColor(0x7C2D12)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "forest":
            LinearGradient(colors: [iosColor(0x064E3B), iosColor(0xA7F3D0)], startPoint: .top, endPoint: .bottom)
        case "desert", "vintage":
            LinearGradient(colors: [iosColor(0xFDE68A), iosColor(0xD97706)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "ocean", "breeze", "raindrops":
            LinearGradient(colors: [iosColor(0xE0F2FE), iosColor(0x0EA5E9)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "mountain":
            LinearGradient(colors: [iosColor(0xCBD5E1), iosColor(0x475569)], startPoint: .top, endPoint: .bottom)
        case "sakura":
            LinearGradient(colors: [iosColor(0xFFF1F2), iosColor(0xFDA4AF)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "marble":
            LinearGradient(colors: [iosColor(0xF8FAFC), iosColor(0xCBD5E1)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "cyberpunk", "matrix":
            LinearGradient(colors: [iosColor(0x020617), iosColor(0x0F766E)], startPoint: .topLeading, endPoint: .bottomTrailing)
        case "shapes":
            LinearGradient(colors: [iosColor(0xEDE9FE), iosColor(0xFDE68A), iosColor(0xA7F3D0)], startPoint: .topLeading, endPoint: .bottomTrailing)
        default:
            LinearGradient(colors: [Color(.systemGroupedBackground), Color(.secondarySystemGroupedBackground)], startPoint: .top, endPoint: .bottom)
        }
    }

    @ViewBuilder
    var wallpaperPattern: some View {
        switch presetId {
        case "dots", "raindrops":
            DotPattern()
                .stroke(Color.primary.opacity(0.12), lineWidth: 1)
        case "grid", "cyberpunk", "matrix":
            GridPattern()
                .stroke(Color.primary.opacity(0.14), lineWidth: 1)
        case "stripes":
            StripePattern()
                .stroke(Color.primary.opacity(0.12), lineWidth: 2)
        case "hexagon", "isometric", "nordic", "topography", "constellation", "doodle", "waves":
            WavePattern()
                .stroke(Color.primary.opacity(0.12), lineWidth: 1.5)
        default:
            EmptyView()
        }
    }
}

struct DotPattern: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        let step: CGFloat = 18
        var y = step / 2
        while y < rect.height {
            var x = step / 2
            while x < rect.width {
                path.addEllipse(in: CGRect(x: x, y: y, width: 2, height: 2))
                x += step
            }
            y += step
        }
        return path
    }
}

struct GridPattern: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        let step: CGFloat = 22
        var x: CGFloat = 0
        while x <= rect.width {
            path.move(to: CGPoint(x: x, y: 0))
            path.addLine(to: CGPoint(x: x, y: rect.height))
            x += step
        }
        var y: CGFloat = 0
        while y <= rect.height {
            path.move(to: CGPoint(x: 0, y: y))
            path.addLine(to: CGPoint(x: rect.width, y: y))
            y += step
        }
        return path
    }
}

struct StripePattern: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        var x = -rect.height
        while x < rect.width {
            path.move(to: CGPoint(x: x, y: rect.height))
            path.addLine(to: CGPoint(x: x + rect.height, y: 0))
            x += 18
        }
        return path
    }
}

struct WavePattern: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        let step: CGFloat = 24
        var y = step
        while y < rect.height {
            path.move(to: CGPoint(x: 0, y: y))
            var x: CGFloat = 0
            while x < rect.width {
                path.addCurve(
                    to: CGPoint(x: x + step, y: y),
                    control1: CGPoint(x: x + step * 0.25, y: y - 10),
                    control2: CGPoint(x: x + step * 0.75, y: y + 10)
                )
                x += step
            }
            y += step
        }
        return path
    }
}

struct HtmlMessageWebView: UIViewRepresentable {
    let html: String
    let allowRemoteImages: Bool

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeUIView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        let preferences = WKWebpagePreferences()
        preferences.allowsContentJavaScript = false
        configuration.defaultWebpagePreferences = preferences
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.navigationDelegate = context.coordinator
        view.isOpaque = false
        view.backgroundColor = .clear
        view.scrollView.backgroundColor = .clear
        return view
    }

    func updateUIView(_ webView: WKWebView, context _: Context) {
        webView.loadHTMLString(preparedHtmlMessageForWebView(html, allowRemoteImages: allowRemoteImages), baseURL: nil)
    }

    final class Coordinator: NSObject, WKNavigationDelegate {
        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
        ) {
            guard navigationAction.navigationType == .linkActivated,
                  let url = externalMessageNavigationUrl(navigationAction.request.url)
            else {
                decisionHandler(.allow)
                return
            }
            UIApplication.shared.open(url)
            decisionHandler(.cancel)
        }
    }
}

let iosTransparentPixelDataUri = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"

func stripTrackingPixelsFromHtml(_ html: String) -> String {
    let pattern = #"<img\b[^>]*>"#
    guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else {
        return html
    }
    let nsRange = NSRange(html.startIndex ..< html.endIndex, in: html)
    let matches = regex.matches(in: html, range: nsRange).reversed()
    var output = html
    for match in matches {
        guard let range = Range(match.range, in: output) else { continue }
        let tag = String(output[range])
        let replacement = sanitizedTrackingPixelImageTag(tag)
        if replacement != tag {
            output.replaceSubrange(range, with: replacement)
        }
    }
    return output
}

func preparedHtmlMessageForWebView(_ html: String, allowRemoteImages: Bool) -> String {
    injectHtmlMessageCsp(
        into: stripTrackingPixelsFromHtml(html),
        allowRemoteImages: allowRemoteImages
    )
}

func injectHtmlMessageCsp(into html: String, allowRemoteImages: Bool) -> String {
    let imageSources = allowRemoteImages ? "* data: blob:" : "data: blob:"
    let csp = """
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'; img-src \(imageSources); media-src \(imageSources); style-src 'unsafe-inline'; font-src data:;">
    """
    if let range = html.range(of: #"<head\b[^>]*>"#, options: [.regularExpression, .caseInsensitive]) {
        var next = html
        next.insert(contentsOf: csp, at: range.upperBound)
        return next
    }
    return "<!doctype html><html><head>\(csp)</head><body>\(html)</body></html>"
}

func externalMessageNavigationUrl(_ url: URL?) -> URL? {
    guard let url,
          let scheme = url.scheme?.lowercased(),
          ["http", "https", "mailto", "tel"].contains(scheme)
    else {
        return nil
    }
    return url
}

func sanitizedTrackingPixelImageTag(_ tag: String) -> String {
    guard imageTagLooksLikeTrackingPixel(tag) else { return tag }
    var next = removeHtmlImageAttribute("srcset", from: tag)
    next = removeHtmlImageAttribute("width", from: next)
    next = removeHtmlImageAttribute("height", from: next)
    if htmlAttributeValue("src", in: next) != nil {
        next = replaceHtmlImageAttribute("src", value: iosTransparentPixelDataUri, in: next)
    } else if let close = next.lastIndex(of: ">") {
        next.insert(contentsOf: #" src="\#(iosTransparentPixelDataUri)""#, at: close)
    }
    return next
}

func imageTagLooksLikeTrackingPixel(_ tag: String) -> Bool {
    let width = htmlAttributeValue("width", in: tag)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let height = htmlAttributeValue("height", in: tag)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let style = htmlAttributeValue("style", in: tag)?.lowercased() ?? ""
    let src = htmlAttributeValue("src", in: tag)?.lowercased() ?? ""
    let isTinyAttribute = ["0", "1", "2"].contains(width) && ["0", "1", "2"].contains(height)
    let compactStyle = style.replacingOccurrences(of: " ", with: "")
    let isHiddenStyle = compactStyle.contains("display:none") || compactStyle.contains("visibility:hidden")
    let hasTinyWidth = ["width:0px", "width:1px", "width:2px"].contains { compactStyle.contains($0) }
    let hasTinyHeight = ["height:0px", "height:1px", "height:2px"].contains { compactStyle.contains($0) }
    let isTinyStyle = hasTinyWidth && hasTinyHeight
    let trackingPatterns = [
        "/open/",
        "/track",
        "/pixel",
        "pixel.gif",
        "cleardot.gif",
        "spacer.gif",
        "/wf/open",
        "/open.php",
        "utm_",
        "bounce",
    ]
    return isTinyAttribute || isHiddenStyle || isTinyStyle || trackingPatterns.contains { src.contains($0) }
}

func htmlAttributeValue(_ name: String, in tag: String) -> String? {
    let escaped = NSRegularExpression.escapedPattern(for: name)
    let pattern = #"\b\#(escaped)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#
    guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else {
        return nil
    }
    let nsRange = NSRange(tag.startIndex ..< tag.endIndex, in: tag)
    guard let match = regex.firstMatch(in: tag, range: nsRange) else {
        return nil
    }
    for index in 1 ..< match.numberOfRanges {
        let range = match.range(at: index)
        if range.location != NSNotFound, let swiftRange = Range(range, in: tag) {
            return String(tag[swiftRange])
        }
    }
    return nil
}

func removeHtmlImageAttribute(_ name: String, from tag: String) -> String {
    let escaped = NSRegularExpression.escapedPattern(for: name)
    let pattern = #"\s+\#(escaped)\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)"#
    return tag.replacingOccurrences(of: pattern, with: "", options: [.regularExpression, .caseInsensitive])
}

func replaceHtmlImageAttribute(_ name: String, value: String, in tag: String) -> String {
    let escapedName = NSRegularExpression.escapedPattern(for: name)
    let escapedValue = value.replacingOccurrences(of: "\"", with: "&quot;")
    let pattern = #"\b\#(escapedName)\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+)"#
    return tag.replacingOccurrences(
        of: pattern,
        with: #"\#(name)="\#(escapedValue)""#,
        options: [.regularExpression, .caseInsensitive]
    )
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

func inboxUnreadCount(folders: [FolderSummary], accountId: String) -> Int {
    folders
        .filter { folder in
            folder.accountId == accountId && folder.name.compare(iosInboxFolderId, options: [.caseInsensitive, .diacriticInsensitive]) == .orderedSame
        }
        .reduce(0) { $0 + Int($1.unread) }
}

func navigationAccountLabel(_ account: AccountSummary, unreadCounts: [String: Int], showUnreadBadges: Bool) -> String {
    let label = account.displayName.isEmpty ? (account.email.isEmpty ? account.id : account.email) : account.displayName
    guard showUnreadBadges, let unread = unreadCounts[account.id], unread > 0 else { return label }
    return "\(label) (\(unread))"
}

func navigationUnifiedInboxLabel(accounts: [AccountSummary], unreadCounts: [String: Int], showUnreadBadges: Bool) -> String {
    let label = String(localized: "kanban.columns.unifiedInbox")
    guard showUnreadBadges else { return label }
    let unread = accounts
        .filter(\.includedInUnified)
        .reduce(0) { $0 + (unreadCounts[$1.id] ?? 0) }
    return unread > 0 ? "\(label) (\(unread))" : label
}

func extractEmail(_ value: String) -> String? {
    let pattern = #"[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}"#
    return value.range(of: pattern, options: [.regularExpression, .caseInsensitive]).map { String(value[$0]) }
}

struct MessageAddressItem: Equatable {
    let name: String
    let email: String
    let original: String
}

struct MessageMetadataRow: Equatable {
    let label: String
    let rawValue: String
}

func conversationParticipantComposeAddress(_ participant: ConversationParticipant) -> String {
    let email = participant.email.trimmingCharacters(in: .whitespacesAndNewlines)
    let name = participant.name.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !name.isEmpty, name.caseInsensitiveCompare(email) != .orderedSame else {
        return email
    }
    return "\(name) <\(email)>"
}

func messageAddressItems(_ value: String) -> [MessageAddressItem] {
    value
        .split(whereSeparator: { $0 == "," || $0 == ";" })
        .compactMap { raw -> MessageAddressItem? in
            let original = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !original.isEmpty else { return nil }
            if let open = original.lastIndex(of: "<"), let close = original.lastIndex(of: ">"), open < close {
                let name = original[..<open]
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                    .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                let email = original[original.index(after: open) ..< close].trimmingCharacters(in: .whitespacesAndNewlines)
                guard !email.isEmpty else { return nil }
                return MessageAddressItem(name: name.isEmpty ? email : name, email: email, original: original)
            }
            return MessageAddressItem(name: original, email: original, original: original)
        }
}

func messageMetadataRows(message: MessageBody, isOutgoing: Bool, ownEmails: Set<String>) -> [MessageMetadataRow] {
    guard !isOutgoing else { return [] }
    let replyTo = message.replyTo.trimmingCharacters(in: .whitespacesAndNewlines)
    let cc = message.cc.trimmingCharacters(in: .whitespacesAndNewlines)
    let to = message.to.trimmingCharacters(in: .whitespacesAndNewlines)
    let fromLabel = message.fromAddr.isEmpty ? message.from : message.fromAddr
    let fromAddress = (extractEmail(fromLabel) ?? fromLabel)
        .trimmingCharacters(in: CharacterSet(charactersIn: " <>;,"))
        .lowercased()

    let toRecipients = messageAddressItems(to)
    let showTo =
        !to.isEmpty &&
        !toRecipients.isEmpty &&
        (ownEmails.isEmpty || toRecipients.contains { !ownEmails.contains($0.email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()) })
    let showReplyTo =
        !replyTo.isEmpty &&
        extractEmail(replyTo)?.lowercased() != fromAddress

    var rows: [MessageMetadataRow] = []
    if showTo {
        rows.append(MessageMetadataRow(label: String(localized: "composer.fields.to"), rawValue: to))
    }
    if showReplyTo {
        rows.append(MessageMetadataRow(label: "Reply-To", rawValue: replyTo))
    }
    if !cc.isEmpty {
        rows.append(MessageMetadataRow(label: String(localized: "composer.fields.cc"), rawValue: cc))
    }
    return rows
}

func messageReaderAddressRows(message: MessageBody) -> [MessageMetadataRow] {
    var rows: [MessageMetadataRow] = []
    let from: String
    if message.from.isEmpty {
        from = message.fromAddr
    } else if message.fromAddr.isEmpty {
        from = message.from
    } else {
        from = "\(message.from) <\(message.fromAddr)>"
    }
    if !from.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        rows.append(MessageMetadataRow(label: String(localized: "composer.fields.from"), rawValue: from))
    }
    if !message.to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        rows.append(MessageMetadataRow(label: String(localized: "composer.fields.to"), rawValue: message.to))
    }
    if !message.cc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        rows.append(MessageMetadataRow(label: String(localized: "composer.fields.cc"), rawValue: message.cc))
    }
    if !message.bcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        rows.append(MessageMetadataRow(label: String(localized: "composer.fields.bcc"), rawValue: message.bcc))
    }
    if !message.replyTo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
        rows.append(MessageMetadataRow(label: "Reply-To", rawValue: message.replyTo))
    }
    return rows
}

func md5Hex(_ value: String) -> String {
    Insecure.MD5.hash(data: Data(value.utf8)).map { String(format: "%02x", $0) }.joined()
}

func threadDeleteActionLabel(_ thread: ThreadSummary) -> String {
    threadDeleteActionLabel(folder: thread.folder)
}

func threadDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "threads.actions.discardDraft")
    }
    if MailStateKt.folderIsTrash(folder: folder) {
        return String(localized: "threads.actions.deleteForever")
    }
    return String(localized: "threads.actions.moveToTrash")
}

func threadReadToggleActionLabel(_ thread: ThreadSummary) -> String {
    thread.unread ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread")
}

func iosAccountStateLabels(
    needsReconnect: Bool,
    paused: Bool,
    muted: Bool,
    hiddenFromNavigation: Bool
) -> [String] {
    var labels: [String] = []
    if needsReconnect {
        labels.append(String(localized: "mobile.ios.needsReconnect"))
    }
    if paused {
        labels.append(String(localized: "settings.account.pauseAccount"))
    }
    if muted {
        labels.append(String(localized: "settings.account.muteNotifications"))
    }
    if hiddenFromNavigation {
        labels.append(String(localized: "settings.account.hiddenFromNavigation"))
    }
    return labels
}

func kanbanColumnHideActionLabel() -> String {
    String(localized: "kanban.actions.hideColumn")
}

func threadReadUndoSeenTarget(afterMarkingSeen seen: Bool) -> Bool {
    !seen
}

func threadStarUndoTarget(afterSettingStarred starred: Bool) -> Bool {
    !starred
}

func threadDeleteRequiresConfirmation(_ thread: ThreadSummary?) -> Bool {
    guard let thread else { return false }
    return threadDeleteRequiresConfirmation(folder: thread.folder)
}

func threadDeleteRequiresConfirmation(folder: String) -> Bool {
    MailStateKt.folderIsDrafts(folder: folder) || MailStateKt.folderIsTrash(folder: folder)
}

func firstThreadRequiringDeleteConfirmation(_ threads: [ThreadSummary]) -> ThreadSummary? {
    threads.first { threadDeleteRequiresConfirmation($0) }
}

func threadDeleteConfirmationTitle(_ thread: ThreadSummary?) -> String {
    guard let thread else { return String(localized: "buttons.delete") }
    return MailStateKt.folderIsDrafts(folder: thread.folder)
        ? String(localized: "mobile.compose.discardDraftTitle")
        : threadDeleteActionLabel(thread)
}

func threadDeleteConfirmationMessage(_ thread: ThreadSummary) -> String {
    if MailStateKt.folderIsDrafts(folder: thread.folder) {
        return String(localized: "mobile.compose.discardDraftText")
    }
    return thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject
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

func sendShortcutHintLocalizationKey(_ mode: String) -> String {
    mode == "enter" ? "settings.composer.sendShortcutEnterHint" : "settings.composer.sendShortcutModHint"
}

func sendShortcutHintText(_ mode: String) -> String {
    if mode == "enter" {
        return String(localized: "settings.composer.sendShortcutEnterHint")
    }
    return localizedCatalogString(
        "settings.composer.sendShortcutModHint",
        args: ["shortcut": sendShortcutLabel("mod_enter")]
    )
}

func iosSyncErrorLooksAuthRelated(_ message: String) -> Bool {
    let lower = message.lowercased()
    return ["auth", "login", "credential", "password", "unauthor", "permission", "token", "401", "535"]
        .contains { lower.contains($0) }
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

func quickReplyCanSend(body: String, attachmentCount: Int, sending: Bool) -> Bool {
    !sending && (!body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || attachmentCount > 0)
}

func rssFeedMoveTargetAccounts(accounts: [AccountSummary], sourceAccountId: String) -> [AccountSummary] {
    accounts.filter { account in
        MailStateKt.accountSummaryIsRss(account: account) && account.id != sourceAccountId
    }
}

struct IosSelectionMoveCopyAvailability: Equatable {
    var canMove: Bool
    var canCopy: Bool
}

func iosSelectionMoveCopyAvailability(selectedThreads: [ThreadSummary], rssMoveTargetCount: Int) -> IosSelectionMoveCopyAvailability {
    guard selectedThreads.count == 1, let thread = selectedThreads.first else {
        return IosSelectionMoveCopyAvailability(canMove: false, canCopy: false)
    }
    if MailStateKt.threadIdIsRss(threadId: thread.id) {
        return IosSelectionMoveCopyAvailability(canMove: rssMoveTargetCount > 0, canCopy: false)
    }
    return IosSelectionMoveCopyAvailability(canMove: true, canCopy: true)
}

struct ThreadRow<Actions: View>: View {
    let thread: ThreadSummary
    let account: AccountSummary?
    let showSenderImages: Bool
    let selected: Bool
    let selectionActive: Bool
    let onArchive: () -> Void
    let onToggleStar: () -> Void
    let onOpen: () -> Void
    let onToggleSelected: () -> Void
    let onLongPress: () -> Void
    @ViewBuilder let actions: () -> Actions

    var senderLabel: String {
        thread.sender.isEmpty ? thread.accountId : thread.sender
    }

    var body: some View {
        Button(action: onOpen) {
            HStack(alignment: .top, spacing: 10) {
                if selectionActive {
                    Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                        .font(.system(size: 26, weight: .semibold))
                        .foregroundStyle(selected ? Color.accentColor : Color.secondary)
                        .frame(width: 40, height: 40)
                        .accessibilityLabel(selected ? "Selected" : "Not selected")
                } else {
                    ThreadAvatarWithAccountBadge(
                        senderLabel: senderLabel,
                        showSenderImages: showSenderImages,
                        account: account,
                        size: 40
                    )
                }
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        Text(senderLabel)
                            .font(.subheadline.weight(thread.unread ? .semibold : .regular))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        Spacer()
                        Text(relativeTime(thread.dateEpochSeconds))
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(thread.unread ? Color.accentColor : Color.secondary)
                            .lineLimit(1)
                        unreadDot
                    }
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject)
                                .font(.headline)
                                .foregroundStyle(.primary)
                                .lineLimit(2)
                            if !thread.preview.isEmpty {
                                Text(thread.preview)
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                            }
                        }
                        Spacer(minLength: 4)
                        if selectionActive {
                            Button(action: onToggleSelected) {
                                Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                                    .font(.caption)
                                    .foregroundStyle(selected ? Color.accentColor : Color.secondary)
                                    .frame(width: 28, height: 28)
                            }
                            .buttonStyle(.borderless)
                            .accessibilityLabel(selected ? "Selected" : "Not selected")
                        } else {
                            Button(action: onToggleStar) {
                                Image(systemName: thread.starred ? "star.fill" : "star")
                                    .font(.caption)
                                    .foregroundStyle(thread.starred ? Color.yellow : Color.secondary)
                                    .frame(width: 28, height: 28)
                            }
                            .buttonStyle(.borderless)
                            .accessibilityLabel(thread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star"))
                        }
                    }
                    Text(threadFooter)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .listRowBackground(selected ? Color.accentColor.opacity(0.12) : Color.clear)
        .onLongPressGesture(perform: onLongPress)
        .swipeActions(edge: .trailing, allowsFullSwipe: false) {
            actions()
        }
        .swipeActions(edge: .leading, allowsFullSwipe: false) {
            if !selectionActive {
                Button {
                    onArchive()
                } label: {
                    Label(
                        MailStateKt.threadIdIsRss(threadId: thread.id)
                            ? String(localized: "feeds.actions.deleteFeed")
                            : String(localized: "threads.actions.archiveThread"),
                        systemImage: MailStateKt.threadIdIsRss(threadId: thread.id) ? "trash" : "archivebox"
                    )
                }
                .tint(.blue)
            }
        }
    }

    @ViewBuilder
    var unreadDot: some View {
        if thread.unread {
            Circle()
                .fill(Color.accentColor)
                .frame(width: 8, height: 8)
                .accessibilityLabel(String(localized: "common.unread"))
        }
    }

    var threadFooter: String {
        if let account {
            let label = accountLabel(account)
            if thread.folder.isEmpty {
                return label
            }
            return "\(label) / \(thread.folder)"
        }
        return thread.folder
    }

    func accountLabel(_ account: AccountSummary) -> String {
        account.displayName.isEmpty ? (account.email.isEmpty ? account.id : account.email) : account.displayName
    }
}

struct ThreadAvatarWithAccountBadge: View {
    let senderLabel: String
    let showSenderImages: Bool
    let account: AccountSummary?
    let size: CGFloat

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            SenderAvatar(label: senderLabel, enabled: showSenderImages, size: size)
            if let account {
                AccountBadgeAvatar(account: account, size: max(16, size * 0.45))
                    .offset(x: -2, y: 2)
            }
        }
        .frame(width: size, height: size)
    }
}

struct AccountBadgeAvatar: View {
    let account: AccountSummary
    let size: CGFloat

    var body: some View {
        Group {
            if let url = account.avatarUrl.imageURL {
                AsyncImage(url: url) { image in
                    image
                        .resizable()
                        .scaledToFill()
                } placeholder: {
                    fallback
                }
            } else {
                fallback
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(Circle().stroke(Color(.systemBackground), lineWidth: 2))
        .accessibilityLabel(accountLabel)
    }

    var fallback: some View {
        Text(initials(for: accountLabel))
            .font(.system(size: max(8, size * 0.42), weight: .semibold))
            .foregroundStyle(.white)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color.accentColor)
    }

    var accountLabel: String {
        account.displayName.isEmpty ? (account.email.isEmpty ? account.id : account.email) : account.displayName
    }
}

struct IosCommand: Identifiable {
    let id: String
    let label: String
    let keywords: String
    let systemImage: String
    let active: Bool
    let role: ButtonRole?
    let action: () -> Void
}

struct IosCommandPaletteSheet: View {
    let commands: [IosCommand]
    @Binding var query: String
    let onRun: (IosCommand) -> Void
    @Environment(\.dismiss) private var dismiss

    var filteredCommands: [IosCommand] {
        commands.filter { iosCommandMatches($0, query: query) }
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    TextField(String(localized: "palette.placeholder"), text: $query)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }

                if filteredCommands.isEmpty {
                    Section {
                        Text(String(localized: "palette.noMatches"))
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Section {
                        ForEach(filteredCommands) { command in
                            Button(role: command.role) {
                                onRun(command)
                            } label: {
                                HStack(spacing: 12) {
                                    Image(systemName: command.systemImage)
                                        .frame(width: 22)
                                        .foregroundStyle(command.role == .destructive ? Color.red : Color.accentColor)
                                    Text(command.label)
                                        .foregroundStyle(command.role == .destructive ? .red : .primary)
                                    Spacer(minLength: 8)
                                    if command.active {
                                        Image(systemName: "checkmark")
                                            .font(.caption.weight(.semibold))
                                            .foregroundStyle(.tint)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            .navigationTitle(String(localized: "palette.label"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(String(localized: "buttons.cancel")) {
                        dismiss()
                    }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

func iosCommandMatches(_ command: IosCommand, query: String) -> Bool {
    let normalized = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
    guard !normalized.isEmpty else { return true }
    return [
        command.label,
        command.keywords,
        command.id,
    ].joined(separator: " ").lowercased().contains(normalized)
}

struct IosMessageReaderSheet: View {
    let message: MessageBody
    let preferHtml: Bool
    let allowRemoteImages: Bool
    let onOpenAttachment: (MessageAttachment) -> Void
    let onSaveAttachment: (MessageAttachment) -> Void
    let onCopy: (String, String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var viewHtml: Bool

    init(
        message: MessageBody,
        preferHtml: Bool,
        allowRemoteImages: Bool,
        onOpenAttachment: @escaping (MessageAttachment) -> Void,
        onSaveAttachment: @escaping (MessageAttachment) -> Void,
        onCopy: @escaping (String, String) -> Void
    ) {
        self.message = message
        self.preferHtml = preferHtml
        self.allowRemoteImages = allowRemoteImages
        self.onOpenAttachment = onOpenAttachment
        self.onSaveAttachment = onSaveAttachment
        self.onCopy = onCopy
        _viewHtml = State(initialValue: preferHtml && !message.bodyHtml.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }

    var subject: String {
        message.subject.isEmpty ? String(localized: "threads.noSubject") : message.subject
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(subject)
                            .font(.headline)
                            .textSelection(.enabled)
                        let timestamp = messageFullTimestamp(message.dateEpochSeconds)
                        if !timestamp.isEmpty {
                            Text(timestamp)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }
                    }
                }

                Section {
                    if !readerAddressRows.isEmpty {
                        ForEach(readerAddressRows, id: \.label) { row in
                            MessageMetadataAddressRow(row: row, onCopy: onCopy)
                        }
                    }
                    if !message.messageId.isEmpty {
                        Button {
                            onCopy(String(localized: "chat.messageId"), message.messageId)
                        } label: {
                            Label(message.messageId, systemImage: "number")
                                .lineLimit(1)
                        }
                    }
                }

                if !message.bodyHtml.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Section {
                        Picker(String(localized: "mobile.ios.view"), selection: $viewHtml) {
                            Text(String(localized: "chat.htmlView")).tag(true)
                            Text(String(localized: "chat.plainView")).tag(false)
                        }
                        .pickerStyle(.segmented)
                    }
                }

                Section {
                    if viewHtml, !message.bodyHtml.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        HtmlMessageWebView(html: message.bodyHtml, allowRemoteImages: allowRemoteImages)
                            .frame(minHeight: 420)
                    } else {
                        PlainMessageBodyView(
                            text: messagePlainText(message),
                            emptyText: String(localized: "mobile.ios.noContent"),
                            searchQuery: "",
                            activeSearchMatch: false,
                            onCopyCode: { code in onCopy(String(localized: "chat.copyCode"), code) }
                        )
                    }
                }

                if !readerAttachmentGroups.isEmpty {
                    Section(String(localized: "chat.sharedFiles")) {
                        if !readerAttachmentGroups.mediaAttachments.isEmpty {
                            ConversationAttachmentSection(
                                title: String(localized: "chat.media"),
                                emptyMessage: String(localized: "chat.noMedia"),
                                attachments: readerAttachmentGroups.mediaAttachments,
                                onOpenAttachment: onOpenAttachment,
                                onSaveAttachment: onSaveAttachment
                            )
                        }
                        if !readerAttachmentGroups.fileAttachments.isEmpty {
                            ConversationAttachmentSection(
                                title: String(localized: "chat.files"),
                                emptyMessage: String(localized: "chat.noFiles"),
                                attachments: readerAttachmentGroups.fileAttachments,
                                onOpenAttachment: onOpenAttachment,
                                onSaveAttachment: onSaveAttachment
                            )
                        }
                    }
                }
            }
            .navigationTitle(String(localized: "threads.actions.openInNewTab"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(String(localized: "chat.closeTab")) {
                        dismiss()
                    }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        onCopy(String(localized: "chat.messageText"), messagePlainText(message))
                    } label: {
                        Label(String(localized: "chat.copyMessageText"), systemImage: "doc.on.doc")
                    }
                }
            }
        }
        .presentationDetents([.large])
    }

    var readerAddressRows: [MessageMetadataRow] {
        messageReaderAddressRows(message: message)
    }

    var readerAttachmentGroups: MessageReaderAttachmentGroups {
        messageReaderAttachmentGroups(message: message, allowRemoteImages: allowRemoteImages)
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

struct ConversationMessageRow: View {
    let message: MessageBody
    let activeSearchMatch: Bool
    let searchQuery: String
    let renderHtml: Bool
    let isOutgoing: Bool
    let ownEmails: Set<String>
    let allowRemoteImages: Bool
    let remoteMediaRevealed: Bool
    let isDraftContext: Bool
    let canComposeFromMessage: Bool
    let onOpenAttachment: (MessageAttachment) -> Void
    let onSaveAttachment: (MessageAttachment) -> Void
    let onRevealRemoteMedia: () -> Void
    let onCopy: (String, String) -> Void
    let onOpenReader: () -> Void
    let onForward: () -> Void
    let onEditAsNew: () -> Void
    let onToggleRead: () -> Void
    let onToggleStarred: () -> Void
    let onDelete: () -> Void
    @State private var metadataOpen = false

    var senderLabel: String {
        message.from.isEmpty ? message.fromAddr : message.from
    }

    var bubbleBackground: Color {
        isOutgoing ? Color.accentColor.opacity(0.16) : Color(.secondarySystemGroupedBackground)
    }

    var bubbleStroke: Color {
        activeSearchMatch ? Color.yellow.opacity(0.75) : Color(.separator).opacity(isOutgoing ? 0.18 : 0.28)
    }

    var mediaVisibility: MessageMediaVisibility {
        messageMediaVisibility(
            attachments: message.attachments,
            allowRemoteImages: allowRemoteImages,
            revealedRemoteMedia: remoteMediaRevealed
        )
    }

    var imageAttachments: [MessageAttachment] {
        visibleInlineMedia.imageAttachments
    }

    var videoAttachments: [MessageAttachment] {
        visibleInlineMedia.videoAttachments
    }

    var visibleInlineMedia: VisibleInlineMediaAttachments {
        visibleInlineMediaAttachments(mediaVisibility: mediaVisibility, renderHtml: renderHtml)
    }

    var hiddenRemoteMediaCount: Int {
        let remoteVisible = allowRemoteImages || remoteMediaRevealed
        let hiddenHtmlImages = remoteVisible ? 0 : htmlRemoteImageSourceCount(message.bodyHtml)
        return mediaVisibility.hiddenRemoteCount + hiddenHtmlImages
    }

    var visibleFileAttachments: [MessageAttachment] {
        message.attachments.filter { !$0.mimeType.hasPrefix("image/") && !$0.mimeType.hasPrefix("video/") }
    }

    var metadataRows: [MessageMetadataRow] {
        messageMetadataRows(message: message, isOutgoing: isOutgoing, ownEmails: ownEmails)
    }

    var body: some View {
        HStack(alignment: .top) {
            if isOutgoing {
                Spacer(minLength: 48)
            }

            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    if !isOutgoing {
                        Text(senderLabel)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.tint)
                            .lineLimit(1)
                        if !metadataRows.isEmpty {
                            Button {
                                metadataOpen.toggle()
                            } label: {
                                Image(systemName: metadataOpen ? "chevron.up.circle" : "chevron.down.circle")
                                    .font(.caption2)
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(.secondary)
                            .accessibilityLabel(String(localized: metadataOpen ? "chat.hideDetails" : "chat.showDetails"))
                        }
                    }
                    Spacer(minLength: 8)
                    if message.starred {
                        Image(systemName: "star.fill")
                            .font(.caption2)
                            .foregroundStyle(.yellow)
                    }
                    Text(relativeTime(message.dateEpochSeconds))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                if metadataOpen, !metadataRows.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(metadataRows, id: \.label) { row in
                            MessageMetadataAddressRow(row: row, onCopy: onCopy)
                        }
                    }
                    .padding(.vertical, 4)
                    .padding(.horizontal, 6)
                    .background(Color(.tertiarySystemGroupedBackground))
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                if hiddenRemoteMediaCount > 0 {
                    Button {
                        onRevealRemoteMedia()
                    } label: {
                        Label(
                            localizedCatalogString("chat.showImages", args: ["count": hiddenRemoteMediaCount]),
                            systemImage: "photo.on.rectangle"
                        )
                            .font(.caption.weight(.semibold))
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
                if renderHtml {
                    HtmlMessageWebView(html: message.bodyHtml, allowRemoteImages: allowRemoteImages || remoteMediaRevealed)
                        .frame(minHeight: 220)
                } else {
                    if !imageAttachments.isEmpty {
                        InlineImageAttachmentGrid(
                            attachments: imageAttachments,
                            allowRemoteImages: allowRemoteImages,
                            onOpenAttachment: onOpenAttachment
                        )
                    }
                    PlainMessageBodyView(
                        text: message.body,
                        emptyText: String(localized: "mobile.ios.noContent"),
                        searchQuery: searchQuery,
                        activeSearchMatch: activeSearchMatch,
                        onCopyCode: { code in onCopy(String(localized: "chat.copyCode"), code) }
                    )
                }
                if !videoAttachments.isEmpty {
                    InlineVideoAttachmentList(
                        attachments: videoAttachments,
                        allowRemoteImages: allowRemoteImages,
                        onOpenAttachment: onOpenAttachment
                    )
                }
                if !visibleFileAttachments.isEmpty {
                    ForEach(Array(visibleFileAttachments.enumerated()), id: \.offset) { _, attachment in
                        HStack(spacing: 8) {
                            Button {
                                onOpenAttachment(attachment)
                            } label: {
                                Label {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(attachment.filename.isEmpty ? String(localized: "chat.attachment") : attachment.filename)
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
                            if attachmentCanSave(attachment) {
                                Button {
                                    onSaveAttachment(attachment)
                                } label: {
                                    Label(String(localized: "buttons.save"), systemImage: "square.and.arrow.down")
                                        .labelStyle(.iconOnly)
                                }
                                .buttonStyle(.bordered)
                                .accessibilityLabel(String(localized: "buttons.save"))
                            }
                        }
                    }
                }
                Menu {
                    Button {
                        onCopy(String(localized: "chat.messageText"), messagePlainText(message))
                    } label: {
                        Label(String(localized: "chat.copyMessageText"), systemImage: "doc.on.doc")
                    }
                    Button {
                        onCopy(String(localized: "mobile.ios.subject"), message.subject.isEmpty ? String(localized: "threads.noSubject") : message.subject)
                    } label: {
                        Label(String(localized: "chat.copySubject"), systemImage: "text.quote")
                    }
                    if !message.messageId.isEmpty {
                        Button {
                            onCopy(String(localized: "chat.messageId"), message.messageId)
                        } label: {
                            Label(String(localized: "chat.copyMessageId"), systemImage: "number")
                        }
                    }
                    let messageLinks = plainMessageBodyLinks(message.body)
                    if messageLinks.count == 1, let link = messageLinks.first {
                        Button {
                            onCopy(String(localized: "chat.actions.copyLinkAddress"), link)
                        } label: {
                            Label(String(localized: "chat.actions.copyLinkAddress"), systemImage: "link")
                        }
                    }
                    if !isDraftContext {
                        Button {
                            onOpenReader()
                        } label: {
                            Label(String(localized: "threads.actions.openInNewTab"), systemImage: "rectangle.on.rectangle")
                        }
                    }
                    if canComposeFromMessage {
                        Divider()
                        if isDraftContext {
                            Button {
                                onEditAsNew()
                            } label: {
                                Label(String(localized: "chat.actions.openDraft"), systemImage: "square.and.pencil")
                            }
                        }
                        Button {
                            onToggleRead()
                        } label: {
                            Label(
                                message.unread ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread"),
                                systemImage: message.unread ? "envelope.open" : "envelope.badge"
                            )
                        }
                        Button {
                            onToggleStarred()
                        } label: {
                            Label(message.starred ? String(localized: "chat.unstar") : String(localized: "chat.star"), systemImage: message.starred ? "star.slash" : "star")
                        }
                        if !isDraftContext {
                            Button {
                                onForward()
                            } label: {
                                Label(String(localized: "chat.actions.forward"), systemImage: "arrowshape.turn.up.forward")
                            }
                            Button {
                                onEditAsNew()
                            } label: {
                                Label(String(localized: "chat.actions.editAsNewMessage"), systemImage: "doc.on.doc")
                            }
                        }
                        Button(role: .destructive) {
                            onDelete()
                        } label: {
                            Label(
                                isDraftContext ? String(localized: "chat.actions.discardDraft") : String(localized: "chat.actions.deleteMessage"),
                                systemImage: "trash"
                            )
                        }
                    }
                } label: {
                    Label(String(localized: "common.more"), systemImage: "ellipsis.circle")
                        .font(.caption)
                }
                .accessibilityLabel(String(localized: "chat.moreMessageActions"))
            }
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
            .frame(maxWidth: renderHtml ? .infinity : 340, alignment: .leading)
            .background(bubbleBackground)
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(bubbleStroke, lineWidth: activeSearchMatch ? 2 : 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 16))

            if !isOutgoing {
                Spacer(minLength: 48)
            }
        }
        .padding(.vertical, 4)
    }
}

enum PlainMessageBlock: Equatable {
    case text(String)
    case code(String)
}

enum PlainMessageInlinePart: Equatable {
    case text(String)
    case link(url: String, label: String?)
}

enum PlainMessageStyle: Equatable {
    case normal
    case bold
    case italic
    case code
}

struct PlainMessageStyledRun: Equatable {
    var text: String
    var style: PlainMessageStyle
}

struct PlainMessageSearchRun: Equatable {
    var text: String
    var highlighted: Bool
}

struct MessageMediaVisibility {
    var imageAttachments: [MessageAttachment]
    var videoAttachments: [MessageAttachment]
    var hiddenRemoteCount: Int
}

struct VisibleInlineMediaAttachments {
    var imageAttachments: [MessageAttachment]
    var videoAttachments: [MessageAttachment]
}

struct MessageReaderAttachmentGroups {
    var mediaAttachments: [MessageAttachment]
    var fileAttachments: [MessageAttachment]

    var isEmpty: Bool {
        mediaAttachments.isEmpty && fileAttachments.isEmpty
    }
}

func visibleInlineMediaAttachments(
    mediaVisibility: MessageMediaVisibility,
    renderHtml: Bool
) -> VisibleInlineMediaAttachments {
    VisibleInlineMediaAttachments(
        imageAttachments: renderHtml ? [] : mediaVisibility.imageAttachments,
        videoAttachments: mediaVisibility.videoAttachments
    )
}

func messageReaderAttachmentGroups(
    message: MessageBody,
    allowRemoteImages: Bool
) -> MessageReaderAttachmentGroups {
    let media = messageMediaVisibility(
        attachments: message.attachments,
        allowRemoteImages: allowRemoteImages,
        revealedRemoteMedia: false
    )
    let files = message.attachments.filter { !messageAttachmentIsMedia($0) }
    return MessageReaderAttachmentGroups(
        mediaAttachments: media.imageAttachments + media.videoAttachments,
        fileAttachments: files
    )
}

func messageMediaVisibility(
    attachments: [MessageAttachment],
    allowRemoteImages: Bool,
    revealedRemoteMedia: Bool
) -> MessageMediaVisibility {
    let localMedia = attachments.filter { messageAttachmentIsInlineMedia($0) }
    let remoteMedia = attachments.filter { messageAttachmentIsRemoteMedia($0) }
    let remoteVisible = allowRemoteImages || revealedRemoteMedia
    let visibleMedia = remoteVisible ? localMedia + remoteMedia : localMedia

    return MessageMediaVisibility(
        imageAttachments: visibleMedia.filter { $0.mimeType.hasPrefix("image/") },
        videoAttachments: visibleMedia.filter { $0.mimeType.hasPrefix("video/") },
        hiddenRemoteCount: remoteVisible ? 0 : remoteMedia.count
    )
}

func visibleConversationAttachments(
    messages: [MessageBody],
    allowRemoteImages: Bool,
    revealedRemoteMediaMessageIds: Set<String>
) -> [MessageAttachment] {
    Array(messages.flatMap { message in
        let media = messageMediaVisibility(
            attachments: message.attachments,
            allowRemoteImages: allowRemoteImages,
            revealedRemoteMedia: revealedRemoteMediaMessageIds.contains(message.id)
        )
        let files = message.attachments.filter { !messageAttachmentIsMedia($0) }
        return media.imageAttachments + media.videoAttachments + files
    }
    .reversed())
}

func messageAttachmentIsInlineMedia(_ attachment: MessageAttachment) -> Bool {
    guard messageAttachmentIsMedia(attachment) else { return false }
    return !attachment.key.isEmpty || attachment.url.hasPrefix("data:")
}

func messageAttachmentIsRemoteMedia(_ attachment: MessageAttachment) -> Bool {
    guard messageAttachmentIsMedia(attachment) else { return false }
    return attachment.key.isEmpty && !attachment.url.isEmpty && !attachment.url.hasPrefix("data:")
}

func messageAttachmentIsMedia(_ attachment: MessageAttachment) -> Bool {
    attachment.mimeType.hasPrefix("image/") || attachment.mimeType.hasPrefix("video/")
}

func htmlRemoteImageSourceCount(_ html: String) -> Int {
    let pattern = #"<img\b[^>]*>"#
    guard let regex = try? NSRegularExpression(pattern: pattern, options: [.caseInsensitive]) else {
        return 0
    }
    let nsRange = NSRange(html.startIndex ..< html.endIndex, in: html)
    return regex.matches(in: html, range: nsRange).filter { match in
        guard let range = Range(match.range, in: html) else { return false }
        guard let src = htmlAttributeValue("src", in: String(html[range]))?.trimmingCharacters(in: .whitespacesAndNewlines),
              let url = URL(string: src.lowercased())
        else {
            return false
        }
        return url.scheme == "http" || url.scheme == "https"
    }.count
}

func imageDataUrlPayload(_ url: String) -> Data? {
    guard url.range(of: #"^data:image/[^;,]+;base64,"#, options: [.regularExpression, .caseInsensitive]) != nil,
          let comma = url.firstIndex(of: ",")
    else {
        return nil
    }
    return Data(base64Encoded: String(url[url.index(after: comma)...]))
}

func normalizedPlainMessageText(_ text: String) -> String {
    let compacted = text
        .replacingOccurrences(of: #"\n{3,}"#, with: "\n\n", options: .regularExpression)
        .trimmingCharacters(in: .whitespacesAndNewlines)
    return compacted
        .components(separatedBy: .newlines)
        .map { line in
            line.replacingOccurrences(of: #"^[ \t]*[-*+] +"#, with: "\u{2022} ", options: .regularExpression)
        }
        .joined(separator: "\n")
}

func plainMessageBlocks(_ text: String) -> [PlainMessageBlock] {
    var blocks: [PlainMessageBlock] = []
    var textBuffer: [String] = []
    var codeBuffer: [String] = []
    var inCode = false

    func flushText() {
        let content = normalizedPlainMessageText(textBuffer.joined(separator: "\n").replacingOccurrences(of: #"\n+$"#, with: "", options: .regularExpression))
        textBuffer.removeAll()
        if !content.isEmpty {
            blocks.append(.text(content))
        }
    }

    for line in text.components(separatedBy: .newlines) {
        if line.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
            if inCode {
                blocks.append(.code(codeBuffer.joined(separator: "\n").replacingOccurrences(of: #"\n+$"#, with: "", options: .regularExpression)))
                codeBuffer.removeAll()
                inCode = false
            } else {
                flushText()
                inCode = true
            }
            continue
        }

        if inCode {
            codeBuffer.append(line)
        } else {
            textBuffer.append(line)
        }
    }

    if inCode {
        textBuffer.append("```")
        textBuffer.append(contentsOf: codeBuffer)
    }
    flushText()
    return blocks
}

func normalizePlainMessageUrl(_ value: String) -> String {
    if value.range(of: #"^https?://"#, options: [.regularExpression, .caseInsensitive]) != nil {
        return value
    }
    if value.range(of: #"^[\w.-]+\.[A-Za-z]{2,}(/|$)"#, options: [.regularExpression, .caseInsensitive]) != nil {
        return "https://\(value)"
    }
    return value
}

func shortenedPlainMessageLinkText(_ value: String) -> String {
    let normalized = normalizePlainMessageUrl(value)
    if let url = URL(string: normalized), let host = url.host {
        var display = host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
        let path = url.path
        if !path.isEmpty, path != "/" {
            let suffix = path.count > 24 ? "\(path.prefix(24))..." : path
            display += suffix
        }
        return display
    }
    return value.count > 30 ? "\(value.prefix(30))..." : value
}

func parsePlainMessageInlineParts(_ text: String) -> [PlainMessageInlinePart] {
    guard !text.isEmpty,
          let regex = try? NSRegularExpression(
              pattern: #"(\[[^\]]+\]\([^)]+\)|(?:https?://|www\.)[^\s<>"']+)"#,
              options: [.caseInsensitive]
          )
    else {
        return text.isEmpty ? [] : [.text(text)]
    }

    let nsRange = NSRange(text.startIndex ..< text.endIndex, in: text)
    var parts: [PlainMessageInlinePart] = []
    var cursor = text.startIndex
    for match in regex.matches(in: text, range: nsRange) {
        guard let range = Range(match.range, in: text) else { continue }
        if cursor < range.lowerBound {
            parts.append(.text(String(text[cursor ..< range.lowerBound])))
        }
        let token = String(text[range])
        if let markdown = plainMessageMarkdownLink(token) {
            parts.append(.link(url: normalizePlainMessageUrl(markdown.url), label: markdown.label))
        } else {
            parts.append(.link(url: normalizePlainMessageUrl(token), label: nil))
        }
        cursor = range.upperBound
    }
    if cursor < text.endIndex {
        parts.append(.text(String(text[cursor ..< text.endIndex])))
    }
    return parts
}

func plainMessageBodyLinks(_ text: String) -> [String] {
    var links: [String] = []
    var seen = Set<String>()
    let textBlocks = plainMessageBlocks(text).compactMap { block -> String? in
        if case let .text(content) = block {
            return content
        }
        return nil
    }
    let candidates = textBlocks.isEmpty ? [normalizedPlainMessageText(text)] : textBlocks
    for candidate in candidates {
        for part in parsePlainMessageInlineParts(candidate) {
            guard case let .link(url, _) = part, !seen.contains(url) else { continue }
            seen.insert(url)
            links.append(url)
        }
    }
    return links
}

func plainMessageMarkdownLink(_ value: String) -> (label: String, url: String)? {
    guard let regex = try? NSRegularExpression(pattern: #"^\[([^\]]+)\]\(([^)]+)\)$"#),
          let match = regex.firstMatch(in: value, range: NSRange(value.startIndex ..< value.endIndex, in: value)),
          let labelRange = Range(match.range(at: 1), in: value),
          let urlRange = Range(match.range(at: 2), in: value)
    else {
        return nil
    }
    return (String(value[labelRange]), String(value[urlRange]))
}

func parsePlainMessageStyledRuns(_ text: String) -> [PlainMessageStyledRun] {
    guard !text.isEmpty,
          let regex = try? NSRegularExpression(pattern: #"(`[^`\n]+`|\*\*[^*]+\*\*|\*[^*\n]+\*)"#)
    else {
        return text.isEmpty ? [] : [PlainMessageStyledRun(text: text, style: .normal)]
    }

    let nsRange = NSRange(text.startIndex ..< text.endIndex, in: text)
    var runs: [PlainMessageStyledRun] = []
    var cursor = text.startIndex
    for match in regex.matches(in: text, range: nsRange) {
        guard let range = Range(match.range, in: text) else { continue }
        if cursor < range.lowerBound {
            runs.append(PlainMessageStyledRun(text: String(text[cursor ..< range.lowerBound]), style: .normal))
        }
        let token = String(text[range])
        if token.hasPrefix("**"), token.hasSuffix("**"), token.count >= 4 {
            runs.append(PlainMessageStyledRun(text: String(token.dropFirst(2).dropLast(2)), style: .bold))
        } else if token.hasPrefix("*"), token.hasSuffix("*"), token.count >= 2 {
            runs.append(PlainMessageStyledRun(text: String(token.dropFirst().dropLast()), style: .italic))
        } else if token.hasPrefix("`"), token.hasSuffix("`"), token.count >= 2 {
            runs.append(PlainMessageStyledRun(text: String(token.dropFirst().dropLast()), style: .code))
        } else {
            runs.append(PlainMessageStyledRun(text: token, style: .normal))
        }
        cursor = range.upperBound
    }
    if cursor < text.endIndex {
        runs.append(PlainMessageStyledRun(text: String(text[cursor ..< text.endIndex]), style: .normal))
    }
    return runs
}

func splitPlainMessageSearchRuns(_ text: String, query: String) -> [PlainMessageSearchRun] {
    let normalizedQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty, !normalizedQuery.isEmpty else {
        return text.isEmpty ? [] : [PlainMessageSearchRun(text: text, highlighted: false)]
    }
    var runs: [PlainMessageSearchRun] = []
    var cursor = text.startIndex
    while cursor < text.endIndex,
          let range = text.range(of: normalizedQuery, options: [.caseInsensitive], range: cursor ..< text.endIndex)
    {
        if cursor < range.lowerBound {
            runs.append(PlainMessageSearchRun(text: String(text[cursor ..< range.lowerBound]), highlighted: false))
        }
        runs.append(PlainMessageSearchRun(text: String(text[range]), highlighted: true))
        cursor = range.upperBound
    }
    if cursor < text.endIndex {
        runs.append(PlainMessageSearchRun(text: String(text[cursor ..< text.endIndex]), highlighted: false))
    }
    return runs
}

func appendPlainMessageStyledRuns(_ text: String, searchQuery: String, activeSearchMatch: Bool, to output: inout AttributedString) {
    for run in parsePlainMessageStyledRuns(text) {
        for segment in splitPlainMessageSearchRuns(run.text, query: searchQuery) {
            var attributed = AttributedString(segment.text)
            switch run.style {
            case .normal:
                break
            case .bold:
                attributed.inlinePresentationIntent = .stronglyEmphasized
            case .italic:
                attributed.inlinePresentationIntent = .emphasized
            case .code:
                attributed.inlinePresentationIntent = .code
                attributed.backgroundColor = Color.black.opacity(0.06)
            }
            if segment.highlighted {
                attributed.backgroundColor = activeSearchMatch ? Color.yellow.opacity(0.72) : Color.yellow.opacity(0.34)
                attributed.foregroundColor = activeSearchMatch ? .black : nil
            }
            output += attributed
        }
    }
}

func plainMessageAttributedString(_ text: String, searchQuery: String = "", activeSearchMatch: Bool = false) -> AttributedString {
    var output = AttributedString()
    for part in parsePlainMessageInlineParts(text) {
        switch part {
        case let .text(content):
            appendPlainMessageStyledRuns(content, searchQuery: searchQuery, activeSearchMatch: activeSearchMatch, to: &output)
        case let .link(url, label):
            var link = plainMessageAttributedString(
                label ?? shortenedPlainMessageLinkText(url),
                searchQuery: searchQuery,
                activeSearchMatch: activeSearchMatch
            )
            if let parsed = URL(string: url) {
                link.link = parsed
                link.foregroundColor = .accentColor
            }
            output += link
        }
    }
    return output
}

struct PlainMessageBodyView: View {
    let text: String
    let emptyText: String
    let searchQuery: String
    let activeSearchMatch: Bool
    let onCopyCode: (String) -> Void

    var blocks: [PlainMessageBlock] {
        plainMessageBlocks(text)
    }

    var isEmpty: Bool {
        text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        Group {
            if isEmpty {
                Text(emptyText)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
            } else if blocks.isEmpty {
                Text(plainMessageAttributedString(normalizedPlainMessageText(text), searchQuery: searchQuery, activeSearchMatch: activeSearchMatch))
                    .font(.body)
                    .textSelection(.enabled)
            } else {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                        switch block {
                        case let .text(content):
                            Text(plainMessageAttributedString(content, searchQuery: searchQuery, activeSearchMatch: activeSearchMatch))
                                .font(.body)
                                .foregroundStyle(.primary)
                                .textSelection(.enabled)
                        case let .code(content):
                            PlainMessageCodeBlock(code: content, onCopy: { onCopyCode(content) })
                        }
                    }
                }
            }
        }
        .environment(\.openURL, OpenURLAction { url in
            guard let externalUrl = externalMessageNavigationUrl(url) else {
                return .discarded
            }
            UIApplication.shared.open(externalUrl)
            return .handled
        })
    }
}

struct PlainMessageCodeBlock: View {
    let code: String
    let onCopy: () -> Void

    var body: some View {
        VStack(alignment: .trailing, spacing: 6) {
            Button(action: onCopy) {
                Label(String(localized: "chat.copyCode"), systemImage: "doc.on.doc")
                    .labelStyle(.iconOnly)
            }
            .buttonStyle(.borderless)
            .accessibilityLabel(String(localized: "chat.copyCode"))

            ScrollView(.horizontal, showsIndicators: true) {
                Text(code.isEmpty ? " " : code)
                    .font(.system(.footnote, design: .monospaced))
                    .foregroundStyle(.primary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
            }
            .background(Color.black.opacity(0.06))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }
}

struct MessageMetadataAddressRow: View {
    let row: MessageMetadataRow
    let onCopy: (String, String) -> Void

    var addresses: [MessageAddressItem] {
        messageAddressItems(row.rawValue)
    }

    var body: some View {
        if !addresses.isEmpty {
            Grid(alignment: .leadingFirstTextBaseline, horizontalSpacing: 6, verticalSpacing: 4) {
                GridRow {
                    Text(row.label)
                        .font(.caption2.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .textCase(.uppercase)
                        .frame(width: 54, alignment: .leading)
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(Array(addresses.enumerated()), id: \.offset) { _, address in
                            Button {
                                onCopy(row.label, address.original)
                            } label: {
                                Text(address.name)
                                    .font(.caption2.weight(.medium))
                                    .lineLimit(1)
                                    .padding(.vertical, 3)
                                    .padding(.horizontal, 6)
                                    .background(Color.accentColor.opacity(0.10))
                                    .clipShape(RoundedRectangle(cornerRadius: 6))
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(.primary)
                            .accessibilityLabel(address.original)
                        }
                    }
                }
            }
        }
    }
}

struct InlineImageAttachmentGrid: View {
    let attachments: [MessageAttachment]
    let allowRemoteImages: Bool
    let onOpenAttachment: (MessageAttachment) -> Void
    @State private var cachedImages: [String: UIImage] = [:]

    var body: some View {
        LazyVGrid(columns: columns, spacing: 4) {
            ForEach(Array(attachments.enumerated()), id: \.offset) { _, attachment in
                Button {
                    onOpenAttachment(attachment)
                } label: {
                    InlineImageAttachmentTile(
                        attachment: attachment,
                        allowRemoteImages: allowRemoteImages,
                        cachedImage: attachment.key.isEmpty ? nil : cachedImages[attachment.key]
                    )
                }
                .buttonStyle(.plain)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .task(id: attachments.map(\.key).joined(separator: "|")) {
            loadCachedImages()
        }
    }

    var columns: [GridItem] {
        let count = min(max(attachments.count, 1), 3)
        return Array(repeating: GridItem(.flexible(), spacing: 4), count: count)
    }

    func loadCachedImages() {
        for attachment in attachments where !attachment.key.isEmpty && cachedImages[attachment.key] == nil {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.attachmentReadRequest(
                    id: 66,
                    params: AttachmentReadParams(key: attachment.key)
                ).toJson()
            )
            guard !response.contains(#""error""#) else { continue }
            let base64 = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
            guard let data = Data(base64Encoded: base64),
                  let image = UIImage(data: data)
            else { continue }
            cachedImages[attachment.key] = image
        }
    }
}

struct InlineImageAttachmentTile: View {
    let attachment: MessageAttachment
    let allowRemoteImages: Bool
    let cachedImage: UIImage?

    var body: some View {
        ZStack {
            if let cachedImage {
                Image(uiImage: cachedImage)
                    .resizable()
                    .scaledToFill()
            } else if allowRemoteImages, !attachment.url.isEmpty, let url = URL(string: attachment.url) {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case let .success(image):
                        image
                            .resizable()
                            .scaledToFill()
                    default:
                        imagePlaceholder
                    }
                }
            } else {
                imagePlaceholder
            }
        }
        .frame(minHeight: 96, maxHeight: 160)
        .aspectRatio(1.15, contentMode: .fit)
        .frame(maxWidth: .infinity)
        .clipped()
        .background(Color(.tertiarySystemGroupedBackground))
        .overlay(alignment: .bottomLeading) {
            Text(attachment.filename.isEmpty ? String(localized: "mobile.ios.image") : attachment.filename)
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.white)
                .lineLimit(1)
                .padding(.horizontal, 6)
                .padding(.vertical, 4)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.black.opacity(0.45))
        }
        .accessibilityLabel(attachment.filename.isEmpty ? String(localized: "mobile.ios.image") : attachment.filename)
    }

    var imagePlaceholder: some View {
        VStack(spacing: 6) {
            Image(systemName: "photo")
                .font(.title2)
            Text(attachmentActionHint(attachment))
                .font(.caption2)
                .multilineTextAlignment(.center)
                .lineLimit(2)
        }
        .foregroundStyle(.secondary)
        .padding(8)
    }
}

struct InlineVideoAttachmentList: View {
    let attachments: [MessageAttachment]
    let allowRemoteImages: Bool
    let onOpenAttachment: (MessageAttachment) -> Void
    @State private var cachedVideoUrls: [String: URL] = [:]

    var body: some View {
        VStack(spacing: 6) {
            ForEach(Array(attachments.enumerated()), id: \.offset) { _, attachment in
                InlineVideoAttachmentPlayer(
                    attachment: attachment,
                    allowRemoteImages: allowRemoteImages,
                    cachedUrl: attachment.key.isEmpty ? nil : cachedVideoUrls[attachment.key],
                    onOpenAttachment: onOpenAttachment
                )
            }
        }
        .task(id: attachments.map(\.key).joined(separator: "|")) {
            loadCachedVideos()
        }
    }

    func loadCachedVideos() {
        for attachment in attachments where !attachment.key.isEmpty && cachedVideoUrls[attachment.key] == nil {
            let response = RustCoreBridge.invokeJson(
                MobileCommandsKt.attachmentReadRequest(
                    id: 67,
                    params: AttachmentReadParams(key: attachment.key)
                ).toJson()
            )
            guard !response.contains(#""error""#) else { continue }
            let base64 = MobileResponseParsersKt.parseAttachmentDataResponse(responseJson: response)
            guard let data = Data(base64Encoded: base64), !data.isEmpty else { continue }
            do {
                let directory = FileManager.default.temporaryDirectory.appendingPathComponent("MeronInlineVideos", isDirectory: true)
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                let filename = safeAttachmentFilename(attachment.filename.isEmpty ? "\(attachment.key).mp4" : attachment.filename)
                let url = directory.appendingPathComponent(filename)
                try data.write(to: url, options: .atomic)
                cachedVideoUrls[attachment.key] = url
            } catch {
                continue
            }
        }
    }
}

struct InlineVideoAttachmentPlayer: View {
    let attachment: MessageAttachment
    let allowRemoteImages: Bool
    let cachedUrl: URL?
    let onOpenAttachment: (MessageAttachment) -> Void

    var playerUrl: URL? {
        if let cachedUrl {
            return cachedUrl
        }
        if allowRemoteImages, !attachment.url.isEmpty {
            return URL(string: attachment.url)
        }
        return nil
    }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            if let playerUrl {
                VideoPlayer(player: AVPlayer(url: playerUrl))
                    .frame(minHeight: 150, maxHeight: 240)
                    .background(Color.black)
            } else {
                videoPlaceholder
                    .frame(minHeight: 120)
                    .frame(maxWidth: .infinity)
                    .background(Color(.tertiarySystemGroupedBackground))
            }
            Button {
                onOpenAttachment(attachment)
            } label: {
                Label(String(localized: "chat.openExternalPlayer"), systemImage: "arrow.up.forward.app")
                    .labelStyle(.iconOnly)
                    .font(.caption)
                    .padding(7)
                    .background(.black.opacity(0.55), in: Circle())
                    .foregroundStyle(.white)
            }
            .buttonStyle(.plain)
            .padding(6)
        }
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay(alignment: .bottomLeading) {
            Text(attachment.filename.isEmpty ? String(localized: "chat.attachment") : attachment.filename)
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.white)
                .lineLimit(1)
                .padding(.horizontal, 6)
                .padding(.vertical, 4)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.black.opacity(0.45))
        }
        .accessibilityLabel(attachment.filename.isEmpty ? String(localized: "chat.attachment") : attachment.filename)
    }

    var videoPlaceholder: some View {
        VStack(spacing: 6) {
            Image(systemName: "video")
                .font(.title2)
            Text(attachmentActionHint(attachment))
                .font(.caption2)
                .multilineTextAlignment(.center)
                .lineLimit(2)
        }
        .foregroundStyle(.secondary)
        .padding(8)
    }
}

struct ConversationDetailsDisclosure: View {
    let participants: [ConversationParticipant]
    let attachments: [MessageAttachment]
    let onCopyEmail: (String) -> Void
    let onComposeTo: (ConversationParticipant) -> Void
    let onOpenAttachment: (MessageAttachment) -> Void
    let onSaveAttachment: (MessageAttachment) -> Void
    @State private var isExpanded = true

    var mediaAttachments: [MessageAttachment] {
        attachments.filter { $0.mimeType.hasPrefix("image/") || $0.mimeType.hasPrefix("video/") }
    }

    var fileAttachments: [MessageAttachment] {
        attachments.filter { !$0.mimeType.hasPrefix("image/") && !$0.mimeType.hasPrefix("video/") }
    }

    var body: some View {
        DisclosureGroup(isExpanded: $isExpanded) {
            HStack {
                ConversationDetailStat(label: String(localized: "chat.peopleLabel"), value: "\(participants.count)")
                ConversationDetailStat(label: String(localized: "chat.media"), value: "\(mediaAttachments.count)")
                ConversationDetailStat(label: String(localized: "chat.files"), value: "\(fileAttachments.count)")
            }
            .padding(.vertical, 4)

            if participants.isEmpty {
                Text(String(localized: "chat.noConversationParticipants"))
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
                            Text("\(person.email) · \(person.count)\(person.isSelf ? " · \(String(localized: "chat.you").lowercased())" : "")")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                        Spacer()
                        Menu {
                            Button(String(localized: "chat.copyEmailAddress")) {
                                onCopyEmail(person.email)
                            }
                            if !person.isSelf {
                                Button(String(localized: "mobile.tabs.compose")) {
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
                Text(String(localized: "chat.noSharedFilesLoaded"))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ConversationAttachmentSection(
                    title: String(localized: "chat.media"),
                    emptyMessage: String(localized: "chat.noMedia"),
                    attachments: mediaAttachments,
                    onOpenAttachment: onOpenAttachment,
                    onSaveAttachment: onSaveAttachment
                )

                ConversationAttachmentSection(
                    title: String(localized: "chat.files"),
                    emptyMessage: String(localized: "chat.noFiles"),
                    attachments: fileAttachments,
                    onOpenAttachment: onOpenAttachment,
                    onSaveAttachment: onSaveAttachment
                )
            }
        } label: {
            Label(String(localized: "chat.showDetails"), systemImage: "info.circle")
        }
    }

}

struct ConversationAttachmentSection: View {
    let title: String
    let emptyMessage: String
    let attachments: [MessageAttachment]
    let onOpenAttachment: (MessageAttachment) -> Void
    let onSaveAttachment: (MessageAttachment) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)

            if attachments.isEmpty {
                Text(emptyMessage)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(Array(attachments.enumerated()), id: \.offset) { _, attachment in
                    ConversationAttachmentRow(
                        attachment: attachment,
                        onOpenAttachment: onOpenAttachment,
                        onSaveAttachment: onSaveAttachment
                    )
                }
            }
        }
        .padding(.vertical, 3)
    }
}

struct ConversationAttachmentRow: View {
    let attachment: MessageAttachment
    let onOpenAttachment: (MessageAttachment) -> Void
    let onSaveAttachment: (MessageAttachment) -> Void

    var body: some View {
        HStack(spacing: 8) {
            Button {
                onOpenAttachment(attachment)
            } label: {
                Label {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(attachment.filename.isEmpty ? String(localized: "chat.attachment") : attachment.filename)
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
            if attachmentCanSave(attachment) {
                Button {
                    onSaveAttachment(attachment)
                } label: {
                    Label(String(localized: "buttons.save"), systemImage: "square.and.arrow.down")
                        .labelStyle(.iconOnly)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(String(localized: "buttons.save"))
            }
        }
        .padding(.vertical, 3)
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
        starredItemDeleteActionLabel(folder: item.folder)
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
                            Button(item.unread ? String(localized: "threads.actions.markAsRead") : String(localized: "threads.actions.markAsUnread"), action: onToggleRead)
                            Button(String(localized: "chat.unstar"), action: onUnstar)
                            if !isRssItem {
                                Button(deleteLabel, role: .destructive, action: onDelete)
                            }
                        } label: {
                            Image(systemName: "ellipsis.circle")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        .buttonStyle(.borderless)
                        .accessibilityLabel(String(localized: "starred.itemActions"))
                        Text(relativeTime(item.dateEpochSeconds))
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                    Text(item.subject.isEmpty ? String(localized: "threads.noSubject") : item.subject)
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
            Button(String(localized: "chat.unstar"), action: onUnstar)
                .tint(.orange)
            Button(item.unread ? String(localized: "common.read") : String(localized: "common.unread"), action: onToggleRead)
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
    let moveTargets: (ThreadSummary) -> [IosKanbanColumnSpec]
    let targetTitle: (IosKanbanColumnSpec) -> String
    let onRefresh: () -> Void
    let onLoadMore: () -> Void
    let onMarkAllRead: () -> Void
    let onRemoveColumn: () -> Void
    let canMoveColumnLeft: Bool
    let canMoveColumnRight: Bool
    let onMoveColumnLeft: () -> Void
    let onMoveColumnRight: () -> Void
    let onMinimizeColumn: () -> Void
    let onSearchColumn: () -> Void
    let onOpen: (ThreadSummary) -> Void
    let onArchive: (ThreadSummary) -> Void
    let onDelete: (ThreadSummary) -> Void
    let onCopyFeedUrl: (ThreadSummary) -> Void
    let onToggleRead: (ThreadSummary) -> Void
    let onToggleStar: (ThreadSummary) -> Void
    let onMove: (ThreadSummary, IosKanbanColumnSpec) -> Void
    let isRssFeed: (ThreadSummary) -> Bool
    let selectedThreadIds: Set<String>
    let selectionActive: Bool
    let onToggleSelected: (ThreadSummary) -> Void
    let onLongPress: (ThreadSummary) -> Void
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
                Button(action: onMinimizeColumn) {
                    Image(systemName: "minus")
                }
                .buttonStyle(.borderless)
                Menu {
                    Button(String(localized: "kanban.actions.searchColumn"), action: onSearchColumn)
                    Button(String(localized: "mobile.actions.refresh"), action: onRefresh)
                    Button(String(localized: "threads.actions.markAllAsRead"), action: onMarkAllRead)
                        .disabled(!threads.contains { $0.unread })
                    Divider()
                    Button(String(localized: "kanban.actions.moveColumnLeft"), action: onMoveColumnLeft)
                        .disabled(!canMoveColumnLeft)
                    Button(String(localized: "kanban.actions.moveColumnRight"), action: onMoveColumnRight)
                        .disabled(!canMoveColumnRight)
                    Divider()
                    Button(String(localized: "kanban.actions.minimizeColumn"), action: onMinimizeColumn)
                    Button(kanbanColumnHideActionLabel(), role: .destructive, action: onRemoveColumn)
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }

            if threads.isEmpty {
                Text(String(localized: "mobile.ios.noCachedItems"))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 12)
            } else {
                ForEach(threads, id: \.id) { thread in
                    KanbanThreadCard(
                        thread: thread,
                        moveTargets: moveTargets(thread),
                        targetTitle: targetTitle,
                        showSenderImages: showSenderImages,
                        onOpen: { onOpen(thread) },
                        onArchive: { onArchive(thread) },
                        onDelete: { onDelete(thread) },
                        onCopyFeedUrl: { onCopyFeedUrl(thread) },
                        onToggleRead: { onToggleRead(thread) },
                        onToggleStar: { onToggleStar(thread) },
                        onMove: { target in onMove(thread, target) },
                        isRssFeed: isRssFeed(thread),
                        selected: selectedThreadIds.contains(thread.id),
                        selectionActive: selectionActive,
                        onToggleSelected: { onToggleSelected(thread) },
                        onLongPress: { onLongPress(thread) }
                    )
                }
                if canLoadMore || isLoadingMore {
                    Button {
                        onLoadMore()
                    } label: {
                        if isLoadingMore {
                            Label(String(localized: "mobile.ios.loadingOlder"), systemImage: "hourglass")
                        } else {
                            Label(String(localized: "mobile.ios.loadOlder"), systemImage: "chevron.down")
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

struct KanbanMinimizedColumnView: View {
    let title: String
    let unreadCount: Int
    let onRestore: () -> Void

    var body: some View {
        Button(action: onRestore) {
            VStack(spacing: 10) {
                Text(avatarInitials(title))
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.white)
                    .frame(width: 28, height: 28)
                    .background(Color.accentColor)
                    .clipShape(Circle())
                VStack(spacing: 2) {
                    ForEach(titleWords, id: \.self) { word in
                        Text(word)
                            .font(.caption2.weight(.semibold))
                            .lineLimit(1)
                    }
                }
                if unreadCount > 0 {
                    Text("\(unreadCount)")
                        .font(.caption2.weight(.bold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.accentColor)
                        .clipShape(Capsule())
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 10)
            .frame(width: 58)
            .frame(minHeight: 180, maxHeight: .infinity, alignment: .top)
            .background(Color(.secondarySystemGroupedBackground))
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(String(localized: "kanban.actions.expand"))
    }

    var titleWords: [String] {
        let words = title
            .split(whereSeparator: { $0.isWhitespace })
            .map(String.init)
            .prefix(3)
        return words.isEmpty ? [title] : Array(words)
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
    let onCopyFeedUrl: () -> Void
    let onToggleRead: () -> Void
    let onToggleStar: () -> Void
    let onMove: (IosKanbanColumnSpec) -> Void
    let isRssFeed: Bool
    let selected: Bool
    let selectionActive: Bool
    let onToggleSelected: () -> Void
    let onLongPress: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Button(action: selectionActive ? onToggleSelected : onOpen) {
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 6) {
                        if selectionActive {
                            Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                                .font(.system(size: 24, weight: .semibold))
                                .foregroundStyle(selected ? Color.accentColor : Color.secondary)
                                .frame(width: 26, height: 26)
                                .accessibilityLabel(selected ? "Selected" : "Not selected")
                        } else {
                            SenderAvatar(label: thread.sender.isEmpty ? thread.accountId : thread.sender, enabled: showSenderImages, size: 26)
                        }
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
                    Text(thread.subject.isEmpty ? String(localized: "threads.noSubject") : thread.subject)
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
                if selectionActive {
                    Button(action: onToggleSelected) {
                        Image(systemName: selected ? "checkmark.circle.fill" : "circle")
                    }
                    .accessibilityLabel(selected ? "Selected" : "Not selected")
                } else {
                    Button(action: onToggleRead) {
                        Image(systemName: thread.unread ? "envelope.open" : "envelope.badge")
                    }
                    Button(action: onToggleStar) {
                        Image(systemName: thread.starred ? "star.slash" : "star")
                    }
                    Menu {
                        if isRssFeed, !thread.feedUrl.isEmpty {
                            Button(String(localized: "feeds.copyUrl"), action: onCopyFeedUrl)
                        }
                        Button(isRssFeed ? String(localized: "feeds.actions.deleteFeed") : String(localized: "threads.actions.archiveThread"), action: onArchive)
                        if !isRssFeed {
                            Button(threadDeleteActionLabel(thread), role: .destructive, action: onDelete)
                        }
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
                }
                Spacer()
            }
            .buttonStyle(.borderless)
        }
        .padding(10)
        .background(selected ? Color.accentColor.opacity(0.12) : Color(.systemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .onLongPressGesture(perform: onLongPress)
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

func messageFullTimestamp(
    _ epochSeconds: Int64,
    locale: Locale = .autoupdatingCurrent,
    timeZone: TimeZone = .autoupdatingCurrent
) -> String {
    guard epochSeconds > 0 else { return "" }
    let formatter = DateFormatter()
    formatter.locale = locale
    formatter.timeZone = timeZone
    formatter.setLocalizedDateFormatFromTemplate("EEE y MMM d HH:mm")
    return formatter.string(from: Date(timeIntervalSince1970: TimeInterval(epochSeconds)))
}

func conversationDateDividerLabel(
    _ epochSeconds: Int64,
    referenceDate: Date = Date(),
    calendar: Calendar = .autoupdatingCurrent
) -> String {
    guard epochSeconds > 0 else { return "" }
    let date = Date(timeIntervalSince1970: TimeInterval(epochSeconds))
    if calendar.isDate(date, inSameDayAs: referenceDate) {
        return "Today"
    }
    if let yesterday = calendar.date(byAdding: .day, value: -1, to: referenceDate),
       calendar.isDate(date, inSameDayAs: yesterday)
    {
        return "Yesterday"
    }
    let formatter = DateFormatter()
    formatter.dateStyle = .medium
    formatter.timeStyle = .none
    return formatter.string(from: date)
}

struct ConversationDateDivider: View {
    let label: String

    var body: some View {
        Text(label.uppercased())
            .font(.caption2.weight(.semibold))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(Color(.tertiarySystemGroupedBackground))
            .clipShape(Capsule())
            .frame(maxWidth: .infinity)
            .textSelection(.disabled)
    }
}

func attachmentDetail(_ attachment: MessageAttachment) -> String {
    let size = attachment.sizeBytes > 0 ? ByteCountFormatter.string(fromByteCount: attachment.sizeBytes, countStyle: .file) : ""
    return [attachment.mimeType, size].filter { !$0.isEmpty }.joined(separator: " · ")
}

func attachmentActionHint(_ attachment: MessageAttachment) -> String {
    if attachment.url.isEmpty, attachment.key.isEmpty {
        return String(localized: "mobile.ios.attachmentNotCached")
    }
    if attachment.url.isEmpty, !attachment.mimeType.hasPrefix("image/") {
        return String(localized: "mobile.ios.attachmentShareOrSave")
    }
    if attachment.mimeType.hasPrefix("image/") {
        return String(localized: "mobile.ios.attachmentPreviewCopyShareSave")
    }
    return String(localized: "mobile.ios.attachmentOpenExternally")
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

func attachmentCanSave(_ attachment: MessageAttachment) -> Bool {
    !attachment.key.isEmpty
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
    return text.isEmpty ? String(localized: "mobile.ios.noContent") : text
}

func messagesAfterDeletingMessage(_ messages: [MessageBody], messageId: String) -> [MessageBody] {
    messages.filter { $0.id != messageId }
}

func messageDeleteRequiresConfirmation(folder: String) -> Bool {
    true
}

func messageDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "chat.actions.discardDraft")
    }
    return String(localized: "chat.actions.deleteMessage")
}

func messageDeleteConfirmationTitle(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "mobile.compose.discardDraftTitle")
    }
    return messageDeleteActionLabel(folder: folder)
}

func messageDeleteConfirmationMessage(_ message: MessageBody, folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "mobile.compose.discardDraftText")
    }
    return message.subject.isEmpty ? String(localized: "threads.noSubject") : message.subject
}

func draftAttachmentSizeLabel(_ attachment: DraftAttachment) -> String {
    ByteCountFormatter.string(fromByteCount: attachment.sizeBytes, countStyle: .file)
}

func starredItemDeleteActionLabel(folder: String) -> String {
    if MailStateKt.folderIsDrafts(folder: folder) {
        return String(localized: "chat.actions.discardDraft")
    }
    return String(localized: "chat.actions.deleteMessage")
}

func starredItemDeleteConfirmationMessage(_ item: StarredItemSummary) -> String {
    if MailStateKt.folderIsDrafts(folder: item.folder) {
        return String(localized: "mobile.compose.discardDraftText")
    }
    return item.subject.isEmpty ? String(localized: "threads.noSubject") : item.subject
}

func composeDraftHasContent(
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    attachments: [DraftAttachment]
) -> Bool {
    !to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
        !cc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
        !bcc.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
        !subject.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
        !body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ||
        !attachments.isEmpty
}

func composeDraftCanSend(
    to: String,
    subject: String,
    body: String,
    attachments: [DraftAttachment]
) -> Bool {
    !to.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty &&
        (!body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !attachments.isEmpty)
}

func composeDraftCanSubmit(
    identityAvailable: Bool,
    to: String,
    subject: String,
    body: String,
    attachments: [DraftAttachment],
    sending: Bool
) -> Bool {
    identityAvailable &&
        !sending &&
        composeDraftCanSend(to: to, subject: subject, body: body, attachments: attachments)
}

func composeDraftNeedsNoSubjectConfirmation(subject: String) -> Bool {
    subject.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
}

func composeDraftAutosaveSignature(
    accountId: String,
    fromEmail: String,
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    body: String,
    rich: Bool,
    replyTo: String = "",
    inReplyTo: String,
    references: String,
    inlineAttachmentIds: Set<String>,
    attachments: [DraftAttachment]
) -> String {
    let attachmentSignature = attachments
        .map { attachment in
            [
                attachment.id,
                attachment.displayName,
                attachment.mimeType,
                "\(attachment.sizeBytes)",
                inlineAttachmentIds.contains(attachment.id) ? "inline" : "file",
            ].joined(separator: "\u{1F}")
        }
        .joined(separator: "\u{1E}")
    return [
        accountId,
        fromEmail,
        to,
        cc,
        bcc,
        subject,
        body,
        rich ? "rich" : "plain",
        replyTo,
        inReplyTo,
        references,
        attachmentSignature,
    ].joined(separator: "\u{1D}")
}

func iosFullReplyShortcutCanOpen(selectedTab: IosAppTab, thread: ThreadSummary?) -> Bool {
    guard selectedTab == .mail, let thread else { return false }
    return !MailStateKt.threadIdIsRss(threadId: thread.id)
}

func iosComposeReturnTab(from selectedTab: IosAppTab) -> IosAppTab {
    switch selectedTab {
    case .kanban, .starred:
        return selectedTab
    case .mail, .compose, .accounts:
        return .mail
    }
}

func iosCommandPaletteAdjacentThreadSource(
    selectedTab: IosAppTab,
    mailThreads: [ThreadSummary],
    kanbanThreads: [ThreadSummary]
) -> [ThreadSummary] {
    selectedTab == .kanban ? kanbanThreads : mailThreads
}

func visibleKanbanThreadSummaries(
    board: IosKanbanBoardSpec?,
    threadsByColumn: [String: [ThreadSummary]]
) -> [ThreadSummary] {
    guard let board else { return [] }
    var seen: Set<String> = []
    return board.columns.flatMap { column in
        threadsByColumn[column.id] ?? []
    }.filter { thread in
        seen.insert(thread.id).inserted
    }
}

func adjacentThreadSummary(_ threads: [ThreadSummary], currentId: String?, delta: Int) -> ThreadSummary? {
    guard !threads.isEmpty else { return nil }
    guard let currentId,
          let currentIndex = threads.firstIndex(where: { $0.id == currentId })
    else {
        return threads.first
    }
    let nextIndex = min(threads.count - 1, max(0, currentIndex + delta))
    return threads[nextIndex]
}

func iosPreferredAccountAfterAccountList(
    accounts: [AccountSummary],
    preferredEmail: String?,
    previousAccountIds: Set<String>?
) -> AccountSummary? {
    let email = preferredEmail?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() ?? ""
    if !email.isEmpty,
       let account = accounts.first(where: { $0.email.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == email })
    {
        return account
    }
    guard let previousAccountIds else { return nil }
    return accounts.first { !previousAccountIds.contains($0.id) }
}

func selectedThreadsMarkReadTarget(_ threads: [ThreadSummary]) -> Bool {
    threads.contains(where: \.unread)
}

func selectedThreadsStarTarget(_ threads: [ThreadSummary]) -> Bool {
    threads.contains { !$0.starred }
}

func firstRssThread(_ threads: [ThreadSummary]) -> ThreadSummary? {
    threads.first { MailStateKt.threadIdIsRss(threadId: $0.id) }
}

func selectedThreadsPartitionedForArchiveOrRemove(_ threads: [ThreadSummary]) -> (mail: [ThreadSummary], rss: [ThreadSummary]) {
    (
        mail: threads.filter { !MailStateKt.threadIdIsRss(threadId: $0.id) },
        rss: threads.filter { MailStateKt.threadIdIsRss(threadId: $0.id) }
    )
}

func rssFeedDeleteConfirmationMessage(_ thread: ThreadSummary) -> String {
    let title = thread.subject.isEmpty ? String(localized: "feeds.fallbackName") : thread.subject
    return "\(title)\n\n\(String(localized: "feeds.deleteHint"))"
}

func iosConversationDeleteCommandLabel(_ thread: ThreadSummary) -> String {
    MailStateKt.threadIdIsRss(threadId: thread.id) ? String(localized: "feeds.actions.deleteFeed") : threadDeleteActionLabel(thread)
}

func kanbanThreadSummary(for item: StarredItemSummary) -> ThreadSummary {
    ThreadSummary(
        id: item.id,
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
}

func starredItemReaderMessage(for item: StarredItemSummary) -> MessageBody {
    MessageBody(
        id: item.id,
        from: item.sender,
        to: "",
        cc: "",
        bcc: "",
        subject: item.subject,
        body: item.preview,
        bodyHtml: "",
        dateEpochSeconds: item.dateEpochSeconds,
        fromAddr: item.sender,
        replyTo: "",
        messageId: "",
        references: "",
        unread: item.unread,
        starred: true,
        hasAttachments: false,
        attachments: []
    )
}

func starredKanbanReaderMessage(for thread: ThreadSummary) -> MessageBody {
    MessageBody(
        id: thread.id,
        from: thread.sender,
        to: "",
        cc: "",
        bcc: "",
        subject: thread.subject,
        body: thread.preview,
        bodyHtml: "",
        dateEpochSeconds: thread.dateEpochSeconds,
        fromAddr: thread.sender,
        replyTo: "",
        messageId: "",
        references: "",
        unread: thread.unread,
        starred: thread.starred,
        hasAttachments: false,
        attachments: []
    )
}

enum StarredItemListUpdate {
    case read(seen: Bool)
    case remove
}

func starredItemsAfterAction(
    _ items: [StarredItemSummary],
    itemId: String,
    update: StarredItemListUpdate
) -> [StarredItemSummary] {
    switch update {
    case let .read(seen):
        return items.map { item in
            guard item.id == itemId else { return item }
            return StarredItemSummary(
                id: item.id,
                threadId: item.threadId,
                accountId: item.accountId,
                folder: item.folder,
                subject: item.subject,
                sender: item.sender,
                preview: item.preview,
                unread: !seen,
                dateEpochSeconds: item.dateEpochSeconds
            )
        }
    case .remove:
        return items.filter { $0.id != itemId }
    }
}

func sortedStarredItems(_ items: [StarredItemSummary]) -> [StarredItemSummary] {
    items.sorted { lhs, rhs in
        if lhs.dateEpochSeconds != rhs.dateEpochSeconds {
            return lhs.dateEpochSeconds > rhs.dateEpochSeconds
        }
        return lhs.id < rhs.id
    }
}

func starredItemsAfterThreadReadState(
    _ items: [StarredItemSummary],
    threadId: String,
    seen: Bool
) -> [StarredItemSummary] {
    items.map { item in
        guard item.threadId == threadId else { return item }
        return StarredItemSummary(
            id: item.id,
            threadId: item.threadId,
            accountId: item.accountId,
            folder: item.folder,
            subject: item.subject,
            sender: item.sender,
            preview: item.preview,
            unread: !seen,
            dateEpochSeconds: item.dateEpochSeconds
        )
    }
}

func starredItemsAfterMessageStarred(
    _ items: [StarredItemSummary],
    message: MessageBody,
    thread: ThreadSummary,
    starred: Bool
) -> [StarredItemSummary] {
    if !starred {
        return starredItemsAfterAction(items, itemId: message.id, update: .remove)
    }
    let item = StarredItemSummary(
        id: message.id,
        threadId: thread.id,
        accountId: thread.accountId,
        folder: thread.folder,
        subject: message.subject,
        sender: message.from.isEmpty ? thread.sender : message.from,
        preview: messagePlainText(message),
        unread: message.unread,
        dateEpochSeconds: message.dateEpochSeconds
    )
    let withoutExisting = items.filter { $0.id != message.id }
    return sortedStarredItems(withoutExisting + [item])
}

func starredItemsAfterThreadStarred(
    _ items: [StarredItemSummary],
    thread: ThreadSummary,
    messages: [MessageBody],
    starred: Bool
) -> [StarredItemSummary] {
    if !starred {
        return items.filter { $0.threadId != thread.id }
    }
    let messageItems = messages.map { message in
        StarredItemSummary(
            id: message.id,
            threadId: thread.id,
            accountId: thread.accountId,
            folder: thread.folder,
            subject: message.subject,
            sender: message.from.isEmpty ? thread.sender : message.from,
            preview: messagePlainText(message),
            unread: message.unread,
            dateEpochSeconds: message.dateEpochSeconds
        )
    }
    let messageIds = Set(messageItems.map(\.id))
    return sortedStarredItems(items.filter { !messageIds.contains($0.id) } + messageItems)
}

func starredKanbanThreadsAfterAction(
    _ threads: [ThreadSummary],
    itemId: String,
    update: StarredItemListUpdate
) -> [ThreadSummary] {
    switch update {
    case let .read(seen):
        return threads.map { thread in
            thread.id == itemId ? thread.withUnread(!seen) : thread
        }
    case .remove:
        return threads.filter { $0.id != itemId }
    }
}

func starredKanbanThreadsAfterThreadReadState(
    _ threads: [ThreadSummary],
    threadId: String,
    threadIdsByItemId: [String: String],
    seen: Bool
) -> [ThreadSummary] {
    threads.map { thread in
        let mappedThreadId = threadIdsByItemId[thread.id] ?? thread.id
        return mappedThreadId == threadId ? thread.withUnread(!seen) : thread
    }
}

func starredKanbanThreadsAfterThreadStarred(
    _ threads: [ThreadSummary],
    threadId: String,
    threadIdsByItemId: [String: String],
    starred: Bool
) -> [ThreadSummary] {
    guard !starred else { return threads }
    return threads.filter { thread in
        let mappedThreadId = threadIdsByItemId[thread.id] ?? thread.id
        return mappedThreadId != threadId
    }
}

func kanbanThreadsAfterThreadFlagUpdate(
    _ threads: [ThreadSummary],
    threadId: String,
    unread: Bool? = nil,
    starred: Bool? = nil
) -> [ThreadSummary] {
    threads.map { thread in
        thread.id == threadId ? thread.withFlags(unread: unread, starred: starred) : thread
    }
}

func kanbanThreadsAfterRemovingThread(_ threads: [ThreadSummary], threadId: String) -> [ThreadSummary] {
    threads.filter { $0.id != threadId }
}

func selectedThreadIdsAfterRemovingThread(_ selectedIds: Set<String>, threadId: String) -> Set<String> {
    var next = selectedIds
    next.remove(threadId)
    return next
}

func threadsAfterMarkingThreadIdsRead(_ threads: [ThreadSummary], threadIds: Set<String>) -> [ThreadSummary] {
    threads.map { thread in
        threadIds.contains(thread.id) ? thread.withUnread(false) : thread
    }
}

func messagesAfterThreadReadState(_ messages: [MessageBody], unread: Bool) -> [MessageBody] {
    messages.map { $0.withFlags(unread: unread) }
}

func messagesAfterThreadStarredState(_ messages: [MessageBody], starred: Bool) -> [MessageBody] {
    messages.map { $0.withFlags(starred: starred) }
}

struct KanbanStarredItemActionTarget: Equatable {
    let itemId: String
    let threadId: String
    let isRss: Bool
}

func kanbanStarredItemActionTarget(
    for thread: ThreadSummary,
    threadIdsByItemId: [String: String]
) -> KanbanStarredItemActionTarget {
    let threadId = threadIdsByItemId[thread.id] ?? thread.id
    return KanbanStarredItemActionTarget(
        itemId: thread.id,
        threadId: threadId,
        isRss: MailStateKt.threadIdIsRss(threadId: threadId)
    )
}

func kanbanDeleteIsStarredMessage(column: IosKanbanColumnSpec) -> Bool {
    column.accountId == iosUnifiedAccountId && column.folderId.caseInsensitiveCompare(iosStarredFolderId) == .orderedSame
}

func kanbanDeleteRequiresConfirmation(thread: ThreadSummary, in column: IosKanbanColumnSpec) -> Bool {
    kanbanDeleteIsStarredMessage(column: column)
        ? messageDeleteRequiresConfirmation(folder: thread.folder)
        : threadDeleteRequiresConfirmation(folder: thread.folder)
}

func kanbanDeleteActionLabel(_ target: IosKanbanDeleteTarget?) -> String {
    guard let target else { return String(localized: "buttons.delete") }
    return kanbanDeleteIsStarredMessage(column: target.column)
        ? messageDeleteActionLabel(folder: target.thread.folder)
        : threadDeleteActionLabel(target.thread)
}

func kanbanDeleteConfirmationTitle(_ target: IosKanbanDeleteTarget?) -> String {
    guard let target else { return String(localized: "buttons.delete") }
    return kanbanDeleteIsStarredMessage(column: target.column)
        ? messageDeleteConfirmationTitle(folder: target.thread.folder)
        : threadDeleteConfirmationTitle(target.thread)
}

func kanbanDeleteConfirmationMessage(_ target: IosKanbanDeleteTarget) -> String {
    if kanbanDeleteIsStarredMessage(column: target.column) {
        if MailStateKt.folderIsDrafts(folder: target.thread.folder) {
            return String(localized: "mobile.compose.discardDraftText")
        }
        return target.thread.subject.isEmpty ? String(localized: "threads.noSubject") : target.thread.subject
    }
    return threadDeleteConfirmationMessage(target.thread)
}

enum IosKanbanMoveValidation: Equatable {
    case allowed
    case missingTargetAccount
    case rssFeedToMailAccount
    case mailThreadToRssAccount
}

func iosKanbanMoveValidation(threadIsRss: Bool, targetAccount: AccountSummary?) -> IosKanbanMoveValidation {
    guard let targetAccount else { return .missingTargetAccount }
    let targetIsRss = MailStateKt.accountSummaryIsRss(account: targetAccount)
    if threadIsRss && !targetIsRss {
        return .rssFeedToMailAccount
    }
    if !threadIsRss && targetIsRss {
        return .mailThreadToRssAccount
    }
    return .allowed
}

func iosKanbanMoveValidationStatus(_ validation: IosKanbanMoveValidation) -> String {
    switch validation {
    case .allowed:
        return ""
    case .missingTargetAccount:
        return String(localized: "mobile.ios.moveFailed")
    case .rssFeedToMailAccount:
        return String(localized: "mobile.ios.rssFeedsCanOnlyMoveToRss")
    case .mailThreadToRssAccount:
        return String(localized: "mobile.ios.mailThreadsCannotMoveIntoRss")
    }
}

func kanbanStarredItemsMatching(_ items: [StarredItemSummary], query: String) -> [StarredItemSummary] {
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        return items.sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
    }
    let needle = trimmed.lowercased()
    return items
        .filter { item in
            [
                item.subject,
                item.sender,
                item.preview,
                item.accountId,
                item.folder,
            ].contains { $0.localizedCaseInsensitiveContains(needle) }
        }
        .sorted { $0.dateEpochSeconds > $1.dateEpochSeconds }
}

extension ContentView {
    func openAddAccountSetup() {
        focusedAccountSettingsId = ""
        selectedTab = .accounts
    }

    func openNewCompose() {
        clearComposeDraftState()
        composeReturnTab = iosComposeReturnTab(from: selectedTab)
        selectedTab = .compose
    }

    @ToolbarContentBuilder
    var iosCommandPaletteToolbar: some ToolbarContent {
        ToolbarItem(placement: .primaryAction) {
            Button {
                presentCommandPalette()
            } label: {
                Label(String(localized: "palette.label"), systemImage: "command")
            }
            .keyboardShortcut("k", modifiers: [.command])
        }
    }

    func presentCommandPalette() {
        commandPaletteQuery = ""
        isCommandPalettePresented = true
    }

    func handleGlobalShortcut(_ press: KeyPress) -> KeyPress.Result {
        guard press.modifiers.contains(.command) || press.modifiers.contains(.control) else {
            return .ignored
        }
        let key = press.characters.lowercased()

        if let slot = Int(key), (1 ... 9).contains(slot) {
            return navigateIosRailShortcut(slot: slot) ? .handled : .ignored
        }

        switch key {
        case "k" where !press.modifiers.contains(.shift):
            presentCommandPalette()
            return .handled
        case "n" where !press.modifiers.contains(.shift):
            openNewCompose()
            return .handled
        case "e" where !press.modifiers.contains(.shift):
            guard iosFullReplyShortcutCanOpen(selectedTab: selectedTab, thread: selectedCoreThread) else {
                return .ignored
            }
            openQuickReplyInFullEditor()
            return .handled
        case "r" where press.modifiers.contains(.shift):
            selectedTab = .mail
            syncSelectedAccount()
            return .handled
        case "v" where press.modifiers.contains(.shift):
            if selectedTab == .kanban {
                selectedTab = .mail
                focusMailShortcutSurfaceIfIdle()
            } else {
                selectedTab = .kanban
                ensureKanbanDefaults()
                loadActiveKanbanBoard(refresh: false)
            }
            return .handled
        case "w" where !press.modifiers.contains(.shift):
            guard selectedTab == .compose else { return .ignored }
            closeComposeSurface()
            return .handled
        case "f":
            selectedTab = .mail
            if press.modifiers.contains(.shift) || selectedCoreThread == nil {
                mailFocusedField = .mailboxSearch
            } else {
                threadSearchPresented = true
                mailFocusedField = .threadSearch
            }
            return .handled
        case ",":
            selectedTab = .accounts
            return .handled
        case "?" where press.modifiers.contains(.shift):
            selectedTab = .accounts
            return .handled
        default:
            return .ignored
        }
    }

    func navigateIosRailShortcut(slot: Int) -> Bool {
        var target = slot - 1
        if showUnifiedInbox {
            if target == 0 {
                selectMailbox(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
                return true
            }
            target -= 1
        }

        let boards = kanbanBoards
        if boards.indices.contains(target) {
            selectedTab = .kanban
            selectKanbanBoard(boards[target].id)
            return true
        }
        target -= boards.count

        let accounts = visibleNavigationAccounts
        guard accounts.indices.contains(target) else {
            return false
        }
        selectMailbox(accountId: accounts[target].id, folderId: iosInboxFolderId)
        return true
    }

    var iosCommandPaletteCommands: [IosCommand] {
        var commands: [IosCommand] = [
            IosCommand(id: "compose.new", label: String(localized: "composer.actions.newMessage"), keywords: "compose write email new", systemImage: "square.and.pencil", active: selectedTab == .compose, role: nil) {
                openNewCompose()
            },
            IosCommand(id: "mail.sync", label: String(localized: "mobile.mail.syncMailbox"), keywords: "sync refresh fetch check", systemImage: "arrow.clockwise", active: false, role: nil) {
                selectedTab = .mail
                syncSelectedAccount()
            },
            IosCommand(id: "mail.markAllRead", label: String(localized: "threads.actions.markAllAsRead"), keywords: "clear unread seen", systemImage: "envelope.open", active: false, role: nil) {
                selectedTab = .mail
                markSelectedMailboxAllRead()
            },
            IosCommand(id: "search.thread", label: String(localized: "chat.searchThread"), keywords: "find conversation current thread", systemImage: "magnifyingglass", active: selectedTab == .mail && threadSearchPresented, role: nil) {
                selectedTab = .mail
                if selectedCoreThread == nil {
                    mailFocusedField = .mailboxSearch
                } else {
                    threadSearchPresented = true
                    mailFocusedField = .threadSearch
                }
            },
            IosCommand(id: "search.global", label: String(localized: "threads.searchMessages"), keywords: "global mailbox search all messages", systemImage: "text.magnifyingglass", active: selectedTab == .mail && mailFocusedField == .mailboxSearch, role: nil) {
                selectedTab = .mail
                mailFocusedField = .mailboxSearch
            },
            IosCommand(id: "settings.open", label: String(localized: "mobile.tabs.accounts"), keywords: "settings preferences accounts config", systemImage: "gearshape", active: selectedTab == .accounts, role: nil) {
                selectedTab = .accounts
            },
            IosCommand(id: "account.add", label: String(localized: "accounts.actions.addAccount"), keywords: "connect account mailbox feed", systemImage: "person.badge.plus", active: selectedTab == .accounts && focusedAccountSettingsId.isEmpty, role: nil) {
                openAddAccountSetup()
            },
            IosCommand(id: "shortcuts.help", label: String(localized: "shortcuts.title"), keywords: "keyboard shortcuts keys help", systemImage: "keyboard", active: selectedTab == .accounts, role: nil) {
                selectedTab = .accounts
            },
            IosCommand(id: "view.mail", label: "Go to: \(String(localized: "mobile.tabs.mail"))", keywords: "mail inbox list conversation", systemImage: "tray", active: selectedTab == .mail, role: nil) {
                selectedTab = .mail
                focusMailShortcutSurfaceIfIdle()
            },
            IosCommand(id: "view.starred", label: "Go to: \(String(localized: "mobile.tabs.starred"))", keywords: "starred favorites", systemImage: "star", active: selectedTab == .starred, role: nil) {
                selectedTab = .starred
            },
            IosCommand(id: "view.kanban", label: "Go to: \(String(localized: "mobile.tabs.kanban"))", keywords: "kanban board columns", systemImage: "rectangle.3.group", active: selectedTab == .kanban, role: nil) {
                selectedTab = .kanban
                ensureKanbanDefaults()
                loadActiveKanbanBoard(refresh: false)
            },
            IosCommand(id: "view.toggle", label: "Toggle: \(String(localized: "mobile.tabs.mail")) / \(String(localized: "mobile.tabs.kanban"))", keywords: "toggle switch mail kanban board", systemImage: "rectangle.2.swap", active: false, role: nil) {
                if selectedTab == .kanban {
                    selectedTab = .mail
                    focusMailShortcutSurfaceIfIdle()
                } else {
                    selectedTab = .kanban
                    ensureKanbanDefaults()
                    loadActiveKanbanBoard(refresh: false)
                }
            },
            IosCommand(id: "kanban.create", label: String(localized: "kanban.actions.addBoard"), keywords: "new board columns", systemImage: "plus.rectangle.on.rectangle", active: false, role: nil) {
                selectedTab = .kanban
                createKanbanBoard()
            },
        ]
        if selectedTab == .compose {
            commands.append(IosCommand(id: "tab.close", label: String(localized: "buttons.close"), keywords: "close tab dismiss compose editor", systemImage: "xmark", active: false, role: nil) {
                closeComposeSurface()
            })
        }
        commands.append(contentsOf: iosFilterCommands)
        commands.append(contentsOf: iosThemeCommands)
        commands.append(contentsOf: iosConversationCommands)
        commands.append(contentsOf: iosBoardCommands)
        commands.append(contentsOf: iosAccountCommands)
        commands.append(contentsOf: iosFolderCommands)
        return commands
    }

    var iosFilterCommands: [IosCommand] {
        let current = selectedTab == .kanban ? kanbanFilter : mailFilter
        return IosFilterMode.allCases.map { mode in
            IosCommand(id: "filter.\(mode.rawValue)", label: "\(String(localized: "filters.label")): \(mode.label)", keywords: "filter \(mode.rawValue)", systemImage: mode == .starred ? "star" : (mode == .unread ? "envelope.badge" : "tray.full"), active: current == mode, role: nil) {
                if selectedTab == .kanban {
                    kanbanFilter = mode
                    loadActiveKanbanBoard(refresh: false)
                } else {
                    selectedTab = .mail
                    mailFilter = mode
                    searchSelectedMailbox()
                }
            }
        }
    }

    var iosThemeCommands: [IosCommand] {
        iosThemeOptions.map { option in
            IosCommand(id: "theme.\(option.id)", label: "\(String(localized: "settings.pages.appearance")): \(option.name)", keywords: "theme appearance color \(option.id)", systemImage: option.dark == true ? "moon" : (option.dark == false ? "sun.max" : "circle.lefthalf.filled"), active: appearanceMode == option.id, role: nil) {
                appearanceMode = option.id
            }
        }
    }

    var iosConversationCommands: [IosCommand] {
        guard let thread = selectedCoreThread else { return [] }
        var commands: [IosCommand] = []
        commands.append(IosCommand(id: "thread.next", label: String(localized: "shortcuts.nextThread"), keywords: "next newer later down j navigation", systemImage: "chevron.down", active: false, role: nil) {
            selectAdjacentThreadFromCommandPalette(1)
        })
        commands.append(IosCommand(id: "thread.prev", label: String(localized: "shortcuts.previousThread"), keywords: "previous older earlier up k navigation", systemImage: "chevron.up", active: false, role: nil) {
            selectAdjacentThreadFromCommandPalette(-1)
        })
        if !isRssThread(thread) {
            commands.append(IosCommand(id: "reply.focus", label: String(localized: "shortcuts.focusQuickReply"), keywords: "reply quick respond", systemImage: "arrowshape.turn.up.left", active: mailFocusedField == .quickReply, role: nil) {
                selectedTab = .mail
                mailFocusedField = .quickReply
            })
            commands.append(IosCommand(id: "compose.replyFull", label: String(localized: "composer.actions.openFullEditor"), keywords: "reply full editor compose", systemImage: "arrow.up.left.and.arrow.down.right", active: false, role: nil) {
                openQuickReplyInFullEditor()
            })
            commands.append(IosCommand(id: "thread.archive", label: String(localized: "threads.actions.archiveThread"), keywords: "archive remove inbox", systemImage: "archivebox", active: false, role: nil) {
                archiveThread(thread)
            })
        }
        commands.append(IosCommand(id: "thread.delete", label: iosConversationDeleteCommandLabel(thread), keywords: isRssThread(thread) ? "delete remove unsubscribe feed rss" : "delete trash remove", systemImage: "trash", active: false, role: .destructive) {
            if isRssThread(thread) {
                removeRssFeed(thread)
            } else {
                deleteThread(thread)
            }
        })
        commands.append(IosCommand(id: "thread.star", label: thread.starred ? String(localized: "chat.unstar") : String(localized: "chat.star"), keywords: "star favorite flag", systemImage: thread.starred ? "star.slash" : "star", active: thread.starred, role: nil) {
            markThreadStarred(thread, starred: !thread.starred)
        })
        commands.append(IosCommand(id: "thread.unread", label: threadReadToggleActionLabel(thread), keywords: "unread seen read", systemImage: thread.unread ? "envelope.open" : "envelope.badge", active: thread.unread, role: nil) {
            markThreadRead(thread, seen: thread.unread)
        })
        commands.append(IosCommand(id: "thread.details", label: String(localized: "chat.conversationDetails"), keywords: "details participants attachments", systemImage: "info.circle", active: conversationDetailsPresented, role: nil) {
            selectedTab = .mail
            conversationDetailsPresented = true
        })
        return commands
    }

    var iosBoardCommands: [IosCommand] {
        kanbanBoards.map { board in
            IosCommand(id: "kanban.\(board.id)", label: "Go to: \(board.name)", keywords: "kanban board \(board.name)", systemImage: "rectangle.3.group", active: selectedTab == .kanban && activeKanbanBoardId == board.id, role: nil) {
                selectedTab = .kanban
                selectKanbanBoard(board.id)
            }
        }
    }

    var iosAccountCommands: [IosCommand] {
        var commands: [IosCommand] = []
        if showUnifiedInbox, coreAccounts.count > 1 {
            commands.append(IosCommand(id: "account.unified", label: "Go to: \(String(localized: "kanban.columns.unifiedInbox"))", keywords: "all accounts unified inbox", systemImage: "person.2", active: selectedTab == .mail && selectedCoreAccountId == iosUnifiedAccountId, role: nil) {
                selectMailbox(accountId: iosUnifiedAccountId, folderId: iosInboxFolderId)
            })
        }
        commands.append(contentsOf: visibleNavigationAccounts.map { account in
            IosCommand(id: "account.\(account.id)", label: "Go to: \(accountLabel(account))", keywords: "account mailbox \(account.email)", systemImage: MailStateKt.accountSummaryIsRss(account: account) ? "dot.radiowaves.left.and.right" : "envelope", active: selectedTab == .mail && selectedCoreAccountId == account.id, role: nil) {
                selectMailbox(accountId: account.id, folderId: iosInboxFolderId)
            }
        })
        return commands
    }

    var iosFolderCommands: [IosCommand] {
        guard selectedCoreAccountId != iosUnifiedAccountId else { return [] }
        return coreFolders.map { folder in
            IosCommand(id: "folder.\(folder.name)", label: "Folder: \(folder.name)", keywords: "mailbox folder \(folder.name)", systemImage: "folder", active: selectedTab == .mail && selectedCoreFolder == folder.name, role: nil) {
                selectMailbox(accountId: selectedCoreAccountId, folderId: folder.name)
            }
        }
    }

    func selectMailbox(accountId: String, folderId: String) {
        selectedTab = .mail
        if selectedCoreAccountId != accountId {
            pendingThreadUndo = nil
            selectedCoreAccountId = accountId
            coreFolders = []
            coreThreads = []
            selectedMailThreadIds = []
            selectedCoreThread = nil
            coreMessages = []
            mailboxCursor = ""
            mailboxAccountCursors = [:]
        }
        selectedCoreFolder = folderId
        searchSelectedMailbox()
        focusMailShortcutSurfaceIfIdle()
    }
}

func composeBodyAsSimpleHtml(_ body: String) -> String {
    let escaped = escapeComposeHtml(body)
    let blocks = escaped
        .components(separatedBy: "\n\n")
        .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        .filter { !$0.isEmpty }

    guard !blocks.isEmpty else { return "<p><br></p>" }

    return blocks.map(composeMarkdownBlockAsHtml).joined()
}

private func escapeComposeHtml(_ value: String) -> String {
    value
        .replacingOccurrences(of: "&", with: "&amp;")
        .replacingOccurrences(of: "<", with: "&lt;")
        .replacingOccurrences(of: ">", with: "&gt;")
        .replacingOccurrences(of: "\"", with: "&quot;")
        .replacingOccurrences(of: "'", with: "&#39;")
}

private func composeMarkdownBlockAsHtml(_ block: String) -> String {
    let lines = block.components(separatedBy: .newlines)
    if let heading = composeHeadingHtml(lines) {
        return heading
    }
    if let list = composeListHtml(lines: lines, pattern: #"^\s*[-*]\s+(.+)$"#, tag: "ul") {
        return list
    }
    if let list = composeListHtml(lines: lines, pattern: #"^\s*\d+[.)]\s+(.+)$"#, tag: "ol") {
        return list
    }
    if lines.allSatisfy({ $0.trimmingCharacters(in: .whitespaces).hasPrefix("&gt;") }) {
        let quoted = lines
            .map { line in
                let text = line.trimmingCharacters(in: .whitespaces)
                    .replacingOccurrences(of: #"^&gt;\s?"#, with: "", options: .regularExpression)
                return composeInlineMarkdownAsHtml(text)
            }
            .joined(separator: "<br>")
        return "<blockquote>\(quoted)</blockquote>"
    }
    let content = lines
        .map { $0.isEmpty ? "<br>" : composeInlineMarkdownAsHtml($0) }
        .joined(separator: "<br>")
    return "<p>\(content.isEmpty ? "<br>" : content)</p>"
}

private func composeHeadingHtml(_ lines: [String]) -> String? {
    guard lines.count == 1 else { return nil }
    let line = lines[0].trimmingCharacters(in: .whitespaces)
    guard let range = line.range(of: #"^(#{1,3})\s+(.+)$"#, options: .regularExpression) else { return nil }
    let matched = String(line[range])
    let level = matched.prefix { $0 == "#" }.count
    let text = matched.drop { $0 == "#" || $0 == " " }
    return "<h\(level)>\(composeInlineMarkdownAsHtml(String(text)))</h\(level)>"
}

private func composeListHtml(lines: [String], pattern: String, tag: String) -> String? {
    var items: [String] = []
    for line in lines {
        guard let range = line.range(of: pattern, options: .regularExpression) else {
            return nil
        }
        let matched = String(line[range])
        let text = matched.replacingOccurrences(of: pattern, with: "$1", options: .regularExpression)
        items.append("<li>\(composeInlineMarkdownAsHtml(text))</li>")
    }
    return "<\(tag)>\(items.joined())</\(tag)>"
}

private func composeInlineMarkdownAsHtml(_ value: String) -> String {
    var output = value
    let replacements: [(String, String)] = [
        (#"`([^`]+)`"#, "<code>$1</code>"),
        (#"!\[([^\]]*)\]\((cid:[A-Za-z0-9._%+\-@]+)\)"#, #"<img src="$2" alt="$1">"#),
        (#"\[([^\]]+)\]\((https?://[^\s)]+)\)"#, #"<a href="$2">$1</a>"#),
        (#"\*\*([^*]+)\*\*"#, "<strong>$1</strong>"),
        (#"__([^_]+)__"#, "<u>$1</u>"),
        (#"~~([^~]+)~~"#, "<s>$1</s>"),
        (#"(^|[^*])\*([^*\n]+)\*([^*]|$)"#, "$1<em>$2</em>$3")
    ]
    for (pattern, replacement) in replacements {
        output = output.replacingOccurrences(of: pattern, with: replacement, options: .regularExpression)
    }
    return output
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
    guard let bytes else { return String(localized: "mobile.ios.notLoaded") }
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
        return String(localized: "mobile.ios.oauthCodeReceived")
    }

    let error = OAuthFlowKt.oauthCallbackValidationErrorForRedirect(
        rawUrl: rawUrl,
        expectedState: expectedState,
        redirectUri: redirectUri
    ) ?? String(localized: "mobile.ios.oauthCallbackFailed")
    let prefix = String(localized: "mobile.ios.oauthCallbackFailed").trimmingCharacters(in: CharacterSet(charactersIn: ". "))
    return "\(prefix): \(error)"
}

func saveIosPendingOAuthFlow(_ flow: IosPendingOAuthFlow, defaults: UserDefaults = .standard) {
    guard let data = try? JSONEncoder().encode(flow) else { return }
    defaults.set(data, forKey: iosPendingOAuthFlowKey)
}

func loadIosPendingOAuthFlow(defaults: UserDefaults = .standard) -> IosPendingOAuthFlow? {
    guard let data = defaults.data(forKey: iosPendingOAuthFlowKey) else { return nil }
    return try? JSONDecoder().decode(IosPendingOAuthFlow.self, from: data)
}

func clearIosPendingOAuthFlow(defaults: UserDefaults = .standard) {
    defaults.removeObject(forKey: iosPendingOAuthFlowKey)
}

func iosOAuthInfoValue(_ key: String, infoDictionary: [String: Any]? = Bundle.main.infoDictionary) -> String {
    let value = (infoDictionary?[key] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    guard !value.isEmpty, !value.hasPrefix("$(") else { return "" }
    return value
}

func iosResolvedOAuthClientId(
    provider: String,
    configuredClientId: String,
    infoDictionary: [String: Any]? = Bundle.main.infoDictionary
) -> String {
    let provider = provider.lowercased()
    let configured = configuredClientId.trimmingCharacters(in: .whitespacesAndNewlines)
    if provider == "outlook" {
        if !configured.isEmpty, configured != iosDefaultGoogleOAuthClientId {
            return configured
        }
        return iosOAuthInfoValue("MERON_OUTLOOK_CLIENT_ID", infoDictionary: infoDictionary)
    }
    if !configured.isEmpty {
        return configured
    }
    return iosOAuthInfoValue("MERON_GOOGLE_CLIENT_ID", infoDictionary: infoDictionary).isEmpty
        ? iosDefaultGoogleOAuthClientId
        : iosOAuthInfoValue("MERON_GOOGLE_CLIENT_ID", infoDictionary: infoDictionary)
}

func iosResolvedOAuthClientSecret(
    provider: String,
    configuredClientSecret: String,
    infoDictionary: [String: Any]? = Bundle.main.infoDictionary
) -> String {
    let configured = configuredClientSecret.trimmingCharacters(in: .whitespacesAndNewlines)
    if !configured.isEmpty {
        return configured
    }
    return iosOAuthInfoValue(
        provider.lowercased() == "outlook" ? "MERON_OUTLOOK_CLIENT_SECRET" : "MERON_GOOGLE_CLIENT_SECRET",
        infoDictionary: infoDictionary
    )
}

func iosResolvedOAuthRedirectUri(
    provider: String,
    configuredRedirectUri: String,
    infoDictionary: [String: Any]? = Bundle.main.infoDictionary
) -> String {
    let configured = configuredRedirectUri.trimmingCharacters(in: .whitespacesAndNewlines)
    let key = provider.lowercased() == "outlook" ? "MERON_OUTLOOK_REDIRECT_URI" : "MERON_GOOGLE_REDIRECT_URI"
    let bundled = iosOAuthInfoValue(key, infoDictionary: infoDictionary)
    if !bundled.isEmpty {
        return bundled
    }
    return configured.isEmpty ? OAuthFlowKt.defaultOAuthRedirectUri() : configured
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
