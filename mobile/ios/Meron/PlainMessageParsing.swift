import Foundation
import MeronUI
import SwiftUI

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
