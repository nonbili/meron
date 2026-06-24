import Foundation

func localizedCatalogString(
    _ key: String,
    localeIdentifier: String? = nil,
    args: [String: Any] = [:],
    bundle: Bundle = .main
) -> String {
    let template = localizedCatalogTemplate(key, localeIdentifier: localeIdentifier, bundle: bundle)
    return formatLocalizedCatalogTemplate(template, args: args)
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

private func formatLocalizedCatalogTemplate(_ template: String, args: [String: Any]) -> String {
    if let plural = parseLocalizedCatalogPlural(template) {
        let count = localizedCatalogInt(args[plural.argument]) ?? 0
        let selected = plural.variants[count == 1 ? "one" : "other"]
            ?? plural.variants["other"]
            ?? template
        return replaceLocalizedCatalogPlaceholders(selected, args: args)
    }
    return replaceLocalizedCatalogPlaceholders(template, args: args)
}

private struct LocalizedCatalogPlural {
    let argument: String
    let variants: [String: String]
}

private func parseLocalizedCatalogPlural(_ template: String) -> LocalizedCatalogPlural? {
    guard template.hasPrefix("{") else { return nil }
    let prefixPattern = #"^\{([A-Za-z][A-Za-z0-9_]*)\s*,\s*plural\s*,"#
    guard
        let regex = try? NSRegularExpression(pattern: prefixPattern),
        let match = regex.firstMatch(in: template, range: NSRange(template.startIndex..., in: template)),
        let argumentRange = Range(match.range(at: 1), in: template),
        let matchRange = Range(match.range, in: template)
    else {
        return nil
    }

    var index = matchRange.upperBound
    var variants: [String: String] = [:]
    while index < template.endIndex {
        while index < template.endIndex, template[index].isWhitespace {
            index = template.index(after: index)
        }
        let categoryStart = index
        while index < template.endIndex, template[index].isLetter {
            index = template.index(after: index)
        }
        guard categoryStart < index else { break }
        let category = String(template[categoryStart ..< index])
        while index < template.endIndex, template[index].isWhitespace {
            index = template.index(after: index)
        }
        guard index < template.endIndex, template[index] == "{" else { break }
        let branch = readLocalizedCatalogBraceBranch(template, start: index)
        variants[category] = branch.value
        index = branch.next
    }
    return LocalizedCatalogPlural(argument: String(template[argumentRange]), variants: variants)
}

private func readLocalizedCatalogBraceBranch(
    _ template: String,
    start: String.Index
) -> (value: String, next: String.Index) {
    var depth = 0
    var index = start
    while index < template.endIndex {
        if template[index] == "{" {
            depth += 1
        } else if template[index] == "}" {
            depth -= 1
            if depth == 0 {
                return (String(template[template.index(after: start) ..< index]), template.index(after: index))
            }
        }
        index = template.index(after: index)
    }
    return (String(template[template.index(after: start)...]), template.endIndex)
}

private func replaceLocalizedCatalogPlaceholders(_ value: String, args: [String: Any]) -> String {
    var out = value
    for (key, value) in args {
        out = out.replacingOccurrences(of: "{\(key)}", with: "\(value)")
    }
    return out
}

private func localizedCatalogInt(_ value: Any?) -> Int? {
    if let value = value as? Int { return value }
    if let value = value as? NSNumber { return value.intValue }
    if let value = value as? String { return Int(value) }
    return nil
}
