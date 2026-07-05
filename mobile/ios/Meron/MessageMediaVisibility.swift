import Foundation
import MeronUI

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

func draftAttachmentSizeLabel(_ attachment: DraftAttachment) -> String {
    ByteCountFormatter.string(fromByteCount: attachment.sizeBytes, countStyle: .file)
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
