import Foundation
import MeronUI

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
    subject _: String,
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
        (#"(^|[^*])\*([^*\n]+)\*([^*]|$)"#, "$1<em>$2</em>$3"),
    ]
    for (pattern, replacement) in replacements {
        output = output.replacingOccurrences(of: pattern, with: replacement, options: .regularExpression)
    }
    return output
}
