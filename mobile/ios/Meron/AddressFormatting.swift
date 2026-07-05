import CryptoKit
import Foundation
import MeronUI
import SwiftUI

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
