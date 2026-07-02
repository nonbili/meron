import CryptoKit
import Foundation
import MeronUI
import SwiftUI
import UIKit

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
struct IosCommand: Identifiable {
    let id: String
    let label: String
    let keywords: String
    let systemImage: String
    let active: Bool
    let role: ButtonRole?
    let action: () -> Void
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
        feedUrl: "",
        threadId: item.threadId
    )
}

func starredItemReaderMessage(for item: StarredItemSummary) -> MessageBody {
    MessageBody(
        id: item.id,
        folderId: item.folder,
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
        bodyMissing: false,
        attachments: [],
        sendStatus: .none
    )
}

func starredKanbanReaderMessage(for thread: ThreadSummary) -> MessageBody {
    MessageBody(
        id: thread.id,
        folderId: thread.folder,
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
        bodyMissing: false,
        attachments: [],
        sendStatus: .none
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
