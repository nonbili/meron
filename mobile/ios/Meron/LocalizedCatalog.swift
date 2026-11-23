import Foundation

func localizedCatalogString(
    _ key: String,
    localeIdentifier: String? = nil,
    args: [String: Any] = [:],
    bundle: Bundle = .main
) -> String {
    let template = localizedCatalogTemplate(key, localeIdentifier: localeIdentifier, bundle: bundle)
    let language = localeIdentifier ?? bundle.preferredLocalizations.first ?? "en"
    return formatLocalizedCatalogTemplate(template, args: args, localeIdentifier: language)
}

private func localizedCatalogTemplate(
    _ key: String,
    localeIdentifier: String?,
    bundle: Bundle
) -> String {
    guard
        let localeIdentifier,
        let path = bundle.path(forResource: localeIdentifier, ofType: "lproj"),
        let localizedBundle = Bundle(path: path)
    else {
        return bundle.localizedString(forKey: key, value: nil, table: nil)
    }
    return localizedBundle.localizedString(forKey: key, value: nil, table: nil)
}

/// Minimal ICU MessageFormat: `{name}`, `{name, number}` and
/// `{name, plural, =0 {…} one {…} other {…}}` anywhere in the message, any
/// number of times, nested, with `#` inside a plural branch replaced by its count.
/// Anything else in braces is kept literally.
private func formatLocalizedCatalogTemplate(
    _ template: String,
    args: [String: Any],
    localeIdentifier: String
) -> String {
    let language = localeIdentifier.replacingOccurrences(of: "_", with: "-").lowercased()
    return formatLocalizedCatalogMessage(Array(template), args: args, language: language, pound: nil)
}

private func formatLocalizedCatalogMessage(
    _ chars: [Character],
    args: [String: Any],
    language: String,
    pound: String?
) -> String {
    var out = ""
    var index = 0
    while index < chars.count {
        let char = chars[index]
        if char == "{" {
            guard let end = localizedCatalogMatchingBrace(chars, start: index) else {
                out += String(chars[index...])
                return out
            }
            let body = String(chars[(index + 1) ..< end])
            out += formatLocalizedCatalogArgument(body, args: args, language: language)
                ?? String(chars[index ... end])
            index = end + 1
        } else if char == "#", let pound {
            out += pound
            index += 1
        } else {
            out.append(char)
            index += 1
        }
    }
    return out
}

private let localizedCatalogArgumentRegex = try? NSRegularExpression(
    pattern: #"^\s*([A-Za-z][A-Za-z0-9_]*)\s*(?:,\s*([A-Za-z]+)\s*(?:,([\s\S]*))?)?$"#
)

/// Format the inside of one `{…}`, or nil when it is not an argument we understand.
private func formatLocalizedCatalogArgument(
    _ body: String,
    args: [String: Any],
    language: String
) -> String? {
    guard
        let regex = localizedCatalogArgumentRegex,
        let match = regex.firstMatch(in: body, range: NSRange(body.startIndex..., in: body)),
        let nameRange = Range(match.range(at: 1), in: body)
    else {
        return nil
    }
    let name = String(body[nameRange])
    let type = Range(match.range(at: 2), in: body).map { String(body[$0]) } ?? ""
    switch type {
    case "", "number":
        return args[name].map { "\($0)" } ?? ""
    case "plural":
        let source = Range(match.range(at: 3), in: body).map { String(body[$0]) } ?? ""
        let variants = parseLocalizedCatalogPluralVariants(Array(source))
        let value = args[name]
        let count = localizedCatalogInt(value) ?? 0
        guard let branch = variants["=\(count)"]
            ?? variants[localizedCatalogPluralCategory(language, count)]
            ?? variants["other"]
        else {
            return nil
        }
        return formatLocalizedCatalogMessage(Array(branch), args: args, language: language, pound: value.map { "\($0)" } ?? "\(count)")
    default:
        return nil
    }
}

private func parseLocalizedCatalogPluralVariants(_ chars: [Character]) -> [String: String] {
    var variants: [String: String] = [:]
    var index = 0
    while index < chars.count {
        while index < chars.count, chars[index].isWhitespace {
            index += 1
        }
        let selectorStart = index
        while index < chars.count, chars[index].isLetter || chars[index].isNumber || chars[index] == "=" || chars[index] == "-" {
            index += 1
        }
        guard selectorStart < index else { break }
        let selector = String(chars[selectorStart ..< index])
        while index < chars.count, chars[index].isWhitespace {
            index += 1
        }
        guard index < chars.count, chars[index] == "{",
              let end = localizedCatalogMatchingBrace(chars, start: index)
        else { break }
        variants[selector] = String(chars[(index + 1) ..< end])
        index = end + 1
    }
    return variants
}

/// Index of the `}` closing the `{` at `start`, or nil.
private func localizedCatalogMatchingBrace(_ chars: [Character], start: Int) -> Int? {
    var depth = 0
    for index in start ..< chars.count {
        if chars[index] == "{" {
            depth += 1
        } else if chars[index] == "}" {
            depth -= 1
            if depth == 0 { return index }
        }
    }
    return nil
}

/// CLDR cardinal category for `language` (a lowercase `lang` or `lang-region` tag).
private func localizedCatalogPluralCategory(_ language: String, _ count: Int) -> String {
    let n = count.magnitude
    let lastTwo = n % 100
    let last = n % 10
    switch language.components(separatedBy: "-")[0] {
    case "fr":
        return n <= 1 ? "one" : "other"
    case "pt":
        return n == 1 || (n == 0 && language == "pt-br") ? "one" : "other"
    case "ru", "uk":
        if last == 1, lastTwo != 11 { return "one" }
        if (2 ... 4).contains(last), !(12 ... 14).contains(lastTwo) { return "few" }
        return "many"
    case "pl":
        if n == 1 { return "one" }
        if (2 ... 4).contains(last), !(12 ... 14).contains(lastTwo) { return "few" }
        return "many"
    case "ar":
        if n == 0 { return "zero" }
        if n == 1 { return "one" }
        if n == 2 { return "two" }
        if (3 ... 10).contains(lastTwo) { return "few" }
        if (11 ... 99).contains(lastTwo) { return "many" }
        return "other"
    case "lv":
        if last == 0 || (11 ... 19).contains(lastTwo) { return "zero" }
        if last == 1, lastTwo != 11 { return "one" }
        return "other"
    default:
        return n == 1 ? "one" : "other"
    }
}

private func localizedCatalogInt(_ value: Any?) -> Int? {
    if let value = value as? Int { return value }
    if let value = value as? NSNumber { return value.intValue }
    if let value = value as? String { return Int(value) }
    return nil
}
