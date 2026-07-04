import Foundation

/// Rolling on-disk log of background sync events, mirroring the Android
/// implementation, so a user seeing repeated sync failures can share what
/// happened without needing a device console attached.
enum IosSyncDiagnosticLog {
    private static let fileName = "sync-diagnostic.log"
    private static let maxLines = 500

    /// Mirrors IosAppPreferences("meron_app") + SYNC_DIAGNOSTIC_LOG_ENABLED_PREF.
    private static let enabledDefaultsKey = "meron_app.sync_diagnostic_log_enabled_v1"

    private static var isEnabled: Bool {
        UserDefaults.standard.bool(forKey: enabledDefaultsKey)
    }

    private static var logFileURL: URL {
        URL(fileURLWithPath: IosAppPaths.mobileDataDirectory()).appendingPathComponent(fileName)
    }

    private static let timestampFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        return formatter
    }()

    static func append(_ message: String) {
        guard isEnabled else { return }
        let url = logFileURL
        let timestamp = timestampFormatter.string(from: Date())
        let existing = (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        var lines = existing.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
        lines.append("\(timestamp) \(message)")
        if lines.count > maxLines {
            lines = Array(lines.suffix(maxLines))
        }
        try? (lines.joined(separator: "\n") + "\n").write(to: url, atomically: true, encoding: .utf8)
    }
}

/// Masks email addresses before they're written to the shareable diagnostic
/// log: keeps the domain for context, never the full address.
enum IosSyncLogRedaction {
    private static let emailRegex = try! NSRegularExpression(
        pattern: "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}"
    )

    static func redactEmail(_ email: String) -> String {
        guard let at = email.firstIndex(of: "@") else { return "***" }
        return "\(email[email.startIndex])***\(email[at...])"
    }

    static func redactMessage(_ message: String) -> String {
        var result = message
        let range = NSRange(message.startIndex..., in: message)
        let matches = emailRegex.matches(in: message, range: range).reversed()
        for match in matches {
            guard let matchRange = Range(match.range, in: message) else { continue }
            result.replaceSubrange(matchRange, with: redactEmail(String(message[matchRange])))
        }
        return result
    }

    /// Redacted label for an account — a masked email, or the bare account id
    /// when there's no email. Never a secret (token/password).
    static func accountLabel(_ account: [String: Any]) -> String {
        if let email = account["email"] as? String, !email.isEmpty {
            return redactEmail(email)
        }
        return account["id"] as? String ?? ""
    }
}
