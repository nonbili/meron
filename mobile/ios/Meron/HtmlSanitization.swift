import Foundation

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
